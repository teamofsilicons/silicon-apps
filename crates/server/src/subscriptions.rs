//! Subscriptions to event feeds and their signed webhook deliveries.
//!
//! A signed-in Carbon or Silicon subscribes to one app it can see, or to its
//! own account feed, choosing event types and how events reach it: a webhook
//! signed with a per-subscription `whsec_` secret, or a stream it reads with
//! `GET /v1/events/stream?subscription=ID`. Deliveries follow the Silicon
//! Accounts webhook conventions: `v1=` HMAC-SHA256 signatures over
//! `"{timestamp}.{raw body}"`, retries for 72 hours and no redirects.

use crate::{
    Shared,
    error::{ApiError, Result},
    events::{self, Event, PUBLIC_TYPES, TypeFilter},
    model::*,
};
use axum::{
    Json,
    body::Bytes,
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use hmac::{Hmac, Mac};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

/// Waits after each failed attempt; the last one repeats.
pub const RETRY_SECONDS: [u64; 7] = [10, 30, 60, 300, 900, 1800, 3600];
/// A delivery fails for good once this long has passed since its event.
pub const RETRY_WINDOW_HOURS: i64 = 72;
pub const DELIVERY_TIMEOUT_SECONDS: u64 = 10;
pub const MAX_SUBSCRIPTIONS: i64 = 50;
const WORKER_BATCH: usize = 16;
const CLAIM_MS: i64 = 60_000;

pub fn missing() -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "subscription_not_found",
        "No subscription with this ID belongs to your account.",
        "List your subscriptions with GET /v1/subscriptions.",
    )
}
fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
fn ms_to_rfc3339(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .to_rfc3339()
}

#[derive(Clone, Debug)]
pub struct Subscription {
    pub id: String,
    pub owner_uuid: String,
    pub owner_id: String,
    pub owner_emails: Vec<String>,
    pub app_id: Option<String>,
    pub types: Vec<String>,
    pub channels: Vec<String>,
    pub delivery: String,
    pub url: Option<String>,
    pub secret: Option<String>,
    pub status: String,
    pub description: String,
    pub cursor: i64,
    pub created_at: String,
    pub updated_at: String,
    pub cancelled_at: Option<String>,
}
const COLUMNS: &str = "id,owner_uuid,owner_id,owner_emails,app_id,types,channels,delivery,url,secret,status,description,cursor,created_at,updated_at,cancelled_at";
fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Subscription> {
    let list = |i: usize| -> rusqlite::Result<Vec<String>> {
        Ok(serde_json::from_str(&r.get::<_, String>(i)?).unwrap_or_default())
    };
    Ok(Subscription {
        id: r.get(0)?,
        owner_uuid: r.get(1)?,
        owner_id: r.get(2)?,
        owner_emails: list(3)?,
        app_id: r.get(4)?,
        types: list(5)?,
        channels: list(6)?,
        delivery: r.get(7)?,
        url: r.get(8)?,
        secret: r.get(9)?,
        status: r.get(10)?,
        description: r.get(11)?,
        cursor: r.get(12)?,
        created_at: r.get(13)?,
        updated_at: r.get(14)?,
        cancelled_at: r.get(15)?,
    })
}
impl Subscription {
    pub fn owner(&self) -> Identity {
        Identity {
            uuid: self.owner_uuid.clone(),
            id: self.owner_id.clone(),
            display_name: String::new(),
            verified_emails: self.owner_emails.clone(),
        }
    }
    /// Whether this subscription receives an event. Access is checked when the
    /// event happens, so losing access to an app stops its deliveries.
    pub fn matches(&self, event: &Event, c: &Catalog) -> bool {
        let owner = self.owner();
        let visible = match &self.app_id {
            Some(app_id) => events::app_subscriber_can_see(event, app_id, &owner, c),
            None => events::account_can_see(event, &owner, c),
        };
        visible
            && TypeFilter(self.types.clone()).allows(&event.kind)
            && (self.channels.is_empty()
                || !event.kind.starts_with("release.")
                || event
                    .channel()
                    .is_some_and(|channel| self.channels.iter().any(|c| c == channel)))
    }
    pub fn view(&self) -> Value {
        json!({
            "id":self.id,
            "app_id":self.app_id,
            "types":self.types,
            "channels":self.channels,
            "delivery":match self.delivery.as_str() {
                "webhook" => json!({"mode":"webhook","url":self.url}),
                _ => json!({"mode":"stream"}),
            },
            "stream_url":format!("/v1/events/stream?subscription={}",self.id),
            "status":self.status,
            "description":self.description,
            "cursor":self.cursor,
            "created_at":self.created_at,
            "updated_at":self.updated_at,
            "cancelled_at":self.cancelled_at,
        })
    }
}
pub fn get(conn: &Connection, id: &str) -> Result<Option<Subscription>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM subscriptions WHERE id=?1"),
            [id],
            from_row,
        )
        .optional()?)
}
/// Webhook subscriptions that queue deliveries. Paused ones are included:
/// their deliveries are held and go out if they resume within the window.
pub fn webhook_targets(conn: &Connection) -> Result<Vec<Subscription>> {
    let mut statement = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM subscriptions WHERE delivery='webhook' AND status IN ('active','paused')"
    ))?;
    let rows = statement.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
