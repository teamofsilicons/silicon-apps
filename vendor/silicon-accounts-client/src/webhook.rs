//! Webhook signature verification and typed event parsing.
//!
//! Every delivery is a `POST` with `Content-Type: application/json` and these headers:
//! `X-Accounts-Event-Id`, `X-Accounts-Event-Type`, `X-Accounts-Delivery-Id`,
//! `X-Accounts-Timestamp` (unix seconds) and
//! `X-Accounts-Signature: v1=<hex HMAC-SHA256(secret, "{timestamp}.{raw body}")>`.
//! Verify the signature over the raw body bytes before parsing, answer 2xx within 10 s,
//! and dedupe on `event_id` (retries and replays reuse it).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use time::OffsetDateTime;

use crate::error::{Error, Result, WebhookError};
use crate::serde_util::{lenient_opt_string, lenient_string, lenient_vec};
use crate::types::{AccountForApp, AccountKind, AccountRef};

/// Header with the event id (dedupe on it).
pub const EVENT_ID_HEADER: &str = "X-Accounts-Event-Id";
/// Header with the event type.
pub const EVENT_TYPE_HEADER: &str = "X-Accounts-Event-Type";
/// Header with the delivery id.
pub const DELIVERY_ID_HEADER: &str = "X-Accounts-Delivery-Id";
/// Header with the signing time (unix seconds).
pub const TIMESTAMP_HEADER: &str = "X-Accounts-Timestamp";
/// Header with the signature(s).
pub const SIGNATURE_HEADER: &str = "X-Accounts-Signature";
/// Default allowed clock difference for webhook timestamps.
pub const DEFAULT_WEBHOOK_TOLERANCE: Duration = Duration::from_secs(300);

type HmacSha256 = Hmac<Sha256>;

/// The `X-Accounts-Signature` value for a body: `v1=<hex>`. Useful for tests and for
/// tools that replay deliveries.
pub fn sign_webhook(secret: &str, timestamp: i64, raw_body: &[u8]) -> String {
    let Some(mut mac) = new_mac(secret) else {
        return String::new();
    };
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(raw_body);
    format!("v1={}", hex::encode(mac.finalize().into_bytes()))
}

/// HMAC accepts keys of any length, so this never returns `None` in practice.
fn new_mac(secret: &str) -> Option<HmacSha256> {
    HmacSha256::new_from_slice(secret.as_bytes()).ok()
}

/// Verifies a webhook delivery: the timestamp is within `tolerance` of this machine's
/// clock, and one of the `v1=` signatures in `signature_header` is the HMAC-SHA256 of
/// `"{timestamp}.{raw_body}"` with `secret` (the `whsec_…` value, used as-is as the key).
pub fn verify_webhook_signature(
    secret: &str,
    timestamp_header: &str,
    signature_header: &str,
    raw_body: &[u8],
    tolerance: Duration,
) -> Result<(), WebhookError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    verify_webhook_signature_at(
        secret,
        timestamp_header,
        signature_header,
        raw_body,
        tolerance,
        now,
    )
}

