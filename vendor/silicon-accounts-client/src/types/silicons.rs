//! Silicons and custodians.

use std::collections::BTreeMap;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::secret::Secret;
use crate::serde_util::{lenient_opt_string, lenient_string};
use crate::types::account::{AccountRef, AccountSummary, Me, PhotoInfo};

/// `POST /v1/silicons`: a Silicon creates its own account and names its custodian.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SiliconSelfCreate {
    /// The wanted id, e.g. `si:scout`.
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// The custodian Carbon: a `c:` id or an email address (an invitation is sent when the
    /// email has no account yet).
    pub custodian: String,
    /// IANA timezone (default UTC).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// Profile photo URL (default: the Silicon mark from Iris).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pfp_url: Option<String>,
    /// Self-chosen STK: `stk-` + 8..32 hex characters. When omitted one is generated and
    /// returned exactly once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stk: Option<String>,
    /// Webhook endpoint for notifications about this account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
}

/// Response of a Silicon self-create. Store `stk` and `webhook_secret` now: they are
/// shown exactly once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SiliconSelfCreated {
    /// The new account (status `pending_custodian`).
    pub silicon: Me,
    /// The generated STK (`None` when you chose one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stk: Option<Secret>,
    /// The custodian request.
    pub request: CustodianRequestInfo,
    /// Bearer token (`sarq_…`) to poll the request status.
    pub request_token: Secret,
    /// Signing secret for the webhook (when a webhook URL was given).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_secret: Option<Secret>,
}

/// The custodian request created with a self-created Silicon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CustodianRequestInfo {
    /// Request id.
    #[serde(deserialize_with = "lenient_string")]
    pub id: String,
    /// `pending`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// `initial` (a self-created Silicon's request).
    #[serde(default, deserialize_with = "lenient_string")]
    pub kind: String,
    /// The custodian has 14 days to accept.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<OffsetDateTime>,
    /// Who was asked (`c:saket` or a masked email).
    #[serde(default, deserialize_with = "lenient_string")]
    pub custodian: String,
}

/// `GET /v1/silicons/requests/{id}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CustodianRequestStatus {
    /// Request id.
    #[serde(deserialize_with = "lenient_string")]
    pub id: String,
    /// `pending`, `accepted`, `declined`, `expired` or `cancelled`.
    #[serde(deserialize_with = "lenient_string")]
    pub status: String,
    /// `initial` for a self-created Silicon.
    #[serde(default, deserialize_with = "lenient_string")]
    pub kind: String,
    /// Who was asked (`c:saket` or a masked email).
    #[serde(default, deserialize_with = "lenient_string")]
    pub custodian: String,
    /// When the request was made.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// When the request expires (14 days after creation).
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<OffsetDateTime>,
    /// When it was decided.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub decided_at: Option<OffsetDateTime>,
    /// The Silicon.
    pub silicon: RequestSilicon,
}

impl CustodianRequestStatus {
    /// True while the custodian has not decided.
    pub fn is_pending(&self) -> bool {
        self.status == "pending"
    }

    /// True when the custodian accepted (the Silicon can sign in).
    pub fn is_accepted(&self) -> bool {
        self.status == "accepted"
    }
}

/// The Silicon a custodian request is about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RequestSilicon {
    /// Account uuid.
    pub uuid: String,
    /// Its si:id; `None` (`null`) once the account was released after a decline or expiry.
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub id: Option<String>,
    /// Account status.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
}

/// `POST /v1/me/silicons`: a Carbon creates a Silicon and becomes its custodian.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CreateSilicon {
    /// The wanted id, e.g. `si:scout`.
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// IANA timezone (default UTC).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// Profile photo URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pfp_url: Option<String>,
    /// Self-chosen STK (`stk-` + 8..32 hex); generated when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stk: Option<String>,
    /// Webhook endpoint for the Silicon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
}

/// Response of `POST /v1/me/silicons`. `stk` and `webhook_secret` are shown exactly once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SiliconCreated {
    /// The new Silicon (active, you are its custodian).
    pub silicon: Me,
    /// The generated STK (`None` when you chose one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stk: Option<Secret>,
    /// The webhook signing secret, when a webhook URL was given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_secret: Option<Secret>,
}

/// Profile fields a custodian can change on a Silicon.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct UpdateSilicon {
    /// New display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// New IANA timezone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// New profile photo URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pfp_url: Option<String>,
}

impl UpdateSilicon {
    /// True when nothing would change.
    pub fn is_empty(&self) -> bool {
        self.display_name.is_none() && self.timezone.is_none() && self.pfp_url.is_none()
    }
}

/// Response of an STK rotation. The old STK stopped working and every token family of
/// the Silicon was revoked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StkRotated {
    /// The generated STK, shown exactly once (`None` when you chose one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stk: Option<Secret>,
    /// When the rotation happened.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub rotated_at: Option<OffsetDateTime>,
}

/// A Silicon's webhook endpoint and its signing secret (shown once each time it is set).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SiliconWebhook {
    /// The endpoint.
    #[serde(default, deserialize_with = "lenient_string")]
    pub webhook_url: String,
    /// `whsec_…`; verify every delivery with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_secret: Option<Secret>,
}

/// A test delivery was queued.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebhookTestResult {
    /// The `ping` event id.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub event_id: Option<String>,
}

/// A Silicon in the custodian's list, with any pending transfer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct ManagedSilicon {
    /// The Silicon.
    #[serde(flatten)]
    pub silicon: Me,
    /// A transfer waiting for the receiving Carbon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_transfer: Option<PendingTransfer>,
}

impl<'de> Deserialize<'de> for ManagedSilicon {
    // Accepts both `{...silicon fields, "pending_transfer": …}` and
    // `{"silicon": {...}, "pending_transfer": …}`.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut map = serde_json::Map::deserialize(deserializer)?;
        let pending = map.remove("pending_transfer").filter(|v| !v.is_null());
        let silicon_value = match map.remove("silicon") {
            Some(inner @ Value::Object(_)) => inner,
            Some(other) => {
                map.insert("silicon".to_owned(), other);
                Value::Object(map)
            }
            None => Value::Object(map),
        };
        let silicon = serde_json::from_value(silicon_value).map_err(de::Error::custom)?;
        let pending_transfer = pending
            .map(serde_json::from_value)
            .transpose()
            .map_err(de::Error::custom)?;
        Ok(Self {
            silicon,
            pending_transfer,
        })
    }
}

/// `POST /v1/me/silicons/{uuid}/photo`: the photo a custodian uploaded for its Silicon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SiliconPhotoUploaded {
    /// The Silicon's new profile photo URL.
    pub pfp_url: String,
    /// The stored image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<PhotoInfo>,
    /// The Silicon after the change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silicon: Option<ManagedSilicon>,
}

/// A pending custodian transfer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PendingTransfer {
    /// Request id.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub id: Option<String>,
    /// The receiving Carbon (an account or an email).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Value>,
    /// Created at.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// Expires at (14 days).
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<OffsetDateTime>,
    /// Any other fields.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// A custodian request (initial or transfer) as Carbons see it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CustodianRequest {
    /// Request id (accept/decline with it).
    #[serde(deserialize_with = "lenient_string")]
    pub id: String,
    /// `initial` or `transfer`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub kind: String,
    /// `pending`, `accepted`, `declined`, `expired` or `cancelled`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// The Silicon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silicon: Option<AccountSummary>,
    /// The current custodian (transfers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<AccountRef>,
    /// The receiving Carbon (an account or an email).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Value>,
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
}
