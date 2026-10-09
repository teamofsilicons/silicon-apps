//! Signed releases.
//!
//! Every package Apps serves is signed. When a release is created or
//! promoted, the API signs each of its packages' release manifest (app,
//! target, version, channel, SHA-256, size, release ID and install script
//! SHA-256) with the active Ed25519 key. The keys come from the runtime secret
//! `APPS_SIGNING_KEYS`; local development generates one in the data
//! directory. Public keys are published at
//! `/.well-known/silicon-apps-keys.json`. Each key carries endorsements from
//! the keys before it, so a CLI that pinned an older key can trust a newer one
//! without a new release of the CLI.
//!
//! Authors may also sign their packages with their own Ed25519 keys. The CLI
//! then checks both signatures.

use crate::{
    config::Config,
    error::{ApiError, Result},
    model::*,
};
use axum::http::StatusCode;
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const ALGORITHM: &str = "ed25519";
/// First line of the message the API signs for one package of one release.
pub const RELEASE_CONTEXT: &str = "silicon-apps-release-v1";
/// First line of the message an author signs for one uploaded package.
pub const AUTHOR_CONTEXT: &str = "silicon-apps-author-package-v1";
/// First line of the message an older key signs to endorse a newer key.
pub const ENDORSEMENT_CONTEXT: &str = "silicon-apps-key-endorsement-v1";
/// Where the API publishes its public keys.
pub const KEYS_PATH: &str = "/.well-known/silicon-apps-keys.json";

/// The fields of a release manifest, in the order they are signed.
pub struct ReleaseFields<'a> {
    pub app_id: &'a str,
    pub target: &'a str,
    pub version: &'a str,
    pub channel: &'a str,
    pub sha256: &'a str,
    pub size: u64,
    pub release_id: &'a str,
    pub install_script_sha256: Option<&'a str>,
}
/// The exact bytes the API signs. One `name=value` line per field, values
/// never contain a line break, and a target without an install script signs
/// `install_script_sha256=none`.
pub fn release_message(f: &ReleaseFields<'_>) -> String {
    format!(
        "{RELEASE_CONTEXT}\napp_id={}\ntarget={}\nversion={}\nchannel={}\nsha256={}\nsize={}\nrelease_id={}\ninstall_script_sha256={}\n",
        f.app_id,
        f.target,
        f.version,
        f.channel,
        f.sha256,
        f.size,
        f.release_id,
        f.install_script_sha256.unwrap_or("none")
    )
}
/// The exact bytes an author signs for one package, before any release exists.
pub fn author_message(
    app_id: &str,
    target: &str,
    sha256: &str,
    size: u64,
    install_script_sha256: Option<&str>,
) -> String {
    format!(
        "{AUTHOR_CONTEXT}\napp_id={app_id}\ntarget={target}\nsha256={sha256}\nsize={size}\ninstall_script_sha256={}\n",
        install_script_sha256.unwrap_or("none")
    )
}
pub fn endorsement_message(key_id: &str, public_key: &str) -> String {
    format!("{ENDORSEMENT_CONTEXT}\nkey_id={key_id}\npublic_key={public_key}\n")
}
pub fn public_key(key: &SigningKey) -> String {
    STANDARD.encode(key.verifying_key().to_bytes())
}
/// Verify a base64 signature with a base64 public key.
pub fn verify(public_key: &str, message: &str, signature: &str) -> bool {
    let Ok(key) = STANDARD.decode(public_key.trim()) else {
        return false;
    };
    let Ok(key) = <[u8; 32]>::try_from(key.as_slice()) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&key) else {
        return false;
    };
    let Ok(signature) = STANDARD.decode(signature.trim()) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&signature) else {
        return false;
    };
    key.verify(message.as_bytes(), &signature).is_ok()
}
/// A stable ID for an author key: `ak_` and 16 hex characters of the
/// SHA-256 of the raw public key.
pub fn author_key_id(public_key: &str) -> Result<String> {
    let raw = decode_public_key(public_key)?;
    Ok(format!("ak_{}", &hex::encode(Sha256::digest(raw))[..16]))
}
pub fn decode_public_key(public_key: &str) -> Result<[u8; 32]> {
    let bad = || {
        ApiError::bad("public_key must be a base64 Ed25519 public key (32 bytes, 44 characters).")
    };
    let raw = STANDARD.decode(public_key.trim()).map_err(|_| bad())?;
    let raw = <[u8; 32]>::try_from(raw.as_slice()).map_err(|_| bad())?;
    VerifyingKey::from_bytes(&raw).map_err(|_| bad())?;
    Ok(raw)
}

