//! The append-only event log and the feeds built on it.
//!
//! Catalog changes record history entries on each app. Whenever a catalog
//! document is saved, the history entries it gained are appended to `events`
//! in the same SQLite transaction, together with a webhook delivery for every
//! subscription that may receive them. A change that rolls back leaves no
//! event, and a committed change always has its events.
//!
//! Package validation progress is not catalog state, so it is appended in its
//! own small transaction as each step finishes.

use crate::{
    Shared,
    error::{ApiError, Result},
    model::*,
    subscriptions,
};
use axum::{
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse, Response,
        sse::{Event as SseEvent, KeepAlive, Sse},
    },
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    time::Duration,
};

/// Every event type the service records, grouped by what it is about.
pub const EVENT_TYPES: &[&str] = &[
    "app.created",
    "app.imported_from_accounts",
    "app.id_migrated",
    "app.details_changed",
    "app.access_changed",
    "app.published",
    "app.secret_rotated",
    "app.installed",
    "media.uploaded",
    "accounts.webhook_changed",
    "package.validation_started",
    "package.validation_step",
    "package.accepted",
    "package.validation_failed",
    "release.created",
    "release.promoted",
    "release.withdrawn",
    "author.invited",
    "author.joined",
    "author.invite_declined",
    "author.invite_cancelled",
    "author.left",
    "author.removed",
    "author.admin_transferred",
    "review.updated",
    "review.removed",
    "ping",
];
/// Events anyone who can see the app may receive. Everything else is for the
/// app's authors, plus the invited or removed account for its own invitation
/// or removal.
pub const PUBLIC_TYPES: &[&str] = &[
    "app.published",
    "release.created",
    "release.promoted",
    "release.withdrawn",
];
/// Streams heartbeat with an SSE comment this often.
pub const HEARTBEAT_SECONDS: u64 = 15;
/// A stream ends after this long; reconnect with `Last-Event-ID`.
pub const STREAM_MAX_SECONDS: u64 = 30 * 60;
const STEP_OUTPUT_LIMIT: usize = 4096;

#[derive(Clone, Debug)]
pub struct NewEvent {
    pub id: String,
    pub kind: String,
    pub app_id: Option<String>,
    pub actor_uuid: String,
    pub visibility: &'static str,
    pub recipient_uuids: Vec<String>,
    pub recipient_emails: Vec<String>,
    pub data: Value,
    pub idempotency_key: Option<String>,
    pub occurred_at: String,
}
impl NewEvent {
    pub fn from_history(app_id: &str, entry: &History) -> Self {
        let mut recipient_uuids = vec![];
        let mut recipient_emails = vec![];
        match entry.kind.as_str() {
            "author.invited" => {
                if let Some(uuid) = entry.data["account_uuid"].as_str() {
                    recipient_uuids.push(uuid.to_owned());
                }
                if let Some(to) = entry.data["to"].as_str()
                    && to.contains('@')
                {
                    recipient_emails.push(to.to_lowercase());
                }
            }
            "author.removed" => {
                if let Some(uuid) = entry.data["uuid"].as_str() {
                    recipient_uuids.push(uuid.to_owned());
                }
            }
            _ => {}
        }
        Self {
            id: entry.id.clone(),
            kind: entry.kind.clone(),
            app_id: Some(app_id.to_owned()),
            actor_uuid: entry.actor_uuid.clone(),
            visibility: if PUBLIC_TYPES.contains(&entry.kind.as_str()) {
                "public"
            } else {
                "authors"
            },
            recipient_uuids,
            recipient_emails,
            data: entry.data.clone(),
            idempotency_key: entry.idempotency_key.clone(),
            occurred_at: entry.at.clone(),
        }
    }
    /// An author-only event that is not catalog history, such as a validation step.
    pub fn progress(app_id: &str, kind: &str, actor: &str, data: Value, key: &str) -> Self {
        Self {
            id: new_id(),
            kind: kind.into(),
            app_id: Some(app_id.into()),
            actor_uuid: actor.into(),
            visibility: "authors",
            recipient_uuids: vec![],
            recipient_emails: vec![],
            data,
            idempotency_key: Some(key.into()),
            occurred_at: now(),
        }
    }
}

