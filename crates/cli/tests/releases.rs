//! Author keys, signed uploads, signed installs in CI mode, install script
//! transparency and withdrawn releases through the `silicon-apps` CLI, against
//! a real Apps server and a fake isolated runner.
use axum::{Json, Router, routing::post};
use serde_json::{Value, json};
use silicon_apps_server::{AppState, config::Config, router};
use std::{fs, path::Path};
use tokio::process::Command;

fn config(dir: &Path, runner: &str) -> Config {
    Config {
        bind: "127.0.0.1:0".parse().unwrap(),
        data_dir: dir.to_owned(),
        dev_auth: true,
        accounts_url: "http://127.0.0.1:1".into(),
        accounts_app_secret: None,
        accounts_service_token: None,
        public_url: "http://127.0.0.1:4311".into(),
        allowed_origins: vec!["http://127.0.0.1:4311".into()],
        runner_url: Some(runner.into()),
        runner_token: Some("runner-token-runner-token-runner-token".into()),
        runner_targets: silicon_apps_client::package::TARGETS
            .iter()
            .map(|t| t.to_string())
            .collect(),
        mail_url: None,
        mail_token: None,
        telemetry_url: None,
        telemetry_enabled: false,
        telemetry_table_key: None,
        import_accounts: false,
        historical_app_ids: Default::default(),
        rate_limit_reads_per_minute: 0,
        rate_limit_writes_per_minute: 0,
        rate_limit_streams: 10,
        signing_keys: None,
        revoked_signing_keys: vec![],
    }
}
fn cli(home: &Path, server: &str, token: &str) -> Command {
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
        "SILICON_APPS_NO_DAEMON",
    ] {
        command.env_remove(name);
    }
    command.env("APPS_TOKEN", token);
    command
}
async fn run_ok(command: &mut Command, args: &[&str]) -> (Value, String) {
    let output = command.args(args).output().await.unwrap();
    assert!(
        output.status.success(),
        "{args:?} exited {}: {}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (
        serde_json::from_slice(&output.stdout).unwrap_or(Value::Null),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}
async fn run_err(command: &mut Command, args: &[&str]) -> Value {
    let output = command.args(args).output().await.unwrap();
    assert_eq!(output.status.code(), Some(1), "{args:?} should fail");
    serde_json::from_slice(&output.stderr)
        .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&output.stderr)}))
}
async fn runner() -> String {
    let app = Router::new().route(
        "/validate",
        post(|Json(body): Json<Value>| async move {
            Json(json!({
                "isolated":true,"target":body["target"],
                "validation":[
                    {"command":"--help","exit_code":0,"stdout":"usage","stderr":""},
                    {"command":"accounts --json","exit_code":0,"stdout":"{\"app_id\":\"ring\"}","stderr":""},
                    {"command":"login status --json","exit_code":0,"stdout":"{\"authenticated\":false}","stderr":""}
                ]
            }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}
fn pack(dir: &Path, version: &str, script: &str) -> std::path::PathBuf {
    let target = silicon_apps_client::package::current_target().unwrap();
    let source = dir.join(format!("source-{version}"));
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("apps.yaml"),
        format!("app_id: ring\nversion: {version}\ncommand: ring\ntargets:\n  {target}:\n    binary: ring\n    install_script: install.sh\n"),
    )
    .unwrap();
    fs::write(source.join("ring"), format!("ring {version}")).unwrap();
    fs::write(source.join("install.sh"), script).unwrap();
    let output = dir.join(format!("ring-{version}.tar.gz"));
    fs::write(
        &output,
        silicon_apps_client::package::pack_directory(&source).unwrap(),
    )
    .unwrap();
    output
}

#[cfg(unix)]
#[tokio::test]
async fn authors_sign_and_withdraw_and_installers_verify_in_ci_mode() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let author_home = tempfile::tempdir().unwrap();
    let ci_home = tempfile::tempdir().unwrap();
    let runner = runner().await;
    let state = AppState::new(config(data.path(), &runner)).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let app = router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let alice = "dev:alice:c:alice";
    let bob = "dev:bob:c:bob";
    let target = silicon_apps_client::package::current_target().unwrap();
    let author = |args: &[&str]| {
        let mut command = cli(author_home.path(), &server, alice);
        command.args(args);
        command
    };

    // An author key: the private key stays in .apps/keys, owner-only.
    let (added, _) = run_ok(&mut author(&["keys", "add", "--name", "laptop"]), &[]).await;
    let key_id = added["key"]["key_id"].as_str().unwrap().to_owned();
    let private = added["private_key_path"].as_str().unwrap().to_owned();
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&private).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let (listed, _) = run_ok(&mut author(&["keys", "list"]), &[]).await;
    assert_eq!(listed["items"][0]["key_id"], key_id.as_str());
    assert_eq!(listed["items"][0]["private_key_path"], private.as_str());

    // Create the app and upload two signed versions.
    let description = "d".repeat(200);
    run_ok(
        &mut author(&[
            "create",
            "ring",
            "--name",
            "Ring",
            "--description",
            &description,
        ]),
        &[],
    )
    .await;
    let mut production = vec![];
    for (version, script) in [
        ("0.1.0", "#!/bin/sh\nexit 0\n"),
        ("0.2.0", "#!/bin/sh\necho second\n"),
    ] {
        let file = pack(work.path(), version, script);
        let (package, _) = run_ok(
            &mut author(&[
                "upload",
                "ring",
                "--target",
                target,
                file.to_str().unwrap(),
                "--sign-key",
                &key_id,
            ]),
            &[],
        )
        .await;
        assert_eq!(package["author_signature"]["key_id"], key_id.as_str());
        let (release, _) = run_ok(
            &mut author(&[
                "release",
                "ring",
                "--version",
                version,
                "--package",
                package["id"].as_str().unwrap(),
            ]),
            &[],
        )
        .await;
        assert_eq!(release["signed_by_author"], true);
        let prod_version = if version == "0.1.0" { "1.0.0" } else { "1.1.0" };
        let (promoted, _) = run_ok(
            &mut author(&[
                "promote",
                "ring",
                release["id"].as_str().unwrap(),
                "--version",
                prod_version,
            ]),
            &[],
        )
        .await;
        production.push(promoted["id"].as_str().unwrap().to_owned());
    }
    run_ok(&mut author(&["publish", "ring"]), &[]).await;
    let (page, _) = run_ok(&mut author(&["show", "ring"]), &[]).await;
    assert_eq!(page["signed"], true);
    assert_eq!(page["signed_by_author"], true);

    // A key that is not registered is refused, with a structured error.
    let stray = work.path().join("stray.key");
    fs::write(&stray, "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=\n").unwrap();
    let file = pack(work.path(), "0.3.0", "#!/bin/sh\nexit 0\n");
    let error = run_err(
        &mut author(&[
            "upload",
            "ring",
            "--target",
            target,
            file.to_str().unwrap(),
            "--sign-key",
            stray.to_str().unwrap(),
        ]),
        &[],
    )
    .await;
    assert_eq!(
        error["error"]["code"], "invalid_author_signature",
        "{error}"
    );
    assert_eq!(error["error"]["status"], 422);

    // CI: install without any updater, and read the install script first.
    let ci = |args: &[&str]| {
        let mut command = cli(ci_home.path(), &server, bob);
        command.env("SILICON_APPS_NO_DAEMON", "1").args(args);
        command
    };
    let (shown, _) = run_ok(&mut ci(&["show", "ring", "--install-script"]), &[]).await;
    assert_eq!(shown["version"], "1.1.0");
    assert_eq!(
        shown["install_script"]["content"],
        "#!/bin/sh\necho second\n"
    );
    assert_eq!(shown["signed_by"], "c:alice");
    let (installed, _) = run_ok(&mut ci(&["install", "ring"]), &[]).await;
    assert_eq!(installed["installed"]["version"], "1.1.0");
    assert_eq!(installed["installed"]["author_key_id"], key_id.as_str());
    assert_eq!(installed["updater"]["status"], "disabled");
    let (status, _) = run_ok(&mut ci(&["daemon", "status"]), &[]).await;
    assert_eq!(status["running"], false);
    let refused = run_err(&mut ci(&["daemon", "start"]), &[]).await;
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap()
            .contains("SILICON_APPS_NO_DAEMON=1"),
        "{refused}"
    );

    // The author withdraws 1.1.0; the next update moves to 1.0.0 and says why.
    let (withdrawn, _) = run_ok(
        &mut author(&[
            "withdraw",
            "ring",
            &production[1],
            "--reason",
            "1.1.0 echoes from its install script.",
        ]),
        &[],
    )
    .await;
    assert_eq!(withdrawn["replacement"]["version"], "1.0.0");
    let (updated, notices) = run_ok(&mut ci(&["update"]), &[]).await;
    assert_eq!(
        updated["items"][0]["status"], "replaced_withdrawn",
        "{updated}"
    );
    assert_eq!(updated["items"][0]["version"], "1.0.0");
    assert!(
        notices.contains("was withdrawn by its authors"),
        "{notices}"
    );
    assert!(notices.contains("changes its install script"), "{notices}");
    let error = run_err(&mut ci(&["install", "ring@1.1.0", "--yes"]), &[]).await;
    assert_eq!(error["error"]["code"], "release_withdrawn", "{error}");
    assert_eq!(error["error"]["status"], 410);

    // Revoking the key stops new signatures with it.
    let (revoked, _) = run_ok(
        &mut author(&["keys", "revoke", &key_id, "--reason", "rotated"]),
        &[],
    )
    .await;
    assert_eq!(revoked["key"]["status"], "revoked");
    let error = run_err(
        &mut author(&[
            "upload",
            "ring",
            "--target",
            target,
            file.to_str().unwrap(),
            "--sign-key",
            &key_id,
        ]),
        &[],
    )
    .await;
    assert_eq!(error["error"]["code"], "invalid_author_signature");
    assert_eq!(error["error"]["details"]["status"], "revoked");
    task.abort();
}
