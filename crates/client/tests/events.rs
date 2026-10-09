//! The client's capabilities, event, stream and subscription methods against
//! a real Apps server with isolated development authentication.
use axum::{Router, body::Bytes, http::HeaderMap, routing::post};
use serde_json::{Value, json};
use silicon_apps_client::{
    Client,
    events::{Delivery, Feed, NewSubscription, verify_webhook, webhook_signature},
};
use silicon_apps_server::{
    AppState,
    config::Config,
    model::{Identity, Package, now},
    router, start_background,
    store::{Mutation, Prepared, hash},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

fn config(dir: &std::path::Path) -> Config {
    Config {
        bind: "127.0.0.1:0".parse().unwrap(),
        data_dir: dir.to_owned(),
        dev_auth: true,
        accounts_url: "http://127.0.0.1:1".into(),
        accounts_app_secret: None,
        accounts_service_token: None,
        public_url: "http://127.0.0.1:4311".into(),
        allowed_origins: vec!["http://127.0.0.1:4311".into()],
        runner_url: None,
        runner_token: None,
        runner_targets: vec![],
        mail_url: None,
        mail_token: None,
        telemetry_url: None,
        telemetry_enabled: false,
        telemetry_table_key: None,
        import_accounts: false,
        historical_app_ids: Default::default(),
        rate_limit_reads_per_minute: 600,
        rate_limit_writes_per_minute: 120,
        rate_limit_streams: 10,
        signing_keys: None,
        revoked_signing_keys: vec![],
    }
}

#[tokio::test]
async fn client_reads_capabilities_streams_events_and_manages_subscriptions() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(config(dir.path())).unwrap();
    start_background(state.clone()).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let alice = Client::new(&base, Some("dev:alice:c:alice".into()))
        .unwrap()
        .with_telemetry(false);
    let bob = alice.authenticated(Some("dev:bob:c:bob".into()));

    let capabilities = alice
        .capabilities(&["streaming", "subscriptions", "version:2026-10-09"])
        .await
        .unwrap();
    assert_eq!(capabilities["requirements"]["satisfied"], true);
    // A requirement the server cannot meet is a structured 422.
    let missing = alice
        .capabilities(&["target:windows-aarch64"])
        .await
        .unwrap_err();
    let missing = missing
        .downcast_ref::<silicon_apps_client::ApiError>()
        .unwrap();
    assert_eq!(missing.status, 422);
    assert_eq!(missing.code, "capabilities_missing");
    assert_eq!(
        missing.details["missing"][0]["requirement"],
        "target:windows-aarch64"
    );

    // A published app with one development release.
    alice
        .create(
            "demo-app",
            "Demo",
            &"d".repeat(200),
            None,
            Some("client-create"),
        )
        .await
        .unwrap();
    let who = Identity {
        uuid: "alice".into(),
        id: "c:alice".into(),
        display_name: "alice".into(),
        verified_emails: vec![],
    };
    let bytes = b"client package";
    std::fs::write(dir.path().join("packages").join(hash(bytes)), bytes).unwrap();
    let package = Package {
        id: "pkg-1".into(),
        target: "linux-x86_64".into(),
        sha256: hash(bytes),
        size: bytes.len() as u64,
        command: "demo-app".into(),
        validation: vec![],
        created_at: now(),
        install_script: None,
        inspected: true,
        author_signature: None,
    };
    {
        let mut store = state.store.lock().unwrap();
        let body = json!({});
        store
            .mutate(Mutation {
                method: "POST",
                path: "apps/demo-app/packages/linux-x86_64",
                key: "client-package",
                who: Some(&who),
                body: &body,
                digest: &hash(b"{}"),
                prepared: Prepared {
                    package: Some(package),
                    ..Default::default()
                },
            })
            .unwrap();
    }
    let release = alice
        .action(
            "POST",
            "demo-app",
            &["releases"],
            Some(json!({"version":"0.1.0","package_ids":["pkg-1"]})),
            None,
        )
        .await
        .unwrap();
    alice
        .action("POST", "demo-app", &["publish"], Some(json!({})), None)
        .await
        .unwrap();

    // Bob follows production releases by webhook.
    let seen: Arc<Mutex<Vec<(HeaderMap, Bytes)>>> = Arc::default();
    let record = seen.clone();
    let receiver = Router::new().route(
        "/hook",
        post(move |headers: HeaderMap, body: Bytes| {
            let record = record.clone();
            async move {
                record.lock().unwrap().push((headers, body));
                "ok"
            }
        }),
    );
    let hook_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hook_url = format!("http://{}/hook", hook_listener.local_addr().unwrap());
    let hook = tokio::spawn(async move { axum::serve(hook_listener, receiver).await.unwrap() });
    let created = bob
        .create_subscription(
            &NewSubscription::production_releases("demo-app", &hook_url),
            Some("bob-subscribes"),
        )
        .await
        .unwrap();
    let secret = created["secret"].as_str().unwrap().to_owned();
    let id = created["subscription"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        created["subscription"]["types"],
        json!(["release.promoted"])
    );
    let again = bob
        .create_subscription(
            &NewSubscription::production_releases("demo-app", &hook_url),
            Some("bob-subscribes"),
        )
        .await
        .unwrap();
    assert_eq!(again["subscription"]["id"], id.as_str());

    // Alice watches her app's stream while she promotes the release.
    let mut stream = alice
        .stream_events(&Feed::App("demo-app".into()), None, &["release.*"])
        .await
        .unwrap();
    alice
        .action(
            "POST",
            "demo-app",
            &["releases", release["id"].as_str().unwrap(), "promote"],
            Some(json!({"version":"1.0.0"})),
            None,
        )
        .await
        .unwrap();
    let event = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(event.event, "release.promoted");
    assert_eq!(event.data["data"]["version"], "1.0.0");
    assert_eq!(stream.last_event_id(), event.id.as_deref());
    let page = alice
        .events(
            &Feed::App("demo-app".into()),
            Some(0),
            &["release.*"],
            Some(10),
        )
        .await
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    let bob_page = bob
        .events(&Feed::Subscription(id.clone()), Some(0), &[], None)
        .await
        .unwrap();
    assert_eq!(bob_page["items"][0]["type"], "release.promoted");

    // The delivery verifies with the secret, and nothing else does.
    for _ in 0..100 {
        if !seen.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let (headers, body) = seen.lock().unwrap()[0].clone();
    let timestamp = headers["x-apps-timestamp"].to_str().unwrap();
    let signature = headers["x-apps-signature"].to_str().unwrap();
    let now = chrono::Utc::now().timestamp();
    verify_webhook(&secret, timestamp, signature, &body, now, 300).unwrap();
    verify_webhook(
        &secret,
        timestamp,
        &format!("v1=00, {signature}"),
        &body,
        now,
        300,
    )
    .unwrap();
    assert!(
        verify_webhook(
            &secret,
            timestamp,
            signature,
            b"{\"type\":\"forged\"}",
            now,
            300
        )
        .is_err()
    );
    assert!(verify_webhook("whsec_wrong", timestamp, signature, &body, now, 300).is_err());
    assert!(verify_webhook(&secret, timestamp, signature, &body, now + 3600, 300).is_err());
    assert_eq!(
        webhook_signature(&secret, timestamp.parse().unwrap(), &body),
        signature
    );
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["subscription_id"], id.as_str());

    // Pause, resume, list deliveries, rotate, ping, cancel.
    assert_eq!(
        bob.pause_subscription(&id, None).await.unwrap()["subscription"]["status"],
        "paused"
    );
    assert_eq!(
        bob.resume_subscription(&id, None).await.unwrap()["subscription"]["status"],
        "active"
    );
    let deliveries = bob
        .subscription_deliveries(&id, Some("delivered"))
        .await
        .unwrap();
    assert_eq!(deliveries["items"].as_array().unwrap().len(), 1);
    let rotated = bob.rotate_subscription_secret(&id, None).await.unwrap();
    assert_ne!(rotated["secret"], secret.as_str());
    assert_eq!(
        bob.ping_subscription(&id, None).await.unwrap()["status"],
        "pending"
    );
    let updated = bob
        .update_subscription(&id, json!({"delivery":{"mode":"stream"}}), None)
        .await
        .unwrap();
    assert_eq!(
        updated["subscription"]["delivery"],
        serde_json::to_value(Delivery::Stream).unwrap()
    );
    assert_eq!(
        bob.subscriptions(None).await.unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        bob.cancel_subscription(&id, None).await.unwrap()["subscription"]["status"],
        "cancelled"
    );
    assert_eq!(bob.subscription(&id).await.unwrap()["status"], "cancelled");
    let error = alice.subscription(&id).await.unwrap_err();
    assert!(format!("{error:#}").contains("subscription_not_found"));
    hook.abort();
    server.abort();
}
