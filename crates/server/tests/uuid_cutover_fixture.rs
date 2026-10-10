//! Opt-in local cutover fixture. Accounts authentication is real; package bytes are
//! deliberately seeded after inspection, so this is retention proof, not runner proof.
mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use silicon_apps_server::{
    AppState,
    model::*,
    router, signing,
    store::{Mutation, Prepared, hash},
};
use std::path::PathBuf;

#[tokio::test]
#[ignore = "requires a current local Accounts developer token and an empty isolated fixture directory"]
async fn prepare_real_accounts_cutover_fixture() {
    let dir = PathBuf::from(std::env::var("APPS_CUTOVER_DIR").unwrap());
    assert!(
        !dir.join("apps.sqlite").exists(),
        "refusing to replace a fixture"
    );
    std::fs::create_dir_all(&dir).unwrap();
    let token: Value = serde_json::from_slice(
        &std::fs::read(std::env::var("APPS_CUTOVER_TOKEN_FILE").unwrap()).unwrap(),
    )
    .unwrap();
    let token = token["access_token"].as_str().unwrap();
    let mut cfg = config(&dir);
    cfg.dev_auth = false;
    cfg.accounts_url = "http://localhost:9590".into();
    cfg.accounts_service_token = Some(
        std::fs::read_to_string(std::env::var("APPS_CUTOVER_SERVICE_TOKEN_FILE").unwrap())
            .unwrap()
            .trim()
            .into(),
    );
    let (key, _) = signing::generate("local-cutover");
    cfg.signing_keys = Some(key.clone());
    let key_path = dir.join("signing.key");
    std::fs::write(&key_path, &key).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let state = AppState::new(cfg).unwrap();
    let app = router(state.clone());
    let (status, me) = call(&app, "GET", "/v1/me", Some(token), json!({}), None).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    let (status, created) = call(&app, "POST", "/v1/apps", Some(token), json!({"app_id":"uuid-retention", "name":"UUID retention fixture", "description":"An isolated local signed package retained across the Accounts identifier cutover."}), Some("fixture-create")).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let catalog = state.store.lock().unwrap().catalog().unwrap();
    let author = &catalog.apps["uuid-retention"].authors[0];
    let who = Identity {
        uuid: author.uuid.clone(),
        id: author.id.clone(),
        display_name: author.display_name.clone(),
        verified_emails: vec![],
    };
    let bytes = package_bytes("uuid-retention", "macos-aarch64");
    silicon_apps_package::inspect_archive(&bytes).unwrap();
    let package = Package {
        id: "cutover-package".into(),
        target: "macos-aarch64".into(),
        sha256: hash(&bytes),
        size: bytes.len() as u64,
        command: "uuid-retention".into(),
        validation: vec![],
        created_at: now(),
        install_script: None,
        inspected: true,
        author_signature: None,
    };
    std::fs::write(dir.join("packages").join(&package.sha256), &bytes).unwrap();
    let body = json!({});
    state
        .store
        .lock()
        .unwrap()
        .mutate(Mutation {
            method: "POST",
            path: "apps/uuid-retention/packages/macos-aarch64",
            key: "fixture-package",
            who: Some(&who),
            body: &body,
            digest: &hash(body.to_string().as_bytes()),
            prepared: Prepared {
                package: Some(package),
                ..Default::default()
            },
        })
        .unwrap();
    let (status, release) = call(
        &app,
        "POST",
        "/v1/apps/uuid-retention/releases",
        Some(token),
        json!({"version":"0.1.0","package_ids":["cutover-package"]}),
        Some("fixture-release"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{release}");
    assert_eq!(
        release["signatures"]["cutover-package"]["algorithm"],
        "ed25519"
    );
    let (status, listing) = call(
        &app,
        "GET",
        "/v1/apps/uuid-retention",
        Some(token),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listing}");
    std::fs::write(
        dir.join("before.json"),
        serde_json::to_vec_pretty(
            &json!({"me":me,"app":listing,"release":release,"package_sha256":hash(&bytes)}),
        )
        .unwrap(),
    )
    .unwrap();
}
