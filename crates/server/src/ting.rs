//! Ting is a live wake-up transport, not the event store or an authorization service.
//! Each Apps stream owns an anonymous receiver that accepts only silicon-apps.
//! Only an empty hint crosses Ting; the stream reads authorized events from SQLite.
//! Dropping the stream drops its socket. A reconnect always creates a new session.

use crate::{
    Shared,
    config::Config,
    error::{ApiError, Result},
    model::APP_ID,
};
use serde_json::json;
use silicon_accounts_client::{IssueAppVerification, ProofKind, Secret};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use ting_client::{
    Client, Prepared, ProofOperation,
    websocket::{Event, WebSocket},
};

pub const EVENT_TYPE: &str = "silicon-apps.eventsAvailable";
pub const TIMEOUT: Duration = Duration::from_secs(8);

/// Explicit deployment operation. Identical registration is idempotent in Ting;
/// a conflicting existing definition fails instead of overwriting another setup.
pub async fn register_type(config: &Config) -> Result<()> {
    let publisher = Publisher::configured(config)?.ok_or_else(unavailable)?;
    let accounts = silicon_accounts_client::AccountsClient::new(&config.accounts_url)
        .map_err(|_| unavailable())?
        .with_telemetry(false);
    let proof = accounts
        .as_app(
            APP_ID,
            config
                .accounts_app_secret
                .as_deref()
                .ok_or_else(unavailable)?,
        )
        .issue_app_verification(
            &IssueAppVerification {
                receiving_app: "ting".into(),
                scopes: vec!["types.manage".into()],
                access_ttl_seconds: Some(60),
            },
            None,
        )
        .await
        .map_err(|_| unavailable())?;
    if proof.kind != Some(ProofKind::AppVerification)
        || proof.issuing_app.as_deref() != Some(APP_ID)
        || proof.receiving_app.as_deref() != Some("ting")
        || !proof.scopes.iter().any(|scope| scope == "types.manage")
        || proof.user.is_some()
        || proof.proof_token.is_empty()
    {
        return Err(unavailable());
    }
    publisher.client.request("POST", &format!("/v1/apps/{APP_ID}/types"), Some(json!({
        "type": EVENT_TYPE,
        "description": "An active Silicon Apps event watcher should reconcile its authorized event feed.",
        "schema": {"type":"object", "properties":{}, "additionalProperties":false}
    }).to_string().into_bytes()), Some(proof.proof_token.expose()), None).await.map_err(|_| {
        ApiError::unavailable("Ting type registration failed. Check types.manage authority and the existing type definition; no credentials or upstream response are logged.")
    })?;
    Ok(())
}

pub fn unavailable() -> ApiError {
    ApiError::new(
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "ting_unavailable",
        "Ting live event delivery is unavailable.",
        "Reconnect with the last event ID and transport=auto or direct; saved events remain available.",
    )
}

pub struct Publisher {
    client: Client,
    socket: tokio::sync::Mutex<Option<WebSocket>>,
    targets: Arc<Mutex<BTreeMap<String, Target>>>,
    // One short-lived, non-delegated app proof shared by all receivers. No login
    // tokens or refresh families, and no secrets in Debug, URLs or notification data.
    proof: tokio::sync::Mutex<Option<(Instant, Secret)>>,
}

#[derive(Clone)]
struct Target {
    who: crate::model::Identity,
    feed: crate::events::Feed,
    types: crate::events::TypeFilter,
    cursor: i64,
    failed: Arc<AtomicBool>,
    failure: Arc<tokio::sync::Notify>,
}

impl Publisher {
    pub fn configured(config: &Config) -> Result<Option<Self>> {
        let Some(url) = &config.ting_url else {
            return Ok(None);
        };
        if config
            .accounts_app_secret
            .as_deref()
            .is_none_or(str::is_empty)
        {
            return Err(ApiError::bad(
                "APPS_TING_URL requires APPS_ACCOUNTS_APP_SECRET.",
            ));
        }
        let client = Client::new(url).map_err(|_| ApiError::bad(
            "APPS_TING_URL must be an HTTPS origin, or a loopback HTTP origin for local development."
        ))?;
        Ok(Some(Self {
            client,
            proof: Default::default(),
            socket: Default::default(),
            targets: Default::default(),
        }))
    }

    async fn token(&self, s: &Shared) -> Result<Secret> {
        let mut cached = self.proof.lock().await;
        if let Some((until, token)) = cached.as_ref()
            && *until > Instant::now()
        {
            return Ok(token.clone());
        }
        let proof = s
            .accounts
            .as_app(
                APP_ID,
                s.config
                    .accounts_app_secret
                    .as_deref()
                    .ok_or_else(unavailable)?,
            )
            .issue_app_verification(
                &IssueAppVerification {
                    receiving_app: "ting".into(),
                    scopes: vec!["tings.send".into()],
                    access_ttl_seconds: Some(60),
                },
                None,
            )
            .await
            .map_err(|_| unavailable())?;
        let remaining = proof
            .expires_at
            .map(|expiry| expiry.unix_timestamp() - chrono::Utc::now().timestamp())
            .unwrap_or(0);
        if proof.kind != Some(ProofKind::AppVerification)
            || proof.issuing_app.as_deref() != Some(APP_ID)
            || proof.receiving_app.as_deref() != Some("ting")
            || !proof.scopes.iter().any(|scope| scope == "tings.send")
            || proof.user.is_some()
            || proof.proof_token.is_empty()
            || remaining <= 10
        {
            return Err(unavailable());
        }
        let token = proof.proof_token;
        *cached = Some((
            Instant::now() + Duration::from_secs(remaining.min(60) as u64 - 10),
            token.clone(),
        ));
        Ok(token)
    }