fn valid_key_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// The API's signing keys. The first one signs; the others are older keys
/// that stay published and endorse the newer ones.
pub struct Keyring {
    keys: Vec<(String, SigningKey)>,
    revoked: Vec<String>,
    /// `secret` for APPS_SIGNING_KEYS, `generated` for a local development key.
    pub source: &'static str,
}
impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keyring")
            .field("active", &self.active_id())
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}
impl Keyring {
    /// Parse `key_id:base64-seed` entries separated by commas or newlines.
    pub fn parse(raw: &str, revoked: &[String]) -> Result<Self> {
        let bad = |message: String| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid_signing_keys",
                message,
                "Set APPS_SIGNING_KEYS to key_id:base64-32-byte-seed entries, newest first. Generate one with `apps-server signing-key generate`.",
            )
        };
        let mut keys: Vec<(String, SigningKey)> = vec![];
        for entry in raw
            .split([',', '\n'])
            .map(str::trim)
            .filter(|e| !e.is_empty())
        {
            let (id, seed) = entry
                .split_once(':')
                .ok_or_else(|| bad("Each signing key is key_id:base64-seed.".into()))?;
            if !valid_key_id(id) {
                return Err(bad(format!(
                    "Signing key ID `{id}` must be 1 to 64 letters, digits, dots, hyphens or underscores."
                )));
            }
            let seed = STANDARD
                .decode(seed.trim())
                .ok()
                .and_then(|s| <[u8; 32]>::try_from(s.as_slice()).ok())
                .ok_or_else(|| {
                    bad(format!(
                        "Signing key `{id}` must be a base64 32-byte Ed25519 seed."
                    ))
                })?;
            if keys.iter().any(|(existing, _)| existing == id) {
                return Err(bad(format!("Signing key ID `{id}` appears twice.")));
            }
            keys.push((id.to_owned(), SigningKey::from_bytes(&seed)));
        }
        if keys.is_empty() {
            return Err(bad("APPS_SIGNING_KEYS has no keys.".into()));
        }
        if revoked.contains(&keys[0].0) {
            return Err(bad(format!(
                "The active signing key `{}` is listed in APPS_REVOKED_SIGNING_KEYS.",
                keys[0].0
            )));
        }
        Ok(Self {
            keys,
            revoked: revoked.to_vec(),
            source: "secret",
        })
    }
    /// The configured keys, or a development key kept in the data directory
    /// when the service runs locally. A deployed service refuses to start
    /// without `APPS_SIGNING_KEYS`, because it could not sign what it serves.
    pub fn load(config: &Config) -> Result<Self> {
        if let Some(raw) = config.signing_keys.as_deref() {
            return Self::parse(raw, &config.revoked_signing_keys);
        }
        if !config.local_development() && !config.dev_auth {
            return Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "signing_keys_required",
                "APPS_SIGNING_KEYS is required: every package Apps serves is signed.",
                "Generate a key with `apps-server signing-key generate`, add its value to the runtime secret and pin its public key in the CLI.",
            ));
        }
        let path = config.data_dir.join("signing-keys");
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => {
                let (entry, _) =
                    generate(&format!("local-{}", hex::encode(rand::random::<[u8; 4]>())));
                write_secret(&path, &entry).map_err(|_| {
                    ApiError::unavailable("Cannot save the local development signing key.")
                })?;
                entry
            }
        };
        let mut ring = Self::parse(&raw, &config.revoked_signing_keys)?;
        ring.source = "generated";
        Ok(ring)
    }
    pub fn active_id(&self) -> &str {
        &self.keys[0].0
    }
    pub fn active_public_key(&self) -> String {
        public_key(&self.keys[0].1)
    }
    pub fn sign(&self, message: &str) -> (String, String) {
        let (id, key) = &self.keys[0];
        (
            id.clone(),
            STANDARD.encode(key.sign(message.as_bytes()).to_bytes()),
        )
    }
    /// The public key of a configured key, if it is one.
    pub fn public_key_of(&self, key_id: &str) -> Option<String> {
        self.keys
            .iter()
            .find(|(id, _)| id == key_id)
            .map(|(_, key)| public_key(key))
    }
    pub fn is_revoked(&self, key_id: &str) -> bool {
        self.revoked.iter().any(|id| id == key_id)
    }

    /// Record the configured public keys and their endorsements, so a key
    /// stays published (and keeps endorsing) after its secret is removed.
    pub fn record(&self, conn: &Connection) -> Result<()> {
        for (index, (id, key)) in self.keys.iter().enumerate().rev() {
            let public = public_key(key);
            let known: Option<String> = conn
                .query_row(
                    "SELECT public_key FROM signing_keys WHERE key_id=?1",
                    [id],
                    |r| r.get(0),
                )
                .ok();
            if known.as_ref().is_some_and(|known| known != &public) {
                return Err(ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "signing_key_id_reused",
                    format!("Signing key ID `{id}` was used before with another public key."),
                    "Give every new signing key a new key ID.",
                ));
            }
            let mut endorsements: Vec<Value> = match &known {
                Some(_) => conn.query_row(
                    "SELECT endorsements FROM signing_keys WHERE key_id=?1",
                    [id],
                    |r| Ok(serde_json::from_str(&r.get::<_, String>(0)?).unwrap_or_default()),
                )?,
                None => vec![],
            };
            // Every older configured key (later in the list) endorses this one.
            for (older_id, older) in &self.keys[index + 1..] {
                if endorsements
                    .iter()
                    .any(|e| e["key_id"] == older_id.as_str())
                {
                    continue;
                }
                let message = endorsement_message(id, &public);
                endorsements.push(json!({"key_id":older_id,"signature":STANDARD.encode(older.sign(message.as_bytes()).to_bytes())}));
            }
            conn.execute(
                "INSERT INTO signing_keys(key_id,public_key,first_seen_at,endorsements) VALUES(?1,?2,?3,?4) ON CONFLICT(key_id) DO UPDATE SET endorsements=excluded.endorsements",
                params![id, public, now(), Value::Array(endorsements).to_string()],
            )?;
        }
        Ok(())
    }
    /// `/.well-known/silicon-apps-keys.json`.
    pub fn document(&self, conn: &Connection, service: &str) -> Result<Value> {
        let mut statement = conn.prepare(
            "SELECT key_id,public_key,first_seen_at,endorsements FROM signing_keys ORDER BY first_seen_at DESC,key_id",
        )?;
        let rows: Vec<(String, String, String, String)> = statement
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut keys = vec![];
        for (id, public, first_seen, endorsements) in rows {
            let status = if self.is_revoked(&id) {
                "revoked"
            } else if id == self.active_id() {
                "active"
            } else {
                "retired"
            };
            let endorsements: Value =
                serde_json::from_str(&endorsements).unwrap_or_else(|_| json!([]));
            keys.push(json!({"key_id":id,"algorithm":ALGORITHM,"public_key":public,"status":status,"first_seen_at":first_seen,"endorsements":endorsements}));
        }
        // Put the active key first.
        keys.sort_by_key(|k| k["status"] != "active");
        Ok(json!({
            "issuer":APP_ID,
            "service":service,
            "algorithm":ALGORITHM,
            "active_key_id":self.active_id(),
            "keys":keys,
            "revoked":self.revoked,
            "messages":{
                "release":format!("{RELEASE_CONTEXT}\napp_id={{app_id}}\ntarget={{target}}\nversion={{version}}\nchannel={{channel}}\nsha256={{sha256}}\nsize={{size}}\nrelease_id={{release_id}}\ninstall_script_sha256={{install_script_sha256 or none}}\n"),
                "author":format!("{AUTHOR_CONTEXT}\napp_id={{app_id}}\ntarget={{target}}\nsha256={{sha256}}\nsize={{size}}\ninstall_script_sha256={{install_script_sha256 or none}}\n"),
                "endorsement":format!("{ENDORSEMENT_CONTEXT}\nkey_id={{key_id}}\npublic_key={{public_key}}\n"),
                "encoding":"UTF-8 text; signatures and public keys are standard base64."
            },
            "docs":"https://developers.teamofsilicons.com/docs/apps/learn/signed-releases"
        }))
    }
}

