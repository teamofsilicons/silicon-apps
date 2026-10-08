use axum::{Router, http::HeaderMap, routing::post};
use silicon_apps_client::Client;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn malformed_successful_mutation_response_preserves_retry_key() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let keys = received.clone();
    let router = Router::new().route(
        "/v1/apps",
        post(move |headers: HeaderMap| {
            let keys = keys.clone();
            async move {
                keys.lock()
                    .unwrap()
                    .push(headers["Idempotency-Key"].to_str().unwrap().to_owned());
                "upstream committed this mutation, but its JSON body was truncated"
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = Client::new(&format!("http://{}", listener.local_addr().unwrap()), None).unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    for key in [None, Some("retry-create-fixture-1")] {
        let error = client
            .create("fixture", "Fixture", "", None, key)
            .await
            .unwrap_err();
        let sent_key = received.lock().unwrap().last().unwrap().clone();
        assert!(format!("{error:#}").contains(&format!("--idempotency-key {sent_key}")));
        if let Some(key) = key {
            assert_eq!(sent_key, key);
        } else {
            assert!(uuid::Uuid::parse_str(&sent_key).is_ok());
        }
    }
    server.abort();
}
