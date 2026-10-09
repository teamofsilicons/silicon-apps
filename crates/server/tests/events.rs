//! The event log, author and account feeds, live streams with resume and
//! filters, and package validation progress.
mod common;
use axum::{body::Body, http::Request};
use common::*;
use serde_json::{Value, json};
use silicon_apps_server::{AppState, router};
use std::time::Duration;
use tower::ServiceExt;

fn kinds(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["type"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn events_are_appended_with_their_change_and_never_on_failure() {
    let (_dir, state, app) = fresh();
    let (status, _) = call(
        &app,
        "POST",
        "/v1/apps",
        Some(ALICE),
        json!({"app_id":"log-app","name":"Log"}),
        Some("create-log-app"),
    )
    .await;
    assert_eq!(status, 200);
    let page = send(
        &app,
        "GET",
        "/v1/apps/log-app/events?after=0",
        Some(ALICE),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(page.status, 200);
    assert_eq!(kinds(&page.body), ["app.created"]);
    let created = page.body["items"][0].clone();
    assert_eq!(created["app_id"], "log-app");
    assert_eq!(created["actor_uuid"], "alice");
    assert!(created["seq"].as_i64().unwrap() > 0);
    // The event carries the same ID as the history entry written with it.
    let history = state.store.lock().unwrap().app("log-app").unwrap().history;
    assert_eq!(created["id"], history[0].id);

    // A refused change leaves no event.
    let (status, _) = call(
        &app,
        "PATCH",
        "/v1/apps/log-app",
        Some(ALICE),
        json!({"name":""}),
        Some("bad-patch"),
    )
    .await;
    assert_eq!(status, 400);
    let page = send(
        &app,
        "GET",
        "/v1/apps/log-app/events",
        Some(ALICE),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(kinds(&page.body), ["app.created"]);
    assert_eq!(page.body["cursor"], created["seq"]);
    // A replayed request does not append again.
    for _ in 0..2 {
        let (status, _) = call(
            &app,
            "PATCH",
            "/v1/apps/log-app",
            Some(ALICE),
            json!({"tags":["x"]}),
            Some("tag-once"),
        )
        .await;
        assert_eq!(status, 200);
    }
    let page = send(
        &app,
        "GET",
        "/v1/apps/log-app/events?types=app.*",
        Some(ALICE),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(kinds(&page.body), ["app.created", "app.details_changed"]);

    // The log is append-only, enforced by the database.
    let store = state.store.lock().unwrap();
    assert!(
        store
            .connection
            .execute("UPDATE events SET type='x'", [])
            .is_err()
    );
    assert!(store.connection.execute("DELETE FROM events", []).is_err());
}

#[tokio::test]
async fn app_feeds_are_for_authors_and_reject_unknown_types() {
    let (_dir, state, app) = fresh();
    seed_app(&state, "alice", "open-app", &[], "linux-x86_64", true);
    seed_app(&state, "alice", "draft-app", &[], "linux-x86_64", false);
    for (path, token, status, code) in [
        (
            "/v1/apps/open-app/events",
            None,
            401,
            "authentication_required",
        ),
        (
            "/v1/apps/open-app/events/stream",
            None,
            401,
            "authentication_required",
        ),
        (
            "/v1/apps/open-app/events",
            Some(BOB),
            403,
            "author_required",
        ),
        (
            "/v1/apps/open-app/events/stream",
            Some(BOB),
            403,
            "author_required",
        ),
        ("/v1/apps/draft-app/events", Some(BOB), 404, "not_found"),
        (
            "/v1/apps/open-app/events?types=bogus.thing",
            Some(ALICE),
            400,
            "unknown_event_type",
        ),
        (
            "/v1/events/stream?types=release.*,nope",
            Some(BOB),
            400,
            "unknown_event_type",
        ),
        ("/v1/events?after=-3", Some(BOB), 400, "invalid_input"),
        ("/v1/events", None, 401, "authentication_required"),
    ] {
        let r = send(&app, "GET", path, token, None, None, &[]).await;
        assert_eq!(r.status, status, "{path}: {}", r.body);
        assert_eq!(r.body["error"]["code"], code, "{path}");
    }
    let r = send(
        &app,
        "GET",
        "/v1/apps/open-app/events?after=0&types=release.*,app.published",
        Some(ALICE),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(kinds(&r.body), ["release.created", "app.published"]);
}

#[tokio::test]
async fn app_stream_delivers_live_events_resumes_and_filters() {
    let (_dir, state, _app) = fresh();
    let (_package, release) = seed_app(&state, "alice", "live-app", &[], "linux-x86_64", true);
    let (base, task) = serve(state.clone()).await;
    let mut stream = Sse::open(
        &base,
        "/v1/apps/live-app/events/stream?types=release.*",
        ALICE,
        None,
    )
    .await;
    // Nothing old is replayed without Last-Event-ID.
    assert!(stream.next(Duration::from_millis(300)).await.is_none());
    assert!(
        stream
            .comments
            .iter()
            .any(|c| c.starts_with("ready cursor="))
    );
    let http = reqwest::Client::new();
    let post = |path: &str, body: Value, key: &str| {
        http.post(format!("{base}{path}"))
            .bearer_auth(ALICE)
            .header("Idempotency-Key", key)
            .json(&body)
            .send()
    };
    // A details change is filtered out; the promotion arrives.
    let patch = http
        .patch(format!("{base}/v1/apps/live-app"))
        .bearer_auth(ALICE)
        .header("Idempotency-Key", "rename-live-app")
        .json(&json!({"name":"Live"}))
        .send()
        .await
        .unwrap();
    assert_eq!(patch.status(), 200);
    let promoted = post(
        &format!("/v1/apps/live-app/releases/{release}/promote"),
        json!({"version":"1.0.0"}),
        "promote-1",
    )
    .await
    .unwrap();
    assert_eq!(promoted.status(), 200);
    let first = stream
        .next(Duration::from_secs(5))
        .await
        .expect("promotion streamed");
    assert_eq!(first.event, "release.promoted");
    assert_eq!(first.data["type"], "release.promoted");
    assert_eq!(first.data["data"]["channel"], "production");
    assert_eq!(first.data["data"]["version"], "1.0.0");
    assert_eq!(first.id, first.data["seq"].to_string());
    let promoted = post(
        &format!("/v1/apps/live-app/releases/{release}/promote"),
        json!({"version":"1.0.1"}),
        "promote-2",
    )
    .await
    .unwrap();
    assert_eq!(promoted.status(), 200);
    let second = stream.next(Duration::from_secs(5)).await.unwrap();
    assert_eq!(second.data["data"]["version"], "1.0.1");
    drop(stream);

    // Resume after the first event: only what followed it is sent again.
    let mut resumed = Sse::open(
        &base,
        "/v1/apps/live-app/events/stream?types=release.promoted",
        ALICE,
        Some(&first.id),
    )
    .await;
    let replay = resumed.next(Duration::from_secs(5)).await.unwrap();
    assert_eq!(replay.id, second.id);
    assert!(resumed.next(Duration::from_millis(300)).await.is_none());
    // The query form works for clients that cannot set headers.
    let mut by_query = Sse::open(
        &base,
        &format!("/v1/apps/live-app/events/stream?last_event_id={}", first.id),
        ALICE,
        None,
    )
    .await;
    assert_eq!(
        by_query.next(Duration::from_secs(5)).await.unwrap().id,
        second.id
    );
    task.abort();
}

#[tokio::test]
async fn account_feed_has_invites_authored_apps_and_releases_of_installed_apps() {
    let (_dir, state, app) = fresh();
    let (package, release) = seed_app(&state, "alice", "shared-app", &[], "linux-x86_64", true);
    seed_app(&state, "alice", "other-app", &[], "linux-x86_64", true);
    // Bob installs shared-app only.
    let r = send(
        &app,
        "POST",
        "/v1/apps/shared-app/installs",
        Some(BOB),
        Some(json!({"release_id":release,"package_id":package})),
        Some("bob-install"),
        &[],
    )
    .await;
    assert_eq!(r.status, 200);
    let (base, task) = serve(state.clone()).await;
    let mut bob = Sse::open(&base, "/v1/events/stream", BOB, None).await;
    let mut carol = Sse::open(&base, "/v1/events/stream", CAROL, None).await;
    for (app_id, key) in [
        ("other-app", "other-promote"),
        ("shared-app", "shared-promote"),
    ] {
        let release_id = state.store.lock().unwrap().app(app_id).unwrap().releases[0]
            .id
            .clone();
        let (status, _) = call(
            &app,
            "POST",
            &format!("/v1/apps/{app_id}/releases/{release_id}/promote"),
            Some(ALICE),
            json!({"version":"1.0.0"}),
            Some(key),
        )
        .await;
        assert_eq!(status, 200);
    }
    let event = bob.next(Duration::from_secs(5)).await.unwrap();
    assert_eq!(event.event, "release.promoted");
    assert_eq!(event.data["app_id"], "shared-app");
    // Invitations reach the invited account.
    let (status, _) = call(
        &app,
        "POST",
        "/v1/apps/other-app/invites",
        Some(ALICE),
        json!({"to":"si:carol"}),
        Some("invite-carol"),
    )
    .await;
    assert_eq!(status, 200);
    let invited = carol.next(Duration::from_secs(5)).await.unwrap();
    assert_eq!(invited.event, "author.invited");
    assert_eq!(invited.data["data"]["to"], "si:carol");
    assert!(
        bob.next(Duration::from_millis(300)).await.is_none(),
        "bob sees neither other-app nor invitations to carol"
    );
    // Bob's page holds no author-only events of the installed app.
    let page = send(
        &app,
        "GET",
        "/v1/events?after=0",
        Some(BOB),
        None,
        None,
        &[],
    )
    .await;
    let kinds = kinds(&page.body);
    assert!(kinds.iter().all(|k| ["app.published", "release.created", "release.promoted"].contains(&k.as_str())), "{kinds:?}");
    assert!(kinds.contains(&"release.promoted".to_owned()));
    // Alice, the author, sees everything about her apps.
    let page = send(
        &app,
        "GET",
        "/v1/events?after=0&types=package.*",
        Some(ALICE),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(page.body["items"].as_array().unwrap().len(), 2);
    task.abort();
}

async fn upload(app: &axum::Router, bytes: Vec<u8>, key: &str) -> (u16, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/apps/demo-app/packages/linux-x86_64")
                .header("Authorization", format!("Bearer {ALICE}"))
                .header("Content-Type", "application/gzip")
                .header("Idempotency-Key", key)
                .body(Body::from(bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn package_validation_progress_is_streamed_step_by_step() {
    for (reported, accepted) in [("demo-app", true), ("someone-else", false)] {
        let (runner, task) = fake_runner(reported).await;
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config(dir.path());
        cfg.runner_url = Some(runner);
        cfg.runner_token = Some("runner-token-runner-token-runner-token".into());
        cfg.runner_targets = vec!["linux-x86_64".into()];
        let state = AppState::new(cfg).unwrap();
        let app = router(state.clone());
        let (status, _) = call(
            &app,
            "POST",
            "/v1/apps",
            Some(ALICE),
            json!({"app_id":"demo-app","name":"Demo"}),
            Some("create-demo"),
        )
        .await;
        assert_eq!(status, 200);
        let (base, server) = serve(state.clone()).await;
        let mut stream = Sse::open(
            &base,
            "/v1/apps/demo-app/events/stream?types=package.*",
            ALICE,
            None,
        )
        .await;
        let (status, body) = upload(
            &app,
            package_bytes("demo-app", "linux-x86_64"),
            "upload-demo",
        )
        .await;
        assert_eq!(status, if accepted { 200 } else { 422 }, "{body}");
        let mut events = vec![];
        while let Some(event) = stream.next(Duration::from_secs(2)).await {
            events.push(event);
        }
        let steps: Vec<String> = events
            .iter()
            .map(|e| match e.data["data"]["step"].as_str() {
                Some("command") => format!(
                    "command {} {}",
                    e.data["data"]["command"].as_str().unwrap(),
                    e.data["data"]["passed"]
                ),
                Some(step) => format!("{step} {}", e.data["data"]["status"].as_str().unwrap()),
                None => e.event.clone(),
            })
            .collect();
        assert_eq!(
            steps,
            [
                "package.validation_started".to_owned(),
                "archive passed".into(),
                "manifest passed".into(),
                "runner started".into(),
                "command --help true".into(),
                format!("command accounts --json {accepted}"),
                "command login status --json true".into(),
                if accepted {
                    "package.accepted"
                } else {
                    "package.validation_failed"
                }
                .into(),
            ]
        );
        let validation_id = &events[0].data["data"]["validation_id"];
        assert!(
            events[1..7]
                .iter()
                .all(|e| &e.data["data"]["validation_id"] == validation_id)
        );
        assert!(
            events[5].data["data"]["expected"]
                .as_str()
                .unwrap()
                .contains("app_id")
        );
        assert!(
            events[5].data["data"]["stdout"]
                .as_str()
                .unwrap()
                .contains(reported)
        );
        // A broken archive stops at the archive step.
        let (status, _) = upload(&app, b"not an archive".to_vec(), "upload-broken").await;
        assert_eq!(status, 422);
        let mut broken = vec![];
        while let Some(event) = stream.next(Duration::from_secs(2)).await {
            broken.push(event);
        }
        assert_eq!(broken.len(), 3, "{broken:?}");
        assert_eq!(broken[1].data["data"]["step"], "archive");
        assert_eq!(broken[1].data["data"]["status"], "failed");
        assert_eq!(broken[2].event, "package.validation_failed");
        server.abort();
        task.abort();
    }
}
