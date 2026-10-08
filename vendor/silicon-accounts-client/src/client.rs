//! The HTTP core: configuration, request building, retries and response decoding.

use std::fmt;
use std::time::Duration;

use bytes::Bytes;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

use crate::error::{ApiError, Error, OAuthError, Result};
use crate::serde_util::describe;
use crate::types::Page;

/// The production Silicon Accounts URL.
pub const DEFAULT_BASE_URL: &str = "https://accounts.teamofsilicons.com";
/// The app id of Silicon Accounts itself (the account site and the CLI). First-party
/// tokens (Carbon CLI sign-in, Silicon login) are issued with `aud = "accounts"`.
pub const FIRST_PARTY_APP_ID: &str = "accounts";

/// The developer platform's app id (developer.teamofsilicons.com): a public client (no
/// secret, PKCE S256 required). Its tokens (`aud = developer`) only read the signed-in Carbon
/// and manage the apps they own; anything else answers 401 `token_wrong_audience`.
pub const DEVELOPER_APP_ID: &str = "developer";
/// This crate's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Header sent on every request when telemetry is disabled.
pub const TELEMETRY_HEADER: &str = "X-Accounts-Telemetry";
/// Header carrying the idempotency key of a mutating request.
pub const IDEMPOTENCY_HEADER: &str = "Idempotency-Key";
/// Header set by the service when it replayed a stored response for an idempotency key.
pub const IDEMPOTENT_REPLAYED_HEADER: &str = "Idempotent-Replayed";
/// Header carrying the service's request id.
pub const REQUEST_ID_HEADER: &str = "X-Request-Id";

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_MAX_RETRIES: u32 = 2;

/// Builder for [`AccountsClient`].
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    base_url: String,
    timeout: Duration,
    connect_timeout: Duration,
    user_agent: Option<String>,
    telemetry: bool,
    allow_insecure_http: bool,
    max_retries: u32,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            timeout: DEFAULT_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            user_agent: None,
            telemetry: true,
            allow_insecure_http: false,
            max_retries: DEFAULT_MAX_RETRIES,
        }
    }
}

impl ClientBuilder {
    /// The Silicon Accounts URL (default `https://accounts.teamofsilicons.com`).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Total time allowed for one request (default 30 s).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Time allowed to establish a connection (default 10 s).
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// A product token prepended to the User-Agent, e.g. `my-app/1.2`.
    pub fn user_agent(mut self, product: impl Into<String>) -> Self {
        self.user_agent = Some(product.into());
        self
    }

    /// When false, every request carries `X-Accounts-Telemetry: off` and
    /// [`AccountsClient::send_telemetry`] sends nothing. Default true.
    pub fn telemetry(mut self, enabled: bool) -> Self {
        self.telemetry = enabled;
        self
    }

    /// Allow plain `http://` URLs for hosts other than loopback. Off by default because
    /// STKs and tokens would travel unencrypted.
    pub fn allow_insecure_http(mut self, allow: bool) -> Self {
        self.allow_insecure_http = allow;
        self
    }

    /// How many times a request that is safe to repeat is retried after a connection
    /// failure or a 502/503/504 (default 2). GET requests and requests carrying an
    /// idempotency key are safe to repeat; others are only retried when the connection
    /// could not be established at all.
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Builds the client.
    pub fn build(self) -> Result<AccountsClient> {
        let base = parse_base_url(&self.base_url, self.allow_insecure_http)?;
        let user_agent = match &self.user_agent {
            Some(product) => format!("{product} silicon-accounts-client/{VERSION}"),
            None => format!("silicon-accounts-client/{VERSION}"),
        };
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(self.connect_timeout)
            .build()
            .map_err(|e| {
                Error::invalid_input(
                    format!("Could not create the HTTP client: {}.", root_cause(&e)),
                    "This usually means the TLS backend failed to initialise; check the system certificate store.",
                )
            })?;
        Ok(AccountsClient {
            base,
            http,
            timeout: self.timeout,
            telemetry: self.telemetry,
            max_retries: self.max_retries,
        })
    }
}

