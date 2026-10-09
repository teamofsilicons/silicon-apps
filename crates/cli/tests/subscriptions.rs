//! `silicon-apps capabilities`, `events` and `subscriptions` against a real
//! Apps server with isolated development authentication.
use serde_json::{Value, json};
use silicon_apps_server::{
    AppState,
    config::Config,
    model::{Identity, Package, now},
    router,
    store::{Mutation, Prepared, hash},
};
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncBufReadExt, process::Command};

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
        rate_limit_reads_per_minute: 600,
        rate_limit_writes_per_minute: 120,
        rate_limit_streams: 10,
        signing_keys: None,
        revoked_signing_keys: vec![],
    }
}
fn cli(home: &std::path::Path, server: &str, token: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_silicon-apps"));
    command
        .args(["--json", "--home"])
        .arg(home)
        .args(["--server", server]);
    for name in [
        "APPS_URL",
        "ACCOUNTS_URL",
        "SILICON_HOME",
        "APPS_TELEMETRY_KEY",
        "APPS_TELEMETRY_TABLE_KEY",
    ] {
        command.env_remove(name);
    }
    command.env("APPS_TOKEN", token);
    command
}
async fn run(home: &std::path::Path, server: &str, token: &str, args: &[&str]) -> Value {
    let output = cli(home, server, token).args(args).output().await.unwrap();
    assert!(
        output.status.success(),
        "{args:?} exited {}: {}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[tokio::test]
async fn cli_subscribes_follows_events_and_negotiates_capabilities() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let state = AppState::new(config(dir.path())).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let app = router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let alice = "dev:alice:c:alice";
    let bob = "dev:bob:c:bob";

    let capabilities = run(
        home.path(),
        &server,
        bob,
        &["capabilities", "--require", "streaming,subscriptions"],
    )
    .await;
    assert_eq!(capabilities["requirements"]["satisfied"], true);

    // A published app with a development release.
    let who = Identity {
        uuid: "alice".into(),
        id: "c:alice".into(),
        display_name: "alice".into(),
        verified_emails: vec![],
    };
    let bytes = b"cli package";
    std::fs::write(dir.path().join("packages").join(hash(bytes)), bytes).unwrap();
    let steps = [
        (
            "POST",
            "apps",
            json!({"app_id":"demo-app","name":"Demo","description":"d".repeat(200)}),
            Prepared {
                secret: Some("s".into()),
                ..Default::default()
            },
        ),
        (
            "POST",
            "apps/demo-app/packages/linux-x86_64",
            json!({}),
            Prepared {
                package: Some(Package {
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
                }),
                ..Default::default()
            },
        ),
        (
            "POST",
            "apps/demo-app/releases",
            json!({"version":"0.1.0","package_ids":["pkg-1"]}),
            Prepared::default(),
        ),
        (
            "POST",
            "apps/demo-app/publish",
            json!({}),
            Prepared::default(),
        ),
    ];
    let mut release_id = String::new();
    for (i, (method, path, body, prepared)) in steps.into_iter().enumerate() {
        let result = state
            .store
            .lock()
            .unwrap()
            .mutate(Mutation {
                method,
                path,
                key: &format!("cli-seed-{i}"),
                who: Some(&who),
                body: &body,
                digest: &hash(body.to_string().as_bytes()),
                prepared,
            })
            .unwrap()
            .0;
        if path.ends_with("releases") {
            release_id = result["id"].as_str().unwrap().to_owned();
        }
    }

    let created = run(
        home.path(),
        &server,
        bob,
        &[
            "subscriptions",
            "create",
            "--app",
            "demo-app",
            "--type",
            "release.promoted",
            "--webhook",
            "http://127.0.0.1:9/hook",
            "--description",
            "production releases",
        ],
    )
    .await;
    assert!(created["secret"].as_str().unwrap().starts_with("whsec_"));
    let id = created["subscription"]["id"].as_str().unwrap().to_owned();
    let listed = run(home.path(), &server, bob, &["subscriptions", "list"]).await;
    assert_eq!(listed["items"][0]["id"], id.as_str());
    assert_eq!(
        run(home.path(), &server, bob, &["subscriptions", "pause", &id]).await["subscription"]["status"],
        "paused"
    );
    assert_eq!(
        run(home.path(), &server, bob, &["subscriptions", "resume", &id]).await["subscription"]["status"],
        "active"
    );
    let updated = run(
        home.path(),
        &server,
        bob,
        &["subscriptions", "update", &id, "--channel", "production"],
    )
    .await;
    assert_eq!(updated["subscription"]["channels"], json!(["production"]));
    let stream = run(
        home.path(),
        &server,
        bob,
        &["subscriptions", "create", "--stream"],
    )
    .await;
    assert!(stream["secret"].is_null());

    // Follow the app's stream while the release is promoted.
    let mut follow = cli(home.path(), &server, alice)
        .args([
            "events",
            "--app",
            "demo-app",
            "--type",
            "release.*",
            "--follow",
            "--limit",
            "1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(700)).await;
    let promote = reqwest_free_post(
        &server,
        alice,
        &format!("/v1/apps/demo-app/releases/{release_id}/promote"),
        r#"{"version":"1.0.0"}"#,
    )
    .await;
    assert!(promote.contains("200 OK"), "{promote}");
    let mut lines = tokio::io::BufReader::new(follow.stdout.take().unwrap()).lines();
    let line = tokio::time::timeout(Duration::from_secs(10), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let event: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(event["type"], "release.promoted");
    assert_eq!(event["data"]["version"], "1.0.0");
    let status = tokio::time::timeout(Duration::from_secs(10), follow.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success());

    let page = run(
        home.path(),
        &server,
        alice,
        &[
            "events",
            "--app",
            "demo-app",
            "--after",
            "0",
            "--type",
            "release.*",
        ],
    )
    .await;
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    let mine = run(
        home.path(),
        &server,
        bob,
        &["events", "--subscription", &id, "--after", "0"],
    )
    .await;
    assert_eq!(mine["items"][0]["type"], "release.promoted");
    let deliveries = run(
        home.path(),
        &server,
        bob,
        &["subscriptions", "deliveries", &id],
    )
    .await;
    assert_eq!(deliveries["items"].as_array().unwrap().len(), 1);
    let cancelled = run(home.path(), &server, bob, &["subscriptions", "cancel", &id]).await;
    assert_eq!(cancelled["subscription"]["status"], "cancelled");
    assert_eq!(
        run(home.path(), &server, bob, &["subscriptions", "show", &id]).await["status"],
        "cancelled"
    );
    // Errors stay structured.
    let output = cli(home.path(), &server, alice)
        .args(["subscriptions", "show", &id])
        .output()
        .await
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "subscription_not_found");
    assert_eq!(error["error"]["status"], 404);
    assert!(
        error["error"]["hint"]
            .as_str()
            .unwrap()
            .contains("GET /v1/subscriptions")
    );
    task.abort();
}

/// A tiny HTTP/1.1 POST so the test does not need another HTTP client.
async fn reqwest_free_post(base: &str, token: &str, path: &str, body: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let address = base.trim_start_matches("http://");
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {token}\r\nIdempotency-Key: cli-promote-one\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).await.unwrap();
    response
}
