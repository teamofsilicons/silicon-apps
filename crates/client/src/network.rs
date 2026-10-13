use anyhow::{Context, Result};
use reqwest::{RequestBuilder, Response};
use std::time::Duration;

/// Retry only a timeout establishing the connection, before an HTTP request
/// reaches the service. Keep the same body and idempotency key on every attempt.
/// Response/read timeouts have an unknown outcome and must not replay mutations.
pub(crate) async fn send(mut request: RequestBuilder) -> Result<Response> {
    const DELAYS: [Duration; 2] = [Duration::from_millis(250), Duration::from_secs(1)];
    for attempt in 0..=DELAYS.len() {
        let next = request.try_clone();
        match request.send().await {
            Ok(response) => return Ok(response),
            Err(error) => {
                if error.is_connect() && error.is_timeout() {
                    if let (Some(next), Some(delay)) = (next, DELAYS.get(attempt)) {
                        tokio::time::sleep(*delay).await;
                        request = next;
                        continue;
                    }
                    return Err(error).with_context(|| {
                        format!(
                            "connection timed out after {} attempt(s); check DNS, internet connectivity, and proxy/VPN access to the configured Silicon Apps server, then retry",
                            attempt + 1
                        )
                    });
                }
                return Err(error.into());
            }
        }
    }
    unreachable!("the final attempt always returns")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, http::StatusCode, routing::post};
    use reqwest::dns::{Addrs, Name, Resolve, Resolving};
    use std::{
        net::SocketAddr,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    struct DelayedDns {
        calls: AtomicUsize,
        timeouts: usize,
        address: SocketAddr,
    }

    impl Resolve for DelayedDns {
        fn resolve(&self, _: Name) -> Resolving {
            let timeout = self.calls.fetch_add(1, Ordering::SeqCst) < self.timeouts;
            let address = self.address;
            Box::pin(async move {
                if timeout {
                    std::future::pending::<()>().await;
                }
                Ok(Box::new(std::iter::once(address)) as Addrs)
            })
        }
    }

    async fn fixture(
        timeouts: usize,
        response_delay: Duration,
        status: StatusCode,
    ) -> (
        reqwest::Client,
        Arc<DelayedDns>,
        Arc<AtomicUsize>,
        tokio::task::JoinHandle<()>,
    ) {
        let received = Arc::new(AtomicUsize::new(0));
        let count = received.clone();
        let app = Router::new().route(
            "/install",
            post(move |headers: axum::http::HeaderMap, body: String| {
                let count = count.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(headers["Idempotency-Key"], "same-operation");
                    assert_eq!(body, "same-body");
                    tokio::time::sleep(response_delay).await;
                    (status, "ok")
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let resolver = Arc::new(DelayedDns {
            calls: AtomicUsize::new(0),
            timeouts,
            address: listener.local_addr().unwrap(),
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .dns_resolver(resolver.clone())
            .connect_timeout(Duration::from_millis(50))
            .timeout(Duration::from_millis(500))
            .build()
            .unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (client, resolver, received, server)
    }

    fn request(client: &reqwest::Client) -> RequestBuilder {
        client
            .post("http://retry.invalid/install")
            .header("Idempotency-Key", "same-operation")
            .body("same-body")
    }

    #[tokio::test]
    async fn recovers_after_two_connection_timeouts_without_replaying_mutation() {
        let (client, dns, received, server) = fixture(2, Duration::ZERO, StatusCode::OK).await;
        let response = send(request(&client)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "ok");
        assert_eq!(dns.calls.load(Ordering::SeqCst), 3);
        assert_eq!(received.load(Ordering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn persistent_connection_timeout_is_bounded_and_actionable() {
        let (client, dns, received, server) =
            fixture(usize::MAX, Duration::ZERO, StatusCode::OK).await;
        let error = send(request(&client)).await.unwrap_err();
        assert!(format!("{error:#}").contains("connection timed out after 3 attempt(s)"));
        assert_eq!(dns.calls.load(Ordering::SeqCst), 3);
        assert_eq!(received.load(Ordering::SeqCst), 0);
        server.abort();
    }

    #[tokio::test]
    async fn response_timeout_does_not_repeat_a_received_mutation() {
        let (client, dns, received, server) =
            fixture(0, Duration::from_secs(2), StatusCode::OK).await;
        let error = send(request(&client)).await.unwrap_err();
        let error = error.downcast_ref::<reqwest::Error>().unwrap();
        assert!(error.is_timeout());
        assert!(!error.is_connect());
        assert_eq!(dns.calls.load(Ordering::SeqCst), 1);
        assert_eq!(received.load(Ordering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn server_errors_are_returned_without_retry() {
        let (client, dns, received, server) =
            fixture(0, Duration::ZERO, StatusCode::SERVICE_UNAVAILABLE).await;
        assert_eq!(
            send(request(&client)).await.unwrap().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(dns.calls.load(Ordering::SeqCst), 1);
        assert_eq!(received.load(Ordering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn non_timeout_connection_errors_are_not_retried() {
        let (client, dns, received, server) = fixture(0, Duration::ZERO, StatusCode::OK).await;
        server.abort();
        let _ = server.await;
        let error = send(request(&client)).await.unwrap_err();
        let error = error.downcast_ref::<reqwest::Error>().unwrap();
        assert!(error.is_connect());
        assert!(!error.is_timeout());
        assert_eq!(dns.calls.load(Ordering::SeqCst), 1);
        assert_eq!(received.load(Ordering::SeqCst), 0);
    }
}
