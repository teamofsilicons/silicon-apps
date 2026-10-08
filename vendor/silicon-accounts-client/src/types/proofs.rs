//! Proofs: OBO (on behalf of an account) and ATA (app to app).

use std::fmt;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::secret::Secret;
use crate::serde_util::{lenient_opt_string, lenient_string, lenient_vec};
use crate::types::account::{AccountKind, AccountSummary, AppSummary};

/// The kind of proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofKind {
    /// On behalf of: app A acts at app B for an account that consented at app A.
    Obo,
    /// App to app: app A proves its identity to one other app (one proof per app).
    Ata,
}

impl ProofKind {
    /// `obo` or `ata`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Obo => "obo",
            Self::Ata => "ata",
        }
    }
}

impl fmt::Display for ProofKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The account an OBO proof speaks for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProofUser {
    /// Account uuid.
    pub uuid: String,
    /// Current public id.
    #[serde(default, deserialize_with = "lenient_string")]
    pub id: String,
    /// Carbon or Silicon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<AccountKind>,
    /// The membership with the app (`{app_id}:{uuid}`).
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub membership_id: Option<String>,
}

/// A newly issued (or refreshed) proof. Keep `proof_refresh_token` on the issuing app's
/// side; send `proof_token` to the receiving app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IssuedProof {
    /// Proof family id (revoke with it).
    #[serde(default, deserialize_with = "lenient_string")]
    pub proof_id: String,
    /// `obo` or `ata`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ProofKind>,
    /// `sap_…`: what the receiving app verifies.
    pub proof_token: Secret,
    /// When `proof_token` expires.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<OffsetDateTime>,
    /// `sapr_…`: get a new proof token with it (rotates on every use).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_refresh_token: Option<Secret>,
    /// When the refresh token stops working.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub refresh_expires_at: Option<OffsetDateTime>,
    /// The issuing app id.
    #[serde(
        default,
        deserialize_with = "app_id_or_object",
        skip_serializing_if = "Option::is_none"
    )]
    pub issuing_app: Option<String>,
    /// The one app that verifies the proof (OBO and ATA alike).
    #[serde(
        default,
        deserialize_with = "app_id_or_object",
        skip_serializing_if = "Option::is_none"
    )]
    pub receiving_app: Option<String>,
    /// OBO: the account; ATA: `None` (serialized as `null`, like the service sends it).
    #[serde(default)]
    pub user: Option<ProofUser>,
    /// App-defined scopes.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub scopes: Vec<String>,
}

fn app_id_or_object<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(Value::Object(map)) => {
            Ok(map.get("app_id").and_then(Value::as_str).map(str::to_owned))
        }
        Some(_) => Err(de::Error::custom("expected an app id or an app object")),
    }
}

/// An app reference in a verification result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProofApp {
    /// App id.
    pub app_id: String,
    /// App name.
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: String,
}

/// The result of `POST /v1/proofs/verify`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ProofVerification {
    /// The proof is valid for the calling app right now.
    Valid(Box<ValidProof>),
    /// Unknown, expired, revoked, for another app, or about an account or app that is no
    /// longer valid. The service deliberately does not say which.
    Invalid,
}

impl ProofVerification {
    /// True for [`ProofVerification::Valid`].
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid(_))
    }

    /// The details of a valid proof.
    pub fn valid(&self) -> Option<&ValidProof> {
        match self {
            Self::Valid(proof) => Some(proof),
            Self::Invalid => None,
        }
    }

    /// The JSON the service returned (`{"valid":false,"expires_at":null}` when invalid).
    pub fn to_json(&self) -> Value {
        match self {
            Self::Valid(proof) => {
                let mut value = serde_json::to_value(proof.as_ref()).unwrap_or(Value::Null);
                if let Value::Object(map) = &mut value {
                    map.insert("valid".to_owned(), Value::Bool(true));
                }
                value
            }
            Self::Invalid => serde_json::json!({ "valid": false, "expires_at": null }),
        }
    }
}

/// Details of a valid proof.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ValidProof {
    /// Proof family id.
    #[serde(default, deserialize_with = "lenient_string")]
    pub proof_id: String,
    /// `obo` or `ata`.
    pub kind: ProofKind,
    /// Valid until.
    #[serde(with = "crate::serde_util::rfc3339_ms")]
    pub expires_at: OffsetDateTime,
    /// Who issued it.
    pub issuing_app: ProofApp,
    /// Who it is for (the verifying app).
    pub receiving_app: ProofApp,
    /// OBO: the account it speaks for; ATA: `None` (serialized as `null`, like the service).
    #[serde(default)]
    pub user: Option<ProofUser>,
    /// App-defined scopes.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub scopes: Vec<String>,
}

