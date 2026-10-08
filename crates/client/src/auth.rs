//! Silicon Accounts authentication through the official current Rust client.
use crate::{Client, Config, LocalState, state};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use silicon_accounts_client::{AccountsClient, DeviceAuthorization, TokenResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSession {
    #[serde(default)]
    pub app_id: String,
    pub tokens: TokenResponse,
    pub expires_at: i64,
    #[serde(default)]
    pub server: String,
    #[serde(default)]
    pub accounts_url: String,
}
pub fn accounts(config: &Config) -> Result<AccountsClient> {
    Ok(AccountsClient::builder()
        .base_url(&config.accounts_url)
        .telemetry(config.telemetry)
        .build()?)
}
pub async fn begin_device(config: &Config) -> Result<DeviceAuthorization> {
    Ok(accounts(config)?
        .device_authorize(Some("Silicon Apps CLI"))
        .await?)
}
pub async fn finish_device(
    config: &Config,
    client: &Client,
    challenge: &DeviceAuthorization,
) -> Result<TokenResponse> {
    let accounts = accounts(config)?;
    let first = accounts.wait_for_device_tokens(challenge, |_| {}).await?;
    let result = async {
        let slt = accounts
            .with_token(first.access_token.expose())
            .short_lived_token(crate::APP_ID)
            .await?;
        exchange(client, slt.slt.expose()).await
    }
    .await;
    if let Some(refresh) = first.refresh_token {
        let _ = accounts.revoke_first_party(refresh.expose()).await;
    }
    result
}
pub async fn silicon_login(
    config: &Config,
    client: &Client,
    id: &str,
    stk: &str,
) -> Result<TokenResponse> {
    let accounts = accounts(config)?;
    let first = accounts
        .silicon_login(id, stk, Some("Silicon Apps CLI"))
        .await?;
    let result = async {
        let slt = accounts
            .with_token(first.access_token.expose())
            .short_lived_token(crate::APP_ID)
            .await?;
        exchange(client, slt.slt.expose()).await
    }
    .await;
    if let Some(refresh) = first.refresh_token {
        let _ = accounts.revoke_first_party(refresh.expose()).await;
    }
    result
}
pub async fn exchange(client: &Client, slt: &str) -> Result<TokenResponse> {
    let value = client
        .request(
            "POST",
            &["v1", "auth", "exchange"],
            &[],
            Some(json!({"slt":slt})),
            None,
        )
        .await?;
    serde_json::from_value(value).context("Apps auth exchange returned invalid tokens")
}
pub fn save(state: &LocalState, config: &Config, tokens: TokenResponse) -> Result<()> {
    state.initialize()?;
    let saved = SavedSession {
        app_id: crate::APP_ID.into(),
        expires_at: chrono::Utc::now().timestamp() + tokens.expires_in as i64,
        tokens,
        server: service_scope(&config.server)?,
        accounts_url: service_scope(&config.accounts_url)?,
    };
    state::atomic_json(&state.root.join("session.json"), &saved)
}

/// Scope credentials to the complete service URL (origin and optional tenant path).
pub fn service_scope(value: &str) -> Result<String> {
    let url = url::Url::parse(value)?;
    ensure!(
        url.has_host() && matches!(url.scheme(), "http" | "https"),
        "service URL must have an HTTP(S) origin"
    );
    Ok(url.as_str().trim_end_matches('/').to_owned())
}
fn check_scope(session: &SavedSession, config: &Config) -> Result<()> {
    ensure!(
        !session.server.is_empty() && !session.accounts_url.is_empty(),
        "This saved session predates server scoping and cannot be safely reused. Run `silicon-apps login` against the intended Apps and Accounts servers."
    );
    ensure!(
        session.server == service_scope(&config.server)?
            && session.accounts_url == service_scope(&config.accounts_url)?,
        "Saved sign-in belongs to Apps {} and Accounts {}; selected servers are Apps {} and Accounts {}. No saved access or refresh token was sent. Select the original servers or run `silicon-apps login` for these servers.",
        session.server,
        session.accounts_url,
        config.server,
        config.accounts_url
    );
    Ok(())
}
pub fn read(state: &LocalState) -> Result<Option<SavedSession>> {
    state::read_json(&state.root.join("session.json"))
}
/// Serialize rotating refresh-token use across the updater and interactive CLI.
pub async fn authenticated_client(
    state: &LocalState,
    config: &Config,
    explicit_token: Option<String>,
) -> Result<Client> {
    let base = Client::new(&config.server, None)?.with_telemetry(config.telemetry);
    if let Some(token) = explicit_token {
        return Ok(base.authenticated(Some(token)));
    }
    if !state.root.join("session.json").exists() {
        return Ok(base);
    }
    let _lock = state.lock("auth")?;
    let Some(session) = read(state)? else {
        return Ok(base);
    };
    check_scope(&session, config)?;
    if session.app_id == crate::APP_ID && session.expires_at > chrono::Utc::now().timestamp() + 30 {
        return Ok(base.authenticated(Some(session.tokens.access_token.expose().into())));
    }
    let Some(refresh) = session.tokens.refresh_token else {
        bail!("Apps sign-in expired and has no refresh token. Run `silicon-apps login` again.");
    };
    let value = base
        .request(
            "POST",
            &["v1", "auth", "refresh"],
            &[],
            Some(json!({"refresh_token":refresh.expose()})),
            None,
        )
        .await?;
    let tokens: TokenResponse = serde_json::from_value(value)?;
    let token = tokens.access_token.expose().to_owned();
    save(state, config, tokens)?;
    Ok(base.authenticated(Some(token)))
}
pub async fn status(state: &LocalState, client: &Client, explicit_token: bool) -> Result<Value> {
    if !explicit_token && read(state)?.is_none() {
        return Ok(json!({"app_id":crate::APP_ID,"authenticated":false}));
    }
    let account = client.me().await?;
    Ok(json!({"app_id":crate::APP_ID,"authenticated":true,"account":account}))
}
pub async fn logout(state: &LocalState, config: &Config, client: &Client) -> Result<()> {
    let _lock = state.lock("auth")?;
    if let Some(session) = read(state)? {
        check_scope(&session, config)?;
        ensure!(
            service_scope(client.base_url())? == session.server,
            "Logout client does not match this session's Apps server; no token was sent"
        );
        let token = session
            .tokens
            .refresh_token
            .as_ref()
            .unwrap_or(&session.tokens.access_token)
            .expose();
        client
            .request(
                "POST",
                &["v1", "auth", "logout"],
                &[],
                Some(json!({"token":token})),
                None,
            )
            .await?;
        std::fs::remove_file(state.root.join("session.json"))?;
    }
    Ok(())
}
