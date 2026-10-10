use crate::{
    error::{ApiError, Result},
    model::*,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

pub struct Store {
    pub connection: Connection,
    /// Bumped after every commit that appends events; streams and the
    /// webhook worker wait on it.
    pub events: tokio::sync::watch::Sender<u64>,
    /// Signs every new release's packages before the catalog is saved.
    pub signer: Option<std::sync::Arc<crate::signing::Keyring>>,
}
/// Orders accepted by `GET /v1/apps?sort=`.
pub const SORTS: [&str; 6] = [
    "relevance",
    "rating",
    "installs",
    "name",
    "updated",
    "newest",
];
const MIGRATIONS: [&str; 4] = [
    include_str!("../../../migrations/001_catalog.sql"),
    include_str!("../../../migrations/002_events.sql"),
    include_str!("../../../migrations/003_signing.sql"),
    include_str!("../../../migrations/004_account_uuid_backfill.sql"),
];
#[derive(Default)]
pub struct Prepared {
    pub identities: Vec<Identity>,
    pub package: Option<Package>,
    pub secret: Option<String>,
    pub webhook: Option<Value>,
}
pub struct Mutation<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub key: &'a str,
    pub who: Option<&'a Identity>,
    pub body: &'a Value,
    pub digest: &'a str,
    pub prepared: Prepared,
}
pub fn str_field<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad(format!("`{k}` must be a string.")))
}
fn strings(v: &Value, k: &str) -> Result<Vec<String>> {
    v.get(k)
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::bad(format!("`{k}` must be an array.")))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| ApiError::bad(format!("`{k}` must contain strings.")))
        })
        .collect()
}
fn need(who: Option<&Identity>) -> Result<&Identity> {
    who.ok_or_else(ApiError::auth)
}
fn author(app: &App, who: Option<&Identity>) -> Result<()> {
    need(who)?;
    if !app.is_author(who) {
        return Err(ApiError::forbidden());
    }
    Ok(())
}
fn admin(app: &App, who: Option<&Identity>) -> Result<()> {
    author(app, who)?;
    if !app.is_admin(who) {
        return Err(ApiError::new(
            axum::http::StatusCode::FORBIDDEN,
            "admin_required",
            "Only the app admin can change access or remove authors.",
            "Ask the current admin to make this change or transfer adminship.",
        ));
    }
    Ok(())
}
fn invite_matches(i: &Invite, w: &Identity) -> bool {
    i.account_uuid.as_deref() == Some(&w.uuid)
        || i.to.contains('@')
            && w.verified_emails
                .iter()
                .any(|e| e.eq_ignore_ascii_case(&i.to))
}
pub fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)
                .map_err(|_| ApiError::unavailable("Cannot create database directory."))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(path)
                .map_err(|_| ApiError::unavailable("Cannot create the database file securely."))?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| ApiError::unavailable("Cannot secure the database file."))?;
        }
        let c = Connection::open(path)?;
        c.pragma_update(None, "journal_mode", "WAL")?;
        c.busy_timeout(std::time::Duration::from_secs(10))?;
        Self::migrate(c)
    }
    pub fn memory() -> Result<Self> {
        Self::migrate(Connection::open_in_memory()?)
    }
    fn migrate(c: Connection) -> Result<Self> {
        for migration in MIGRATIONS {
            c.execute_batch(migration)?;
        }
        Ok(Self {
            connection: c,
            events: tokio::sync::watch::channel(0).0,
            signer: None,
        })
    }
    /// Retired subjects can never authenticate, including before Accounts cache expiry.
    pub fn account_uuid_retired(&self, uuid: &str) -> Result<bool> {
        Ok(self
            .connection
            .query_row(
                "SELECT 1 FROM account_uuid_migrations WHERE old_uuid=?1",
                [uuid],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }
    pub fn notify_events(&self) {
        self.events.send_modify(|n| *n = n.wrapping_add(1));
    }
    /// Save a changed catalog and append the history entries it gained since
    /// `marks` to the event log, in the caller's transaction.
    pub fn save_catalog(
        tx: &Connection,
        c: &Catalog,
        marks: &BTreeMap<String, usize>,
    ) -> Result<usize> {
        tx.execute(
            "UPDATE catalog SET document=?1 WHERE id=1",
            [serde_json::to_string(c).unwrap()],
        )?;
        crate::events::append(tx, c, &crate::events::new_since(c, marks))
    }
    pub fn catalog(&self) -> Result<Catalog> {
        let raw: String =
            self.connection
                .query_row("SELECT document FROM catalog WHERE id=1", [], |r| r.get(0))?;
        serde_json::from_str(&raw)
            .map_err(|_| ApiError::unavailable("Catalog storage is invalid; restore from backup."))
    }
    /// One-time correction of the platform's own ID, never a public rename API.
    pub fn migrate_silicon_apps_id(&mut self) -> Result<bool> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let raw: String =
            tx.query_row("SELECT document FROM catalog WHERE id=1", [], |r| r.get(0))?;
        let mut catalog: Catalog = serde_json::from_str(&raw)
            .map_err(|_| ApiError::unavailable("Catalog storage is invalid."))?;
        let Some(old) = catalog.apps.get("apps") else {
            return Ok(false);
        };
        if old.name != "Silicon Apps" {
            return Err(ApiError::conflict(
                "The legacy apps ID is not Silicon Apps; review the identity before migration.",
            ));
        }
        if catalog.apps.contains_key(APP_ID) {
            return Err(ApiError::conflict(
                "Both Apps identities exist; refusing to merge different apps.",
            ));
        }
        let mut app = catalog.apps.remove("apps").unwrap();
        app.app_id = APP_ID.into();
        for release in &mut app.releases {
            release.app_id = APP_ID.into();
        }
        app.event(
            "system",
            "app.id_migrated",
            json!({"previous_app_id":"apps","app_id":APP_ID}),
        );
        catalog.apps.insert(APP_ID.into(), app);
        for invite in &mut catalog.invites {
            if invite.app_id == "apps" {
                invite.app_id = APP_ID.into();
            }
        }
        tx.execute(
            "UPDATE catalog SET document=?1 WHERE id=1",
            [serde_json::to_string(&catalog).unwrap()],
        )?;
        let migrated = catalog.apps[APP_ID].history.last().unwrap();
        crate::events::append(
            &tx,
            &catalog,
            &[crate::events::NewEvent::from_history(APP_ID, migrated)],
        )?;
        tx.execute(
            "UPDATE pending_secrets SET app_id=?1 WHERE app_id='apps'",
            [APP_ID],
        )?;
        tx.execute("UPDATE outbox SET body=json_set(body,'$.app_id',?1) WHERE delivered_at IS NULL AND json_extract(body,'$.app_id')='apps'", [APP_ID])?;
        // Reuse rotating refresh tokens to obtain the new audience before any
        // browser access token from the old identity is presented again.
        tx.execute("UPDATE sessions SET expires_at=0", [])?;
        tx.commit()?;
        Ok(true)
    }
    pub fn app(&self, id: &str) -> Result<App> {
        self.catalog()?
            .apps
            .remove(id)
            .ok_or_else(ApiError::missing)
    }
    pub fn pending_secret(
        &self,
        actor: &str,
        key: &str,
        fingerprint: &str,
    ) -> Result<Option<String>> {
        let row: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT fingerprint,secret FROM pending_secrets WHERE actor=?1 AND key=?2",
                params![actor, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match row {
            Some((fp, secret)) if fp == fingerprint => Ok(Some(secret)),
            Some(_) => Err(ApiError::conflict(
                "This idempotency key belongs to a different pending Accounts operation.",
            )),
            None => Ok(None),
        }
    }
    pub fn reserve_secret(
        &self,
        actor: &str,
        key: &str,
        fingerprint: &str,
        app_id: &str,
        secret: &str,
    ) -> Result<()> {
        let occupied: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM pending_secrets WHERE app_id=?1)",
            [app_id],
            |r| r.get(0),
        )?;
        if occupied {
            return Err(ApiError::conflict(
                "An Accounts operation for this app is pending. Retry the original request with its original idempotency key.",
            ));
        }
        self.connection.execute("INSERT INTO pending_secrets(actor,key,fingerprint,app_id,secret,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![actor,key,fingerprint,app_id,secret,now()])?;
        Ok(())
    }
    pub fn replay(&self, actor: &str, key: &str, fingerprint: &str) -> Result<Option<Value>> {
        let prior: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT fingerprint,response FROM idempotency WHERE actor=?1 AND key=?2",
                params![actor, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match prior {
            None => Ok(None),
            Some((fp, response)) if fp == fingerprint => {
                let v: Value = serde_json::from_str(&response)
                    .map_err(|_| ApiError::unavailable("Stored replay response is invalid."))?;
                if let Some(error) = v.get("error") {
                    return Err(ApiError {
                        status: axum::http::StatusCode::from_u16(
                            v["_status"].as_u64().unwrap_or(422) as u16,
                        )
                        .unwrap_or(axum::http::StatusCode::UNPROCESSABLE_ENTITY),
                        code: error["code"]
                            .as_str()
                            .unwrap_or("package_validation_failed")
                            .into(),
                        message: error["message"]
                            .as_str()
                            .unwrap_or("Package validation failed.")
                            .into(),
                        hint: error["hint"]
                            .as_str()
                            .unwrap_or("Fix the package and upload with a new idempotency key.")
                            .into(),
                        details: error["details"].clone(),
                    });
                }
                if v["_secret_expired"] == true {
                    return Err(ApiError::new(
                        axum::http::StatusCode::CONFLICT,
                        "secret_replay_expired",
                        "This operation completed, but its one-time secret replay window has expired.",
                        "Get the app and rotate its secret if you did not save it. The operation will not run twice.",
                    ));
                }
                Ok(Some(v))
            }
            Some(_) => Err(ApiError::conflict(
                "This idempotency key already belongs to a different request.",
            )),
        }
    }
    pub fn expire_secret_replays(&self) -> Result<usize> {
        Ok(self.connection.execute("UPDATE idempotency SET response=json_set(json_remove(response,'$.app_secret','$.webhook_secret','$.secret'),'$._secret_expired',json('true')) WHERE julianday(created_at)<julianday('now','-10 minutes') AND (json_type(response,'$.app_secret') IS NOT NULL OR json_type(response,'$.webhook_secret') IS NOT NULL OR json_type(response,'$.secret') IS NOT NULL)",[])?)
    }

    pub fn read(
        &self,
        path: &str,
        q: &BTreeMap<String, String>,
        who: Option<&Identity>,
    ) -> Result<Value> {
        let c = self.catalog()?;
        let p: Vec<_> = path.trim_matches('/').split('/').collect();
        if p == ["me"] {
            return Ok(json!(need(who)?));
        }
        if p == ["invites"] {
            let w = need(who)?;
            return Ok(
                json!({"items":c.invites.iter().filter(|i|i.status=="pending"&&invite_matches(i,w)).collect::<Vec<_>>()}),
            );
        }
        if p.len() == 3 && p[..2] == ["apps", "availability"] {
            return Ok(
                json!({"available":valid_app_id(p[2])&&!reserved_app_id(p[2])&&!c.apps.contains_key(p[2])}),
            );
        }
        if p.len() == 2 && p[0] == "authors" {
            let mut apps: Vec<_> = c
                .apps
                .values()
                .filter(|app| app.published && app.visible(who))
                .filter(|app| app.authors.iter().any(|author| author.uuid == p[1]))
                .collect();
            let author = apps
                .iter()
                .flat_map(|app| &app.authors)
                .find(|author| author.uuid == p[1])
                .cloned()
                .ok_or_else(ApiError::missing)?;
            apps.sort_by_key(|app| (app.name.to_lowercase(), app.app_id.clone()));
            let total = apps.len();
            let limit = page(q, "limit", 24).clamp(1, 100);
            let offset = page(q, "offset", 0);
            return Ok(json!({
                "uuid":author.uuid,"id":author.id,"display_name":author.display_name,
                "items":apps.into_iter().skip(offset).take(limit).map(|app|app.view(who)).collect::<Vec<_>>(),
                "total":total
            }));
        }
        if p == ["apps"] {
            let mine = q.get("mine").is_some_and(|v| v == "true");
            let visibility = q.get("visibility").filter(|v| !v.is_empty());
            if visibility.is_some_and(|v| v != "public" && v != "private") {
                return Err(ApiError::bad("visibility must be public or private."));
            }
            if mine || visibility.is_some_and(|v| v == "private") {
                need(who)?;
            }
            let query = q.get("q").map(|v| v.to_lowercase()).unwrap_or_default();
            if query.len() > 512 {
                return Err(ApiError::bad("Search query is limited to 512 bytes."));
            }
            let tags: Vec<String> = q
                .get("tags")
                .map(|v| {
                    v.split(',')
                        .map(|t| t.trim().to_lowercase())
                        .filter(|t| !t.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            if tags.len() > 20 {
                return Err(ApiError::bad("Filter by at most 20 tags."));
            }
            let target = q.get("target").filter(|v| !v.is_empty());
            if target.is_some_and(|t| !TARGETS.contains(&t.as_str())) {
                let mut error = ApiError::bad("target is not a package target.");
                error.details = json!({"targets":TARGETS});
                return Err(error);
            }
            let sort = q
                .get("sort")
                .map(String::as_str)
                .filter(|v| !v.is_empty())
                .unwrap_or("relevance");
            if !SORTS.contains(&sort) {
                let mut error = ApiError::bad("sort is not supported.");
                error.details = json!({"sort":SORTS});
                return Err(error);
            }
            let limit = strict_page(q, "limit", 50, 1, 100)?;
            let offset = strict_page(q, "offset", 0, 0, usize::MAX)?;
            let mut apps: Vec<_> = c
                .apps
                .values()
                .filter(|a| {
                    if mine {
                        a.is_author(who)
                    } else {
                        a.published && a.visible(who)
                    }
                })
                .filter(|a| visibility.is_none_or(|v| v == &a.visibility))
                .filter(|a| {
                    tags.iter()
                        .all(|tag| a.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
                })
                .filter(|a| target.is_none_or(|t| a.targets().contains(t)))
                .filter_map(|a| search_score(a, &query).map(|s| (s, a)))
                .collect();
            let by_rating = |a: &App, b: &App| {
                b.rating()
                    .unwrap_or(-1.0)
                    .partial_cmp(&a.rating().unwrap_or(-1.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            };
            apps.sort_by(|(sa, a), (sb, b)| {
                match sort {
                    "rating" => by_rating(a, b).then_with(|| b.reviews.len().cmp(&a.reviews.len())),
                    "installs" => b.installs.cmp(&a.installs),
                    "name" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    "updated" => b.updated_at.cmp(&a.updated_at),
                    "newest" => b.created_at.cmp(&a.created_at),
                    _ => sb.cmp(sa).then_with(|| by_rating(a, b)),
                }
                .then_with(|| a.app_id.cmp(&b.app_id))
            });
            let total = apps.len();
            let next_offset = (offset.saturating_add(limit) < total).then(|| offset + limit);
            return Ok(json!({
                "items":apps.into_iter().skip(offset).take(limit).map(|(_,a)|a.view(who)).collect::<Vec<_>>(),
                "total":total,
                "limit":limit,
                "offset":offset,
                "next_offset":next_offset,
                "sort":sort
            }));
        }
        if p.len() < 2 || p[0] != "apps" {
            return Err(ApiError::missing());
        }
        let app = c.apps.get(p[1]).ok_or_else(ApiError::missing)?;
        if !app.visible(who) {
            return Err(ApiError::missing());
        }
        if p.len() == 2 {
            return Ok(app.view(who));
        }
        if p.len() != 3 {
            return Err(ApiError::missing());
        }
        match p[2] {
            "authors" => Ok(json!({"items":app.authors})),
            "readiness" => {
                author(app, who)?;
                Ok(app.readiness())
            }
            "packages" => {
                author(app, who)?;
                Ok(json!({"items":app.packages}))
            }
            "invites" => {
                author(app, who)?;
                Ok(
                    json!({"items":c.invites.iter().filter(|i|i.app_id==app.app_id).collect::<Vec<_>>()}),
                )
            }
            "history" => {
                author(app, who)?;
                Ok(
                    json!({"items":app.history.iter().rev().skip(page(q,"offset",0)).take(page(q,"limit",100).min(500)).collect::<Vec<_>>(),"total":app.history.len()}),
                )
            }
            "releases" => Ok(
                json!({"items":app.releases.iter().rev().filter(|r|q.get("channel").is_none_or(|ch|ch==&r.channel)).collect::<Vec<_>>()}),
            ),
            "reviews" => {
                Ok(json!({"items":app.reviews,"rating":app.rating(),"count":app.reviews.len()}))
            }
            "resolve" => {
                let channel = q.get("channel").map(String::as_str).unwrap_or("production");
                if !["production", "development"].contains(&channel) {
                    return Err(ApiError::bad("channel must be production or development."));
                }
                let release = if let Some(v) = q.get("version") {
                    app.releases
                        .iter()
                        .find(|r| r.channel == channel && &r.version == v)
                } else {
                    app.latest(channel)
                }
                .ok_or_else(|| {
                    let mut error = ApiError::new(
                        axum::http::StatusCode::NOT_FOUND,
                        "release_not_found",
                        format!("No {channel} release matches this request."),
                        "Use the development channel or choose an available version.",
                    );
                    error.details =
                        json!({"channel":channel,"withdrawn":app.withdrawn(Some(channel))});
                    error
                })?;
                // A withdrawn release is never served, not even by exact version.
                if let Some(w) = &release.withdrawn {
                    let replacement = app.latest(channel);
                    let mut error = ApiError::new(
                        axum::http::StatusCode::GONE,
                        "release_withdrawn",
                        format!(
                            "{} {channel} {} was withdrawn: {}",
                            app.app_id, release.version, w.reason
                        ),
                        match replacement {
                            Some(r) => format!(
                                "Install {} {} instead: it is the latest good {channel} release.",
                                app.app_id, r.version
                            ),
                            None => format!(
                                "No good {channel} release is available yet. Try again after the authors publish one."
                            ),
                        },
                    );
                    error.details = json!({"release_id":release.id,"version":release.version,"channel":channel,"reason":w.reason,"withdrawn_at":w.at,"replacement":replacement.map(|r|json!({"release_id":r.id,"version":r.version}))});
                    return Err(error);
                }
                let target = q
                    .get("target")
                    .ok_or_else(|| ApiError::bad("target is required."))?;
                let package = app
                    .packages
                    .iter()
                    .find(|p| &p.target == target && release.package_ids.contains(&p.id))
                    .ok_or_else(|| {
                        ApiError::new(
                            axum::http::StatusCode::NOT_FOUND,
                            "target_unavailable",
                            format!("This release does not support {target}."),
                            "Choose a supported target shown on the app page.",
                        )
                    })?;
                // Every package Apps serves is signed.
                let signature =
                    crate::signing::resolution_signature(release, package).ok_or_else(|| {
                        ApiError::new(
                            axum::http::StatusCode::SERVICE_UNAVAILABLE,
                            "release_not_signed",
                            "This release's package has no signature yet, so it is not served.",
                            "Try again in a minute. If it lasts, report it with silicon-apps report.",
                        )
                    })?;
                Ok(json!({
                    "app_id":app.app_id,
                    "release":release,
                    "package":package,
                    "download_path":format!("/v1/apps/{}/packages/{}/download",app.app_id,package.id),
                    "signature":signature,
                    "author_signature":package.author_signature,
                    "install_script":package.install_script,
                    "withdrawn":app.withdrawn(Some(channel)),
                }))
            }
            _ => Err(ApiError::missing()),
        }
    }
    pub fn mutate(&mut self, m: Mutation<'_>) -> Result<(Value, bool)> {
        self.expire_secret_replays()?;
        let actor = m.who.map(|i| i.uuid.as_str()).unwrap_or("anonymous");
        let fingerprint = format!("{}:{}:{}", m.method, m.path, m.digest);
        if let Some(v) = self.replay(actor, m.key, &fingerprint)? {
            return Ok((v, true));
        }
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let raw: String =
            tx.query_row("SELECT document FROM catalog WHERE id=1", [], |r| r.get(0))?;
        let mut c: Catalog = serde_json::from_str(&raw)
            .map_err(|_| ApiError::unavailable("Catalog storage is invalid."))?;
        let before = crate::events::marks(&c);
        let mut outbox = vec![];
        let mut response = apply(&mut c, &m, &mut outbox)?;
        if let Some(signer) = &self.signer
            && crate::signing::sign_catalog(&mut c, signer) > 0
        {
            // A new or promoted release answers with its signatures.
            let p: Vec<_> = m.path.trim_matches('/').split('/').collect();
            if let (Some(app_id), Some(release_id)) = (p.get(1), response["id"].as_str())
                && response.get("package_ids").is_some()
                && let Some(release) = c
                    .apps
                    .get(*app_id)
                    .and_then(|a| a.releases.iter().find(|r| r.id == release_id))
            {
                response = json!(release);
            }
        }
        for (id, app) in &mut c.apps {
            for event in app.history.iter_mut().skip(*before.get(id).unwrap_or(&0)) {
                event.idempotency_key = Some(m.key.into());
            }
        }
        let appended = Self::save_catalog(&tx, &c, &before)?;
        tx.execute("INSERT INTO idempotency(actor,key,fingerprint,response,created_at) VALUES(?1,?2,?3,?4,?5)",params![actor,m.key,fingerprint,response.to_string(),now()])?;
        tx.execute(
            "DELETE FROM pending_secrets WHERE actor=?1 AND key=?2",
            params![actor, m.key],
        )?;
        for (kind, body) in outbox {
            tx.execute(
                "INSERT INTO outbox(id,kind,body,created_at) VALUES(?1,?2,?3,?4)",
                params![new_id(), kind, body.to_string(), now()],
            )?;
        }
        tx.commit()?;
        if appended > 0 {
            self.notify_events();
        }
        Ok((response, false))
    }
    pub fn validation_failure(
        &mut self,
        app_id: &str,
        who: &str,
        key: &str,
        fingerprint: &str,
        error: &ApiError,
    ) -> Result<()> {
        let mut c = self.catalog()?;
        let marks = crate::events::marks(&c);
        let app = c.apps.get_mut(app_id).ok_or_else(ApiError::missing)?;
        app.event(who, "package.validation_failed", error.details.clone());
        app.history.last_mut().unwrap().idempotency_key = Some(key.into());
        let response = json!({"_status":error.status.as_u16(),"error":{"code":error.code,"message":error.message,"hint":error.hint,"details":error.details}});
        let tx = self.connection.transaction()?;
        Self::save_catalog(&tx, &c, &marks)?;
        tx.execute("INSERT INTO idempotency(actor,key,fingerprint,response,created_at) VALUES(?1,?2,?3,?4,?5)",params![who,key,fingerprint,response.to_string(),now()])?;
        tx.commit()?;
        self.notify_events();
        Ok(())
    }
}
fn page(q: &BTreeMap<String, String>, key: &str, default: usize) -> usize {
    q.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}
/// A pagination value that must be a whole number in range when present.
fn strict_page(
    q: &BTreeMap<String, String>,
    key: &str,
    default: usize,
    min: usize,
    max: usize,
) -> Result<usize> {
    match q.get(key).filter(|v| !v.is_empty()) {
        None => Ok(default),
        Some(v) => v
            .parse::<usize>()
            .ok()
            .filter(|n| (min..=max).contains(n))
            .ok_or_else(|| {
                ApiError::bad(if max == usize::MAX {
                    format!("{key} must be a whole number of at least {min}.")
                } else {
                    format!("{key} must be a whole number from {min} to {max}.")
                })
            }),
    }
}
fn apply(c: &mut Catalog, m: &Mutation<'_>, outbox: &mut Vec<(String, Value)>) -> Result<Value> {
    let p: Vec<_> = m.path.trim_matches('/').split('/').collect();
    if !mutation_route_exists(m.method, &p) {
        return Err(ApiError::missing());
    }
    let b = m.body;
    let who = m.who;
    let actor = who.map(|i| i.uuid.as_str()).unwrap_or("anonymous");
    if p == ["platforms"] && m.method == "POST" {
        let w = need(who)?;
        let target = str_field(b, "target")?;
        if !TARGETS.contains(&target) {
            return Err(ApiError::bad("Unsupported target."));
        }
        let targets = c.platforms.entry(w.uuid.clone()).or_default();
        if !targets.iter().any(|t| t == target) {
            targets.push(target.into());
        }
        return Ok(json!({"registered":true,"target":target}));
    }
    if p == ["reports"] && m.method == "POST" {
        let message = str_field(b, "message")?;
        if message.trim().is_empty() || message.len() > 20000 {
            return Err(ApiError::bad("message must contain 1–20,000 bytes."));
        }
        let report = json!({"id":new_id(),"message":message,"pr":b.get("pr"),"actor_uuid":actor,"created_at":now(),"recipients":["saketdev12@gmail.com","shubhastro2@gmail.com","bugs@teamofsilicons.com"]});
        c.reports.push(report.clone());
        outbox.push(("mail.report".into(), report.clone()));
        return Ok(json!({"id":report["id"],"status":"queued"}));
    }
    if p == ["apps"] && m.method == "POST" {
        let w = need(who)?;
        let id = str_field(b, "app_id")?;
        if !valid_app_id(id) {
            return Err(ApiError::bad(
                "app_id must be 3–30 lowercase letters, digits, hyphens or underscores.",
            ));
        }
        if reserved_app_id(id) {
            return Err(ApiError::conflict(
                "This app_id is reserved for a built-in Silicon Accounts service.",
            ));
        }
        if c.apps.contains_key(id) {
            return Err(ApiError::conflict(
                "This app_id already exists and cannot be reused.",
            ));
        }
        let name = str_field(b, "name")?.trim();
        if name.is_empty() || name.chars().count() > 120 {
            return Err(ApiError::bad("name must contain 1–120 characters."));
        }
        let secret =
            m.prepared.secret.as_ref().ok_or_else(|| {
                ApiError::unavailable("Accounts app registration did not complete.")
            })?;
        let mut app = App {
            app_id: id.into(),
            name: name.into(),
            description: b["description"].as_str().unwrap_or_default().into(),
            logo: b["logo"].as_str().unwrap_or_default().into(),
            logo_alt: String::new(),
            banner: String::new(),
            banner_alt: String::new(),
            tags: vec![],
            visibility: "public".into(),
            domains: vec![],
            account_ids: vec![],
            access_uuids: vec![],
            links: json!({}),
            carousel: vec![],
            published: false,
            setup_step: 1,
            created_at: now(),
            updated_at: now(),
            authors: vec![Author::from_identity(w)],
            admin_uuid: w.uuid.clone(),
            packages: vec![],
            releases: vec![],
            reviews: vec![],
            installs: 0,
            history: vec![],
            secret_hash: hash(secret.as_bytes()),
        };
        validate_details(&app)?;
        app.event(actor, "app.created", json!({"name":name}));
        let view = app.view(who);
        c.apps.insert(id.into(), app);
        return Ok(json!({"app":view,"app_secret":secret}));
    }
    if p.len() == 3 && p[0] == "invites" && m.method == "POST" {
        let w = need(who)?;
        let invite = c
            .invites
            .iter_mut()
            .find(|i| i.id == p[1])
            .ok_or_else(ApiError::missing)?;
        if !invite_matches(invite, w) {
            return Err(ApiError::missing());
        }
        if invite.status != "pending" {
            return Err(ApiError::conflict("This invitation is no longer pending."));
        }
        let app = c
            .apps
            .get_mut(&invite.app_id)
            .ok_or_else(ApiError::missing)?;
        match p[2] {
            "accept" => {
                if !app.is_author(who) {
                    app.authors.push(Author::from_identity(w));
                }
                invite.status = "accepted".into();
                invite.account_uuid = Some(w.uuid.clone());
                app.event(
                    actor,
                    "author.joined",
                    json!({"invite_id":invite.id,"uuid":w.uuid}),
                );
                outbox.push(("accounts.sync".into(), json!({"app_id":app.app_id})));
            }
            "decline" => {
                invite.status = "declined".into();
                app.event(
                    actor,
                    "author.invite_declined",
                    json!({"invite_id":invite.id}),
                );
            }
            _ => return Err(ApiError::missing()),
        };
        return Ok(json!({"status":invite.status}));
    }
    if p.len() < 2 || p[0] != "apps" {
        return Err(ApiError::missing());
    }
    let app = c.apps.get_mut(p[1]).ok_or_else(ApiError::missing)?;
    if p.len() == 3 && p[2] == "installs" && m.method == "POST" {
        if !app.published || !app.visible(who) {
            return Err(ApiError::missing());
        }
        let release_id = str_field(b, "release_id")?;
        let package_id = str_field(b, "package_id")?;
        if !app
            .releases
            .iter()
            .any(|r| r.id == release_id && r.package_ids.iter().any(|p| p == package_id))
        {
            return Err(ApiError::bad(
                "package_id must belong to the installed release.",
            ));
        }
        if let Some(w) = who
            && let Some(pkg) = app.packages.iter().find(|p| p.id == package_id)
        {
            let targets = c.platforms.entry(w.uuid.clone()).or_default();
            if !targets.contains(&pkg.target) {
                targets.push(pkg.target.clone());
            }
        }
        app.installs += 1;
        app.event(actor, "app.installed", b.clone());
        return Ok(json!({"installs":app.installs}));
    }
    if p.len() == 3 && p[2] == "review" {
        let w = need(who)?;
        if m.method == "DELETE" && app.reviews.iter().any(|r| r.uuid == w.uuid) {
            // Review ownership survives loss of access to the app. This returns no
            // app details and only removes the authenticated account's own review.
            app.reviews.retain(|r| r.uuid != w.uuid);
            app.event(actor, "review.removed", json!({}));
            return Ok(json!({"status":"removed"}));
        }
        if !app.published || !app.visible(who) {
            return Err(ApiError::missing());
        }
        if m.method == "DELETE" {
            app.reviews.retain(|r| r.uuid != w.uuid);
            app.event(actor, "review.removed", json!({}));
            return Ok(json!({"status":"removed"}));
        }
        if m.method != "PUT" {
            return Err(ApiError::missing());
        }
        let rating = b["rating"]
            .as_u64()
            .filter(|r| (1..=5).contains(r))
            .ok_or_else(|| ApiError::bad("rating must be an integer from 1 to 5."))?
            as u8;
        let text = b["text"].as_str().unwrap_or_default();
        if text.chars().count() > 600 {
            return Err(ApiError::bad("Review text is limited to 600 characters."));
        }
        let review = Review {
            uuid: w.uuid.clone(),
            id: w.id.clone(),
            rating,
            text: text.into(),
            updated_at: now(),
        };
        app.reviews.retain(|r| r.uuid != w.uuid);
        app.reviews.push(review.clone());
        app.event(actor, "review.updated", json!({"rating":rating}));
        return Ok(json!(review));
    }
    author(app, who)?;
    if p.len() == 2 && m.method == "PATCH" {
        let obj = b
            .as_object()
            .ok_or_else(|| ApiError::bad("Expected a JSON object."))?;
        for key in obj.keys() {
            if ![
                "name",
                "description",
                "tags",
                "logo",
                "logo_alt",
                "banner",
                "banner_alt",
                "carousel",
                "links",
                "setup_step",
            ]
            .contains(&key.as_str())
            {
                return Err(ApiError::bad(format!(
                    "Field `{key}` cannot be changed here."
                )));
            }
        }
        for (key, dest) in [
            ("name", &mut app.name),
            ("description", &mut app.description),
            ("logo", &mut app.logo),
            ("banner", &mut app.banner),
            ("logo_alt", &mut app.logo_alt),
            ("banner_alt", &mut app.banner_alt),
        ] {
            if b.get(key).is_some() {
                *dest = str_field(b, key)?.to_owned();
            }
        }
        if b.get("tags").is_some() {
            app.tags = strings(b, "tags")?;
        }
        if let Some(links) = b.get("links") {
            app.links = links.clone();
        }
        if let Some(carousel) = b.get("carousel") {
            app.carousel = carousel
                .as_array()
                .ok_or_else(|| ApiError::bad("carousel must be an array."))?
                .clone();
        }
        if let Some(step) = b.get("setup_step") {
            app.setup_step =
                step.as_u64()
                    .filter(|n| (1..=7).contains(n))
                    .ok_or_else(|| ApiError::bad("setup_step must be 1–7."))? as u8;
        }
        validate_details(app)?;
        app.event(actor, "app.details_changed", b.clone());
        outbox.push(("accounts.sync".into(), json!({"app_id":app.app_id})));
        return Ok(app.view(who));
    }
    match p.get(2).copied().unwrap_or("") {
        "access" if m.method == "PUT" => {
            admin(app, who)?;
            let visibility = str_field(b, "visibility")?;
            if !["public", "private"].contains(&visibility) {
                return Err(ApiError::bad("visibility must be public or private."));
            }
            let domains = if b.get("domains").is_some() {
                strings(b, "domains")?
            } else {
                vec![]
            };
            if domains.len() > 100
                || domains.iter().any(|d| {
                    d.len() > 253
                        || !d.contains('.')
                        || d.contains(['@', '/', ':', ' '])
                        || d.split('.').any(|s| {
                            s.is_empty()
                                || s.starts_with('-')
                                || s.ends_with('-')
                                || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                        })
                })
            {
                return Err(ApiError::bad(
                    "domains must contain at most 100 valid email domains without @ or wildcards.",
                ));
            }
            let ids = if b.get("account_ids").is_some() {
                strings(b, "account_ids")?
            } else {
                vec![]
            };
            if ids.len() != m.prepared.identities.len() {
                return Err(ApiError::bad(
                    "Every shared account must resolve through Silicon Accounts.",
                ));
            }
            app.visibility = visibility.into();
            app.domains = domains.into_iter().map(|d| d.to_lowercase()).collect();
            app.account_ids = m.prepared.identities.iter().map(|i| i.id.clone()).collect();
            app.access_uuids = m
                .prepared
                .identities
                .iter()
                .map(|i| i.uuid.clone())
                .collect();
            app.event(actor, "app.access_changed", b.clone());
            Ok(app.view(who))
        }
        "publish" if m.method == "POST" => {
            let readiness = app.readiness();
            if readiness["ready"] != true {
                let mut e = ApiError::bad("Complete the required publishing steps first.");
                e.details = readiness;
                return Err(e);
            }
            app.published = true;
            app.setup_step = 7;
            app.event(actor, "app.published", json!({}));
            Ok(app.view(who))
        }
        "secret" if p.get(3) == Some(&"rotate") && m.method == "POST" => {
            let secret = m.prepared.secret.as_ref().ok_or_else(|| {
                ApiError::unavailable("Accounts secret rotation did not complete.")
            })?;
            app.secret_hash = hash(secret.as_bytes());
            app.event(actor, "app.secret_rotated", json!({}));
            Ok(json!({"app_secret":secret}))
        }
        "admin" if m.method == "POST" => {
            admin(app, who)?;
            let uuid = str_field(b, "uuid")?;
            if !app.authors.iter().any(|a| a.uuid == uuid) {
                return Err(ApiError::bad(
                    "New admin must already be an accepted author.",
                ));
            }
            app.admin_uuid = uuid.into();
            app.event(actor, "author.admin_transferred", b.clone());
            Ok(json!({"status":"transferred"}))
        }
        "authors" if p.len() == 4 => {
            if p[3] == "leave" && m.method == "POST" {
                if app.authors.len() == 1 {
                    return Err(ApiError::conflict(
                        "The last author cannot leave. Invite another author and wait for acceptance first.",
                    ));
                }
                app.authors.retain(|a| a.uuid != actor);
                if app.admin_uuid == actor {
                    app.admin_uuid = app
                        .authors
                        .iter()
                        .min_by_key(|a| &a.joined_at)
                        .unwrap()
                        .uuid
                        .clone();
                }
                app.event(actor, "author.left", json!({"uuid":actor}));
                outbox.push(("accounts.sync".into(), json!({"app_id":app.app_id})));
                Ok(json!({"status":"left"}))
            } else if m.method == "DELETE" {
                admin(app, who)?;
                if p[3] == actor {
                    return Err(ApiError::bad("Use authors/leave to leave an app."));
                }
                if !app.authors.iter().any(|a| a.uuid == p[3]) {
                    return Err(ApiError::missing());
                }
                app.authors.retain(|a| a.uuid != p[3]);
                app.event(actor, "author.removed", json!({"uuid":p[3]}));
                outbox.push(("accounts.sync".into(), json!({"app_id":app.app_id})));
                Ok(json!({"status":"removed"}))
            } else {
                Err(ApiError::missing())
            }
        }
        "invites" => {
            if p.len() == 3 && m.method == "POST" {
                let to = str_field(b, "to")?.trim();
                let resolved = m.prepared.identities.first();
                if !to.contains('@') && resolved.is_none() {
                    return Err(ApiError::bad("Invitee must resolve to a c:id or si:id."));
                }
                if to.len() > 254 || to.is_empty() || to.contains('@') && !valid_email(to) {
                    return Err(ApiError::bad("Invite destination is invalid."));
                }
                if resolved.is_some_and(|i| app.authors.iter().any(|a| a.uuid == i.uuid)) {
                    return Err(ApiError::conflict("This account is already an author."));
                }
                if c.invites.iter().any(|i| {
                    i.app_id == app.app_id
                        && i.status == "pending"
                        && (i.to.eq_ignore_ascii_case(to)
                            || resolved.is_some_and(|account| {
                                i.account_uuid.as_ref() == Some(&account.uuid)
                            }))
                }) {
                    return Err(ApiError::conflict(
                        "A pending invitation already exists for this account.",
                    ));
                }
                let invite = Invite {
                    id: new_id(),
                    app_id: app.app_id.clone(),
                    to: to.into(),
                    account_uuid: resolved.map(|i| i.uuid.clone()),
                    status: "pending".into(),
                    created_at: now(),
                };
                c.invites.push(invite.clone());
                app.event(actor, "author.invited", json!(invite));
                if to.contains('@') || to.starts_with("c:") {
                    outbox.push(("mail.invite".into(), json!(invite)));
                }
                Ok(json!(invite))
            } else if p.len() == 4 && m.method == "DELETE" {
                let i = c
                    .invites
                    .iter_mut()
                    .find(|i| i.id == p[3] && i.app_id == app.app_id)
                    .ok_or_else(ApiError::missing)?;
                if i.status != "pending" {
                    return Err(ApiError::conflict("This invitation is no longer pending."));
                }
                i.status = "cancelled".into();
                app.event(actor, "author.invite_cancelled", json!({"invite_id":i.id}));
                Ok(json!({"status":"cancelled"}))
            } else {
                Err(ApiError::missing())
            }
        }
        "packages" if p.len() == 4 && m.method == "POST" => {
            let package = m
                .prepared
                .package
                .clone()
                .ok_or_else(|| ApiError::bad("No validated package was supplied."))?;
            app.packages.push(package.clone());
            app.event(actor, "package.accepted", json!(package));
            Ok(json!(package))
        }
        "releases" if m.method == "POST" && p.len() == 5 && p[4] == "withdraw" => {
            let w = need(who)?;
            let reason = str_field(b, "reason")
                .map_err(|_| ApiError::bad("reason is required: say why the release is withdrawn, for example what breaks."))?
                .trim();
            if reason.is_empty() || reason.chars().count() > 500 {
                return Err(ApiError::bad("reason must contain 1 to 500 characters."));
            }
            let index = app
                .releases
                .iter()
                .position(|r| r.id == p[3])
                .ok_or_else(ApiError::missing)?;
            if let Some(w) = &app.releases[index].withdrawn {
                let mut error = ApiError::new(
                    axum::http::StatusCode::CONFLICT,
                    "release_withdrawn",
                    format!("This release was already withdrawn at {}.", w.at),
                    "Nothing to do. Publish a new release to replace it.",
                );
                error.details = json!({"reason":w.reason,"withdrawn_at":w.at});
                return Err(error);
            }
            app.releases[index].withdrawn = Some(Withdrawal {
                at: now(),
                by_uuid: w.uuid.clone(),
                by_id: w.id.clone(),
                reason: reason.into(),
            });
            let release = app.releases[index].clone();
            let replacement = app
                .latest(&release.channel)
                .map(|r| json!({"release_id":r.id,"version":r.version}));
            app.event(
                actor,
                "release.withdrawn",
                json!({"release_id":release.id,"version":release.version,"channel":release.channel,"reason":reason,"replacement":replacement}),
            );
            let mut view = json!(release);
            view["replacement"] = replacement.unwrap_or(Value::Null);
            Ok(view)
        }
        "releases" if m.method == "POST" => {
            let version = str_field(b, "version")?;
            if version_tuple(version).is_none() {
                return Err(ApiError::bad(
                    "version must be x.y.z without prefixes, prerelease identifiers or leading zeros.",
                ));
            }
            let (channel, package_ids, notes, promoted_from) = if p.len() == 5 && p[4] == "promote"
            {
                let dev = app
                    .releases
                    .iter()
                    .find(|r| r.id == p[3] && r.channel == "development")
                    .ok_or_else(ApiError::missing)?;
                if let Some(w) = &dev.withdrawn {
                    let mut error = ApiError::new(
                        axum::http::StatusCode::CONFLICT,
                        "release_withdrawn",
                        "A withdrawn release cannot be promoted.",
                        "Create a new development release with fixed packages and promote that one.",
                    );
                    error.details = json!({"reason":w.reason,"withdrawn_at":w.at});
                    return Err(error);
                }
                (
                    "production",
                    dev.package_ids.clone(),
                    dev.notes.clone(),
                    Some(dev.id.clone()),
                )
            } else if p.len() == 3 {
                (
                    "development",
                    strings(b, "package_ids")?,
                    b["notes"].as_str().unwrap_or_default().into(),
                    None,
                )
            } else {
                return Err(ApiError::missing());
            };
            if app
                .releases
                .iter()
                .any(|r| r.channel == channel && r.version == version)
            {
                return Err(ApiError::conflict(format!(
                    "{channel} version {version} already exists."
                )));
            }
            if package_ids.is_empty() {
                return Err(ApiError::bad("At least one validated package is required."));
            }
            let mut targets = std::collections::BTreeSet::new();
            for id in &package_ids {
                let pkg = app.packages.iter().find(|p| &p.id == id).ok_or_else(|| {
                    ApiError::bad(
                        "Every package_id must belong to this app and have passed validation.",
                    )
                })?;
                if !targets.insert(&pkg.target) {
                    return Err(ApiError::bad(
                        "A release can contain only one package for each target.",
                    ));
                }
            }
            let release = Release {
                id: new_id(),
                app_id: app.app_id.clone(),
                channel: channel.into(),
                version: version.into(),
                package_ids,
                notes,
                created_at: now(),
                promoted_from,
                signatures: BTreeMap::new(),
                signed_by_author: false,
                withdrawn: None,
            };
            app.releases.push(release.clone());
            app.event(
                actor,
                if channel == "production" {
                    "release.promoted"
                } else {
                    "release.created"
                },
                json!(release),
            );
            Ok(json!(release))
        }
        "webhook" => {
            let result = m.prepared.webhook.clone().ok_or_else(|| {
                ApiError::unavailable("Accounts webhook configuration did not complete.")
            })?;
            app.event(
                actor,
                "accounts.webhook_changed",
                json!({"events":b.get("events"),"url":b.get("url"),"rotated":p.len()==4}),
            );
            Ok(result)
        }
        _ => Err(ApiError::missing()),
    }
}
fn valid_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && !local.chars().any(char::is_whitespace)
        && domain.contains('.')
        && !domain.contains('@')
        && domain.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
pub(crate) fn mutation_route_exists(method: &str, p: &[&str]) -> bool {
    matches!(
        (method, p),
        ("POST", ["apps" | "platforms" | "reports" | "telemetry"])
            | ("PATCH", ["apps", _])
            | ("POST", ["invites", _, "accept" | "decline"])
            | ("PUT", ["apps", _, "access" | "webhook" | "review"])
            | ("DELETE", ["apps", _, "review"])
            | (
                "POST",
                [
                    "apps",
                    _,
                    "publish" | "admin" | "invites" | "releases" | "media" | "installs",
                ],
            )
            | ("POST", ["apps", _, "secret" | "webhook", "rotate"])
            | ("POST", ["apps", _, "authors", "leave"])
            | ("DELETE", ["apps", _, "authors" | "invites", _])
            | ("POST", ["apps", _, "packages", _])
            | ("POST", ["apps", _, "releases", _, "promote" | "withdraw"])
    )
}
fn validate_url(value: &str) -> bool {
    value.is_empty()
        || url::Url::parse(value).is_ok_and(|u| {
            matches!(u.scheme(), "http" | "https")
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        })
}
fn valid_media_path(value: &str) -> bool {
    let p: Vec<_> = value.split('/').collect();
    p.len() == 6
        && p[0].is_empty()
        && p[1] == "v1"
        && p[2] == "apps"
        && p[4] == "media"
        && !p[3].is_empty()
        && p[3].len() <= 30
        && p[3]
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        && p[5].len() == 64
        && p[5].bytes().all(|b| b.is_ascii_hexdigit())
}
fn validate_image_url(value: &str) -> bool {
    validate_url(value)
        || valid_media_path(value)
        || value.len() <= 1024 * 1024
            && [
                "data:image/png;base64,",
                "data:image/jpeg;base64,",
                "data:image/gif;base64,",
                "data:image/webp;base64,",
                "data:image/svg+xml;base64,",
            ]
            .iter()
            .any(|prefix| {
                value.strip_prefix(prefix).is_some_and(|data| {
                    use base64::Engine;
                    base64::engine::general_purpose::STANDARD
                        .decode(data)
                        .is_ok()
                })
            })
}
fn validate_details(a: &App) -> Result<()> {
    if a.name.trim().is_empty() || a.name.chars().count() > 120 {
        return Err(ApiError::bad("name must contain 1–120 characters."));
    }
    if a.description.chars().count() > 600 {
        return Err(ApiError::bad("description is limited to 600 characters."));
    }
    if a.published && a.description.chars().count() < 200 {
        return Err(ApiError::bad(
            "Published apps need at least 200 description characters.",
        ));
    }
    if a.logo_alt.chars().count() > 10000 || a.banner_alt.chars().count() > 10000 {
        return Err(ApiError::bad(
            "Logo and banner alt text are limited to 10,000 characters.",
        ));
    }
    if a.tags.len() > 20
        || a.tags
            .iter()
            .any(|t| t.trim().is_empty() || t.chars().count() > 60)
    {
        return Err(ApiError::bad(
            "Use up to 20 non-empty tags of at most 60 characters.",
        ));
    }
    if !validate_image_url(&a.logo) || !validate_image_url(&a.banner) {
        return Err(ApiError::bad("logo and banner must be HTTP(S) URLs."));
    }
    if a.carousel.len() > 20 {
        return Err(ApiError::bad("Carousel is limited to 20 images or videos."));
    }
    for media in &a.carousel {
        if !validate_image_url(str_field(media, "url")?)
            || !matches!(media["kind"].as_str(), Some("image" | "video"))
            || media["alt"].as_str().unwrap_or_default().chars().count() > 10000
        {
            return Err(ApiError::bad(
                "Media requires a valid URL, image/video kind, and alt text of at most 10,000 characters.",
            ));
        }
    }
    let links = a
        .links
        .as_object()
        .ok_or_else(|| ApiError::bad("links must be an object."))?;
    for (k, v) in links {
        if k == "custom" {
            let custom = v
                .as_array()
                .ok_or_else(|| ApiError::bad("links.custom must be an array."))?;
            if custom.len() > 4 {
                return Err(ApiError::bad("At most four custom links are allowed."));
            }
            for l in custom {
                if str_field(l, "label")?.is_empty()
                    || !validate_url(str_field(l, "url")?)
                    || !validate_url(l["logo"].as_str().unwrap_or_default())
                {
                    return Err(ApiError::bad(
                        "Custom links require a label and HTTP(S) URL.",
                    ));
                }
            }
        } else if !["website", "developer_docs", "android", "ios"].contains(&k.as_str())
            || !v.as_str().is_some_and(validate_url)
        {
            return Err(ApiError::bad(
                "Links must be website, developer_docs, android, ios or custom HTTP(S) links.",
            ));
        }
    }
    Ok(())
}
fn search_score(a: &App, q: &str) -> Option<u32> {
    if q.is_empty() {
        return Some(0);
    }
    let id = a.app_id.to_lowercase();
    let name = a.name.to_lowercase();
    if id == q {
        return Some(1000);
    }
    if name == q {
        return Some(950);
    }
    if id.starts_with(q) || name.starts_with(q) {
        return Some(800);
    }
    if id.contains(q) || name.contains(q) {
        return Some(700);
    }
    if a.tags.iter().any(|t| t.eq_ignore_ascii_case(q)) {
        return Some(600);
    }
    if a.description.to_lowercase().contains(q)
        || a.tags.iter().any(|t| t.to_lowercase().contains(q))
    {
        return Some(400);
    }
    if q.chars().count() >= 3 {
        let threshold = if q.chars().count() > 6 { 2 } else { 1 };
        if edit_distance(&id, q) <= threshold
            || edit_distance(&name, q) <= threshold
            || id
                .split(['-', '_'])
                .chain(name.split_whitespace())
                .any(|word| edit_distance(word, q) <= threshold)
        {
            return Some(300);
        }
        if a.tags.iter().any(|tag| {
            let tag = tag.to_lowercase();
            edit_distance(&tag, q) <= threshold
                || tag
                    .split_whitespace()
                    .any(|word| edit_distance(word, q) <= threshold)
        }) {
            return Some(200);
        }
        if a.description
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .any(|word| edit_distance(word, q) <= threshold)
        {
            return Some(100);
        }
    }
    None
}
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<_> = b.chars().collect();
    let mut prev: Vec<_> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut row = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            row.push(
                (row[j] + 1)
                    .min(prev[j + 1] + 1)
                    .min(prev[j] + usize::from(ca != *cb)),
            );
        }
        prev = row;
    }
    prev[b.len()]
}
pub(crate) fn validate_mutation(
    c: &mut Catalog,
    m: &Mutation<'_>,
    outbox: &mut Vec<(String, Value)>,
) -> Result<Value> {
    apply(c, m, outbox)
}
