//! `silicon-apps availability` and `create` send a 1–2 character historical
//! Silicon Accounts app ID to the server unchanged, and the server decides:
//! only the account the operator reserved it for may claim it.
use serde_json::{Value, json};
use silicon_apps_server::{
    AppState,
    config::{Config, HistoricalAppIds},
    router,
};
use std::path::Path;
use tokio::process::Command;

fn config(dir: &Path) -> Config {
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
        historical_app_ids: HistoricalAppIds::parse("dm:alice").unwrap(),
        rate_limit_reads_per_minute: 0,
        rate_limit_writes_per_minute: 0,
        rate_limit_streams: 10,
        signing_keys: None,
        revoked_signing_keys: vec![],
    }
}
/// Exit code, stdout JSON and stderr JSON of one `silicon-apps --json` run.
async fn run(home: &Path, server: &str, token: Option<&str>, args: &[&str]) -> (i32, Value, Value) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_silicon-apps"));
    command
        .args(["--json", "--home"])
        .arg(home)
        .args(["--server", server]);
    for name in [
        "APPS_TOKEN",
        "APPS_URL",
        "ACCOUNTS_URL",
        "SILICON_HOME",
        "APPS_TELEMETRY_KEY",
        "APPS_TELEMETRY_TABLE_KEY",
    ] {
        command.env_remove(name);
    }
    if let Some(token) = token {
        command.env("APPS_TOKEN", token);
    }
    let output = command.args(args).output().await.unwrap();
    let parse = |bytes: &[u8]| serde_json::from_slice(bytes).unwrap_or(Value::Null);
    (
        output.status.code().unwrap(),
        parse(&output.stdout),
        parse(&output.stderr),
    )
}

#[tokio::test]
async fn availability_and_create_leave_historical_ids_to_the_server() {
    let data = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let state = AppState::new(config(data.path())).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let app = router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let alice = Some("dev:alice:c:alice");
    let bob = Some("dev:bob:c:bob");

    for (token, available) in [(None, false), (bob, false), (alice, true)] {
        let (code, out, err) = run(home.path(), &server, token, &["availability", "dm"]).await;
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, json!({"available":available}), "{token:?}");
    }
    // Anyone else's create reaches the server, which answers as for any
    // invalid ID: the CLI does not refuse it first.
    let (code, out, err) = run(home.path(), &server, bob, &["create", "dm", "--name", "DM"]).await;
    assert_eq!(code, 1, "{out}");
    assert_eq!(err["error"]["code"], "invalid_input", "{err}");
    assert_eq!(err["error"]["status"], 400, "{err}");
    assert_eq!(
        err["error"]["message"],
        "app_id must be 3–30 lowercase letters, digits, hyphens or underscores."
    );
    // Its owner creates it, sets it up and finds it among her apps.
    let (code, created, err) = run(
        home.path(),
        &server,
        alice,
        &["create", "dm", "--name", "DM"],
    )
    .await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(created["app"]["app_id"], "dm");
    assert!(
        created["app_secret"]
            .as_str()
            .unwrap()
            .starts_with("sa_app_")
    );
    let description = "DM sends direct messages between Carbons and Silicons. ".repeat(4);
    let (code, details, err) = run(
        home.path(),
        &server,
        alice,
        &["setup", "dm", "details", "--description", &description],
    )
    .await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(details["description"], description);
    let (code, mine, err) = run(home.path(), &server, alice, &["list", "--mine"]).await;
    assert_eq!(code, 0, "{err}");
    assert!(
        mine["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["app_id"] == "dm" && a["published"] == false),
        "{mine}"
    );
    task.abort();
}
