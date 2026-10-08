use crate::{
    Shared,
    error::{ApiError, Result},
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};

fn initialize_tls() {
    // Accounts and Space Station enable different rustls provider features.
    // The WebSocket shipper uses the process default and otherwise panics when
    // both features are present. Preserve a provider already chosen by a host.
    let _ = rustls::crypto::ring::default_provider().install_default();
}

pub fn build(config: &crate::config::Config) -> Result<Option<Arc<space_station::SpaceClient>>> {
    if !config.telemetry_enabled {
        return Ok(None);
    }
    let Some(key) = config.telemetry_table_key.as_deref() else {
        return Ok(None);
    };
    initialize_tls();
    let client = space_station::SpaceClient::builder(key)
        .home(config.data_dir.join("telemetry"))
        .url(
            std::env::var("SPACE_STATION_URL")
                .unwrap_or_else(|_| space_station::DEFAULT_URL.into()),
        )
        .flush_timeout(Duration::from_millis(250))
        .on_error(|_| {
            eprintln!(
                "Space Station telemetry delivery is temporarily unavailable; local spool retained."
            )
        })
        .build()
        .map_err(|_| {
            ApiError::bad("APPS_TELEMETRY_TABLE_KEY is not a valid Space Station table key.")
        })?;
    Ok(Some(Arc::new(client)))
}
pub fn route_template(path: &str) -> String {
    let mut p: Vec<_> = path
        .split('?')
        .next()
        .unwrap_or("")
        .trim_matches('/')
        .split('/')
        .collect();
    if p.first() == Some(&"v1") {
        p.remove(0);
    }
    let developer = p.first() == Some(&"developer");
    if developer {
        p.remove(0);
    }
    if p.first() == Some(&"apps") && p.len() > 1 {
        if p[1] == "availability" {
            if p.len() > 2 {
                p[2] = ":app_id";
            }
        } else {
            p[1] = ":app_id";
            if p.len() > 3 {
                p[3] = ":resource_id";
            }
        }
    } else if p.first() == Some(&"authors") && p.len() > 1 {
        p[1] = ":author_uuid";
    } else if p.first() == Some(&"store") && p.len() > 1 {
        p[1] = ":app_id";
    } else if p.first() == Some(&"invites") && p.len() > 1 {
        p[1] = ":invite_id";
    } else if !p.first().is_some_and(|first| {
        [
            "",
            "session",
            "auth",
            "targets",
            "platforms",
            "reports",
            "telemetry",
            "me",
            "docs",
            "settings",
            "invitations",
            "store",
            "apps",
        ]
        .contains(first)
    }) {
        return "other".into();
    }
    if developer {
        p.insert(0, "developer");
    }
    p.join("/")
}

pub fn frontend_event(body: &Value) -> Result<Value> {
    let step = body["step"]
        .as_str()
        .filter(|v| {
            !v.is_empty()
                && v.len() <= 80
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
        })
        .ok_or_else(|| {
            ApiError::bad(
                "Telemetry step must contain 1–80 letters, digits, underscores, hyphens or dots.",
            )
        })?;
    let progress = body
        .get("progress")
        .filter(|v| {
            v.is_number()
                || v.as_str()
                    .is_some_and(|v| ["started", "complete", "failed", "pending"].contains(&v))
        })
        .cloned()
        .ok_or_else(|| {
            ApiError::bad("Telemetry progress must be a number or started/complete/failed/pending.")
        })?;
    let mut context = json!({});
    for key in [
        "target",
        "status_code",
        "item_count",
        "byte_count",
        "duration_ms",
        "error_code",
    ] {
        if let Some(v) = body.get(key)
            && (v.is_number()
                || v.is_boolean()
                || v.as_str().is_some_and(|v| {
                    v.len() <= 100
                        && v.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b))
                }))
        {
            context[key] = v.clone();
        }
    }
    if let Some(path) = body["path"].as_str() {
        context["route"] = json!(route_template(path));
    }
    Ok(
        json!({"source":"silicon-apps.frontend","version":env!("CARGO_PKG_VERSION"),"step":step,"progress":progress,"context":context}),
    )
}
pub fn record(s: &Shared, event: Value) {
    if let Some(client) = &s.telemetry {
        client.record(event);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn websocket_tls_configuration_works_with_both_dependency_crypto_providers() {
        super::initialize_tls();
        // This is the SDK WebSocket connector's builder path. Without selecting
        // a provider it panics when Accounts' aws-lc and Space Station's ring
        // features are unified into this executable.
        let _ = rustls::ClientConfig::builder()
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth();
        // Initializing another application state must also remain safe.
        super::initialize_tls();
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }
}