/// History length per app before a change, to find the entries it adds.
pub fn marks(c: &Catalog) -> BTreeMap<String, usize> {
    c.apps
        .iter()
        .map(|(id, app)| (id.clone(), app.history.len()))
        .collect()
}
pub fn new_since(c: &Catalog, marks: &BTreeMap<String, usize>) -> Vec<NewEvent> {
    let mut events: Vec<_> = c
        .apps
        .iter()
        .flat_map(|(id, app)| {
            app.history
                .iter()
                .skip(*marks.get(id).unwrap_or(&0))
                .map(move |entry| NewEvent::from_history(id, entry))
        })
        .collect();
    events.sort_by(|a, b| a.occurred_at.cmp(&b.occurred_at));
    events
}

/// Append events and queue webhook deliveries inside the caller's transaction.
pub fn append(conn: &Connection, c: &Catalog, events: &[NewEvent]) -> Result<usize> {
    if events.is_empty() {
        return Ok(0);
    }
    let targets = subscriptions::webhook_targets(conn)?;
    for event in events {
        conn.execute(
            "INSERT INTO events(id,type,app_id,actor_uuid,visibility,recipient_uuids,recipient_emails,data,idempotency_key,occurred_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                event.id,
                event.kind,
                event.app_id,
                event.actor_uuid,
                event.visibility,
                json!(event.recipient_uuids).to_string(),
                json!(event.recipient_emails).to_string(),
                event.data.to_string(),
                event.idempotency_key,
                event.occurred_at
            ],
        )?;
        let stored = Event {
            seq: conn.last_insert_rowid(),
            id: event.id.clone(),
            kind: event.kind.clone(),
            app_id: event.app_id.clone(),
            actor_uuid: event.actor_uuid.clone(),
            visibility: event.visibility.into(),
            recipient_uuids: event.recipient_uuids.clone(),
            recipient_emails: event.recipient_emails.clone(),
            data: event.data.clone(),
            occurred_at: event.occurred_at.clone(),
        };
        for subscription in &targets {
            if subscription.matches(&stored, c) {
                subscriptions::queue_delivery(conn, &subscription.id, stored.seq)?;
            }
        }
    }
    Ok(events.len())
}

/// Record validation progress for an upload's authors. Progress is advisory:
/// failing to record it never changes the upload's outcome.
pub fn progress(s: &Shared, app_id: &str, actor: &str, key: &str, kind: &str, data: Value) {
    let event = NewEvent::progress(app_id, kind, actor, data, key);
    let mut store = s.store.lock().unwrap();
    let result = (|| -> Result<()> {
        let c = store.catalog()?;
        let tx = store.connection.transaction()?;
        append(&tx, &c, &[event])?;
        tx.commit()?;
        Ok(())
    })();
    match result {
        Ok(()) => store.notify_events(),
        Err(error) => eprintln!("validation progress was not recorded: {}", error.message),
    }
}
/// Bounded command output for a progress step; the full output is kept on the package.
pub fn step_output(value: &str) -> String {
    if value.len() <= STEP_OUTPUT_LIMIT {
        return value.into();
    }
    let mut end = STEP_OUTPUT_LIMIT;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n[truncated; the package record keeps the full output]",
        &value[..end]
    )
}

#[derive(Clone, Debug)]
pub struct Event {
    pub seq: i64,
    pub id: String,
    pub kind: String,
    pub app_id: Option<String>,
    pub actor_uuid: String,
    pub visibility: String,
    pub recipient_uuids: Vec<String>,
    pub recipient_emails: Vec<String>,
    pub data: Value,
    pub occurred_at: String,
}
impl Event {
    pub fn view(&self) -> Value {
        json!({"seq":self.seq,"id":self.id,"type":self.kind,"app_id":self.app_id,"actor_uuid":self.actor_uuid,"occurred_at":self.occurred_at,"data":self.data})
    }
    fn targeted(&self, who: &Identity) -> bool {
        self.recipient_uuids.contains(&who.uuid)
            || self.recipient_emails.iter().any(|email| {
                who.verified_emails
                    .iter()
                    .any(|verified| verified.eq_ignore_ascii_case(email))
            })
    }
    pub fn channel(&self) -> Option<&str> {
        self.data["channel"].as_str()
    }
}
pub const EVENT_COLUMNS: &str =
    "seq,id,type,app_id,actor_uuid,visibility,recipient_uuids,recipient_emails,data,occurred_at";
