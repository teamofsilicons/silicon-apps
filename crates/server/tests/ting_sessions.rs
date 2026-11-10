mod common;
use axum::{Json, Router, body::Body, http::Request, routing::post};
use common::*;
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use silicon_apps_server::{AppState, router, store::Prepared};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::Notify, task::JoinHandle};
use tokio_tungstenite::tungstenite::Message;
use tower::ServiceExt;

#[derive(Default)]
struct Wire {
    sent: Mutex<Vec<Value>>,
    receivers: Mutex<std::collections::BTreeMap<String, tokio::sync::mpsc::UnboundedSender<Value>>>,
    active: AtomicUsize,
    fail: AtomicBool,
    hold: AtomicBool,
    arrived: Notify,
    release: Notify,
    proofs: AtomicUsize,
}
struct Mock {
    url: String,
    accounts: String,
    wire: Arc<Wire>,
    tasks: Vec<JoinHandle<()>>,
}
impl Drop for Mock {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
async fn mock() -> Mock {
    let wire = Arc::new(Wire::default());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let w = wire.clone();
    let task = tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let w = w.clone();
            tokio::spawn(async move {
                let mut socket = tokio_tungstenite::accept_async(socket).await.unwrap();
                let id = uuid::Uuid::new_v4().to_string();
                w.active.fetch_add(1, Ordering::SeqCst);
                socket
                    .send(Message::Text(
                        json!({"op":"ready","protocol":"v1","session_id":id})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
                loop {
                    let frame = tokio::select! {
                        frame = socket.next() => match frame { Some(Ok(frame)) => frame, _ => break },
                        Some(value) = rx.recv() => {
                            if socket.send(Message::Text(value.to_string().into())).await.is_err() { break; }
                            continue;
                        }
                    };
                    let text = match frame {
                        Message::Text(text) => text,
                        Message::Ping(p) => {
                            if socket.send(Message::Pong(p)).await.is_err() {
                                break;
                            }
                            continue;
                        }
                        Message::Close(_) => break,
                        _ => continue,
                    };
                    let frame: Value = serde_json::from_str(&text).unwrap();
                    let reply = match frame["op"].as_str() {
                        Some("register") => {
                            assert_eq!(frame["app_ids"], json!(["silicon-apps"]));
                            assert!(frame.get("session_token").is_none());
                            w.receivers.lock().unwrap().insert(id.clone(), tx.clone());
                            json!({"op":"registered","request_id":frame["request_id"],"session_id":id,"for":null,"app_ids":["silicon-apps"]})
                        }
                        Some("send") => {
                            assert_eq!(frame["proof_token"], "sap_test_only");
                            let body: Value =
                                serde_json::from_str(frame["body"].as_str().unwrap()).unwrap();
                            let recipient = body["session_id"].as_str().unwrap().to_owned();
                            assert_ne!(recipient, id);
                            assert_eq!(body["type"], "silicon-apps.eventsAvailable");
                            assert_eq!(body["data"], json!({}));
                            assert_eq!(body["metadata"], json!({}));
                            assert!(body.get("for").is_none());
                            w.sent.lock().unwrap().push(body.clone());
                            w.arrived.notify_one();
                            if w.fail.load(Ordering::SeqCst) {
                                break;
                            }
                            if w.hold.load(Ordering::SeqCst) {
                                w.release.notified().await;
                            }
                            // Deliver to the registered receiver, not the publisher's socket.
                            if let Some(receiver) = w.receivers.lock().unwrap().get(&recipient) {
                                let _ = receiver.send(
                                    json!({"op":"tings","session_id":recipient,"tings":[body]}),
                                );
                            }
                            json!({"op":"accepted","request_id":frame["request_id"],"result":{"id":"message","status":"accepted"}})
                        }
                        _ => panic!("unexpected command"),
                    };
                    if socket
                        .send(Message::Text(reply.to_string().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                w.receivers.lock().unwrap().remove(&id);
                w.active.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let accounts = format!("http://{}", listener.local_addr().unwrap());
    let w = wire.clone();
    let app = Router::new().route("/v1/proofs/app-verification", post(move |Json(body): Json<Value>| {
        let w = w.clone();
        async move {
            w.proofs.fetch_add(1, Ordering::SeqCst);
            assert_eq!(body["receiving_app"], "ting");
            assert_eq!(body["scopes"], json!(["tings.send"]));
            Json(json!({"proof_token":"sap_test_only","kind":"app_verification","issuing_app":"silicon-apps","receiving_app":"ting","scopes":["tings.send"],"user":null,"expires_at":(chrono::Utc::now()+chrono::Duration::seconds(60)).to_rfc3339()}))
        }
    }));
    let proof = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Mock {
        url,
        accounts,
        wire,
        tasks: vec![task, proof],
    }
}
async fn fixture(mock: &Mock) -> (tempfile::TempDir, silicon_apps_server::Shared, Router) {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.ting_url = Some(mock.url.clone());
    cfg.accounts_url = mock.accounts.clone();
    cfg.accounts_app_secret = Some("test-secret".into());
    let s = AppState::new(cfg).unwrap();
    for (author, app) in [("alice", "alice-app"), ("bob", "bob-app")] {
        mutate(
            &s,
            author,
            "POST",
            "apps",
            &format!("create-{app}"),
            json!({"app_id":app,"name":app}),
            Prepared {
                secret: Some("fixture-secret".into()),
                ..Default::default()
            },
        );
    }
    silicon_apps_server::start_background(s.clone())
        .await
        .unwrap();
    let app = router(s.clone());
    (dir, s, app)
}
async fn open(app: &Router, token: &str, query: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/events/stream?{query}"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn event(body: &mut Body) -> Option<Value> {
    tokio::time::timeout(Duration::from_secs(4), async {
        while let Some(frame) = body.frame().await {
            if let Ok(bytes) = frame.unwrap().into_data() {
                let text = String::from_utf8(bytes.to_vec()).unwrap();
                if let Some(data) = text.lines().find_map(|line| line.strip_prefix("data: ")) {
                    return Some(serde_json::from_str(data).unwrap());
                }
            }
        }
        None
    })
    .await
    .expect("event stream stalled")
}
// Poll through the opening comment and initial catch-up before committing an event.
async fn listening(mut body: Body) -> JoinHandle<(Option<Value>, Body)> {
    let (ready, wait) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let value = {
            let future = event(&mut body);
            tokio::pin!(future);
            assert!(futures_util::poll!(&mut future).is_pending());
            ready.send(()).unwrap();
            future.await
        };
        (value, body)
    });
    wait.await.unwrap();
    task
}

fn change(s: &silicon_apps_server::Shared, author: &str, app: &str, key: &str) {
    mutate(
        s,
        author,
        "PATCH",
        &format!("apps/{app}"),
        key,
        json!({"name":key}),
        Prepared::default(),
    );
}

#[tokio::test]
async fn receivers_are_isolated_content_free_and_replay_after_reconnect() {
    let mock = mock().await;
    let (_dir, s, app) = fixture(&mock).await;
    let first = open(&app, ALICE, "transport=ting").await;
    assert_eq!(first.status(), 200);
    assert_eq!(first.headers()["X-Apps-Event-Transport"], "ting");
    let id = first.headers()["X-Apps-Ting-Session"]
        .to_str()
        .unwrap()
        .to_owned();
    let first = listening(first.into_body()).await;
    let second = open(&app, BOB, "transport=ting").await;
    assert_ne!(second.headers()["X-Apps-Ting-Session"], id);
    let second = listening(second.into_body()).await;
    change(&s, "alice", "alice-app", "private-alice-change");
    change(&s, "bob", "bob-app", "private-bob-change");
    let (a, first) = first.await.unwrap();
    let (b, second) = second.await.unwrap();
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a["app_id"], "alice-app");
    assert_eq!(b["app_id"], "bob-app");
    assert_eq!(mock.wire.sent.lock().unwrap().len(), 2);
    assert_eq!(mock.wire.proofs.load(Ordering::SeqCst), 1);
    drop(first);
    change(&s, "alice", "alice-app", "change-during-reconnect");
    let resumed = open(
        &app,
        ALICE,
        &format!("transport=ting&last_event_id={}", a["seq"]),
    )
    .await;
    assert_ne!(resumed.headers()["X-Apps-Ting-Session"], id);
    assert_eq!(
        event(&mut resumed.into_body()).await.unwrap()["data"]["name"],
        "change-during-reconnect"
    );
    drop(second);
    tokio::time::timeout(Duration::from_secs(2), async {
        while !mock.wire.receivers.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        s.store
            .lock()
            .unwrap()
            .connection
            .query_row("SELECT COUNT(*) FROM subscriptions", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn failed_send_falls_back_without_losing_events_but_required_ting_closes() {
    let mock = mock().await;
    let (_dir, s, app) = fixture(&mock).await;
    mock.wire.fail.store(true, Ordering::SeqCst);
    let automatic = listening(open(&app, ALICE, "transport=auto").await.into_body()).await;
    change(&s, "alice", "alice-app", "fallback-change");
    let (value, _automatic) = automatic.await.unwrap();
    let value = value.unwrap();
    assert_eq!(value["data"]["name"], "fallback-change");
    let required = open(
        &app,
        ALICE,
        &format!("transport=ting&after={}", value["seq"]),
    )
    .await
    .into_body();
    let required = listening(required).await;
    change(&s, "alice", "alice-app", "saved-despite-ting-failure");
    assert!(required.await.unwrap().0.is_none());
    let mut resumed = open(
        &app,
        ALICE,
        &format!("transport=direct&after={}", value["seq"]),
    )
    .await
    .into_body();
    assert_eq!(
        event(&mut resumed).await.unwrap()["data"]["name"],
        "saved-despite-ting-failure"
    );
}

#[tokio::test]
async fn pausing_subscription_during_ting_delivery_discards_in_flight_events() {
    let mock = mock().await;
    let (_dir, s, app) = fixture(&mock).await;
    let (_, sub) = call(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(ALICE),
        json!({"delivery":{"mode":"stream"}}),
        Some("create-stream-sub"),
    )
    .await;
    let id = sub["subscription"]["id"].as_str().unwrap();
    let body = open(&app, ALICE, &format!("transport=ting&subscription={id}"))
        .await
        .into_body();
    mock.wire.hold.store(true, Ordering::SeqCst);
    let reading = listening(body).await;
    change(&s, "alice", "alice-app", "never-delivered-after-pause");
    tokio::time::timeout(Duration::from_secs(3), mock.wire.arrived.notified())
        .await
        .unwrap();
    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/v1/subscriptions/{id}"),
        Some(ALICE),
        json!({"status":"paused"}),
        Some("pause-before-delivery"),
    )
    .await;
    assert_eq!(status, 200);
    mock.wire.release.notify_one();
    assert!(reading.await.unwrap().0.is_none());
}

#[tokio::test]
async fn disabled_invalid_or_unauthorized_transports_fail_before_opening_a_receiver() {
    let (_dir, _s, app) = fresh();
    assert_eq!(open(&app, ALICE, "transport=ting").await.status(), 503);
    assert_eq!(open(&app, ALICE, "transport=unknown").await.status(), 400);
    assert_eq!(
        open(&app, ALICE, "transport=auto").await.headers()["X-Apps-Event-Transport"],
        "direct"
    );
    let mock = mock().await;
    let (_dir, _s, app) = fixture(&mock).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/events/stream?transport=ting")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(mock.wire.active.load(Ordering::SeqCst), 0);
}