fn parse_base_url(raw: &str, allow_insecure_http: bool) -> Result<Url> {
    let trimmed = raw.trim();
    let mut url = Url::parse(trimmed).map_err(|e| {
        Error::invalid_input(
            format!("The Silicon Accounts URL `{trimmed}` is not a valid absolute URL ({e})."),
            format!("Use a full URL such as {DEFAULT_BASE_URL} or http://127.0.0.1:8590."),
        )
    })?;
    match url.scheme() {
        "https" => {}
        "http" => {
            let loopback = match url.host() {
                Some(url::Host::Domain(d)) => d == "localhost" || d.ends_with(".localhost"),
                Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                None => false,
            };
            if !loopback && !allow_insecure_http {
                return Err(Error::invalid_input(
                    format!(
                        "The Silicon Accounts URL `{trimmed}` uses plain http for a host that is not this machine, so tokens and STKs would travel unencrypted."
                    ),
                    "Use https, or explicitly allow insecure http (ClientBuilder::allow_insecure_http, or ACCOUNTS_ALLOW_INSECURE_HTTP=1 for the CLI) on a trusted network.",
                ));
            }
        }
        other => {
            return Err(Error::invalid_input(
                format!(
                    "The Silicon Accounts URL `{trimmed}` uses the `{other}` scheme; only https (and http for this machine) are supported."
                ),
                format!("Use a URL such as {DEFAULT_BASE_URL}."),
            ));
        }
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(Error::invalid_input(
            format!("The Silicon Accounts URL `{trimmed}` has a query string or fragment."),
            "Pass only the origin (and an optional path prefix), e.g. https://accounts.teamofsilicons.com.",
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Error::invalid_input(
            format!("The Silicon Accounts URL `{trimmed}` contains credentials."),
            "Remove the user:password@ part; credentials are passed separately.",
        ));
    }
    // Keep a path prefix (e.g. a reverse proxy mount) but drop the trailing slash so
    // joining segments never produces `//`.
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&path);
    Ok(url)
}

/// The Silicon Accounts client. Stateless: it holds only configuration and a connection
/// pool, never tokens. Cheap to clone; share one per process.
///
/// ```no_run
/// # async fn demo() -> silicon_accounts_client::Result<()> {
/// use silicon_accounts_client::AccountsClient;
/// let client = AccountsClient::new("https://accounts.teamofsilicons.com")?;
/// let tokens = client.silicon_login("si:scout", "stk-0123456789ab", Some("scout on build box")).await?;
/// let me = client.with_token(tokens.access_token.expose()).me().await?;
/// println!("signed in as {} ({})", me.id, me.uuid);
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct AccountsClient {
    base: Url,
    http: reqwest::Client,
    timeout: Duration,
    telemetry: bool,
    max_retries: u32,
}

impl fmt::Debug for AccountsClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountsClient")
            .field("base_url", &self.base.as_str())
            .field("timeout", &self.timeout)
            .field("telemetry", &self.telemetry)
            .finish_non_exhaustive()
    }
}