/// A new `key_id:seed` entry for APPS_SIGNING_KEYS and its public key.
pub fn generate(key_id: &str) -> (String, String) {
    let key = SigningKey::from_bytes(&rand::random::<[u8; 32]>());
    (
        format!("{key_id}:{}", STANDARD.encode(key.to_bytes())),
        public_key(&key),
    )
}
fn write_secret(path: &Path, value: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(value.as_bytes())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, value)
    }
}

/// Read the install script information of packages uploaded before Apps
/// recorded it. Archives that cannot be read stay unread and unsigned.
pub fn inspect_packages(c: &mut Catalog, packages_dir: &Path) -> usize {
    let mut count = 0;
    for app in c.apps.values_mut() {
        for package in app.packages.iter_mut().filter(|p| !p.inspected) {
            let Ok(bytes) = std::fs::read(packages_dir.join(&package.sha256)) else {
                continue;
            };
            let Ok(manifest) = silicon_apps_package::inspect_archive(&bytes) else {
                continue;
            };
            match silicon_apps_package::install_script(&bytes, &manifest, &package.target) {
                Ok(script) => {
                    package.install_script = script.map(|(s, _)| InstallScriptInfo {
                        path: s.path,
                        sha256: s.sha256,
                        size: s.size,
                    });
                    package.inspected = true;
                    count += 1;
                }
                Err(error) => eprintln!(
                    "package {} of {} could not be inspected: {error:#}",
                    package.id, app.app_id
                ),
            }
        }
    }
    count
}

