//! Apps: public config, sign-in setup, user base, imports, webhooks.

use std::collections::BTreeMap;

use bytes::Bytes;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::secret::Secret;
use crate::serde_util::{
    lenient_bool, lenient_i64, lenient_opt_string, lenient_string, lenient_u64, lenient_vec,
};
use crate::types::account::{AccountKind, AccountSummary};

/// `GET /v1/apps/{app_id}/public`: what the hosted pages and the SDK need.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AppPublic {
    /// The app id.
    pub app_id: String,
    /// The app's name.
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: String,
    /// Logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    /// Logo for dark backgrounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_dark_url: Option<String>,
    /// Homepage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage_url: Option<String>,
    /// Enabled sign-in methods in display order (`google`, `apple`, `email`, `phone`).
    #[serde(default, deserialize_with = "lenient_vec")]
    pub methods: Vec<String>,
    /// Page styling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branding: Option<Branding>,
    /// Custom page copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<CopyText>,
    /// Origins allowed to embed the sign-in iframe (its `frame-ancestors`) and use the SDK.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub allowed_origins: Vec<String>,
}

/// Branding variables for the hosted sign-in pages. `None` means the Silicon Accounts
/// default. "Powered by Silicon Accounts" is always shown and is not configurable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Branding {
    /// `auto`, `light` or `dark`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    /// Logo for dark mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_dark_url: Option<String>,
    /// Logo height in px (16..96).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_height: Option<u32>,
    /// Show the app name next to the logo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_app_name: Option<bool>,
    /// Body font (allowlisted families).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// Heading font (allowlisted families).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading_font_family: Option<String>,
    /// `squircle`, `rounded` or `sharp`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_style: Option<String>,
    /// Corner radius in px (0..40).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<u32>,
    /// `solid`, `soft` or `outline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button_style: Option<String>,
    /// `card`, `split` or `minimal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    /// `plain`, `dots`, `grain`, `gradient` or `image`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_style: Option<String>,
    /// Background image (https) when `background_style` is `image`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_image_url: Option<String>,
    /// `comfortable` or `compact`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<String>,
    /// Light theme colours (`#RRGGBB`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<ThemeColors>,
    /// Dark theme colours (`#RRGGBB`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark: Option<ThemeColors>,
    /// Variables added after this client was released.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Colours for one theme. The service rejects primary/primary_foreground (button text) or
/// foreground/background (page text) pairs with contrast below 4.5:1 (WCAG AA for text). The
/// defaults fill buttons with `#1F5FB8` under `#FFFDF9` in both themes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ThemeColors {
    /// Buttons and links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<String>,
    /// Text on primary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_foreground: Option<String>,
    /// Page background.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// Card background.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    /// Text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground: Option<String>,
    /// Secondary text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<String>,
    /// Hairlines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    /// Errors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub danger: Option<String>,
    /// Colours added after this client was released.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Custom copy on the hosted pages.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CopyText {
    /// Page title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Subtitle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// Terms of service link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms_url: Option<String>,
    /// Privacy policy link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_url: Option<String>,
    /// Support address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_email: Option<String>,
    /// Title of the Opening page shown before Google or Apple (≤ 80 chars; may contain
    /// `{provider}` and `{app}`), e.g. "Opening {provider} to sign you in to {app}…".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening_title: Option<String>,
    /// Title of the sign-up version of the hosted pages (`intent=signup`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signup_title: Option<String>,
    /// Subtitle of the sign-up version of the hosted pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signup_subtitle: Option<String>,
    /// Fields added after this client was released.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// An app the signed-in Carbon owns (`GET /v1/me/owned-apps`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OwnedApp {
    /// The app id.
    pub app_id: String,
    /// Name.
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: String,
    /// Logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    /// `active` or `disabled`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// `first_party`, `fake` or `silicon_apps`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub source: String,
    /// Number of accounts in the app's user base.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub users: u64,
    /// Created at.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
}

