//! Primary Silicon Apps interface. Clients read no environment or local files implicitly.
//! Persistent operations take an explicit [`LocalState`]; callers own credentials and configuration.
pub mod auth;
pub mod docs;
pub mod events;
pub mod install;
pub mod signing;
pub mod state;
pub mod updater;
pub use silicon_apps_package as package;
pub use state::{Config, LocalState};

use anyhow::{Context, Result, bail, ensure};
use reqwest::{Client as HttpClient, Method};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use url::Url;

pub const APP_ID: &str = "silicon-apps";
pub const DEFAULT_URL: &str = "https://apps.teamofsilicons.com";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct Client {
    http: HttpClient,
    /// For event streams: no total timeout, only a read timeout longer than
    /// the server's 15 second heartbeat.
    stream_http: HttpClient,
    base: Url,
    token: Option<String>,
    telemetry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub id: String,
    pub target: String,
    pub sha256: String,
    pub size: u64,
    pub command: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub id: String,
    pub app_id: String,
    pub channel: String,
    pub version: String,
    #[serde(default)]
    pub package_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    pub app_id: String,
    pub release: Release,
    pub package: Package,
    pub download_path: String,
    /// The API's signature over this package's release manifest.
    #[serde(default)]
    pub signature: Option<signing::ReleaseSignature>,
    /// The uploading author's own signature, when there is one.
    #[serde(default)]
    pub author_signature: Option<signing::AuthorSignature>,
    /// The install script this target runs, as the service recorded it.
    #[serde(default)]
    pub install_script: Option<Value>,
    /// Withdrawn releases on the same channel, newest first.
    #[serde(default)]
    pub withdrawn: Vec<WithdrawnRelease>,
}
/// A release its authors withdrew. It is never served.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawnRelease {
    pub release_id: String,
    pub version: String,
    pub channel: String,
    pub reason: String,
    pub withdrawn_at: String,
    #[serde(default)]
    pub withdrawn_by: String,
}

/// An error answer from the Apps API, with its stable code. The text form is
/// `code (HTTP status): message`, then the hint and details.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub hint: String,
    pub details: Value,
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (HTTP {}): {}\nHint: {}",
            self.code, self.status, self.message, self.hint
        )?;
        if !self.details.is_null() {
            write!(
                f,
                "\nDetails: {}",
                serde_json::to_string_pretty(&self.details).unwrap_or_default()
            )?;
        }
        Ok(())
    }
}
impl std::error::Error for ApiError {}

