//! Signed releases, key rotation, author keys and withdrawn releases.
mod common;

use axum::{Json, Router, body::Body, http::Request, routing::post};
use common::*;
use serde_json::{Value, json};
use silicon_apps_server::{
    AppState, Shared,
    config::Config,
    router,
    signing::{self, Keyring, ReleaseFields},
    store::Prepared,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;

fn seed() -> String {
    signing::generate("unused")
        .0
        .split_once(':')
        .unwrap()
        .1
        .to_owned()
}
fn public_of(id: &str, seed: &str) -> String {
    Keyring::parse(&format!("{id}:{seed}"), &[])
        .unwrap()
        .active_public_key()
}
fn state_with(dir: &std::path::Path, keys: &str, revoked: &[&str]) -> Shared {
    let mut cfg: Config = config(dir);
    cfg.signing_keys = Some(keys.into());
    cfg.revoked_signing_keys = revoked.iter().map(|s| s.to_string()).collect();
    AppState::new(cfg).unwrap()
}
async fn get(app: &Router, path: &str, token: Option<&str>) -> Response {
    send(app, "GET", path, token, None, None, &[]).await
}
/// Check a resolution's signature with a public key, from the signed fields.
fn verifies(resolution: &Value, public_key: &str) -> bool {
    let m = &resolution["signature"]["manifest"];
    let message = signing::release_message(&ReleaseFields {
        app_id: m["app_id"].as_str().unwrap(),
        target: m["target"].as_str().unwrap(),
        version: m["version"].as_str().unwrap(),
        channel: m["channel"].as_str().unwrap(),
        sha256: m["sha256"].as_str().unwrap(),
        size: m["size"].as_u64().unwrap(),
        release_id: m["release_id"].as_str().unwrap(),
        install_script_sha256: m["install_script_sha256"].as_str(),
    });
    signing::verify(
        public_key,
        &message,
        resolution["signature"]["signature"].as_str().unwrap(),
    )
}
fn key<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["key_id"] == id)
        .unwrap_or_else(|| panic!("{id} is not in {document}"))
}