/// `GET /v1/apps/{app_id}`: the app and its sign-in setup (secrets masked).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AppDetails {
    /// The app id.
    pub app_id: String,
    /// Name.
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: String,
    /// Description.
    #[serde(default, deserialize_with = "lenient_string")]
    pub description: String,
    /// Logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    /// Dark logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_dark_url: Option<String>,
    /// Homepage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage_url: Option<String>,
    /// The owning Carbon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<AccountSummary>,
    /// `active` or `disabled`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// `first_party`, `fake` or `silicon_apps`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub source: String,
    /// Created at.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// Last change to the app's own record (name, logo, homepage, owner, status). Sign-in
    /// setup changes don't move it: they show as `config_version` and in the config history.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub updated_at: Option<OffsetDateTime>,
    /// The sign-in setup.
    #[serde(default)]
    pub signin_config: SigninConfig,
    /// Version of the sign-in setup; pass it as `expected_version` to avoid lost updates.
    #[serde(default, deserialize_with = "lenient_i64")]
    pub config_version: i64,
    /// The app's webhook endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook: Option<AppWebhookInfo>,
    /// User base statistics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<AppStats>,
}

/// The sign-in setup document. Secrets are never returned: `client_secret_set` /
/// `private_key_set` say whether one is stored.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SigninConfig {
    /// Which methods are on: `{"email": true, "phone": false, "google": false, "apple": false}`.
    #[serde(default)]
    pub methods: BTreeMap<String, bool>,
    /// Display order of the methods.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub method_order: Vec<String>,
    /// Google settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google: Option<GoogleConfig>,
    /// Apple settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple: Option<AppleConfig>,
    /// Exact redirect URIs allowed after sign-in.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub redirect_uris: Vec<String>,
    /// Origins allowed to embed the iframe and use the SDK.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub allowed_origins: Vec<String>,
    /// Details every account must share (`email`, `phone`, `dob`, `timezone`).
    #[serde(default, deserialize_with = "lenient_vec")]
    pub required_fields: Vec<String>,
    /// Details the account may choose to share.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub optional_fields: Vec<String>,
    /// When non-empty, only these email domains may sign in.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub allowed_email_domains: Vec<String>,
    /// Whether new Carbons may sign up through this app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_signup: Option<bool>,
    /// Offer "Continue as …" from the browser session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remember_browser: Option<bool>,
    /// Page styling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branding: Option<Branding>,
    /// Page copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<CopyText>,
    /// The app's flow: which pages a Carbon goes through and which details each page asks.
    /// `null` (the default) is one page with every requested detail, which is also the
    /// what's-shared screen. Always serialized (as `null` when unset), like the service
    /// returns it.
    #[serde(default)]
    pub flow: Option<SigninFlow>,
    /// Settings added after this client was released.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// An app's sign-in flow (`signin_config.flow`): 1 to 8 pages of details. Every requested
/// detail (`required_fields` ∪ `optional_fields`) appears on exactly one page.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SigninFlow {
    /// The detail pages, in order.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub steps: Vec<FlowStep>,
    /// Show a final review page of everything that will be shared.
    #[serde(default, deserialize_with = "lenient_bool")]
    pub review: bool,
    /// Settings added after this client was released.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl SigninFlow {
    /// A flow with these pages.
    pub fn new(steps: Vec<FlowStep>, review: bool) -> Self {
        Self {
            steps,
            review,
            extra: BTreeMap::new(),
        }
    }
}

/// One page of a [`SigninFlow`]. `null` texts and layout mean the defaults (the layout then
/// follows `branding.layout`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FlowStep {
    /// `[a-z0-9-]{1,40}`, unique within the flow.
    #[serde(default, deserialize_with = "lenient_string")]
    pub id: String,
    /// The details asked on this page: `email`, `phone`, `dob`, `timezone`.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub fields: Vec<String>,
    /// Page title (≤ 80 chars).
    #[serde(default)]
    pub title: Option<String>,
    /// Page subtitle (≤ 200 chars).
    #[serde(default)]
    pub subtitle: Option<String>,
    /// The continue button's label (≤ 30 chars).
    #[serde(default)]
    pub continue_label: Option<String>,
    /// `card`, `split` or `minimal`.
    #[serde(default)]
    pub layout: Option<String>,
    /// Settings added after this client was released.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl FlowStep {
    /// A page `id` asking these details, with default texts and layout.
    pub fn new<I, S>(id: impl Into<String>, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            id: id.into(),
            fields: fields.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }
}

