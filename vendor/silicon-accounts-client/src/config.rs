//! Optional configuration from environment variables. The client itself never reads the
//! environment; call [`Config::from_env`] when you want to.

use std::time::Duration;

use crate::app::AppClient;
use crate::client::{AccountsClient, DEFAULT_BASE_URL};
use crate::error::{Error, Result};

/// Client configuration read from the environment.
///
/// | variable | meaning |
/// |---|---|
/// | `ACCOUNTS_URL` | Silicon Accounts URL (default `https://accounts.teamofsilicons.com`) |
/// | `ACCOUNTS_APP_ID` / `ACCOUNTS_APP_SECRET` | app credentials |
/// | `ACCOUNTS_TELEMETRY` | `0`, `off`, `false` or `no` disables telemetry |
/// | `ACCOUNTS_TIMEOUT_SECONDS` | request timeout (default 30) |
/// | `ACCOUNTS_ALLOW_INSECURE_HTTP` | `1` allows plain http to hosts other than this machine |
#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    /// Silicon Accounts URL.
    pub base_url: String,
    /// App id, when acting as an app.
    pub app_id: Option<String>,
    /// App secret, when acting as an app.
    pub app_secret: Option<String>,
    /// Telemetry on/off.
    pub telemetry: bool,
    /// Request timeout.
    pub timeout: Duration,
    /// Allow plain http to remote hosts.
    pub allow_insecure_http: bool,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("base_url", &self.base_url)
            .field("app_id", &self.app_id)
            .field("app_secret", &self.app_secret.as_ref().map(|_| "…"))
            .field("telemetry", &self.telemetry)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            app_id: None,
            app_secret: None,
            telemetry: true,
            timeout: Duration::from_secs(30),
            allow_insecure_http: false,
        }
    }
}

/// Parses a boolean-ish environment value: `1/true/yes/on` → true, `0/false/no/off` → false.
pub fn parse_flag(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

impl Config {
    /// Reads the variables in the table above.
    pub fn from_env() -> Result<Self> {
        let get = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        };
        let mut config = Self::default();
        if let Some(url) = get("ACCOUNTS_URL") {
            config.base_url = url;
        }
        config.app_id = get("ACCOUNTS_APP_ID");
        config.app_secret = get("ACCOUNTS_APP_SECRET");
        if let Some(value) = get("ACCOUNTS_TELEMETRY") {
            config.telemetry = parse_flag(&value).ok_or_else(|| {
                Error::invalid_input(
                    format!("ACCOUNTS_TELEMETRY is `{value}`, which is not on or off."),
                    "Set it to 1/on/true or 0/off/false.",
                )
            })?;
        }
        if let Some(value) = get("ACCOUNTS_TIMEOUT_SECONDS") {
            let seconds: u64 = value.parse().map_err(|_| {
                Error::invalid_input(
                    format!("ACCOUNTS_TIMEOUT_SECONDS is `{value}`, which is not a whole number of seconds."),
                    "Set it to a number such as 30.",
                )
            })?;
            config.timeout = Duration::from_secs(seconds.max(1));
        }
        if let Some(value) = get("ACCOUNTS_ALLOW_INSECURE_HTTP") {
            config.allow_insecure_http = parse_flag(&value).unwrap_or(false);
        }
        Ok(config)
    }

    /// Builds a client from this configuration.
    pub fn client(&self) -> Result<AccountsClient> {
        AccountsClient::builder()
            .base_url(&self.base_url)
            .timeout(self.timeout)
            .telemetry(self.telemetry)
            .allow_insecure_http(self.allow_insecure_http)
            .build()
    }

    /// The app client, when both `app_id` and `app_secret` are set.
    pub fn app_client<'a>(&self, client: &'a AccountsClient) -> Option<AppClient<'a>> {
        match (&self.app_id, &self.app_secret) {
            (Some(id), Some(secret)) => Some(client.as_app(id.clone(), secret.clone())),
            _ => None,
        }
    }
}