impl AccountsClient {
    /// Creates a client for the given Silicon Accounts URL with default settings.
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        ClientBuilder::default().base_url(base_url).build()
    }

    /// Starts a [`ClientBuilder`].
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Returns a copy that uses `timeout` for every request.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Returns a copy with telemetry enabled or disabled (`X-Accounts-Telemetry: off`).
    pub fn with_telemetry(mut self, enabled: bool) -> Self {
        self.telemetry = enabled;
        self
    }

    /// The base URL requests go to.
    pub fn base_url(&self) -> &Url {
        &self.base
    }

    /// Whether telemetry is enabled.
    pub fn telemetry_enabled(&self) -> bool {
        self.telemetry
    }

    /// Builds `{base}/{segments…}` with every segment percent-encoded.
    pub(crate) fn endpoint(&self, segments: &[&str]) -> Url {
        let mut url = self.base.clone();
        if let Ok(mut path) = url.path_segments_mut() {
            path.pop_if_empty();
            path.extend(segments);
        }
        url
    }

    /// Builds an endpoint URL with query parameters (`None` values are skipped).
    pub(crate) fn endpoint_with_query(
        &self,
        segments: &[&str],
        query: &[(&str, Option<String>)],
    ) -> Url {
        let mut url = self.endpoint(segments);
        let present: Vec<(&str, &String)> = query
            .iter()
            .filter_map(|(k, v)| v.as_ref().map(|v| (*k, v)))
            .collect();
        if !present.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in present {
                pairs.append_pair(k, v);
            }
        }
        url
    }

    pub(crate) async fn execute(&self, request: Request<'_>) -> Result<Response> {
        let what = format!("{} {}", request.method, request.url.path());
        let repeatable = request.method == Method::GET || request.idempotency_key.is_some();
        let mut attempt: u32 = 0;
        loop {
            let builder = self.build_request(&request);
            match builder.send().await {
                Ok(response) => {
                    let status = response.status();
                    let transient = matches!(status.as_u16(), 502..=504);
                    if transient && repeatable && attempt < self.max_retries {
                        tokio::time::sleep(backoff(attempt)).await;
                        attempt += 1;
                        continue;
                    }
                    let headers = response.headers().clone();
                    let body = response
                        .bytes()
                        .await
                        .map_err(|e| self.transport_error(e, &what))?;
                    if status.is_success() {
                        return Ok(Response { status, body, what });
                    }
                    return Err(error_from_response(status, &headers, &body, &what));
                }
                Err(err) => {
                    let retry = attempt < self.max_retries
                        && (err.is_connect() || (repeatable && err.is_timeout()));
                    if retry {
                        tokio::time::sleep(backoff(attempt)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(self.transport_error(err, &what));
                }
            }
        }
    }

    fn build_request(&self, request: &Request<'_>) -> reqwest::RequestBuilder {
        let mut builder = self
            .http
            .request(request.method.clone(), request.url.clone())
            .timeout(request.timeout.unwrap_or(self.timeout))
            .header(header::ACCEPT, "application/json");
        builder = match &request.auth {
            Auth::None => builder,
            Auth::Bearer(token) => builder.bearer_auth(token),
            Auth::Basic { username, password } => builder.basic_auth(username, Some(password)),
        };
        if !self.telemetry {
            builder = builder.header(TELEMETRY_HEADER, "off");
        }
        if let Some(key) = request.idempotency_key {
            builder = builder.header(IDEMPOTENCY_HEADER, key);
        }
        match &request.body {
            Body::Empty => builder,
            Body::Json(bytes) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(bytes.clone()),
            Body::Form(text) => builder
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(text.clone()),
            Body::Raw {
                bytes,
                content_type,
            } => builder
                .header(header::CONTENT_TYPE, content_type.as_str())
                .body(bytes.clone()),
        }
    }

    fn transport_error(&self, err: reqwest::Error, what: &str) -> Error {
        let base = self.base.as_str().trim_end_matches('/');
        let cause = root_cause(&err);
        let (message, hint) = if err.is_timeout() {
            (
                format!("Silicon Accounts at {base} did not answer {what} within {}s.", self.timeout.as_secs()),
                "Check your network, or raise the timeout if the service is slow; the request may or may not have been applied, so retry with the same idempotency key where the command supports one.".to_owned(),
            )
        } else if cause.to_ascii_lowercase().contains("certificate") {
            (
                format!("Could not establish a trusted TLS connection to Silicon Accounts at {base} ({what}): {cause}."),
                "The server's certificate is not trusted on this machine; check the URL and the system certificate store.".to_owned(),
            )
        } else if err.is_connect() {
            (
                format!("Could not connect to Silicon Accounts at {base} ({what}): {cause}."),
                "Check the URL (--url, ACCOUNTS_URL or `accounts config set url`) and that the service is running and reachable from this machine.".to_owned(),
            )
        } else {
            (
                format!("The request {what} to Silicon Accounts at {base} failed: {cause}."),
                "Retry; if it keeps failing, check the URL and your network.".to_owned(),
            )
        };
        Error::Http {
            message,
            hint,
            source: err,
        }
    }

    // ---- small helpers used by the endpoint modules -------------------------------------

    pub(crate) async fn get<T: DeserializeOwned>(&self, url: Url, auth: Auth<'_>) -> Result<T> {
        self.execute(Request::new(Method::GET, url, auth))
            .await?
            .json()
    }

    pub(crate) async fn get_page<T: DeserializeOwned>(
        &self,
        url: Url,
        auth: Auth<'_>,
    ) -> Result<Page<T>> {
        self.execute(Request::new(Method::GET, url, auth))
            .await?
            .page()
    }

    pub(crate) async fn send_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: Method,
        url: Url,
        auth: Auth<'_>,
        body: &B,
        idempotency_key: Option<&str>,
    ) -> Result<T> {
        let request = Request::new(method, url, auth)
            .json(body)?
            .idempotency_key(idempotency_key)?;
        self.execute(request).await?.json()
    }
}

