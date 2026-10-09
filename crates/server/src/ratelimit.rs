//! Per-client rate limits: token buckets for reads and writes, and a cap on
//! open event streams. Clients are identified by a hash of their bearer token
//! or session cookie, otherwise by their address. Requests that arrive from a
//! loopback peer without `X-Forwarded-For` come from a trusted local caller,
//! such as the store's own server, and are not limited.

use crate::error::ApiError;
use axum::http::{HeaderMap, StatusCode};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Instant,
};

pub const WINDOW_SECONDS: u64 = 60;

#[derive(Clone, Debug)]
pub struct ClientKey(pub Option<String>);
impl ClientKey {
    pub fn from_request(headers: &HeaderMap, peer: Option<SocketAddr>) -> Self {
        let digest = |kind: &str, value: &str| {
            Some(format!(
                "{kind}:{}",
                &hex::encode(Sha256::digest(value.as_bytes()))[..24]
            ))
        };
        if let Some(token) = headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .filter(|v| !v.is_empty())
        {
            return Self(digest("token", token));
        }
        if let Some(session) = headers
            .get("Cookie")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                v.split(';').find_map(|part| {
                    let (k, v) = part.trim().split_once('=')?;
                    (k == "apps_session" && !v.is_empty()).then(|| v.to_owned())
                })
            })
        {
            return Self(digest("session", &session));
        }
        let forwarded = headers
            .get("X-Forwarded-For")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').map(str::trim).find(|v| !v.is_empty()))
            .map(str::to_owned);
        match peer {
            Some(peer) if peer.ip().is_loopback() => forwarded.map(|ip| format!("ip:{ip}")).into(),
            Some(peer) => Self(Some(format!("ip:{}", peer.ip()))),
            None => Self(Some(format!(
                "ip:{}",
                forwarded.unwrap_or_else(|| "unknown".into())
            ))),
        }
    }
}
impl From<Option<String>> for ClientKey {
    fn from(value: Option<String>) -> Self {
        Self(value)
    }
}

struct Bucket {
    tokens: f64,
    updated: Instant,
}
pub struct Decision {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub reset_seconds: u64,
    pub retry_after: u64,
    pub scope: &'static str,
}
impl Decision {
    pub fn error(&self) -> ApiError {
        let mut error = ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            format!(
                "Too many {} requests: the limit is {} per {WINDOW_SECONDS} seconds for each client.",
                self.scope, self.limit
            ),
            format!(
                "Wait {} seconds (Retry-After), then retry. Mutations are safe to retry with the same Idempotency-Key.",
                self.retry_after
            ),
        );
        error.details = json!({"scope":self.scope,"limit":self.limit,"window_seconds":WINDOW_SECONDS,"retry_after_seconds":self.retry_after});
        error
    }
}

pub struct Limiter {
    pub reads_per_minute: u32,
    pub writes_per_minute: u32,
    pub max_streams: u32,
    buckets: Mutex<HashMap<(String, &'static str), Bucket>>,
    streams: OpenStreams,
}
impl Limiter {
    pub fn new(reads_per_minute: u32, writes_per_minute: u32, max_streams: u32) -> Self {
        Self {
            reads_per_minute,
            writes_per_minute,
            max_streams,
            buckets: Mutex::new(HashMap::new()),
            streams: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    /// Take one request from the client's bucket. `None` means not limited.
    pub fn check(&self, key: &ClientKey, write: bool) -> Option<Decision> {
        let key = key.0.as_ref()?;
        let (scope, limit) = if write {
            ("write", self.writes_per_minute)
        } else {
            ("read", self.reads_per_minute)
        };
        if limit == 0 {
            return None;
        }
        let rate = limit as f64 / WINDOW_SECONDS as f64;
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap();
        if buckets.len() > 50_000 {
            buckets.retain(|_, b| now.duration_since(b.updated).as_secs() < WINDOW_SECONDS);
        }
        let bucket = buckets.entry((key.clone(), scope)).or_insert(Bucket {
            tokens: limit as f64,
            updated: now,
        });
        bucket.tokens = (bucket.tokens + now.duration_since(bucket.updated).as_secs_f64() * rate)
            .min(limit as f64);
        bucket.updated = now;
        let allowed = bucket.tokens >= 1.0;
        if allowed {
            bucket.tokens -= 1.0;
        }
        Some(Decision {
            allowed,
            limit,
            remaining: bucket.tokens.floor().max(0.0) as u32,
            reset_seconds: ((limit as f64 - bucket.tokens) / rate).ceil().max(0.0) as u64,
            retry_after: if allowed {
                0
            } else {
                ((1.0 - bucket.tokens) / rate).ceil().max(1.0) as u64
            },
            scope,
        })
    }
    /// Count an open event stream for the client until the guard drops.
    pub fn open_stream(&self, key: &ClientKey) -> Result<StreamGuard, ApiError> {
        let Some(key) = key.0.clone() else {
            return Ok(StreamGuard(None));
        };
        if self.max_streams == 0 {
            return Ok(StreamGuard(None));
        }
        let mut streams = self.streams.lock().unwrap();
        let open = streams.entry(key.clone()).or_insert(0);
        if *open >= self.max_streams {
            let mut error = ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_streams",
                format!(
                    "This client already has {} open event streams.",
                    self.max_streams
                ),
                "Close a stream you no longer read, or read several feeds through one stream with ?types=.",
            );
            error.details = json!({"limit":self.max_streams,"retry_after_seconds":5});
            return Err(error);
        }
        *open += 1;
        Ok(StreamGuard(Some((self.streams.clone(), key))))
    }
}

type OpenStreams = Arc<Mutex<HashMap<String, u32>>>;
pub struct StreamGuard(Option<(OpenStreams, String)>);
impl Drop for StreamGuard {
    fn drop(&mut self) {
        if let Some((streams, key)) = self.0.take() {
            let mut streams = streams.lock().unwrap();
            if let Some(open) = streams.get_mut(&key) {
                *open = open.saturating_sub(1);
                if *open == 0 {
                    streams.remove(&key);
                }
            }
        }
    }
}