impl SigninConfig {
    /// Enabled methods in display order.
    pub fn enabled_methods(&self) -> Vec<&str> {
        let mut ordered: Vec<&str> = self
            .method_order
            .iter()
            .map(String::as_str)
            .filter(|m| self.methods.get(*m).copied().unwrap_or(false))
            .collect();
        for (method, on) in &self.methods {
            if *on && !ordered.contains(&method.as_str()) {
                ordered.push(method);
            }
        }
        ordered
    }
}

/// Google sign-in settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GoogleConfig {
    /// `managed` (one click, our Google setup) or `byo` (your own client).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Your Google client id (`byo`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// Google `prompt` parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Google Workspace domain restriction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosted_domain: Option<String>,
    /// Whether a client secret is stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_set: Option<bool>,
    /// Settings added later.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Apple sign-in settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AppleConfig {
    /// `managed` or `byo`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Your Services ID (`byo`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub services_id: Option<String>,
    /// Your Apple team id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    /// Your key id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
    /// Whether a private key is stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key_set: Option<bool>,
    /// Settings added later.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// The app's webhook endpoint (secret masked).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AppWebhookInfo {
    /// The endpoint, if set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Whether a signing secret exists.
    #[serde(default, deserialize_with = "lenient_bool")]
    pub secret_set: bool,
}

/// User base statistics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AppStats {
    /// Accounts in the user base.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub users: u64,
    /// Accounts that signed in during the last 30 days.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub active_last_30d: u64,
    /// Imported accounts that never finished setting up.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub imported_unclaimed: u64,
}

/// One change of an app's sign-in setup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ConfigHistoryEntry {
    /// The version this change produced.
    #[serde(default, deserialize_with = "lenient_i64")]
    pub version: i64,
    /// `app`, an account uuid, `system` or `silicon_apps`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub actor: String,
    /// The account behind `actor` when it is an account uuid (an owner's change): show its
    /// id instead of the uuid. `null` for `app`, `system` and `silicon_apps`.
    #[serde(default)]
    pub actor_account: Option<AccountSummary>,
    /// List of `{path, before, after}` (secrets redacted).
    #[serde(default)]
    pub changes: Value,
    /// When.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub at: Option<OffsetDateTime>,
}

/// An account in an app's user base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AppUser {
    /// `{app_id}:{uuid}`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub membership_id: String,
    /// Account uuid.
    pub uuid: String,
    /// Carbon or Silicon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<AccountKind>,
    /// Current public id; `None` (`null`) once the account was deleted.
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub id: Option<String>,
    /// Display name.
    #[serde(default, deserialize_with = "lenient_string")]
    pub display_name: String,
    /// Photo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pfp_url: Option<String>,
    /// Email (within granted scopes, or as imported).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Phone (within granted scopes, or as imported).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    /// Date of birth (within granted scopes, or as imported).
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub dob: Option<String>,
    /// Timezone (within granted scopes, or as imported).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// `active`, `access_removed`, `imported`, or `deleted` when the account was deleted.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// The account's own status: `active`, `unclaimed` (imported, not finished yet),
    /// `pending_custodian` or `deleted`.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub account_status: Option<String>,
    /// `signin`, `slt` or `import`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub source: String,
    /// Your own id for this user (from an import); `null` when none was imported.
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub external_id: Option<String>,
    /// Scopes the account granted.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub granted_scopes: Vec<String>,
    /// First sign-in (`null` for an imported account that never signed in).
    #[serde(default, with = "crate::serde_util::rfc3339_ms::option")]
    pub first_signed_in_at: Option<OffsetDateTime>,
    /// Latest sign-in.
    #[serde(default, with = "crate::serde_util::rfc3339_ms::option")]
    pub last_signed_in_at: Option<OffsetDateTime>,
    /// When the membership was created.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// Last 20 sign-ins (only on `GET /v1/apps/{app_id}/users/{uuid}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<SigninEvent>>,
}