fn backoff(attempt: u32) -> Duration {
    let base_ms = 250u64.saturating_mul(1u64 << attempt.min(5));
    let jitter = rand::random::<u64>() % (base_ms / 2 + 1);
    Duration::from_millis(base_ms + jitter)
}

/// The innermost error message in a chain (e.g. `Connection refused (os error 61)`).
pub(crate) fn root_cause(err: &(dyn std::error::Error + 'static)) -> String {
    let mut current: &(dyn std::error::Error + 'static) = err;
    while let Some(next) = current.source() {
        current = next;
    }
    current.to_string()
}

/// How a request authenticates.
#[derive(Clone)]
pub(crate) enum Auth<'a> {
    None,
    Bearer(&'a str),
    Basic {
        username: &'a str,
        password: &'a str,
    },
}

pub(crate) enum Body {
    Empty,
    Json(Bytes),
    Form(String),
    Raw { bytes: Bytes, content_type: String },
}

pub(crate) struct Request<'a> {
    pub(crate) method: Method,
    pub(crate) url: Url,
    pub(crate) auth: Auth<'a>,
    pub(crate) body: Body,
    pub(crate) idempotency_key: Option<&'a str>,
    pub(crate) timeout: Option<Duration>,
}

impl<'a> Request<'a> {
    pub(crate) fn new(method: Method, url: Url, auth: Auth<'a>) -> Self {
        Self {
            method,
            url,
            auth,
            body: Body::Empty,
            idempotency_key: None,
            timeout: None,
        }
    }

    pub(crate) fn json<B: Serialize + ?Sized>(mut self, body: &B) -> Result<Self> {
        let bytes = serde_json::to_vec(body).map_err(|e| {
            Error::invalid_input(
                format!("The request body could not be encoded as JSON: {e}."),
                "Pass plain data (strings, numbers, lists, maps) in the request.",
            )
        })?;
        self.body = Body::Json(Bytes::from(bytes));
        Ok(self)
    }

    pub(crate) fn form(mut self, pairs: &[(&str, &str)]) -> Self {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in pairs {
            serializer.append_pair(k, v);
        }
        self.body = Body::Form(serializer.finish());
        self
    }

    pub(crate) fn raw(mut self, bytes: Bytes, content_type: impl Into<String>) -> Self {
        self.body = Body::Raw {
            bytes,
            content_type: content_type.into(),
        };
        self
    }

    pub(crate) fn idempotency_key(mut self, key: Option<&'a str>) -> Result<Self> {
        if let Some(key) = key {
            validate_idempotency_key(key)?;
        }
        self.idempotency_key = key;
        Ok(self)
    }

    pub(crate) fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Bytes in the request body.
    pub(crate) fn body_len(&self) -> usize {
        match &self.body {
            Body::Empty => 0,
            Body::Json(bytes) | Body::Raw { bytes, .. } => bytes.len(),
            Body::Form(text) => text.len(),
        }
    }
}

/// Idempotency keys are 1-200 printable ASCII characters.
pub(crate) fn validate_idempotency_key(key: &str) -> Result<()> {
    let ok = !key.is_empty() && key.len() <= 200 && key.bytes().all(|b| (0x21..=0x7e).contains(&b));
    if ok {
        Ok(())
    } else {
        Err(Error::invalid_input(
            format!(
                "The idempotency key `{key}` is invalid: it must be 1 to 200 printable ASCII characters without spaces."
            ),
            "Use a UUID or another unique string per logical operation, and reuse it only when retrying that same operation.",
        ))
    }
}

/// A successful response.
pub(crate) struct Response {
    pub(crate) status: StatusCode,
    pub(crate) body: Bytes,
    pub(crate) what: String,
}

impl Response {
    pub(crate) fn json<T: DeserializeOwned>(&self) -> Result<T> {
        if self.body.is_empty() {
            return Err(self.decode_error("the body was empty"));
        }
        serde_json::from_slice(&self.body).map_err(|e| self.decode_error(&e.to_string()))
    }

    pub(crate) fn value(&self) -> Result<Value> {
        if self.body.iter().all(u8::is_ascii_whitespace) {
            return Ok(Value::Null);
        }
        self.json()
    }

    pub(crate) fn json_from<T: DeserializeOwned>(&self, value: Value) -> Result<T> {
        serde_json::from_value(value).map_err(|e| self.decode_error(&e.to_string()))
    }

    /// Decodes `{"items":[…],"next_cursor":…}`, a bare list, or `{"<name>":[…]}`.
    pub(crate) fn page<T: DeserializeOwned>(&self) -> Result<Page<T>> {
        let value = self.json::<Value>()?;
        let (items, next_cursor) = match value {
            Value::Array(items) => (items, None),
            Value::Object(mut map) => {
                let next_cursor = map
                    .remove("next_cursor")
                    .and_then(|c| c.as_str().map(str::to_owned));
                match map.remove("items") {
                    Some(Value::Array(items)) => (items, next_cursor),
                    Some(other) => {
                        return Err(self.decode_error(&format!(
                            "`items` is {}, expected a list",
                            describe(&other)
                        )));
                    }
                    None => {
                        let mut lists = map.into_iter().filter(|(_, v)| v.is_array());
                        match (lists.next(), lists.next()) {
                            (Some((_, Value::Array(items))), None) => (items, next_cursor),
                            _ => {
                                return Err(
                                    self.decode_error("expected a list or an object with `items`")
                                );
                            }
                        }
                    }
                }
            }
            other => {
                return Err(
                    self.decode_error(&format!("expected a list, found {}", describe(&other)))
                );
            }
        };
        let mut decoded = Vec::with_capacity(items.len());
        for (index, item) in items.into_iter().enumerate() {
            let item = serde_json::from_value(item)
                .map_err(|e| self.decode_error(&format!("item {index}: {e}")))?;
            decoded.push(item);
        }
        Ok(Page {
            items: decoded,
            next_cursor,
        })
    }

    fn decode_error(&self, why: &str) -> Error {
        Error::decode(
            format!(
                "Silicon Accounts answered {} with HTTP {}, but the response is not what this client expects: {why}.",
                self.what,
                self.status.as_u16()
            ),
            "Make sure the URL points at Silicon Accounts (GET /v1/meta answers with its version) and that this client is up to date with the service.",
        )
    }
}

fn header_str<'h>(headers: &'h HeaderMap, name: &str) -> Option<&'h str> {
    headers
        .get(name)
        .and_then(|v: &HeaderValue| v.to_str().ok())
        .filter(|v| !v.is_empty())
}