pub fn queue_delivery(conn: &Connection, subscription_id: &str, seq: i64) -> Result<String> {
    let id = format!("dlv_{}", uuid::Uuid::new_v4().simple());
    let now = now_ms();
    conn.execute(
        "INSERT OR IGNORE INTO subscription_deliveries(id,subscription_id,event_seq,status,attempts,next_attempt_ms,created_ms,created_at) VALUES(?1,?2,?3,'pending',0,?4,?4,?5)",
        params![id, subscription_id, seq, now, ms_to_rfc3339(now)],
    )?;
    Ok(id)
}
/// Record the last event a subscription's reader acknowledged.
pub fn advance_cursor(s: &Shared, id: &str, seq: i64) {
    let _ = s.store.lock().unwrap().connection.execute(
        "UPDATE subscriptions SET cursor=?1 WHERE id=?2 AND cursor<?1",
        params![seq, id],
    );
}

fn new_secret() -> String {
    use base64::Engine;
    format!(
        "whsec_{}",
        base64::engine::general_purpose::STANDARD.encode(rand::random::<[u8; 32]>())
    )
}
/// `v1=` signature value for one attempt.
pub fn sign(secret: &str, timestamp: i64, body: &str) -> String {
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body.as_bytes());
    format!("v1={}", hex::encode(mac.finalize().into_bytes()))
}

/// Addresses deliveries may never reach outside local development.
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || o[0] == 0
                || o[0] == 100 && (64..128).contains(&o[1])
                || o[0] == 192 && o[1] == 0 && o[2] == 0
                || o[0] == 198 && (18..20).contains(&o[1])
                || o[0] >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || first & 0xfe00 == 0xfc00
                || first & 0xffc0 == 0xfe80
                || first == 0x2001 && v6.segments()[1] == 0xdb8
                || first == 0x64 && v6.segments()[1] == 0xff9b)
        }
    }
}
fn local_host_name(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".internal")
        || host.ends_with(".local")
        || !host.contains('.')
}
/// Check a webhook URL when it is set. Production accepts only HTTPS URLs on
/// public host names or addresses; local development also accepts loopback HTTP.
pub fn check_url(raw: &str, local: bool) -> Result<String> {
    let bad = |message: &str| {
        let mut error = ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_webhook_url",
            message,
            "Use an https:// URL on a public host, without credentials or a fragment.",
        );
        error.details = json!({"url":raw});
        error
    };
    if raw.len() > 2048 {
        return Err(bad("The webhook URL is limited to 2048 characters."));
    }
    let url = url::Url::parse(raw).map_err(|_| bad("The webhook URL is not a valid URL."))?;
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(bad(
            "The webhook URL cannot contain credentials or a fragment.",
        ));
    }
    let Some(host) = url.host() else {
        return Err(bad("The webhook URL needs a host."));
    };
    if local {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(bad("The webhook URL must use HTTP or HTTPS."));
        }
        return Ok(url.to_string());
    }
    if url.scheme() != "https" {
        return Err(bad("Webhook deliveries go only to https:// URLs."));
    }
    let public = match host {
        url::Host::Domain(name) => !local_host_name(name),
        url::Host::Ipv4(ip) => public_ip(IpAddr::V4(ip)),
        url::Host::Ipv6(ip) => public_ip(IpAddr::V6(ip)),
    };
    if !public {
        return Err(bad(
            "Webhook deliveries never go to local host names or private, loopback or reserved addresses.",
        ));
    }
    Ok(url.to_string())
}