/// One sign-in from an account's history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SigninEvent {
    /// When.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub at: Option<OffsetDateTime>,
    /// `email`, `phone`, `google`, `apple`, `silicon_stk`, `slt`, `device` or `session`.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub method: Option<String>,
    /// `success`, `failed` or `new_account`.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub outcome: Option<String>,
    /// Other fields (ip, user agent…).
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Filters for an app's user base.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsersQuery {
    /// Matches id, display name, email, phone and external_id.
    pub q: Option<String>,
    /// `active`, `access_removed`, `imported` or `deleted`.
    pub status: Option<String>,
    /// `carbon` or `silicon`.
    pub kind: Option<String>,
    /// `signin`, `slt` or `import`.
    pub source: Option<String>,
    /// Items per page (max 200).
    pub limit: Option<u32>,
    /// Cursor from the previous page.
    pub cursor: Option<String>,
}

/// One row to import. These are the only columns an import accepts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportRow {
    /// Your own id for the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    /// Primary email.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// More emails.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<String>,
    /// Primary phone (E.164 or local with `default_country`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    /// More phones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phones: Vec<String>,
    /// Display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Desired handle (`john` or `c:john`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Date of birth (`YYYY-MM-DD`, `DD/MM/YYYY`, `MM/DD/YYYY` when unambiguous, `YYYY/MM/DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dob: Option<String>,
    /// IANA timezone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// Photo URL (https).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pfp_url: Option<String>,
    /// Informational only; imported emails are never trusted as verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
}

/// Import options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportOptions {
    /// ISO country for local phone formats, e.g. `US`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_country: Option<String>,
    /// Import anyway when there are unknown columns (each affected row gets a warning).
    #[serde(default)]
    pub ignore_unknown_columns: bool,
    /// Validate and report without writing anything.
    #[serde(default)]
    pub dry_run: bool,
    /// Also refresh the imported profile of users that are already members.
    #[serde(default)]
    pub update_existing: bool,
}

/// What to import.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ImportInput {
    /// Typed rows (sent as JSON).
    Rows(Vec<ImportRow>),
    /// Raw JSON objects, passed through unchanged so the service reports unknown
    /// columns itself.
    Json(Vec<Value>),
    /// A CSV file with a header row (sent as `text/csv`).
    Csv(Bytes),
}

/// An import job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImportJob {
    /// Job id.
    #[serde(deserialize_with = "lenient_string")]
    pub id: String,
    /// `queued`, `running`, `completed` or `failed`.
    #[serde(deserialize_with = "lenient_string")]
    pub status: String,
    /// `json` or `csv`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub format: String,
    /// Rows in the file.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub total_rows: u64,
    /// Rows processed so far.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub processed_rows: u64,
    /// Outcome counts.
    #[serde(default)]
    pub counts: ImportCounts,
    /// Created at.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// Started at (`null` while queued).
    #[serde(default, with = "crate::serde_util::rfc3339_ms::option")]
    pub started_at: Option<OffsetDateTime>,
    /// Finished at (`null` until it is completed or failed).
    #[serde(default, with = "crate::serde_util::rfc3339_ms::option")]
    pub finished_at: Option<OffsetDateTime>,
    /// Why the whole job failed (`null` unless it failed).
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub error: Option<String>,
    /// The app it imports into.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub app_id: Option<String>,
    /// True for a dry run (nothing was written).
    #[serde(default, deserialize_with = "lenient_bool")]
    pub dry_run: bool,
    /// The options the job ran with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<ImportOptions>,
    /// Who started it: `app` or the owner's uuid.
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub created_by: Option<String>,
}

