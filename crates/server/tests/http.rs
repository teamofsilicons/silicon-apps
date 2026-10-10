use axum::{
    Json, Router,
    body::Body,
    http::{Request, StatusCode},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer, SigningKey};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use silicon_apps_server::{
    AppState,
    config::Config,
    model::*,
    router,
    store::{Mutation, Prepared, hash},
};
use tower::ServiceExt;
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
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
    key: Option<&str>,
) -> (StatusCode, Value) {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("Content-Type", "application/json");
    if let Some(t) = token {
        b = b.header("Authorization", format!("Bearer {t}"));
    }
    if let Some(k) = key {
        b = b.header("Idempotency-Key", k);
    }
    let response = app
        .clone()
        .oneshot(b.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}
fn signed(key: &SigningKey, issuer: &str, aud: &str, expiry: i64) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"EdDSA","kid":"test-key","typ":"JWT"}"#);
    let body = URL_SAFE_NO_PAD.encode(
        json!({"iss":issuer,"sub":"alice","aud":aud,"exp":expiry,"id":"c:alice","kind":"carbon"})
            .to_string(),
    );
    let data = format!("{header}.{body}");
    let signature = key.sign(data.as_bytes());
    format!("{data}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
}
#[tokio::test]
async fn built_in_account_service_ids_are_unavailable_and_cannot_be_created() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(config(dir.path())).unwrap();
    let app = router(state.clone());
    for id in ["accounts", "silicon-accounts", "developer"] {
        let (status, body) = call(
            &app,
            "GET",
            &format!("/v1/apps/availability/{id}"),
            None,
            json!({}),
            None,
        )
        .await;
        assert_eq!(status, 200);
        assert_eq!(body["available"], false);
        let (status, body) = call(
            &app,
            "POST",
            "/v1/apps",
            Some("dev:alice:c:alice"),
            json!({"app_id":id,"name":"Reserved"}),
            Some(&format!("create-{id}")),
        )
        .await;
        assert_eq!(status, 409, "{body}");
    }
    let store = state.store.lock().unwrap();
    assert!(store.catalog().unwrap().apps.is_empty());
    let pending: i64 = store
        .connection
        .query_row("SELECT COUNT(*) FROM pending_secrets", [], |r| r.get(0))
        .unwrap();
    assert_eq!(pending, 0);
}
#[tokio::test]
async fn production_auth_verifies_signature_issuer_audience_expiry_and_active_account() {
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let x = URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes());
    let provider=Router::new().route("/.well-known/jwks.json",get(move||{let x=x.clone();async move{Json(json!({"keys":[{"kty":"OKP","crv":"Ed25519","alg":"EdDSA","kid":"test-key","x":x}]}))}})).route("/v1/userinfo",get(||async{Json(json!({"uuid":"alice","kind":"carbon","id":"c:renamed","display_name":"Alice","email":"alice@example.com","email_verified":true,"verified_emails":["alice@example.com","alice@secondary.example"]}))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.dev_auth = false;
    cfg.accounts_url = issuer.clone();
    let app = router(AppState::new(cfg).unwrap());
    let now = chrono::Utc::now().timestamp();
    let token = signed(&key, &issuer, "silicon-apps", now + 600);
    let (status, body) = call(&app, "GET", "/v1/me", Some(&token), json!({}), None).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["id"], "c:renamed");
    assert_eq!(
        body["verified_emails"],
        json!(["alice@example.com", "alice@secondary.example"])
    );
    for bad in [
        signed(&key, &issuer, "silicon-accounts", now + 600),
        signed(&key, "https://forged.example", "silicon-apps", now + 600),
        signed(&key, &issuer, "silicon-apps", now - 300),
        signed(
            &SigningKey::from_bytes(&[8u8; 32]),
            &issuer,
            "silicon-apps",
            now + 600,
        ),
        "dev:alice:c:alice".into(),
    ] {
        let (status, body) = call(&app, "GET", "/v1/me", Some(&bad), json!({}), None).await;
        assert_eq!(status, 401, "{body}");
    }
    task.abort();
}
#[tokio::test]
async fn developer_portal_tokens_manage_owned_apps_but_not_store_actions_or_other_authors() {
    use axum::response::IntoResponse;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let active = Arc::new(AtomicBool::new(true));
    let current = active.clone();
    let mismatched_profile = Arc::new(AtomicBool::new(false));
    let mismatched = mismatched_profile.clone();
    let key = SigningKey::from_bytes(&[9u8; 32]);
    let x = URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes());
    let provider = Router::new()
        .route("/.well-known/jwks.json", get(move || {let x=x.clone();async move {Json(json!({"keys":[{"kty":"OKP","crv":"Ed25519","alg":"EdDSA","kid":"test-key","x":x}]}))}}))
        .route("/v1/userinfo", get(move || {let active=current.clone();async move {
            if !active.load(Ordering::SeqCst) {
                return (StatusCode::UNAUTHORIZED, Json(json!({"error":{"code":"invalid_token","message":"Token family revoked"}}))).into_response();
            }
            Json(json!({"uuid":"alice","kind":"carbon","id":"c:alice","display_name":"Alice"})).into_response()
        }}))
        .route("/v1/me", get(move || {let mismatched=mismatched.clone();async move {
            Json(json!({"uuid":if mismatched.load(Ordering::SeqCst) {"bob"} else {"alice"},"kind":"carbon","id":"c:alice","emails":[{"email":"alice@secondary.example","verified_at":"2026-10-08T00:00:00Z"},{"email":"unverified@example.com"}]}))
        }}))
        .route("/v1/internal/apps", get(|| async {Json(json!({"apps":[]}))}))
        .route("/v1/internal/apps/sync", axum::routing::post(|| async {Json(json!({"status":"ok"}))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.dev_auth = false;
    cfg.accounts_url = issuer.clone();
    cfg.accounts_service_token = Some("test-service-token".into());
    let state = AppState::new(cfg).unwrap();
    seed_private(&state);
    {
        let store = state.store.lock().unwrap();
        let mut catalog = store.catalog().unwrap();
        for (id, to) in [
            ("secondary-invite", "alice@secondary.example"),
            ("unverified-invite", "unverified@example.com"),
        ] {
            catalog.invites.push(Invite {
                id: id.into(),
                app_id: "secret-app".into(),
                to: to.into(),
                account_uuid: None,
                status: "pending".into(),
                created_at: now(),
            });
        }
        store
            .connection
            .execute(
                "UPDATE catalog SET document=?1 WHERE id=1",
                [serde_json::to_string(&catalog).unwrap()],
            )
            .unwrap();
    }
    let app = router(state.clone());
    let now = chrono::Utc::now().timestamp();
    let token = signed(&key, &issuer, "developer", now + 600);
    for path in [
        "/v1/me",
        "/v1/session",
        "/v1/apps?mine=true",
        "/v1/apps/secret-app",
        "/v1/apps/secret-app/history",
    ] {
        let (status, body) = call(&app, "GET", path, Some(&token), json!({}), None).await;
        assert_eq!(status, 200, "{path}: {body}");
    }
    let (status, body) = call(&app, "GET", "/v1/me", Some(&token), json!({}), None).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["verified_emails"], json!(["alice@secondary.example"]));
    let (status, body) = call(&app, "GET", "/v1/invites", Some(&token), json!({}), None).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["id"], "secondary-invite");
    mismatched_profile.store(true, Ordering::SeqCst);
    let (status, body) = call(&app, "GET", "/v1/invites", Some(&token), json!({}), None).await;
    assert_eq!(status, 401, "{body}");
    mismatched_profile.store(false, Ordering::SeqCst);
    let (status, body) = call(
        &app,
        "PATCH",
        "/v1/apps/secret-app",
        Some(&token),
        json!({"name":"Managed in common portal"}),
        Some("developer-update"),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["name"], "Managed in common portal");
    let (status, body) = call(
        &app,
        "POST",
        "/v1/apps",
        Some(&token),
        json!({"app_id":"portal-app","name":"Portal App"}),
        Some("developer-create"),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["app"]["authors"][0]["uuid"], "alice");
    for (method, path) in [
        ("PUT", "/v1/apps/secret-app/review"),
        ("DELETE", "/v1/apps/secret-app/review"),
        ("POST", "/v1/apps/secret-app/installs"),
        ("POST", "/v1/reports"),
        ("POST", "/v1/platforms"),
        ("GET", "/v1/apps/secret-app/resolve"),
    ] {
        let (status, body) = call(
            &app,
            method,
            path,
            Some(&token),
            json!({}),
            Some("forbidden-developer"),
        )
        .await;
        assert_eq!(status, 401, "{method} {path}: {body}");
    }
    // A path outside the route table is refused before any credential is read.
    let (status, body) = call(
        &app,
        "POST",
        "/v1/apps/secret-app/publish/extra",
        Some(&token),
        json!({}),
        Some("forbidden-developer"),
    )
    .await;
    assert_eq!(status, 404, "{body}");
    for bad in [
        signed(&key, &issuer, "silicon-accounts", now + 600),
        signed(&key, &issuer, "other", now + 600),
        signed(&key, &issuer, "developer", now - 300),
        signed(
            &SigningKey::from_bytes(&[8; 32]),
            &issuer,
            "developer",
            now + 600,
        ),
    ] {
        let (status, body) = call(
            &app,
            "GET",
            "/v1/apps?mine=true",
            Some(&bad),
            json!({}),
            None,
        )
        .await;
        assert_eq!(status, 401, "{body}");
    }
    // A correctly signed token still needs a live, active token family.
    active.store(false, Ordering::SeqCst);
    let (status, body) = call(
        &app,
        "PATCH",
        "/v1/apps/secret-app",
        Some(&token),
        json!({"name":"Revoked"}),
        Some("developer-revoked"),
    )
    .await;
    assert_eq!(status, 401, "{body}");
    active.store(true, Ordering::SeqCst);
    // The audience bridge does not grant any author capability on another app.
    {
        let store = state.store.lock().unwrap();
        let mut catalog = store.catalog().unwrap();
        let other = catalog.apps.get_mut("secret-app").unwrap();
        other.authors[0].uuid = "bob".into();
        other.admin_uuid = "bob".into();
        store
            .connection
            .execute(
                "UPDATE catalog SET document=?1 WHERE id=1",
                [serde_json::to_string(&catalog).unwrap()],
            )
            .unwrap();
    }
    let (status, body) = call(
        &app,
        "PATCH",
        "/v1/apps/secret-app",
        Some(&token),
        json!({"name":"Not mine"}),
        Some("developer-not-owner"),
    )
    .await;
    assert_eq!(status, 403, "{body}");
    let (status, body) = call(
        &app,
        "GET",
        "/v1/apps/secret-app/history",
        Some(&token),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, 404, "{body}");
    task.abort();
}
#[tokio::test]
async fn author_refresh_uses_subscribed_profile_and_id_only_fallback_preserves_display_name() {
    use axum::response::IntoResponse;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let mode = Arc::new(AtomicUsize::new(0));
    let profile_mode = mode.clone();
    let lookups = Arc::new(AtomicUsize::new(0));
    let lookup_count = lookups.clone();
    let provider = Router::new()
        .route("/v1/apps/silicon-apps/users/alice", get(move || {
            let mode = profile_mode.clone();
            async move {
                let mode = mode.load(Ordering::SeqCst);
                if mode == 1 {
                    return StatusCode::NOT_FOUND.into_response();
                }
                Json(json!({"user":{"uuid":"alice","id":"c:renamed","display_name":if mode == 2 { "" } else { "Alice Current" },"status":"active"}})).into_response()
            }
        }))
        .route("/v1/accounts/alice", get(move || {
            let calls = lookup_count.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Json(json!({"uuid":"alice","kind":"carbon","id":"c:fallback"}))
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.dev_auth = false;
    cfg.accounts_url = url;
    cfg.accounts_app_secret = Some("test-app-secret".into());
    let state = AppState::new(cfg).unwrap();
    seed_private(&state);
    {
        let store = state.store.lock().unwrap();
        let mut catalog = store.catalog().unwrap();
        catalog.apps.get_mut("secret-app").unwrap().visibility = "public".into();
        catalog.invites.push(Invite {
            id: "rename-invite".into(),
            app_id: "secret-app".into(),
            to: "c:alice".into(),
            account_uuid: Some("alice".into()),
            status: "pending".into(),
            created_at: now(),
        });
        store
            .connection
            .execute(
                "UPDATE catalog SET document=?1 WHERE id=1",
                [serde_json::to_string(&catalog).unwrap()],
            )
            .unwrap();
    }
    let app = router(state.clone());
    for (step, expected_id) in [(0, "c:renamed"), (1, "c:fallback"), (2, "c:renamed")] {
        mode.store(step, Ordering::SeqCst);
        state.identity_cache.lock().unwrap().clear();
        let (status, body) = call(&app, "GET", "/v1/apps/secret-app", None, json!({}), None).await;
        assert_eq!(status, 200, "{body}");
        let author = &body["authors"][0];
        assert_eq!(author["id"], expected_id);
        assert_eq!(author["display_name"], "Alice Current");
        assert!(author.get("authors").is_none());
        assert!(author.get("items").is_none());
        let (status, profile) = call(&app, "GET", "/v1/authors/alice", None, json!({}), None).await;
        assert_eq!(status, 200, "{profile}");
        assert_eq!(profile["uuid"], "alice");
        assert_eq!(profile["id"], expected_id);
        assert_eq!(profile["display_name"], "Alice Current");
        assert_eq!(profile["total"], 1);
        assert_eq!(profile["items"][0]["authors"][0]["id"], expected_id);

        let stored = state.store.lock().unwrap().app("secret-app").unwrap();
        assert_eq!(stored.authors[0].display_name, "Alice Current");
        assert_eq!(stored.authors[0].id, expected_id);
        assert_eq!(
            state.store.lock().unwrap().catalog().unwrap().invites[0].to,
            expected_id
        );
        assert_eq!(lookups.load(Ordering::SeqCst), usize::from(step > 0));
    }
    task.abort();
}
#[tokio::test]
async fn malformed_routes_are_rejected_before_contacting_accounts_or_creating_history() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let provider = Router::new().fallback(move || {
        let count = count.clone();
        async move {
            count.fetch_add(1, Ordering::SeqCst);
            Json(json!({"secret":"whsec_should-never-be-created"}))
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.accounts_url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    let state = AppState::new(cfg).unwrap();
    seed_private(&state);
    let before = state
        .store
        .lock()
        .unwrap()
        .app("secret-app")
        .unwrap()
        .history
        .len();
    let app = router(state.clone());
    for (method, path) in [
        ("DELETE", "/v1/apps/secret-app/webhook/rotate"),
        ("POST", "/v1/apps/secret-app/webhook"),
        ("PUT", "/v1/apps/secret-app/webhook/unrecognized"),
        ("PUT", "/v1/apps/secret-app/media"),
        ("DELETE", "/v1/apps/secret-app/packages/linux-x86_64"),
        ("POST", "/v1/apps/secret-app/publish/unrecognized"),
        ("GET", "/v1/apps/secret-app/authors/unrecognized"),
        ("GET", "/v1/apps/availability/example/unrecognized"),
    ] {
        let (status, body) = call(
            &app,
            method,
            path,
            Some("dev:alice:c:alice"),
            json!({"url":"https://example.com/events"}),
            Some("malformed-route-key"),
        )
        .await;
        assert_eq!(status, 404, "{method} {path}: {body}");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        state
            .store
            .lock()
            .unwrap()
            .app("secret-app")
            .unwrap()
            .history
            .len(),
        before
    );
    task.abort();
}
#[tokio::test]
async fn http_mutations_require_keys_hide_drafts_and_do_not_accept_forged_identity_headers() {
    let dir = tempfile::tempdir().unwrap();
    let app = router(AppState::new(config(dir.path())).unwrap());
    let (status, _) = call(
        &app,
        "POST",
        "/v1/apps",
        Some("dev:alice:c:alice"),
        json!({"app_id":"test-app","name":"Test"}),
        None,
    )
    .await;
    assert_eq!(status, 400);
    let (status, created) = call(
        &app,
        "POST",
        "/v1/apps",
        Some("dev:alice:c:alice"),
        json!({"app_id":"test-app","name":"Test"}),
        Some("create-one"),
    )
    .await;
    assert_eq!(status, 200, "{created}");
    assert!(
        created["app_secret"]
            .as_str()
            .unwrap()
            .starts_with("sa_app_")
    );
    let (status, body) = call(&app, "GET", "/v1/apps/test-app", None, json!({}), None).await;
    assert_eq!(status, 404, "{body}");
    let (status, body) = call(
        &app,
        "GET",
        "/v1/apps?mine=true",
        Some("dev:alice:c:alice"),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert!(body.to_string().find("sa_app_").is_none());
    let req = Request::builder()
        .uri("/v1/me")
        .header("X-Account-UUID", "alice")
        .header("X-Account-ID", "c:alice")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(req).await.unwrap().status(), 401);
}
fn seed_private(state: &silicon_apps_server::Shared) {
    let who = Identity {
        uuid: "alice".into(),
        id: "c:alice".into(),
        display_name: "Alice".into(),
        verified_emails: vec![],
    };
    let mut s = state.store.lock().unwrap();
    let mut change = |method: &str, path: &str, key: &str, body: Value, prepared: Prepared| {
        let digest = hash(body.to_string().as_bytes());
        s.mutate(Mutation {
            method,
            path,
            key,
            who: Some(&who),
            body: &body,
            digest: &digest,
            prepared,
        })
        .unwrap();
    };
    change(
        "POST",
        "apps",
        "seed-app",
        json!({"app_id":"secret-app","name":"Secret","description":"a".repeat(200)}),
        Prepared {
            secret: Some("secret".into()),
            ..Default::default()
        },
    );
    let bytes = b"private package";
    let pkg = Package {
        id: "private-pkg".into(),
        target: "linux-x86_64".into(),
        sha256: hash(bytes),
        size: bytes.len() as u64,
        command: "secret-app".into(),
        validation: vec![],
        created_at: now(),
        install_script: None,
        inspected: true,
        author_signature: None,
    };
    std::fs::write(
        state.config.data_dir.join("packages").join(&pkg.sha256),
        bytes,
    )
    .unwrap();
    change(
        "POST",
        "apps/secret-app/packages/linux-x86_64",
        "seed-package",
        json!({}),
        Prepared {
            package: Some(pkg),
            ..Default::default()
        },
    );
    change(
        "POST",
        "apps/secret-app/releases",
        "seed-release",
        json!({"version":"1.0.0","package_ids":["private-pkg"]}),
        Prepared::default(),
    );
    change(
        "POST",
        "apps/secret-app/publish",
        "seed-publish",
        json!({}),
        Prepared::default(),
    );
    change(
        "PUT",
        "apps/secret-app/access",
        "seed-private",
        json!({"visibility":"private","domains":[],"account_ids":[]}),
        Prepared::default(),
    );
}
#[tokio::test]
async fn private_metadata_release_download_and_media_are_authorized_independently() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(config(dir.path())).unwrap();
    seed_private(&state);
    let app = router(state);
    for path in [
        "/v1/apps/secret-app",
        "/v1/apps/secret-app/releases",
        "/v1/apps/secret-app/reviews",
        "/v1/apps/secret-app/packages/private-pkg/download",
        "/v1/apps/secret-app/resolve?target=linux-x86_64&channel=development",
        "/v1/apps/secret-app/media/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        let (status, body) = call(&app, "GET", path, Some("dev:bob:c:bob"), json!({}), None).await;
        assert_eq!(status, 404, "{path}: {body}");
    }
    let request = Request::builder()
        .uri("/v1/apps/secret-app/packages/private-pkg/download")
        .header("Authorization", "Bearer dev:alice:c:alice")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "private package"
    );
}
#[tokio::test]
async fn csrf_and_telemetry_optout_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let app = router(AppState::new(config(dir.path())).unwrap());
    let request = Request::builder()
        .method("POST")
        .uri("/v1/platforms")
        .header("Authorization", "Bearer dev:alice:c:alice")
        .header("Cookie", "apps_session=anything")
        .header("Origin", "https://evil.example")
        .header("Idempotency-Key", "platform-key")
        .body(Body::from(r#"{"target":"linux-x86_64"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(request).await.unwrap().status(), 403);
    let (status, body) = call(
        &app,
        "POST",
        "/v1/telemetry",
        None,
        json!({"step":"page_view","progress":"complete","access_token":"must-not-store"}),
        Some("telemetry-key"),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["accepted"], false);
}
#[tokio::test]
async fn oauth_login_uses_only_configured_origins_and_binds_state_to_browser() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.dev_auth = false;
    cfg.public_url = "https://apps.example.test".into();
    cfg.allowed_origins = vec![
        "https://apps.example.test".into(),
        "https://developer.example.test".into(),
    ];
    cfg.accounts_app_secret = Some("test-secret".into());
    // A deployed service signs with the key from its runtime secret.
    cfg.signing_keys = Some(silicon_apps_server::signing::generate("oauth-test").0);
    let app = router(AppState::new(cfg).unwrap());
    for (host, expected) in [
        (
            "developer.example.test",
            "https://developer.example.test/v1/auth/callback",
        ),
        (
            "attacker.example.test",
            "https://apps.example.test/v1/auth/callback",
        ),
    ] {
        let req = Request::builder()
            .uri("/v1/auth/login?return_to=%2Fdeveloper")
            .header("Host", host)
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), 303);
        let location = url::Url::parse(response.headers()["Location"].to_str().unwrap()).unwrap();
        let query: std::collections::BTreeMap<_, _> = location.query_pairs().into_owned().collect();
        assert_eq!(query["redirect_uri"], expected);
        assert_eq!(query["code_challenge_method"], "S256");
        let cookie = response.headers()["Set-Cookie"].to_str().unwrap();
        assert!(
            cookie.contains("HttpOnly")
                && cookie.contains("Secure")
                && cookie.contains("SameSite=Lax")
        );
        let state = query["state"].clone();
        let (status, _) = call(
            &app,
            "GET",
            &format!("/v1/auth/callback?state={state}&code=stolen"),
            None,
            json!({}),
            None,
        )
        .await;
        assert_eq!(status, 403);
    }
    let (status, _) = call(
        &app,
        "GET",
        "/v1/auth/login?return_to=https%3A%2F%2Fevil.example",
        None,
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, 400);
}
#[tokio::test]
async fn repeated_media_uploads_preserve_object_and_reject_conflicting_mime() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(config(dir.path())).unwrap();
    seed_private(&state);
    let app = router(state.clone());
    // Both JPEG and MP4 signature checks accept this sequence. Once published,
    // its URL must keep the first Content-Type and the same immutable object.
    let bytes = b"\xff\xd8\xff\0ftypabcdefgh";
    let digest = hash(bytes);
    let path = dir.path().join("media/secret-app").join(&digest);
    let upload = |mime: &str, key: &str| {
        Request::builder()
            .method("POST")
            .uri("/v1/apps/secret-app/media")
            .header("Authorization", "Bearer dev:alice:c:alice")
            .header("Content-Type", mime)
            .header("Idempotency-Key", key)
            .body(Body::from(bytes.as_slice()))
            .unwrap()
    };
    assert_eq!(
        app.clone()
            .oneshot(upload("image/jpeg", "media-first"))
            .await
            .unwrap()
            .status(),
        200
    );
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&path).unwrap().ino()
    };
    assert_eq!(
        app.clone()
            .oneshot(upload("image/jpeg", "media-duplicate"))
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        app.clone()
            .oneshot(upload("video/mp4", "media-conflict"))
            .await
            .unwrap()
            .status(),
        409
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/apps/secret-app/media/{digest}"))
                .header("Authorization", "Bearer dev:alice:c:alice")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["Content-Type"], "image/jpeg");
    assert_eq!(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .as_ref(),
        bytes
    );
    assert_eq!(
        state
            .store
            .lock()
            .unwrap()
            .app("secret-app")
            .unwrap()
            .history
            .iter()
            .filter(|event| event.kind == "media.uploaded")
            .count(),
        2
    );
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
        2
    );
}