/// Turns a non-2xx response into a typed error.
pub(crate) fn error_from_response(
    status: StatusCode,
    headers: &HeaderMap,
    body: &[u8],
    what: &str,
) -> Error {
    let header_request_id = header_str(headers, REQUEST_ID_HEADER).map(str::to_owned);
    let header_retry_after =
        header_str(headers, "retry-after").and_then(|v| v.trim().parse::<u64>().ok());
    let parsed: Option<Value> = serde_json::from_slice(body).ok();

    if let Some(Value::Object(map)) = &parsed {
        match map.get("error") {
            Some(Value::Object(err)) => {
                let text = |k: &str| err.get(k).and_then(Value::as_str).map(str::to_owned);
                let details = err.get("details").filter(|d| !d.is_null()).cloned();
                let detail_request_id = details
                    .as_ref()
                    .and_then(|d| d.get("request_id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let detail_retry = details
                    .as_ref()
                    .and_then(|d| d.get("retry_after_seconds"))
                    .and_then(Value::as_u64);
                let code = text("code").unwrap_or_else(|| format!("http_{}", status.as_u16()));
                let message = text("message")
                    .filter(|m| !m.trim().is_empty())
                    .unwrap_or_else(|| {
                        format!("{what} failed with HTTP {} ({code}).", status.as_u16())
                    });
                return ApiError {
                    status: status.as_u16(),
                    code,
                    message,
                    hint: text("hint").or_else(|| default_hint(status).map(str::to_owned)),
                    details,
                    request_id: header_request_id.or(detail_request_id),
                    retry_after: header_retry_after.or(detail_retry).map(Duration::from_secs),
                }
                .into();
            }
            Some(Value::String(code)) => {
                let description = map
                    .get("error_description")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                return OAuthError {
                    status: status.as_u16(),
                    error: code.clone(),
                    description,
                    request_id: header_request_id,
                }
                .into();
            }
            _ => {}
        }
    }

    // Not a Silicon Accounts error body: a proxy, a load balancer, or the wrong URL.
    let snippet = String::from_utf8_lossy(body);
    let snippet: String = snippet
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect();
    let reason = status.canonical_reason().unwrap_or("");
    let message = if snippet.is_empty() {
        format!(
            "{what} failed with HTTP {} {reason} and an empty body.",
            status.as_u16()
        )
    } else {
        format!(
            "{what} failed with HTTP {} {reason}, and the body is not a Silicon Accounts error: {snippet}",
            status.as_u16()
        )
    };
    ApiError {
        status: status.as_u16(),
        code: format!("http_{}", status.as_u16()),
        message,
        hint: Some(
            default_hint(status)
                .unwrap_or(
                    "Check that the URL points at Silicon Accounts (GET /v1/meta should answer).",
                )
                .to_owned(),
        ),
        details: None,
        request_id: header_request_id,
        retry_after: header_retry_after.map(Duration::from_secs),
    }
    .into()
}

fn default_hint(status: StatusCode) -> Option<&'static str> {
    Some(match status.as_u16() {
        401 => {
            "Sign in again (`accounts login`), or check the token or app credentials you passed."
        }
        403 => "This account or app is not allowed to do this.",
        404 => "Check the identifier; it may be mistyped, deleted, or not visible to you.",
        409 => "The resource changed or already exists; fetch the current state and retry.",
        410 => "It expired; start the step again.",
        413 => "The request body is too large.",
        423 | 429 => "Wait for the time in Retry-After, then retry.",
        500..=599 => {
            "This is a problem on the Silicon Accounts side; retry shortly and report it with the request id if it persists (`accounts report`)."
        }
        _ => return None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn base_url_rules() {
        assert!(parse_base_url("https://accounts.teamofsilicons.com/", false).is_ok());
        assert!(parse_base_url("http://127.0.0.1:8590", false).is_ok());
        assert!(parse_base_url("http://localhost:8590", false).is_ok());
        assert!(parse_base_url("http://[::1]:8590", false).is_ok());
        let err = parse_base_url("http://example.com", false).unwrap_err();
        assert!(err.message().contains("plain http"), "{err}");
        assert!(parse_base_url("http://example.com", true).is_ok());
        assert!(parse_base_url("ftp://example.com", false).is_err());
        assert!(parse_base_url("not a url", false).is_err());
        assert!(parse_base_url("https://example.com/?x=1", false).is_err());
        let prefixed = parse_base_url("https://example.com/accounts/", false).unwrap();
        assert_eq!(prefixed.path(), "/accounts");
    }

    #[test]
    fn endpoint_encodes_segments() {
        let client = AccountsClient::new("https://example.com/prefix").unwrap();
        let url = client.endpoint(&["v1", "me", "emails", "a+b@x.com"]);
        assert_eq!(
            url.as_str(),
            "https://example.com/prefix/v1/me/emails/a+b@x.com"
        );
        let url = client.endpoint(&["v1", "accounts", "by-id", "c:saket"]);
        assert_eq!(
            url.as_str(),
            "https://example.com/prefix/v1/accounts/by-id/c:saket"
        );
        let url = client.endpoint(&["v1", "x", "a/b?c#d"]);
        assert_eq!(
            url.as_str(),
            "https://example.com/prefix/v1/x/a%2Fb%3Fc%23d"
        );
        let url = client.endpoint_with_query(
            &["v1", "ids", "available"],
            &[("id", Some("c:a b".into())), ("x", None)],
        );
        assert_eq!(
            url.as_str(),
            "https://example.com/prefix/v1/ids/available?id=c%3Aa+b"
        );
    }

    #[test]
    fn idempotency_key_rules() {
        assert!(validate_idempotency_key("abc-123").is_ok());
        assert!(validate_idempotency_key("").is_err());
        assert!(validate_idempotency_key("has space").is_err());
        assert!(validate_idempotency_key(&"x".repeat(201)).is_err());
    }

    #[test]
    fn decodes_standard_error_body() {
        let mut headers = HeaderMap::new();
        headers.insert(REQUEST_ID_HEADER, HeaderValue::from_static("req-1"));
        let body = br#"{"error":{"code":"rate_limited","message":"Too many codes sent to s***@x.com.","hint":"Wait 10 minutes.","details":{"retry_after_seconds":42}}}"#;
        let err = error_from_response(
            StatusCode::TOO_MANY_REQUESTS,
            &headers,
            body,
            "POST /v1/flows/x/email",
        );
        assert_eq!(err.code(), "rate_limited");
        assert_eq!(err.status(), Some(429));
        assert_eq!(err.request_id(), Some("req-1"));
        assert_eq!(err.retry_after(), Some(Duration::from_secs(42)));
        assert_eq!(
            err.to_string(),
            "Too many codes sent to s***@x.com. Hint: Wait 10 minutes."
        );
    }

    #[test]
    fn decodes_oauth_and_foreign_bodies() {
        let headers = HeaderMap::new();
        let err = error_from_response(
            StatusCode::BAD_REQUEST,
            &headers,
            br#"{"error":"invalid_grant","error_description":"The refresh token was already used."}"#,
            "POST /v1/oauth/token",
        );
        assert_eq!(err.code(), "invalid_grant");
        assert!(err.is_unauthenticated());
        assert!(
            err.to_string()
                .starts_with("The refresh token was already used. Hint: ")
        );

        let err = error_from_response(
            StatusCode::BAD_GATEWAY,
            &headers,
            b"<html>Bad gateway</html>",
            "GET /v1/me",
        );
        assert_eq!(err.code(), "http_502");
        assert!(err.message().contains("<html>Bad gateway</html>"), "{err}");
        assert!(err.hint().unwrap().contains("Silicon Accounts side"));
    }
}