/// [`verify_webhook_signature`] with an explicit "now" (unix seconds), for tests.
pub fn verify_webhook_signature_at(
    secret: &str,
    timestamp_header: &str,
    signature_header: &str,
    raw_body: &[u8],
    tolerance: Duration,
    now_unix: u64,
) -> Result<(), WebhookError> {
    if secret.is_empty() {
        return Err(WebhookError::EmptySecret);
    }
    let timestamp_header = timestamp_header.trim();
    if timestamp_header.is_empty() {
        return Err(WebhookError::MissingHeader(TIMESTAMP_HEADER));
    }
    if signature_header.trim().is_empty() {
        return Err(WebhookError::MissingHeader(SIGNATURE_HEADER));
    }
    let timestamp: i64 = timestamp_header
        .parse()
        .map_err(|_| WebhookError::InvalidTimestamp(timestamp_header.chars().take(40).collect()))?;
    let age = (i128::from(timestamp) - i128::from(now_unix)).unsigned_abs();
    if age > u128::from(tolerance.as_secs()) {
        return Err(WebhookError::TimestampOutOfTolerance {
            age_seconds: u64::try_from(age).unwrap_or(u64::MAX),
            tolerance_seconds: tolerance.as_secs(),
        });
    }

    let candidates: Vec<Vec<u8>> = signature_header
        .split([',', ' '])
        .filter_map(|part| part.trim().strip_prefix("v1="))
        .filter_map(|hex_sig| hex::decode(hex_sig.trim()).ok())
        .collect();
    if candidates.is_empty() {
        return Err(WebhookError::InvalidSignatureFormat);
    }
    let Some(mut mac) = new_mac(secret) else {
        return Err(WebhookError::EmptySecret);
    };
    mac.update(timestamp_header.as_bytes());
    mac.update(b".");
    mac.update(raw_body);
    for candidate in &candidates {
        // verify_slice compares in constant time.
        if mac.clone().verify_slice(candidate).is_ok() {
            return Ok(());
        }
    }
    Err(WebhookError::SignatureMismatch)
}

/// Verifies and then parses a delivery.
pub fn verify_and_parse_webhook(
    secret: &str,
    timestamp_header: &str,
    signature_header: &str,
    raw_body: &[u8],
    tolerance: Duration,
) -> Result<WebhookEvent, WebhookError> {
    verify_webhook_signature(
        secret,
        timestamp_header,
        signature_header,
        raw_body,
        tolerance,
    )?;
    parse_webhook(raw_body).map_err(|e| WebhookError::InvalidBody(e.message()))
}

/// A webhook event: `{"event_id","type","occurred_at","app_id","silicon","data"}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct WebhookEvent {
    /// Unique per event and target; retries and replays reuse it. Dedupe on it.
    pub event_id: String,
    /// The event type, e.g. `account.id_changed`.
    #[serde(rename = "type")]
    pub event_type: String,
    /// When the change happened.
    #[serde(
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub occurred_at: Option<OffsetDateTime>,
    /// The receiving app (app webhooks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// The Silicon's uuid (Silicon webhooks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silicon: Option<String>,
    /// The raw `data` object.
    pub data: Value,
    /// `data` decoded according to `type`.
    #[serde(skip)]
    pub payload: WebhookPayload,
}

/// Typed event data.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub enum WebhookPayload {
    /// App: `account.id_changed`. Update the displayed id; keep keying on the uuid.
    AccountIdChanged(AccountIdChanged),
    /// App: `account.updated`. Fields the app may see changed.
    AccountUpdated(AccountUpdated),
    /// App: `account.deleted`. Delete or anonymise the user.
    AccountDeleted(MembershipEvent),
    /// App: `membership.signed_out`. The account's tokens for the app were revoked.
    MembershipSignedOut(MembershipSignedOut),
    /// App: `membership.access_removed`. The account removed the app's access.
    MembershipAccessRemoved(MembershipEvent),
    /// App: `silicon.custodian_changed`. A member Silicon has a new custodian.
    CustodianChanged(CustodianChanged),
    /// `ping` (test deliveries).
    Ping,
    /// Silicon: `silicon.created`.
    SiliconCreated(Value),
    /// Silicon: `silicon.custodian.accepted`. You can sign in now.
    SiliconCustodianAccepted(Value),
    /// Silicon: `silicon.custodian.declined`. The account was released.
    SiliconCustodianDeclined(Value),
    /// Silicon: `silicon.custodian.expired`. Nobody accepted within 14 days.
    SiliconCustodianExpired(Value),
    /// Silicon: `silicon.updated`.
    SiliconUpdated(Value),
    /// Silicon: `silicon.id_changed`.
    SiliconIdChanged(Value),
    /// Silicon: `silicon.stk_rotated`. The old STK no longer works.
    SiliconStkRotated(Value),
    /// Silicon: `silicon.custodian.changed` (a transfer was accepted).
    SiliconCustodianChanged(Value),
    /// A type this client version does not know; use `event_type` and `data`.
    #[default]
    Unknown,
}

