mod auth;
pub mod config;
pub mod error;
mod integrations;
pub mod model;
pub mod store;
mod telemetry;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, Method, Uri},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use config::Config;
use error::{ApiError, Result};
use model::*;
use serde_json::{Value, json};
use silicon_accounts_client::AccountsClient;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use store::{Mutation, Prepared, Store, hash, str_field};

pub struct AppState {
    pub config: Config,
    pub store: Mutex<Store>,
    pub accounts: AccountsClient,
    pub http: reqwest::Client,
    pub mutation_gate: tokio::sync::Mutex<()>,
    pub telemetry: Option<Arc<space_station::SpaceClient>>,
    pub identity_cache: Mutex<BTreeMap<String, (std::time::Instant, Identity)>>,
}
pub type Shared = Arc<AppState>;
impl AppState {
    pub fn new(config: Config) -> Result<Shared> {
        config.validate_dev_boundary()?;
        std::fs::create_dir_all(config.data_dir.join("packages"))
            .map_err(|_| ApiError::unavailable("Cannot create Apps data directory."))?;
        let store = Store::open(&config.data_dir.join("apps.sqlite"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&config.data_dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| ApiError::unavailable("Cannot secure Apps data directory."))?;
            std::fs::set_permissions(
                config.data_dir.join("apps.sqlite"),
                std::fs::Permissions::from_mode(0o600),
            )
            .map_err(|_| ApiError::unavailable("Cannot secure Apps database."))?;
        }
        store.connection.execute_batch("CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,access_token TEXT NOT NULL,refresh_token TEXT,expires_at INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS oauth_pending(state TEXT PRIMARY KEY,verifier TEXT NOT NULL,return_to TEXT NOT NULL,expires_at INTEGER NOT NULL);")?;
        {
            let has_origin: bool = store.connection.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('oauth_pending') WHERE name='origin')", [], |r| r.get(0))?;
            if !has_origin {
                store.connection.execute_batch(
                    "ALTER TABLE oauth_pending ADD COLUMN origin TEXT NOT NULL DEFAULT ''",
                )?;
            }
        }
        let accounts =
            AccountsClient::new(&config.accounts_url).map_err(|e| ApiError::bad(e.to_string()))?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| ApiError::bad(e.to_string()))?;
        let telemetry = telemetry::build(&config)?;
        Ok(Arc::new(Self {
            config,
            telemetry,
            identity_cache: Mutex::new(BTreeMap::new()),
            store: Mutex::new(store),
            accounts,
            http,
            mutation_gate: tokio::sync::Mutex::new(()),
        }))
    }
}
pub fn router(state: Shared) -> Router {
    Router::new().route("/health",get(||async{Json(json!({"status":"ok","service":"silicon-apps","version":env!("CARGO_PKG_VERSION")}))})).route("/v1/{*path}",any(handle)).layer(DefaultBodyLimit::max(512*1024*1024)).with_state(state)
}
async fn handle(
    State(s): State<Shared>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let start = std::time::Instant::now();
    let enabled = headers.get("X-Apps-Telemetry").is_none_or(|v| v != "off");
    let route = telemetry::route_template(uri.path());
    let verb = method.to_string();
    let mut result = match dispatch(s.clone(), method, uri, headers, body).await {
        Ok(r) => r,
        Err(e) => e.into_response(),
    };
    result
        .headers_mut()
        .insert("Cache-Control", "private, no-store".parse().unwrap());
    result
        .headers_mut()
        .insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    result
        .headers_mut()
        .insert("Vary", "Authorization, Cookie, Origin".parse().unwrap());
    if enabled {
        telemetry::record(
            &s,
            json!({"source":"silicon-apps.server","version":env!("CARGO_PKG_VERSION"),"step":"api.response","progress":1,"context":{"method":verb,"route":route,"status_code":result.status().as_u16(),"duration_ms":start.elapsed().as_millis()}}),
        );
    }
    result
}