/// Sign every package of every release that has no signature by the active
/// key yet. A signature by an older key is checked before it is replaced, so
/// rotating keys never signs data that was changed behind the API's back.
/// Returns how many signatures were added or replaced.
pub fn sign_catalog(c: &mut Catalog, ring: &Keyring) -> usize {
    let mut count = 0;
    for app in c.apps.values_mut() {
        // Most saves change nothing here: every package is already signed by
        // the active key.
        let settled = app.releases.iter().all(|r| {
            r.package_ids.iter().all(|id| {
                r.signatures
                    .get(id)
                    .is_some_and(|s| s.key_id == ring.active_id())
            })
        });
        if settled {
            continue;
        }
        let packages = app.packages.clone();
        for release in app.releases.iter_mut() {
            for package_id in release.package_ids.clone() {
                let Some(package) = packages.iter().find(|p| p.id == package_id) else {
                    continue;
                };
                if !package.inspected {
                    continue;
                }
                let script = package.install_script.as_ref().map(|s| s.sha256.as_str());
                let message = release_message(&ReleaseFields {
                    app_id: &release.app_id,
                    target: &package.target,
                    version: &release.version,
                    channel: &release.channel,
                    sha256: &package.sha256,
                    size: package.size,
                    release_id: &release.id,
                    install_script_sha256: script,
                });
                if let Some(existing) = release.signatures.get(&package_id) {
                    if existing.key_id == ring.active_id() {
                        continue;
                    }
                    if let Some(old) = ring.public_key_of(&existing.key_id)
                        && !verify(&old, &message, &existing.signature)
                    {
                        eprintln!(
                            "release {} package {} has a signature by {} that does not match its data; it was not re-signed",
                            release.id, package_id, existing.key_id
                        );
                        continue;
                    }
                }
                let (key_id, signature) = ring.sign(&message);
                release.signatures.insert(
                    package_id.clone(),
                    ReleaseSignature {
                        key_id,
                        algorithm: ALGORITHM.into(),
                        signature,
                        install_script_sha256: script.map(str::to_owned),
                        signed_at: now(),
                    },
                );
                count += 1;
            }
            release.signed_by_author = !release.package_ids.is_empty()
                && release.package_ids.iter().all(|id| {
                    packages
                        .iter()
                        .find(|p| &p.id == id)
                        .is_some_and(|p| p.author_signature.is_some())
                });
        }
    }
    count
}

