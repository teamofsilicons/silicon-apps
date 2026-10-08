use serde_json::{Value, json};
use silicon_apps_client::{LocalState, install::Installed};
use std::collections::BTreeMap;
use tokio::process::Command;

fn apps(home: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_apps"));
    command.args(["--json", "--home"]).arg(home);
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
    command
}

#[tokio::test]
async fn malformed_arguments_return_actionable_json_and_nonzero_status() {
    let home = tempfile::tempdir().unwrap();
    let output = apps(home.path()).arg("install").output().await.unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "invalid_arguments");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("<APP>")
    );
}

#[tokio::test]
async fn docs_are_bundled_machine_readable_and_include_the_traversable_command_tree() {
    let home = tempfile::tempdir().unwrap();
    for topic in [
        "start", "publish", "manifest", "install", "auth", "why", "tree",
    ] {
        let output = apps(home.path())
            .args(["docs", topic])
            .output()
            .await
            .unwrap();
        assert!(output.status.success());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["topic"], topic);
        let content = value["content"].as_str().unwrap();
        assert!(content.len() > 100);
        if topic == "tree" {
            assert!(content.contains("apps authors invite"));
            assert!(content.contains("apps config telemetry"));
            assert!(content.contains("--idempotency-key"));
        } else {
            assert_eq!(content, silicon_apps_client::docs::guide(topic));
        }
    }
    assert!(
        !home.path().join(".apps").exists(),
        "reading docs created local state"
    );
}

#[tokio::test]
async fn daemon_once_exits_unsuccessfully_when_an_app_cannot_be_updated() {
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let item = Installed {
        app_id: "fixture".into(),
        channel: "production".into(),
        version: "1.0.0".into(),
        target: silicon_apps_client::package::current_target()
            .unwrap()
            .into(),
        command: "fixture".into(),
        release_id: "release-1".into(),
        package_id: "package-1".into(),
        sha256: "0".repeat(64),
        installed_at: "2026-01-01T00:00:00Z".into(),
        server: server.clone(),
        allow_install_script: false,
    };
    state
        .save_installed(&BTreeMap::from([("fixture".into(), item)]))
        .unwrap();
    let output = apps(home.path())
        .args(["--server", &server, "daemon", "run", "--once"])
        .output()
        .await
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["result"]["items"][0]["status"], "failed");
    assert!(
        value["result"]["items"][0]["error"]
            .as_str()
            .unwrap()
            .contains("could not reach")
    );
    assert_eq!(
        value,
        serde_json::from_slice::<Value>(&std::fs::read(state.root.join("updater.json")).unwrap())
            .unwrap()
    );
}

#[tokio::test]
async fn validation_returns_all_schema_and_semantic_errors_together() {
    let home = tempfile::tempdir().unwrap();
    let package = tempfile::tempdir().unwrap();
    std::fs::write(package.path().join("apps.yaml"), "app_id: []\nversion: invalid\nunknown: true\ntargets:\n  linux-x86_64:\n    binary: missing\n    install_script: 17\n").unwrap();
    let output = apps(home.path())
        .arg("validate")
        .arg(package.path())
        .output()
        .await
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], false);
    let errors = report["errors"].as_array().unwrap();
    for name in [
        "app_id:",
        "version:",
        "command:",
        "unknown",
        "install_script:",
        "existing regular file",
    ] {
        assert!(
            errors
                .iter()
                .any(|error| error.as_str().unwrap().contains(name)),
            "missing {name}: {report}"
        );
    }
    assert_ne!(report, json!({"valid": true}));
}

#[cfg(unix)]
#[tokio::test]
async fn unix_daemon_self_update_executes_new_binary_with_same_supervised_pid() {
    use axum::{
        Json, Router,
        routing::{get, post},
    };
    use silicon_apps_client::{Config, install, package};
    use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

    fn archive(version: &str, script: &str) -> Vec<u8> {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("apps.yaml"), format!("app_id: apps\nversion: {version}\ncommand: apps\ntargets:\n  {}:\n    binary: apps\n", package::current_target().unwrap())).unwrap();
        fs::write(dir.path().join("apps"), script).unwrap();
        fs::set_permissions(dir.path().join("apps"), fs::Permissions::from_mode(0o755)).unwrap();
        package::pack_directory(dir.path()).unwrap()
    }
    let updated = archive(
        "2.0.0",
        "#!/bin/sh\nprintf '%s\\n%s\\n' \"$$\" \"$*\" > \"$APPS_HANDOFF_MARKER\"\n",
    );
    let resolution = json!({
        "app_id":"apps",
        "release":{"id":"release-2","app_id":"apps","channel":"production","version":"2.0.0","package_ids":["package-2"]},
        "package":{"id":"package-2","target":package::current_target().unwrap(),"sha256":package::sha256(&updated),"size":updated.len(),"command":"apps"},
        "download_path":"/package"
    });
    let router = Router::new()
        .route(
            "/v1/apps/apps/resolve",
            get(move || {
                let value = resolution.clone();
                async move { Json(value) }
            }),
        )
        .route(
            "/package",
            get(move || {
                let bytes = updated.clone();
                async move { bytes }
            }),
        )
        .route(
            "/v1/apps/apps/installs",
            post(|| async { Json(json!({"installs":1})) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let config = Config {
        server: server_url.clone(),
        ..Config::default()
    };
    state.save_config(&config).unwrap();
    let original = archive("1.0.0", "#!/bin/sh\nexit 1\n");
    install::install_local(
        &state,
        &config,
        &"apps".parse().unwrap(),
        install::LocalArchive {
            sha256: package::sha256(&original),
            bytes: original,
        },
        false,
        false,
    )
    .await
    .unwrap();
    let marker = home.path().join("new-updater-started");
    let mut child = apps(home.path())
        .args(["daemon", "run"])
        .env("APPS_HANDOFF_MARKER", &marker)
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let original_pid = child.id().unwrap();
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success());
    let receipt = fs::read_to_string(marker).unwrap();
    let mut lines = receipt.lines();
    assert_eq!(lines.next().unwrap().parse::<u32>().unwrap(), original_pid);
    let arguments = lines.next().unwrap();
    assert!(arguments.contains(&format!("--server {server_url}")));
    assert!(arguments.ends_with("daemon run"));
    assert_eq!(state.installed().unwrap()["apps"].version, "2.0.0");
    assert_eq!(
        silicon_apps_client::updater::status(&state).unwrap()["running"],
        false
    );
    server.abort();
}