fn body_object(raw: &Bytes) -> Result<serde_json::Map<String, Value>> {
    if raw.is_empty() {
        return Ok(Default::default());
    }
    match serde_json::from_slice::<Value>(raw)
        .map_err(|e| ApiError::bad(format!("Request JSON is invalid: {e}")))?
    {
        Value::Object(map) => Ok(map),
        _ => Err(ApiError::bad("Expected a JSON object.")),
    }
}
fn string_list(v: &Value, field: &str) -> Result<Vec<String>> {
    v.as_array()
        .ok_or_else(|| ApiError::bad(format!("`{field}` must be an array of strings.")))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ApiError::bad(format!("`{field}` must contain non-empty strings.")))
        })
        .collect()
}
fn channels(v: &Value) -> Result<Vec<String>> {
    let mut list = string_list(v, "channels")?;
    if list
        .iter()
        .any(|c| !["production", "development"].contains(&c.as_str()))
    {
        return Err(ApiError::bad(
            "channels may contain production and development.",
        ));
    }
    list.sort();
    list.dedup();
    Ok(list)
}
/// Types a subscriber can receive from one app, checked when it subscribes.
fn check_types(types: &[String], author: bool) -> Result<()> {
    TypeFilter::from_list(types.to_vec())?;
    if author {
        return Ok(());
    }
    let hidden: Vec<_> = types
        .iter()
        .filter(|pattern| {
            !PUBLIC_TYPES
                .iter()
                .any(|kind| events::pattern_matches(pattern, kind))
        })
        .cloned()
        .collect();
    if !hidden.is_empty() {
        let mut error = ApiError::new(
            StatusCode::FORBIDDEN,
            "event_type_not_visible",
            format!(
                "Only the app's authors can subscribe to {}.",
                hidden.join(", ")
            ),
            "Subscribe to app.published, release.created, release.promoted or release.withdrawn, or ask an author to invite you.",
        );
        error.details = json!({"not_visible":hidden,"visible":PUBLIC_TYPES});
        return Err(error);
    }
    Ok(())
}

struct Delivery {
    mode: String,
    url: Option<String>,
}
fn delivery(v: &Value, local: bool) -> Result<Delivery> {
    let mode = v["mode"]
        .as_str()
        .ok_or_else(|| ApiError::bad("delivery.mode must be webhook or stream."))?;
    match mode {
        "webhook" => {
            let url = v["url"]
                .as_str()
                .ok_or_else(|| ApiError::bad("delivery.url is required for webhook delivery."))?;
            Ok(Delivery {
                mode: mode.into(),
                url: Some(check_url(url, local)?),
            })
        }
        "stream" => {
            if v.get("url").is_some_and(|u| !u.is_null()) {
                return Err(ApiError::bad("Stream delivery takes no url."));
            }
            Ok(Delivery {
                mode: mode.into(),
                url: None,
            })
        }
        _ => Err(ApiError::bad("delivery.mode must be webhook or stream.")),
    }
}
fn description(v: &Value) -> Result<String> {
    let text = v
        .as_str()
        .ok_or_else(|| ApiError::bad("description must be a string."))?
        .trim()
        .to_owned();
    if text.chars().count() > 200 {
        return Err(ApiError::bad("description is limited to 200 characters."));
    }
    Ok(text)
}

fn refresh_owner(conn: &Connection, who: &Identity) -> Result<()> {
    conn.execute(
        "UPDATE subscriptions SET owner_id=?1,owner_emails=?2 WHERE owner_uuid=?3 AND status!='cancelled'",
        params![who.id, json!(who.verified_emails).to_string(), who.uuid],
    )?;
    Ok(())
}
fn owned(conn: &Connection, id: &str, who: &Identity) -> Result<Subscription> {
    get(conn, id)?
        .filter(|sub| sub.owner_uuid == who.uuid)
        .ok_or_else(missing)
}
fn delivery_counts(conn: &Connection, id: &str) -> Result<Value> {
    let mut counts = json!({"pending":0,"delivered":0,"failed":0});
    let mut statement = conn.prepare(
        "SELECT status,COUNT(*) FROM subscription_deliveries WHERE subscription_id=?1 GROUP BY status",
    )?;
    let rows = statement.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows {
        let (status, count) = row?;
        counts[status] = json!(count);
    }
    Ok(counts)
}

