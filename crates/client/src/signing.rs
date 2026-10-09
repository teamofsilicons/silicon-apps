//! Signed releases: checking the API's signature, and the author's when there
//! is one, before anything from a package is installed.
//!
//! Every package Apps serves comes with an Ed25519 signature over its release
//! manifest. This module rebuilds that manifest from the bytes actually
//! downloaded and checks the signature with a key this client trusts:
//!
//! - keys pinned in this crate for the official service ([`PINNED_KEYS`]);
//! - keys a trusted key endorsed in the service's keys document, which is how
//!   a key rotation reaches clients without a new release;
//! - for any other service with nothing pinned, the keys it published the
//!   first time this home talked to it (trust on first use).
//!
//! Trusted keys are kept per service in `.apps/trusted-keys.json`.

use crate::{Client, LocalState, Resolution, package, state};
use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};

pub const RELEASE_CONTEXT: &str = "silicon-apps-release-v1";
pub const AUTHOR_CONTEXT: &str = "silicon-apps-author-package-v1";
pub const ENDORSEMENT_CONTEXT: &str = "silicon-apps-key-endorsement-v1";
pub const KEYS_PATH: &str = ".well-known/silicon-apps-keys.json";

/// A release signing key built into this client.
#[derive(Debug, Clone, Copy)]
pub struct PinnedKey {
    /// The service it signs for, as [`crate::auth::service_scope`] writes it.
    pub server: &'static str,
    pub key_id: &'static str,
    /// Base64 Ed25519 public key.
    pub public_key: &'static str,
}
/// Keys pinned for the official service. A newer key is trusted when one of
/// these (or a key they endorsed) endorses it.
pub const PINNED_KEYS: &[PinnedKey] = &[PinnedKey {
    server: crate::DEFAULT_URL,
    key_id: "apps-2026-10",
    public_key: "5WC06dtS61w+mPv3E0xNVz5ZoNQNMpX5/8YutuLZ3DU=",
}];

/// A signature or checksum check that failed. Installing stops before any
/// file from the package is used.
#[derive(Debug, Clone)]
pub struct VerificationError {
    pub code: &'static str,
    pub message: String,
    pub hint: String,
    pub details: Value,
}
impl std::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}\nHint: {}", self.code, self.message, self.hint)?;
        if !self.details.is_null() {
            write!(
                f,
                "\nDetails: {}",
                serde_json::to_string_pretty(&self.details).unwrap_or_default()
            )?;
        }
        Ok(())
    }
}
impl std::error::Error for VerificationError {}
fn refuse(
    code: &'static str,
    message: impl Into<String>,
    hint: impl Into<String>,
    details: Value,
) -> anyhow::Error {
    anyhow::Error::new(VerificationError {
        code,
        message: message.into(),
        hint: hint.into(),
        details,
    })
}

/// The API's signature over one package of one release, from `resolve`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseSignature {
    pub key_id: String,
    pub algorithm: String,
    pub signature: String,
    #[serde(default)]
    pub manifest: Value,
}
/// The uploading author's signature, from `resolve`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorSignature {
    pub key_id: String,
    pub algorithm: String,
    pub public_key: String,
    pub signature: String,
    #[serde(default)]
    pub signer_uuid: String,
    #[serde(default)]
    pub signer_id: String,
}
/// What a successful check established.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Verified {
    /// The API key that signed the release manifest.
    pub key_id: String,
    /// The author key that also signed the package, if any.
    pub author_key_id: Option<String>,
    /// The c:id or si:id that signed as author, if any.
    pub author: Option<String>,
    /// The install script's SHA-256, or `None` when the target has none.
    pub install_script_sha256: Option<String>,
}

