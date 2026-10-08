use axum::{
    Json, Router,
    extract::State,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::{Value, json};
use silicon_apps_client::{
    Client, Config, LocalState,
    install::{self, InstallSpec},
    package,
};
use std::{
    fs,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
struct Fixture {
    bytes: Vec<u8>,
    version: String,
    checksum: String,
}
async fn resolve(State(fixture): State<Arc<Mutex<Fixture>>>) -> Json<Value> {
    let f = fixture.lock().unwrap();
    Json(
        json!({"app_id":"fixture","release":{"id":format!("release-{}",f.version),"app_id":"fixture","channel":"production","version":f.version,"package_ids":["pkg"]},"package":{"id":"pkg","target":package::current_target().unwrap(),"sha256":f.checksum,"size":f.bytes.len(),"command":"fixture"},"download_path":"/package"}),
    )
}
async fn download(State(fixture): State<Arc<Mutex<Fixture>>>) -> impl IntoResponse {
    fixture.lock().unwrap().bytes.clone()
}
fn package_fixture(version: &str, script: Option<&str>) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let script_name = if cfg!(windows) {
        "scripts/setup script.cmd"
    } else {
        "install.sh"
    };
    let script_field = script
        .map(|_| format!("\n    install_script: {script_name}"))
        .unwrap_or_default();
    fs::write(dir.path().join("apps.yaml"),format!("app_id: fixture\nversion: {version}\ncommand: fixture\ntargets:\n  {}:\n    binary: fixture{script_field}\n",package::current_target().unwrap())).unwrap();
    fs::write(
        dir.path().join("fixture"),
        format!("fixture version {version}"),
    )
    .unwrap();
    if let Some(script) = script {
        let script_path = dir.path().join(script_name);
        fs::create_dir_all(script_path.parent().unwrap()).unwrap();
        fs::write(script_path, script).unwrap();
    }
    let bytes = package::pack_directory(dir.path()).unwrap();
    Fixture {
        checksum: package::sha256(&bytes),
        bytes,
        version: version.into(),
    }
}
async fn server(fixture: Arc<Mutex<Fixture>>) -> (Client, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/v1/apps/fixture/resolve", get(resolve))
        .route("/package", get(download))
        .route(
            "/v1/apps/fixture/installs",
            post(|| async { Json(json!({"installs":1})) }),
        )
        .with_state(fixture);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (
        Client::new(&format!("http://{address}"), None).unwrap(),
        server,
    )
}

#[tokio::test]
async fn verified_install_then_uninstall_updates_registry_and_command() {
    let fixture = Arc::new(Mutex::new(package_fixture("1.0.0", None)));
    let (client, server) = server(fixture).await;
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let spec: InstallSpec = "fixture".parse().unwrap();
    let outcome = install::install(&client, &state, &Config::default(), &spec, false)
        .await
        .unwrap();
    assert_eq!(outcome.installed.version, "1.0.0");
    assert!(outcome.count_recorded);
    assert_eq!(state.installed().unwrap()["fixture"].version, "1.0.0");
    assert!(state.root.join("installed/fixture/fixture").is_file());
    assert!(
        install::uninstall(&state, "fixture")
            .unwrap()
            .contains("apps review")
    );
    assert!(state.installed().unwrap().is_empty());
    assert!(!state.root.join("installed/fixture").exists());
    server.abort();
}
#[tokio::test]
async fn checksum_mismatch_does_not_install_untrusted_bytes() {
    let mut f = package_fixture("1.0.0", None);
    f.checksum = "0".repeat(64);
    let (client, server) = server(Arc::new(Mutex::new(f))).await;
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let error = install::install(
        &client,
        &state,
        &Config::default(),
        &"fixture".parse().unwrap(),
        false,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("checksum mismatch"));
    assert!(state.installed().unwrap().is_empty());
    assert!(!state.root.join("installed/fixture").exists());
    server.abort();
}
#[cfg(unix)]
#[tokio::test]
async fn failed_install_script_rolls_back_previous_files_registry_and_command() {
    let fixture = Arc::new(Mutex::new(package_fixture("1.0.0", None)));
    let (client, server) = server(fixture.clone()).await;
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let spec: InstallSpec = "fixture".parse().unwrap();
    install::install(&client, &state, &Config::default(), &spec, false)
        .await
        .unwrap();
    *fixture.lock().unwrap() = package_fixture("2.0.0", Some("exit 27\n"));
    let error = install::install(&client, &state, &Config::default(), &spec, false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("previous version was restored"));
    assert_eq!(state.installed().unwrap()["fixture"].version, "1.0.0");
    assert_eq!(
        fs::read_to_string(state.root.join("installed/fixture/fixture")).unwrap(),
        "fixture version 1.0.0"
    );
    assert_eq!(
        fs::read_to_string(state.root.join("bin/fixture")).unwrap(),
        "fixture version 1.0.0"
    );
    server.abort();
}
#[tokio::test]
async fn bundled_scripts_run_on_install_and_update_despite_legacy_false_setting() {
    let fixture = Arc::new(Mutex::new(package_fixture(
        "1.0.0",
        Some("echo first > script-ran.txt\n"),
    )));
    let (client, server) = server(fixture.clone()).await;
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let spec: InstallSpec = "fixture".parse().unwrap();
    install::install(&client, &state, &Config::default(), &spec, false)
        .await
        .unwrap();
    let marker = state.root.join("installed/fixture/script-ran.txt");
    assert_eq!(fs::read_to_string(&marker).unwrap().trim(), "first");
    let mut legacy = serde_json::to_value(state.installed().unwrap()).unwrap();
    assert!(legacy["fixture"].get("allow_install_script").is_none());
    legacy["fixture"]["allow_install_script"] = json!(false);
    fs::write(
        state.root.join("installed.json"),
        serde_json::to_vec(&legacy).unwrap(),
    )
    .unwrap();
    assert!(
        serde_json::to_value(state.installed().unwrap()).unwrap()["fixture"]
            .get("allow_install_script")
            .is_none()
    );
    *fixture.lock().unwrap() = package_fixture("2.0.0", Some("echo second > script-ran.txt\n"));
    let result = silicon_apps_client::updater::update(&client, &state, &Config::default(), None)
        .await
        .unwrap();
    assert_eq!(result["items"][0]["status"], "updated");
    assert_eq!(fs::read_to_string(&marker).unwrap().trim(), "second");
    let stored: Value =
        serde_json::from_slice(&fs::read(state.root.join("installed.json")).unwrap()).unwrap();
    assert!(stored["fixture"].get("allow_install_script").is_none());
    server.abort();
}
#[test]
fn concurrent_local_mutations_are_excluded() {
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let lock = state.lock("install").unwrap();
    assert!(state.lock("install").is_err());
    drop(lock);
    assert!(state.lock("install").is_ok());
}

#[tokio::test]
async fn download_cannot_send_credentials_to_another_origin() {
    let client = Client::new("https://apps.example.com", Some("secret-token".into())).unwrap();
    let resolution=serde_json::from_value(json!({"app_id":"fixture","release":{"id":"r","app_id":"fixture","channel":"production","version":"1.0.0"},"package":{"id":"p","target":"macos-aarch64","sha256":"0","size":1,"command":"fixture"},"download_path":"https://attacker.example/package"})).unwrap();
    assert!(
        client
            .download(&resolution)
            .await
            .unwrap_err()
            .to_string()
            .contains("another origin")
    );
}

#[tokio::test]
async fn install_count_survives_transport_failure_and_retries_with_same_key() {
    let fixture = Arc::new(Mutex::new(package_fixture("1.0.0", None)));
    let keys = Arc::new(Mutex::new(Vec::<String>::new()));
    let received = keys.clone();
    let app = Router::new()
        .route("/v1/apps/fixture/resolve", get(resolve))
        .route("/package", get(download))
        .route(
            "/v1/apps/fixture/installs",
            post(move |headers: axum::http::HeaderMap| {
                let keys = received.clone();
                async move {
                    let mut keys = keys.lock().unwrap();
                    keys.push(headers["Idempotency-Key"].to_str().unwrap().into());
                    if keys.len() == 1 {
                        (
                            axum::http::StatusCode::SERVICE_UNAVAILABLE,
                            Json(json!({"error":{"code":"unavailable","message":"try again"}})),
                        )
                    } else {
                        (axum::http::StatusCode::OK, Json(json!({"installs":1})))
                    }
                }
            }),
        )
        .with_state(fixture);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = Client::new(&format!("http://{address}"), None).unwrap();
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let outcome = install::install(
        &client,
        &state,
        &Config::default(),
        &"fixture".parse().unwrap(),
        false,
    )
    .await
    .unwrap();
    assert!(!outcome.count_recorded);
    assert_eq!(
        fs::read_dir(state.root.join("install-events"))
            .unwrap()
            .count(),
        1
    );
    silicon_apps_client::updater::update(&client, &state, &Config::default(), None)
        .await
        .unwrap();
    assert_eq!(
        fs::read_dir(state.root.join("install-events"))
            .unwrap()
            .count(),
        0
    );
    let delivered = keys.lock().unwrap();
    assert_eq!(delivered[0], delivered[1]);
    server.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn script_timeout_terminates_process_and_keeps_previous_install() {
    let fixture = Arc::new(Mutex::new(package_fixture("1.0.0", None)));
    let (client, server) = server(fixture.clone()).await;
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let spec: InstallSpec = "fixture".parse().unwrap();
    install::install(&client, &state, &Config::default(), &spec, false)
        .await
        .unwrap();
    *fixture.lock().unwrap() = package_fixture("2.0.0", Some("sleep 10\n"));
    let config = Config {
        install_script_timeout_seconds: 1,
        ..Config::default()
    };
    let started = std::time::Instant::now();
    let error = install::install(&client, &state, &config, &spec, false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("exceeded 1 seconds"));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    assert_eq!(state.installed().unwrap()["fixture"].version, "1.0.0");
    server.abort();
}

#[tokio::test]
async fn local_bootstrap_registers_for_updates_without_fabricating_registry_counts() {
    let f = package_fixture("1.0.0", Some("echo bootstrap > script-ran.txt\n"));
    let bytes = f.bytes.clone();
    let checksum = f.checksum.clone();
    let fixture = Arc::new(Mutex::new(f));
    let (client, server) = server(fixture).await;
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let config = Config {
        server: client.base_url().to_owned(),
        ..Config::default()
    };
    let spec: InstallSpec = "fixture".parse().unwrap();
    let receipt = install::install_local(
        &state,
        &config,
        &spec,
        install::LocalArchive {
            bytes,
            sha256: checksum,
        },
        false,
    )
    .await
    .unwrap();
    assert!(!receipt.count_recorded);
    assert_eq!(
        fs::read_to_string(state.root.join("installed/fixture/script-ran.txt"))
            .unwrap()
            .trim(),
        "bootstrap"
    );
    assert!(
        state.installed().unwrap()["fixture"]
            .release_id
            .starts_with("bootstrap:")
    );
    let result = silicon_apps_client::updater::update(&client, &state, &Config::default(), None)
        .await
        .unwrap();
    assert_eq!(result["items"][0]["status"], "updated");
    assert_eq!(
        state.installed().unwrap()["fixture"].release_id,
        "release-1.0.0"
    );
    server.abort();
}

#[tokio::test]
async fn changed_registry_never_silently_replaces_an_existing_app() {
    let (first, first_task) = server(Arc::new(Mutex::new(package_fixture("1.0.0", None)))).await;
    let (second, second_task) = server(Arc::new(Mutex::new(package_fixture("9.0.0", None)))).await;
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let spec: InstallSpec = "fixture".parse().unwrap();
    install::install(&first, &state, &Config::default(), &spec, false)
        .await
        .unwrap();
    let result = silicon_apps_client::updater::update(&second, &state, &Config::default(), None)
        .await
        .unwrap();
    assert_eq!(result["items"][0]["status"], "failed");
    assert!(
        result["items"][0]["error"]
            .as_str()
            .unwrap()
            .contains("cannot update")
    );
    assert_eq!(state.installed().unwrap()["fixture"].version, "1.0.0");
    assert!(
        install::install(&second, &state, &Config::default(), &spec, false)
            .await
            .unwrap_err()
            .to_string()
            .contains("explicit confirmation")
    );
    install::install(&second, &state, &Config::default(), &spec, true)
        .await
        .unwrap();
    let installed = state.installed().unwrap();
    assert_eq!(installed["fixture"].version, "9.0.0");
    assert_eq!(
        installed["fixture"].server,
        silicon_apps_client::auth::service_scope(second.base_url()).unwrap()
    );
    first_task.abort();
    second_task.abort();
}

#[tokio::test]
async fn legacy_unscoped_install_does_not_guess_an_update_registry() {
    let (client, task) = server(Arc::new(Mutex::new(package_fixture("1.0.0", None)))).await;
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let spec: InstallSpec = "fixture".parse().unwrap();
    install::install(&client, &state, &Config::default(), &spec, false)
        .await
        .unwrap();
    let mut records = state.installed().unwrap();
    records.get_mut("fixture").unwrap().server.clear();
    state.save_installed(&records).unwrap();
    let result = silicon_apps_client::updater::update(&client, &state, &Config::default(), None)
        .await
        .unwrap();
    assert_eq!(result["items"][0]["status"], "failed");
    assert!(
        result["items"][0]["error"]
            .as_str()
            .unwrap()
            .contains("unscoped legacy")
    );
    task.abort();
}

#[tokio::test]
async fn telemetry_opt_out_survives_auth_and_covers_json_upload_and_download() {
    let fixture = Arc::new(Mutex::new(package_fixture("1.0.0", None)));
    let bytes = fixture.lock().unwrap().bytes.clone();
    let payload = bytes.clone();
    let app = Router::new()
        .route(
            "/v1/me",
            get(|headers: axum::http::HeaderMap| async move {
                Json(json!({"telemetry":headers["X-Apps-Telemetry"].to_str().unwrap()}))
            }),
        )
        .route(
            "/v1/apps/fixture/packages/{target}",
            post(|headers: axum::http::HeaderMap| async move {
                Json(json!({"telemetry":headers["X-Apps-Telemetry"].to_str().unwrap()}))
            }),
        )
        .route("/v1/apps/fixture/resolve", get(resolve))
        .route(
            "/package",
            get(move |headers: axum::http::HeaderMap| {
                let bytes = payload.clone();
                async move {
                    assert_eq!(headers["X-Apps-Telemetry"], "off");
                    bytes
                }
            }),
        )
        .with_state(fixture);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = Client::new(&format!("http://{address}"), None)
        .unwrap()
        .with_telemetry(false)
        .authenticated(Some("test-token".into()));
    assert_eq!(client.me().await.unwrap()["telemetry"], "off");
    assert_eq!(
        client
            .upload(
                "fixture",
                package::current_target().unwrap(),
                bytes.clone(),
                None
            )
            .await
            .unwrap()["telemetry"],
        "off"
    );
    let resolved = client
        .resolve(
            &"fixture".parse().unwrap(),
            package::current_target().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(client.download(&resolved).await.unwrap(), bytes);
    assert_eq!(
        client.with_telemetry(true).me().await.unwrap()["telemetry"],
        "on"
    );
    task.abort();
}