async fn dispatch(
    s: Shared,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response> {
    let path = uri
        .path()
        .strip_prefix("/v1/")
        .ok_or_else(ApiError::missing)?;
    let q: BTreeMap<String, String> =
        serde_urlencoded::from_str(uri.query().unwrap_or_default())
            .map_err(|_| ApiError::bad("Query parameters are invalid."))?;
    if path.starts_with("auth/") {
        if raw.len() > 64 * 1024 {
            return Err(ApiError::bad("Auth requests are limited to 64 KiB."));
        }
        return auth::handle_auth(&s, path, &method, &q, &headers, &raw).await;
    }
    if raw.len() > 2 * 1024 * 1024 && !path.contains("/packages/") && !path.ends_with("/media") {
        return Err(ApiError::bad("JSON requests are limited to 2 MiB."));
    }
    let token = auth::bearer(&s, &headers).await?;
    let who = auth::identity(&s, token.as_deref()).await?;
    if let Some(who) = &who {
        integrations::refresh_identity(&s, who)?;
    }
    if path == "session" && method == Method::GET {
        return Ok(Json(json!({"authenticated":who.is_some(),"account":who})).into_response());
    }
    if path == "targets" && method == Method::GET {
        let c = s.store.lock().unwrap().catalog()?;
        let selected: Vec<_> = q
            .get("targets")
            .map(|v| v.split(',').collect())
            .unwrap_or_default();
        let reach = c
            .platforms
            .values()
            .filter(|targets| targets.iter().any(|t| selected.contains(&t.as_str())))
            .count();
        return Ok(Json(json!({"items":TARGETS.iter().map(|t|json!({"target":t,"population":c.platforms.values().filter(|targets|targets.iter().any(|v|v==t)).count(),"runner_available":s.config.runner_url.is_some()&&s.config.runner_targets.iter().any(|x|x==t)})).collect::<Vec<_>>(),"total_population":c.platforms.len(),"total_reach":reach,"source":"registered_accounts"})).into_response());
    }
    if path.starts_with("apps/availability/") && method == Method::GET {
        let id = path.strip_prefix("apps/availability/").unwrap();
        if id.contains('/') {
            return Err(ApiError::missing());
        }
        let local = s.store.lock().unwrap().catalog()?;
        let available = valid_app_id(id) && !reserved_app_id(id) && !local.apps.contains_key(id);
        let available = available && integrations::registry_available(&s, id).await?;
        return Ok(Json(json!({"available":available})).into_response());
    }
    let p: Vec<_> = path.trim_matches('/').split('/').collect();
    if method == Method::GET {
        if p.len() == 5 && p[0] == "apps" && p[2] == "packages" && p[4] == "download" {
            let app = s.store.lock().unwrap().app(p[1])?;
            if !app.visible(who.as_ref()) {
                return Err(ApiError::missing());
            }
            let pkg = app
                .packages
                .iter()
                .find(|pkg| pkg.id == p[3])
                .ok_or_else(ApiError::missing)?;
            if !app.is_author(who.as_ref())
                && !app.releases.iter().any(|r| r.package_ids.contains(&pkg.id))
            {
                return Err(ApiError::missing());
            }
            let bytes = tokio::fs::read(s.config.data_dir.join("packages").join(&pkg.sha256))
                .await
                .map_err(|_| {
                    ApiError::unavailable("The package artifact is unavailable in storage.")
                })?;
            return Ok((
                [
                    ("Content-Type", "application/gzip"),
                    ("Cache-Control", "private, no-store"),
                ],
                bytes,
            )
                .into_response());
        }
        if p.len() == 4 && p[0] == "apps" && p[2] == "media" {
            let app = s.store.lock().unwrap().app(p[1])?;
            if !app.visible(who.as_ref()) {
                return Err(ApiError::missing());
            }
            if p[3].len() != 64 || !p[3].bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(ApiError::missing());
            }
            let base = s.config.data_dir.join("media").join(p[1]);
            let mime = tokio::fs::read_to_string(base.join(format!("{}.mime", p[3])))
                .await
                .map_err(|_| ApiError::missing())?;
            let bytes = tokio::fs::read(base.join(p[3]))
                .await
                .map_err(|_| ApiError::missing())?;
            return Ok((
                [
                    ("Content-Type", mime),
                    ("X-Content-Type-Options", "nosniff".into()),
                    ("Cache-Control", "private, no-store".into()),
                ],
                bytes,
            )
                .into_response());
        }
        if p.len() == 3 && p[0] == "apps" && p[2] == "webhook" {
            let app = s.store.lock().unwrap().app(p[1])?;
            if !app.is_author(who.as_ref()) {
                return Err(ApiError::forbidden());
            }
            let t = token.as_deref().ok_or_else(ApiError::auth)?;
            let result = s
                .accounts
                .with_token(t)
                .app(p[1])
                .webhook_settings()
                .await
                .map_err(integrations::accounts_error)?;
            return Ok(Json(result).into_response());
        }
        let result = s.store.lock().unwrap().read(path, &q, who.as_ref())?;
        let result = integrations::refresh_view_identities(&s, result).await?;
        return Ok(Json(result).into_response());
    }
    if ![Method::POST, Method::PUT, Method::PATCH, Method::DELETE].contains(&method) {
        return Err(ApiError::missing());
    }
    if !store::mutation_route_exists(method.as_str(), &p) {
        return Err(ApiError::missing());
    }
    auth::check_csrf(&s, &headers)?;
    let key=headers.get("Idempotency-Key").and_then(|v|v.to_str().ok()).filter(|v|(8..=200).contains(&v.len())&&v.bytes().all(|b|(33..=126).contains(&b))).ok_or_else(||ApiError::bad("Every mutation requires an Idempotency-Key containing 8–200 printable non-space characters."))?;
    let _guard = s.mutation_gate.lock().await;
    let actor = who.as_ref().map(|i| i.uuid.as_str()).unwrap_or("anonymous");
    let digest = hash(&raw);
    let fingerprint = format!("{}:{}:{}", method.as_str(), path, digest);
    s.store.lock().unwrap().expire_secret_replays()?;
    s.store
        .lock()
        .unwrap()
        .pending_secret(actor, key, &fingerprint)?;
    if let Some(v) = s.store.lock().unwrap().replay(actor, key, &fingerprint)? {
        return Ok(([("Idempotent-Replayed", "true")], Json(v)).into_response());
    }
    let upload = p.len() == 4 && p[0] == "apps" && p[2] == "packages";
    let media = p.len() == 3 && p[0] == "apps" && p[2] == "media";
    let body: Value = if upload || media || raw.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&raw)
            .map_err(|e| ApiError::bad(format!("Request JSON is invalid: {e}")))?
    };
    // Check app authorization before contacting runners, Accounts or mail providers.
    if p.len() >= 3 && p[0] == "apps" && !matches!(p[2], "installs" | "review") {
        let app = s.store.lock().unwrap().app(p[1])?;
        if !app.is_author(who.as_ref()) {
            return Err(ApiError::forbidden());
        }
        if matches!(p[2], "access" | "admin") && !app.is_admin(who.as_ref()) {
            return Err(ApiError::forbidden());
        }
    }
    if media {
        return upload_media(&s, p[1], actor, key, &fingerprint, &headers, raw).await;
    }
    if path == "telemetry" {
        if headers.get("X-Apps-Telemetry").is_some_and(|v| v == "off")
            || !s.config.telemetry_enabled
        {
            return Ok(Json(json!({"accepted":false,"reason":"opted_out"})).into_response());
        }
        let event = telemetry::frontend_event(&body)?;
        if s.telemetry.is_none() && s.config.telemetry_url.is_none() {
            return Ok(Json(json!({"accepted":false,"reason":"not_configured"})).into_response());
        }
        if s.telemetry.is_some() {
            telemetry::record(&s, event);
        } else {
            let store = s.store.lock().unwrap();
            store.connection.execute("INSERT OR IGNORE INTO outbox(id,kind,body,created_at) VALUES(?1,'telemetry',?2,?3)",rusqlite::params![key,event.to_string(),now()])?;
        }
        let result = json!({"accepted":true});
        s.store.lock().unwrap().connection.execute("INSERT INTO idempotency(actor,key,fingerprint,response,created_at) VALUES(?1,?2,?3,?4,?5)",rusqlite::params![actor,key,fingerprint,result.to_string(),now()])?;
        return Ok(Json(result).into_response());
    }

    let mut prepared = Prepared::default();
    if p == ["apps"] && method == Method::POST
        || p.len() == 4 && p[2] == "secret" && p[3] == "rotate"
    {
        who.as_ref().ok_or_else(ApiError::auth)?;
        let pending = s
            .store
            .lock()
            .unwrap()
            .pending_secret(actor, key, &fingerprint)?;
        if pending.is_none()
            && p.len() == 1
            && !integrations::registry_available(&s, str_field(&body, "app_id")?).await?
        {
            return Err(ApiError::conflict(
                "This app_id already exists in Silicon Accounts. Import existing apps instead of creating a duplicate.",
            ));
        }
        prepared.secret =
            Some(pending.clone().unwrap_or_else(|| {
                format!("sa_app_{}", silicon_accounts_client::random_token(32))
            }));
        // Dry-run complete domain validation before an external app registration or secret change.
        let mut c = s.store.lock().unwrap().catalog()?;
        let mut outbox = vec![];
        store::validate_mutation(
            &mut c,
            &Mutation {
                method: method.as_str(),
                path,
                key,
                who: who.as_ref(),
                body: &body,
                digest: &digest,
                prepared: Prepared {
                    secret: prepared.secret.clone(),
                    ..Default::default()
                },
            },
            &mut outbox,
        )?;
        let id = if p.len() == 1 {
            str_field(&body, "app_id")?
        } else {
            p[1]
        };
        let app = c.apps.get(id).unwrap();
        if pending.is_none() {
            s.store.lock().unwrap().reserve_secret(
                actor,
                key,
                &fingerprint,
                id,
                prepared.secret.as_deref().unwrap(),
            )?;
        }
        integrations::sync_app(&s, app, prepared.secret.as_deref()).await?;
    }
    if p.len() >= 3 && p[0] == "apps" && p[2] == "access" {
        for id in body["account_ids"].as_array().into_iter().flatten() {
            let id = id
                .as_str()
                .ok_or_else(|| ApiError::bad("account_ids must contain c:id or si:id strings."))?;
            prepared
                .identities
                .push(integrations::resolve(&s, id).await?);
        }
    }
    if p.len() == 3 && p[0] == "apps" && p[2] == "invites" && method == Method::POST {
        let to = str_field(&body, "to")?;
        if !to.contains('@') {
            prepared
                .identities
                .push(integrations::resolve(&s, to).await?);
        }
        if (to.contains('@') || to.starts_with("c:"))
            && s.config.mail_url.is_none()
            && s.config.accounts_service_token.is_none()
        {
            return Err(ApiError::unavailable(
                "Invitation email delivery is not configured (APPS_MAIL_URL).",
            ));
        }
    }
    if path == "reports"
        && s.config.mail_url.is_none()
        && s.config.accounts_service_token.is_none()
        && s.config.accounts_service_token.is_none()
    {
        return Err(ApiError::unavailable(
            "Bug report email delivery is not configured (APPS_MAIL_URL).",
        ));
    }
    if upload {
        match integrations::validate_package(&s, p[1], p[3], actor, &raw).await {
            Ok(package) => prepared.package = Some(package),
            Err(error) => {
                if error.status == axum::http::StatusCode::UNPROCESSABLE_ENTITY {
                    s.store.lock().unwrap().validation_failure(
                        p[1],
                        actor,
                        key,
                        &fingerprint,
                        &error,
                    )?;
                }
                return Err(error);
            }
        }
    }
    if p.len() >= 3 && p[0] == "apps" && p[2] == "webhook" {
        let t = token.as_deref().ok_or_else(ApiError::auth)?;
        let session = s.accounts.with_token(t);
        let app = session.app(p[1]);
        let v = if p.len() == 4 && p[3] == "rotate" {
            let secret = app
                .generate_webhook_secret(Some(key))
                .await
                .map_err(integrations::accounts_error)?;
            let raw = serde_json::to_value(secret).unwrap();
            json!({"webhook_secret":raw.get("secret").or_else(||raw.get("webhook_secret"))})
        } else {
            let events: Vec<String> = body
                .get("events")
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| ApiError::bad("events must be a list of strings."))?
                .unwrap_or_else(|| {
                    vec![
                        "id_change".into(),
                        "display_name_change".into(),
                        "pfp_change".into(),
                        "access_removed".into(),
                        "account_deleted".into(),
                    ]
                });
            serde_json::to_value(
                app.set_webhook_events(str_field(&body, "url")?, &events, Some(key))
                    .await
                    .map_err(integrations::accounts_error)?,
            )
            .unwrap()
        };
        prepared.webhook = Some(v);
    }
    // Synchronize ownership before acknowledging it, so a removed author immediately loses Accounts configuration rights.
    let sync_change = p.len() == 2 && p[0] == "apps" && method == Method::PATCH
        || p.len() >= 3 && p[0] == "apps" && matches!(p[2], "authors" | "admin")
        || p.len() == 3 && p[0] == "invites" && p[2] == "accept";
    if sync_change {
        let mut simulated = s.store.lock().unwrap().catalog()?;
        let mut jobs = vec![];
        store::validate_mutation(
            &mut simulated,
            &Mutation {
                method: method.as_str(),
                path,
                key,
                who: who.as_ref(),
                body: &body,
                digest: &digest,
                prepared: Prepared::default(),
            },
            &mut jobs,
        )?;
        let app_id = if p[0] == "apps" {
            p[1].to_owned()
        } else {
            simulated
                .invites
                .iter()
                .find(|i| i.id == p[1])
                .ok_or_else(ApiError::missing)?
                .app_id
                .clone()
        };
        integrations::sync_app(&s, simulated.apps.get(&app_id).unwrap(), None).await?;
    }
    let (response, replayed) = s.store.lock().unwrap().mutate(Mutation {
        method: method.as_str(),
        path,
        key,
        who: who.as_ref(),
        body: &body,
        digest: &digest,
        prepared,
    })?;
    Ok((
        [(
            "Idempotent-Replayed",
            if replayed { "true" } else { "false" },
        )],
        Json(response),
    )
        .into_response())
}
async fn upload_media(
    s: &Shared,
    app_id: &str,
    actor: &str,
    key: &str,
    fingerprint: &str,
    headers: &HeaderMap,
    raw: Bytes,
) -> Result<Response> {
    let mime = headers
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("");
    if ![
        "image/png",
        "image/jpeg",
        "image/webp",
        "image/gif",
        "video/mp4",
        "video/webm",
    ]
    .contains(&mime)
    {
        return Err(ApiError::bad(
            "Media must be PNG, JPEG, WebP, GIF, MP4 or WebM; SVG is not accepted.",
        ));
    }
    if raw.is_empty() || raw.len() > 100 * 1024 * 1024 {
        return Err(ApiError::bad("Media must contain 1 byte to 100 MiB."));
    }
    let signature_ok = match mime {
        "image/png" => raw.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => raw.starts_with(b"\xff\xd8\xff"),
        "image/gif" => raw.starts_with(b"GIF87a") || raw.starts_with(b"GIF89a"),
        "image/webp" => raw.starts_with(b"RIFF") && raw.get(8..12) == Some(b"WEBP"),
        "video/mp4" => raw.get(4..8) == Some(b"ftyp"),
        "video/webm" => raw.starts_with(b"\x1a\x45\xdf\xa3"),
        _ => false,
    };
    if !signature_ok {
        return Err(ApiError::bad(
            "Media bytes do not match the supplied Content-Type.",
        ));
    }
    let digest = hash(&raw);
    let base = s.config.data_dir.join("media").join(app_id);
    tokio::fs::create_dir_all(&base)
        .await
        .map_err(|_| ApiError::unavailable("Cannot create media storage."))?;
    tokio::fs::write(base.join(&digest), &raw)
        .await
        .map_err(|_| ApiError::unavailable("Cannot save media."))?;
    tokio::fs::write(base.join(format!("{digest}.mime")), mime)
        .await
        .map_err(|_| ApiError::unavailable("Cannot save media metadata."))?;
    let result = json!({"url":format!("/v1/apps/{app_id}/media/{digest}"),"id":digest,"kind":if mime.starts_with("image/"){"image"}else{"video"},"size":raw.len(),"content_type":mime});
    let mut store = s.store.lock().unwrap();
    let mut c = store.catalog()?;
    c.apps.get_mut(app_id).ok_or_else(ApiError::missing)?.event(
        actor,
        "media.uploaded",
        result.clone(),
    );
    c.apps
        .get_mut(app_id)
        .unwrap()
        .history
        .last_mut()
        .unwrap()
        .idempotency_key = Some(key.into());
    let tx = store.connection.transaction()?;
    tx.execute(
        "UPDATE catalog SET document=?1 WHERE id=1",
        [serde_json::to_string(&c).unwrap()],
    )?;
    tx.execute(
        "INSERT INTO idempotency(actor,key,fingerprint,response,created_at) VALUES(?1,?2,?3,?4,?5)",
        rusqlite::params![actor, key, fingerprint, result.to_string(), now()],
    )?;
    tx.commit()?;
    Ok(Json(result).into_response())
}
pub use integrations::start_background;