/// The message the API signs for a package of a release.
#[allow(clippy::too_many_arguments)]
pub fn release_message(
    app_id: &str,
    target: &str,
    version: &str,
    channel: &str,
    sha256: &str,
    size: u64,
    release_id: &str,
    install_script_sha256: Option<&str>,
) -> String {
    format!(
        "{RELEASE_CONTEXT}\napp_id={app_id}\ntarget={target}\nversion={version}\nchannel={channel}\nsha256={sha256}\nsize={size}\nrelease_id={release_id}\ninstall_script_sha256={}\n",
        install_script_sha256.unwrap_or("none")
    )
}
/// The message an author signs for a package before uploading it.
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
/// Check a base64 Ed25519 signature with a base64 public key.
pub fn verify(public_key: &str, message: &str, signature: &str) -> bool {
    let key = STANDARD
        .decode(public_key.trim())
        .ok()
        .and_then(|k| <[u8; 32]>::try_from(k.as_slice()).ok())
        .and_then(|k| VerifyingKey::from_bytes(&k).ok());
    let signature = STANDARD
        .decode(signature.trim())
        .ok()
        .and_then(|s| Signature::from_slice(&s).ok());
    match (key, signature) {
        (Some(key), Some(signature)) => key.verify(message.as_bytes(), &signature).is_ok(),
        _ => false,
    }
}
/// The ID Apps gives an author key: `ak_` and 16 hex characters of the
/// SHA-256 of the raw public key.
pub fn author_key_id(public_key: &str) -> Result<String> {
    let raw = STANDARD
        .decode(public_key.trim())
        .context("an author public key is base64")?;
    anyhow::ensure!(
        raw.len() == 32,
        "an Ed25519 public key is 32 bytes, not {}",
        raw.len()
    );
    Ok(format!("ak_{}", &hex::encode(Sha256::digest(raw))[..16]))
}

/// The install script digest a package's target runs, read from its bytes.
pub fn install_script_sha256(
    bytes: &[u8],
    manifest: &package::Manifest,
    target: &str,
) -> Result<Option<String>> {
    Ok(package::install_script(bytes, manifest, target)?.map(|(script, _)| script.sha256))
}

fn plain(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}

/// Check a downloaded package against its release signature, and its author
/// signature when it has one. Call this before extracting or running
/// anything from `bytes`; `manifest` is the archive's checked `apps.yaml`.
pub async fn verify_package(
    client: &Client,
    state: &LocalState,
    resolution: &Resolution,
    bytes: &[u8],
    manifest: &package::Manifest,
    target: &str,
) -> Result<Verified> {
    let sha256 = package::sha256(bytes);
    let size = bytes.len() as u64;
    if sha256 != resolution.package.sha256.to_ascii_lowercase() || size != resolution.package.size {
        return Err(refuse(
            "checksum_mismatch",
            "The downloaded bytes are not the package the release names.",
            "Nothing was installed. Try again; if it repeats, report it with silicon-apps report.",
            json!({"expected":{"sha256":resolution.package.sha256,"size":resolution.package.size},"received":{"sha256":sha256,"size":size}}),
        ));
    }
    let Some(signature) = &resolution.signature else {
        return Err(refuse(
            "release_unsigned",
            format!(
                "{} {} came without a signature, so it is not installed.",
                resolution.app_id, resolution.release.version
            ),
            "Every package Apps serves is signed. Check that --server points at a Silicon Apps service.",
            json!({"server":client.base_url()}),
        ));
    };
    if signature.algorithm != "ed25519" {
        return Err(refuse(
            "signature_algorithm_unsupported",
            format!("The release is signed with {}.", signature.algorithm),
            "Update silicon-apps; this version checks ed25519 signatures.",
            json!({"algorithm":signature.algorithm}),
        ));
    }
    let script = install_script_sha256(bytes, manifest, target)?;
    let version = &resolution.release.version;
    let release_id = &resolution.release.id;
    if !package::strict_version(version) || !plain(release_id) {
        return Err(refuse(
            "signature_mismatch",
            "The release names a version or ID that cannot have been signed.",
            "Nothing was installed. Report it with silicon-apps report.",
            json!({"version":version,"release_id":release_id}),
        ));
    }
    // Rebuild the signed manifest from what was downloaded, not from what
    // the server says about it.
    let expected = json!({
        "app_id":resolution.app_id,
        "target":target,
        "version":version,
        "channel":resolution.release.channel,
        "sha256":sha256,
        "size":size,
        "release_id":release_id,
        "install_script_sha256":script,
    });
    let differs: Vec<&str> = expected
        .as_object()
        .unwrap()
        .iter()
        .filter(|(field, value)| {
            signature
                .manifest
                .get(field.as_str())
                .is_some_and(|claimed| claimed != *value)
        })
        .map(|(field, _)| field.as_str())
        .collect();
    let public_key = trusted_key(client, state, &signature.key_id).await?;
    let message = release_message(
        &resolution.app_id,
        target,
        version,
        &resolution.release.channel,
        &sha256,
        size,
        release_id,
        script.as_deref(),
    );
    if !verify(&public_key, &message, &signature.signature) {
        return Err(refuse(
            "signature_mismatch",
            format!(
                "The signature by {} does not match {} {} as downloaded.",
                signature.key_id, resolution.app_id, version
            ),
            "Nothing was installed. The package or its release data changed after it was signed. Report it with silicon-apps report.",
            json!({"key_id":signature.key_id,"fields_that_differ":differs,"downloaded":expected}),
        ));
    }
    let mut verified = Verified {
        key_id: signature.key_id.clone(),
        author_key_id: None,
        author: None,
        install_script_sha256: script.clone(),
    };
    if let Some(author) = &resolution.author_signature {
        let message = author_message(&resolution.app_id, target, &sha256, size, script.as_deref());
        let key_matches = author_key_id(&author.public_key).is_ok_and(|id| id == author.key_id);
        if author.algorithm != "ed25519"
            || !key_matches
            || !verify(&author.public_key, &message, &author.signature)
        {
            return Err(refuse(
                "author_signature_mismatch",
                format!(
                    "The author signature by {} ({}) does not match this package.",
                    author.signer_id, author.key_id
                ),
                "Nothing was installed. Tell the app's authors, or report it with silicon-apps report.",
                json!({"key_id":author.key_id,"signer":author.signer_id}),
            ));
        }
        verified.author_key_id = Some(author.key_id.clone());
        verified.author = Some(author.signer_id.clone());
    }
    Ok(verified)
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TrustStore {
    #[serde(default)]
    servers: BTreeMap<String, ServerTrust>,
}
#[derive(Debug, Default, Serialize, Deserialize)]
struct ServerTrust {
    #[serde(default)]
    keys: Vec<TrustedKey>,
    #[serde(default)]
    revoked: Vec<String>,
    /// When the service's keys document was last read.
    #[serde(default)]
    checked_at: String,
}
/// A release signing key this home trusts for one service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedKey {
    pub key_id: String,
    pub public_key: String,
    /// `pinned`, `endorsed` (by `endorsed_by`) or `first_use`.
    pub source: String,
    #[serde(default)]
    pub endorsed_by: Option<String>,
    #[serde(default)]
    pub trusted_at: String,
}
#[derive(Debug, Deserialize)]
struct KeysDocument {
    #[serde(default)]
    keys: Vec<PublishedKey>,
    #[serde(default)]
    revoked: Vec<String>,
}
#[derive(Debug, Deserialize)]
struct PublishedKey {
    key_id: String,
    public_key: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    endorsements: Vec<Endorsement>,
}
#[derive(Debug, Deserialize)]
struct Endorsement {
    key_id: String,
    signature: String,
}

