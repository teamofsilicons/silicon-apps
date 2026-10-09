//! What a Silicon reads before it calls the API: the OpenAPI document, the
//! A2A agent card, the capabilities document, and `Apps-Version` negotiation.

use crate::{
    Shared,
    error::{ApiError, Result},
    events::{EVENT_TYPES, HEARTBEAT_SECONDS, PUBLIC_TYPES, STREAM_MAX_SECONDS},
    model::TARGETS,
    ratelimit::{ClientKey, WINDOW_SECONDS},
    subscriptions::{
        DELIVERY_TIMEOUT_SECONDS, MAX_SUBSCRIPTIONS, RETRY_SECONDS, RETRY_WINDOW_HOURS,
    },
};
use axum::{
    Json,
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, net::SocketAddr, time::Duration};

/// API versions this server speaks, newest first. Send one in `Apps-Version`.
pub const API_VERSIONS: &[&str] = &["2026-10-09"];
pub const API_VERSION: &str = API_VERSIONS[0];
pub const OPENAPI: &str = include_str!("../openapi.json");
pub const DOCS_URL: &str = "https://developers.teamofsilicons.com/docs/apps";
const AUTH_METHODS: &[&str] = &[
    "anonymous",
    "bearer",
    "slt_exchange",
    "refresh_token",
    "browser_session",
    "developer_token",
];

/// Choose the API version for a request. The header may list several
/// versions in order of preference; the first one this server speaks wins.
pub fn negotiate_version(header: Option<&str>) -> std::result::Result<&'static str, ApiError> {
    let Some(raw) = header.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(API_VERSION);
    };
    let requested: Vec<_> = raw
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect();
    if let Some(version) = requested
        .iter()
        .find_map(|v| API_VERSIONS.iter().find(|known| *known == v))
    {
        return Ok(version);
    }
    let mut error = ApiError::new(
        StatusCode::BAD_REQUEST,
        "unsupported_api_version",
        format!("Apps does not speak API version {raw}."),
        format!("Send Apps-Version: {API_VERSION}, or omit the header to use the current version."),
    );
    error.details = json!({"requested":requested,"supported":API_VERSIONS,"current":API_VERSION});
    Err(error)
}

fn finish(
    response: &mut Response,
    version: &'static str,
    decision: Option<&crate::ratelimit::Decision>,
) {
    let headers = response.headers_mut();
    headers.insert("Apps-Version", HeaderValue::from_static(version));
    let vary = headers
        .get("Vary")
        .and_then(|v| v.to_str().ok())
        .map(|v| format!("{v}, Apps-Version"))
        .unwrap_or_else(|| "Apps-Version".into());
    if let Ok(vary) = vary.parse() {
        headers.insert("Vary", vary);
    }
    if let Some(d) = decision {
        let pairs = [
            ("RateLimit-Limit", d.limit.to_string()),
            ("RateLimit-Remaining", d.remaining.to_string()),
            ("RateLimit-Reset", d.reset_seconds.to_string()),
            (
                "RateLimit-Policy",
                format!("{};w={WINDOW_SECONDS}", d.limit),
            ),
        ];
        for (name, value) in pairs {
            if let Ok(value) = value.parse() {
                headers.insert(name, value);
            }
        }
        if !d.allowed {
            headers.insert("Retry-After", d.retry_after.to_string().parse().unwrap());
        }
    }
}

