use axum::{
    Json, Router,
    routing::{get, post},
};
use serde_json::{Value, json};
use silicon_apps_client::{Client, Config, LocalState, auth, state};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn tokens(expires: u64) -> silicon_accounts_client::TokenResponse {
    serde_json::from_value(json!({"access_token":"original-access","refresh_token":"original-refresh","expires_in":expires})).unwrap()
}
async fn recording_server() -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let router = Router::new().fallback(move || {
        let calls = observed.clone();
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Json(json!({}))
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    (
        format!("http://{address}"),
        calls,
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        }),
    )
}

#[tokio::test]
async fn saved_access_and_refresh_tokens_never_cross_apps_or_accounts_service_scopes() {
    let (other, calls, task) = recording_server().await;
    let home = tempfile::tempdir().unwrap();
    let local = LocalState::new(home.path()).unwrap();
    let original = Config {
        server: "http://127.0.0.1:4310/tenant-a".into(),
        accounts_url: "https://accounts.example.com".into(),
        ..Config::default()
    };
    for expires in [0, 3600] {
        auth::save(&local, &original, tokens(expires)).unwrap();
        for changed in [
            Config {
                server: other.clone(),
                ..original.clone()
            },
            Config {
                accounts_url: "https://unrelated.example.com".into(),
                ..original.clone()
            },
            Config {
                server: "http://127.0.0.1:4310/tenant-b".into(),
                ..original.clone()
            },
        ] {
            let error = auth::authenticated_client(&local, &changed, None)
                .await
                .err()
                .unwrap();
            assert!(
                error
                    .to_string()
                    .contains("No saved access or refresh token was sent")
            );
            assert!(
                auth::logout(
                    &local,
                    &changed,
                    &Client::new(&changed.server, None).unwrap()
                )
                .await
                .is_err()
            );
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(local.root.join("session.json").exists());
    task.abort();
}

#[tokio::test]
async fn unscoped_legacy_session_requires_login_before_any_token_reuse() {
    let (url, calls, task) = recording_server().await;
    let home = tempfile::tempdir().unwrap();
    let local = LocalState::new(home.path()).unwrap();
    local.initialize().unwrap();
    state::atomic_json(
        &local.root.join("session.json"),
        &json!({"tokens":tokens(0),"expires_at":0}),
    )
    .unwrap();
    let config = Config {
        server: url,
        ..Config::default()
    };
    let error = auth::authenticated_client(&local, &config, None)
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("predates server scoping"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    task.abort();
}

#[tokio::test]
async fn same_service_refresh_rotates_and_retains_explicit_scope() {
    let app=Router::new().route("/v1/auth/refresh",post(|Json(body):Json<Value>|async move{assert_eq!(body["refresh_token"],"original-refresh");Json(json!({"access_token":"rotated-access","refresh_token":"rotated-refresh","expires_in":3600}))})).route("/v1/me",get(|headers:axum::http::HeaderMap|async move{assert_eq!(headers["Authorization"],"Bearer rotated-access");Json(json!({"uuid":"account-a"}))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let config = Config {
        server: format!("http://{address}/"),
        ..Config::default()
    };
    let home = tempfile::tempdir().unwrap();
    let local = LocalState::new(home.path()).unwrap();
    auth::save(&local, &config, tokens(0)).unwrap();
    let client = auth::authenticated_client(&local, &config, None)
        .await
        .unwrap();
    assert_eq!(client.me().await.unwrap()["uuid"], "account-a");
    let saved = auth::read(&local).unwrap().unwrap();
    assert_eq!(saved.server, format!("http://{address}"));
    assert_eq!(
        saved.tokens.refresh_token.unwrap().expose(),
        "rotated-refresh"
    );
    task.abort();
}
