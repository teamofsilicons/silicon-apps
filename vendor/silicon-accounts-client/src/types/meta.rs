//! Public, unauthenticated information: service meta, id availability, keys, reports.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::serde_util::{
    lenient_bool, lenient_opt_string, lenient_string, lenient_u64, lenient_vec,
};

/// `GET /v1/meta`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Meta {
    /// `Silicon Accounts`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: String,
    /// Service version.
    #[serde(default, deserialize_with = "lenient_string")]
    pub version: String,
    /// `development`, `test` or `production`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub environment: String,
    /// The public URL (token issuer).
    #[serde(default, deserialize_with = "lenient_string")]
    pub public_url: String,
    /// Where apps are created.
    #[serde(default, deserialize_with = "lenient_string")]
    pub silicon_apps_url: String,
    /// Where the published docs live (absent on servers that don't report it).
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub docs_url: Option<String>,
    /// The developer platform, where apps' sign-in is set up (absent on servers that don't
    /// report it).
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub developer_url: Option<String>,
    /// Which managed sign-in providers are configured.
    #[serde(default)]
    pub providers: Providers,
    /// `local` (messages are recorded, not sent) or `providers`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub delivery: String,
}

/// Managed provider availability.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Providers {
    /// Managed Google sign-in is configured.
    #[serde(default, deserialize_with = "lenient_bool")]
    pub google: bool,
    /// Managed Apple sign-in is configured.
    #[serde(default, deserialize_with = "lenient_bool")]
    pub apple: bool,
}

/// `GET /v1/ids/available`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IdAvailability {
    /// The normalized id that was checked.
    #[serde(default, deserialize_with = "lenient_string")]
    pub id: String,
    /// Whether the id can be taken now.
    pub available: bool,
    /// `taken`, `reserved`, `reserved_word` or `invalid` when unavailable.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub reason: Option<String>,
    /// A precise explanation.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub message: Option<String>,
    /// True when the id is reserved for the signed-in caller (its previous owner), who may
    /// take it back within the 10-day reservation.
    #[serde(default, deserialize_with = "lenient_bool")]
    pub reclaimable: bool,
    /// Available ids close to the one asked for (when it can't be taken).
    #[serde(
        default,
        deserialize_with = "lenient_vec",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub suggestions: Vec<String>,
}

/// A JSON Web Key Set (`GET /.well-known/jwks.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Jwks {
    /// The signing keys.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub keys: Vec<Jwk>,
}

impl Jwks {
    /// Creates a key set.
    pub fn new(keys: Vec<Jwk>) -> Self {
        Self { keys }
    }

    /// The key with this `kid`.
    pub fn find(&self, kid: &str) -> Option<&Jwk> {
        self.keys.iter().find(|k| k.kid.as_deref() == Some(kid))
    }
}

/// One public key. Silicon Accounts publishes Ed25519 keys (`kty=OKP`, `crv=Ed25519`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Jwk {
    /// Key type (`OKP`).
    #[serde(default, deserialize_with = "lenient_string")]
    pub kty: String,
    /// Curve (`Ed25519`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crv: Option<String>,
    /// Public key, base64url.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    /// Key id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kid: Option<String>,
    /// `sig`.
    #[serde(default, rename = "use", skip_serializing_if = "Option::is_none")]
    pub use_: Option<String>,
    /// `EdDSA`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alg: Option<String>,
}

impl Jwk {
    /// An Ed25519 public key entry.
    pub fn ed25519(kid: impl Into<String>, x: impl Into<String>) -> Self {
        Self {
            kty: "OKP".to_owned(),
            crv: Some("Ed25519".to_owned()),
            x: Some(x.into()),
            kid: Some(kid.into()),
            use_: Some("sig".to_owned()),
            alg: Some("EdDSA".to_owned()),
        }
    }
}

/// `GET /.well-known/openid-configuration` (the commonly used fields; the rest are in
/// `extra`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OidcDiscovery {
    /// Issuer (`iss` of every token).
    pub issuer: String,
    /// Where apps send browsers to sign in.
    pub authorization_endpoint: String,
    /// The token endpoint.
    pub token_endpoint: String,
    /// The userinfo endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub userinfo_endpoint: Option<String>,
    /// The JWKS URL.
    pub jwks_uri: String,
    /// The revocation endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revocation_endpoint: Option<String>,
    /// The introspection endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub introspection_endpoint: Option<String>,
    /// The device authorization endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_authorization_endpoint: Option<String>,
    /// Everything else in the document.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Receipt for `POST /v1/reports`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReportReceipt {
    /// Report id.
    #[serde(deserialize_with = "lenient_string")]
    pub report_id: String,
    /// `queued`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// How many people it is mailed to.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub recipients: u64,
}

/// One telemetry event for `POST /v1/telemetry/events`. Keep events self-contained: say
/// where they come from (`source`), which `step` of a flow they describe and how far the
/// flow got (`progress`, 0.0..=1.0). Never put secrets or personal data in `data`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Where the event comes from, e.g. `cli`.
    pub source: String,
    /// The step of the flow, e.g. `login.device.approved`.
    pub step: String,
    /// Event name matching `^[a-z0-9_.]{1,64}$`.
    pub name: String,
    /// Progress through the flow, 0.0..=1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    /// Extra context.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub data: Value,
}

impl TelemetryEvent {
    /// True when `name` matches the service's allowlist pattern `^[a-z0-9_.]{1,64}$`.
    pub fn has_valid_name(&self) -> bool {
        !self.name.is_empty()
            && self.name.len() <= 64
            && self
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
    }
}