/// The signature block `resolve` returns for one package of one release.
pub fn resolution_signature(release: &Release, package: &Package) -> Option<Value> {
    let signature = release.signatures.get(&package.id)?;
    Some(json!({
        "key_id":signature.key_id,
        "algorithm":signature.algorithm,
        "signature":signature.signature,
        "keys_url":KEYS_PATH,
        "manifest":{
            "app_id":release.app_id,
            "target":package.target,
            "version":release.version,
            "channel":release.channel,
            "sha256":package.sha256,
            "size":package.size,
            "release_id":release.id,
            "install_script_sha256":signature.install_script_sha256,
        }
    }))
}

/// An account's registered author keys.
pub fn author_keys(conn: &Connection, owner_uuid: &str) -> Result<Vec<Value>> {
    let mut statement = conn.prepare(
        "SELECT key_id,name,public_key,created_at,revoked_at,revoked_reason FROM author_keys WHERE owner_uuid=?1 ORDER BY created_at,key_id",
    )?;
    let rows = statement.query_map([owner_uuid], |r| {
        let revoked_at: Option<String> = r.get(4)?;
        Ok(json!({
            "key_id":r.get::<_,String>(0)?,
            "name":r.get::<_,String>(1)?,
            "algorithm":ALGORITHM,
            "public_key":r.get::<_,String>(2)?,
            "created_at":r.get::<_,String>(3)?,
            "status":if revoked_at.is_some() {"revoked"} else {"active"},
            "revoked_at":revoked_at,
            "revoked_reason":r.get::<_,Option<String>>(5)?,
        }))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
/// An active author key of this account, by ID.
pub fn active_author_key(conn: &Connection, owner_uuid: &str, key_id: &str) -> Result<String> {
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT public_key,revoked_at FROM author_keys WHERE key_id=?1 AND owner_uuid=?2",
            params![key_id, owner_uuid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    match row {
        Some((public, None)) => Ok(public),
        Some((_, Some(_))) => Err(invalid_author_signature(
            format!("Author key {key_id} is revoked."),
            json!({"key_id":key_id,"status":"revoked"}),
        )),
        None => Err(invalid_author_signature(
            format!("Author key {key_id} is not registered to your account."),
            json!({"key_id":key_id,"status":"unknown"}),
        )),
    }
}
pub fn invalid_author_signature(message: String, details: Value) -> ApiError {
    let mut error = ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_author_signature",
        message,
        "Sign the exact archive with an active key from `silicon-apps keys list`, or upload without --sign-key.",
    );
    error.details = details;
    error
}

/// `GET /v1/keys`, `POST /v1/keys` and `DELETE /v1/keys/{key_id}`: the
/// signed-in account's author keys.
pub async fn handle(
    s: &crate::Shared,
    method: &axum::http::Method,
    p: &[&str],
    headers: &axum::http::HeaderMap,
    raw: &axum::body::Bytes,
    who: Option<&Identity>,
) -> Result<axum::response::Response> {
    use axum::{Json, http::Method, response::IntoResponse};
    let who = who.ok_or_else(ApiError::auth)?;
    if *method == Method::GET {
        let store = s.store.lock().unwrap();
        return Ok(
            Json(json!({"items":author_keys(&store.connection, &who.uuid)?})).into_response(),
        );
    }
    crate::auth::check_csrf(s, headers)?;
    let key = crate::idempotency_key(headers)?;
    let _guard = s.mutation_gate.lock().await;
    let fingerprint = format!(
        "{}:{}:{}",
        method.as_str(),
        p.join("/"),
        crate::store::hash(raw)
    );
    let status = if *method == Method::POST {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    let mut store = s.store.lock().unwrap();
    if let Some(prior) = store.replay(&who.uuid, key, &fingerprint)? {
        return Ok((status, [("Idempotent-Replayed", "true")], Json(prior)).into_response());
    }
    let body: Value = if raw.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(raw)
            .map_err(|e| ApiError::bad(format!("Request JSON is invalid: {e}")))?
    };
    let fields = body
        .as_object()
        .ok_or_else(|| ApiError::bad("Expected a JSON object."))?;
    let not_found = || {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "author_key_not_found",
            "No author key with this ID belongs to your account.",
            "List your keys with GET /v1/keys or silicon-apps keys list.",
        )
    };
    let tx = store
        .connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let one = |tx: &Connection, key_id: &str| -> Result<Value> {
        author_keys(tx, &who.uuid)?
            .into_iter()
            .find(|k| k["key_id"] == key_id)
            .ok_or_else(not_found)
    };
    let response = match (method.as_str(), p) {
        ("POST", ["keys"]) => {
            for field in fields.keys() {
                if !["public_key", "name"].contains(&field.as_str()) {
                    return Err(ApiError::bad(format!(
                        "Field `{field}` is not part of an author key."
                    )));
                }
            }
            let public = fields
                .get("public_key")
                .and_then(Value::as_str)
                .ok_or_else(|| ApiError::bad("public_key is required."))?
                .trim()
                .to_owned();
            let key_id = author_key_id(&public)?;
            let name = fields
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_owned();
            if name.chars().count() > 100 || name.contains(['\n', '\r']) {
                return Err(ApiError::bad("name is one line of at most 100 characters."));
            }
            let taken: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM author_keys WHERE key_id=?1)",
                [&key_id],
                |r| r.get(0),
            )?;
            if taken {
                let mut error = ApiError::new(
                    StatusCode::CONFLICT,
                    "author_key_exists",
                    "This public key is already registered.",
                    "Generate a new key pair for a new key. A revoked key cannot be registered again.",
                );
                error.details = json!({"key_id":key_id});
                return Err(error);
            }
            let active: i64 = tx.query_row(
                "SELECT COUNT(*) FROM author_keys WHERE owner_uuid=?1 AND revoked_at IS NULL",
                [&who.uuid],
                |r| r.get(0),
            )?;
            if active >= 20 {
                return Err(ApiError::conflict(
                    "An account can have 20 active author keys. Revoke one you no longer use.",
                ));
            }
            tx.execute(
                "INSERT INTO author_keys(key_id,owner_uuid,owner_id,name,public_key,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
                params![key_id, who.uuid, who.id, name, public, now()],
            )?;
            json!({"key":one(&tx, &key_id)?})
        }
        ("DELETE", ["keys", key_id]) => {
            for field in fields.keys() {
                if field != "reason" {
                    return Err(ApiError::bad(format!(
                        "Field `{field}` is not accepted when revoking a key."
                    )));
                }
            }
            let reason = fields
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_owned();
            if reason.chars().count() > 500 {
                return Err(ApiError::bad("reason is limited to 500 characters."));
            }
            one(&tx, key_id)?;
            tx.execute(
                "UPDATE author_keys SET revoked_at=?1,revoked_reason=?2 WHERE key_id=?3 AND owner_uuid=?4 AND revoked_at IS NULL",
                params![now(), reason, key_id, who.uuid],
            )?;
            json!({"key":one(&tx, key_id)?})
        }
        _ => return Err(ApiError::missing()),
    };
    tx.execute(
        "INSERT INTO idempotency(actor,key,fingerprint,response,created_at) VALUES(?1,?2,?3,?4,?5)",
        params![who.uuid, key, fingerprint, response.to_string(), now()],
    )?;
    tx.commit()?;
    Ok((status, [("Idempotent-Replayed", "false")], Json(response)).into_response())
}