/// Version negotiation and rate limits for every route.
pub async fn negotiate(State(s): State<Shared>, mut request: Request, next: Next) -> Response {
    let header = request
        .headers()
        .get("Apps-Version")
        .map(|v| v.to_str().unwrap_or("\u{fffd}").to_owned());
    let version = match negotiate_version(header.as_deref()) {
        Ok(version) => version,
        Err(error) => {
            let mut response = error.into_response();
            response
                .headers_mut()
                .insert("Cache-Control", HeaderValue::from_static("no-store"));
            finish(&mut response, API_VERSION, None);
            return response;
        }
    };
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0);
    let key = ClientKey::from_request(request.headers(), peer);
    let decision = if request.uri().path() == "/health" {
        None
    } else {
        let write = !matches!(request.method().as_str(), "GET" | "HEAD" | "OPTIONS");
        s.limiter.check(&key, write)
    };
    if let Some(d) = decision.as_ref().filter(|d| !d.allowed) {
        let mut response = d.error().into_response();
        response
            .headers_mut()
            .insert("Cache-Control", HeaderValue::from_static("no-store"));
        finish(&mut response, version, Some(d));
        return response;
    }
    request.extensions_mut().insert(key);
    let mut response = structured(next.run(request).await).await;
    finish(&mut response, version, decision.as_ref());
    response
}

/// Errors that do not come from our handlers, such as a body over the size
/// limit, still answer with the structured error shape.
async fn structured(response: Response) -> Response {
    let status = response.status();
    let json = response
        .headers()
        .get("Content-Type")
        .is_some_and(|v| v.as_bytes().starts_with(b"application/json"));
    if json || !(status.is_client_error() || status.is_server_error()) {
        return response;
    }
    let (parts, body) = response.into_parts();
    let reason = axum::body::to_bytes(body, 2048)
        .await
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
        .unwrap_or_default();
    let (code, message, hint) = match status.as_u16() {
        413 => (
            "payload_too_large",
            "The request body is larger than this route accepts.",
            "Send at most 2 MiB of JSON, 512 MiB for one package or 100 MiB for one media file.",
        ),
        415 => (
            "unsupported_media_type",
            "This route does not accept that Content-Type.",
            "Read /openapi.json for the body each route takes.",
        ),
        _ => (
            "http_error",
            status.canonical_reason().unwrap_or("The request failed."),
            "Read /openapi.json for the routes, bodies and parameters.",
        ),
    };
    let mut error = ApiError::new(status, code, message, hint);
    if !reason.is_empty() {
        error.details = json!({"reason":reason});
    }
    let mut rebuilt = error.into_response();
    for (name, value) in &parts.headers {
        if name != "content-type" && name != "content-length" {
            rebuilt.headers_mut().append(name.clone(), value.clone());
        }
    }
    rebuilt
}

/// Structured 404 for paths outside the route table.
pub async fn not_found() -> Response {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        "No route matches this path.",
        "Read /openapi.json for every route.",
    )
    .into_response()
}
/// Structured 405 for a known path with another method.
pub async fn method_not_allowed() -> Response {
    ApiError::new(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "This route does not accept that method.",
        "Read /openapi.json for the methods each route accepts.",
    )
    .into_response()
}

pub async fn openapi() -> Response {
    (
        [
            ("Content-Type", "application/json"),
            ("Cache-Control", "public, max-age=300"),
            ("Access-Control-Allow-Origin", "*"),
        ],
        OPENAPI,
    )
        .into_response()
}

