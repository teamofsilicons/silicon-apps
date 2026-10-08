//! Accounts: identity, profile, contacts, sessions, history.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{Date, OffsetDateTime};

use crate::serde_util::{
    lenient_bool, lenient_i64, lenient_opt_string, lenient_string, lenient_u64, lenient_vec, ymd,
};

/// Whether an account belongs to a Carbon or a Silicon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    /// A Carbon account (`c:` ids).
    Carbon,
    /// A Silicon account (`si:` ids).
    Silicon,
}

impl AccountKind {
    /// `"carbon"` or `"silicon"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Carbon => "carbon",
            Self::Silicon => "silicon",
        }
    }

    /// The id prefix: `c:` or `si:`.
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Carbon => "c:",
            Self::Silicon => "si:",
        }
    }

    /// `"Carbon"` or `"Silicon"`.
    pub fn title(self) -> &'static str {
        match self {
            Self::Carbon => "Carbon",
            Self::Silicon => "Silicon",
        }
    }

    /// The kind an id belongs to, from its prefix (`c:saket` → Carbon).
    pub fn of_id(id: &str) -> Option<Self> {
        let lower = id.trim().to_ascii_lowercase();
        if lower.starts_with("c:") {
            Some(Self::Carbon)
        } else if lower.starts_with("si:") {
            Some(Self::Silicon)
        } else {
            None
        }
    }
}

impl fmt::Display for AccountKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AccountKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "carbon" | "c" => Ok(Self::Carbon),
            "silicon" | "si" => Ok(Self::Silicon),
            other => Err(format!(
                "`{other}` is not an account kind; use carbon or silicon"
            )),
        }
    }
}

/// Public identity of an account (lookups, custodians, lists).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AccountSummary {
    /// Permanent account identifier (base62, case-sensitive). Store this, not the id.
    pub uuid: String,
    /// Carbon or Silicon.
    pub kind: AccountKind,
    /// Current public id (`c:saket`, `si:scout`); empty for deleted accounts.
    #[serde(default, deserialize_with = "lenient_string")]
    pub id: String,
    /// Display name.
    #[serde(default, deserialize_with = "lenient_string")]
    pub display_name: String,
    /// Profile photo URL.
    #[serde(default, deserialize_with = "lenient_string")]
    pub pfp_url: String,
    /// `active`, `unclaimed`, `pending_custodian` or `deleted`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// A Silicon's custodian (present on Silicon lookups).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custodian: Option<AccountRef>,
}

/// A reference to another account. Only `uuid` is guaranteed; the rest depends on where
/// the reference appears.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AccountRef {
    /// Permanent account identifier.
    pub uuid: String,
    /// Current public id, when known.
    #[serde(default, deserialize_with = "lenient_string")]
    pub id: String,
    /// Carbon or Silicon, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<AccountKind>,
    /// Display name, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Profile photo URL, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pfp_url: Option<String>,
    /// Account status, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// The signed-in account's full view of itself (`GET /v1/me`). Also used for the
/// Silicon views custodians see.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Me {
    /// Permanent account identifier.
    pub uuid: String,
    /// Carbon or Silicon.
    pub kind: AccountKind,
    /// Current public id.
    #[serde(default, deserialize_with = "lenient_string")]
    pub id: String,
    /// Display name.
    #[serde(default, deserialize_with = "lenient_string")]
    pub display_name: String,
    /// Profile photo URL.
    #[serde(default, deserialize_with = "lenient_string")]
    pub pfp_url: String,
    /// Date of birth (a Silicon's is the day its account was created).
    #[serde(default, with = "ymd::option", skip_serializing_if = "Option::is_none")]
    pub dob: Option<Date>,
    /// IANA timezone, e.g. `Asia/Kolkata`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub timezone: String,
    /// `active`, `unclaimed`, `pending_custodian` or `deleted`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// When the account was created.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// When the profile last changed.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub updated_at: Option<OffsetDateTime>,
    /// Bumped on every profile or id change.
    #[serde(default, deserialize_with = "lenient_i64")]
    pub version: i64,
    /// Carbon only: verified email addresses.
    #[serde(
        default,
        deserialize_with = "lenient_vec",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub emails: Vec<EmailAddress>,
    /// Carbon only: verified phone numbers.
    #[serde(
        default,
        deserialize_with = "lenient_vec",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub phones: Vec<PhoneNumber>,
    /// Carbon only: linked Google / Apple identities.
    #[serde(
        default,
        deserialize_with = "lenient_vec",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub identities: Vec<Identity>,
    /// Carbon only: how many Silicons this Carbon is custodian of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custodian_of: Option<u64>,
    /// Silicon only: the custodian Carbon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custodian: Option<AccountRef>,
    /// Silicon only: the Silicon's own webhook endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
    /// Silicon only: when the STK was last set.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub stk_rotated_at: Option<OffsetDateTime>,
}

