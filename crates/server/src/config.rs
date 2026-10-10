use crate::{
    error::{ApiError, Result},
    model::{Identity, reserved_app_id, valid_app_id},
};
use std::{collections::BTreeMap, net::SocketAddr, path::PathBuf};

/// `APPS_HISTORICAL_APP_IDS`: app IDs that Silicon Accounts issued before
/// Silicon Apps existed and that are shorter than a new ID may be (1 or 2
/// characters, such as `dm`). Each is reserved for the one Carbon or Silicon,
/// by Silicon Accounts UUID, who may create it here; to anyone else it is an
/// invalid ID. Comma-separated `app_id:owner_uuid` entries, such as `dm:zQo`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoricalAppIds(BTreeMap<String, String>);
impl HistoricalAppIds {
    pub fn parse(raw: &str) -> Result<Self> {
        let bad = |message: String| {
            ApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "invalid_historical_app_ids",
                message,
                "Set APPS_HISTORICAL_APP_IDS to comma-separated app_id:owner_uuid entries, such as dm:zQo, or leave it unset.",
            )
        };
        let mut ids = BTreeMap::new();
        for entry in raw.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let Some((id, owner)) = entry.split_once(':') else {
                return Err(bad(format!(
                    "APPS_HISTORICAL_APP_IDS entry `{entry}` is not app_id:owner_uuid."
                )));
            };
            let (id, owner) = (id.trim(), owner.trim());
            if !silicon_apps_package::valid_existing_app_id(id) {
                return Err(bad(format!(
                    "APPS_HISTORICAL_APP_IDS: `{id}` is not an app ID; a historical app ID is 1 or 2 of a-z, 0-9, - and _."
                )));
            }
            if reserved_app_id(id) {
                return Err(bad(format!(
                    "APPS_HISTORICAL_APP_IDS: `{id}` is reserved for a built-in Silicon Accounts service."
                )));
            }
            if valid_app_id(id) {
                return Err(bad(format!(
                    "APPS_HISTORICAL_APP_IDS: `{id}` has 3 or more characters, so it is an ordinary new app ID; list only historical app IDs of 1 or 2 characters."
                )));
            }
            if owner.is_empty() {
                return Err(bad(format!(
                    "APPS_HISTORICAL_APP_IDS: `{id}` has no owner; put the owner's Silicon Accounts UUID after the colon."
                )));
            }
            let canonical_uuid = uuid::Uuid::parse_str(owner).is_ok_and(|value| {
                value.get_variant() == uuid::Variant::RFC4122 && value.to_string() == owner
            });
            let legacy_uuid =
                (3..=12).contains(&owner.len()) && owner.bytes().all(|b| b.is_ascii_alphanumeric());
            if !canonical_uuid && !legacy_uuid {
                return Err(bad(format!(
                    "APPS_HISTORICAL_APP_IDS: owner `{owner}` of `{id}` is not a Silicon Accounts UUID; use a canonical lowercase UUID or an existing 3–12 character legacy ID."
                )));
            }
            if ids.insert(id.to_owned(), owner.to_owned()).is_some() {
                return Err(bad(format!(
                    "APPS_HISTORICAL_APP_IDS lists `{id}` more than once; give each historical app ID exactly one owner."
                )));
            }
        }
        Ok(Self(ids))
    }
    /// Whether `who` is signed in as the configured owner of the historical
    /// app ID `id`. UUIDs are case-sensitive and compared exactly.
    pub fn allows(&self, id: &str, who: Option<&Identity>) -> bool {
        who.is_some_and(|who| self.0.get(id).is_some_and(|owner| *owner == who.uuid))
    }
    /// The configured historical app IDs, in order.
    pub fn ids(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

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
    /// `APPS_HISTORICAL_APP_IDS`: 1–2 character Accounts app IDs and the one
    /// account each may be created by. Empty unless configured.
    pub historical_app_ids: HistoricalAppIds,
    /// Requests per minute per client for reads (GET); 0 disables the limit.
    pub rate_limit_reads_per_minute: u32,
    /// Requests per minute per client for mutations; 0 disables the limit.
    pub rate_limit_writes_per_minute: u32,
    /// Event streams one client may hold open at once; 0 disables the limit.
    pub rate_limit_streams: u32,
    /// `APPS_SIGNING_KEYS`: `key_id:base64-seed` Ed25519 keys, newest first.
    /// Required outside local development; locally a key is generated.
    pub signing_keys: Option<String>,
    /// `APPS_REVOKED_SIGNING_KEYS`: key IDs CLIs must stop trusting.
    pub revoked_signing_keys: Vec<String>,
}
impl Config {
    /// A loopback public URL means local development: webhook subscriptions
    /// may then deliver to loopback and private addresses over HTTP.
    pub fn local_development(&self) -> bool {
        url::Url::parse(&self.public_url)
            .ok()
            .is_some_and(|u| match u.host() {
                Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                Some(url::Host::Domain(d)) => d == "localhost" || d.ends_with(".localhost"),
                None => false,
            })
    }
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
        let limit = |k: &str, default: u32| -> Result<u32> {
            get(k).map_or(Ok(default), |v| {
                v.parse().map_err(|_| {
                    ApiError::bad(format!("{k} must be a whole number; 0 disables it."))
                })
            })
        };
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
            historical_app_ids: HistoricalAppIds::parse(
                &get("APPS_HISTORICAL_APP_IDS").unwrap_or_default(),
            )?,
            rate_limit_reads_per_minute: limit("APPS_RATE_LIMIT_READS_PER_MINUTE", 600)?,
            rate_limit_writes_per_minute: limit("APPS_RATE_LIMIT_WRITES_PER_MINUTE", 120)?,
            rate_limit_streams: limit("APPS_RATE_LIMIT_STREAMS", 10)?,
            signing_keys: get("APPS_SIGNING_KEYS"),
            revoked_signing_keys: get("APPS_REVOKED_SIGNING_KEYS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(uuid: &str) -> Identity {
        Identity {
            uuid: uuid.into(),
            id: format!("c:{uuid}"),
            display_name: uuid.into(),
            verified_emails: vec![],
        }
    }
    fn refused(raw: &str) -> String {
        let error = HistoricalAppIds::parse(raw).unwrap_err();
        assert_eq!(error.code, "invalid_historical_app_ids", "{raw}");
        error.message
    }