pub fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Event> {
    let list = |i: usize| -> rusqlite::Result<Vec<String>> {
        Ok(serde_json::from_str(&r.get::<_, String>(i)?).unwrap_or_default())
    };
    Ok(Event {
        seq: r.get(0)?,
        id: r.get(1)?,
        kind: r.get(2)?,
        app_id: r.get(3)?,
        actor_uuid: r.get(4)?,
        visibility: r.get(5)?,
        recipient_uuids: list(6)?,
        recipient_emails: list(7)?,
        data: serde_json::from_str(&r.get::<_, String>(8)?).unwrap_or(Value::Null),
        occurred_at: r.get(9)?,
    })
}
pub fn head(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COALESCE(MAX(seq),0) FROM events", [], |r| r.get(0))?)
}
pub fn get(conn: &Connection, seq: i64) -> Result<Option<Event>> {
    Ok(conn
        .query_row(
            &format!("SELECT {EVENT_COLUMNS} FROM events WHERE seq=?1 AND NOT EXISTS(SELECT 1 FROM event_identity_retirements r WHERE r.event_seq=events.seq)"),
            [seq],
            from_row,
        )
        .optional()?)
}
fn read_after(
    conn: &Connection,
    after: i64,
    app_id: Option<&str>,
    limit: usize,
) -> Result<Vec<Event>> {
    let mut statement = conn.prepare(&format!(
        "SELECT {EVENT_COLUMNS} FROM events WHERE seq>?1 AND (?2 IS NULL OR app_id=?2) AND NOT EXISTS(SELECT 1 FROM event_identity_retirements r WHERE r.event_seq=events.seq) ORDER BY seq LIMIT ?3"
    ))?;
    let rows = statement.query_map(params![after, app_id, limit as i64], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Whether an account's own feed includes an event: invitations and removals
/// addressed to it, everything about apps it authors, and public events of
/// apps it installed and can still see.
pub fn account_can_see(event: &Event, who: &Identity, c: &Catalog) -> bool {
    if event.visibility == "direct" {
        return false;
    }
    if event.targeted(who) {
        return true;
    }
    let Some(app) = event.app_id.as_ref().and_then(|id| c.apps.get(id)) else {
        return false;
    };
    if app.is_author(Some(who)) {
        return true;
    }
    event.visibility == "public"
        && app.visible(Some(who))
        && app
            .history
            .iter()
            .any(|entry| entry.kind == "app.installed" && entry.actor_uuid == who.uuid)
}
/// Whether a subscriber may receive an event about one app.
pub fn app_subscriber_can_see(event: &Event, app_id: &str, who: &Identity, c: &Catalog) -> bool {
    if event.visibility == "direct" || event.app_id.as_deref() != Some(app_id) {
        return false;
    }
    c.apps.get(app_id).is_some_and(|app| {
        app.is_author(Some(who)) || event.visibility == "public" && app.visible(Some(who))
    })
}

/// `?types=` and subscription type lists: exact types, `group.*` or `*`.
#[derive(Clone, Debug, Default)]
pub struct TypeFilter(pub Vec<String>);
impl TypeFilter {
    pub fn parse(raw: Option<&str>) -> Result<Self> {
        let items: Vec<String> = raw
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
            .collect();
        Self::from_list(items)
    }
    pub fn from_list(items: Vec<String>) -> Result<Self> {
        if items.len() > 50 {
            return Err(ApiError::bad("List at most 50 event types."));
        }
        let unknown: Vec<_> = items
            .iter()
            .filter(|item| !known_pattern(item))
            .cloned()
            .collect();
        if !unknown.is_empty() {
            let mut error = ApiError::new(
                StatusCode::BAD_REQUEST,
                "unknown_event_type",
                format!("Unknown event type: {}.", unknown.join(", ")),
                "Use exact event types, a group such as release.*, or * for everything.",
            );
            error.details = json!({"unknown":unknown,"known":EVENT_TYPES});
            return Err(error);
        }
        Ok(Self(items))
    }
    pub fn allows(&self, kind: &str) -> bool {
        self.0.is_empty() || self.0.iter().any(|pattern| pattern_matches(pattern, kind))
    }
}
pub fn pattern_matches(pattern: &str, kind: &str) -> bool {
    pattern == "*"
        || pattern == kind
        || pattern
            .strip_suffix(".*")
            .is_some_and(|group| kind.split('.').next() == Some(group))
}
fn known_pattern(pattern: &str) -> bool {
    EVENT_TYPES
        .iter()
        .any(|kind| pattern_matches(pattern, kind))
}

pub enum Feed {
    App(String),
    Account,
    Subscription(String),
}
struct Batch {
    events: Vec<Event>,
    scanned_to: i64,
    open: bool,
}
fn collect(
    s: &Shared,
    who: &Identity,
    feed: &Feed,
    types: &TypeFilter,
    after: i64,
    limit: usize,
) -> Result<Batch> {
    let store = s.store.lock().unwrap();
    let c = store.catalog()?;
    let subscription = match feed {
        Feed::Subscription(id) => match subscriptions::get(&store.connection, id)? {
            Some(sub) if sub.owner_uuid == who.uuid && sub.status == "active" => Some(sub),
            _ => {
                return Ok(Batch {
                    events: vec![],
                    scanned_to: after,
                    open: false,
                });
            }
        },
        _ => None,
    };
    let app_filter = match feed {
        Feed::App(id) => {
            if !c.apps.get(id).is_some_and(|app| app.is_author(Some(who))) {
                return Ok(Batch {
                    events: vec![],
                    scanned_to: after,
                    open: false,
                });
            }
            Some(id.as_str())
        }
        Feed::Subscription(_) => subscription.as_ref().and_then(|s| s.app_id.as_deref()),
        Feed::Account => None,
    };
    let mut events = vec![];
    let mut cursor = after;
    let mut scanned = 0;
    loop {
        let rows = read_after(&store.connection, cursor, app_filter, 500)?;
        if rows.is_empty() {
            // Retired events and events outside this app still consume sequence
            // numbers. Advance past them so replay cannot loop on an empty page.
            cursor = cursor.max(head(&store.connection)?);
            break;
        }
        scanned += rows.len();
        for event in rows {
            cursor = event.seq;
            let visible = match (feed, &subscription) {
                (Feed::App(id), _) => {
                    event.app_id.as_deref() == Some(id) && event.visibility != "direct"
                }
                (Feed::Account, _) => account_can_see(&event, who, &c),
                (Feed::Subscription(_), Some(sub)) => sub.matches(&event, &c),
                _ => false,
            };
            if visible && types.allows(&event.kind) {
                events.push(event);
                if events.len() >= limit {
                    return Ok(Batch {
                        events,
                        scanned_to: cursor,
                        open: true,
                    });
                }
            }
        }
        if scanned >= 5000 {
            break;
        }
    }
    Ok(Batch {
        events,
        scanned_to: cursor,
        open: true,
    })
}

/// Check the caller may open a feed before any event is read.
pub fn authorize(s: &Shared, who: Option<&Identity>, feed: &Feed) -> Result<Identity> {
    let who = who.ok_or_else(ApiError::auth)?;
    let store = s.store.lock().unwrap();
    match feed {
        Feed::App(id) => {
            let app = store.app(id)?;
            if !app.visible(Some(who)) {
                return Err(ApiError::missing());
            }
            if !app.is_author(Some(who)) {
                return Err(ApiError::forbidden());
            }
        }
        Feed::Subscription(id) => {
            let sub = subscriptions::get(&store.connection, id)?
                .filter(|sub| sub.owner_uuid == who.uuid)
                .ok_or_else(subscriptions::missing)?;
            if sub.status != "active" {
                let mut error = ApiError::new(
                    StatusCode::CONFLICT,
                    "subscription_inactive",
                    format!("This subscription is {}.", sub.status),
                    "Resume a paused subscription with PATCH {\"status\":\"active\"}, or create a new one.",
                );
                error.details = json!({"status":sub.status});
                return Err(error);
            }
        }
        Feed::Account => {}
    }
    Ok(who.clone())
}

fn cursor_param(q: &BTreeMap<String, String>, headers: Option<&HeaderMap>) -> Result<Option<i64>> {
    let header = headers
        .and_then(|h| h.get("Last-Event-ID"))
        .map(|v| v.to_str().unwrap_or("x").trim().to_owned());
    let raw = header
        .or_else(|| q.get("last_event_id").cloned())
        .or_else(|| q.get("after").cloned())
        .filter(|v| !v.is_empty());
    raw.map(|v| {
        v.parse::<i64>().ok().filter(|n| *n >= 0).ok_or_else(|| {
            ApiError::bad(
                "Last-Event-ID, last_event_id and after must be a non-negative event seq.",
            )
        })
    })
    .transpose()
}

/// `GET /v1/events` and `/v1/apps/{app_id}/events`: the same feed as JSON pages.
pub fn list(s: &Shared, who: &Identity, feed: Feed, q: &BTreeMap<String, String>) -> Result<Value> {
    let types = TypeFilter::parse(q.get("types").map(String::as_str))?;
    let limit = match q.get("limit").filter(|v| !v.is_empty()) {
        None => 100,
        Some(v) => v
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=500).contains(n))
            .ok_or_else(|| ApiError::bad("limit must be an integer from 1 to 500."))?,
    };
    let requested = cursor_param(q, None)?;
    let after = match (&feed, requested) {
        (Feed::Subscription(id), Some(seq)) => {
            subscriptions::advance_cursor(s, id, seq);
            seq
        }
        (Feed::Subscription(id), None) => {
            subscriptions::get(&s.store.lock().unwrap().connection, id)?
                .map(|sub| sub.cursor)
                .unwrap_or(0)
        }
        (_, requested) => requested.unwrap_or(0),
    };
    let batch = collect(s, who, &feed, &types, after, limit)?;
    let head = head(&s.store.lock().unwrap().connection)?;
    Ok(json!({
        "items":batch.events.iter().map(Event::view).collect::<Vec<_>>(),
        "next_after":batch.scanned_to,
        "has_more":batch.scanned_to < head,
        "cursor":head
    }))
}