pub fn trust_path(state: &LocalState) -> PathBuf {
    state.root.join("trusted-keys.json")
}
/// The keys this home trusts for a service: pinned ones first.
pub fn trusted_keys(state: &LocalState, server: &str) -> Result<Vec<TrustedKey>> {
    let scope = crate::auth::service_scope(server)?;
    let store: TrustStore = state::read_json(&trust_path(state))?;
    let entry = store.servers.get(&scope);
    let revoked = |id: &str| entry.is_some_and(|e| e.revoked.iter().any(|r| r == id));
    let mut keys: Vec<TrustedKey> = PINNED_KEYS
        .iter()
        .filter(|p| p.server == scope && !revoked(p.key_id))
        .map(|p| TrustedKey {
            key_id: p.key_id.into(),
            public_key: p.public_key.into(),
            source: "pinned".into(),
            endorsed_by: None,
            trusted_at: String::new(),
        })
        .collect();
    for key in entry.map(|e| e.keys.clone()).unwrap_or_default() {
        if !revoked(&key.key_id) && !keys.iter().any(|k| k.key_id == key.key_id) {
            keys.push(key);
        }
    }
    Ok(keys)
}

/// How long a service's keys document is trusted before it is read again,
/// so revocations reach installs within this many seconds.
pub const KEYS_REFRESH_SECONDS: i64 = 600;

