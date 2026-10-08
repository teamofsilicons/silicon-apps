//! Primary Silicon Apps interface. Clients read no environment or local files implicitly.
//! Persistent operations take an explicit [`LocalState`]; callers own credentials and configuration.
pub mod auth;
pub mod docs;
pub mod install;
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
}

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
        Ok(Self {
            http,
            base,
            token,
            telemetry: true,
        })
    }
    pub fn authenticated(&self, token: Option<String>) -> Self {
        Self {
            http: self.http.clone(),
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
            .header("Idempotency-Key", &operation_key)
            .body(bytes);
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
        ensure!(
            bytes.len() as u64 == resolution.package.size,
            "package size differs: expected {}, received {}",
            resolution.package.size,
            bytes.len()
        );
        ensure!(
            package::sha256(&bytes) == resolution.package.sha256.to_ascii_lowercase(),
            "SHA-256 checksum mismatch; downloaded package was not installed"
        );
        Ok(bytes)
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
        let code = e["code"].as_str().unwrap_or("api_error");
        let message = e["message"].as_str().unwrap_or("request failed");
        let hint = e["hint"]
            .as_str()
            .unwrap_or("Inspect the request arguments and try again.");
        let details = if e["details"].is_null() {
            String::new()
        } else {
            format!(
                "\nDetails: {}",
                serde_json::to_string_pretty(&e["details"])?
            )
        };
        bail!(
            "{code} (HTTP {}): {message}\nHint: {hint}{details}",
            status.as_u16()
        );
    }
    bail!(
        "Silicon Apps returned HTTP {} with unexpected body: {}",
        status.as_u16(),
        text.chars().take(500).collect::<String>()
    )
}
