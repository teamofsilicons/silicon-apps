//! Installs check the release signature, and the author's when there is one,
//! before anything from a package is used.
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use serde_json::{Value, json};
use silicon_apps_client::{
    Client, Config, LocalState, install, package,
    signing::{self, VerificationError},
    updater,
};
use silicon_apps_server::signing::{Keyring, ReleaseFields};
use std::{
    fs,
    sync::{Arc, Mutex},
};

fn archive(version: &str, script: Option<&str>) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let target = package::current_target().unwrap();
    let script_name = if cfg!(windows) {
        "setup.cmd"
    } else {
        "install.sh"
    };
    let field = script
        .map(|_| format!("\n    install_script: {script_name}"))
        .unwrap_or_default();
    fs::write(
        dir.path().join("apps.yaml"),
        format!("app_id: fixture\nversion: {version}\ncommand: fixture\ntargets:\n  {target}:\n    binary: fixture{field}\n"),
    )
    .unwrap();
    fs::write(dir.path().join("fixture"), format!("fixture {version}")).unwrap();
    if let Some(script) = script {
        fs::write(dir.path().join(script_name), script).unwrap();
    }
    package::pack_directory(dir.path()).unwrap()
}
fn ring(id: &str) -> Keyring {
    Keyring::parse(&silicon_apps_server::signing::generate(id).0, &[]).unwrap()
}
fn script_digest(bytes: &[u8]) -> Option<String> {
    let manifest = package::inspect_archive(bytes).unwrap();
    package::install_script(bytes, &manifest, package::current_target().unwrap())
        .unwrap()
        .map(|(s, _)| s.sha256)
}
fn release_message(version: &str, release_id: &str, bytes: &[u8]) -> String {
    silicon_apps_server::signing::release_message(&ReleaseFields {
        app_id: "fixture",
        target: package::current_target().unwrap(),
        version,
        channel: "production",
        sha256: &package::sha256(bytes),
        size: bytes.len() as u64,
        release_id,
        install_script_sha256: script_digest(bytes).as_deref(),
    })
}

/// What the fake service serves right now.
#[derive(Clone)]
struct Scenario {
    resolution: Value,
    bytes: Vec<u8>,
    keys: Value,
}
type Shared = Arc<Mutex<Scenario>>;
/// A resolution for `bytes`, signed by `signer` over `signed` (usually the
/// same bytes) and claiming `key_id`.
fn resolution(version: &str, bytes: &[u8], signed: &[u8], signer: &Keyring, key_id: &str) -> Value {
    let release_id = format!("release-{version}");
    let (_, signature) = signer.sign(&release_message(version, &release_id, signed));
    json!({
        "app_id":"fixture",
        "release":{"id":release_id,"app_id":"fixture","channel":"production","version":version,"package_ids":["pkg"]},
        "package":{"id":format!("pkg-{version}"),"target":package::current_target().unwrap(),"sha256":package::sha256(bytes),"size":bytes.len(),"command":"fixture"},
        "download_path":"/package",
        "signature":{"key_id":key_id,"algorithm":"ed25519","signature":signature},
        "withdrawn":[]
    })
}
fn key_entry(id: &str, ring: &Keyring, status: &str, endorsers: &[(&str, &Keyring)]) -> Value {
    let public = ring.active_public_key();
    let message = signing::endorsement_message(id, &public);
    json!({
        "key_id":id,"public_key":public,"status":status,
        "endorsements":endorsers.iter().map(|(by, r)| json!({"key_id":by,"signature":r.sign(&message).1})).collect::<Vec<_>>()
    })
}
async fn serve(scenario: Shared) -> (Client, LocalState, Config, tempfile::TempDir) {
    let app = Router::new()
        .route(
            "/v1/apps/fixture/resolve",
            get(
                |State(s): State<Shared>| async move { Json(s.lock().unwrap().resolution.clone()) },
            ),
        )
        .route(
            "/package",
            get(|State(s): State<Shared>| async move { s.lock().unwrap().bytes.clone() }),
        )
        .route(
            "/.well-known/silicon-apps-keys.json",
            get(|State(s): State<Shared>| async move { Json(s.lock().unwrap().keys.clone()) }),
        )
        .route(
            "/v1/apps/fixture/installs",
            post(|| async { Json(json!({"installs":1})) }),
        )
        .with_state(scenario);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let config = Config {
        server: format!("http://{address}"),
        ..Config::default()
    };
    (
        Client::new(&config.server, None).unwrap(),
        state,
        config,
        home,
    )
}
fn code(error: &anyhow::Error) -> &'static str {
    error
        .chain()
        .find_map(|e| e.downcast_ref::<VerificationError>())
        .unwrap_or_else(|| panic!("not a verification error: {error:#}"))
        .code
}
async fn install(
    client: &Client,
    state: &LocalState,
    config: &Config,
) -> anyhow::Result<install::InstallOutcome> {
    install::install(client, state, config, &"fixture".parse().unwrap(), true).await
}