impl ImportJob {
    /// True once the job is `completed` or `failed`.
    pub fn is_finished(&self) -> bool {
        matches!(self.status.as_str(), "completed" | "failed")
    }

    /// Progress 0.0..=1.0.
    pub fn progress(&self) -> f64 {
        if self.is_finished() {
            return 1.0;
        }
        if self.total_rows == 0 {
            return 0.0;
        }
        // Row counts are far below 2^52, so the conversion is exact.
        #[allow(clippy::cast_precision_loss)]
        let ratio = self.processed_rows as f64 / self.total_rows as f64;
        ratio.clamp(0.0, 1.0)
    }
}

/// Per-outcome row counts of an import.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImportCounts {
    /// New (unclaimed) Carbon accounts.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub created: u64,
    /// Matched to an existing account.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub matched: u64,
    /// Matched and imported profile refreshed (`update_existing`).
    #[serde(default, deserialize_with = "lenient_u64")]
    pub updated: u64,
    /// Skipped (e.g. duplicate in file).
    #[serde(default, deserialize_with = "lenient_u64")]
    pub skipped: u64,
    /// Rows with errors.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub error: u64,
    /// Total warnings.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub warnings: u64,
}

/// The outcome of one imported row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImportRowResult {
    /// 1-based data row (the CSV header is not counted).
    #[serde(default, deserialize_with = "lenient_u64")]
    pub row_number: u64,
    /// `created`, `matched`, `updated`, `skipped`, `error` or `pending`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub outcome: String,
    /// The account the row maps to (`null` for a row that maps to none, and for matched rows
    /// of a dry run).
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub account_uuid: Option<String>,
    /// The account's id (`null` when there is none to show).
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub id: Option<String>,
    /// Errors, warnings and notes.
    #[serde(default, deserialize_with = "lenient_vec")]
    pub messages: Vec<RowMessage>,
    /// The row as received.
    #[serde(default)]
    pub input: Value,
}

/// A message about one imported row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RowMessage {
    /// `error`, `warning` or `info`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub level: String,
    /// Stable code, e.g. `id_conflict`, `missing_identifier`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub code: String,
    /// What happened.
    #[serde(default, deserialize_with = "lenient_string")]
    pub message: String,
    /// The column concerned.
    #[serde(
        default,
        deserialize_with = "lenient_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub field: Option<String>,
}

/// Filters for import rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportRowsQuery {
    /// Only rows with this outcome: `created`, `matched`, `updated`, `skipped`, `error` or
    /// `pending`.
    pub outcome: Option<String>,
    /// Only rows with a message of this level: `error`, `warning` or `info`.
    pub level: Option<String>,
    /// Only rows with a message of this code, e.g. `id_conflict` or `missing_identifier`.
    pub code: Option<String>,
    /// Items per page (max 200).
    pub limit: Option<u32>,
    /// Cursor from the previous page.
    pub cursor: Option<String>,
}

/// The app's webhook endpoint with its new signing secret (shown once).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AppWebhook {
    /// The endpoint.
    #[serde(default, deserialize_with = "lenient_string")]
    pub url: String,
    /// `whsec_…`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<Secret>,
}

/// A rotated webhook signing secret (shown once).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebhookSecret {
    /// `whsec_…`.
    pub secret: Secret,
}