pub async fn agent_card(State(s): State<Shared>) -> Response {
    let base = s.config.public_url.trim_end_matches('/').to_owned();
    let card = json!({
        "protocolVersion":"0.3.0",
        "name":"Silicon Apps",
        "description":"The app store and developer platform for Carbons and Silicons. Search the catalog, read an app's page, resolve and install a signed release for your platform, publish validated releases of your own apps, withdraw a bad release, and subscribe to release events by webhook or server-sent events. This service speaks REST (JSON over HTTPS, described at /openapi.json) and MCP (Streamable HTTP at /mcp). It does not take A2A JSON-RPC messages: call the REST routes in each skill, or the MCP tools.",
        "url":base,
        "preferredTransport":"HTTP+JSON",
        "version":env!("CARGO_PKG_VERSION"),
        "provider":{"organization":"Team of Silicons","url":"https://teamofsilicons.com"},
        "documentationUrl":DOCS_URL,
        "capabilities":{"streaming":true,"pushNotifications":true,"stateTransitionHistory":false},
        "securitySchemes":{
            "siliconAccounts":{"type":"http","scheme":"bearer","bearerFormat":"JWT","description":"A Silicon Accounts access token for the silicon-apps audience. Silicons sign in without a browser: get a single-use token with `silicon-accounts login --app silicon-apps`, then exchange it at POST /v1/auth/exchange. Public reads need no token."}
        },
        "security":[{"siliconAccounts":[]}],
        "defaultInputModes":["application/json"],
        "defaultOutputModes":["application/json","text/event-stream"],
        "skills":[
            {"id":"search-apps","name":"Search apps","description":"Find apps by ID, name, description or tags, with typo tolerance, tag, target and visibility filters, sorting and pagination. GET /v1/apps?q=&tags=&target=&sort=.","tags":["search","catalog","discovery"],"examples":["Find a terminal app that runs on macos-aarch64","GET /v1/apps?q=terminal&target=macos-aarch64"],"inputModes":["application/json"],"outputModes":["application/json"]},
            {"id":"get-app","name":"Get an app","description":"Read an app's page: authors, targets, latest production and development releases, links, media alt text, rating and installs. GET /v1/apps/{app_id}.","tags":["catalog","details"],"examples":["Show briefcase","GET /v1/apps/briefcase"],"inputModes":["application/json"],"outputModes":["application/json"]},
            {"id":"install-app","name":"Install an app","description":"Resolve the release for your target and download its package: GET /v1/apps/{app_id}/resolve?target=, then the download_path. Check the SHA-256 and the Ed25519 signature against /.well-known/silicon-apps-keys.json before you run anything. The silicon-apps CLI does all of this with `silicon-apps install APP` and keeps it updated.","tags":["install","packages"],"examples":["silicon-apps install briefcase","GET /v1/apps/briefcase/resolve?target=linux-x86_64"],"inputModes":["application/json"],"outputModes":["application/json","application/gzip"]},
            {"id":"publish-release","name":"Publish a release","description":"For an app's authors: upload a package per target for isolated validation of --help, accounts --json and login status --json, create a development release, promote it to production and publish the app. Watch validation step by step on the app's event stream. Withdraw a bad release with POST /v1/apps/{app_id}/releases/{release_id}/withdraw.","tags":["publish","releases","authoring"],"examples":["POST /v1/apps/ring/packages/macos-aarch64","POST /v1/apps/ring/releases/{release_id}/promote"],"inputModes":["application/json","application/gzip"],"outputModes":["application/json","text/event-stream"]},
            {"id":"subscribe-to-releases","name":"Subscribe to releases","description":"Get told when an app you can see ships a release: a webhook signed with a per-subscription whsec_ secret, or a stream at /v1/events/stream?subscription=ID. POST /v1/subscriptions.","tags":["subscriptions","webhooks","events","sse"],"examples":["Tell me when briefcase ships a production release","POST /v1/subscriptions {\"app_id\":\"briefcase\",\"types\":[\"release.promoted\"],\"delivery\":{\"mode\":\"webhook\",\"url\":\"https://example.com/hooks/apps\"}}"],"inputModes":["application/json"],"outputModes":["application/json","text/event-stream"]}
        ],
        "supportsAuthenticatedExtendedCard":false,
        "links":{
            "openapi":format!("{base}/openapi.json"),
            "capabilities":format!("{base}/v1/capabilities"),
            "llms_txt":format!("{base}/llms.txt"),
            "docs":DOCS_URL,
            "mcp":format!("{base}/mcp"),
            "signing_keys":format!("{base}{}",crate::signing::KEYS_PATH)
        },
        "interfaces":[
            {"protocol":"REST","transport":"HTTP+JSON","url":format!("{base}/v1"),"description":format!("{base}/openapi.json")},
            {"protocol":"MCP","transport":"Streamable HTTP","url":format!("{base}/mcp")}
        ]
    });
    (
        [
            ("Cache-Control", "public, max-age=300"),
            ("Access-Control-Allow-Origin", "*"),
        ],
        Json(card),
    )
        .into_response()
}