    #[test]
    fn historical_app_ids_parse_and_allow_only_their_exact_owner() {
        assert!(HistoricalAppIds::parse("").unwrap().is_empty());
        let ids = HistoricalAppIds::parse(" dm : zQo ,, x_:Ab9,").unwrap();
        assert_eq!(ids.ids(), ["dm", "x_"]);
        assert!(ids.allows("dm", Some(&account("zQo"))));
        assert!(ids.allows("x_", Some(&account("Ab9"))));
        // Signed out, another account, another case or another ID: never.
        assert!(!ids.allows("dm", None));
        assert!(!ids.allows("dm", Some(&account("Ab9"))));
        assert!(!ids.allows("dm", Some(&account("zqo"))));
        assert!(!ids.allows("dm", Some(&account("zQo "))));
        assert!(!ids.allows("x_", Some(&account("zQo"))));
        assert!(!ids.allows("wf", Some(&account("zQo"))));
        // One owner may hold several historical IDs.
        let both = HistoricalAppIds::parse("dm:zQo,7:zQo").unwrap();
        assert!(
            both.allows("dm", Some(&account("zQo"))) && both.allows("7", Some(&account("zQo")))
        );
    }

    #[test]
    fn malformed_historical_app_ids_are_refused_with_exact_messages() {
        assert_eq!(
            refused("dm"),
            "APPS_HISTORICAL_APP_IDS entry `dm` is not app_id:owner_uuid."
        );
        assert_eq!(
            refused("dm:zQo,wf"),
            "APPS_HISTORICAL_APP_IDS entry `wf` is not app_id:owner_uuid."
        );
        assert_eq!(
            refused("dm:"),
            "APPS_HISTORICAL_APP_IDS: `dm` has no owner; put the owner's Silicon Accounts UUID after the colon."
        );
        assert_eq!(
            refused("dm:zQo:extra"),
            "APPS_HISTORICAL_APP_IDS: owner `zQo:extra` of `dm` is not a Silicon Accounts UUID; use a canonical lowercase UUID or an existing 3–12 character legacy ID."
        );
        assert_eq!(
            refused("dm:c:saket"),
            "APPS_HISTORICAL_APP_IDS: owner `c:saket` of `dm` is not a Silicon Accounts UUID; use a canonical lowercase UUID or an existing 3–12 character legacy ID."
        );
        for id in ["DM", "d.m", "", "d m"] {
            assert_eq!(
                refused(&format!("{id}:zQo")),
                format!(
                    "APPS_HISTORICAL_APP_IDS: `{id}` is not an app ID; a historical app ID is 1 or 2 of a-z, 0-9, - and _."
                )
            );
        }
        assert_eq!(
            refused(&format!("{}:zQo", "a".repeat(31))),
            format!(
                "APPS_HISTORICAL_APP_IDS: `{}` is not an app ID; a historical app ID is 1 or 2 of a-z, 0-9, - and _.",
                "a".repeat(31)
            )
        );
    }

    #[test]
    fn historical_app_owner_survives_the_standard_uuid_cutover_without_aliases() {
        let owner = "d7ce239a-7b3e-4e0b-9236-b936405c1fda";
        let ids = HistoricalAppIds::parse(&format!("dm:{owner}")).unwrap();
        assert!(ids.allows("dm", Some(&account(owner))));
        assert!(!ids.allows("dm", Some(&account("zQo"))));
        assert!(!ids.allows("dm", Some(&account(&owner.to_uppercase()))));
        assert!(!ids.allows("dm", None));
        for bad in [
            owner.to_uppercase(),
            owner.replace('-', ""),
            "x".into(),
            "x".repeat(13),
        ] {
            assert!(HistoricalAppIds::parse(&format!("dm:{bad}")).is_err());
        }
    }

    #[test]
    fn duplicate_historical_app_ids_are_refused() {
        for raw in ["dm:zQo,dm:Ab9", "dm:zQo, dm:zQo"] {
            assert_eq!(
                refused(raw),
                "APPS_HISTORICAL_APP_IDS lists `dm` more than once; give each historical app ID exactly one owner."
            );
        }
    }

    #[test]
    fn ids_anyone_can_create_or_that_are_reserved_are_refused() {
        for id in ["abc", "dm-", "briefcase"] {
            assert_eq!(
                refused(&format!("{id}:zQo")),
                format!(
                    "APPS_HISTORICAL_APP_IDS: `{id}` has 3 or more characters, so it is an ordinary new app ID; list only historical app IDs of 1 or 2 characters."
                )
            );
        }
        for id in ["apps", "accounts", "silicon-accounts", "developer"] {
            assert_eq!(
                refused(&format!("dm:zQo,{id}:zQo")),
                format!(
                    "APPS_HISTORICAL_APP_IDS: `{id}` is reserved for a built-in Silicon Accounts service."
                )
            );
        }
    }
}
