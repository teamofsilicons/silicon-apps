//! Shared fixtures for the discovery, event and subscription tests.
#![allow(dead_code)]
use axum::{
    Json, Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    routing::post,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use silicon_apps_server::{
    AppState, Shared,
    config::Config,
    model::*,
    router,
    store::{Mutation, Prepared, hash},
};
use std::{net::SocketAddr, time::Duration};
use tower::ServiceExt;

pub const ALICE: &str = "dev:alice:c:alice";
pub const BOB: &str = "dev:bob:c:bob";
pub const CAROL: &str = "dev:carol:si:carol";

pub fn config(dir: &std::path::Path) -> Config {
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

pub struct Response {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
}
pub async fn send(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
    key: Option<&str>,
    extra: &[(&str, &str)],
) -> Response {
    let mut b = Request::builder().method(method).uri(path);
    if body.is_some() {
        b = b.header("Content-Type", "application/json");
    }
    if let Some(t) = token {
        b = b.header("Authorization", format!("Bearer {t}"));
    }
    if let Some(k) = key {
        b = b.header("Idempotency-Key", k);
    }
    for (name, value) in extra {
        b = b.header(*name, *value);
    }
    let request = b
        .body(match body {
            Some(v) => Body::from(v.to_string()),
            None => Body::empty(),
        })
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Response {
        status,
        headers,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}
pub async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
    key: Option<&str>,
) -> (StatusCode, Value) {
    let r = send(app, method, path, token, Some(body), key, &[]).await;
    (r.status, r.body)
}

/// Apply a catalog mutation directly, as the HTTP layer does after its checks.
pub fn mutate(
    state: &Shared,
    who: &str,
    method: &str,
    path: &str,
    key: &str,
    body: Value,
    prepared: Prepared,
) -> Value {
    let identity = Identity {
        uuid: who.into(),
        id: format!("c:{who}"),
        display_name: who.into(),
        verified_emails: vec![],
    };
    let digest = hash(body.to_string().as_bytes());
    state
        .store
        .lock()
        .unwrap()
        .mutate(Mutation {
            method,
            path,
            key,
            who: Some(&identity),
            body: &body,
            digest: &digest,
            prepared,
        })
        .unwrap()
        .0
}

/// A published app by `author` with a validated package and a development
/// release, without a runner. Returns (package_id, development release id).
pub fn seed_app(
    state: &Shared,
    author: &str,
    app_id: &str,
    tags: &[&str],
    target: &str,
    publish: bool,
) -> (String, String) {
    mutate(
        state,
        author,
        "POST",
        "apps",
        &format!("{app_id}-create"),
        json!({"app_id":app_id,"name":format!("{app_id} app"),"description":"d".repeat(200)}),
        Prepared {
            secret: Some("secret".into()),
            ..Default::default()
        },
    );
    mutate(
        state,
        author,
        "PATCH",
        &format!("apps/{app_id}"),
        &format!("{app_id}-tags"),
        json!({"tags":tags}),
        Prepared::default(),
    );
    let bytes = format!("package for {app_id} {target}");
    let package = Package {
        id: format!("{app_id}-{target}-pkg"),
        target: target.into(),
        sha256: hash(bytes.as_bytes()),
        size: bytes.len() as u64,
        command: app_id.into(),
        validation: vec![],
        created_at: now(),
        install_script: None,
        inspected: true,
        author_signature: None,
    };
    std::fs::write(
        state.config.data_dir.join("packages").join(&package.sha256),
        bytes,
    )
    .unwrap();
    let package_id = package.id.clone();
    mutate(
        state,
        author,
        "POST",
        &format!("apps/{app_id}/packages/{target}"),
        &format!("{app_id}-package"),
        json!({}),
        Prepared {
            package: Some(package),
            ..Default::default()
        },
    );
    let release = mutate(
        state,
        author,
        "POST",
        &format!("apps/{app_id}/releases"),
        &format!("{app_id}-release"),
        json!({"version":"0.1.0","package_ids":[package_id]}),
        Prepared::default(),
    );
    if publish {
        mutate(
            state,
            author,
            "POST",
            &format!("apps/{app_id}/publish"),
            &format!("{app_id}-publish"),
            json!({}),
            Prepared::default(),
        );
    }
    (package_id, release["id"].as_str().unwrap().to_owned())
}

/// Serve the router on a real loopback port.
pub async fn serve(state: Shared) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = router(state).into_make_service_with_connect_info::<SocketAddr>();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, task)
}
pub fn fresh() -> (tempfile::TempDir, Shared, Router) {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(config(dir.path())).unwrap();
    let app = router(state.clone());
    (dir, state, app)
}

