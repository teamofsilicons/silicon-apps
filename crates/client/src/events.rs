//! Event feeds, server-sent event streams, subscriptions and webhook
//! signature checks.

use anyhow::{Context, Result, bail, ensure};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Which event feed to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feed {
    /// The signed-in account: invitations to it, apps it authors and public
    /// events of apps it installed.
    Account,
    /// One app's events, for its authors.
    App(String),
    /// One of your subscriptions, through its filters.
    Subscription(String),
}
impl Feed {
    pub(crate) fn path(&self, stream: bool) -> Vec<String> {
        let mut path = match self {
            Feed::App(app) => vec!["v1".into(), "apps".into(), app.clone(), "events".into()],
            _ => vec!["v1".into(), "events".into()],
        };
        if stream {
            path.push("stream".into());
        }
        path
    }
    pub(crate) fn query(&self) -> Vec<(&'static str, String)> {
        match self {
            Feed::Subscription(id) => vec![("subscription", id.clone())],
            _ => vec![],
        }
    }
}

/// Where a subscription's events go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum Delivery {
    /// POST each event to this URL, signed with the subscription's secret.
    Webhook { url: String },
    /// Read events at `/v1/events/stream?subscription=ID`.
    Stream,
}

/// A new subscription. `types` and `channels` may be empty for the defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewSubscription {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<String>,
    pub delivery: Delivery,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
impl NewSubscription {
    /// Follow one app's production releases, delivered to a webhook.
    pub fn production_releases(app_id: &str, url: &str) -> Self {
        Self {
            app_id: Some(app_id.into()),
            types: vec!["release.promoted".into()],
            channels: vec![],
            delivery: Delivery::Webhook { url: url.into() },
            description: None,
        }
    }
    pub fn body(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| json!({}))
    }
}

/// One event from a stream.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamEvent {
    /// The event's seq. Pass it as `last_event_id` to resume after it.
    pub id: Option<String>,
    /// The event type, such as `release.promoted`.
    pub event: String,
    /// The event as JSON: seq, id, type, app_id, actor_uuid, occurred_at, data.
    pub data: Value,
}

/// A server-sent event stream. Comments (the ready line and heartbeats) are
/// skipped. `next` returns `None` when the server closes the stream; open it
/// again with the last id to continue.
pub struct EventStream {
    response: reqwest::Response,
    buffer: String,
    last_event_id: Option<String>,
}
impl EventStream {
    pub(crate) fn new(response: reqwest::Response) -> Self {
        Self {
            response,
            buffer: String::new(),
            last_event_id: None,
        }
    }
    /// The id of the last event returned, for resuming.
    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }
    pub async fn next(&mut self) -> Result<Option<StreamEvent>> {
        loop {
            while let Some(end) = self.buffer.find("\n\n") {
                let block: String = self.buffer.drain(..end + 2).collect();
                if let Some(event) = parse_block(&block)? {
                    if event.id.is_some() {
                        self.last_event_id = event.id.clone();
                    }
                    return Ok(Some(event));
                }
            }
            match self
                .response
                .chunk()
                .await
                .context("the event stream was interrupted")?
            {
                Some(chunk) => {
                    self.buffer
                        .push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n"));
                    ensure!(
                        self.buffer.len() <= 4 * 1024 * 1024,
                        "an event stream message exceeded 4 MiB"
                    );
                }
                None => return Ok(None),
            }
        }
    }
}
fn parse_block(block: &str) -> Result<Option<StreamEvent>> {
    let mut id = None;
    let mut event = None;
    let mut data = Vec::new();
    for line in block.lines() {
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "id" => id = Some(value.to_owned()),
            "event" => event = Some(value.to_owned()),
            "data" => data.push(value),
            _ => {}
        }
    }
    if data.is_empty() {
        return Ok(None);
    }
    let data: Value =
        serde_json::from_str(&data.join("\n")).context("an event's data was not JSON")?;
    Ok(Some(StreamEvent {
        id,
        event: event.unwrap_or_else(|| "message".into()),
        data,
    }))
}

/// Check a subscription webhook delivery before trusting it.
///
/// `body` must be the exact bytes received. `signature` is the
/// `X-Apps-Signature` header, a comma-separated list of `v1=` values; any
/// matching value is accepted. Timestamps further than `tolerance_seconds`
/// from `now` (unix seconds) are refused, so a captured delivery cannot be
/// replayed later. Five minutes is the recommended tolerance.
pub fn verify_webhook(
    secret: &str,
    timestamp: &str,
    signature: &str,
    body: &[u8],
    now: i64,
    tolerance_seconds: i64,
) -> Result<()> {
    let sent: i64 = timestamp
        .trim()
        .parse()
        .context("X-Apps-Timestamp is not a unix timestamp")?;
    ensure!(
        (now - sent).abs() <= tolerance_seconds,
        "X-Apps-Timestamp is {} seconds from this clock, more than the {tolerance_seconds} second tolerance",
        (now - sent).abs()
    );
    let expected = webhook_signature(secret, sent, body);
    if signature
        .split(',')
        .map(str::trim)
        .filter_map(|v| v.strip_prefix("v1="))
        .any(|candidate| constant_time_eq(candidate.as_bytes(), &expected.as_bytes()[3..]))
    {
        return Ok(());
    }
    bail!("X-Apps-Signature does not match this body and secret")
}
/// The `v1=` signature Apps sends for a body at a timestamp.
pub fn webhook_signature(secret: &str, timestamp: i64, body: &[u8]) -> String {
    let mut mac =
        Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes any key");
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body);
    format!("v1={}", hex::encode(mac.finalize().into_bytes()))
}
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