impl Client {
    pub fn new(base: &str, token: Option<String>) -> Result<Self> {
        let base = Url::parse(base).context("Apps server URL must be an absolute HTTP(S) URL")?;
        ensure!(
            base.scheme() == "https"
                || (base.scheme() == "http"
                    && matches!(
                        base.host_str(),
                        Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
                    )),
            "Apps server URL must use HTTPS, except localhost development"
        );
        ensure!(
            base.username().is_empty()
                && base.password().is_none()
                && base.query().is_none()
                && base.fragment().is_none(),
            "Apps server URL cannot contain credentials, query or fragment"
        );
        let http = HttpClient::builder()
            .user_agent(format!("silicon-apps/{VERSION}"))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let stream_http = HttpClient::builder()
            .user_agent(format!("silicon-apps/{VERSION}"))
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            http,
            stream_http,
            base,
            token,
            telemetry: true,
        })
    }
    pub fn authenticated(&self, token: Option<String>) -> Self {
        Self {
            http: self.http.clone(),
            stream_http: self.stream_http.clone(),
            base: self.base.clone(),
            token,
            telemetry: self.telemetry,
        }
    }
    /// Apply diagnostic opt-out to every Apps API request, including uploads and downloads.
    pub fn with_telemetry(mut self, enabled: bool) -> Self {
        self.telemetry = enabled;
        self
    }
    pub fn base_url(&self) -> &str {
        self.base.as_str()
    }
    fn url(&self, segments: &[&str]) -> Result<Url> {
        let mut url = self.base.clone();
        {
            let mut parts = url
                .path_segments_mut()
                .map_err(|_| anyhow::anyhow!("server URL has no path"))?;
            parts.pop_if_empty();
            parts.extend(segments);
        }
        Ok(url)
    }
    async fn response(
        &self,
        method: Method,
        path: &[&str],
        query: &[(&str, String)],
        body: Option<Value>,
        key: Option<&str>,
    ) -> Result<reqwest::Response> {
        let mut url = self.url(path)?;
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
        }
        let mut request = self.http.request(method.clone(), url).header(
            "X-Apps-Telemetry",
            if self.telemetry { "on" } else { "off" },
        );
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let operation_key = (method != Method::GET).then(|| {
            key.map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
        });
        if let Some(key) = &operation_key {
            request = request.header("Idempotency-Key", key);
        }
        let result = async {
            check_response(request.send().await.with_context(|| {
                format!(
                    "{method} /{}: could not reach Silicon Apps at {}",
                    path.join("/"),
                    self.base
                )
            })?)
            .await
        }
        .await;
        if let Some(key) = operation_key {
            result.with_context(||format!("Operation Idempotency-Key: {key}. To retry an identical request after an unknown outcome, pass --idempotency-key {key}."))
        } else {
            result
        }
    }
    /// Author/user endpoint escape hatch; CLI features all call this primary package interface.
    pub async fn request(
        &self,
        method: &str,
        path: &[&str],
        query: &[(&str, String)],
        body: Option<Value>,
        idempotency_key: Option<&str>,
    ) -> Result<Value> {
        let method = Method::from_bytes(method.as_bytes())?;
        let operation_key = (method != Method::GET).then(|| {
            idempotency_key
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
        });
        let response = self
            .response(method, path, query, body, operation_key.as_deref())
            .await?;
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(json!({}));
        }
        response.json().await.with_context(|| {
            match operation_key {
                Some(key) => format!("Silicon Apps returned malformed JSON after a mutation. Operation Idempotency-Key: {key}. Retry the identical request with --idempotency-key {key}."),
                None => "Silicon Apps returned malformed JSON".into(),
            }
        })
    }
    pub async fn me(&self) -> Result<Value> {
        self.request("GET", &["v1", "me"], &[], None, None).await
    }
    /// Record this signed-in account's platform for the author-visible target population.
    pub async fn register_platform(&self, target: &str) -> Result<Value> {
        self.request(
            "POST",
            &["v1", "platforms"],
            &[],
            Some(json!({"target":target})),
            None,
        )
        .await
    }
    pub async fn search(&self, query: &str, private: bool, mine: bool) -> Result<Value> {
        let mut params = vec![("q", query.to_owned()), ("mine", mine.to_string())];
        if private {
            params.push(("visibility", "private".into()));
        }
        self.request("GET", &["v1", "apps"], &params, None, None)
            .await
    }
    pub async fn app(&self, id: &str) -> Result<Value> {
        self.request("GET", &["v1", "apps", id], &[], None, None)
            .await
    }
    pub async fn available(&self, id: &str) -> Result<Value> {
        self.request("GET", &["v1", "apps", "availability", id], &[], None, None)
            .await
    }
    pub async fn create(
        &self,
        id: &str,
        name: &str,
        description: &str,
        logo: Option<&str>,
        key: Option<&str>,
    ) -> Result<Value> {
        self.request("POST", &["v1","apps"], &[], Some(json!({"app_id":id,"name":name,"description":description,"logo":logo.unwrap_or("")})), key).await
    }
    pub async fn edit(&self, id: &str, changes: Value, key: Option<&str>) -> Result<Value> {
        self.request("PATCH", &["v1", "apps", id], &[], Some(changes), key)
            .await
    }
    pub async fn action(
        &self,
        method: &str,
        app: &str,
        rest: &[&str],
        body: Option<Value>,
        key: Option<&str>,
    ) -> Result<Value> {
        let mut p = vec!["v1", "apps", app];
        p.extend_from_slice(rest);
        self.request(method, &p, &[], body, key).await
    }
    pub async fn resolve(&self, spec: &install::InstallSpec, target: &str) -> Result<Resolution> {
        let mut query = vec![
            ("channel", spec.channel.clone()),
            ("target", target.to_owned()),
        ];
        if let Some(v) = &spec.version {
            query.push(("version", v.clone()));
        }
        let value = self
            .request(
                "GET",
                &["v1", "apps", &spec.app_id, "resolve"],
                &query,
                None,
                None,
            )
            .await?;
        serde_json::from_value(value).context("invalid release resolution returned by Silicon Apps")
    }
    pub async fn upload(
        &self,
        app: &str,
        target: &str,
        bytes: Vec<u8>,
        key: Option<&str>,
    ) -> Result<Value> {
        self.upload_signed(app, target, bytes, None, key).await
    }
    /// Upload a package, signed with your author key when one is given.
    pub async fn upload_signed(
        &self,
        app: &str,
        target: &str,
        bytes: Vec<u8>,
        author: Option<&signing::AuthorKey>,
        key: Option<&str>,
    ) -> Result<Value> {
        ensure!(
            package::TARGETS.contains(&target),
            "unknown target `{target}`"
        );
        let manifest = package::inspect_archive(&bytes)?;
        ensure!(
            manifest.app_id == app,
            "package app_id `{}` does not match `{app}`",
            manifest.app_id
        );
        ensure!(
            manifest.targets.contains_key(target),
            "package does not contain target `{target}`"
        );
        let operation_key = key
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let mut request = self
            .http
            .post(self.url(&["v1", "apps", app, "packages", target])?)
            .header("Content-Type", "application/gzip")
            .header(
                "X-Apps-Telemetry",
                if self.telemetry { "on" } else { "off" },
            )
            .header("Idempotency-Key", &operation_key);
        if let Some(author) = author {
            request = request
                .header("X-Apps-Author-Key-Id", &author.key_id)
                .header(
                    "X-Apps-Author-Signature",
                    author.sign_package(app, target, &bytes)?,
                );
        }
        let mut request = request.body(bytes);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let result = async {
            check_response(request.send().await?)
                .await?
                .json()
                .await
                .context("invalid package upload response")
        }
        .await;
        result.with_context(||format!("Upload Idempotency-Key: {operation_key}. Retry identical bytes with --idempotency-key {operation_key} after an unknown outcome."))
    }
    pub async fn download(&self, resolution: &Resolution) -> Result<Vec<u8>> {
        let url = self.base.join(&resolution.download_path)?;
        ensure!(
            url.origin() == self.base.origin(),
            "server returned a download URL on another origin; refusing to expose credentials"
        );
        let mut req = self.http.get(url).header(
            "X-Apps-Telemetry",
            if self.telemetry { "on" } else { "off" },
        );
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        let mut response = check_response(req.send().await?).await?;
        ensure!(
            response.content_length().unwrap_or(0) <= package::MAX_ARCHIVE_BYTES,
            "download is too large"
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                (bytes.len() + chunk.len()) as u64 <= package::MAX_ARCHIVE_BYTES,
                "download exceeds package size limit"
            );
            bytes.extend_from_slice(&chunk);
        }
        let digest = package::sha256(&bytes);
        if bytes.len() as u64 != resolution.package.size
            || digest != resolution.package.sha256.to_ascii_lowercase()
        {
            return Err(anyhow::Error::new(signing::VerificationError {
                code: "checksum_mismatch",
                message: format!(
                    "SHA-256 checksum mismatch: expected {} ({} bytes), received {digest} ({} bytes); downloaded package was not installed",
                    resolution.package.sha256,
                    resolution.package.size,
                    bytes.len()
                ),
                hint: "Nothing was installed. Try again; if it repeats, report it with silicon-apps report.".into(),
                details: json!({"expected":{"sha256":resolution.package.sha256,"size":resolution.package.size},"received":{"sha256":digest,"size":bytes.len()}}),
            }));
        }
        Ok(bytes)
    }
    /// The public keys that sign this service's releases.
    pub async fn signing_keys(&self) -> Result<Value> {
        self.request(
            "GET",
            &[".well-known", "silicon-apps-keys.json"],
            &[],
            None,
            None,
        )
        .await
    }
    /// Withdraw a bad release. It stops being served; installs and the
    /// updater move to the latest good release on its channel.
    pub async fn withdraw_release(
        &self,
        app: &str,
        release_id: &str,
        reason: &str,
        key: Option<&str>,
    ) -> Result<Value> {
        self.action(
            "POST",
            app,
            &["releases", release_id, "withdraw"],
            Some(json!({"reason":reason})),
            key,
        )
        .await
    }
    /// Your registered author keys.
    pub async fn author_keys(&self) -> Result<Value> {
        self.request("GET", &["v1", "keys"], &[], None, None).await
    }
    /// Register an author public key (base64 Ed25519).
    pub async fn add_author_key(
        &self,
        public_key: &str,
        name: Option<&str>,
        key: Option<&str>,
    ) -> Result<Value> {
        let mut body = json!({"public_key":public_key});
        if let Some(name) = name {
            body["name"] = json!(name);
        }
        self.request("POST", &["v1", "keys"], &[], Some(body), key)
            .await
    }
    /// Revoke an author key so nothing new can be signed with it.
    pub async fn revoke_author_key(
        &self,
        key_id: &str,
        reason: Option<&str>,
        key: Option<&str>,
    ) -> Result<Value> {
        let body = reason
            .map(|r| json!({"reason":r}))
            .unwrap_or_else(|| json!({}));
        self.request("DELETE", &["v1", "keys", key_id], &[], Some(body), key)
            .await
    }
    /// What this server supports, answering `require` queries such as
    /// `streaming`, `subscriptions` or `target:linux-x86_64` in `requirements`.
    pub async fn capabilities(&self, require: &[&str]) -> Result<Value> {
        let query = if require.is_empty() {
            vec![]
        } else {
            vec![("require", require.join(","))]
        };
        self.request("GET", &["v1", "capabilities"], &query, None, None)
            .await
    }
    /// One page of a feed as JSON, after the event seq `after`.
    pub async fn events(
        &self,
        feed: &events::Feed,
        after: Option<i64>,
        types: &[&str],
        limit: Option<u32>,
    ) -> Result<Value> {
        let path = feed.path(false);
        let path: Vec<&str> = path.iter().map(String::as_str).collect();
        let mut query = feed.query();
        if let Some(after) = after {
            query.push(("after", after.to_string()));
        }
        if !types.is_empty() {
            query.push(("types", types.join(",")));
        }
        if let Some(limit) = limit {
            query.push(("limit", limit.to_string()));
        }
        self.request("GET", &path, &query, None, None).await
    }
    /// Open a server-sent event stream. Without `last_event_id` it starts at
    /// the newest event, or where a subscription's stream last stopped.
    pub async fn stream_events(
        &self,
        feed: &events::Feed,
        last_event_id: Option<&str>,
        types: &[&str],
    ) -> Result<events::EventStream> {
        let path = feed.path(true);
        let path: Vec<&str> = path.iter().map(String::as_str).collect();
        let mut url = self.url(&path)?;
        {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in feed.query() {
                pairs.append_pair(k, &v);
            }
            if !types.is_empty() {
                pairs.append_pair("types", &types.join(","));
            }
        }
        if url.query() == Some("") {
            url.set_query(None);
        }
        let mut request = self
            .stream_http
            .get(url)
            .header("Accept", "text/event-stream")
            .header(
                "X-Apps-Telemetry",
                if self.telemetry { "on" } else { "off" },
            );
        if let Some(id) = last_event_id {
            request = request.header("Last-Event-ID", id);
        }
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let response = check_response(request.send().await.with_context(|| {
            format!(
                "could not reach Silicon Apps at {} to stream events",
                self.base
            )
        })?)
        .await?;
        Ok(events::EventStream::new(response))
    }
    /// Your subscriptions. `status` is active, paused, cancelled or all;
    /// the default lists active and paused ones.
    pub async fn subscriptions(&self, status: Option<&str>) -> Result<Value> {
        let query = status
            .map(|s| vec![("status", s.to_owned())])
            .unwrap_or_default();
        self.request("GET", &["v1", "subscriptions"], &query, None, None)
            .await
    }
    pub async fn subscription(&self, id: &str) -> Result<Value> {
        self.request("GET", &["v1", "subscriptions", id], &[], None, None)
            .await
    }
    /// Subscribe. A webhook subscription's `secret` is in the result once.
    pub async fn create_subscription(
        &self,
        subscription: &events::NewSubscription,
        key: Option<&str>,
    ) -> Result<Value> {
        self.request(
            "POST",
            &["v1", "subscriptions"],
            &[],
            Some(subscription.body()),
            key,
        )
        .await
    }
    /// Change types, channels, delivery, description or status.
    pub async fn update_subscription(
        &self,
        id: &str,
        changes: Value,
        key: Option<&str>,
    ) -> Result<Value> {
        self.request(
            "PATCH",
            &["v1", "subscriptions", id],
            &[],
            Some(changes),
            key,
        )
        .await
    }
    pub async fn pause_subscription(&self, id: &str, key: Option<&str>) -> Result<Value> {
        self.update_subscription(id, json!({"status":"paused"}), key)
            .await
    }
    pub async fn resume_subscription(&self, id: &str, key: Option<&str>) -> Result<Value> {
        self.update_subscription(id, json!({"status":"active"}), key)
            .await
    }
    pub async fn cancel_subscription(&self, id: &str, key: Option<&str>) -> Result<Value> {
        self.request("DELETE", &["v1", "subscriptions", id], &[], None, key)
            .await
    }
    pub async fn subscription_deliveries(&self, id: &str, status: Option<&str>) -> Result<Value> {
        let query = status
            .map(|s| vec![("status", s.to_owned())])
            .unwrap_or_default();
        self.request(
            "GET",
            &["v1", "subscriptions", id, "deliveries"],
            &query,
            None,
            None,
        )
        .await
    }
    pub async fn rotate_subscription_secret(&self, id: &str, key: Option<&str>) -> Result<Value> {
        self.request(
            "POST",
            &["v1", "subscriptions", id, "secret", "rotate"],
            &[],
            None,
            key,
        )
        .await
    }
    /// Queue a signed `ping` delivery to a webhook subscription.
    pub async fn ping_subscription(&self, id: &str, key: Option<&str>) -> Result<Value> {
        self.request("POST", &["v1", "subscriptions", id, "ping"], &[], None, key)
            .await
    }
    pub async fn report(
        &self,
        message: &str,
        pr: Option<&str>,
        key: Option<&str>,
    ) -> Result<Value> {
        self.request(
            "POST",
            &["v1", "reports"],
            &[],
            Some(json!({"message":message,"pr":pr})),
            key,
        )
        .await
    }
}

async fn check_response(response: reqwest::Response) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if let Ok(value) = serde_json::from_str::<Value>(&text) {
        let e = &value["error"];
        return Err(anyhow::Error::new(ApiError {
            status: status.as_u16(),
            code: e["code"].as_str().unwrap_or("api_error").into(),
            message: e["message"].as_str().unwrap_or("request failed").into(),
            hint: e["hint"]
                .as_str()
                .unwrap_or("Inspect the request arguments and try again.")
                .into(),
            details: e["details"].clone(),
        }));
    }
    bail!(
        "Silicon Apps returned HTTP {} with unexpected body: {}",
        status.as_u16(),
        text.chars().take(500).collect::<String>()
    )
}