/// `account.id_changed` data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AccountIdChanged {
    /// Account uuid.
    pub uuid: String,
    /// `{app_id}:{uuid}`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub membership_id: String,
    /// Carbon or Silicon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<AccountKind>,
    /// The previous id.
    #[serde(default, deserialize_with = "lenient_string")]
    pub old_id: String,
    /// The new id.
    #[serde(default, deserialize_with = "lenient_string")]
    pub new_id: String,
}

/// `account.updated` data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AccountUpdated {
    /// Account uuid.
    pub uuid: String,
    /// `{app_id}:{uuid}`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub membership_id: String,
    /// Changed fields (only ones the app may see).
    #[serde(default, deserialize_with = "lenient_vec")]
    pub changed: Vec<String>,
    /// The account after the change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<AccountForApp>,
}

/// Data of `account.deleted` and `membership.access_removed`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MembershipEvent {
    /// Account uuid.
    pub uuid: String,
    /// `{app_id}:{uuid}`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub membership_id: String,
}

/// `membership.signed_out` data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MembershipSignedOut {
    /// Account uuid.
    pub uuid: String,
    /// `{app_id}:{uuid}`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub membership_id: String,
    /// Why, e.g. `app_revoked`, `stk_rotated`.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub reason: Option<String>,
}

/// `silicon.custodian_changed` data (app webhooks).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CustodianChanged {
    /// The Silicon's uuid.
    pub uuid: String,
    /// `{app_id}:{uuid}`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub membership_id: String,
    /// The previous custodian.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<AccountRef>,
    /// The new custodian.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<AccountRef>,
}

#[derive(Deserialize)]
struct RawEvent {
    #[serde(default, deserialize_with = "lenient_string")]
    event_id: String,
    #[serde(rename = "type", default, deserialize_with = "lenient_string")]
    event_type: String,
    #[serde(default, with = "crate::serde_util::rfc3339_ms::option")]
    occurred_at: Option<OffsetDateTime>,
    #[serde(default, deserialize_with = "lenient_opt_string")]
    app_id: Option<String>,
    #[serde(default, deserialize_with = "lenient_opt_string")]
    silicon: Option<String>,
    #[serde(default)]
    data: Value,
}

