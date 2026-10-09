use crate::{
    Shared,
    error::{ApiError, Result},
    model::*,
    store::hash,
};
use base64::Engine;
use rusqlite::params;
use serde_json::{Value, json};

pub fn accounts_error(e: silicon_accounts_client::Error) -> ApiError {
    let status = match &e {
        silicon_accounts_client::Error::Api(e) => axum::http::StatusCode::from_u16(e.status)
            .unwrap_or(axum::http::StatusCode::BAD_GATEWAY),
        silicon_accounts_client::Error::OAuth(e) => axum::http::StatusCode::from_u16(e.status)
            .unwrap_or(axum::http::StatusCode::BAD_GATEWAY),
        _ => axum::http::StatusCode::BAD_GATEWAY,
    };
    ApiError::new(
        status,
        "accounts_error",
        format!("Silicon Accounts: {e}"),
        "Check Accounts connectivity, app registration, credentials and account authorization; retry with the same idempotency key.",
    )
}
pub async fn resolve(s: &Shared, id: &str) -> Result<Identity> {
    if !id.starts_with("c:") && !id.starts_with("si:") {
        return Err(ApiError::bad(
            "Use a c:id or si:id so the account can be resolved to its immutable UUID.",
        ));
    }
    if s.config.dev_auth {
        // Isolated fixture identity mapping, never used by production authentication.
        let name = id.split_once(':').unwrap().1;
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(ApiError::bad("Development account ID is invalid."));
        }
        return Ok(Identity {
            uuid: name.into(),
            id: id.into(),
            display_name: id.into(),
            verified_emails: vec![],
        });
    }
    let secret = s.config.accounts_app_secret.as_deref().ok_or_else(|| {
        ApiError::unavailable("APPS_ACCOUNTS_APP_SECRET is required to resolve accounts.")
    })?;
    let account = s
        .accounts
        .as_app(APP_ID, secret)
        .resolve(id)
        .await
        .map_err(accounts_error)?;
    if account.status == "deleted" {
        return Err(ApiError::bad("This account has been deleted."));
    }
    Ok(Identity {
        uuid: account.uuid,
        id: account.id,
        display_name: account.display_name,
        verified_emails: vec![],
    })
}
pub async fn registry_available(s: &Shared, id: &str) -> Result<bool> {
    if reserved_app_id(id) {
        return Ok(false);
    }
    let reserved: bool = s.store.lock().unwrap().connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM pending_secrets WHERE app_id=?1)",
        [id],
        |r| r.get(0),
    )?;
    if reserved {
        return Ok(false);
    }
    if s.config.dev_auth && s.config.accounts_service_token.is_none() {
        return Ok(true);
    }
    let Some(token) = s.config.accounts_service_token.as_deref() else {
        return Err(ApiError::unavailable(
            "APPS_ACCOUNTS_SERVICE_TOKEN is required to check the global app registry.",
        ));
    };
    let response = s
        .accounts
        .service_list_apps(token)
        .await
        .map_err(accounts_error)?;
    let apps = response["apps"]
        .as_array()
        .ok_or_else(|| ApiError::unavailable("Accounts registry response has no apps list."))?;
    Ok(!apps.iter().any(|a| a["app_id"] == id))
}
pub async fn sync_app(s: &Shared, app: &App, secret: Option<&str>) -> Result<()> {
    if s.config.dev_auth && s.config.accounts_service_token.is_none() {
        return Ok(());
    }
    let token = s.config.accounts_service_token.as_deref().ok_or_else(|| {
        ApiError::unavailable(
            "APPS_ACCOUNTS_SERVICE_TOKEN is required to register apps with Silicon Accounts.",
        )
    })?;
    let mut value = json!({"app_id":app.app_id,"name":app.name,"description":app.description,"logo_url":app.logo,"homepage_url":app.links["website"].as_str().unwrap_or_default(),"owner_uuid":app.admin_uuid,"author_uuids":app.authors.iter().map(|a|a.uuid.clone()).collect::<Vec<_>>()});
    if let Some(secret) = secret {
        value["secret"] = json!(secret);
    }
    s.accounts
        .service_sync_apps(token, &json!({"apps":[value]}))
        .await
        .map_err(accounts_error)?;
    Ok(())
}
/// Validate an uploaded package, recording each step on the app's event
/// stream so its authors can follow the upload as it happens.
/// An author's signature sent with an upload: key ID and base64 signature.
pub struct AuthorUploadSignature {
    pub key_id: String,
    pub signature: String,
    pub signer_id: String,
}
pub async fn validate_package(
    s: &Shared,
    app_id: &str,
    target: &str,
    actor: &str,
    bytes: &[u8],
    key: &str,
    author: Option<AuthorUploadSignature>,
) -> Result<Package> {
    if !TARGETS.contains(&target) {
        return Err(ApiError::bad(format!("Unsupported target `{target}`.")));
    }
    let digest = hash(bytes);
    let validation_id = new_id();
    let step = |kind: &str, data: Value| {
        let mut data = data;
        data["validation_id"] = json!(validation_id);
        data["target"] = json!(target);
        data["sha256"] = json!(digest);
        data["idempotency_key"] = json!(key);
        crate::events::progress(s, app_id, actor, key, kind, data);
    };
    step(
        "package.validation_started",
        json!({"size":bytes.len(),"commands":COMMANDS}),
    );
    let fail = |stage: &str, error: ApiError| -> ApiError {
        step(
            "package.validation_step",
            json!({"step":stage,"status":if error.status.is_server_error() {"unavailable"} else {"failed"},"code":error.code,"message":error.message}),
        );
        error
    };
    let manifest = match silicon_apps_package::inspect_archive(bytes) {
        Ok(manifest) => manifest,
        Err(e) => {
            let mut e2 = ApiError::new(
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_archive",
                format!("Package archive failed validation: {e:#}"),
                "Run silicon-apps validate and resolve every error before uploading.",
            );
            e2.details = json!({"stage":"archive","error":format!("{e:#}")});
            return Err(fail("archive", e2));
        }
    };
    step(
        "package.validation_step",
        json!({"step":"archive","status":"passed","command":manifest.command}),
    );
    if manifest.app_id != app_id {
        return Err(fail(
            "manifest",
            ApiError::bad("apps.yaml app_id must match the app receiving this package."),
        ));
    }
    if !manifest.targets.contains_key(target) {
        return Err(fail(
            "manifest",
            ApiError::bad("apps.yaml must describe the uploaded target."),
        ));
    }
    let install_script = match silicon_apps_package::install_script(bytes, &manifest, target) {
        Ok(script) => script.map(|(script, _)| InstallScriptInfo {
            path: script.path,
            sha256: script.sha256,
            size: script.size,
        }),
        Err(e) => {
            let mut error = ApiError::new(
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_archive",
                format!("The install script could not be read: {e:#}"),
                "Keep the install script named in apps.yaml inside the archive, under 16 MiB.",
            );
            error.details = json!({"stage":"manifest","error":format!("{e:#}")});
            return Err(fail("manifest", error));
        }
    };
    step(
        "package.validation_step",
        json!({"step":"manifest","status":"passed","install_script":install_script}),
    );
    // An author signature is checked before any runner time is spent.
    let author_signature = match author {
        None => None,
        Some(author) => {
            let public = {
                let store = s.store.lock().unwrap();
                crate::signing::active_author_key(&store.connection, actor, &author.key_id)
            }
            .map_err(|e| fail("author_signature", e))?;
            let message = crate::signing::author_message(
                app_id,
                target,
                &digest,
                bytes.len() as u64,
                install_script.as_ref().map(|s| s.sha256.as_str()),
            );
            if !crate::signing::verify(&public, &message, &author.signature) {
                return Err(fail(
                    "author_signature",
                    crate::signing::invalid_author_signature(
                        format!(
                            "The signature by author key {} does not match this archive.",
                            author.key_id
                        ),
                        json!({"key_id":author.key_id,"signed_message":message}),
                    ),
                ));
            }
            step(
                "package.validation_step",
                json!({"step":"author_signature","status":"passed","key_id":author.key_id}),
            );
            Some(AuthorSignature {
                key_id: author.key_id,
                algorithm: crate::signing::ALGORITHM.into(),
                public_key: public,
                signature: author.signature,
                signer_uuid: actor.into(),
                signer_id: author.signer_id,
                signed_at: now(),
            })
        }
    };
    let Some(url) = s.config.runner_url.as_ref() else {
        return Err(fail(
            "runner",
            ApiError::unavailable(
                "No isolated package runner is configured. Set APPS_RUNNER_URL and APPS_RUNNER_TARGETS.",
            ),
        ));
    };
    if !s.config.runner_targets.iter().any(|t| t == target) {
        return Err(fail(
            "runner",
            ApiError::unavailable(format!(
                "No isolated runner is available for {target}; configure a native runner before uploading this target."
            )),
        ));
    }
    let Some(token) = s.config.runner_token.as_ref() else {
        return Err(fail(
            "runner",
            ApiError::unavailable("APPS_RUNNER_TOKEN is required for the isolated runner."),
        ));
    };
    step(
        "package.validation_step",
        json!({"step":"runner","status":"started","commands":COMMANDS}),
    );
    let response = s.http.post(format!("{}/validate",url.trim_end_matches('/'))).bearer_auth(token).json(&json!({"app_id":app_id,"target":target,"package_sha256":digest,"package_base64":base64::engine::general_purpose::STANDARD.encode(bytes),"manifest":manifest})).send().await.map_err(|e|fail("runner", ApiError::unavailable(format!("Package runner could not be reached: {e}"))))?;
    let status = response.status();
    let result: Value = response.json().await.map_err(|_| {
        fail(
            "runner",
            ApiError::unavailable("Package runner returned invalid JSON."),
        )
    })?;
    if status.is_server_error() {
        let mut error = ApiError::unavailable(
            "The configured isolated target runner is unavailable; retry this upload with the same idempotency key.",
        );
        error.details = result;
        return Err(fail("runner", error));
    }
    if !status.is_success() {
        let mut e = ApiError::new(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "package_validation_failed",
            "The isolated runner rejected the package.",
            "Read the exact runner error and command results, fix the package and upload again.",
        );
        for check in result["validation"].as_array().into_iter().flatten() {
            step(
                "package.validation_step",
                json!({"step":"command","command":check["command"],"exit_code":check["exit_code"],"passed":check["passed"],"expected":check["expected"],"stdout":crate::events::step_output(check["stdout"].as_str().unwrap_or_default()),"stderr":crate::events::step_output(check["stderr"].as_str().unwrap_or_default())}),
            );
        }
        e.details = result;
        return Err(fail("runner", e));
    }
    if result["isolated"] != true || result["target"] != target {
        return Err(fail(
            "runner",
            ApiError::unavailable(
                "Runner did not attest isolated execution on the requested target.",
            ),
        ));
    }
    let mut checks = vec![];
    let mut passed = true;
    for command in COMMANDS {
        let raw = result["validation"]
            .as_array()
            .and_then(|v| v.iter().find(|v| v["command"] == command));
        let stdout = raw.and_then(|v| v["stdout"].as_str()).unwrap_or_default();
        let stderr = raw.and_then(|v| v["stderr"].as_str()).unwrap_or_default();
        let exit = raw.and_then(|v| v["exit_code"].as_i64());
        let output: Option<Value> = serde_json::from_str(stdout).ok();
        let (ok, expected) = match command {
            "--help" => (
                exit == Some(0) && (!stdout.trim().is_empty() || !stderr.trim().is_empty()),
                "Exit 0 and non-empty help text.",
            ),
            "accounts --json" => (
                exit == Some(0) && output.as_ref().is_some_and(|v| v["app_id"] == app_id),
                "Exit 0 and JSON containing this exact app_id.",
            ),
            _ => (
                exit == Some(0) && output.as_ref().is_some_and(|v| v["authenticated"] == false),
                "Exit 0 and JSON containing authenticated:false in a clean signed-out sandbox.",
            ),
        };
        passed &= ok;
        step(
            "package.validation_step",
            json!({"step":"command","command":command,"exit_code":exit,"passed":ok,"expected":expected,"stdout":crate::events::step_output(stdout),"stderr":crate::events::step_output(stderr)}),
        );
        checks.push(json!({"command":command,"exit_code":exit,"stdout":stdout.chars().take(32768).collect::<String>(),"stderr":stderr.chars().take(32768).collect::<String>(),"passed":ok,"expected":expected}));
    }
    if !passed {
        let mut e = ApiError::new(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "package_validation_failed",
            "At least one required command failed in the isolated target runner.",
            "These commands let Silicons discover app help, account ownership and login state. Fix the exact output shown and re-upload.",
        );
        e.details = json!({"target":target,"sha256":digest,"validation":checks});
        return Err(e);
    }
    let dest = s.config.data_dir.join("packages").join(&digest);
    crate::objects::publish(&dest, bytes).await.map_err(|_| {
        fail(
            "storage",
            ApiError::unavailable("Could not commit the immutable package artifact."),
        )
    })?;
    Ok(Package {
        id: new_id(),
        target: target.into(),
        sha256: digest,
        size: bytes.len() as u64,
        command: manifest.command,
        validation: checks,
        created_at: now(),
        install_script,
        inspected: true,
        author_signature,
    })
}
pub async fn import_accounts(s: &Shared) -> Result<usize> {
    let token = s
        .config
        .accounts_service_token
        .as_deref()
        .ok_or_else(|| ApiError::unavailable("Import requires APPS_ACCOUNTS_SERVICE_TOKEN."))?;
    let response = s
        .accounts
        .service_list_apps(token)
        .await
        .map_err(accounts_error)?;
    let apps = response["apps"]
        .as_array()
        .ok_or_else(|| ApiError::unavailable("Accounts import response has no apps list."))?;
    let _guard = s.mutation_gate.lock().await;
    let mut store = s.store.lock().unwrap();
    let mut c = store.catalog()?;
    let marks = crate::events::marks(&c);
    let mut count = 0;
    for v in apps {
        let id = v["app_id"]
            .as_str()
            .ok_or_else(|| ApiError::bad("Imported Accounts app has no app_id."))?;
        if c.apps.contains_key(id)
            || store.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM pending_secrets WHERE app_id=?1)",
                [id],
                |r| r.get::<_, bool>(0),
            )?
        {
            continue;
        }
        if id.is_empty()
            || id.len() > 30
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        {
            return Err(ApiError::bad(format!(
                "Existing Accounts app `{id}` has an unsafe app_id; migration requires explicit remediation."
            )));
        }
        let authors: Vec<Author> = serde_json::from_value(v["authors"].clone())
            .map_err(|e| ApiError::bad(format!("Imported authors are invalid: {e}")))?;
        if authors.is_empty() {
            return Err(ApiError::bad(format!(
                "Existing app `{id}` has no valid authors."
            )));
        }
        let owner = v["owner_uuid"]
            .as_str()
            .unwrap_or(&authors[0].uuid)
            .to_owned();
        let admin = if authors.iter().any(|a| a.uuid == owner) {
            owner
        } else {
            authors[0].uuid.clone()
        };
        let mut app = App {
            app_id: id.into(),
            name: v["name"].as_str().unwrap_or(id).into(),
            description: v["description"].as_str().unwrap_or_default().into(),
            logo: v["logo_url"].as_str().unwrap_or_default().into(),
            logo_alt: String::new(),
            banner: String::new(),
            banner_alt: String::new(),
            tags: vec![],
            visibility: "public".into(),
            domains: vec![],
            account_ids: vec![],
            access_uuids: vec![],
            links: json!({"website":v["homepage_url"].as_str().unwrap_or_default()}),
            carousel: vec![],
            published: false,
            setup_step: 1,
            created_at: v["created_at"].as_str().unwrap_or(&now()).into(),
            updated_at: now(),
            authors,
            admin_uuid: admin,
            packages: vec![],
            releases: vec![],
            reviews: vec![],
            installs: 0,
            history: vec![],
            secret_hash: String::new(),
        };
        app.event(
            "system",
            "app.imported_from_accounts",
            json!({"source":v["source"],"users_preserved":true}),
        );
        c.apps.insert(id.into(), app);
        count += 1;
    }
    let tx = store.connection.transaction()?;
    crate::store::Store::save_catalog(&tx, &c, &marks)?;
    tx.commit()?;
    store.notify_events();
    Ok(count)
}
pub async fn outbox_worker(s: Shared) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
    loop {
        interval.tick().await;
        let pending: Vec<(String, String, Value)> = {
            let store = s.store.lock().unwrap();
            let _ = store.expire_secret_replays();
            let result = (|| -> std::result::Result<Vec<_>, rusqlite::Error> {
                let mut st=store.connection.prepare("SELECT id,kind,body FROM outbox WHERE delivered_at IS NULL ORDER BY attempts,created_at LIMIT 25")?;
                let rows = st.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })?;
                Ok(rows
                    .filter_map(|r| r.ok())
                    .filter_map(|(id, k, b)| serde_json::from_str(&b).ok().map(|v| (id, k, v)))
                    .collect())
            })();
            result.unwrap_or_default()
        };
        for (id, kind, body) in pending {
            let result = deliver(&s, &id, &kind, &body).await;
            let store = s.store.lock().unwrap();
            match result {
                Ok(()) => {
                    let _=store.connection.execute("UPDATE outbox SET delivered_at=?1,attempts=attempts+1,last_error=NULL WHERE id=?2",params![now(),id]);
                }
                Err(e) => {
                    let _ = store.connection.execute(
                        "UPDATE outbox SET attempts=attempts+1,last_error=?1 WHERE id=?2",
                        params![e.message, id],
                    );
                }
            }
        }
    }
}
async fn deliver(s: &Shared, id: &str, kind: &str, body: &Value) -> Result<()> {
    if kind == "telemetry" && (!s.config.telemetry_enabled || s.config.telemetry_url.is_none()) {
        return Ok(());
    }
    if kind == "accounts.sync" {
        let _guard = s.mutation_gate.lock().await;
        let app = s.store.lock().unwrap().app(
            body["app_id"]
                .as_str()
                .ok_or_else(|| ApiError::bad("Sync job has no app_id."))?,
        )?;
        return sync_app(s, &app, None).await;
    }
    if kind.starts_with("mail.") && s.config.mail_url.is_none() {
        let token = s.config.accounts_service_token.as_deref().ok_or_else(|| {
            ApiError::unavailable(
                "Configure APPS_ACCOUNTS_SERVICE_TOKEN or APPS_MAIL_URL for email delivery.",
            )
        })?;
        s.accounts
            .service_send_apps_mail(token, kind, body, id)
            .await
            .map_err(accounts_error)?;
        return Ok(());
    }
    let url = if kind == "telemetry" {
        s.config.telemetry_url.as_ref()
    } else {
        s.config.mail_url.as_ref()
    }
    .ok_or_else(|| ApiError::unavailable(format!("No delivery endpoint configured for {kind}.")))?;
    let mut request = s
        .http
        .post(url)
        .header("Idempotency-Key", id)
        .json(&json!({"id":id,"kind":kind,"data":body}));
    if kind != "telemetry"
        && let Some(token) = &s.config.mail_token
    {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|e| ApiError::unavailable(format!("Outbox delivery failed: {e}")))?;
    if !response.status().is_success() {
        return Err(ApiError::unavailable(format!(
            "Outbox destination returned HTTP {}.",
            response.status()
        )));
    }
    Ok(())
}
pub async fn start_background(s: Shared) -> Result<()> {
    if s.config.import_accounts {
        let count = import_accounts(&s).await?;
        eprintln!(
            "Imported {count} existing Accounts apps, preserving app IDs and Accounts users."
        );
    }
    tokio::spawn(outbox_worker(s.clone()));
    tokio::spawn(crate::subscriptions::delivery_worker(s));
    Ok(())
}
pub fn refresh_identity(s: &Shared, who: &Identity) -> Result<()> {
    let mut cache = s.identity_cache.lock().unwrap();
    if cache.len() > 10000 {
        cache.clear();
    }
    cache.insert(who.uuid.clone(), (std::time::Instant::now(), who.clone()));
    drop(cache);
    let store = s.store.lock().unwrap();
    let mut c = store.catalog()?;
    let mut changed = false;
    for app in c.apps.values_mut() {
        for author in &mut app.authors {
            if author.uuid == who.uuid {
                if !who.id.is_empty() && author.id != who.id {
                    author.id = who.id.clone();
                    changed = true;
                }
                // Accounts identity lookups may omit profile fields. Omission must not
                // erase a display name previously obtained from a granted profile.
                if !who.display_name.trim().is_empty() && author.display_name != who.display_name {
                    author.display_name = who.display_name.clone();
                    changed = true;
                }
            }
        }
        for review in &mut app.reviews {
            if review.uuid == who.uuid && !who.id.is_empty() && review.id != who.id {
                review.id = who.id.clone();
                changed = true;
            }
        }
        for (uuid, id) in app.access_uuids.iter().zip(app.account_ids.iter_mut()) {
            if uuid == &who.uuid && !who.id.is_empty() && id != &who.id {
                *id = who.id.clone();
                changed = true;
            }
        }
    }
    for invite in &mut c.invites {
        if invite.account_uuid.as_deref() == Some(&who.uuid)
            && (invite.to.starts_with("c:") || invite.to.starts_with("si:"))
            && !who.id.is_empty()
            && invite.to != who.id
        {
            invite.to = who.id.clone();
            changed = true;
        }
    }
    if changed {
        store.connection.execute(
            "UPDATE catalog SET document=?1 WHERE id=1",
            [serde_json::to_string(&c).unwrap()],
        )?;
    }
    Ok(())
}
pub async fn refresh_view_identities(s: &Shared, mut value: Value) -> Result<Value> {
    let mut ids = std::collections::BTreeSet::new();
    fn collect(v: &Value, ids: &mut std::collections::BTreeSet<String>) {
        for key in ["uuid", "account_uuid"] {
            if let Some(id) = v[key].as_str() {
                ids.insert(id.into());
            }
        }
        if let Some(authors) = v["authors"].as_array() {
            for a in authors {
                if let Some(id) = a["uuid"].as_str() {
                    ids.insert(id.into());
                }
            }
        }
        if let Some(items) = v["items"].as_array() {
            for item in items {
                if let Some(id) = item["uuid"].as_str() {
                    ids.insert(id.into());
                }
                collect(item, ids);
            }
        }
    }
    collect(&value, &mut ids);
    for id in ids {
        let fresh = s
            .identity_cache
            .lock()
            .unwrap()
            .get(&id)
            .is_some_and(|(when, _)| when.elapsed() < std::time::Duration::from_secs(300));
        if !fresh
            && !s.config.dev_auth
            && let Some(secret) = s.config.accounts_app_secret.as_deref()
        {
            let app = s.accounts.as_app(APP_ID, secret);
            let profile = app.user(&id).await.ok().filter(|account| {
                account.uuid == id
                    && account.status == "active"
                    && account.id.as_ref().is_some_and(|id| !id.is_empty())
            });
            let identity = if let Some(account) = profile {
                Some(Identity {
                    uuid: account.uuid,
                    id: account.id.unwrap(),
                    display_name: account.display_name,
                    verified_emails: vec![],
                })
            } else {
                // Public lookup supplies the current ID even when there is no active
                // Apps membership. Its missing profile fields retain the stored name.
                app.resolve(&id)
                    .await
                    .ok()
                    .filter(|account| account.uuid == id)
                    .map(|account| Identity {
                        uuid: account.uuid,
                        id: account.id,
                        display_name: account.display_name,
                        verified_emails: vec![],
                    })
            };
            if let Some(identity) = identity {
                refresh_identity(s, &identity)?;
            }
        }
    }
    let cache = s.identity_cache.lock().unwrap();
    fn apply(
        v: &mut Value,
        cache: &std::collections::BTreeMap<String, (std::time::Instant, Identity)>,
    ) {
        if let Some(id) = v["uuid"].as_str()
            && let Some((_, identity)) = cache.get(id)
        {
            if !identity.id.is_empty() {
                v["id"] = json!(identity.id);
            }
            if v.get("display_name").is_some() && !identity.display_name.trim().is_empty() {
                v["display_name"] = json!(identity.display_name);
            }
        }
        if let Some(id) = v["account_uuid"].as_str()
            && let Some((_, identity)) = cache.get(id)
            && !identity.id.is_empty()
            && v["to"]
                .as_str()
                .is_some_and(|to| to.starts_with("c:") || to.starts_with("si:"))
        {
            v["to"] = json!(identity.id);
        }
        if let Some(authors) = v.get_mut("authors").and_then(Value::as_array_mut) {
            for a in authors {
                apply(a, cache);
            }
        }
        if let Some(items) = v.get_mut("items").and_then(Value::as_array_mut) {
            for item in items {
                apply(item, cache);
            }
        }
    }
    apply(&mut value, &cache);
    Ok(value)
}