/// `/v1/subscriptions` routes. The route table has already matched the path.
pub async fn handle(
    s: &Shared,
    method: &Method,
    p: &[&str],
    q: &BTreeMap<String, String>,
    headers: &HeaderMap,
    raw: &Bytes,
    who: Option<&Identity>,
) -> Result<Response> {
    let who = who.ok_or_else(ApiError::auth)?;
    {
        let store = s.store.lock().unwrap();
        refresh_owner(&store.connection, who)?;
    }
    if *method == Method::GET {
        let store = s.store.lock().unwrap();
        let conn = &store.connection;
        return Ok(Json(match p {
            ["subscriptions"] => {
                let status = q.get("status").map(String::as_str).unwrap_or("");
                let statuses: &[&str] = match status {
                    "" => &["active", "paused"],
                    "active" => &["active"],
                    "paused" => &["paused"],
                    "cancelled" => &["cancelled"],
                    "all" => &["active", "paused", "cancelled"],
                    _ => {
                        return Err(ApiError::bad(
                            "status must be active, paused, cancelled or all.",
                        ));
                    }
                };
                let mut statement = conn.prepare(&format!(
                    "SELECT {COLUMNS} FROM subscriptions WHERE owner_uuid=?1 ORDER BY created_at,id"
                ))?;
                let items: Vec<Value> = statement
                    .query_map([&who.uuid], from_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
                    .into_iter()
                    .filter(|sub| statuses.contains(&sub.status.as_str()))
                    .map(|sub| sub.view())
                    .collect();
                json!({"items":items})
            }
            ["subscriptions", id] => {
                let sub = owned(conn, id, who)?;
                let mut view = sub.view();
                view["deliveries"] = delivery_counts(conn, id)?;
                view
            }
            ["subscriptions", id, "deliveries"] => {
                owned(conn, id, who)?;
                let status = q.get("status").map(String::as_str).unwrap_or("");
                if !["", "pending", "delivered", "failed"].contains(&status) {
                    return Err(ApiError::bad(
                        "status must be pending, delivered or failed.",
                    ));
                }
                let limit = match q.get("limit").filter(|v| !v.is_empty()) {
                    None => 50,
                    Some(v) => v
                        .parse::<i64>()
                        .ok()
                        .filter(|n| (1..=100).contains(n))
                        .ok_or_else(|| ApiError::bad("limit must be an integer from 1 to 100."))?,
                };
                let mut statement = conn.prepare(
                    "SELECT d.id,e.id,e.type,d.event_seq,d.status,d.attempts,d.next_attempt_ms,d.created_at,d.delivered_at,d.last_attempt_at,d.last_status,d.last_error FROM subscription_deliveries d JOIN events e ON e.seq=d.event_seq WHERE d.subscription_id=?1 AND (?2='' OR d.status=?2) ORDER BY d.created_ms DESC,d.event_seq DESC LIMIT ?3",
                )?;
                let items: Vec<Value> = statement
                    .query_map(params![id, status, limit], |r| {
                        let status: String = r.get(4)?;
                        Ok(json!({
                            "id":r.get::<_,String>(0)?,
                            "event_id":r.get::<_,String>(1)?,
                            "event_type":r.get::<_,String>(2)?,
                            "event_seq":r.get::<_,i64>(3)?,
                            "status":status,
                            "attempts":r.get::<_,i64>(5)?,
                            "next_attempt_at":(status=="pending").then(||ms_to_rfc3339(r.get::<_,i64>(6).unwrap_or_default())),
                            "created_at":r.get::<_,String>(7)?,
                            "delivered_at":r.get::<_,Option<String>>(8)?,
                            "last_attempt_at":r.get::<_,Option<String>>(9)?,
                            "last_status":r.get::<_,Option<i64>>(10)?,
                            "last_error":r.get::<_,Option<String>>(11)?,
                        }))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                json!({"items":items})
            }
            _ => return Err(ApiError::missing()),
        })
        .into_response());
    }

    crate::auth::check_csrf(s, headers)?;
    let key = crate::idempotency_key(headers)?;
    let _guard = s.mutation_gate.lock().await;
    let fingerprint = format!(
        "{}:{}:{}",
        method.as_str(),
        p.join("/"),
        crate::store::hash(raw)
    );
    let created_status = if p == ["subscriptions"] {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    {
        let store = s.store.lock().unwrap();
        store.expire_secret_replays()?;
        if let Some(prior) = store.replay(&who.uuid, key, &fingerprint)? {
            return Ok((
                created_status,
                [("Idempotent-Replayed", "true")],
                Json(prior),
            )
                .into_response());
        }
    }
    let body = body_object(raw)?;
    let local = s.config.local_development();
    let mut store = s.store.lock().unwrap();
    let catalog = store.catalog()?;
    let tx = store
        .connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let now = now();
    let (response, notify) = match (method.as_str(), p) {
        ("POST", ["subscriptions"]) => {
            for field in body.keys() {
                if !["app_id", "types", "channels", "delivery", "description"]
                    .contains(&field.as_str())
                {
                    return Err(ApiError::bad(format!(
                        "Field `{field}` is not part of a subscription."
                    )));
                }
            }
            let count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM subscriptions WHERE owner_uuid=?1 AND status!='cancelled'",
                [&who.uuid],
                |r| r.get(0),
            )?;
            if count >= MAX_SUBSCRIPTIONS {
                let mut error = ApiError::new(
                    StatusCode::CONFLICT,
                    "subscription_limit_reached",
                    format!(
                        "An account can have {MAX_SUBSCRIPTIONS} active or paused subscriptions."
                    ),
                    "Cancel a subscription you no longer need, then retry with a new idempotency key.",
                );
                error.details = json!({"limit":MAX_SUBSCRIPTIONS});
                return Err(error);
            }
            let app_id = match body.get("app_id") {
                None | Some(Value::Null) => None,
                Some(Value::String(id)) => Some(id.clone()),
                Some(_) => return Err(ApiError::bad("app_id must be a string or null.")),
            };
            let author = match &app_id {
                Some(id) => {
                    let app = catalog.apps.get(id).ok_or_else(ApiError::missing)?;
                    if !app.visible(Some(who)) {
                        return Err(ApiError::missing());
                    }
                    app.is_author(Some(who))
                }
                None => true,
            };
            let types = match body.get("types") {
                None | Some(Value::Null) => {
                    if author {
                        vec!["*".to_owned()]
                    } else {
                        PUBLIC_TYPES.iter().map(|t| t.to_string()).collect()
                    }
                }
                Some(v) => string_list(v, "types")?,
            };
            if types.is_empty() {
                return Err(ApiError::bad("types needs at least one event type."));
            }
            check_types(&types, author)?;
            let channels = match body.get("channels") {
                None | Some(Value::Null) => vec![],
                Some(v) => channels(v)?,
            };
            let target = delivery(
                body.get("delivery").ok_or_else(|| {
                    ApiError::bad("delivery is required: {\"mode\":\"webhook\",\"url\":\"https://...\"} or {\"mode\":\"stream\"}.")
                })?,
                local,
            )?;
            let description = match body.get("description") {
                None | Some(Value::Null) => String::new(),
                Some(v) => description(v)?,
            };
            let secret = (target.mode == "webhook").then(new_secret);
            let id = format!("sub_{}", uuid::Uuid::new_v4().simple());
            let cursor = events::head(&tx)?;
            tx.execute(
                &format!("INSERT INTO subscriptions({COLUMNS}) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'active',?11,?12,?13,?13,NULL)"),
                params![
                    id,
                    who.uuid,
                    who.id,
                    json!(who.verified_emails).to_string(),
                    app_id,
                    json!(types).to_string(),
                    json!(channels).to_string(),
                    target.mode,
                    target.url,
                    secret,
                    description,
                    cursor,
                    now
                ],
            )?;
            let sub = get(&tx, &id)?.unwrap();
            let mut response = json!({"subscription":sub.view()});
            if let Some(secret) = secret {
                response["secret"] = json!(secret);
            }
            (response, false)
        }
        ("PATCH", ["subscriptions", id]) => {
            let sub = owned(&tx, id, who)?;
            if sub.status == "cancelled" {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "subscription_cancelled",
                    "A cancelled subscription cannot change.",
                    "Create a new subscription.",
                ));
            }
            for field in body.keys() {
                if !["types", "channels", "delivery", "description", "status"]
                    .contains(&field.as_str())
                {
                    return Err(ApiError::bad(format!(
                        "Field `{field}` cannot be changed. Cancel and create a new subscription to follow another app."
                    )));
                }
            }
            let author = match &sub.app_id {
                Some(app_id) => {
                    let app = catalog.apps.get(app_id).ok_or_else(ApiError::missing)?;
                    if !app.visible(Some(who)) {
                        return Err(ApiError::missing());
                    }
                    app.is_author(Some(who))
                }
                None => true,
            };
            let types = match body.get("types") {
                Some(v) => {
                    let types = string_list(v, "types")?;
                    if types.is_empty() {
                        return Err(ApiError::bad("types needs at least one event type."));
                    }
                    check_types(&types, author)?;
                    types
                }
                None => sub.types.clone(),
            };
            let channels = match body.get("channels") {
                Some(Value::Null) => vec![],
                Some(v) => channels(v)?,
                None => sub.channels.clone(),
            };
            let status = match body.get("status") {
                None => sub.status.clone(),
                Some(v) => match v.as_str() {
                    Some("active" | "paused") => v.as_str().unwrap().to_owned(),
                    _ => {
                        return Err(ApiError::bad(
                            "status must be active or paused. DELETE the subscription to cancel it.",
                        ));
                    }
                },
            };
            let description = match body.get("description") {
                Some(v) => description(v)?,
                None => sub.description.clone(),
            };
            let (mode, url, mut secret, new_secret_value) = match body.get("delivery") {
                Some(v) => {
                    let target = delivery(v, local)?;
                    if target.mode == "webhook" {
                        if sub.delivery == "webhook" {
                            (target.mode, target.url, sub.secret.clone(), None)
                        } else {
                            let secret = new_secret();
                            (target.mode, target.url, Some(secret.clone()), Some(secret))
                        }
                    } else {
                        (target.mode, None, None, None)
                    }
                }
                None => (
                    sub.delivery.clone(),
                    sub.url.clone(),
                    sub.secret.clone(),
                    None,
                ),
            };
            if mode == "stream" {
                secret = None;
                tx.execute(
                    "UPDATE subscription_deliveries SET status='failed',last_error='The subscription changed to stream delivery.' WHERE subscription_id=?1 AND status='pending'",
                    [id],
                )?;
            }
            tx.execute(
                "UPDATE subscriptions SET types=?1,channels=?2,delivery=?3,url=?4,secret=?5,status=?6,description=?7,updated_at=?8 WHERE id=?9",
                params![
                    json!(types).to_string(),
                    json!(channels).to_string(),
                    mode,
                    url,
                    secret,
                    status,
                    description,
                    now,
                    id
                ],
            )?;
            let mut response = json!({"subscription":get(&tx, id)?.unwrap().view()});
            if let Some(secret) = new_secret_value {
                response["secret"] = json!(secret);
            }
            (response, status == "active" && sub.status == "paused")
        }
        ("DELETE", ["subscriptions", id]) => {
            let sub = owned(&tx, id, who)?;
            if sub.status != "cancelled" {
                tx.execute(
                    "UPDATE subscriptions SET status='cancelled',cancelled_at=?1,updated_at=?1 WHERE id=?2",
                    params![now, id],
                )?;
                tx.execute(
                    "UPDATE subscription_deliveries SET status='failed',last_error='The subscription was cancelled.' WHERE subscription_id=?1 AND status='pending'",
                    [id],
                )?;
            }
            (json!({"subscription":get(&tx, id)?.unwrap().view()}), false)
        }
        ("POST", ["subscriptions", id, "secret", "rotate"]) => {
            let sub = owned(&tx, id, who)?;
            if sub.status == "cancelled" || sub.delivery != "webhook" {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "not_a_webhook_subscription",
                    "Only an active or paused webhook subscription has a signing secret.",
                    "Switch delivery to a webhook with PATCH first.",
                ));
            }
            let secret = new_secret();
            tx.execute(
                "UPDATE subscriptions SET secret=?1,updated_at=?2 WHERE id=?3",
                params![secret, now, id],
            )?;
            (json!({"secret":secret}), false)
        }
        ("POST", ["subscriptions", id, "ping"]) => {
            let sub = owned(&tx, id, who)?;
            if sub.status != "active" || sub.delivery != "webhook" {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "not_a_webhook_subscription",
                    "Only an active webhook subscription can be pinged.",
                    "Resume the subscription or switch it to webhook delivery first.",
                ));
            }
            let event_id = new_id();
            tx.execute(
                "INSERT INTO events(id,type,app_id,actor_uuid,visibility,data,idempotency_key,occurred_at) VALUES(?1,'ping',?2,?3,'direct',?4,?5,?6)",
                params![
                    event_id,
                    sub.app_id,
                    who.uuid,
                    json!({"subscription_id":sub.id}).to_string(),
                    key,
                    now
                ],
            )?;
            let delivery_id = queue_delivery(&tx, id, tx.last_insert_rowid())?;
            (
                json!({"delivery_id":delivery_id,"event_id":event_id,"status":"pending"}),
                true,
            )
        }
        _ => return Err(ApiError::missing()),
    };
    tx.execute(
        "INSERT INTO idempotency(actor,key,fingerprint,response,created_at) VALUES(?1,?2,?3,?4,?5)",
        params![who.uuid, key, fingerprint, response.to_string(), now],
    )?;
    tx.commit()?;
    if notify {
        store.notify_events();
    }
    drop(store);
    let _ = catalog;
    Ok((
        created_status,
        [("Idempotent-Replayed", "false")],
        Json(response),
    )
        .into_response())
}