    pub async fn receiver(
        &self,
        who: crate::model::Identity,
        feed: crate::events::Feed,
        types: crate::events::TypeFilter,
        cursor: i64,
    ) -> Result<Receiver> {
        tokio::time::timeout(TIMEOUT, async {
            let mut socket = WebSocket::connect(&self.client)
                .await
                .map_err(|_| unavailable())?;
            let registered = socket
                .register(&[APP_ID.into()], None)
                .await
                .map_err(|_| unavailable())?;
            if registered["session_id"] != socket.session_id()
                || registered.get("for") != Some(&serde_json::Value::Null)
                || registered["app_ids"] != json!([APP_ID])
            {
                return Err(unavailable());
            }
            let failed = Arc::new(AtomicBool::new(false));
            let failure = Arc::new(tokio::sync::Notify::new());
            self.targets.lock().unwrap().insert(
                socket.session_id().into(),
                Target {
                    who,
                    feed,
                    types,
                    cursor,
                    failed: failed.clone(),
                    failure: failure.clone(),
                },
            );
            Ok(Receiver {
                socket,
                targets: self.targets.clone(),
                failed,
                failure,
            })
        })
        .await
        .map_err(|_| unavailable())?
    }
}

pub struct Receiver {
    socket: WebSocket,
    targets: Arc<Mutex<BTreeMap<String, Target>>>,
    failed: Arc<AtomicBool>,
    failure: Arc<tokio::sync::Notify>,
}
impl Drop for Receiver {
    fn drop(&mut self) {
        self.targets.lock().unwrap().remove(self.session_id());
    }
}
impl Receiver {
    pub fn is_connected(&self) -> bool {
        self.socket.is_connected() && !self.failed.load(Ordering::SeqCst)
    }
    pub fn session_id(&self) -> &str {
        self.socket.session_id()
    }

    pub fn advance(&self, cursor: i64) {
        if let Some(target) = self.targets.lock().unwrap().get_mut(self.session_id()) {
            target.cursor = target.cursor.max(cursor);
        }
    }
    pub fn identity(&self, who: &crate::model::Identity) {
        if let Some(target) = self.targets.lock().unwrap().get_mut(self.session_id()) {
            target.who = who.clone();
        }
    }
    pub async fn changed(&mut self) -> Result<()> {
        loop {
            let event = tokio::select! {
                _ = self.failure.notified() => return Err(unavailable()),
                event = self.socket.next_event() => event.map_err(|_| unavailable())?,
            };
            match event {
                Event::Tings {
                    session_id: Some(session),
                    webhook_id: None,
                    tings,
                } if session == self.session_id() => {
                    if tings.iter().any(|ting| {
                        ting["type"] == EVENT_TYPE && ting["session_id"] == self.session_id()
                    }) {
                        return Ok(());
                    }
                }
                Event::Paused { .. } => return Err(unavailable()),
                _ => {}
            }
        }
    }
}

impl Publisher {
    async fn send_hint(&self, s: &Shared, session: &str, through: i64) -> Result<()> {
        let key = format!(
            "apps-events:{}",
            crate::store::hash(format!("{session}:{through}").as_bytes())
        );
        let request = Prepared::new(
            ProofOperation::Send,
            json!({
                "type":EVENT_TYPE,"session_id":session,"key":key,"data":{},"metadata":{}
            })
            .to_string()
            .into_bytes(),
        )
        .map_err(|_| unavailable())?;
        // One publishing connection and short-lived app proof, shared by every
        // receiver. Publisher failures never participate in a catalog transaction.
        let mut socket = self.socket.lock().await;
        let result = tokio::time::timeout(TIMEOUT, async {
            let proof = self.token(s).await?;
            if socket.as_ref().is_none_or(|socket| !socket.is_connected()) {
                *socket = Some(
                    WebSocket::connect(&self.client)
                        .await
                        .map_err(|_| unavailable())?,
                );
            }
            socket
                .as_mut()
                .unwrap()
                .send(&request, proof.expose())
                .await
                .map_err(|_| unavailable())?;
            Ok(())
        })
        .await
        .map_err(|_| unavailable())
        .and_then(|r| r);
        if result.is_err() {
            *socket = None;
            if let Ok(mut proof) = self.proof.try_lock() {
                *proof = None;
            }
        }
        result
    }
}

/// Observe committed events and coalesce hints for active authorized watchers.
/// The durable event log is the retry/replay source. Session IDs are never stored
/// in SQLite, reused after reconnect, or turned into account-wide broadcasts.
pub async fn delivery_worker(s: Shared) {
    let Some(publisher) = &s.ting else { return };
    let mut notify = s.store.lock().unwrap().events.subscribe();
    loop {
        let targets = publisher.targets.lock().unwrap().clone();
        if targets.is_empty() {
            *publisher.socket.lock().await = None;
        }
        for (session, target) in targets {
            if target.failed.load(Ordering::SeqCst) {
                continue;
            }
            let scan = crate::events::pending_hint(
                &s,
                &target.who,
                &target.feed,
                &target.types,
                target.cursor,
            );
            match scan {
                Ok((pending, through)) => {
                    if pending && publisher.send_hint(&s, &session, through).await.is_err() {
                        target.failed.store(true, Ordering::SeqCst);
                        target.failure.notify_one();
                    } else if let Some(current) =
                        publisher.targets.lock().unwrap().get_mut(&session)
                    {
                        current.cursor = current.cursor.max(through);
                    }
                }
                Err(_) => {
                    target.failed.store(true, Ordering::SeqCst);
                    target.failure.notify_one();
                }
            }
        }
        tokio::select! {
            _ = notify.changed() => {},
            _ = tokio::time::sleep(Duration::from_secs(1)) => {},
        }
    }
}