/// The public key of a release signing key this home trusts. The service's
/// keys document is read when the key is new (to follow a rotation) and at
/// least every [`KEYS_REFRESH_SECONDS`] (to see revocations).
pub async fn trusted_key(client: &Client, state: &LocalState, key_id: &str) -> Result<String> {
    let scope = crate::auth::service_scope(client.base_url())?;
    let local = trusted_keys(state, &scope)?
        .into_iter()
        .find(|k| k.key_id == key_id);
    let store: TrustStore = state::read_json(&trust_path(state))?;
    let fresh = store
        .servers
        .get(&scope)
        .and_then(|e| chrono::DateTime::parse_from_rfc3339(&e.checked_at).ok())
        .is_some_and(|at| {
            chrono::Utc::now().signed_duration_since(at).num_seconds() < KEYS_REFRESH_SECONDS
        });
    if let Some(key) = &local
        && fresh
    {
        return Ok(key.public_key.clone());
    }
    if let Err(error) = refresh_keys(client, state).await {
        // A trusted key keeps working while the keys document is unreachable.
        return match local {
            Some(key) => Ok(key.public_key),
            None => Err(error),
        };
    }
    let path = trust_path(state);
    let store: TrustStore = state::read_json(&path)?;
    let entry = store.servers.get(&scope);
    if entry.is_some_and(|e| e.revoked.iter().any(|r| r == key_id)) {
        return Err(refuse(
            "signing_key_revoked",
            format!("The release is signed by {key_id}, which {scope} revoked."),
            "Nothing was installed. Try again in a few minutes: the service re-signs releases with its active key.",
            json!({"key_id":key_id}),
        ));
    }
    let trusted = trusted_keys(state, &scope)?;
    trusted
        .iter()
        .find(|k| k.key_id == key_id)
        .map(|k| k.public_key.clone())
        .ok_or_else(|| {
            refuse(
                "untrusted_signing_key",
                format!(
                    "The release is signed by {key_id}, and no key this home trusts for {scope} endorses it."
                ),
                format!(
                    "Nothing was installed. If {scope} is a local development service whose data was reset, remove its entry from {} and try again. Otherwise report it with silicon-apps report.",
                    path.display()
                ),
                json!({"key_id":key_id,"trusted":trusted.iter().map(|k| &k.key_id).collect::<Vec<_>>(),"keys_url":format!("{scope}/{KEYS_PATH}")}),
            )
        })
}

/// Read the service's keys document now: apply its revocations, and trust
/// new keys that a trusted key endorses. With nothing trusted or pinned for
/// the service yet, trust the keys it publishes (first use).
pub async fn refresh_keys(client: &Client, state: &LocalState) -> Result<()> {
    let scope = crate::auth::service_scope(client.base_url())?;
    let document: KeysDocument = serde_json::from_value(
        client
            .request(
                "GET",
                &[".well-known", "silicon-apps-keys.json"],
                &[],
                None,
                None,
            )
            .await
            .map_err(|e| {
                refuse(
                    "signing_keys_unavailable",
                    format!("Could not read the signing keys of {scope}: {e:#}"),
                    "Nothing was installed. Try again when the service is reachable.",
                    json!({"keys_url":format!("{scope}/{KEYS_PATH}")}),
                )
            })?,
    )
    .context("the signing keys document is not valid")?;
    state.initialize()?;
    let _lock = state.lock("trust")?;
    let path = trust_path(state);
    let mut store: TrustStore = state::read_json(&path)?;
    let pinned = PINNED_KEYS.iter().any(|p| p.server == scope);
    let entry = store.servers.entry(scope.clone()).or_default();
    // Revocations only ever remove trust, so they are applied as published.
    for id in document.revoked.iter().cloned().chain(
        document
            .keys
            .iter()
            .filter(|k| k.status == "revoked")
            .map(|k| k.key_id.clone()),
    ) {
        if !entry.revoked.contains(&id) {
            entry.revoked.push(id.clone());
        }
        entry.keys.retain(|k| k.key_id != id);
    }
    let now = chrono::Utc::now().to_rfc3339();
    let mut known: Vec<(String, String)> = PINNED_KEYS
        .iter()
        .filter(|p| p.server == scope && !entry.revoked.iter().any(|r| r == p.key_id))
        .map(|p| (p.key_id.to_owned(), p.public_key.to_owned()))
        .chain(
            entry
                .keys
                .iter()
                .map(|k| (k.key_id.clone(), k.public_key.clone())),
        )
        .collect();
    let candidates: Vec<&PublishedKey> = document
        .keys
        .iter()
        .filter(|k| k.status != "revoked" && !entry.revoked.contains(&k.key_id))
        .collect();
    if known.is_empty() && !pinned {
        for key in &candidates {
            entry.keys.push(TrustedKey {
                key_id: key.key_id.clone(),
                public_key: key.public_key.clone(),
                source: "first_use".into(),
                endorsed_by: None,
                trusted_at: now.clone(),
            });
        }
    } else {
        // Follow endorsements from trusted keys until nothing new is trusted.
        loop {
            let mut added = false;
            for key in &candidates {
                if known.iter().any(|(id, _)| id == &key.key_id) {
                    continue;
                }
                let message = endorsement_message(&key.key_id, &key.public_key);
                let endorser = key.endorsements.iter().find(|e| {
                    known.iter().any(|(id, public)| {
                        id == &e.key_id && verify(public, &message, &e.signature)
                    })
                });
                if let Some(endorser) = endorser {
                    known.push((key.key_id.clone(), key.public_key.clone()));
                    entry.keys.push(TrustedKey {
                        key_id: key.key_id.clone(),
                        public_key: key.public_key.clone(),
                        source: "endorsed".into(),
                        endorsed_by: Some(endorser.key_id.clone()),
                        trusted_at: now.clone(),
                    });
                    added = true;
                }
            }
            if !added {
                break;
            }
        }
    }
    entry.checked_at = now;
    state::atomic_json(&path, &store)
}

