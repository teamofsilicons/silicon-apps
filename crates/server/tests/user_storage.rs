mod common;
use axum::{Json, Router, routing::post};
use common::*;
use serde_json::{Value, json};
use silicon_apps_server::{AppState, Shared, router, store::Prepared};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

const USER: &str = "11111111-1111-4111-8111-111111111111";
const OTHER: &str = "22222222-2222-4222-8222-222222222222";

#[tokio::test]
async fn concurrent_updates_and_schema_growth_respect_saved_values() {
    let (_dir, _state, app, _proofs, task) = fixture().await;
    let (a, b) = tokio::join!(
        storage(
            &app,
            "PATCH",
            "notes",
            USER,
            "write",
            Some("owner"),
            Some(json!({"values":{"theme":"dark"},"if_revision":0})),
            Some("concurrent-a"),
            false
        ),
        storage(
            &app,
            "PATCH",
            "notes",
            USER,
            "write",
            Some("owner"),
            Some(json!({"values":{"scale":2},"if_revision":0})),
            Some("concurrent-b"),
            false
        )
    );
    let mut statuses = vec![a.status.as_u16(), b.status.as_u16()];
    statuses.sort();
    assert_eq!(statuses, vec![200, 409]);
    let (a, b) = tokio::join!(
        storage(
            &app,
            "PATCH",
            "notes",
            USER,
            "write",
            Some("owner"),
            Some(json!({"values":{"theme":"dark"}})),
            Some("merge-update-a"),
            false
        ),
        storage(
            &app,
            "PATCH",
            "notes",
            USER,
            "write",
            Some("owner"),
            Some(json!({"values":{"scale":2}})),
            Some("merge-update-b"),
            false
        )
    );
    assert_eq!(a.status, 200);
    assert_eq!(b.status, 200);
    let merged = storage(&app, "GET", "notes", USER, "read", None, None, None, false).await;
    assert_eq!(merged.body["values"]["theme"], "dark");
    assert_eq!(merged.body["values"]["scale"], 2);
    let mut definition = schema();
    let mut values = json!({});
    for i in 0..16 {
        definition[format!("large{i}")] = json!({"type":"string","default":""});
        values[format!("large{i}")] = json!("x".repeat(64000));
    }
    assert_eq!(
        call(
            &app,
            "PUT",
            "/v1/apps/notes/storage-schema",
            Some(ALICE),
            definition.clone(),
            Some("large-schema")
        )
        .await
        .0,
        200
    );
    assert_eq!(
        storage(
            &app,
            "PATCH",
            "notes",
            USER,
            "write",
            Some("owner"),
            Some(json!({"values":values})),
            Some("large-record"),
            false
        )
        .await
        .status,
        200
    );
    definition["overflow"] = json!({"type":"string","default":"x".repeat(40000)});
    assert_eq!(
        call(
            &app,
            "PUT",
            "/v1/apps/notes/storage-schema",
            Some(ALICE),
            definition,
            Some("overflow-schema")
        )
        .await
        .0,
        409
    );
    let view = storage(&app, "GET", "notes", USER, "read", None, None, None, false).await;
    assert!(view.body["values"].get("overflow").is_none());
    task.abort();
}
fn schema() -> Value {
    json!({"theme":{"type":"string","default":"system"},"alerts":{"type":"boolean","default":true},"scale":{"type":"number","default":1},"options":{"type":"object","default":{}},"recent":{"type":"array","default":[]}})
}
fn token(name: &str) -> String {
    format!(
        "sap_{}",
        &silicon_apps_server::store::hash(name.as_bytes())[..43]
    )
}
fn proof(kind: &str, issuer: &str, user: Option<&str>, scopes: &[&str]) -> Value {
    json!({"valid":true,"proof_id":"proof-family","kind":kind,"expires_at":(chrono::Utc::now()+chrono::Duration::minutes(10)).to_rfc3339(),"issuing_app":{"app_id":issuer,"name":issuer},"receiving_app":{"app_id":"silicon-apps","name":"Apps"},"user":user.map(|uuid|json!({"uuid":uuid,"id":"c:test","kind":"carbon","membership_id":format!("{issuer}:{uuid}")})),"scopes":scopes})
}
type Proofs = Arc<Mutex<BTreeMap<String, Value>>>;
async fn fixture() -> (
    tempfile::TempDir,
    Shared,
    Router,
    Proofs,
    tokio::task::JoinHandle<()>,
) {
    let proofs: Proofs = Arc::default();
    let p = proofs.clone();
    let verifier = Router::new().route(
        "/v1/proofs/verify",
        post(
            move |headers: axum::http::HeaderMap, Json(body): Json<Value>| {
                let p = p.clone();
                async move {
                    use base64::Engine;
                    assert_eq!(
                        headers["authorization"],
                        format!(
                            "Basic {}",
                            base64::engine::general_purpose::STANDARD
                                .encode("silicon-apps:verification-secret")
                        )
                    );
                    Json(
                        p.lock()
                            .unwrap()
                            .get(body["proof_token"].as_str().unwrap())
                            .cloned()
                            .unwrap_or(json!({"valid":false,"expires_at":null})),
                    )
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, verifier).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.accounts_url = url;
    cfg.accounts_app_secret = Some("verification-secret".into());
    let state = AppState::new(cfg).unwrap();
    for app in ["notes", "other", "disabled"] {
        mutate(
            &state,
            "alice",
            "POST",
            "apps",
            &format!("create-{app}"),
            json!({"app_id":app,"name":app,"user_storage":if app == "disabled" {Value::Null} else {schema()}}),
            Prepared {
                secret: Some("unused".into()),
                ..Default::default()
            },
        );
    }
    for (name, issuer, user, scopes) in [
        ("read", "reader", Some(USER), vec!["storage:notes:read"]),
        ("write", "notes", Some(USER), vec!["storage:notes:write"]),
        (
            "other-user",
            "notes",
            Some(OTHER),
            vec!["storage:notes:write"],
        ),
        (
            "other-app",
            "other",
            Some(USER),
            vec!["storage:other:write"],
        ),
        ("owner", "notes", None, vec!["storage:write"]),
        ("anon-reader", "notes", None, vec!["storage:read"]),
        ("wrong-owner", "other", None, vec!["storage:write"]),
        ("disabled", "disabled", None, vec!["storage:read"]),
    ] {
        proofs.lock().unwrap().insert(
            token(name),
            proof(
                if user.is_some() {
                    "user_verification"
                } else {
                    "app_verification"
                },
                issuer,
                user,
                &scopes,
            ),
        );
    }
    let app = router(state.clone());
    (dir, state, app, proofs, task)
}
async fn storage(
    app: &Router,
    method: &str,
    target: &str,
    user: &str,
    proof: &str,
    owner: Option<&str>,
    body: Option<Value>,
    key: Option<&str>,
    reset: bool,
) -> Response {
    let auth = format!("Proof {}", token(proof));
    let owner = owner.map(token);
    let mut headers = vec![("Authorization", auth.as_str())];
    if let Some(owner) = &owner {
        headers.push(("X-App-Verification", owner));
    }
    let path = format!(
        "/v1/apps/{target}/storage/users/{user}{}",
        if reset { "/reset" } else { "" }
    );
    send(app, method, &path, None, body, key, &headers).await
}

#[tokio::test]
async fn defaults_writes_reset_isolation_persistence_and_private_metadata() {
    let (_dir, state, app, _proofs, task) = fixture().await;
    let initial = storage(&app, "GET", "notes", USER, "read", None, None, None, false).await;
    assert_eq!(initial.status, 200);
    assert!(
        initial.headers["cache-control"]
            .to_str()
            .unwrap()
            .contains("no-store")
    );
    assert_eq!(
        initial.body["values"],
        json!({"theme":"system","alerts":true,"scale":1,"options":{},"recent":[]})
    );
    let result = storage(
        &app,
        "PATCH",
        "notes",
        USER,
        "write",
        Some("owner"),
        Some(json!({"values":{"theme":"private-dark","alerts":false},"if_revision":0})),
        Some("write-first"),
        false,
    )
    .await;
    assert_eq!(result.status, 200, "{}", result.body);
    assert_eq!(result.body["revision"], 1);
    let fresh = storage(&app, "GET", "notes", USER, "read", None, None, None, false).await;
    assert_eq!(fresh.body["values"]["theme"], "private-dark");
    for (target, user, proof) in [
        ("notes", OTHER, "other-user"),
        ("other", USER, "other-app"),
        ("notes", "anon", "owner"),
    ] {
        let separate = storage(&app, "GET", target, user, proof, None, None, None, false).await;
        assert_eq!(separate.status, 200);
        assert_eq!(separate.body["values"]["theme"], "system");
    }
    let anon = storage(
        &app,
        "PATCH",
        "notes",
        "anon",
        "owner",
        None,
        Some(json!({"values":{"theme":"guest"}})),
        Some("anon-update"),
        false,
    )
    .await;
    assert_eq!(anon.status, 200);
    let anon = storage(
        &app,
        "GET",
        "notes",
        "anon",
        "anon-reader",
        None,
        None,
        None,
        false,
    )
    .await;
    assert_eq!(anon.body["values"]["theme"], "guest");
    let persisted = AppState::new(state.config.clone()).unwrap();
    let again = storage(
        &router(persisted),
        "GET",
        "notes",
        USER,
        "read",
        None,
        None,
        None,
        false,
    )
    .await;
    assert_eq!(again.body["values"]["theme"], "private-dark");
    let conflict = storage(
        &app,
        "PATCH",
        "notes",
        USER,
        "write",
        Some("owner"),
        Some(json!({"values":{"theme":"stale"},"if_revision":0})),
        Some("write-stale"),
        false,
    )
    .await;
    assert_eq!(conflict.status, 409);
    let reset = storage(
        &app,
        "POST",
        "notes",
        USER,
        "write",
        Some("owner"),
        Some(json!({"keys":["theme"],"if_revision":1})),
        Some("reset-theme"),
        true,
    )
    .await;
    assert_eq!(reset.status, 200);
    assert_eq!(reset.body["values"]["theme"], "system");
    assert_eq!(reset.body["values"]["alerts"], false);
    let catalog = state.store.lock().unwrap().catalog().unwrap();
    assert!(
        !catalog.apps["notes"]
            .view(None)
            .to_string()
            .contains("user_storage")
    );
    assert!(
        !serde_json::to_string(&catalog)
            .unwrap()
            .contains("private-dark")
    );
    task.abort();
}

#[tokio::test]
async fn authorization_matrix_and_revoked_idempotent_replays() {
    let (_dir, _state, app, proofs, task) = fixture().await;
    let request = json!({"values":{"theme":"dark"}});
    for (method, target, user, proof, owner) in [
        ("GET", "notes", OTHER, "read", None),
        ("GET", "other", USER, "read", None),
        ("GET", "notes", USER, "owner", None),
        ("GET", "notes", "anon", "read", None),
        ("GET", "notes", "anon", "wrong-owner", None),
        ("PATCH", "notes", USER, "write", None),
        ("PATCH", "notes", USER, "write", Some("wrong-owner")),
        ("PATCH", "notes", USER, "write", Some("read")),
        ("PATCH", "notes", USER, "read", Some("owner")),
        ("PATCH", "notes", "anon", "anon-reader", None),
    ] {
        let r = storage(
            &app,
            method,
            target,
            user,
            proof,
            owner,
            Some(request.clone()),
            Some("denied-request"),
            false,
        )
        .await;
        assert_eq!(
            r.status, 403,
            "{method} {target} {user} {proof}: {}",
            r.body
        );
        assert!(
            r.headers["cache-control"]
                .to_str()
                .unwrap()
                .contains("no-store")
        );
    }
    for token in [Some(ALICE), None] {
        let r = send(
            &app,
            "GET",
            &format!("/v1/apps/notes/storage/users/{USER}"),
            token,
            None,
            None,
            &[],
        )
        .await;
        assert_eq!(r.status, 403);
    }
    for change in ["expired", "recipient", "scope", "kind"] {
        let mut p = proof(
            "user_verification",
            "reader",
            Some(USER),
            &["storage:notes:read"],
        );
        match change {
            "expired" => p["expires_at"] = json!("2000-01-01T00:00:00Z"),
            "recipient" => p["receiving_app"]["app_id"] = json!("elsewhere"),
            "scope" => p["scopes"] = json!(["profile.read"]),
            _ => p["kind"] = json!("app_verification"),
        }
        proofs.lock().unwrap().insert(token("bad"), p);
        assert_eq!(
            storage(&app, "GET", "notes", USER, "bad", None, None, None, false)
                .await
                .status,
            403
        );
    }
    let first = storage(
        &app,
        "PATCH",
        "notes",
        USER,
        "write",
        Some("owner"),
        Some(request.clone()),
        Some("retry-write"),
        false,
    )
    .await;
    assert_eq!(first.status, 200);
    let second = storage(
        &app,
        "PATCH",
        "notes",
        USER,
        "write",
        Some("owner"),
        Some(request.clone()),
        Some("retry-write"),
        false,
    )
    .await;
    assert_eq!(second.body, first.body);
    assert_eq!(second.headers["idempotent-replayed"], "true");
    assert_eq!(
        storage(
            &app,
            "PATCH",
            "notes",
            USER,
            "write",
            Some("owner"),
            Some(json!({"values":{"theme":"different"}})),
            Some("retry-write"),
            false
        )
        .await
        .status,
        409
    );
    proofs.lock().unwrap().remove(&token("write"));
    assert_eq!(
        storage(
            &app,
            "PATCH",
            "notes",
            USER,
            "write",
            Some("owner"),
            Some(request),
            Some("retry-write"),
            false
        )
        .await
        .status,
        403
    );
    proofs.lock().unwrap().remove(&token("read"));
    assert_eq!(
        storage(&app, "GET", "notes", USER, "read", None, None, None, false)
            .await
            .status,
        403
    );
    assert_eq!(
        storage(
            &app, "GET", "disabled", "anon", "disabled", None, None, None, false
        )
        .await
        .body["error"]["code"],
        "storage_not_defined"
    );
    task.abort();
}

#[tokio::test]
async fn schema_management_and_atomic_validation() {
    let (_dir, _state, app, _proofs, task) = fixture().await;
    let r = send(
        &app,
        "GET",
        "/v1/apps/notes/storage-schema",
        Some(BOB),
        None,
        None,
        &[],
    )
    .await;
    assert!(!r.status.is_success());
    let mut next = schema();
    next["font"] = json!({"type":"string","default":"serif"});
    let r = call(
        &app,
        "PUT",
        "/v1/apps/notes/storage-schema",
        Some(ALICE),
        next.clone(),
        Some("extend-schema"),
    )
    .await;
    assert_eq!(r.0, 200, "{}", r.1);
    assert_eq!(
        storage(&app, "GET", "notes", USER, "read", None, None, None, false)
            .await
            .body["values"]["font"],
        "serif"
    );
    next["theme"]["default"] = json!("changed");
    assert_eq!(
        call(
            &app,
            "PUT",
            "/v1/apps/notes/storage-schema",
            Some(ALICE),
            next,
            Some("change-default")
        )
        .await
        .0,
        409
    );
    for values in [
        json!({"unknown":1}),
        json!({"theme":false}),
        json!({"theme":"valid","alerts":"invalid"}),
        json!({"theme":"x".repeat(65537)}),
    ] {
        assert_eq!(
            storage(
                &app,
                "PATCH",
                "notes",
                USER,
                "write",
                Some("owner"),
                Some(json!({"values":values})),
                Some("invalid-write"),
                false
            )
            .await
            .status,
            400
        );
    }
    let r = storage(&app, "GET", "notes", USER, "read", None, None, None, false).await;
    assert_eq!(r.body["values"]["theme"], "system");
    assert_eq!(r.body["revision"], 0);
    for definition in [
        json!({"x":{"type":"string","default":false}}),
        json!({"x":{"type":"mystery","default":0}}),
        json!({"__proto__":{"type":"object","default":{}}}),
        json!({"x":{"type":"string","default":"", "extra":true}}),
    ] {
        assert_eq!(
            call(
                &app,
                "PUT",
                "/v1/apps/disabled/storage-schema",
                Some(ALICE),
                definition,
                Some("invalid-schema")
            )
            .await
            .0,
            400
        );
    }
    let r = call(
        &app,
        "PUT",
        "/v1/apps/disabled/storage-schema",
        Some(ALICE),
        schema(),
        Some("enable-storage"),
    )
    .await;
    assert_eq!(r.0, 200);
    task.abort();
}
