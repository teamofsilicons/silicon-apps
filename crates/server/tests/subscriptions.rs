//! Subscriptions: create, idempotency, pause, resume, cancel, signed webhook
//! deliveries with retries, holds and expiry, streams that remember their
//! place, and webhook URL safety.
mod common;
use axum::{
    Router,
    body::Bytes,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use common::*;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use silicon_apps_server::{start_background, subscriptions::check_url};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[tokio::test]
async fn subscriptions_are_created_once_paused_resumed_and_cancelled() {
    let (_dir, state, app) = fresh();
    seed_app(&state, "alice", "briefcase", &[], "linux-x86_64", true);
    seed_app(&state, "alice", "hidden-app", &[], "linux-x86_64", true);
    mutate(
        &state,
        "alice",
        "PUT",
        "apps/hidden-app/access",
        "make-private",
        json!({"visibility":"private"}),
        Default::default(),
    );
    let create = json!({"app_id":"briefcase","types":["release.promoted"],"delivery":{"mode":"webhook","url":"http://127.0.0.1:9/hooks/apps"},"description":"production releases"});
    let first = send(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(BOB),
        Some(create.clone()),
        Some("subscribe-briefcase"),
        &[],
    )
    .await;
    assert_eq!(first.status, 201, "{}", first.body);
    assert_eq!(first.headers["idempotent-replayed"], "false");
    let secret = first.body["secret"].as_str().unwrap().to_owned();
    assert!(secret.starts_with("whsec_") && secret.len() > 40);
    let sub = first.body["subscription"].clone();
    let id = sub["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("sub_"));
    assert_eq!(sub["status"], "active");
    assert_eq!(
        sub["delivery"],
        json!({"mode":"webhook","url":"http://127.0.0.1:9/hooks/apps"})
    );
    assert_eq!(
        sub["stream_url"],
        format!("/v1/events/stream?subscription={id}")
    );
    // Same key and body: the same result, not a second subscription.
    let replay = send(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(BOB),
        Some(create.clone()),
        Some("subscribe-briefcase"),
        &[],
    )
    .await;
    assert_eq!(replay.status, 201);
    assert_eq!(replay.headers["idempotent-replayed"], "true");
    assert_eq!(replay.body["secret"], secret.as_str());
    let conflict = send(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(BOB),
        Some(json!({"delivery":{"mode":"stream"}})),
        Some("subscribe-briefcase"),
        &[],
    )
    .await;
    assert_eq!(conflict.status, 409);
    // Defaults: the account feed with stream delivery and no secret.
    let feed = send(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(BOB),
        Some(json!({"delivery":{"mode":"stream"}})),
        Some("subscribe-feed"),
        &[],
    )
    .await;
    assert_eq!(feed.status, 201);
    assert!(feed.body["secret"].is_null());
    assert_eq!(feed.body["subscription"]["types"], json!(["*"]));
    assert_eq!(feed.body["subscription"]["app_id"], Value::Null);
    // A non-author's default types are what it may see.
    let defaults = send(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(CAROL),
        Some(json!({"app_id":"briefcase","delivery":{"mode":"stream"}})),
        Some("carol-defaults"),
        &[],
    )
    .await;
    assert_eq!(
        defaults.body["subscription"]["types"],
        json!([
            "app.published",
            "release.created",
            "release.promoted",
            "release.withdrawn"
        ])
    );

    for (body, status, code) in [
        (
            json!({"app_id":"briefcase","types":["package.*"],"delivery":{"mode":"stream"}}),
            403,
            "event_type_not_visible",
        ),
        (
            json!({"app_id":"hidden-app","delivery":{"mode":"stream"}}),
            404,
            "not_found",
        ),
        (
            json!({"app_id":"no-such-app","delivery":{"mode":"stream"}}),
            404,
            "not_found",
        ),
        (
            json!({"types":["release.launched"],"delivery":{"mode":"stream"}}),
            400,
            "unknown_event_type",
        ),
        (json!({"app_id":"briefcase"}), 400, "invalid_input"),
        (
            json!({"delivery":{"mode":"webhook","url":"ftp://example.com/x"}}),
            400,
            "invalid_webhook_url",
        ),
        (
            json!({"delivery":{"mode":"carrier-pigeon"}}),
            400,
            "invalid_input",
        ),
        (
            json!({"delivery":{"mode":"stream"},"channels":["beta"]}),
            400,
            "invalid_input",
        ),
        (
            json!({"delivery":{"mode":"stream"},"owner":"alice"}),
            400,
            "invalid_input",
        ),
    ] {
        let r = send(
            &app,
            "POST",
            "/v1/subscriptions",
            Some(BOB),
            Some(body.clone()),
            Some(&format!("bad-{}", rand_key())),
            &[],
        )
        .await;
        assert_eq!(r.status, status, "{body}: {}", r.body);
        assert_eq!(r.body["error"]["code"], code, "{body}");
    }
    let r = send(
        &app,
        "POST",
        "/v1/subscriptions",
        None,
        Some(create.clone()),
        Some("anonymous-key"),
        &[],
    )
    .await;
    assert_eq!(r.status, 401);
    let r = send(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(BOB),
        Some(create.clone()),
        None,
        &[],
    )
    .await;
    assert_eq!(r.status, 400);

    // Only the owner can see or change it.
    let r = send(
        &app,
        "GET",
        &format!("/v1/subscriptions/{id}"),
        Some(ALICE),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(r.status, 404);
    assert_eq!(r.body["error"]["code"], "subscription_not_found");
    let r = send(
        &app,
        "GET",
        &format!("/v1/subscriptions/{id}"),
        Some(BOB),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(
        r.body["deliveries"],
        json!({"pending":0,"delivered":0,"failed":0})
    );
    assert!(r.body.get("secret").is_none() && !r.body.to_string().contains(&secret));

    let paused = patch(&app, &id, json!({"status":"paused"}), "pause-briefcase").await;
    assert_eq!(paused.body["subscription"]["status"], "paused");
    let resumed = patch(
        &app,
        &id,
        json!({"status":"active","channels":["production"]}),
        "resume-briefcase",
    )
    .await;
    assert_eq!(resumed.body["subscription"]["status"], "active");
    assert_eq!(
        resumed.body["subscription"]["channels"],
        json!(["production"])
    );
    assert!(
        resumed.body["secret"].is_null(),
        "changing filters keeps the secret"
    );
    let r = patch(
        &app,
        &id,
        json!({"app_id":"hidden-app"}),
        "move-subscription",
    )
    .await;
    assert_eq!(r.status, 400);
    let r = patch(&app, &id, json!({"status":"cancelled"}), "cancel-by-patch").await;
    assert_eq!(r.status, 400);
    let r = patch(&app, &id, json!({"types":["review.*"]}), "review-types").await;
    assert_eq!(r.status, 403);
    // Switching a stream subscription to a webhook generates its secret.
    let feed_id = feed.body["subscription"]["id"].as_str().unwrap();
    let switched = send(
        &app,
        "PATCH",
        &format!("/v1/subscriptions/{feed_id}"),
        Some(BOB),
        Some(json!({"delivery":{"mode":"webhook","url":"http://127.0.0.1:9/feed"}})),
        Some("feed-to-webhook"),
        &[],
    )
    .await;
    assert!(
        switched.body["secret"]
            .as_str()
            .unwrap()
            .starts_with("whsec_")
    );
    let rotated = send(
        &app,
        "POST",
        &format!("/v1/subscriptions/{feed_id}/secret/rotate"),
        Some(BOB),
        None,
        Some("rotate-feed"),
        &[],
    )
    .await;
    assert_ne!(rotated.body["secret"], switched.body["secret"]);

    let listed = send(&app, "GET", "/v1/subscriptions", Some(BOB), None, None, &[]).await;
    assert_eq!(listed.body["items"].as_array().unwrap().len(), 2);
    let cancelled = send(
        &app,
        "DELETE",
        &format!("/v1/subscriptions/{id}"),
        Some(BOB),
        None,
        Some("cancel-briefcase"),
        &[],
    )
    .await;
    assert_eq!(cancelled.status, 200);
    assert_eq!(cancelled.body["subscription"]["status"], "cancelled");
    assert!(cancelled.body["subscription"]["cancelled_at"].is_string());
    let again = send(
        &app,
        "DELETE",
        &format!("/v1/subscriptions/{id}"),
        Some(BOB),
        None,
        Some("cancel-again"),
        &[],
    )
    .await;
    assert_eq!(again.body["subscription"]["status"], "cancelled");
    let listed = send(&app, "GET", "/v1/subscriptions", Some(BOB), None, None, &[]).await;
    assert_eq!(listed.body["items"].as_array().unwrap().len(), 1);
    let all = send(
        &app,
        "GET",
        "/v1/subscriptions?status=all",
        Some(BOB),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(all.body["items"].as_array().unwrap().len(), 2);
    let r = patch(&app, &id, json!({"status":"active"}), "revive-cancelled").await;
    assert_eq!(r.status, 409);
    assert_eq!(r.body["error"]["code"], "subscription_cancelled");
}

async fn patch(app: &axum::Router, id: &str, body: Value, key: &str) -> Response {
    send(
        app,
        "PATCH",
        &format!("/v1/subscriptions/{id}"),
        Some(BOB),
        Some(body),
        Some(key),
        &[],
    )
    .await
}
fn rand_key() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    format!("key-{:08}", N.fetch_add(1, Ordering::SeqCst))
}

#[derive(Clone, Default)]
struct Receiver {
    seen: Arc<Mutex<Vec<(HeaderMap, Bytes)>>>,
    statuses: Arc<Mutex<Vec<u16>>>,
}
async fn receiver() -> (String, Receiver, tokio::task::JoinHandle<()>) {
    let r = Receiver::default();
    let shared = r.clone();
    let router = Router::new().route(
        "/hook",
        post(move |headers: HeaderMap, body: Bytes| {
            let shared = shared.clone();
            async move {
                shared.seen.lock().unwrap().push((headers, body));
                let status = shared.statuses.lock().unwrap().pop().unwrap_or(200);
                (StatusCode::from_u16(status).unwrap(), "receiver says hello")
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/hook", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (url, r, task)
}
fn verify(secret: &str, headers: &HeaderMap, body: &[u8]) {
    let timestamp = headers["x-apps-timestamp"].to_str().unwrap();
    assert!((chrono::Utc::now().timestamp() - timestamp.parse::<i64>().unwrap()).abs() < 60);
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body);
    let expected = format!("v1={}", hex::encode(mac.finalize().into_bytes()));
    assert_eq!(headers["x-apps-signature"].to_str().unwrap(), expected);
}

#[tokio::test]
async fn webhook_deliveries_are_signed_retried_held_while_paused_and_expire() {
    let (_dir, state, app) = fresh();
    let (_package, release) = seed_app(&state, "alice", "briefcase", &[], "linux-x86_64", true);
    let (url, hook, task) = receiver().await;
    start_background(state.clone()).await.unwrap();
    let created = send(&app, "POST", "/v1/subscriptions", Some(BOB), Some(json!({"app_id":"briefcase","types":["release.*"],"channels":["production"],"delivery":{"mode":"webhook","url":url}})), Some("bob-briefcase-hook"), &[]).await;
    assert_eq!(created.status, 201, "{}", created.body);
    let secret = created.body["secret"].as_str().unwrap().to_owned();
    let id = created.body["subscription"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    // The first attempt fails with 500; a development release does not match.
    hook.statuses.lock().unwrap().push(500);
    let dev = mutate(
        &state,
        "alice",
        "POST",
        "apps/briefcase/releases",
        "second-dev",
        json!({"version":"0.2.0","package_ids":["briefcase-linux-x86_64-pkg"]}),
        Default::default(),
    );
    assert_eq!(dev["channel"], "development");
    let (status, promoted) = call(
        &app,
        "POST",
        &format!("/v1/apps/briefcase/releases/{release}/promote"),
        Some(ALICE),
        json!({"version":"1.0.0"}),
        Some("promote-briefcase"),
    )
    .await;
    assert_eq!(status, 200);
    assert!(
        wait_until(|| hook.seen.lock().unwrap().len() == 1, 10).await,
        "first attempt"
    );
    let deliveries = || async {
        send(
            &app,
            "GET",
            &format!("/v1/subscriptions/{id}/deliveries"),
            Some(BOB),
            None,
            None,
            &[],
        )
        .await
        .body["items"]
            .clone()
    };
    let mut listed = Value::Null;
    for _ in 0..40 {
        listed = deliveries().await;
        if listed[0]["attempts"] == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        listed.as_array().unwrap().len(),
        1,
        "only the production release matched"
    );
    assert_eq!(listed[0]["status"], "pending");
    assert_eq!(listed[0]["last_status"], 500);
    assert!(
        listed[0]["last_error"]
            .as_str()
            .unwrap()
            .contains("HTTP 500")
    );
    assert!(
        listed[0]["last_error"]
            .as_str()
            .unwrap()
            .contains("receiver says hello")
    );
    // Skip the 10 second wait instead of sleeping through it.
    state
        .store
        .lock()
        .unwrap()
        .connection
        .execute("UPDATE subscription_deliveries SET next_attempt_ms=0", [])
        .unwrap();
    assert!(
        wait_until(|| hook.seen.lock().unwrap().len() == 2, 10).await,
        "retry"
    );
    let seen = hook.seen.lock().unwrap().clone();
    for (headers, body) in &seen {
        verify(&secret, headers, body);
        assert_eq!(headers["user-agent"], "SiliconApps-Webhooks/1");
        assert_eq!(headers["content-type"], "application/json");
        assert_eq!(headers["x-apps-event-type"], "release.promoted");
        assert_eq!(headers["x-apps-subscription-id"], id.as_str());
    }
    assert_eq!(seen[0].0["x-apps-event-id"], seen[1].0["x-apps-event-id"]);
    assert_eq!(
        seen[0].0["x-apps-delivery-id"],
        seen[1].0["x-apps-delivery-id"]
    );
    let payload: Value = serde_json::from_slice(&seen[1].1).unwrap();
    assert_eq!(payload["type"], "release.promoted");
    assert_eq!(payload["app_id"], "briefcase");
    assert_eq!(payload["subscription_id"], id.as_str());
    assert_eq!(payload["data"]["id"], promoted["id"]);
    assert_eq!(
        payload["event_id"],
        seen[1].0["x-apps-event-id"].to_str().unwrap()
    );
    assert!(
        wait_until(
            || state
                .store
                .lock()
                .unwrap()
                .connection
                .query_row("SELECT status FROM subscription_deliveries", [], |r| r
                    .get::<_, String>(
                    0
                ))
                .unwrap()
                == "delivered",
            5
        )
        .await
    );
    let listed = deliveries().await;
    assert_eq!(listed[0]["attempts"], 2);
    assert_eq!(listed[0]["last_status"], 200);
    assert!(listed[0]["delivered_at"].is_string());

    // Paused: the delivery is held, then goes out on resume.
    send(
        &app,
        "PATCH",
        &format!("/v1/subscriptions/{id}"),
        Some(BOB),
        Some(json!({"status":"paused"})),
        Some("pause-hook"),
        &[],
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/v1/apps/briefcase/releases/{release}/promote"),
        Some(ALICE),
        json!({"version":"1.0.1"}),
        Some("promote-paused"),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(hook.seen.lock().unwrap().len(), 2, "held while paused");
    send(
        &app,
        "PATCH",
        &format!("/v1/subscriptions/{id}"),
        Some(BOB),
        Some(json!({"status":"active"})),
        Some("resume-hook"),
        &[],
    )
    .await;
    assert!(
        wait_until(|| hook.seen.lock().unwrap().len() == 3, 10).await,
        "released on resume"
    );

    // A test ping goes only to this subscription.
    let ping = send(
        &app,
        "POST",
        &format!("/v1/subscriptions/{id}/ping"),
        Some(BOB),
        None,
        Some("ping-hook"),
        &[],
    )
    .await;
    assert_eq!(ping.status, 200, "{}", ping.body);
    assert!(
        wait_until(|| hook.seen.lock().unwrap().len() == 4, 10).await,
        "ping"
    );
    let (headers, body) = hook.seen.lock().unwrap()[3].clone();
    assert_eq!(headers["x-apps-event-type"], "ping");
    verify(&secret, &headers, &body);
    assert!(
        send(
            &app,
            "GET",
            "/v1/events?after=0",
            Some(BOB),
            None,
            None,
            &[]
        )
        .await
        .body["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["type"] != "ping")
    );

    // A delivery that cannot succeed within 72 hours fails for good.
    hook.statuses.lock().unwrap().extend([500, 500, 500]);
    call(
        &app,
        "POST",
        &format!("/v1/apps/briefcase/releases/{release}/promote"),
        Some(ALICE),
        json!({"version":"1.0.2"}),
        Some("promote-expire"),
    )
    .await;
    assert!(wait_until(|| hook.seen.lock().unwrap().len() == 5, 10).await);
    state.store.lock().unwrap().connection.execute("UPDATE subscription_deliveries SET created_ms=created_ms-73*3600*1000,next_attempt_ms=0 WHERE status='pending'", []).unwrap();
    let failed = || {
        state
            .store
            .lock()
            .unwrap()
            .connection
            .query_row(
                "SELECT COUNT(*) FROM subscription_deliveries WHERE status='failed'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
    };
    assert!(wait_until(|| failed() == 1, 5).await);
    let failed_list = send(
        &app,
        "GET",
        &format!("/v1/subscriptions/{id}/deliveries?status=failed"),
        Some(BOB),
        None,
        None,
        &[],
    )
    .await;
    assert!(
        failed_list.body["items"][0]["last_error"]
            .as_str()
            .unwrap()
            .contains("72 hours")
    );
    // Cancelling stops everything.
    send(
        &app,
        "DELETE",
        &format!("/v1/subscriptions/{id}"),
        Some(BOB),
        None,
        Some("cancel-hook"),
        &[],
    )
    .await;
    let before = hook.seen.lock().unwrap().len();
    call(
        &app,
        "POST",
        &format!("/v1/apps/briefcase/releases/{release}/promote"),
        Some(ALICE),
        json!({"version":"1.0.3"}),
        Some("promote-cancelled"),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(hook.seen.lock().unwrap().len(), before);
    task.abort();
}

#[tokio::test]
async fn losing_access_stops_deliveries() {
    let (_dir, state, app) = fresh();
    seed_app(&state, "alice", "secretive", &[], "linux-x86_64", true);
    let (url, hook, task) = receiver().await;
    start_background(state.clone()).await.unwrap();
    let created = send(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(BOB),
        Some(json!({"app_id":"secretive","delivery":{"mode":"webhook","url":url}})),
        Some("bob-secretive"),
        &[],
    )
    .await;
    assert_eq!(created.status, 201);
    mutate(
        &state,
        "alice",
        "PUT",
        "apps/secretive/access",
        "lock-it",
        json!({"visibility":"private"}),
        Default::default(),
    );
    let release = state
        .store
        .lock()
        .unwrap()
        .app("secretive")
        .unwrap()
        .releases[0]
        .id
        .clone();
    call(
        &app,
        "POST",
        &format!("/v1/apps/secretive/releases/{release}/promote"),
        Some(ALICE),
        json!({"version":"1.0.0"}),
        Some("promote-secret"),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(hook.seen.lock().unwrap().is_empty());
    task.abort();
}

#[tokio::test]
async fn stream_subscriptions_remember_where_they_stopped() {
    let (_dir, state, app) = fresh();
    let (_package, release) = seed_app(&state, "alice", "notes", &[], "linux-x86_64", true);
    let created = send(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(ALICE),
        Some(json!({"app_id":"notes","types":["release.promoted"],"delivery":{"mode":"stream"}})),
        Some("alice-notes-stream"),
        &[],
    )
    .await;
    let id = created.body["subscription"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (base, task) = serve(state.clone()).await;
    let path = format!("/v1/events/stream?subscription={id}");
    let mut stream = Sse::open(&base, &path, ALICE, None).await;
    call(
        &app,
        "POST",
        &format!("/v1/apps/notes/releases/{release}/promote"),
        Some(ALICE),
        json!({"version":"1.0.0"}),
        Some("notes-promote-1"),
    )
    .await;
    let first = stream.next(Duration::from_secs(5)).await.unwrap();
    assert_eq!(first.data["data"]["version"], "1.0.0");
    drop(stream);
    // Missed while disconnected. Without Last-Event-ID the stream starts after
    // the last acknowledged event, so nothing is lost: delivery is at least once.
    call(
        &app,
        "POST",
        &format!("/v1/apps/notes/releases/{release}/promote"),
        Some(ALICE),
        json!({"version":"1.0.1"}),
        Some("notes-promote-2"),
    )
    .await;
    let mut stream = Sse::open(&base, &path, ALICE, None).await;
    assert_eq!(
        stream.next(Duration::from_secs(5)).await.unwrap().id,
        first.id
    );
    let second = stream.next(Duration::from_secs(5)).await.unwrap();
    assert_eq!(second.data["data"]["version"], "1.0.1");
    drop(stream);
    // Reconnecting with Last-Event-ID acknowledges it.
    let mut stream = Sse::open(&base, &path, ALICE, Some(&second.id)).await;
    assert!(stream.next(Duration::from_millis(300)).await.is_none());
    drop(stream);
    let mut stream = Sse::open(&base, &path, ALICE, None).await;
    assert!(
        stream.next(Duration::from_millis(300)).await.is_none(),
        "acknowledged events are not replayed"
    );
    drop(stream);
    let sub = send(
        &app,
        "GET",
        &format!("/v1/subscriptions/{id}"),
        Some(ALICE),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(sub.body["cursor"].to_string(), second.id);
    // The JSON page through the subscription has both, from the start.
    let page = send(
        &app,
        "GET",
        &format!("/v1/events?subscription={id}&after=0"),
        Some(ALICE),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(page.body["items"].as_array().unwrap().len(), 2);
    // Other accounts cannot read it; paused subscriptions cannot stream.
    let r = send(&app, "GET", &path, Some(BOB), None, None, &[]).await;
    assert_eq!(r.status, 404);
    send(
        &app,
        "PATCH",
        &format!("/v1/subscriptions/{id}"),
        Some(ALICE),
        Some(json!({"status":"paused"})),
        Some("pause-notes"),
        &[],
    )
    .await;
    let r = send(&app, "GET", &path, Some(ALICE), None, None, &[]).await;
    assert_eq!(r.status, 409);
    assert_eq!(r.body["error"]["code"], "subscription_inactive");
    task.abort();
}

#[test]
fn production_webhook_urls_must_be_public_https() {
    for bad in [
        "http://example.com/hook",
        "https://localhost/hook",
        "https://apps.localhost/hook",
        "https://metadata.internal/hook",
        "https://intranet/hook",
        "https://10.1.2.3/hook",
        "https://127.0.0.1/hook",
        "https://169.254.169.254/latest",
        "https://100.64.0.1/",
        "https://[::1]/hook",
        "https://[fd00::1]/hook",
        "https://[::ffff:192.168.0.1]/hook",
        "https://user:pass@example.com/hook",
        "https://example.com/hook#fragment",
        "not a url",
    ] {
        let error = check_url(bad, false).unwrap_err();
        assert_eq!(error.code, "invalid_webhook_url", "{bad}");
    }
    assert_eq!(
        check_url("https://example.com/hooks/apps?x=1", false).unwrap(),
        "https://example.com/hooks/apps?x=1"
    );
    assert!(check_url("https://8.8.8.8/hook", false).is_ok());
    assert!(
        check_url("http://127.0.0.1:9000/hook", true).is_ok(),
        "local development accepts loopback HTTP"
    );
}