impl Me {
    /// The primary email, if any.
    pub fn primary_email(&self) -> Option<&str> {
        self.emails
            .iter()
            .find(|e| e.is_primary)
            .map(|e| e.email.as_str())
    }

    /// The primary phone number, if any.
    pub fn primary_phone(&self) -> Option<&str> {
        self.phones
            .iter()
            .find(|p| p.is_primary)
            .map(|p| p.phone.as_str())
    }
}

/// A Silicon as its custodian sees it (same shape as [`Me`]).
pub type SiliconView = Me;

/// An email address on a Carbon account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EmailAddress {
    /// Normalized (lowercase) address.
    pub email: String,
    /// Whether this is the primary email.
    #[serde(default, deserialize_with = "lenient_bool")]
    pub is_primary: bool,
    /// When it was verified.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub verified_at: Option<OffsetDateTime>,
    /// `code`, `google` or `apple`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_via: Option<String>,
    /// When it was added to the account.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
}

/// A phone number on a Carbon account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PhoneNumber {
    /// E.164 number, e.g. `+919876543210`.
    pub phone: String,
    /// Whether this is the primary phone.
    #[serde(default, deserialize_with = "lenient_bool")]
    pub is_primary: bool,
    /// When it was verified.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub verified_at: Option<OffsetDateTime>,
    /// How it was verified (`code`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_via: Option<String>,
    /// When it was added to the account.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
}

/// A Google or Apple identity linked to a Carbon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Identity {
    /// `google` or `apple`.
    pub provider: String,
    /// The provider's subject id (needed to unlink).
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub subject: Option<String>,
    /// The email the provider reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// When it was linked.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// When it was last used to sign in.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_used_at: Option<OffsetDateTime>,
}

/// Profile fields to change (`PATCH /v1/me`). `None` leaves a field unchanged.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ProfileUpdate {
    /// New display name (1..100 characters).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// New IANA timezone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// New date of birth (Carbons only; a Silicon's is fixed).
    #[serde(skip_serializing_if = "Option::is_none", with = "ymd::option")]
    pub dob: Option<Date>,
    /// New profile photo URL (https).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pfp_url: Option<String>,
}

impl ProfileUpdate {
    /// True when nothing would change.
    pub fn is_empty(&self) -> bool {
        self.display_name.is_none()
            && self.timezone.is_none()
            && self.dob.is_none()
            && self.pfp_url.is_none()
    }
}

/// Result of uploading a profile photo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PhotoUploaded {
    /// The new profile photo URL.
    pub pfp_url: String,
    /// The stored image (when the service reports it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<PhotoInfo>,
}

/// An uploaded photo as stored: `{id, content_type, bytes, width, height}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PhotoInfo {
    /// The photo id (its URL is `{PUBLIC_URL}/v1/photos/{id}`).
    #[serde(default, deserialize_with = "lenient_string")]
    pub id: String,
    /// `image/png`, `image/jpeg`, `image/webp` or `image/gif`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub content_type: String,
    /// Size in bytes.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub bytes: u64,
    /// Width in pixels.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub width: u64,
    /// Height in pixels.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub height: u64,
}

/// A verification code was sent; finish with the matching `verify_*` call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ContactChallenge {
    /// Pass this with the code to verify.
    pub challenge_id: String,
    /// The code expires at this time (10 minutes after sending).
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<OffsetDateTime>,
    /// Where the code went, masked (when the service reports it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
}