/// An author's signing key kept in `.apps/keys/{key_id}.key`.
pub struct AuthorKey {
    pub key_id: String,
    pub public_key: String,
    key: SigningKey,
}
impl AuthorKey {
    pub fn generate() -> Self {
        Self::from_seed(rand::random::<[u8; 32]>())
    }
    fn from_seed(seed: [u8; 32]) -> Self {
        let key = SigningKey::from_bytes(&seed);
        let public_key = STANDARD.encode(key.verifying_key().to_bytes());
        Self {
            key_id: author_key_id(&public_key).expect("a 32 byte key"),
            public_key,
            key,
        }
    }
    /// Read a key file: the base64 32-byte seed on one line.
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read author key {}", path.display()))?;
        let seed = STANDARD
            .decode(raw.trim())
            .ok()
            .and_then(|s| <[u8; 32]>::try_from(s.as_slice()).ok())
            .with_context(|| {
                format!(
                    "{} is not an author key: expected a base64 32-byte Ed25519 seed",
                    path.display()
                )
            })?;
        Ok(Self::from_seed(seed))
    }
    /// A key ID from `silicon-apps keys list`, or a path to a key file.
    pub fn load(state: &LocalState, id_or_path: &str) -> Result<Self> {
        let path = std::path::Path::new(id_or_path);
        if path.is_file() {
            return Self::from_file(path);
        }
        let stored = key_path(state, id_or_path)?;
        anyhow::ensure!(
            stored.is_file(),
            "no private key for {id_or_path} in {}; pass the key file's path, or create one with `silicon-apps keys add`",
            stored.parent().unwrap().display()
        );
        Self::from_file(&stored)
    }
    /// Save the private key with owner-only permissions; never overwrites.
    pub fn save(&self, state: &LocalState) -> Result<PathBuf> {
        let path = key_path(state, &self.key_id)?;
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        use std::io::Write;
        let mut file = options
            .open(&path)
            .with_context(|| format!("cannot create {}", path.display()))?;
        file.write_all(STANDARD.encode(self.key.to_bytes()).as_bytes())?;
        file.write_all(b"\n")?;
        Ok(path)
    }
    pub fn sign(&self, message: &str) -> String {
        STANDARD.encode(self.key.sign(message.as_bytes()).to_bytes())
    }
    /// Sign one package for upload to `app_id` on `target`.
    pub fn sign_package(&self, app_id: &str, target: &str, bytes: &[u8]) -> Result<String> {
        let manifest = package::inspect_archive(bytes)?;
        let script = install_script_sha256(bytes, &manifest, target)?;
        Ok(self.sign(&author_message(
            app_id,
            target,
            &package::sha256(bytes),
            bytes.len() as u64,
            script.as_deref(),
        )))
    }
}
pub fn key_path(state: &LocalState, key_id: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        key_id.starts_with("ak_")
            && key_id.len() == 19
            && key_id[3..].bytes().all(|b| b.is_ascii_hexdigit()),
        "`{key_id}` is not an author key ID (ak_ and 16 hex characters)"
    );
    Ok(state.root.join("keys").join(format!("{key_id}.key")))
}