/// A webhook delivery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebhookDelivery {
    /// Delivery id (replay with it).
    #[serde(deserialize_with = "lenient_string")]
    pub id: String,
    /// Event id (dedupe on it).
    #[serde(default, deserialize_with = "lenient_string")]
    pub event_id: String,
    /// Event type, e.g. `account.id_changed`.
    #[serde(default, rename = "type", deserialize_with = "lenient_string")]
    pub event_type: String,
    /// `pending`, `delivered` or `failed`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    /// Attempts made so far.
    #[serde(default, deserialize_with = "count_or_len")]
    pub attempts: u64,
    /// HTTP status of the last attempt.
    #[serde(default)]
    pub last_status: Option<u16>,
    /// Error of the last attempt.
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub last_error: Option<String>,
    /// Next retry (only a pending delivery has one).
    #[serde(default, with = "crate::serde_util::rfc3339_ms::option")]
    pub next_attempt_at: Option<OffsetDateTime>,
    /// When it was delivered.
    #[serde(default, with = "crate::serde_util::rfc3339_ms::option")]
    pub delivered_at: Option<OffsetDateTime>,
    /// Created at.
    #[serde(
        default,
        with = "crate::serde_util::rfc3339_ms::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<OffsetDateTime>,
    /// How many times it was replayed by hand.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub manual_replays: u64,
    /// The endpoint it goes to (the current webhook URL).
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub url: Option<String>,
    /// The account the event is about, if any.
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub account_uuid: Option<String>,
    /// When the last attempt was made.
    #[serde(default, with = "crate::serde_util::rfc3339_ms::option")]
    pub last_attempt_at: Option<OffsetDateTime>,
    /// Detail view: true when the payload is withheld because the account deleted itself or
    /// removed the app's access.
    #[serde(
        default,
        deserialize_with = "lenient_bool",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub payload_redacted: bool,
}

fn count_or_len<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => Ok(0),
        Some(Value::Number(n)) => n
            .as_u64()
            .ok_or_else(|| de::Error::custom("attempts must be a non-negative integer")),
        Some(Value::Array(items)) => Ok(items.len() as u64),
        Some(_) => Err(de::Error::custom("attempts must be a number or a list")),
    }
}

/// A delivery with its attempts and the exact payload that was signed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct DeliveryDetail {
    /// The delivery.
    pub delivery: WebhookDelivery,
    /// Each attempt (status code, error, duration).
    pub attempt_log: Vec<Value>,
    /// The event body.
    pub payload: Value,
}

impl<'de> Deserialize<'de> for DeliveryDetail {
    // Accepts `{...delivery, "attempts": [...], "payload": {...}}` and
    // `{"delivery": {...}, "attempts": [...], "payload": {...}}`.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut map = serde_json::Map::deserialize(deserializer)?;
        let payload = map.remove("payload").unwrap_or(Value::Null);
        let attempt_log = match map.get("attempts") {
            Some(Value::Array(items)) => items.clone(),
            _ => match map.get("attempt_log") {
                Some(Value::Array(items)) => items.clone(),
                _ => Vec::new(),
            },
        };
        let delivery_value = match map.remove("delivery") {
            Some(inner @ Value::Object(_)) => inner,
            _ => Value::Object(map),
        };
        let delivery = serde_json::from_value(delivery_value).map_err(de::Error::custom)?;
        Ok(Self {
            delivery,
            attempt_log,
            payload,
        })
    }
}

/// Filters for webhook deliveries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeliveriesQuery {
    /// `pending`, `delivered` or `failed`.
    pub status: Option<String>,
    /// Items per page (max 200).
    pub limit: Option<u32>,
    /// Cursor from the previous page.
    pub cursor: Option<String>,
}

/// Which deliveries to replay (at most 100). Replays go to the current URL, signed with
/// the current secret, keeping the original event id. For an app's webhook, accounts that no
/// longer have a membership with the app are skipped; for a Silicon's webhook, test pings are
/// never replayed (send a new one).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplayRequest {
    /// These delivery ids.
    Deliveries(Vec<String>),
    /// Every failed delivery (optionally only those created since a time).
    Failed {
        /// Only deliveries created at or after this time.
        since: Option<OffsetDateTime>,
    },
}