#[tokio::test]
async fn tampered_bytes_are_refused_before_anything_is_installed() {
    let k1 = ring("k1");
    let good = archive("1.0.0", None);
    let evil = archive(
        "1.0.0",
        Some("#!/bin/sh\ntouch \"$APPS_INSTALL_DIR/../../pwned\"\n"),
    );
    // The checksum matches what is served, but the signature covers other bytes.
    let scenario = Arc::new(Mutex::new(Scenario {
        resolution: resolution("1.0.0", &evil, &good, &k1, "k1"),
        bytes: evil,
        keys: json!({"keys":[key_entry("k1", &k1, "active", &[])],"revoked":[]}),
    }));
    let (client, state, config, home) = serve(scenario.clone()).await;
    let error = install(&client, &state, &config).await.unwrap_err();
    assert_eq!(code(&error), "signature_mismatch", "{error:#}");
    let details = &error
        .chain()
        .find_map(|e| e.downcast_ref::<VerificationError>())
        .unwrap()
        .details;
    assert_eq!(details["key_id"], "k1");
    assert!(state.installed().unwrap().is_empty());
    assert!(!home.path().join(".apps/bin/fixture").exists());
    assert!(!home.path().join(".apps/pwned").exists());
    // Bytes that do not match the release's checksum never get that far.
    let good = archive("1.0.0", None);
    scenario.lock().unwrap().resolution = resolution("1.0.0", &good, &good, &k1, "k1");
    scenario.lock().unwrap().bytes = archive("1.0.1", None);
    let error = install(&client, &state, &config).await.unwrap_err();
    assert_eq!(code(&error), "checksum_mismatch", "{error:#}");
    // With the right bytes, the same release installs.
    scenario.lock().unwrap().bytes = good;
    let outcome = install(&client, &state, &config).await.unwrap();
    assert_eq!(outcome.installed.signature_key_id.as_deref(), Some("k1"));
    assert_eq!(outcome.installed.install_script.as_deref(), Some("none"));
}