struct StreamState {
    s: Shared,
    who: Identity,
    feed: Feed,
    types: TypeFilter,
    cursor: i64,
    queue: VecDeque<Event>,
    ready: Option<String>,
    notify: tokio::sync::watch::Receiver<u64>,
    notify_open: bool,
    deadline: tokio::time::Instant,
    _guard: crate::ratelimit::StreamGuard,
}

/// `GET /v1/events/stream` and `/v1/apps/{app_id}/events/stream`.
pub fn stream(
    s: &Shared,
    who: Identity,
    feed: Feed,
    q: &BTreeMap<String, String>,
    headers: &HeaderMap,
    guard: crate::ratelimit::StreamGuard,
) -> Result<Response> {
    let types = TypeFilter::parse(q.get("types").map(String::as_str))?;
    let requested = cursor_param(q, Some(headers))?;
    // A subscription remembers the last event its reader acknowledged by
    // reconnecting with Last-Event-ID, so delivery is at least once.
    if let (Feed::Subscription(id), Some(seq)) = (&feed, requested) {
        subscriptions::advance_cursor(s, id, seq);
    }
    let (start, notify) = {
        let store = s.store.lock().unwrap();
        let start = match (requested, &feed) {
            (Some(seq), _) => seq,
            (None, Feed::Subscription(id)) => subscriptions::get(&store.connection, id)?
                .map(|sub| sub.cursor)
                .unwrap_or(0),
            (None, _) => head(&store.connection)?,
        };
        (start, store.events.subscribe())
    };
    let state = StreamState {
        s: s.clone(),
        who,
        feed,
        types,
        cursor: start,
        queue: VecDeque::new(),
        ready: Some(format!("ready cursor={start}")),
        notify,
        notify_open: true,
        deadline: tokio::time::Instant::now() + Duration::from_secs(STREAM_MAX_SECONDS),
        _guard: guard,
    };
    let events = futures_util::stream::unfold(state, |mut st| async move {
        if let Some(comment) = st.ready.take() {
            let first = SseEvent::default()
                .retry(Duration::from_secs(3))
                .comment(comment);
            return Some((Ok::<_, std::convert::Infallible>(first), st));
        }
        loop {
            if let Some(event) = st.queue.pop_front() {
                let sse = SseEvent::default()
                    .id(event.seq.to_string())
                    .event(event.kind.as_str())
                    .data(event.view().to_string());
                return Some((Ok(sse), st));
            }
            if tokio::time::Instant::now() >= st.deadline {
                return None;
            }
            let batch = match collect(&st.s, &st.who, &st.feed, &st.types, st.cursor, 100) {
                Ok(batch) => batch,
                Err(_) => return None,
            };
            if !batch.open {
                return None;
            }
            st.cursor = batch.scanned_to;
            if batch.events.is_empty() {
                let wait = tokio::time::sleep(Duration::from_secs(5));
                let end = tokio::time::sleep_until(st.deadline);
                if st.notify_open {
                    tokio::select! {
                        changed = st.notify.changed() => st.notify_open = changed.is_ok(),
                        _ = wait => {},
                        _ = end => {},
                    }
                } else {
                    tokio::select! { _ = wait => {}, _ = end => {} }
                }
            } else {
                st.queue.extend(batch.events);
            }
        }
    });
    let mut response = Sse::new(events)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(HEARTBEAT_SECONDS))
                .text("heartbeat"),
        )
        .into_response();
    response
        .headers_mut()
        .insert("X-Accel-Buffering", "no".parse().unwrap());
    Ok(response)
}
