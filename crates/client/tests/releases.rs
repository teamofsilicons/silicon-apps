//! Signed installs, install script transparency and withdrawn releases from
//! the client against a real Apps server.
use serde_json::json;
use silicon_apps_client::{Client, Config, LocalState, install, package, updater};
use silicon_apps_server::{
    AppState,
    config::Config as ServerConfig,
    model::{Identity, InstallScriptInfo, Package, now},
    router,
    store::{Mutation, Prepared, hash},
};
use std::fs;

fn server_config(dir: &std::path::Path) -> ServerConfig {
    ServerConfig {
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
fn archive(version: &str, script: &str) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let target = package::current_target().unwrap();
    let script_name = if cfg!(windows) {
        "setup.cmd"
    } else {
        "install.sh"
    };
    fs::write(
        dir.path().join("apps.yaml"),
        format!("app_id: fixture\nversion: {version}\ncommand: fixture\ntargets:\n  {target}:\n    binary: fixture\n    install_script: {script_name}\n"),
    )
    .unwrap();
    fs::write(dir.path().join("fixture"), format!("fixture {version}")).unwrap();
    fs::write(dir.path().join(script_name), script).unwrap();
    package::pack_directory(dir.path()).unwrap()
}

#[tokio::test]
async fn signed_installs_show_scripts_and_leave_withdrawn_releases() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(server_config(dir.path())).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let alice = Client::new(&base, Some("dev:alice:c:alice".into()))
        .unwrap()
        .with_telemetry(false);
    alice
        .create("fixture", "Fixture", &"d".repeat(200), None, None)
        .await
        .unwrap();
    let who = Identity {
        uuid: "alice".into(),
        id: "c:alice".into(),
        display_name: "alice".into(),
        verified_emails: vec![],
    };
    let target = package::current_target().unwrap();
    let mut production = vec![];
    let scripts = if cfg!(windows) {
        ["@echo off\r\nexit /b 0\r\n", "@echo off\r\necho second\r\n"]
    } else {
        ["#!/bin/sh\nexit 0\n", "#!/bin/sh\necho second\n"]
    };
    for (index, (version, script)) in [("0.1.0", scripts[0]), ("0.2.0", scripts[1])]
        .into_iter()
        .enumerate()
    {
        let bytes = archive(version, script);
        fs::write(dir.path().join("packages").join(hash(&bytes)), &bytes).unwrap();
        let manifest = package::inspect_archive(&bytes).unwrap();
        let (info, _) = package::install_script(&bytes, &manifest, target)
            .unwrap()
            .unwrap();
        let id = format!("pkg-{index}");
        let package = Package {
            id: id.clone(),
            target: target.into(),
            sha256: hash(&bytes),
            size: bytes.len() as u64,
            command: "fixture".into(),
            validation: vec![],
            created_at: now(),
            install_script: Some(InstallScriptInfo {
                path: info.path,
                sha256: info.sha256,
                size: info.size,
            }),
            inspected: true,
            author_signature: None,
        };
        let body = json!({});
        state
            .store
            .lock()
            .unwrap()
            .mutate(Mutation {
                method: "POST",
                path: &format!("apps/fixture/packages/{target}"),
                key: &format!("package-{index}"),
                who: Some(&who),
                body: &body,
                digest: &hash(b"{}"),
                prepared: Prepared {
                    package: Some(package),
                    ..Default::default()
                },
            })
            .unwrap();
        let dev = alice
            .action(
                "POST",
                "fixture",
                &["releases"],
                Some(json!({"version":version,"package_ids":[id]})),
                None,
            )
            .await
            .unwrap();
        let prod = alice
            .action(
                "POST",
                "fixture",
                &["releases", dev["id"].as_str().unwrap(), "promote"],
                Some(json!({"version":format!("1.{index}.0")})),
                None,
            )
            .await
            .unwrap();
        production.push(prod);
    }
    alice
        .action("POST", "fixture", &["publish"], Some(json!({})), None)
        .await
        .unwrap();

    // Anyone installs the latest production release; its signature checks out.
    let home = tempfile::tempdir().unwrap();
    let local = LocalState::new(home.path()).unwrap();
    let config = Config {
        server: base.clone(),
        ..Config::default()
    };
    let anyone = Client::new(&base, None).unwrap().with_telemetry(false);
    let keys = anyone.signing_keys().await.unwrap();
    assert_eq!(keys["active_key_id"], state.signer.active_id());
    let outcome = install::install(&anyone, &local, &config, &"fixture".parse().unwrap(), false)
        .await
        .unwrap();
    assert_eq!(outcome.installed.version, "1.1.0");
    assert_eq!(
        outcome.installed.signature_key_id.as_deref(),
        Some(state.signer.active_id())
    );
    // The install script can be read before anyone trusts it.
    let shown =
        install::inspect_install_script(&anyone, &local, &"fixture".parse().unwrap(), target)
            .await
            .unwrap();
    assert_eq!(shown["version"], "1.1.0");
    assert_eq!(shown["install_script"]["content"], scripts[1]);
    assert_eq!(
        shown["install_script"]["sha256"],
        package::sha256(scripts[1].as_bytes())
    );

    // The authors withdraw 1.1.0.
    let withdrawn = alice
        .withdraw_release(
            "fixture",
            production[1]["id"].as_str().unwrap(),
            "1.1.0 prints to stdout from its install script.",
            None,
        )
        .await
        .unwrap();
    assert_eq!(withdrawn["replacement"]["version"], "1.0.0");
    // An exact install of it is refused with the reason.
    let error = anyone
        .resolve(&"fixture@1.1.0".parse().unwrap(), target)
        .await
        .unwrap_err();
    let api = error
        .downcast_ref::<silicon_apps_client::ApiError>()
        .unwrap();
    assert_eq!(api.status, 410);
    assert_eq!(api.code, "release_withdrawn");
    assert_eq!(
        api.details["reason"],
        "1.1.0 prints to stdout from its install script."
    );
    // The next update check moves the installed copy off it, and says that
    // the install script differs.
    let result = updater::update(&anyone, &local, &config, None)
        .await
        .unwrap();
    let item = &result["items"][0];
    assert_eq!(item["status"], "replaced_withdrawn", "{result}");
    assert_eq!(item["version"], "1.0.0");
    assert_eq!(item["previous_version"], "1.1.0");
    assert!(
        item["install_script_notice"]
            .as_str()
            .unwrap()
            .contains("changes its install script"),
        "{result}"
    );
    assert_eq!(local.installed().unwrap()["fixture"].version, "1.0.0");
    server.abort();
}