#[tokio::test]
async fn signatures_by_the_wrong_key_and_unsigned_releases_are_refused() {
    let (published, other) = (ring("y"), ring("x"));
    let bytes = archive("1.0.0", None);
    let scenario = Arc::new(Mutex::new(Scenario {
        // Signed by a key the service does not publish.
        resolution: resolution("1.0.0", &bytes, &bytes, &other, "x"),
        bytes: bytes.clone(),
        keys: json!({"keys":[key_entry("y", &published, "active", &[])],"revoked":[]}),
    }));
    let (client, state, config, _home) = serve(scenario.clone()).await;
    let error = install(&client, &state, &config).await.unwrap_err();
    assert_eq!(code(&error), "untrusted_signing_key", "{error:#}");
    // Claims the published key, but another key made the signature.
    scenario.lock().unwrap().resolution = resolution("1.0.0", &bytes, &bytes, &other, "y");
    let error = install(&client, &state, &config).await.unwrap_err();
    assert_eq!(code(&error), "signature_mismatch", "{error:#}");
    // No signature at all.
    let mut unsigned = resolution("1.0.0", &bytes, &bytes, &published, "y");
    unsigned.as_object_mut().unwrap().remove("signature");
    scenario.lock().unwrap().resolution = unsigned;
    let error = install(&client, &state, &config).await.unwrap_err();
    assert_eq!(code(&error), "release_unsigned", "{error:#}");
    assert!(state.installed().unwrap().is_empty());
    // The right key works, and it is now trusted on first use for this service.
    scenario.lock().unwrap().resolution = resolution("1.0.0", &bytes, &bytes, &published, "y");
    install(&client, &state, &config).await.unwrap();
    let trusted = signing::trusted_keys(&state, &config.server).unwrap();
    assert_eq!(trusted.len(), 1);
    assert_eq!(trusted[0].key_id, "y");
    assert_eq!(trusted[0].source, "first_use");
}

#[tokio::test]
async fn rotated_keys_are_followed_through_endorsements_and_others_are_refused() {
    let (k1, k2, k3) = (ring("k1"), ring("k2"), ring("k3"));
    let v1 = archive("1.0.0", None);
    let scenario = Arc::new(Mutex::new(Scenario {
        resolution: resolution("1.0.0", &v1, &v1, &k1, "k1"),
        bytes: v1,
        keys: json!({"keys":[key_entry("k1", &k1, "active", &[])],"revoked":[]}),
    }));
    let (client, state, config, _home) = serve(scenario.clone()).await;
    install(&client, &state, &config).await.unwrap();

    // The service rotates to k2; k1 endorses it.
    let v2 = archive("2.0.0", None);
    {
        let mut s = scenario.lock().unwrap();
        s.resolution = resolution("2.0.0", &v2, &v2, &k2, "k2");
        s.bytes = v2;
        s.keys = json!({"keys":[key_entry("k2", &k2, "active", &[("k1", &k1)]),key_entry("k1", &k1, "retired", &[])],"revoked":[]});
    }
    let result = updater::update(&client, &state, &config, None)
        .await
        .unwrap();
    assert_eq!(result["items"][0]["status"], "updated", "{result}");
    assert_eq!(result["items"][0]["version"], "2.0.0");
    let trusted = signing::trusted_keys(&state, &config.server).unwrap();
    let k2_entry = trusted.iter().find(|k| k.key_id == "k2").unwrap();
    assert_eq!(k2_entry.source, "endorsed");
    assert_eq!(k2_entry.endorsed_by.as_deref(), Some("k1"));

    // A key nobody trusted endorses is refused, and nothing changes.
    let v3 = archive("3.0.0", None);
    {
        let mut s = scenario.lock().unwrap();
        s.resolution = resolution("3.0.0", &v3, &v3, &k3, "k3");
        s.bytes = v3.clone();
        s.keys = json!({"keys":[key_entry("k3", &k3, "active", &[])],"revoked":[]});
    }
    let result = updater::update(&client, &state, &config, None)
        .await
        .unwrap();
    assert_eq!(result["items"][0]["status"], "failed");
    assert!(
        result["items"][0]["error"]
            .as_str()
            .unwrap()
            .contains("untrusted_signing_key"),
        "{result}"
    );
    assert_eq!(state.installed().unwrap()["fixture"].version, "2.0.0");

    // A revoked key is refused even though it was trusted before.
    {
        let mut s = scenario.lock().unwrap();
        s.resolution = resolution("3.0.0", &v3, &v3, &k2, "k2");
        s.keys = json!({"keys":[key_entry("k3", &k3, "active", &[("k2", &k2)])],"revoked":["k2"]});
    }
    // Clients read the keys document at least every ten minutes; read it now.
    signing::refresh_keys(&client, &state).await.unwrap();
    let error = install(&client, &state, &config).await.unwrap_err();
    assert_eq!(code(&error), "signing_key_revoked", "{error:#}");
    assert!(
        !signing::trusted_keys(&state, &config.server)
            .unwrap()
            .iter()
            .any(|k| k.key_id == "k2")
    );
    // k3 was endorsed by k2 before k2 was revoked; that no longer counts.
    scenario.lock().unwrap().resolution = resolution("3.0.0", &v3, &v3, &k3, "k3");
    let error = install(&client, &state, &config).await.unwrap_err();
    assert_eq!(code(&error), "untrusted_signing_key", "{error:#}");
}