/// `/.well-known/silicon-apps-keys.json`: the public keys that sign releases.
pub async fn signing_keys(State(s): State<Shared>) -> Response {
    let document = {
        let store = s.store.lock().unwrap();
        s.signer.document(&store.connection, &s.config.public_url)
    };
    match document {
        Ok(document) => (
            [
                ("Cache-Control", "public, max-age=300"),
                ("Access-Control-Allow-Origin", "*"),
            ],
            Json(document),
        )
            .into_response(),
        Err(error) => error.into_response(),
    }
}

/// The isolated runner answers any HTTP request when it is up. Cached briefly.
pub async fn runner_reachable(s: &Shared) -> Option<(bool, String)> {
    let url = s.config.runner_url.as_ref()?;
    s.config.runner_token.as_ref()?;
    if let Some((at, ok, checked)) = s.runner_probe.lock().unwrap().clone()
        && at.elapsed() < Duration::from_secs(60)
    {
        return Some((ok, checked));
    }
    let ok = s
        .http
        .get(format!("{}/validate", url.trim_end_matches('/')))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .is_ok();
    let checked = crate::model::now();
    *s.runner_probe.lock().unwrap() = Some((std::time::Instant::now(), ok, checked.clone()));
    Some((ok, checked))
}

/// `GET /v1/capabilities`, answering `?require=` queries.
pub async fn capabilities(s: &Shared, q: &BTreeMap<String, String>) -> Result<Value> {
    let base = s.config.public_url.trim_end_matches('/').to_owned();
    let probe = runner_reachable(s).await;
    let targets: Vec<Value> = TARGETS
        .iter()
        .map(|target| {
            let configured = s.config.runner_url.is_some()
                && s.config.runner_token.is_some()
                && s.config.runner_targets.iter().any(|t| t == target);
            let live = configured && probe.as_ref().is_some_and(|(ok, _)| *ok);
            json!({"target":target,"validation":if live {"live"} else if configured {"unreachable"} else {"not_configured"},"live":live})
        })
        .collect();
    let mut document = json!({
        "service":"silicon-apps",
        "version":env!("CARGO_PKG_VERSION"),
        "api":{"current":API_VERSION,"versions":API_VERSIONS,"header":"Apps-Version","base_path":"/v1","negotiation":"Send Apps-Version with one or more versions in order of preference. The response names the version used. An unknown version returns 400 unsupported_api_version. Without the header you get the current version."},
        "auth":{
            "methods":AUTH_METHODS,
            "issuer":s.config.accounts_url.trim_end_matches('/'),
            "audience":crate::model::APP_ID,
            "token_exchange":"/v1/auth/exchange",
            "refresh":"/v1/auth/refresh",
            "browser_login":"/v1/auth/login",
            "note":"Send Authorization: Bearer <Silicon Accounts access token for silicon-apps>. Silicons get one without a browser: silicon-accounts login --app silicon-apps gives a single-use token for POST /v1/auth/exchange. Public reads need no token."
        },
        "targets":targets,
        "validation_runner":{
            "configured":probe.is_some(),
            "reachable":probe.as_ref().map(|(ok,_)|*ok),
            "checked_at":probe.as_ref().map(|(_,at)|at.clone()),
            "commands":crate::model::COMMANDS
        },
        "search":{"route":"/v1/apps","parameters":["q","tags","target","visibility","mine","sort","limit","offset"],"sort":crate::store::SORTS,"max_limit":100},
        "streaming":{
            "sse":true,
            "routes":["/v1/events/stream","/v1/apps/{app_id}/events/stream"],
            "resume":"Last-Event-ID header or ?last_event_id=",
            "filter":"?types=release.promoted,package.* (exact types, group.* or *)",
            "heartbeat_seconds":HEARTBEAT_SECONDS,
            "max_stream_seconds":STREAM_MAX_SECONDS,
            "event_types":EVENT_TYPES,
            "public_event_types":PUBLIC_TYPES
        },
        "subscriptions":{
            "route":"/v1/subscriptions",
            "deliveries":["webhook","stream"],
            "max_per_account":MAX_SUBSCRIPTIONS,
            "signature":"X-Apps-Signature: v1=<hex HMAC-SHA256(whsec secret, \"{X-Apps-Timestamp}.{raw body}\")>",
            "retry_seconds":RETRY_SECONDS,
            "retry_window_hours":RETRY_WINDOW_HOURS,
            "timeout_seconds":DELIVERY_TIMEOUT_SECONDS,
            "https_only":!s.config.local_development()
        },
        "signing":{
            "algorithm":crate::signing::ALGORITHM,
            "keys":format!("{base}{}",crate::signing::KEYS_PATH),
            "active_key_id":s.signer.active_id(),
            "signed":"Every package Apps serves. resolve returns the signature and the signed manifest: app_id, target, version, channel, sha256, size, release_id, install_script_sha256.",
            "author_keys":"/v1/keys",
            "author_signature_headers":["X-Apps-Author-Key-Id","X-Apps-Author-Signature"],
            "rotation":"Each key in the keys document carries endorsements from older keys. Trust a new key when a key you already trust endorses it."
        },
        "withdrawal":{
            "route":"/v1/apps/{app_id}/releases/{release_id}/withdraw",
            "effect":"A withdrawn release is never served. Installs resolve to the latest good release on its channel, and the updater moves installed copies off it on the next check.",
            "event":"release.withdrawn"
        },
        "idempotency":{"header":"Idempotency-Key","required_for":"every POST, PUT, PATCH and DELETE except /v1/auth/*","length":"8 to 200 printable characters","replay_header":"Idempotent-Replayed","secret_replay_minutes":10},
        "rate_limits":{
            "window_seconds":WINDOW_SECONDS,
            "reads_per_window":s.limiter.reads_per_minute,
            "writes_per_window":s.limiter.writes_per_minute,
            "open_streams":s.limiter.max_streams,
            "headers":["RateLimit-Limit","RateLimit-Remaining","RateLimit-Reset","RateLimit-Policy","Retry-After"],
            "error":"429 rate_limited with Retry-After"
        },
        "errors":{"shape":{"error":{"code":"string","message":"string","hint":"string","details":"any"}}},
        "links":{
            "openapi":format!("{base}/openapi.json"),
            "agent_card":format!("{base}/.well-known/agent.json"),
            "mcp":format!("{base}/mcp"),
            "llms_txt":format!("{base}/llms.txt"),
            "docs":DOCS_URL
        }
    });
    let raw = q.get("require").map(String::as_str).unwrap_or("").trim();
    if !raw.is_empty() {
        let names: Vec<_> = raw
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .collect();
        if names.len() > 50 {
            return Err(ApiError::bad("require lists at most 50 capabilities."));
        }
        let results: Vec<Value> = names
            .iter()
            .map(|name| {
                let (satisfied, reason) = requirement(&document, name);
                json!({"requirement":name,"satisfied":satisfied,"reason":reason})
            })
            .collect();
        let missing: Vec<Value> = results
            .iter()
            .filter(|r| r["satisfied"] != true)
            .cloned()
            .collect();
        if !missing.is_empty() {
            let names: Vec<&str> = missing
                .iter()
                .filter_map(|r| r["requirement"].as_str())
                .collect();
            let mut error = ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "capabilities_missing",
                format!("This server does not meet: {}.", names.join(", ")),
                "Each item in details.missing says why. Drop what you can live without, or try again when a worker is live.",
            );
            error.details = json!({"missing":missing,"results":results});
            return Err(error);
        }
        document["requirements"] = json!({"satisfied":true,"results":results});
    }
    Ok(document)
}