/// Parses a webhook body into a typed event. Unknown types parse as
/// [`WebhookPayload::Unknown`] so new event types never break a receiver.
pub fn parse_webhook(raw_body: &[u8]) -> Result<WebhookEvent> {
    let raw: RawEvent = serde_json::from_slice(raw_body).map_err(|e| {
        Error::decode(
            format!("The webhook body is not a Silicon Accounts event: {e}."),
            "Pass the raw request body exactly as received.",
        )
    })?;
    if raw.event_id.is_empty() || raw.event_type.is_empty() {
        return Err(Error::decode(
            "The webhook body has no `event_id` or no `type`.",
            "Pass the raw request body exactly as received.",
        ));
    }
    let data = raw.data;
    let typed = |what: &str| -> Result<Value> {
        if data.is_object() {
            Ok(data.clone())
        } else {
            Err(Error::decode(
                format!("The `{what}` event has no `data` object."),
                "Pass the raw request body exactly as received.",
            ))
        }
    };
    let decode = |value: Value, what: &str| -> Result<WebhookPayload> {
        let parsed = match what {
            "account.id_changed" => {
                serde_json::from_value(value).map(WebhookPayload::AccountIdChanged)
            }
            "account.updated" => serde_json::from_value(value).map(WebhookPayload::AccountUpdated),
            "account.deleted" => serde_json::from_value(value).map(WebhookPayload::AccountDeleted),
            "membership.signed_out" => {
                serde_json::from_value(value).map(WebhookPayload::MembershipSignedOut)
            }
            "membership.access_removed" => {
                serde_json::from_value(value).map(WebhookPayload::MembershipAccessRemoved)
            }
            "silicon.custodian_changed" => {
                serde_json::from_value(value).map(WebhookPayload::CustodianChanged)
            }
            _ => Ok(WebhookPayload::Unknown),
        };
        parsed.map_err(|e| {
            Error::decode(
                format!("The `{what}` event's data does not have the documented shape: {e}."),
                "Make sure this client is up to date with the service.",
            )
        })
    };
    let payload = match raw.event_type.as_str() {
        "ping" => WebhookPayload::Ping,
        t @ ("account.id_changed"
        | "account.updated"
        | "account.deleted"
        | "membership.signed_out"
        | "membership.access_removed"
        | "silicon.custodian_changed") => decode(typed(t)?, t)?,
        "silicon.created" => WebhookPayload::SiliconCreated(data.clone()),
        "silicon.custodian.accepted" => WebhookPayload::SiliconCustodianAccepted(data.clone()),
        "silicon.custodian.declined" => WebhookPayload::SiliconCustodianDeclined(data.clone()),
        "silicon.custodian.expired" => WebhookPayload::SiliconCustodianExpired(data.clone()),
        "silicon.updated" => WebhookPayload::SiliconUpdated(data.clone()),
        "silicon.id_changed" => WebhookPayload::SiliconIdChanged(data.clone()),
        "silicon.stk_rotated" => WebhookPayload::SiliconStkRotated(data.clone()),
        "silicon.custodian.changed" => WebhookPayload::SiliconCustodianChanged(data.clone()),
        _ => WebhookPayload::Unknown,
    };
    Ok(WebhookEvent {
        event_id: raw.event_id,
        event_type: raw.event_type,
        occurred_at: raw.occurred_at,
        app_id: raw.app_id,
        silicon: raw.silicon,
        data,
        payload,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const SECRET: &str = "whsec_test_secret";
    const BODY: &[u8] = br#"{"event_id":"0192","type":"account.id_changed","occurred_at":"2026-10-06T12:00:00.000Z","app_id":"briefcase","silicon":null,"data":{"uuid":"a8K","membership_id":"briefcase:a8K","kind":"carbon","old_id":"c:saket","new_id":"c:saket2"}}"#;

    #[test]
    fn accepts_a_valid_signature() {
        let ts = 1_800_000_000_i64;
        let sig = sign_webhook(SECRET, ts, BODY);
        assert!(sig.starts_with("v1="));
        verify_webhook_signature_at(
            SECRET,
            &ts.to_string(),
            &sig,
            BODY,
            DEFAULT_WEBHOOK_TOLERANCE,
            1_800_000_010,
        )
        .unwrap();
        // Several signatures (e.g. during a secret rotation): any match is enough.
        let multi = format!("v1=deadbeef, {sig}");
        verify_webhook_signature_at(
            SECRET,
            &ts.to_string(),
            &multi,
            BODY,
            DEFAULT_WEBHOOK_TOLERANCE,
            1_800_000_010,
        )
        .unwrap();
    }

    #[test]
    fn rejects_tampering_wrong_secret_and_stale_timestamps() {
        let ts = 1_800_000_000_i64;
        let sig = sign_webhook(SECRET, ts, BODY);
        let tampered = String::from_utf8_lossy(BODY)
            .replace("saket2", "evil")
            .into_bytes();
        assert_eq!(
            verify_webhook_signature_at(
                SECRET,
                &ts.to_string(),
                &sig,
                &tampered,
                DEFAULT_WEBHOOK_TOLERANCE,
                1_800_000_000
            ),
            Err(WebhookError::SignatureMismatch)
        );
        assert_eq!(
            verify_webhook_signature_at(
                "whsec_other",
                &ts.to_string(),
                &sig,
                BODY,
                DEFAULT_WEBHOOK_TOLERANCE,
                1_800_000_000
            ),
            Err(WebhookError::SignatureMismatch)
        );
        assert!(matches!(
            verify_webhook_signature_at(
                SECRET,
                &ts.to_string(),
                &sig,
                BODY,
                DEFAULT_WEBHOOK_TOLERANCE,
                1_800_000_301
            ),
            Err(WebhookError::TimestampOutOfTolerance {
                age_seconds: 301,
                tolerance_seconds: 300
            })
        ));
        // A timestamp different from the signed one fails too.
        assert_eq!(
            verify_webhook_signature_at(
                SECRET,
                &(ts + 1).to_string(),
                &sig,
                BODY,
                DEFAULT_WEBHOOK_TOLERANCE,
                1_800_000_000
            ),
            Err(WebhookError::SignatureMismatch)
        );
        assert_eq!(
            verify_webhook_signature_at(
                SECRET,
                "yesterday",
                &sig,
                BODY,
                DEFAULT_WEBHOOK_TOLERANCE,
                1_800_000_000
            ),
            Err(WebhookError::InvalidTimestamp("yesterday".into()))
        );
        assert_eq!(
            verify_webhook_signature_at(
                SECRET,
                &ts.to_string(),
                "sha256=abc",
                BODY,
                DEFAULT_WEBHOOK_TOLERANCE,
                1_800_000_000
            ),
            Err(WebhookError::InvalidSignatureFormat)
        );
        assert_eq!(
            verify_webhook_signature_at(
                SECRET,
                "",
                &sig,
                BODY,
                DEFAULT_WEBHOOK_TOLERANCE,
                1_800_000_000
            ),
            Err(WebhookError::MissingHeader(TIMESTAMP_HEADER))
        );
        assert_eq!(
            verify_webhook_signature_at(
                "",
                &ts.to_string(),
                &sig,
                BODY,
                DEFAULT_WEBHOOK_TOLERANCE,
                1_800_000_000
            ),
            Err(WebhookError::EmptySecret)
        );
        let message = WebhookError::SignatureMismatch.to_string();
        assert!(message.contains("Hint:"), "{message}");
    }

    #[test]
    fn parses_typed_events() {
        let event = parse_webhook(BODY).unwrap();
        assert_eq!(event.event_id, "0192");
        assert_eq!(event.app_id.as_deref(), Some("briefcase"));
        match &event.payload {
            WebhookPayload::AccountIdChanged(data) => {
                assert_eq!(data.old_id, "c:saket");
                assert_eq!(data.new_id, "c:saket2");
                assert_eq!(data.kind, Some(AccountKind::Carbon));
            }
            other => panic!("unexpected payload {other:?}"),
        }

        let custodian = br#"{"event_id":"1","type":"silicon.custodian_changed","app_id":"remind","data":{"uuid":"b9Z","membership_id":"remind:b9Z","from":{"uuid":"a8K","id":"c:saket"},"to":{"uuid":"c7Q","id":"c:shubham"}}}"#;
        let event = parse_webhook(custodian).unwrap();
        match event.payload {
            WebhookPayload::CustodianChanged(data) => assert_eq!(data.to.unwrap().id, "c:shubham"),
            other => panic!("unexpected payload {other:?}"),
        }

        let ping = parse_webhook(br#"{"event_id":"2","type":"ping","data":{}}"#).unwrap();
        assert_eq!(ping.payload, WebhookPayload::Ping);
        let own = parse_webhook(br#"{"event_id":"3","type":"silicon.stk_rotated","silicon":"b9Z","data":{"uuid":"b9Z"}}"#).unwrap();
        assert!(matches!(own.payload, WebhookPayload::SiliconStkRotated(_)));
        let future =
            parse_webhook(br#"{"event_id":"4","type":"account.teleported","data":{}}"#).unwrap();
        assert_eq!(future.payload, WebhookPayload::Unknown);

        assert!(parse_webhook(b"not json").is_err());
        assert!(parse_webhook(br#"{"type":"ping"}"#).is_err());
        assert!(
            parse_webhook(
                br#"{"event_id":"5","type":"account.updated","data":{"membership_id":"x"}}"#
            )
            .is_err()
        );
    }
}