#[tokio::test]
async fn releases_are_signed_and_keys_rotate_with_endorsements() {
    let dir = tempfile::tempdir().unwrap();
    let (s1, s2, s3) = (seed(), seed(), seed());
    let (pk1, pk2, pk3) = (
        public_of("k1", &s1),
        public_of("k2", &s2),
        public_of("k3", &s3),
    );
    let state = state_with(dir.path(), &format!("k1:{s1}"), &[]);
    let (package_id, release_id) = seed_app(&state, "alice", "ring", &[], "linux-x86_64", true);
    let app = router(state.clone());

    // The development release was signed when it was created.
    let releases = get(&app, "/v1/apps/ring/releases", None).await.body;
    let release = &releases["items"][0];
    assert_eq!(release["id"], release_id.as_str());
    assert_eq!(release["signatures"][&package_id]["key_id"], "k1");

    // Promotion signs the production release, with its own manifest.
    let (status, promoted) = call(
        &app,
        "POST",
        &format!("/v1/apps/ring/releases/{release_id}/promote"),
        Some("dev:alice:c:alice"),
        json!({"version":"1.0.0"}),
        Some("promote-ring-1"),
    )
    .await;
    assert_eq!(status, 200, "{promoted}");
    assert_eq!(promoted["signatures"][&package_id]["algorithm"], "ed25519");

    let keys = get(&app, signing::KEYS_PATH, None).await;
    assert_eq!(keys.status, 200);
    assert_eq!(keys.body["active_key_id"], "k1");
    assert_eq!(key(&keys.body, "k1")["public_key"], pk1.as_str());
    assert_eq!(key(&keys.body, "k1")["status"], "active");

    let resolved = get(&app, "/v1/apps/ring/resolve?target=linux-x86_64", None)
        .await
        .body;
    assert_eq!(resolved["signature"]["key_id"], "k1");
    assert_eq!(resolved["signature"]["manifest"]["channel"], "production");
    assert_eq!(resolved["signature"]["manifest"]["version"], "1.0.0");
    assert_eq!(
        resolved["signature"]["manifest"]["sha256"],
        resolved["package"]["sha256"]
    );
    assert!(verifies(&resolved, &pk1));
    assert!(!verifies(&resolved, &pk2), "another key must not verify");
    let mut tampered = resolved.clone();
    tampered["signature"]["manifest"]["sha256"] = json!("0".repeat(64));
    assert!(!verifies(&tampered, &pk1), "changed bytes must not verify");
    let mut tampered = resolved.clone();
    tampered["signature"]["manifest"]["version"] = json!("9.9.9");
    assert!(
        !verifies(&tampered, &pk1),
        "a changed version must not verify"
    );
    drop(app);
    drop(state);

    // Rotate: k2 signs, k1 stays configured long enough to endorse it.
    let state = state_with(dir.path(), &format!("k2:{s2},k1:{s1}"), &[]);
    let app = router(state.clone());
    let keys = get(&app, signing::KEYS_PATH, None).await.body;
    assert_eq!(keys["active_key_id"], "k2");
    assert_eq!(keys["keys"][0]["key_id"], "k2");
    assert_eq!(key(&keys, "k1")["status"], "retired");
    let endorsement = key(&keys, "k2")["endorsements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["key_id"] == "k1")
        .unwrap()
        .clone();
    assert!(signing::verify(
        &pk1,
        &signing::endorsement_message("k2", &pk2),
        endorsement["signature"].as_str().unwrap()
    ));
    let resolved = get(&app, "/v1/apps/ring/resolve?target=linux-x86_64", None)
        .await
        .body;
    assert_eq!(resolved["signature"]["key_id"], "k2", "re-signed at start");
    assert!(verifies(&resolved, &pk2));
    drop(app);
    drop(state);

    // Rotate again and revoke k1. Endorsements are kept after a key's
    // secret is removed, so the chain k1 -> k2 -> k3 stays checkable.
    let state = state_with(dir.path(), &format!("k3:{s3},k2:{s2}"), &["k1"]);
    drop(state);
    let state = state_with(dir.path(), &format!("k3:{s3}"), &["k1"]);
    let app = router(state.clone());
    let keys = get(&app, signing::KEYS_PATH, None).await.body;
    assert_eq!(keys["active_key_id"], "k3");
    assert_eq!(keys["revoked"], json!(["k1"]));
    assert_eq!(key(&keys, "k1")["status"], "revoked");
    assert_eq!(key(&keys, "k2")["status"], "retired");
    assert_eq!(key(&keys, "k2")["public_key"], pk2.as_str());
    let by_k2 = key(&keys, "k3")["endorsements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["key_id"] == "k2")
        .unwrap()
        .clone();
    assert!(signing::verify(
        &pk2,
        &signing::endorsement_message("k3", &pk3),
        by_k2["signature"].as_str().unwrap()
    ));
    let resolved = get(&app, "/v1/apps/ring/resolve?target=linux-x86_64", None)
        .await
        .body;
    assert!(verifies(&resolved, &pk3));
    drop(app);
    drop(state);

    // A key ID can never be reused with another key.
    let mut cfg = config(dir.path());
    cfg.signing_keys = Some(format!("k2:{}", seed()));
    let error = AppState::new(cfg).err().unwrap();
    assert_eq!(error.code, "signing_key_id_reused");
}

#[tokio::test]
async fn a_deployed_service_will_not_start_without_signing_keys() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.dev_auth = false;
    cfg.public_url = "https://apps.example.test".into();
    assert_eq!(
        AppState::new(cfg.clone()).err().unwrap().code,
        "signing_keys_required"
    );
    cfg.signing_keys = Some("bad".into());
    assert_eq!(
        AppState::new(cfg).err().unwrap().code,
        "invalid_signing_keys"
    );
    // Local development generates and keeps one key.
    let local = tempfile::tempdir().unwrap();
    let first = AppState::new(config(local.path())).unwrap();
    let id = first.signer.active_id().to_owned();
    assert_eq!(first.signer.source, "generated");
    drop(first);
    let again = AppState::new(config(local.path())).unwrap();
    assert_eq!(again.signer.active_id(), id);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(local.path().join("signing-keys"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[tokio::test]
async fn withdrawn_releases_are_never_served_and_installs_fall_back() {
    let (_dir, state, app) = fresh();
    let alice = "dev:alice:c:alice";
    let (first_package, first_dev) = seed_app(&state, "alice", "ring", &[], "linux-x86_64", true);
    let promote = |release: String, version: &'static str, key: &'static str| {
        let app = app.clone();
        async move {
            let (status, body) = call(
                &app,
                "POST",
                &format!("/v1/apps/ring/releases/{release}/promote"),
                Some(alice),
                json!({"version":version}),
                Some(key),
            )
            .await;
            assert_eq!(status, 200, "{body}");
            body["id"].as_str().unwrap().to_owned()
        }
    };
    promote(first_dev.clone(), "1.0.0", "promote-1").await;
    // A second package and release.
    let bytes = "second package for ring";
    let second = silicon_apps_server::model::Package {
        id: "ring-second".into(),
        target: "linux-x86_64".into(),
        sha256: silicon_apps_server::store::hash(bytes.as_bytes()),
        size: bytes.len() as u64,
        command: "ring".into(),
        validation: vec![],
        created_at: silicon_apps_server::model::now(),
        install_script: None,
        inspected: true,
        author_signature: None,
    };
    std::fs::write(
        state.config.data_dir.join("packages").join(&second.sha256),
        bytes,
    )
    .unwrap();
    mutate(
        &state,
        "alice",
        "POST",
        "apps/ring/packages/linux-x86_64",
        "second-package",
        json!({}),
        Prepared {
            package: Some(second),
            ..Default::default()
        },
    );
    let second_dev = mutate(
        &state,
        "alice",
        "POST",
        "apps/ring/releases",
        "second-release",
        json!({"version":"0.2.0","package_ids":["ring-second"]}),
        Prepared::default(),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let bad = promote(second_dev.clone(), "1.1.0", "promote-2").await;
    let latest = get(&app, "/v1/apps/ring/resolve?target=linux-x86_64", None).await;
    assert_eq!(latest.body["release"]["version"], "1.1.0");

    // Only authors withdraw, and they say why.
    let path = format!("/v1/apps/ring/releases/{bad}/withdraw");
    let (status, _) = call(
        &app,
        "POST",
        &path,
        Some("dev:bob:c:bob"),
        json!({"reason":"no"}),
        Some("bob-withdraw"),
    )
    .await;
    assert_eq!(status, 403);
    let (status, body) = call(
        &app,
        "POST",
        &path,
        Some(alice),
        json!({}),
        Some("no-reason"),
    )
    .await;
    assert_eq!(status, 400, "{body}");
    let (status, withdrawn) = call(
        &app,
        "POST",
        &path,
        Some(alice),
        json!({"reason":"1.1.0 deletes the config file on start."}),
        Some("withdraw-1.1.0"),
    )
    .await;
    assert_eq!(status, 200, "{withdrawn}");
    assert_eq!(
        withdrawn["withdrawn"]["reason"],
        "1.1.0 deletes the config file on start."
    );
    assert_eq!(withdrawn["withdrawn"]["by_id"], "c:alice");
    assert_eq!(withdrawn["replacement"]["version"], "1.0.0");
    // The same request again replays; a new one conflicts.
    let (status, replay) = call(
        &app,
        "POST",
        &path,
        Some(alice),
        json!({"reason":"1.1.0 deletes the config file on start."}),
        Some("withdraw-1.1.0"),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(replay, withdrawn);
    let (status, again) = call(
        &app,
        "POST",
        &path,
        Some(alice),
        json!({"reason":"again"}),
        Some("withdraw-again"),
    )
    .await;
    assert_eq!(status, 409, "{again}");
    assert_eq!(again["error"]["code"], "release_withdrawn");

    // Installs fall back to the previous good release on the channel.
    let latest = get(&app, "/v1/apps/ring/resolve?target=linux-x86_64", None).await;
    assert_eq!(latest.status, 200);
    assert_eq!(latest.body["release"]["version"], "1.0.0");
    assert_eq!(latest.body["withdrawn"][0]["version"], "1.1.0");
    assert_eq!(latest.body["withdrawn"][0]["release_id"], bad.as_str());
    let exact = get(
        &app,
        "/v1/apps/ring/resolve?target=linux-x86_64&version=1.1.0",
        None,
    )
    .await;
    assert_eq!(exact.status, 410, "{}", exact.body);
    assert_eq!(exact.body["error"]["code"], "release_withdrawn");
    assert_eq!(
        exact.body["error"]["details"]["reason"],
        "1.1.0 deletes the config file on start."
    );
    assert_eq!(
        exact.body["error"]["details"]["replacement"]["version"],
        "1.0.0"
    );

    // The app page shows it, the history keeps it, and streams carry it.
    let page = get(&app, "/v1/apps/ring", None).await.body;
    assert_eq!(page["latest_production"]["version"], "1.0.0");
    assert_eq!(page["withdrawn_releases"][0]["version"], "1.1.0");
    assert_eq!(page["withdrawn_releases"][0]["withdrawn_by"], "c:alice");
    assert_eq!(page["signed"], true);
    let history = get(&app, "/v1/apps/ring/history", Some(alice)).await.body;
    let entry = history["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "release.withdrawn")
        .unwrap()
        .clone();
    assert_eq!(entry["idempotency_key"], "withdraw-1.1.0");
    assert_eq!(entry["data"]["channel"], "production");
    let events = get(
        &app,
        "/v1/apps/ring/events?types=release.withdrawn",
        Some(alice),
    )
    .await
    .body;
    assert_eq!(events["items"].as_array().unwrap().len(), 1);

    // A package still in a good release is served; once every release
    // holding it is withdrawn, only authors can download it.
    let download = "/v1/apps/ring/packages/ring-second/download";
    let bob = "dev:bob:c:bob";
    assert_eq!(get(&app, download, Some(bob)).await.status, 200);
    let (status, _) = call(
        &app,
        "POST",
        &format!("/v1/apps/ring/releases/{second_dev}/withdraw"),
        Some(alice),
        json!({"reason":"Same bug as 1.1.0."}),
        Some("withdraw-0.2.0"),
    )
    .await;
    assert_eq!(status, 200);
    let refused = get(&app, download, Some(bob)).await;
    assert_eq!(refused.status, 410);
    assert_eq!(refused.body["error"]["code"], "release_withdrawn");
    assert_eq!(get(&app, download, Some(alice)).await.status, 200);
    // A withdrawn development release cannot be promoted.
    let (status, body) = call(
        &app,
        "POST",
        &format!("/v1/apps/ring/releases/{second_dev}/promote"),
        Some(alice),
        json!({"version":"1.2.0"}),
        Some("promote-withdrawn"),
    )
    .await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["error"]["code"], "release_withdrawn");
    let dev = get(
        &app,
        "/v1/apps/ring/resolve?target=linux-x86_64&channel=development",
        None,
    )
    .await;
    assert_eq!(dev.body["release"]["id"], first_dev.as_str());
    assert_eq!(dev.body["package"]["id"], first_package.as_str());
}

/// A runner that counts how often it was asked to validate.
async fn counting_runner(app_id: &'static str) -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let runner = Router::new().route(
        "/validate",
        post(move |Json(body): Json<Value>| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Json(json!({
                    "isolated":true,
                    "target":body["target"],
                    "validation":[
                        {"command":"--help","exit_code":0,"stdout":"usage","stderr":""},
                        {"command":"accounts --json","exit_code":0,"stdout":json!({"app_id":app_id}).to_string(),"stderr":""},
                        {"command":"login status --json","exit_code":0,"stdout":"{\"authenticated\":false}","stderr":""}
                    ]
                }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, runner).await.unwrap() });
    (url, calls)
}
/// A package with an install script.
fn scripted_package(app_id: &str, target: &str, script: &str) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("bin")).unwrap();
    std::fs::create_dir_all(dir.path().join("scripts")).unwrap();
    std::fs::write(
        dir.path().join("apps.yaml"),
        format!(
            "schema_version: 1\napp_id: {app_id}\nversion: 0.1.0\ncommand: {app_id}\ntargets:\n  {target}:\n    binary: bin/{app_id}\n    install_script: scripts/install.sh\n"
        ),
    )
    .unwrap();
    let binary = dir.path().join("bin").join(app_id);
    std::fs::write(&binary, "#!/bin/sh\necho demo\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::write(dir.path().join("scripts/install.sh"), script).unwrap();
    silicon_apps_package::pack_directory(dir.path()).unwrap()
}
async fn upload(
    app: &Router,
    bytes: &[u8],
    key: &str,
    signature: Option<(&str, &str)>,
) -> Response {
    let mut request = Request::builder()
        .method("POST")
        .uri("/v1/apps/demo-app/packages/linux-x86_64")
        .header("Authorization", "Bearer dev:alice:c:alice")
        .header("Content-Type", "application/gzip")
        .header("Idempotency-Key", key);
    if let Some((key_id, signature)) = signature {
        request = request
            .header("X-Apps-Author-Key-Id", key_id)
            .header("X-Apps-Author-Signature", signature);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(bytes.to_vec())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    Response {
        status,
        headers,
        body: serde_json::from_slice(&body).unwrap_or(Value::Null),
    }
}

#[tokio::test]
async fn author_keys_sign_uploads_and_bad_signatures_are_refused_before_the_runner() {
    let dir = tempfile::tempdir().unwrap();
    let (runner, runs) = counting_runner("demo-app").await;
    let mut cfg = config(dir.path());
    cfg.runner_url = Some(runner);
    cfg.runner_token = Some("runner-token-runner-token-runner-token".into());
    cfg.runner_targets = vec!["linux-x86_64".into()];
    let state = AppState::new(cfg).unwrap();
    let app = router(state.clone());
    seed_app(&state, "alice", "demo-app", &[], "linux-x86_64", true);
    let alice = "dev:alice:c:alice";
    let bob = "dev:bob:c:bob";

    // Alice registers a key. Its ID comes from the key itself.
    let author = Keyring::parse(&signing::generate("author").0, &[]).unwrap();
    let public = author.active_public_key();
    let (status, created) = call(
        &app,
        "POST",
        "/v1/keys",
        Some(alice),
        json!({"public_key":public,"name":"build machine"}),
        Some("add-alice-key"),
    )
    .await;
    assert_eq!(status, 201, "{created}");
    let key_id = created["key"]["key_id"].as_str().unwrap().to_owned();
    assert_eq!(key_id, signing::author_key_id(&public).unwrap());
    assert_eq!(created["key"]["status"], "active");
    let (status, dup) = call(
        &app,
        "POST",
        "/v1/keys",
        Some(alice),
        json!({"public_key":public}),
        Some("add-alice-key-again"),
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(dup["error"]["code"], "author_key_exists");
    let (status, bad) = call(
        &app,
        "POST",
        "/v1/keys",
        Some(alice),
        json!({"public_key":"not a key"}),
        Some("add-bad-key"),
    )
    .await;
    assert_eq!(status, 400, "{bad}");
    assert_eq!(
        get(&app, "/v1/keys", Some(alice)).await.body["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        get(&app, "/v1/keys", Some(bob)).await.body["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(get(&app, "/v1/keys", None).await.status, 401);

    let script = "#!/bin/sh\necho configured\n";
    let bytes = scripted_package("demo-app", "linux-x86_64", script);
    let digest = silicon_apps_package::sha256(&bytes);
    let script_digest = silicon_apps_package::sha256(script.as_bytes());
    let message = signing::author_message(
        "demo-app",
        "linux-x86_64",
        &digest,
        bytes.len() as u64,
        Some(&script_digest),
    );

    // A signature over anything else is refused before the runner runs.
    let (_, wrong) = author.sign(&signing::author_message(
        "demo-app",
        "linux-x86_64",
        &digest,
        bytes.len() as u64,
        None,
    ));
    let refused = upload(&app, &bytes, "upload-wrong", Some((&key_id, &wrong))).await;
    assert_eq!(refused.status, 422, "{}", refused.body);
    assert_eq!(refused.body["error"]["code"], "invalid_author_signature");
    assert_eq!(runs.load(Ordering::SeqCst), 0);
    // Another key signing the right message is refused too.
    let other = Keyring::parse(&signing::generate("other").0, &[]).unwrap();
    let (_, forged) = other.sign(&message);
    let refused = upload(&app, &bytes, "upload-forged", Some((&key_id, &forged))).await;
    assert_eq!(refused.status, 422);
    // A key that belongs to someone else cannot be used.
    let (status, bobs) = call(
        &app,
        "POST",
        "/v1/keys",
        Some(bob),
        json!({"public_key":other.active_public_key()}),
        Some("add-bob-key"),
    )
    .await;
    assert_eq!(status, 201);
    let refused = upload(
        &app,
        &bytes,
        "upload-bobs-key",
        Some((bobs["key"]["key_id"].as_str().unwrap(), &forged)),
    )
    .await;
    assert_eq!(refused.status, 422);
    assert_eq!(refused.body["error"]["details"]["status"], "unknown");
    assert_eq!(runs.load(Ordering::SeqCst), 0);

    // The right signature is accepted and kept with the package.
    let (_, good) = author.sign(&message);
    let accepted = upload(&app, &bytes, "upload-signed", Some((&key_id, &good))).await;
    assert_eq!(accepted.status, 200, "{}", accepted.body);
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    let package = accepted.body.clone();
    assert_eq!(package["author_signature"]["key_id"], key_id.as_str());
    assert_eq!(package["author_signature"]["signer_id"], "c:alice");
    assert_eq!(package["install_script"]["path"], "scripts/install.sh");
    assert_eq!(package["install_script"]["sha256"], script_digest.as_str());
    assert_eq!(package["inspected"], true);

    let (status, release) = call(
        &app,
        "POST",
        "/v1/apps/demo-app/releases",
        Some(alice),
        json!({"version":"0.2.0","package_ids":[package["id"]]}),
        Some("signed-release"),
    )
    .await;
    assert_eq!(status, 200, "{release}");
    assert_eq!(release["signed_by_author"], true);
    let page = get(&app, "/v1/apps/demo-app", None).await.body;
    assert_eq!(page["latest_development"]["version"], "0.2.0");
    assert_eq!(page["signed_by_author"], true);
    assert_eq!(page["signed_by"], json!(["c:alice"]));
    let resolved = get(
        &app,
        "/v1/apps/demo-app/resolve?target=linux-x86_64&channel=development",
        None,
    )
    .await
    .body;
    assert_eq!(resolved["install_script"]["sha256"], script_digest.as_str());
    assert_eq!(
        resolved["signature"]["manifest"]["install_script_sha256"],
        script_digest.as_str()
    );
    assert!(verifies(&resolved, &state.signer.active_public_key()));
    assert!(signing::verify(
        resolved["author_signature"]["public_key"].as_str().unwrap(),
        &message,
        resolved["author_signature"]["signature"].as_str().unwrap()
    ));

    // A revoked key signs nothing new.
    let (status, revoked) = send(
        &app,
        "DELETE",
        &format!("/v1/keys/{key_id}"),
        Some(alice),
        Some(json!({"reason":"The build machine was replaced."})),
        Some("revoke-alice-key"),
        &[],
    )
    .await
    .into_pair();
    assert_eq!(status, 200, "{revoked}");
    assert_eq!(revoked["key"]["status"], "revoked");
    let refused = upload(&app, &bytes, "upload-revoked", Some((&key_id, &good))).await;
    assert_eq!(refused.status, 422);
    assert_eq!(refused.body["error"]["details"]["status"], "revoked");
    // Bob cannot revoke Alice's key.
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/v1/keys/{key_id}"),
        Some(bob),
        None,
        Some("bob-revokes"),
        &[],
    )
    .await
    .into_pair();
    assert_eq!(status, 404);
}

trait Pair {
    fn into_pair(self) -> (u16, Value);
}
impl Pair for Response {
    fn into_pair(self) -> (u16, Value) {
        (self.status.as_u16(), self.body)
    }
}
