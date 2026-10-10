//! Historical Silicon Accounts app IDs (`APPS_HISTORICAL_APP_IDS`): a 1–2
//! character ID such as `dm` is available to, and can be created by, only its
//! configured owner. Everyone else gets the answer for an invalid ID. Once
//! created it is an ordinary app.
mod common;
use axum::{body::Body, http::Request};
use common::*;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use silicon_apps_server::{AppState, Shared, config::HistoricalAppIds, router, store::hash};
use tower::ServiceExt;

/// `dm` belongs to alice (`dev:alice:c:alice`, UUID `alice`).
fn historical_state(dir: &std::path::Path, runner: Option<&str>) -> Shared {
    let mut cfg = config(dir);
    cfg.historical_app_ids = HistoricalAppIds::parse("dm:alice").unwrap();
    if let Some(url) = runner {
        cfg.runner_url = Some(url.into());
        cfg.runner_token = Some("runner-token-runner-token-runner-token".into());
        cfg.runner_targets = vec!["linux-x86_64".into()];
    }
    AppState::new(cfg).unwrap()
}
async fn get(app: &axum::Router, path: &str, token: Option<&str>) -> Response {
    send(app, "GET", path, token, None, None, &[]).await
}
async fn availability(app: &axum::Router, id: &str, token: Option<&str>) -> (u16, Value) {
    let r = get(app, &format!("/v1/apps/availability/{id}"), token).await;
    (r.status.as_u16(), r.body)
}
async fn create(app: &axum::Router, token: &str, id: &str, key: &str) -> Response {
    let body = json!({"app_id":id,"name":id.to_uppercase()});
    send(
        app,
        "POST",
        "/v1/apps",
        Some(token),
        Some(body),
        Some(key),
        &[],
    )
    .await
}
fn pending_secrets(state: &Shared) -> i64 {
    state
        .store
        .lock()
        .unwrap()
        .connection
        .query_row("SELECT COUNT(*) FROM pending_secrets", [], |r| r.get(0))
        .unwrap()
}
async fn upload(app: &axum::Router, path: &str, mime: &str, bytes: Vec<u8>, key: &str) -> Response {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("Authorization", format!("Bearer {ALICE}"))
                .header("Content-Type", mime)
                .header("Idempotency-Key", key)
                .body(Body::from(bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Response {
        status,
        headers,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

#[tokio::test]
async fn a_historical_id_is_available_only_to_its_signed_in_owner() {
    let dir = tempfile::tempdir().unwrap();
    let app = router(historical_state(dir.path(), None));
    let invalid = (200, json!({"available":false}));
    // An unconfigured two-character ID is invalid for everyone, alice included.
    assert_eq!(availability(&app, "wf", None).await, invalid);
    assert_eq!(availability(&app, "wf", Some(ALICE)).await, invalid);
    // Signed out, or signed in as anyone else, dm gets exactly that answer.
    assert_eq!(availability(&app, "dm", None).await, invalid);
    assert_eq!(availability(&app, "dm", Some(BOB)).await, invalid);
    assert_eq!(availability(&app, "dm", Some(CAROL)).await, invalid);
    // Its owner, signed in, may claim it.
    assert_eq!(
        availability(&app, "dm", Some(ALICE)).await,
        (200, json!({"available":true}))
    );
    // New IDs answer as before.
    assert_eq!(
        availability(&app, "ring", None).await,
        (200, json!({"available":true}))
    );
    assert_eq!(availability(&app, "apps", Some(ALICE)).await, invalid);
    // Once created, dm is taken for its owner as well.
    let created = create(&app, ALICE, "dm", "create-dm").await;
    assert_eq!(created.status, 200, "{}", created.body);
    assert_eq!(availability(&app, "dm", Some(ALICE)).await, invalid);
    assert_eq!(availability(&app, "dm", None).await, invalid);
}

#[tokio::test]
async fn only_the_configured_owner_can_create_a_historical_id() {
    let dir = tempfile::tempdir().unwrap();
    let state = historical_state(dir.path(), None);
    let app = router(state.clone());
    // An unconfigured two-character ID is refused as invalid.
    let invalid = create(&app, ALICE, "wf", "create-wf").await;
    assert_eq!(invalid.status, 400, "{}", invalid.body);
    assert_eq!(invalid.body["error"]["code"], "invalid_input");
    assert_eq!(
        invalid.body["error"]["message"],
        "app_id must be 3–30 lowercase letters, digits, hyphens or underscores."
    );
    // Any other account gets exactly that refusal for dm, so the answer never
    // reveals that dm is configured or whose it is.
    for (token, key) in [(BOB, "create-dm-bob"), (CAROL, "create-dm-carol")] {
        let refused = create(&app, token, "dm", key).await;
        assert_eq!(
            (refused.status, &refused.body),
            (invalid.status, &invalid.body)
        );
    }
    let anonymous = send(
        &app,
        "POST",
        "/v1/apps",
        None,
        Some(json!({"app_id":"dm","name":"DM"})),
        Some("create-dm-anonymous"),
        &[],
    )
    .await;
    assert_eq!(anonymous.status, 401, "{}", anonymous.body);
    assert!(
        state
            .store
            .lock()
            .unwrap()
            .catalog()
            .unwrap()
            .apps
            .is_empty()
    );
    assert_eq!(pending_secrets(&state), 0);

    // Its owner creates it like any app: the secret shown once, the owner as
    // first author and admin, an unpublished draft.
    let created = create(&app, ALICE, "dm", "create-dm").await;
    assert_eq!(created.status, 200, "{}", created.body);
    assert_eq!(created.headers["Idempotent-Replayed"], "false");
    let view = &created.body["app"];
    assert_eq!(view["app_id"], "dm");
    assert_eq!(view["name"], "DM");
    assert_eq!(view["published"], false);
    assert_eq!(view["setup_step"], 1);
    assert_eq!(view["is_admin"], true);
    assert_eq!(view["authors"].as_array().unwrap().len(), 1);
    assert_eq!(view["authors"][0]["uuid"], "alice");
    let secret = created.body["app_secret"].as_str().unwrap();
    assert!(secret.starts_with("sa_app_"), "{secret}");
    assert_eq!(
        state.store.lock().unwrap().app("dm").unwrap().secret_hash,
        hash(secret.as_bytes())
    );
    // Retrying replays the same answer and secret; a new request conflicts.
    let replay = create(&app, ALICE, "dm", "create-dm").await;
    assert_eq!(replay.status, 200);
    assert_eq!(replay.headers["Idempotent-Replayed"], "true");
    assert_eq!(replay.body, created.body);
    let again = create(&app, ALICE, "dm", "create-dm-again").await;
    assert_eq!(again.status, 409, "{}", again.body);
    let later = create(&app, BOB, "dm", "create-dm-bob-later").await;
    assert_eq!((later.status, &later.body), (invalid.status, &invalid.body));
    assert_eq!(pending_secrets(&state), 0);

    // Creating an ordinary ID is unchanged.
    let ring = create(&app, ALICE, "ring", "create-ring").await;
    assert_eq!(ring.status, 200, "{}", ring.body);
    assert_eq!(ring.body["app"]["app_id"], "ring");
    // History records who created each app, with which key, and notes the
    // historical ID only where one was claimed.
    for (id, data) in [
        ("dm", json!({"name":"DM","historical_app_id":true})),
        ("ring", json!({"name":"RING"})),
    ] {
        let history = get(&app, &format!("/v1/apps/{id}/history"), Some(ALICE)).await;
        assert_eq!(history.status, 200, "{}", history.body);
        let items = history.body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0]["kind"], "app.created");
        assert_eq!(items[0]["data"], data);
        assert_eq!(items[0]["actor_uuid"], "alice");
        assert_eq!(items[0]["idempotency_key"], format!("create-{id}"));
    }
    // The author feed carries the same detail.
    let events = get(&app, "/v1/apps/dm/events", Some(ALICE)).await;
    assert_eq!(events.status, 200, "{}", events.body);
    assert_eq!(events.body["items"][0]["type"], "app.created");
    assert_eq!(events.body["items"][0]["data"]["historical_app_id"], true);
}

#[tokio::test]
async fn the_owner_registers_a_historical_id_with_accounts_unless_accounts_holds_it() {
    use axum::{
        Json, Router,
        routing::{get as get_route, post},
    };
    use std::sync::{Arc, Mutex};
    // A Silicon Accounts registry that already holds `wf` and records syncs.
    let synced = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (listed, recorded) = (synced.clone(), synced.clone());
    let accounts = Router::new()
        .route(
            "/v1/internal/apps",
            get_route(move || {
                let listed = listed.clone();
                async move {
                    let mut apps = vec![json!({"app_id":"wf"})];
                    for sync in listed.lock().unwrap().iter() {
                        apps.push(json!({"app_id":sync["apps"][0]["app_id"]}));
                    }
                    Json(json!({"apps":apps}))
                }
            }),
        )
        .route(
            "/v1/internal/apps/sync",
            post(move |Json(body): Json<Value>| {
                let recorded = recorded.clone();
                async move {
                    recorded.lock().unwrap().push(body);
                    Json(json!({"synced":1}))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let accounts_url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, accounts).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.accounts_url = accounts_url;
    cfg.accounts_service_token = Some("server-test-token".into());
    cfg.historical_app_ids = HistoricalAppIds::parse("dm:alice,wf:alice").unwrap();
    let state = AppState::new(cfg).unwrap();
    let app = router(state.clone());

    // dm is free in Apps and in Accounts: its owner creates it, and Accounts
    // is told about the app with its secret and its owner, as for any app.
    assert_eq!(
        availability(&app, "dm", Some(ALICE)).await,
        (200, json!({"available":true}))
    );
    let created = create(&app, ALICE, "dm", "create-dm").await;
    assert_eq!(created.status, 200, "{}", created.body);
    {
        let synced = synced.lock().unwrap();
        assert_eq!(synced.len(), 1, "{synced:?}");
        let registered = &synced[0]["apps"][0];
        assert_eq!(registered["app_id"], "dm");
        assert_eq!(registered["secret"], created.body["app_secret"]);
        assert_eq!(registered["owner_uuid"], "alice");
        assert_eq!(registered["author_uuids"], json!(["alice"]));
    }
    // wf is still in the Accounts registry, so it is taken, even for its owner.
    assert_eq!(
        availability(&app, "wf", Some(ALICE)).await,
        (200, json!({"available":false}))
    );
    let taken = create(&app, ALICE, "wf", "create-wf").await;
    assert_eq!(taken.status, 409, "{}", taken.body);
    assert_eq!(
        taken.body["error"]["message"],
        "This app_id already exists in Silicon Accounts. Import existing apps instead of creating a duplicate."
    );
    assert_eq!(synced.lock().unwrap().len(), 1);
    assert!(state.store.lock().unwrap().app("wf").is_err());
    assert_eq!(pending_secrets(&state), 0);
    task.abort();
}

#[tokio::test]
async fn without_the_setting_a_two_character_id_cannot_be_created() {
    let (_dir, state, app) = fresh();
    assert!(state.config.historical_app_ids.is_empty());
    assert_eq!(
        availability(&app, "dm", Some(ALICE)).await,
        (200, json!({"available":false}))
    );
    let refused = create(&app, ALICE, "dm", "create-dm").await;
    assert_eq!(refused.status, 400, "{}", refused.body);
    assert_eq!(
        refused.body["error"]["message"],
        "app_id must be 3–30 lowercase letters, digits, hyphens or underscores."
    );
    assert!(
        state
            .store
            .lock()
            .unwrap()
            .catalog()
            .unwrap()
            .apps
            .is_empty()
    );
}

#[tokio::test]
async fn a_historical_app_is_set_up_published_found_and_installed_like_any_other() {
    let (runner, task) = fake_runner("dm").await;
    let dir = tempfile::tempdir().unwrap();
    let state = historical_state(dir.path(), Some(&runner));
    let app = router(state.clone());
    assert_eq!(create(&app, ALICE, "dm", "create-dm").await.status, 200);
    let as_alice = |method: &'static str, path: String, body: Value, key: &'static str| {
        let app = app.clone();
        async move { send(&app, method, &path, Some(ALICE), Some(body), Some(key), &[]).await }
    };

    // Setup step 1, details.
    let description = "DM sends direct messages between Carbons and Silicons. ".repeat(4);
    let details = as_alice(
        "PATCH",
        "/v1/apps/dm".into(),
        json!({"name":"DM","description":description,"tags":["messaging"],"setup_step":1}),
        "dm-details",
    )
    .await;
    assert_eq!(details.status, 200, "{}", details.body);
    assert_eq!(details.body["description"], description);
    assert_eq!(details.body["tags"], json!(["messaging"]));
    // The draft is in its author's list, and nobody else's.
    let mine = get(&app, "/v1/apps?mine=true", Some(ALICE)).await;
    assert_eq!(mine.status, 200, "{}", mine.body);
    let draft = mine.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["app_id"] == "dm")
        .expect("dm is in its author's list");
    assert_eq!(draft["published"], false);
    assert_eq!(draft["description"], description);
    let bobs = get(&app, "/v1/apps?mine=true", Some(BOB)).await;
    assert_eq!(bobs.body["total"], 0, "{}", bobs.body);

    // Media is stored and addressed under the short ID.
    let media = upload(
        &app,
        "/v1/apps/dm/media",
        "image/png",
        b"\x89PNG\r\n\x1a\nhistorical".to_vec(),
        "dm-logo-upload",
    )
    .await;
    assert_eq!(media.status, 200, "{}", media.body);
    let logo = media.body["url"].as_str().unwrap().to_owned();
    assert!(logo.starts_with("/v1/apps/dm/media/"), "{logo}");
    let with_logo = as_alice(
        "PATCH",
        "/v1/apps/dm".into(),
        json!({"logo":logo,"setup_step":5}),
        "dm-media",
    )
    .await;
    assert_eq!(with_logo.status, 200, "{}", with_logo.body);

    // A package whose apps.yaml names app_id dm passes isolated validation.
    let package = upload(
        &app,
        "/v1/apps/dm/packages/linux-x86_64",
        "application/gzip",
        package_bytes("dm", "linux-x86_64"),
        "dm-package",
    )
    .await;
    assert_eq!(package.status, 200, "{}", package.body);
    let package_id = package.body["id"].as_str().unwrap().to_owned();
    let release = as_alice(
        "POST",
        "/v1/apps/dm/releases".into(),
        json!({"version":"0.1.0","package_ids":[package_id]}),
        "dm-release",
    )
    .await;
    assert_eq!(release.status, 200, "{}", release.body);
    assert_eq!(release.body["app_id"], "dm");
    let promoted = as_alice(
        "POST",
        format!(
            "/v1/apps/dm/releases/{}/promote",
            release.body["id"].as_str().unwrap()
        ),
        json!({"version":"1.0.0"}),
        "dm-promote",
    )
    .await;
    assert_eq!(promoted.status, 200, "{}", promoted.body);
    let published = as_alice(
        "POST",
        "/v1/apps/dm/publish".into(),
        json!({}),
        "dm-publish",
    )
    .await;
    assert_eq!(published.status, 200, "{}", published.body);
    assert_eq!(published.body["published"], true);

    // Anyone finds it by its ID, resolves the signed release and installs it.
    let found = get(&app, "/v1/apps?q=dm", None).await;
    assert_eq!(found.body["items"][0]["app_id"], "dm", "{}", found.body);
    let resolved = get(&app, "/v1/apps/dm/resolve?target=linux-x86_64", None).await;
    assert_eq!(resolved.status, 200, "{}", resolved.body);
    assert_eq!(resolved.body["app_id"], "dm");
    assert_eq!(resolved.body["release"]["version"], "1.0.0");
    assert!(resolved.body["signature"].is_object(), "{}", resolved.body);
    let download = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(resolved.body["download_path"].as_str().unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(download.status(), 200);
    let bytes = download.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        hash(&bytes),
        resolved.body["package"]["sha256"].as_str().unwrap()
    );
    let installed = send(
        &app,
        "POST",
        "/v1/apps/dm/installs",
        None,
        Some(json!({"release_id":resolved.body["release"]["id"],"package_id":resolved.body["package"]["id"]})),
        Some("dm-install"),
        &[],
    )
    .await;
    assert_eq!(installed.status, 200, "{}", installed.body);
    assert_eq!(installed.body["installs"], 1);
    task.abort();
}

#[test]
fn the_api_refuses_to_start_with_an_invalid_historical_app_id() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_apps-server"))
        .env_clear()
        .env("APPS_DATA_DIR", dir.path())
        .env("APPS_HISTORICAL_APP_IDS", "dm:zQo,abc:zQo")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.lines().next().unwrap(),
        "invalid_historical_app_ids: APPS_HISTORICAL_APP_IDS: `abc` has 3 or more characters, so it is an ordinary new app ID; list only historical app IDs of 1 or 2 characters.",
        "{stderr}"
    );
    assert!(!dir.path().join("apps.sqlite").exists());
}