#[tokio::test]
async fn failed_package_validation_is_durable_idempotent_and_conflicting_retries_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(config(dir.path())).unwrap();
    let app = router(state.clone());
    call(
        &app,
        "POST",
        "/v1/apps",
        Some("dev:alice:c:alice"),
        json!({"app_id":"test-app","name":"Test"}),
        Some("create-package-app"),
    )
    .await;
    let upload = |bytes: &'static str| {
        Request::builder()
            .method("POST")
            .uri("/v1/apps/test-app/packages/linux-x86_64")
            .header("Authorization", "Bearer dev:alice:c:alice")
            .header("Content-Type", "application/gzip")
            .header("Idempotency-Key", "invalid-package-once")
            .body(Body::from(bytes))
            .unwrap()
    };
    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(upload("not a gzip archive"))
            .await
            .unwrap();
        assert_eq!(response.status(), 422);
    }
    let app_state = state.store.lock().unwrap().app("test-app").unwrap();
    let failures: Vec<_> = app_state
        .history
        .iter()
        .filter(|e| e.kind == "package.validation_failed")
        .collect();
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].idempotency_key.as_deref(),
        Some("invalid-package-once")
    );
    assert_eq!(
        app.oneshot(upload("different bytes"))
            .await
            .unwrap()
            .status(),
        409
    );
}
#[tokio::test]
async fn tam_counts_distinct_authenticated_accounts_across_selected_targets() {
    let dir = tempfile::tempdir().unwrap();
    let app = router(AppState::new(config(dir.path())).unwrap());
    for (target, key) in [
        ("linux-x86_64", "platform-one"),
        ("macos-aarch64", "platform-two"),
    ] {
        assert_eq!(
            call(
                &app,
                "POST",
                "/v1/platforms",
                Some("dev:alice:c:alice"),
                json!({"target":target}),
                Some(key)
            )
            .await
            .0,
            200
        );
    }
    let (status, result) = call(
        &app,
        "GET",
        "/v1/targets?targets=linux-x86_64,macos-aarch64",
        None,
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(result["total_population"], 1);
    assert_eq!(result["total_reach"], 1);
    assert_eq!(result["source"], "registered_accounts");
}
#[tokio::test]
async fn ambiguous_accounts_registration_recovers_same_secret_after_restart() {
    use std::sync::{Arc, Mutex};
    let documents = Arc::new(Mutex::new(Vec::<Value>::new()));
    let submitted = documents.clone();
    let registered = documents.clone();
    let provider=Router::new().route("/v1/internal/apps",get(move||{let registered=registered.clone();async move{Json(json!({"apps":if registered.lock().unwrap().is_empty(){vec![]}else{vec![json!({"app_id":"recover-app"})]}}))}})).route("/v1/internal/apps/sync",axum::routing::post(move|Json(body):Json<Value>|{let submitted=submitted.clone();async move{let mut docs=submitted.lock().unwrap();docs.push(body);if docs.len()==1{(StatusCode::SERVICE_UNAVAILABLE,Json(json!({"error":{"code":"lost_response","message":"Simulated response loss after commit"}})))}else{(StatusCode::OK,Json(json!({"synced":1})))}}}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.accounts_url = issuer;
    cfg.accounts_service_token = Some("server-test-token".into());
    let state = AppState::new(cfg.clone()).unwrap();
    let app = router(state.clone());
    let payload = json!({"app_id":"recover-app","name":"Recovery"});
    let (status, _) = call(
        &app,
        "POST",
        "/v1/apps",
        Some("dev:alice:c:alice"),
        payload.clone(),
        Some("recover-original-key"),
    )
    .await;
    assert_eq!(status, 503);
    assert!(state.store.lock().unwrap().app("recover-app").is_err());
    drop(app);
    drop(state);
    let state = AppState::new(cfg).unwrap();
    let app = router(state.clone());
    let (status, created) = call(
        &app,
        "POST",
        "/v1/apps",
        Some("dev:alice:c:alice"),
        payload,
        Some("recover-original-key"),
    )
    .await;
    assert_eq!(status, 200, "{created}");
    let docs = documents.lock().unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0]["apps"][0]["secret"], docs[1]["apps"][0]["secret"]);
    assert_eq!(created["app_secret"], docs[1]["apps"][0]["secret"]);
    let pending: i64 = state
        .store
        .lock()
        .unwrap()
        .connection
        .query_row("SELECT count(*) FROM pending_secrets", [], |r| r.get(0))
        .unwrap();
    assert_eq!(pending, 0);
    task.abort();
}

#[tokio::test]
async fn account_uuid_backfill_preserves_signed_private_catalog_and_denies_old_jwt() {
    let key = SigningKey::from_bytes(&[42u8; 32]);
    let x = URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes());
    let provider = Router::new()
        .route("/.well-known/jwks.json", get(move || {let x=x.clone();async move {Json(json!({"keys":[{"kty":"OKP","crv":"Ed25519","alg":"EdDSA","kid":"test-key","x":x}]}))}}))
        .route("/v1/userinfo", get(|headers: axum::http::HeaderMap| async move {
            let token=headers["authorization"].to_str().unwrap().trim_start_matches("Bearer ");
            let claims:Value=serde_json::from_slice(&URL_SAFE_NO_PAD.decode(token.split('.').nth(1).unwrap()).unwrap()).unwrap();
            Json(json!({"uuid":claims["sub"],"kind":"carbon","id":"c:alice","display_name":"Alice"}))
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.dev_auth = false;
    cfg.accounts_url = issuer.clone();
    let state = AppState::new(cfg).unwrap();
    seed_private(&state);
    let app = router(state.clone());
    let old = signed(
        &key,
        &issuer,
        "silicon-apps",
        chrono::Utc::now().timestamp() + 600,
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/v1/apps/secret-app",
            Some(&old),
            json!({}),
            None
        )
        .await
        .0,
        200
    );
    let original = state
        .store
        .lock()
        .unwrap()
        .catalog()
        .unwrap()
        .apps
        .remove("secret-app")
        .unwrap();
    let package_path = dir
        .path()
        .join("packages")
        .join(&original.packages[0].sha256);
    let bytes = std::fs::read(&package_path).unwrap();
    let new = "f858d0b5-98ba-4a4d-8ce5-114e93136f23";
    let mapping = dir.path().join("mapping.csv");
    std::fs::write(
        &mapping,
        format!("old_uuid,new_uuid,kind\nalice,{new},carbon\n"),
    )
    .unwrap();
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/migrate_account_uuids.py");
    let output = std::process::Command::new("python3")
        .arg(script)
        .arg(&mapping)
        .arg("--database")
        .arg(dir.path().join("apps.sqlite"))
        .arg("--apply")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        call(&app, "GET", "/v1/me", Some(&old), json!({}), None)
            .await
            .0,
        401
    );
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"EdDSA","kid":"test-key","typ":"JWT"}"#);
    let payload=URL_SAFE_NO_PAD.encode(json!({"iss":issuer,"sub":new,"aud":"silicon-apps","exp":chrono::Utc::now().timestamp()+600,"id":"c:alice","kind":"carbon"}).to_string());
    let message = format!("{header}.{payload}");
    let fresh = format!(
        "{message}.{}",
        URL_SAFE_NO_PAD.encode(key.sign(message.as_bytes()).to_bytes())
    );
    let (status, body) = call(
        &app,
        "GET",
        "/v1/apps/secret-app",
        Some(&fresh),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["authors"][0]["uuid"], new);
    let migrated = state
        .store
        .lock()
        .unwrap()
        .catalog()
        .unwrap()
        .apps
        .remove("secret-app")
        .unwrap();
    assert_eq!(
        serde_json::to_value(&migrated.releases).unwrap(),
        serde_json::to_value(&original.releases).unwrap()
    );
    assert_eq!(std::fs::read(&package_path).unwrap(), bytes);
    assert_eq!(migrated.secret_hash, original.secret_hash);
    assert_eq!(migrated.app_id, original.app_id);
    assert!(migrated.history.iter().all(|e| e.actor_uuid == new));
    task.abort();
}