/// Minimal SSE reader over a live response.
pub struct Sse {
    response: reqwest::Response,
    buffer: String,
    pub comments: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub id: String,
    pub event: String,
    pub data: Value,
}
impl Sse {
    pub async fn open(base: &str, path: &str, token: &str, last_event_id: Option<&str>) -> Self {
        let mut request = reqwest::Client::new()
            .get(format!("{base}{path}"))
            .bearer_auth(token);
        if let Some(id) = last_event_id {
            request = request.header("Last-Event-ID", id);
        }
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), 200, "{path}");
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("text/event-stream")
        );
        Self {
            response,
            buffer: String::new(),
            comments: vec![],
        }
    }
    /// The next event, or None when nothing arrives within `wait`.
    pub async fn next(&mut self, wait: Duration) -> Option<SseEvent> {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            while let Some(end) = self.buffer.find("\n\n") {
                let block: String = self.buffer.drain(..end + 2).collect();
                let (mut id, mut event, mut data) = (String::new(), String::new(), None);
                for line in block.lines() {
                    if let Some(comment) = line.strip_prefix(':') {
                        self.comments.push(comment.trim().to_owned());
                    } else if let Some(v) = line.strip_prefix("id: ") {
                        id = v.into();
                    } else if let Some(v) = line.strip_prefix("event: ") {
                        event = v.into();
                    } else if let Some(v) = line.strip_prefix("data: ") {
                        data = Some(serde_json::from_str(v).unwrap());
                    }
                }
                if let Some(data) = data {
                    return Some(SseEvent { id, event, data });
                }
            }
            let chunk = tokio::time::timeout_at(deadline, self.response.chunk())
                .await
                .ok()?
                .ok()??;
            self.buffer.push_str(&String::from_utf8_lossy(&chunk));
        }
    }
}

/// A fake isolated runner. `accounts_app_id` is what `accounts --json` reports.
pub async fn fake_runner(accounts_app_id: &'static str) -> (String, tokio::task::JoinHandle<()>) {
    let runner = Router::new().route(
        "/validate",
        post(move |Json(body): Json<Value>| async move {
            Json(json!({
                "isolated":true,
                "target":body["target"],
                "validation":[
                    {"command":"--help","exit_code":0,"stdout":"usage: demo","stderr":""},
                    {"command":"accounts --json","exit_code":0,"stdout":json!({"app_id":accounts_app_id}).to_string(),"stderr":""},
                    {"command":"login status --json","exit_code":0,"stdout":"{\"authenticated\":false}","stderr":""}
                ]
            }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, runner).await.unwrap() });
    (url, task)
}
/// A real package archive for `app_id` on `target`.
pub fn package_bytes(app_id: &str, target: &str) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("bin")).unwrap();
    std::fs::write(
        dir.path().join("apps.yaml"),
        format!(
            "schema_version: 1\napp_id: {app_id}\nversion: 0.1.0\ncommand: {app_id}\ntargets:\n  {target}:\n    binary: bin/{app_id}\n"
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
    silicon_apps_package::pack_directory(dir.path()).unwrap()
}
pub async fn wait_until<F: FnMut() -> bool>(mut check: F, seconds: u64) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    while tokio::time::Instant::now() < deadline {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    check()
}