/// Identifies a proof to revoke.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProofRef {
    /// The proof id.
    Id(String),
    /// A proof token (`sap_…`) of the proof.
    Token(String),
    /// A proof refresh token (`sapr_…`) of the proof.
    RefreshToken(String),
}

impl ProofRef {
    pub(crate) fn to_json(&self) -> Value {
        match self {
            Self::Id(id) => serde_json::json!({ "proof_id": id }),
            Self::Token(token) => serde_json::json!({ "proof_token": token }),
            Self::RefreshToken(token) => serde_json::json!({ "proof_refresh_token": token }),
        }
    }
}

/// A proof issued by an app (`GET /v1/apps/{app_id}/proofs`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AppProof {
    /// Proof id.
    #[serde(deserialize_with = "lenient_string")]
    pub proof_id: String,
    /// `obo` or `ata`.
    pub kind: ProofKind,
    /// The one app that verifies it.
    #[serde(default, deserialize_with = "lenient_string")]
    pub receiving_app: String,
    /// OBO: the account; ATA: `None` (serialized as `null`, like the service sends it).
    #[serde(default)]
    pub user: Option<AccountSummary>,
    /// Scopes.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub scopes: Vec<String>,
    /// Created at.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// When the proof family expires.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<OffsetDateTime>,
    /// Last refresh.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_refreshed_at: Option<OffsetDateTime>,
    /// When it was revoked.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub revoked_at: Option<OffsetDateTime>,
    /// `active`, `revoked` or `expired` (a proof whose sign-in, membership or account
    /// ended reads `revoked` with the matching `revoke_reason`).
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// Why it ended, e.g. `revoked_by_app`, `sign_in_revoked`, `access_removed`.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub revoke_reason: Option<String>,
    /// When the current proof token expires.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub token_expires_at: Option<OffsetDateTime>,
    /// Lifetime of each proof token, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_ttl_seconds: Option<u64>,
}

/// An OBO proof issued about the signed-in account (`GET /v1/me/proofs`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MyProof {
    /// Proof id (revoke with it).
    #[serde(deserialize_with = "lenient_string")]
    pub proof_id: String,
    /// The app acting on your behalf.
    pub issuing_app: AppSummary,
    /// The app it acts at.
    pub receiving_app: AppSummary,
    /// Scopes.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub scopes: Vec<String>,
    /// Created at.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// Expires at.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<OffsetDateTime>,
    /// Last refresh.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_refreshed_at: Option<OffsetDateTime>,
    /// `active`, `revoked` or `expired`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// When it was revoked.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub revoked_at: Option<OffsetDateTime>,
    /// Why it ended, e.g. `revoked_by_account`, `sign_in_revoked`, `access_removed`.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub revoke_reason: Option<String>,
    /// When the current proof token expires.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub token_expires_at: Option<OffsetDateTime>,
}

/// Filters for an app's proofs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofsQuery {
    /// `obo` or `ata`.
    pub kind: Option<String>,
    /// `active` or `revoked`.
    pub status: Option<String>,
    /// Items per page (max 200).
    pub limit: Option<u32>,
    /// Cursor from the previous page.
    pub cursor: Option<String>,
}

/// `POST /v1/proofs/obo`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IssueObo {
    /// The account's access token issued to the calling app (after the account consented
    /// at the calling app).
    pub subject_token: String,
    /// The app the proof is for.
    pub receiving_app: String,
    /// App-defined scopes (≤ 20, each ≤ 100 chars of `[A-Za-z0-9_.:/-]`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    /// Proof token lifetime, 60..=1800 seconds (default 1800).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_ttl_seconds: Option<u32>,
}

/// `POST /v1/proofs/ata` (or the app's ATA page, `POST /v1/apps/{app_id}/proofs/ata`). An
/// ATA proof is for exactly one app: to talk to several apps, issue one proof per app.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IssueAta {
    /// The one app that may verify the proof.
    pub receiving_app: String,
    /// App-defined scopes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    /// Proof token lifetime, 60..=1800 seconds (default 1800).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_ttl_seconds: Option<u32>,
}
