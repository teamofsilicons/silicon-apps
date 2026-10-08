use crate::error::{ApiError, Result};
use std::{net::SocketAddr, path::PathBuf};
#[derive(Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
    pub dev_auth: bool,
    pub accounts_url: String,
    pub accounts_app_secret: Option<String>,
    pub accounts_service_token: Option<String>,
    pub public_url: String,
    pub allowed_origins: Vec<String>,
    pub runner_url: Option<String>,
    pub runner_token: Option<String>,
    pub runner_targets: Vec<String>,
    pub mail_url: Option<String>,
    pub mail_token: Option<String>,
    pub telemetry_url: Option<String>,
    pub telemetry_enabled: bool,
    pub telemetry_table_key: Option<String>,
    pub import_accounts: bool,
}
impl Config {
    pub fn validate_dev_boundary(&self) -> Result<()> {
        if self.dev_auth {
            let local = |raw: &str| {
                url::Url::parse(raw).ok().is_some_and(|u| match u.host() {
                    Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                    Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                    Some(url::Host::Domain(d)) => d == "localhost" || d.ends_with(".localhost"),
                    None => false,
                })
            };
            if !self.bind.ip().is_loopback()
                || !local(&self.public_url)
                || self.allowed_origins.iter().any(|origin| !local(origin))
            {
                return Err(ApiError::bad(
                    "Development authentication requires a loopback bind, public URL and every allowed origin. Never expose APPS_DEV_AUTH through a public proxy.",
                ));
            }
        }
        Ok(())
    }
    pub fn from_env() -> Result<Self> {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let bind = get("APPS_BIND")
            .unwrap_or_else(|| "127.0.0.1:4310".into())
            .parse::<SocketAddr>()
            .map_err(|_| ApiError::bad("APPS_BIND must be an IP address and port."))?;
        let dev_auth = get("APPS_DEV_AUTH").as_deref() == Some("1");
        if dev_auth && !bind.ip().is_loopback() {
            return Err(ApiError::bad(
                "APPS_DEV_AUTH=1 requires a loopback bind address.",
            ));
        }
        let public_url = get("APPS_PUBLIC_URL").unwrap_or_else(|| format!("http://{bind}"));
        let url = url::Url::parse(&public_url)
            .map_err(|_| ApiError::bad("APPS_PUBLIC_URL must be an absolute URL."))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(ApiError::bad("APPS_PUBLIC_URL must use HTTP(S)."));
        }
        if !bind.ip().is_loopback() && url.scheme() != "https" {
            return Err(ApiError::bad(
                "Non-loopback servers require APPS_PUBLIC_URL=https://… for secure sign-in cookies.",
            ));
        }
        let mut allowed_origins = vec![url.origin().ascii_serialization()];
        if let Some(origins) = get("APPS_ALLOWED_ORIGINS") {
            for origin in origins.split(',').map(str::trim) {
                let parsed = url::Url::parse(origin).map_err(|_| {
                    ApiError::bad(
                        "APPS_ALLOWED_ORIGINS must contain comma-separated HTTP(S) origins.",
                    )
                })?;
                if !matches!(parsed.scheme(), "https" | "http")
                    || parsed.path() != "/"
                    || parsed.query().is_some()
                    || parsed.fragment().is_some()
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                    || parsed.host_str().is_none()
                    || !bind.ip().is_loopback() && parsed.scheme() != "https"
                {
                    return Err(ApiError::bad(
                        "Allowed origins must be HTTP(S) origins without paths, queries, fragments or credentials; deployed origins require HTTPS.",
                    ));
                }
                let origin = parsed.origin().ascii_serialization();
                if !allowed_origins.contains(&origin) {
                    allowed_origins.push(origin);
                }
            }
        }
        Ok(Self {
            bind,
            allowed_origins,
            data_dir: get("APPS_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".data")),
            dev_auth,
            accounts_url: get("APPS_ACCOUNTS_URL")
                .unwrap_or_else(|| "https://accounts.teamofsilicons.com".into()),
            accounts_app_secret: get("APPS_ACCOUNTS_APP_SECRET"),
            accounts_service_token: get("APPS_ACCOUNTS_SERVICE_TOKEN"),
            public_url: public_url.trim_end_matches('/').into(),
            runner_url: get("APPS_RUNNER_URL"),
            runner_token: get("APPS_RUNNER_TOKEN"),
            runner_targets: get("APPS_RUNNER_TARGETS")
                .unwrap_or_default()
                .split(',')
                .filter(|v| !v.trim().is_empty())
                .map(|v| v.trim().into())
                .collect(),
            mail_url: get("APPS_MAIL_URL"),
            mail_token: get("APPS_MAIL_TOKEN"),
            telemetry_url: get("APPS_TELEMETRY_URL"),
            telemetry_enabled: get("APPS_TELEMETRY_ENABLED")
                .is_none_or(|v| !["0", "false", "off"].contains(&v.as_str())),
            telemetry_table_key: get("APPS_TELEMETRY_TABLE_KEY"),
            import_accounts: get("APPS_IMPORT_ACCOUNTS")
                .map(|v| v == "1")
                .unwrap_or(!dev_auth && get("APPS_ACCOUNTS_SERVICE_TOKEN").is_some()),
        })
    }
}
