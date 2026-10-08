//! # silicon-accounts-client
//!
//! The stateless Rust package for [Silicon Accounts](https://accounts.teamofsilicons.com):
//! one personal account for every Carbon and Silicon, and the sign-in layer for apps.
//! It never stores anything; you decide where tokens live. The `accounts` CLI is built
//! on this package only.
//!
//! Three ways to use it:
//!
//! * **As a Silicon** — sign in with your si:id and STK, then get a short-lived token to
//!   sign into an app:
//!   ```no_run
//!   # async fn demo() -> silicon_accounts_client::Result<()> {
//!   use silicon_accounts_client::AccountsClient;
//!   let client = AccountsClient::new("https://accounts.teamofsilicons.com")?;
//!   let tokens = client.silicon_login("si:scout", "stk-0123456789ab", None).await?;
//!   let session = client.with_token(tokens.access_token.expose());
//!   let slt = session.short_lived_token("remind").await?;
//!   // hand slt.slt.expose() to the app; it calls exchange_slt
//!   # Ok(()) }
//!   ```
//! * **As an app** — send browsers to [`AccountsClient::authorize_url`], exchange the code,
//!   verify proofs and webhooks:
//!   ```no_run
//!   # async fn demo(code: &str, verifier: &str) -> silicon_accounts_client::Result<()> {
//!   use silicon_accounts_client::AccountsClient;
//!   let client = AccountsClient::new("https://accounts.teamofsilicons.com")?;
//!   let app = client.as_app("briefcase", "sa_app_…");
//!   let tokens = app.exchange_code(code, "https://briefcase.example/callback", Some(verifier)).await?;
//!   let account = tokens.account.expect("token responses carry the account");
//!   println!("{} signed in (membership {})", account.id, account.membership_id);
//!   # Ok(()) }
//!   ```
//! * **As a Carbon** — sign in with the device flow ([`AccountsClient::device_authorize`])
//!   or a code ([`AccountsClient::cli_login_start`]) and manage your account and the
//!   Silicons you are custodian of through [`AccountSession`].
//!
//! Errors are typed ([`Error`]); every one carries a precise `message` and a `hint`.
//!
//! Identifiers: the account `uuid` never changes and is what you store; the `c:`/`si:`
//! id is what people see and it can change (apps get `account.id_changed`). A membership
//! with an app is `{app_id}:{uuid}`.

#![warn(missing_docs)]

mod app;
mod client;
mod config;
mod error;
mod jwt;
mod pkce;
mod public;
mod secret;
mod serde_util;
mod service;
mod session;
pub mod types;
mod wait;
mod webhook;

pub use app::{AppClient, MAX_IMPORT_BYTES};
pub use client::{
    AccountsClient, ClientBuilder, DEFAULT_BASE_URL, DEVELOPER_APP_ID, FIRST_PARTY_APP_ID,
    IDEMPOTENCY_HEADER, IDEMPOTENT_REPLAYED_HEADER, REQUEST_ID_HEADER, TELEMETRY_HEADER, VERSION,
};
pub use config::{Config, parse_flag};
pub use error::{ApiError, Error, OAuthError, Result, TokenError, WebhookError};
pub use jwt::{Claims, VerifyOptions, verify_access_token};
pub use pkce::{
    AuthorizeParams, PkcePair, pkce_challenge, pkce_pair, random_nonce, random_state, random_token,
};
pub use public::{DEVICE_CODE_GRANT_TYPE, SLT_GRANT_TYPE};
pub use secret::Secret;
pub use session::AccountSession;
pub use types::*;
pub use wait::{WaitEvent, WaitOptions};
pub use webhook::{
    AccountIdChanged, AccountUpdated, CustodianChanged, DEFAULT_WEBHOOK_TOLERANCE,
    DELIVERY_ID_HEADER, EVENT_ID_HEADER, EVENT_TYPE_HEADER, MembershipEvent, MembershipSignedOut,
    SIGNATURE_HEADER, TIMESTAMP_HEADER, WebhookEvent, WebhookPayload, parse_webhook, sign_webhook,
    verify_and_parse_webhook, verify_webhook_signature, verify_webhook_signature_at,
};