fn requirement(document: &Value, name: &str) -> (bool, String) {
    let (kind, value) = name.split_once(':').unwrap_or((name, ""));
    let yes = |reason: String| (true, reason);
    match (kind, value) {
        ("streaming" | "sse", "") => yes("Server-sent event streams with Last-Event-ID resume.".into()),
        ("subscriptions", "") => yes("POST /v1/subscriptions.".into()),
        ("webhooks", "") => yes("Signed webhook deliveries for subscriptions.".into()),
        ("idempotency", "") => yes("Idempotency-Key on every mutation.".into()),
        ("search", "") => yes("GET /v1/apps with q, tags, target, visibility, sort and pagination.".into()),
        ("rate_limits", "") => yes("429 with Retry-After and RateLimit headers.".into()),
        ("openapi", "") => yes("/openapi.json".into()),
        ("agent_card", "") => yes("/.well-known/agent.json".into()),
        ("signing" | "signed_releases", "") => yes(format!("Every served package is signed with Ed25519; keys at {}.", crate::signing::KEYS_PATH)),
        ("author_signatures", "") => yes("Authors register keys at /v1/keys and sign uploads.".into()),
        ("withdrawal", "") => yes("Authors withdraw a release with POST /v1/apps/{app_id}/releases/{release_id}/withdraw.".into()),
        ("mcp", "") => yes("Streamable HTTP MCP at /mcp.".into()),
        ("version", v) => {
            let ok = API_VERSIONS.contains(&v);
            (ok, if ok { format!("API version {v} is supported.") } else { format!("Supported versions: {}.", API_VERSIONS.join(", ")) })
        }
        ("auth", m) => {
            let ok = AUTH_METHODS.contains(&m);
            (ok, if ok { format!("{m} is accepted.") } else { format!("Accepted methods: {}.", AUTH_METHODS.join(", ")) })
        }
        ("delivery", m) => {
            let ok = ["webhook", "stream"].contains(&m);
            (ok, if ok { format!("{m} delivery is available.") } else { "Deliveries: webhook, stream.".into() })
        }
        ("event", t) => {
            let ok = EVENT_TYPES.contains(&t);
            (ok, if ok { format!("{t} is recorded.") } else { "Unknown event type; see streaming.event_types.".into() })
        }
        ("target", t) => match document["targets"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["target"] == t))
        {
            Some(item) if item["live"] == true => (true, format!("A validation worker for {t} is live.")),
            Some(item) => (false, format!("Uploads for {t} cannot be validated now: the worker is {}.", item["validation"].as_str().unwrap_or("unavailable").replace('_', " "))),
            None => (false, format!("{t} is not a package target.")),
        },
        _ => (false, "Unknown capability. Known: streaming, subscriptions, webhooks, idempotency, search, rate_limits, openapi, agent_card, signing, author_signatures, withdrawal, mcp, version:V, auth:METHOD, delivery:MODE, event:TYPE, target:TARGET.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    #[tokio::test]
    async fn plain_text_errors_become_structured() {
        let plain = Response::builder()
            .status(StatusCode::PAYLOAD_TOO_LARGE)
            .header("Content-Type", "text/plain; charset=utf-8")
            .header("Retry-After", "7")
            .body(Body::from("length limit exceeded"))
            .unwrap();
        let response = structured(plain).await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(response.headers()["retry-after"], "7");
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("application/json")
        );
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["error"]["code"], "payload_too_large");
        assert_eq!(body["error"]["details"]["reason"], "length limit exceeded");
        // Successful and JSON responses pass through untouched.
        let ok = Response::builder()
            .status(StatusCode::OK)
            .body(Body::from("fine"))
            .unwrap();
        assert_eq!(structured(ok).await.status(), StatusCode::OK);
    }
}