struct Due {
    id: String,
    subscription_id: String,
    seq: i64,
    attempts: i64,
}
/// Claim due deliveries of active subscriptions and expire old ones.
fn claim(s: &Shared) -> Result<Vec<Due>> {
    let store = s.store.lock().unwrap();
    let conn = &store.connection;
    let now = now_ms();
    conn.execute(
        "UPDATE subscription_deliveries SET status='failed',last_error=COALESCE(last_error||' ','')||'No successful attempt within 72 hours of the event.' WHERE status='pending' AND created_ms<?1",
        [now - RETRY_WINDOW_HOURS * 3600 * 1000],
    )?;
    let mut statement = conn.prepare(
        "SELECT d.id,d.subscription_id,d.event_seq,d.attempts FROM subscription_deliveries d JOIN subscriptions s ON s.id=d.subscription_id WHERE d.status='pending' AND d.next_attempt_ms<=?1 AND s.status='active' AND s.delivery='webhook' ORDER BY d.next_attempt_ms LIMIT ?2",
    )?;
    let due: Vec<Due> = statement
        .query_map(params![now, WORKER_BATCH as i64], |r| {
            Ok(Due {
                id: r.get(0)?,
                subscription_id: r.get(1)?,
                seq: r.get(2)?,
                attempts: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    for item in &due {
        // A lease, so a slow attempt is not sent twice by the next tick.
        conn.execute(
            "UPDATE subscription_deliveries SET next_attempt_ms=?1 WHERE id=?2",
            params![now + CLAIM_MS, item.id],
        )?;
    }
    Ok(due)
}

/// Resolve and check every address a delivery would connect to.
async fn resolve(url: &url::Url, local: bool) -> std::result::Result<Vec<SocketAddr>, String> {
    let host = url.host_str().ok_or("The webhook URL has no host.")?;
    let port = url.port_or_known_default().unwrap_or(443);
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let addresses: Vec<SocketAddr> = match bare.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        Err(_) => tokio::net::lookup_host((bare, port))
            .await
            .map_err(|_| format!("Could not resolve {host}."))?
            .collect(),
    };
    if addresses.is_empty() {
        return Err(format!("{host} has no addresses."));
    }
    if !local && addresses.iter().any(|a| !public_ip(a.ip())) {
        return Err(
            "The webhook host resolves to a private, loopback or reserved address; deliveries go only to public HTTPS endpoints."
                .into(),
        );
    }
    Ok(addresses)
}

async fn attempt(s: &Shared, due: &Due) -> std::result::Result<u16, (Option<u16>, String)> {
    let (sub, event) = {
        let store = s.store.lock().unwrap();
        let sub = get(&store.connection, &due.subscription_id).map_err(|e| (None, e.message))?;
        let event = events::get(&store.connection, due.seq).map_err(|e| (None, e.message))?;
        (sub, event)
    };
    let (Some(sub), Some(event)) = (sub, event) else {
        return Err((
            None,
            "The subscription or its event no longer exists.".into(),
        ));
    };
    let (Some(raw_url), Some(secret)) = (sub.url.as_deref(), sub.secret.as_deref()) else {
        return Err((
            None,
            "The subscription has no webhook URL or secret.".into(),
        ));
    };
    let url = url::Url::parse(raw_url).map_err(|_| (None, "The webhook URL is invalid.".into()))?;
    let local = s.config.local_development();
    let addresses = resolve(&url, local).await.map_err(|e| (None, e))?;
    let body = json!({
        "actor_uuid":event.actor_uuid,
        "app_id":event.app_id,
        "data":event.data,
        "event_id":event.id,
        "occurred_at":event.occurred_at,
        "seq":event.seq,
        "subscription_id":sub.id,
        "type":event.kind,
    })
    .to_string();
    let timestamp = chrono::Utc::now().timestamp();
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(DELIVERY_TIMEOUT_SECONDS))
        .no_proxy()
        .user_agent("SiliconApps-Webhooks/1");
    if let Some(url::Host::Domain(domain)) = url.host() {
        builder = builder.resolve_to_addrs(domain, &addresses);
    }
    let client = builder
        .build()
        .map_err(|e| (None, format!("Could not prepare the request: {e}")))?;
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("X-Apps-Event-Id", &event.id)
        .header("X-Apps-Event-Type", &event.kind)
        .header("X-Apps-Delivery-Id", &due.id)
        .header("X-Apps-Subscription-Id", &sub.id)
        .header("X-Apps-Timestamp", timestamp.to_string())
        .header("X-Apps-Signature", sign(secret, timestamp, &body))
        .body(body)
        .send()
        .await
        .map_err(|e| {
            (
                None,
                if e.is_timeout() {
                    format!("No response within {DELIVERY_TIMEOUT_SECONDS} seconds.")
                } else {
                    "Could not connect to the webhook URL.".into()
                },
            )
        })?;
    let status = response.status();
    if status.is_success() {
        return Ok(status.as_u16());
    }
    let text: String = response
        .text()
        .await
        .unwrap_or_default()
        .chars()
        .take(300)
        .collect();
    Err((
        Some(status.as_u16()),
        format!(
            "HTTP {status}: the endpoint must answer with a 2xx status within {DELIVERY_TIMEOUT_SECONDS} seconds{}{}",
            if status.is_redirection() {
                " (redirects are not followed)"
            } else {
                ""
            },
            if text.is_empty() {
                ".".to_owned()
            } else {
                format!(". Response body: {text}")
            }
        ),
    ))
}

fn record(s: &Shared, due: &Due, result: std::result::Result<u16, (Option<u16>, String)>) {
    let store = s.store.lock().unwrap();
    let now = now_ms();
    let at = ms_to_rfc3339(now);
    let _ = match result {
        Ok(status) => store.connection.execute(
            "UPDATE subscription_deliveries SET status='delivered',attempts=attempts+1,delivered_at=?1,last_attempt_at=?1,last_status=?2,last_error=NULL WHERE id=?3",
            params![at, status, due.id],
        ),
        Err((status, error)) => {
            let wait = RETRY_SECONDS[(due.attempts as usize).min(RETRY_SECONDS.len() - 1)];
            store.connection.execute(
                "UPDATE subscription_deliveries SET attempts=attempts+1,last_attempt_at=?1,last_status=?2,last_error=?3,next_attempt_ms=?4 WHERE id=?5 AND status='pending'",
                params![at, status, error, now + wait as i64 * 1000, due.id],
            )
        }
    };
}

/// Send due webhook deliveries, up to 16 at a time, about once a second and
/// immediately after new events.
pub async fn delivery_worker(s: Shared) {
    let mut notify = s.store.lock().unwrap().events.subscribe();
    loop {
        match claim(&s) {
            Ok(due) => {
                let mut tasks = tokio::task::JoinSet::new();
                for item in due {
                    let s = s.clone();
                    tasks.spawn(async move {
                        let result = attempt(&s, &item).await;
                        record(&s, &item, result);
                    });
                }
                while tasks.join_next().await.is_some() {}
            }
            Err(error) => eprintln!("webhook delivery worker: {}", error.message),
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => {},
            changed = notify.changed() => {
                if changed.is_err() {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            },
        }
    }
}