#[test]
fn the_official_service_key_is_pinned() {
    let home = tempfile::tempdir().unwrap();
    let state = LocalState::new(home.path()).unwrap();
    let keys = signing::trusted_keys(&state, silicon_apps_client::DEFAULT_URL).unwrap();
    assert_eq!(keys.len(), signing::PINNED_KEYS.len());
    assert!(keys.iter().all(|k| k.source == "pinned"));
    for pinned in signing::PINNED_KEYS {
        assert_eq!(
            base64_len(pinned.public_key),
            32,
            "{} is not a 32 byte key",
            pinned.key_id
        );
    }
}
fn base64_len(value: &str) -> usize {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .unwrap()
        .len()
}

#[tokio::test]
async fn author_signatures_are_checked_and_shown() {
    let k1 = ring("k1");
    let author = signing::AuthorKey::generate();
    let bytes = archive("1.0.0", None);
    let target = package::current_target().unwrap();
    let mut signed = resolution("1.0.0", &bytes, &bytes, &k1, "k1");
    signed["author_signature"] = json!({
        "key_id":author.key_id,"algorithm":"ed25519","public_key":author.public_key,
        "signature":author.sign_package("fixture", target, &bytes).unwrap(),
        "signer_uuid":"alice","signer_id":"c:alice"
    });
    let scenario = Arc::new(Mutex::new(Scenario {
        resolution: signed.clone(),
        bytes: bytes.clone(),
        keys: json!({"keys":[key_entry("k1", &k1, "active", &[])],"revoked":[]}),
    }));
    let (client, state, config, _home) = serve(scenario.clone()).await;
    // A broken author signature stops the install.
    let mut broken = signed.clone();
    broken["author_signature"]["signature"] = json!(author.sign("something else entirely"));
    scenario.lock().unwrap().resolution = broken;
    let error = install(&client, &state, &config).await.unwrap_err();
    assert_eq!(code(&error), "author_signature_mismatch", "{error:#}");
    // A public key that does not match its key ID is refused too.
    let mut swapped = signed.clone();
    swapped["author_signature"]["public_key"] = json!(signing::AuthorKey::generate().public_key);
    scenario.lock().unwrap().resolution = swapped;
    let error = install(&client, &state, &config).await.unwrap_err();
    assert_eq!(code(&error), "author_signature_mismatch", "{error:#}");
    scenario.lock().unwrap().resolution = signed;
    let outcome = install(&client, &state, &config).await.unwrap();
    assert_eq!(
        outcome.installed.author_key_id.as_deref(),
        Some(author.key_id.as_str())
    );
    assert_eq!(outcome.installed.signed_by.as_deref(), Some("c:alice"));

    // The next version is not author-signed: the install says so in one line.
    let v2 = archive("2.0.0", None);
    scenario.lock().unwrap().resolution = resolution("2.0.0", &v2, &v2, &k1, "k1");
    scenario.lock().unwrap().bytes = v2;
    let outcome = install(&client, &state, &config).await.unwrap();
    let notice = outcome.notice.unwrap();
    assert!(notice.contains("is not signed by an author"), "{notice}");
    assert_eq!(notice.lines().count(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn changed_install_scripts_are_announced_in_one_line() {
    let k1 = ring("k1");
    let v1 = archive("1.0.0", Some("#!/bin/sh\nexit 0\n"));
    let scenario = Arc::new(Mutex::new(Scenario {
        resolution: resolution("1.0.0", &v1, &v1, &k1, "k1"),
        bytes: v1.clone(),
        keys: json!({"keys":[key_entry("k1", &k1, "active", &[])],"revoked":[]}),
    }));
    let (client, state, config, _home) = serve(scenario.clone()).await;
    let first = install(&client, &state, &config).await.unwrap();
    assert!(first.notice.is_none());
    assert_eq!(first.installed.install_script, script_digest(&v1));
    // Same script, new version: nothing to say.
    let v2 = archive("2.0.0", Some("#!/bin/sh\nexit 0\n"));
    scenario.lock().unwrap().resolution = resolution("2.0.0", &v2, &v2, &k1, "k1");
    scenario.lock().unwrap().bytes = v2;
    let result = updater::update(&client, &state, &config, None)
        .await
        .unwrap();
    assert_eq!(result["items"][0]["status"], "updated");
    assert!(result["items"][0]["notice"].is_null(), "{result}");
    // A changed script is announced.
    let v3 = archive("3.0.0", Some("#!/bin/sh\necho new step\n"));
    scenario.lock().unwrap().resolution = resolution("3.0.0", &v3, &v3, &k1, "k1");
    scenario.lock().unwrap().bytes = v3.clone();
    let result = updater::update(&client, &state, &config, None)
        .await
        .unwrap();
    let notice = result["items"][0]["notice"].as_str().unwrap();
    assert!(notice.contains("changes its install script"), "{notice}");
    assert!(notice.contains("silicon-apps show fixture --install-script"));
    assert_eq!(notice.lines().count(), 1);
    // And it can be read, with its digest, without installing anything.
    let shown = install::inspect_install_script(
        &client,
        &state,
        &"fixture".parse().unwrap(),
        package::current_target().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(shown["install_script"]["sha256"], json!(script_digest(&v3)));
    assert_eq!(
        shown["install_script"]["content"],
        "#!/bin/sh\necho new step\n"
    );
    assert_eq!(shown["signature_key_id"], "k1");
}

#[tokio::test]
async fn the_updater_moves_off_a_withdrawn_release() {
    let k1 = ring("k1");
    let v1 = archive("1.0.0", None);
    let v2 = archive("1.1.0", None);
    let scenario = Arc::new(Mutex::new(Scenario {
        resolution: resolution("1.1.0", &v2, &v2, &k1, "k1"),
        bytes: v2,
        keys: json!({"keys":[key_entry("k1", &k1, "active", &[])],"revoked":[]}),
    }));
    let (client, state, config, _home) = serve(scenario.clone()).await;
    install(&client, &state, &config).await.unwrap();
    assert_eq!(state.installed().unwrap()["fixture"].version, "1.1.0");
    // The authors withdraw 1.1.0: the service now resolves to 1.0.0.
    {
        let mut s = scenario.lock().unwrap();
        s.resolution = resolution("1.0.0", &v1, &v1, &k1, "k1");
        s.resolution["withdrawn"] = json!([{"release_id":"release-1.1.0","version":"1.1.0","channel":"production","reason":"Deletes the config file.","withdrawn_at":"2026-10-09T00:00:00Z","withdrawn_by":"c:alice"}]);
        s.bytes = v1;
    }
    let result = updater::update(&client, &state, &config, None)
        .await
        .unwrap();
    let item = &result["items"][0];
    assert_eq!(item["status"], "replaced_withdrawn", "{result}");
    assert_eq!(item["version"], "1.0.0");
    assert_eq!(item["withdrawn"]["reason"], "Deletes the config file.");
    assert!(item["notice"].as_str().unwrap().contains("was withdrawn"));
    assert_eq!(state.installed().unwrap()["fixture"].version, "1.0.0");
    // On the next check it is current.
    let result = updater::update(&client, &state, &config, None)
        .await
        .unwrap();
    assert_eq!(result["items"][0]["status"], "current");
}