impl ReplayRequest {
    /// Refuses an id list the service would refuse (1 to 100 ids), before sending it.
    pub(crate) fn check(&self) -> crate::error::Result<()> {
        if let Self::Deliveries(ids) = self
            && (ids.is_empty() || ids.len() > 100)
        {
            return Err(crate::error::Error::invalid_input(
                format!(
                    "A replay takes 1 to 100 delivery ids; {} were given.",
                    ids.len()
                ),
                "Pass the ids from `deliveries`, or replay every failed delivery instead.",
            ));
        }
        Ok(())
    }

    pub(crate) fn to_json(&self) -> Value {
        match self {
            Self::Deliveries(ids) => serde_json::json!({ "delivery_ids": ids }),
            Self::Failed { since: None } => serde_json::json!({ "status": "failed" }),
            Self::Failed { since: Some(since) } => {
                let since = since
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default();
                serde_json::json!({ "status": "failed", "since": since })
            }
        }
    }
}

/// Result of a replay request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReplayResult {
    /// What was re-queued (a count or a list of delivery ids, as the service reports it).
    #[serde(default)]
    pub replayed: Value,
    /// What was skipped and why.
    #[serde(default)]
    pub skipped: Value,
    /// Other fields.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ReplayResult {
    /// Number of re-queued deliveries.
    pub fn replayed_count(&self) -> u64 {
        value_count(&self.replayed)
    }

    /// Number of skipped deliveries.
    pub fn skipped_count(&self) -> u64 {
        value_count(&self.skipped)
    }
}