/// A way to reach a Carbon: an email address or a phone number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contact {
    /// An email address.
    Email(String),
    /// A phone number, with an optional ISO country (e.g. `IN`) for local formats.
    Phone {
        /// The number, E.164 or local.
        phone: String,
        /// Default country for local formats.
        country: Option<String>,
    },
}

impl Contact {
    /// The address or number as given.
    pub fn value(&self) -> &str {
        match self {
            Self::Email(email) => email,
            Self::Phone { phone, .. } => phone,
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        match self {
            Self::Email(email) => serde_json::json!({ "email": email }),
            Self::Phone {
                phone,
                country: Some(country),
            } => serde_json::json!({ "phone": phone, "country": country }),
            Self::Phone {
                phone,
                country: None,
            } => serde_json::json!({ "phone": phone }),
        }
    }
}

/// Short summary of an app (in lists and references).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AppSummary {
    /// The app id, e.g. `briefcase`.
    pub app_id: String,
    /// The app's name.
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: String,
    /// Logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    /// Logo URL for dark backgrounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_dark_url: Option<String>,
    /// Homepage URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage_url: Option<String>,
}

/// An app the signed-in account has signed into (`GET /v1/me/apps`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MyApp {
    /// The app.
    pub app: AppSummary,
    /// `{app_id}:{uuid}`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub membership_id: String,
    /// `active`, `access_removed` or `imported`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// How the membership started: `signin`, `slt` or `import`.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub source: Option<String>,
    /// When the account removed the app's access (status `access_removed`).
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub access_removed_at: Option<OffsetDateTime>,
    /// The scopes this account shared with the app.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub granted_scopes: Vec<String>,
    /// First sign-in.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub first_signed_in_at: Option<OffsetDateTime>,
    /// Latest sign-in.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_signed_in_at: Option<OffsetDateTime>,
    /// Live token families the app holds for this account.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub active_sessions: u64,
}

/// A browser session or a first-party (CLI) token family (`GET /v1/me/sessions`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SessionInfo {
    /// Pass to `revoke_session`.
    #[serde(deserialize_with = "lenient_string")]
    pub id: String,
    /// `browser` or `cli`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub kind: String,
    /// How it was created, e.g. `browser`, `device`, `cli_code` or `silicon_login`.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub origin: Option<String>,
    /// Client label, e.g. `accounts CLI on build-box (linux)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// IP address seen at creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    /// User agent seen at creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Created at.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// Last used at.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_seen_at: Option<OffsetDateTime>,
    /// When it ends unless used or revoked.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<OffsetDateTime>,
    /// True for the session making this request.
    #[serde(default, deserialize_with = "lenient_bool")]
    pub current: bool,
}

/// One entry of the account's history (`GET /v1/me/history`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HistoryItem {
    /// Entry id.
    #[serde(default, deserialize_with = "lenient_string")]
    pub id: String,
    /// `signin`, `id_change`, `custodian`, `proof`, `app_access` or `security`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub kind: String,
    /// When it happened.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub at: Option<OffsetDateTime>,
    /// One-line summary.
    #[serde(default, deserialize_with = "lenient_string")]
    pub title: String,
    /// More detail.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub detail: Option<String>,
    /// The app involved, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<AppSummary>,
    /// Extra structured data.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub meta: Value,
}

/// Filters for `GET /v1/me/history`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryQuery {
    /// `signin`, `id_change`, `custodian`, `proof`, `app_access` or `security`.
    pub kind: Option<String>,
    /// Items per page (max 200).
    pub limit: Option<u32>,
    /// Cursor from the previous page.
    pub cursor: Option<String>,
}

/// A pending CLI sign-in as the approving Carbon sees it (`GET /v1/device/{user_code}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DeviceRequest {
    /// The code shown by the CLI, e.g. `WDJB-MJHT`.
    pub user_code: String,
    /// The label the CLI sent, e.g. `accounts CLI on build-box (linux)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_label: Option<String>,
    /// When the sign-in was started.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// When the code expires.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<OffsetDateTime>,
    /// `pending`, `approved`, `denied`, `consumed` or `expired`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
}
