//! Tokens: token responses, what apps see about an account, device sign-in.

use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};

use crate::secret::Secret;
use crate::serde_util::{lenient_i64, lenient_string, ymd};
use crate::types::account::{AccountKind, AccountRef};

/// Response of the token endpoint and of every sign-in call (Silicon login, CLI sign-in,
/// code exchange, SLT exchange, refresh).
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TokenResponse {
    /// JWT (EdDSA) valid for `expires_in` seconds (30 minutes).
    pub access_token: Secret,
    /// Always `Bearer`.
    #[serde(default = "bearer")]
    pub token_type: String,
    /// Seconds until the access token expires.
    pub expires_in: u64,
    /// Rotating refresh token (`sar_…`), valid up to 900 days from the first sign-in.
    /// Every use returns a new one; presenting a used one revokes the whole family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<Secret>,
    /// When the refresh token family ends.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub refresh_token_expires_at: Option<OffsetDateTime>,
    /// Space-separated granted scopes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// OIDC id token (when `openid` was requested).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_token: Option<Secret>,
    /// `{app_id}:{uuid}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub membership_id: Option<String>,
    /// The account as the app may see it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<AccountForApp>,
}

fn bearer() -> String {
    "Bearer".to_owned()
}

impl TokenResponse {
    /// Granted scopes as a list.
    pub fn scopes(&self) -> Vec<&str> {
        self.scope
            .as_deref()
            .map(|s| s.split_whitespace().collect())
            .unwrap_or_default()
    }

    /// When the access token expires, counted from `issued_at` (use the time you
    /// received the response).
    pub fn access_expires_at(&self, issued_at: OffsetDateTime) -> OffsetDateTime {
        let seconds = i64::try_from(self.expires_in).unwrap_or(i64::MAX);
        issued_at.saturating_add(time::Duration::seconds(seconds))
    }
}

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &self.access_token)
            .field("expires_in", &self.expires_in)
            .field("refresh_token", &self.refresh_token)
            .field("scope", &self.scope)
            .field("membership_id", &self.membership_id)
            .field("account", &self.account)
            .finish_non_exhaustive()
    }
}

/// What an app sees about an account. The granted scopes decide which optional fields
/// are present; `profile` is always granted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AccountForApp {
    /// Permanent account identifier. Key your user records on this (or `membership_id`).
    pub uuid: String,
    /// `{app_id}:{uuid}`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub membership_id: String,
    /// Carbon or Silicon.
    pub kind: AccountKind,
    /// Current public id; it can change, never key on it.
    #[serde(default, deserialize_with = "lenient_string")]
    pub id: String,
    /// Display name.
    #[serde(default, deserialize_with = "lenient_string")]
    pub display_name: String,
    /// Profile photo URL.
    #[serde(default, deserialize_with = "lenient_string")]
    pub pfp_url: String,
    /// Primary email (scope `email`, Carbons only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Whether the email is verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
    /// Primary phone (scope `phone`, Carbons only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    /// Whether the phone is verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone_verified: Option<bool>,
    /// Date of birth (scope `dob`).
    #[serde(default, with = "ymd::option", skip_serializing_if = "Option::is_none")]
    pub dob: Option<Date>,
    /// IANA timezone (scope `timezone`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// A Silicon's custodian (always present for Silicons).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custodian: Option<AccountRef>,
    /// When the account last changed.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub updated_at: Option<OffsetDateTime>,
    /// Monotonic version; use it to order webhook updates.
    #[serde(default, deserialize_with = "lenient_i64")]
    pub version: i64,
}

/// `GET /v1/userinfo`: [`AccountForApp`] plus the standard OIDC claim names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UserInfo {
    /// Silicon Apps-only domain access extension, present with granted email scope.
    /// Other app audiences retain the primary-email contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_emails: Option<Vec<String>>,
    /// The account fields.
    #[serde(flatten)]
    pub account: AccountForApp,
    /// OIDC `sub` (the uuid).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    /// OIDC `name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// OIDC `picture`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// OIDC `phone_number`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,
    /// OIDC `phone_number_verified`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone_number_verified: Option<bool>,
    /// OIDC `zoneinfo`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zoneinfo: Option<String>,
    /// OIDC `birthdate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub birthdate: Option<String>,
}

/// `POST /v1/oauth/introspect` (RFC 7662). Only tokens of the calling app are active.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Introspection {
    /// Whether the token is currently valid.
    pub active: bool,
    /// Account uuid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    /// Audience (app id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aud: Option<serde_json::Value>,
    /// Expiry (unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp: Option<i64>,
    /// Issued at (unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iat: Option<i64>,
    /// Space-separated scopes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// `carbon` or `silicon`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<AccountKind>,
    /// Public id at issue time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// `{app_id}:{uuid}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub membership_id: Option<String>,
    /// `access_token` or `refresh_token`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    /// Issuer (the service's public URL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
    /// The app the token was issued to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// Not before (unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,
    /// Token id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
    /// The account's current public id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

/// `POST /v1/device/authorize`: show `user_code` and `verification_uri` to the Carbon,
/// then poll with the device code.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DeviceAuthorization {
    /// Poll the token endpoint with this (`sad_…`); never show it.
    pub device_code: Secret,
    /// The code the Carbon confirms on the site, e.g. `WDJB-MJHT`.
    pub user_code: String,
    /// Where the Carbon approves, e.g. `https://accounts.teamofsilicons.com/device`.
    pub verification_uri: String,
    /// The same page with the code filled in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_uri_complete: Option<String>,
    /// Seconds until the codes expire (600).
    pub expires_in: u64,
    /// Minimum seconds between polls (5).
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

impl std::fmt::Debug for DeviceAuthorization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceAuthorization")
            .field("device_code", &self.device_code)
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .field("expires_in", &self.expires_in)
            .field("interval", &self.interval)
            .finish_non_exhaustive()
    }
}

impl DeviceAuthorization {
    /// The URL to open in a browser: the complete one when available.
    pub fn browser_url(&self) -> &str {
        self.verification_uri_complete
            .as_deref()
            .unwrap_or(&self.verification_uri)
    }
}

/// One poll of a device sign-in.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DevicePoll {
    /// Not approved yet; poll again after `interval` seconds.
    Pending,
    /// Polling too fast; add 5 seconds to the interval.
    SlowDown,
    /// The Carbon denied the sign-in.
    Denied,
    /// The device code expired (10 minutes).
    Expired,
    /// Approved: the first-party tokens.
    Tokens(Box<TokenResponse>),
}

/// `POST /v1/cli/login/start`: a code was sent to an existing Carbon's email or phone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CliLoginChallenge {
    /// Pass to `cli_login_verify` with the code.
    pub challenge_id: String,
    /// Masked destination, e.g. `s***@gmail.com`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub destination: String,
    /// When the code expires (10 minutes).
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<OffsetDateTime>,
}

/// A short-lived token (`slt_…`) an app exchanges for the account's tokens. Single use,
/// valid 2 minutes, bound to one app.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ShortLivedToken {
    /// The token to hand to the app.
    pub slt: Secret,
    /// The app it is bound to.
    pub app_id: String,
    /// When it expires.
    #[serde(with = "crate::serde_util::rfc3339_ms")]
    pub expires_at: OffsetDateTime,
}

impl std::fmt::Debug for ShortLivedToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShortLivedToken")
            .field("slt", &self.slt)
            .field("app_id", &self.app_id)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}