fn value_count(value: &Value) -> u64 {
    match value {
        Value::Number(n) => n.as_u64().unwrap_or(0),
        Value::Array(items) => items.len() as u64,
        _ => 0,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn config_history_keeps_the_owner_behind_a_change() {
        let entry: ConfigHistoryEntry = serde_json::from_value(json!({
            "version": 4, "actor": "a8K",
            "actor_account": {"uuid": "a8K", "kind": "carbon", "id": "c:saket", "display_name": "Saket", "pfp_url": "", "status": "active"},
            "changes": [{"path": "redirect_uris", "before": [], "after": ["https://x"]}],
            "at": "2026-10-07T05:17:55.590Z"
        }))
        .unwrap();
        assert_eq!(entry.actor_account.as_ref().unwrap().id, "c:saket");
        let out = serde_json::to_value(&entry).unwrap();
        assert_eq!(out["actor_account"]["id"], "c:saket");
        assert_eq!(out["at"], "2026-10-07T05:17:55.590Z");
        // A change by the app itself has no account: null, as the API sends it.
        let entry: ConfigHistoryEntry = serde_json::from_value(
            json!({"version": 2, "actor": "app", "actor_account": null, "changes": [], "at": "2026-10-07T05:17:55.000Z"}),
        )
        .unwrap();
        assert!(entry.actor_account.is_none());
        let out = serde_json::to_value(&entry).unwrap();
        assert_eq!(out["actor_account"], Value::Null);
        assert_eq!(out["at"], "2026-10-07T05:17:55.000Z");
    }

    #[test]
    fn nulls_the_api_sends_stay_null() {
        // A deleted account in the user base: no id and no external id any more.
        let api = json!({
            "membership_id": "briefcase:a8K", "uuid": "a8K", "kind": "carbon", "id": null,
            "display_name": "Deleted account", "pfp_url": "https://iris.example/pfp", "status": "deleted",
            "source": "signin", "external_id": null, "granted_scopes": [],
            "first_signed_in_at": "2026-10-07T05:17:55.590Z", "last_signed_in_at": null,
            "created_at": "2026-10-07T05:17:55.000Z", "account_status": "deleted"
        });
        let user: AppUser = serde_json::from_value(api.clone()).unwrap();
        assert_eq!(user.id, None);
        assert_eq!(serde_json::to_value(&user).unwrap(), api);

        // A released Silicon of a custodian request has no si:id.
        let status: crate::types::CustodianRequestStatus = serde_json::from_value(json!({
            "id": "req-1", "status": "declined", "kind": "initial", "custodian": "c:saket",
            "silicon": {"uuid": "b9Z", "id": null, "status": "deleted"}
        }))
        .unwrap();
        assert_eq!(status.silicon.id, None);
        assert_eq!(
            serde_json::to_value(&status).unwrap()["silicon"]["id"],
            Value::Null
        );

        // A delivery that was never attempted.
        let api = json!({
            "id": "d1", "event_id": "e1", "type": "ping", "account_uuid": null, "url": "https://x",
            "status": "pending", "attempts": 0, "last_status": null, "last_error": null,
            "next_attempt_at": "2026-10-07T05:17:55.590Z", "last_attempt_at": null, "delivered_at": null,
            "created_at": "2026-10-07T05:17:55.000Z", "manual_replays": 0
        });
        let delivery: WebhookDelivery = serde_json::from_value(api.clone()).unwrap();
        assert_eq!(serde_json::to_value(&delivery).unwrap(), api);

        // A running import job.
        let job: ImportJob = serde_json::from_value(json!({
            "id": "job-1", "status": "running", "format": "csv", "total_rows": 3, "processed_rows": 1,
            "counts": {}, "created_at": "2026-10-07T05:17:55.000Z", "started_at": "2026-10-07T05:17:56.100Z",
            "finished_at": null, "error": null, "created_by": "app", "app_id": "briefcase", "dry_run": false
        }))
        .unwrap();
        let out = serde_json::to_value(&job).unwrap();
        assert_eq!(out["finished_at"], Value::Null);
        assert_eq!(out["error"], Value::Null);
        assert_eq!(out["started_at"], "2026-10-07T05:17:56.100Z");
    }

    #[test]
    fn sign_in_setups_keep_their_flow_and_page_copy() {
        let api = json!({
            "methods": {"email": true},
            "required_fields": ["email", "dob"],
            "optional_fields": ["timezone"],
            "copy": {"title": null, "opening_title": "Opening {provider} for {app}…",
                     "signup_title": "Create your Ledgerly account", "signup_subtitle": null},
            "flow": {
                "steps": [
                    {"id": "contact", "fields": ["email"], "title": "How can we reach you?",
                     "subtitle": null, "continue_label": null, "layout": null},
                    {"id": "about-you", "fields": ["dob", "timezone"], "title": null,
                     "subtitle": null, "continue_label": "Finish", "layout": "split"}
                ],
                "review": true
            }
        });
        let config: SigninConfig = serde_json::from_value(api).unwrap();
        let flow = config.flow.as_ref().unwrap();
        assert!(flow.review);
        assert_eq!(flow.steps.len(), 2);
        assert_eq!(flow.steps[1].fields, vec!["dob", "timezone"]);
        assert_eq!(flow.steps[1].continue_label.as_deref(), Some("Finish"));
        assert_eq!(flow.steps[1].layout.as_deref(), Some("split"));
        let copy = config.copy.as_ref().unwrap();
        assert_eq!(
            copy.opening_title.as_deref(),
            Some("Opening {provider} for {app}…")
        );
        assert_eq!(
            copy.signup_title.as_deref(),
            Some("Create your Ledgerly account")
        );
        // What `--json` prints keeps the flow, null texts included.
        let out = serde_json::to_value(&config).unwrap();
        assert_eq!(out["flow"]["steps"][0]["subtitle"], Value::Null);
        assert_eq!(out["flow"]["steps"][0]["title"], "How can we reach you?");
        assert_eq!(out["flow"]["review"], true);

        // The default flow is null, and stays null on the way out.
        let config: SigninConfig = serde_json::from_value(json!({"flow": null})).unwrap();
        assert!(config.flow.is_none());
        assert_eq!(serde_json::to_value(&config).unwrap()["flow"], Value::Null);

        let built = SigninFlow::new(vec![FlowStep::new("contact", ["email", "phone"])], false);
        assert_eq!(
            serde_json::to_value(&built).unwrap(),
            json!({"steps": [{"id": "contact", "fields": ["email", "phone"], "title": null,
                              "subtitle": null, "continue_label": null, "layout": null}],
                   "review": false})
        );
    }
}
