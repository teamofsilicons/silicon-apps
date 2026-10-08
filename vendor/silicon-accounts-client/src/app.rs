//! Everything an app does: token exchange, sign-in setup, user base, imports, webhooks,
//! proofs and lookups.

use std::fmt;
use std::time::Duration;

use reqwest::Method;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::client::{AccountsClient, Auth, Request, Response};
use crate::error::{Error, Result};
use crate::jwt::{Claims, VerifyOptions, verify_access_token};
use crate::public::SLT_GRANT_TYPE;
use crate::secret::Secret;
use crate::serde_util::unwrap_key;
use crate::types::{
    AccountKind, AccountSummary, AppDetails, AppProof, AppUser, AppWebhook, ConfigHistoryEntry,
    DeliveriesQuery, DeliveryDetail, ImportInput, ImportJob, ImportOptions, ImportRowResult,
    ImportRowsQuery, Introspection, IssueAppVerification, IssueUserVerification, IssuedProof, Jwks,
    Page, PageRequest, ProofRef, ProofVerification, ProofsQuery, ReplayRequest, ReplayResult,
    TokenResponse, UserInfo, UsersQuery, WebhookDelivery, WebhookSecret, WebhookTestResult,
};
use crate::wait::{WaitEvent, WaitOptions};

/// The largest import Silicon Accounts accepts: 50 MB (52,428,800 bytes) of request body,
/// which is the CSV file itself, or the JSON (`{"rows": […], "options": {…}}`) the rows are
/// sent as. One import also carries at most 100,000 rows. [`AppClient::start_import`]
/// refuses a larger body before sending it ([`Error::PayloadTooLarge`]).
pub const MAX_IMPORT_BYTES: usize = 50 * 1024 * 1024;

enum AppAuth {
    /// The app's own credentials (HTTP Basic).
    Credentials(Secret),
    /// A session of the Carbon who owns the app (Bearer).
    Owner(Secret),
}

/// An app talking to Silicon Accounts, either with its own credentials
/// ([`AccountsClient::as_app`]) or through its owner's session
/// ([`crate::AccountSession::app`]).
pub struct AppClient<'a> {
    client: &'a AccountsClient,
    app_id: String,
    auth: AppAuth,
}

impl fmt::Debug for AppClient<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mode = match self.auth {
            AppAuth::Credentials(_) => "app credentials",
            AppAuth::Owner(_) => "owner session",
        };
        f.debug_struct("AppClient")
            .field("app_id", &self.app_id)
            .field("auth", &mode)
            .finish_non_exhaustive()
    }
}

impl<'a> AppClient<'a> {
    pub(crate) fn with_credentials(
        client: &'a AccountsClient,
        app_id: String,
        secret: String,
    ) -> Self {
        Self {
            client,
            app_id,
            auth: AppAuth::Credentials(Secret::new(secret)),
        }
    }

    pub(crate) fn as_owner(client: &'a AccountsClient, app_id: String, token: String) -> Self {
        Self {
            client,
            app_id,
            auth: AppAuth::Owner(Secret::new(token)),
        }
    }

    /// The app id.
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// The underlying client (for public calls such as [`AccountsClient::jwks`]).
    pub fn client(&self) -> &'a AccountsClient {
        self.client
    }

    /// True when acting through the owner's session rather than the app's credentials.
    pub fn is_owner_session(&self) -> bool {
        matches!(self.auth, AppAuth::Owner(_))
    }

    fn auth(&self) -> Auth<'_> {
        match &self.auth {
            AppAuth::Credentials(secret) => Auth::Basic {
                username: &self.app_id,
                password: secret.expose(),
            },
            AppAuth::Owner(token) => Auth::Bearer(token.expose()),
        }
    }

    /// Basic auth with the app's credentials, or a precise error in owner mode.
    fn credentials(&self, what: &str) -> Result<Auth<'_>> {
        match &self.auth {
            AppAuth::Credentials(secret) => Ok(Auth::Basic {
                username: &self.app_id,
                password: secret.expose(),
            }),
            AppAuth::Owner(_) => Err(Error::invalid_input(
                format!(
                    "{what} needs app {}'s own credentials (app_id + app secret); the owner's session can't do it on the app's behalf.",
                    self.app_id
                ),
                "Pass the app secret (silicon-accounts: --app-secret, ACCOUNTS_APP_SECRET, or `silicon-accounts app use <app_id> --secret-stdin`). App secrets come from Silicon Apps.",
            )),
        }
    }

    fn app_url(&self, rest: &[&str]) -> url::Url {
        let mut segments = vec!["v1", "apps", self.app_id.as_str()];
        segments.extend_from_slice(rest);
        self.client.endpoint(&segments)
    }

    fn app_url_query(&self, rest: &[&str], query: &[(&str, Option<String>)]) -> url::Url {
        let mut segments = vec!["v1", "apps", self.app_id.as_str()];
        segments.extend_from_slice(rest);
        self.client.endpoint_with_query(&segments, query)
    }

    async fn send(
        &self,
        method: Method,
        url: url::Url,
        body: Option<&Value>,
        idempotency_key: Option<&str>,
    ) -> Result<Response> {
        let mut request =
            Request::new(method, url, self.auth()).idempotency_key(idempotency_key)?;
        if let Some(body) = body {
            request = request.json(body)?;
        }
        self.client.execute(request).await
    }

    async fn get<T: DeserializeOwned>(&self, url: url::Url) -> Result<T> {
        self.client.get(url, self.auth()).await
    }

    async fn token(&self, what: &str, form: &[(&str, &str)]) -> Result<TokenResponse> {
        let auth = self.credentials(what)?;
        let request = Request::new(
            Method::POST,
            self.client.endpoint(&["v1", "oauth", "token"]),
            auth,
        )
        .form(form);
        self.client.execute(request).await?.json()
    }

    // ---- OAuth ---------------------------------------------------------------------------

    /// Exchanges an authorization code from the redirect (`?code=…&state=…`). Pass the
    /// same `redirect_uri` you sent to `/authorize`, and the PKCE verifier if you sent a
    /// challenge. Codes are single-use and live 2 minutes.
    pub async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
        code_verifier: Option<&str>,
    ) -> Result<TokenResponse> {
        let mut form = vec![
            ("grant_type", "authorization_code"),
            ("code", code.trim()),
            ("redirect_uri", redirect_uri),
        ];
        if let Some(verifier) = code_verifier {
            form.push(("code_verifier", verifier));
        }
        self.token("Exchanging an authorization code", &form).await
    }

    /// Exchanges a Silicon's short-lived token (`slt_…`) for its tokens. This is how
    /// Silicons sign into apps: they never see your sign-in page.
    pub async fn exchange_slt(&self, slt: &str) -> Result<TokenResponse> {
        self.token(
            "Exchanging a short-lived token",
            &[("grant_type", SLT_GRANT_TYPE), ("slt", slt.trim())],
        )
        .await
    }

    /// Rotates a refresh token: store the new refresh token from the response; the old one
    /// is dead (presenting it again revokes the whole family).
    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenResponse> {
        self.token(
            "Refreshing tokens",
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.trim()),
            ],
        )
        .await
    }

    /// Revokes the token family of an access or refresh token issued to this app (the
    /// account gets signed out of the app).
    pub async fn revoke(&self, token: &str) -> Result<()> {
        let auth = self.credentials("Revoking a token")?;
        let request = Request::new(
            Method::POST,
            self.client.endpoint(&["v1", "oauth", "revoke"]),
            auth,
        )
        .form(&[("token", token.trim())]);
        self.client.execute(request).await.map(|_| ())
    }

    /// Asks the service whether a token of this app is active (RFC 7662).
    pub async fn introspect(&self, token: &str) -> Result<Introspection> {
        let auth = self.credentials("Introspecting a token")?;
        let request = Request::new(
            Method::POST,
            self.client.endpoint(&["v1", "oauth", "introspect"]),
            auth,
        )
        .form(&[("token", token.trim())]);
        self.client.execute(request).await?.json()
    }

    /// `GET /v1/userinfo` with an access token of this app.
    pub async fn userinfo(&self, access_token: &str) -> Result<UserInfo> {
        self.client
            .get(
                self.client.endpoint(&["v1", "userinfo"]),
                Auth::Bearer(access_token.trim()),
            )
            .await
    }

    /// Verifies an access token locally (EdDSA signature, expiry, `aud == app_id`) with a
    /// JWKS you fetched with [`AccountsClient::jwks`]. No network call. Revocation is not
    /// visible locally: use [`AppClient::introspect`] when that matters.
    pub fn verify_access_token_locally(&self, jwks: &Jwks, access_token: &str) -> Result<Claims> {
        verify_access_token(jwks, access_token, &VerifyOptions::for_app(&self.app_id))
    }

    // ---- the app and its sign-in setup ----------------------------------------------------

    /// `GET /v1/apps/{app_id}`.
    pub async fn app(&self) -> Result<AppDetails> {
        self.get(self.app_url(&[])).await
    }

    /// `PATCH /v1/apps/{app_id}/signin-config`: deep-merges `patch` into the sign-in setup
    /// (arrays replace). Pass `expected_version` (from [`AppDetails::config_version`]) to
    /// fail with `config_version_conflict` instead of overwriting someone else's change.
    /// Secrets go in `{"google":{"client_secret":"…"}}` / `{"apple":{"private_key":"…"}}`.
    pub async fn update_signin_config<P: Serialize + ?Sized>(
        &self,
        patch: &P,
        expected_version: Option<i64>,
        idempotency_key: Option<&str>,
    ) -> Result<AppDetails> {
        let mut body = serde_json::to_value(patch).map_err(|e| {
            Error::invalid_input(
                format!("The sign-in config patch is not valid JSON data: {e}."),
                "Pass a JSON object.",
            )
        })?;
        let Value::Object(map) = &mut body else {
            return Err(Error::invalid_input(
                "The sign-in config patch must be a JSON object, e.g. {\"methods\":{\"google\":true}}.",
                "Wrap the settings you want to change in an object.",
            ));
        };
        if let Some(version) = expected_version {
            map.insert("expected_version".to_owned(), json!(version));
        }
        let response = self
            .send(
                Method::PATCH,
                self.app_url(&["signin-config"]),
                Some(&body),
                idempotency_key,
            )
            .await?;
        response.json()
    }

    /// `GET /v1/apps/{app_id}/signin-config/history`.
    pub async fn signin_config_history(
        &self,
        page: &PageRequest,
    ) -> Result<Page<ConfigHistoryEntry>> {
        let url = self.app_url_query(
            &["signin-config", "history"],
            &[
                ("limit", page.limit.map(|l| l.to_string())),
                ("cursor", page.cursor.clone()),
            ],
        );
        self.client.get_page(url, self.auth()).await
    }

    // ---- user base ------------------------------------------------------------------------

    /// `GET /v1/apps/{app_id}/users`.
    pub async fn users(&self, query: &UsersQuery) -> Result<Page<AppUser>> {
        let url = self.app_url_query(
            &["users"],
            &[
                ("q", query.q.clone()),
                ("status", query.status.clone()),
                ("kind", query.kind.clone()),
                ("source", query.source.clone()),
                ("limit", query.limit.map(|l| l.to_string())),
                ("cursor", query.cursor.clone()),
            ],
        );
        self.client.get_page(url, self.auth()).await
    }

    /// `GET /v1/apps/{app_id}/users/{uuid}` (with the last 20 sign-ins).
    pub async fn user(&self, uuid: &str) -> Result<AppUser> {
        let response = self
            .send(
                Method::GET,
                self.app_url(&["users", uuid.trim()]),
                None,
                None,
            )
            .await?;
        response.json_from(unwrap_key(response.value()?, "user"))
    }

    // ---- imports --------------------------------------------------------------------------

    /// `POST /v1/apps/{app_id}/imports`: starts a background import of existing users. Each
    /// row is matched to the account that has its email or phone; otherwise an unclaimed
    /// Carbon is created (no email or SMS is sent). Pass an idempotency key so a retried
    /// upload never creates a second job.
    ///
    /// An import whose request body (the CSV itself, or the JSON the rows are sent as) is
    /// over [`MAX_IMPORT_BYTES`] is refused here with [`Error::PayloadTooLarge`] and nothing
    /// is sent: the service would refuse it anyway, often before the upload finishes.
    pub async fn start_import(
        &self,
        input: &ImportInput,
        options: &ImportOptions,
        idempotency_key: Option<&str>,
    ) -> Result<ImportJob> {
        let request = match input {
            ImportInput::Csv(bytes) => {
                let mut query = vec![
                    (
                        "ignore_unknown_columns",
                        Some(options.ignore_unknown_columns.to_string()),
                    ),
                    ("dry_run", Some(options.dry_run.to_string())),
                    ("update_existing", Some(options.update_existing.to_string())),
                ];
                if let Some(country) = &options.default_country {
                    query.push(("default_country", Some(country.clone())));
                }
                let url = self.app_url_query(&["imports"], &query);
                Request::new(Method::POST, url, self.auth())
                    .raw(bytes.clone(), "text/csv")
                    .idempotency_key(idempotency_key)?
                    .timeout(Duration::from_secs(300))
            }
            ImportInput::Rows(rows) => {
                let body = json!({ "rows": rows, "options": options });
                Request::new(Method::POST, self.app_url(&["imports"]), self.auth())
                    .json(&body)?
                    .idempotency_key(idempotency_key)?
                    .timeout(Duration::from_secs(300))
            }
            ImportInput::Json(rows) => {
                let body = json!({ "rows": rows, "options": options });
                Request::new(Method::POST, self.app_url(&["imports"]), self.auth())
                    .json(&body)?
                    .idempotency_key(idempotency_key)?
                    .timeout(Duration::from_secs(300))
            }
        };
        let size = request.body_len();
        if size > MAX_IMPORT_BYTES {
            return Err(import_too_large(input, size));
        }
        let response = self.client.execute(request).await?;
        response.json_from(unwrap_key(response.value()?, "job"))
    }

    /// `GET /v1/apps/{app_id}/imports`.
    pub async fn imports(&self, page: &PageRequest) -> Result<Page<ImportJob>> {
        let url = self.app_url_query(
            &["imports"],
            &[
                ("limit", page.limit.map(|l| l.to_string())),
                ("cursor", page.cursor.clone()),
            ],
        );
        self.client.get_page(url, self.auth()).await
    }

    /// `GET /v1/apps/{app_id}/imports/{job_id}`.
    pub async fn import_job(&self, job_id: &str) -> Result<ImportJob> {
        let response = self
            .send(
                Method::GET,
                self.app_url(&["imports", job_id.trim()]),
                None,
                None,
            )
            .await?;
        response.json_from(unwrap_key(response.value()?, "job"))
    }

    /// `GET /v1/apps/{app_id}/imports/{job_id}/rows`: per-row outcomes, filtered by outcome,
    /// by message level (`error`, `warning`, `info`) or by message code (`id_conflict`…).
    pub async fn import_rows(
        &self,
        job_id: &str,
        query: &ImportRowsQuery,
    ) -> Result<Page<ImportRowResult>> {
        let url = self.app_url_query(
            &["imports", job_id.trim(), "rows"],
            &[
                ("outcome", query.outcome.clone()),
                ("level", query.level.clone()),
                ("code", query.code.clone()),
                ("limit", query.limit.map(|l| l.to_string())),
                ("cursor", query.cursor.clone()),
            ],
        );
        self.client.get_page(url, self.auth()).await
    }

    /// Polls an import job every `poll` until it is `completed` or `failed`.
    pub async fn wait_for_import(&self, job_id: &str, poll: Duration) -> Result<ImportJob> {
        self.wait_for_import_with(job_id, &WaitOptions::fixed(poll), |_| {})
            .await
    }

    /// Polls an import job until it finishes, calling `on_event` after every poll (use it
    /// to show progress). Transient network errors are retried; a timeout ends with
    /// [`Error::TimedOut`] while the job keeps running on the service.
    pub async fn wait_for_import_with(
        &self,
        job_id: &str,
        options: &WaitOptions,
        mut on_event: impl FnMut(WaitEvent<'_, ImportJob>),
    ) -> Result<ImportJob> {
        let started = tokio::time::Instant::now();
        let mut interval = options.initial_interval;
        loop {
            match self.import_job(job_id).await {
                Ok(job) => {
                    on_event(WaitEvent::Polled(&job));
                    if job.is_finished() {
                        return Ok(job);
                    }
                }
                Err(error) if crate::wait::is_transient(&error) => {
                    on_event(WaitEvent::TransientError {
                        error: &error,
                        retry_in: interval,
                    });
                }
                Err(error) => return Err(error),
            }
            if let Some(timeout) = options.timeout
                && started.elapsed() + interval > timeout
            {
                return Err(Error::TimedOut {
                    message: format!(
                        "Import job {job_id} is still running after {}s.",
                        started.elapsed().as_secs()
                    ),
                    hint: format!(
                        "It keeps running on the service; check it with `silicon-accounts app import status {job_id}`."
                    ),
                });
            }
            tokio::time::sleep(interval).await;
            interval = options.next_interval(interval);
        }
    }

    // ---- webhooks -------------------------------------------------------------------------

    /// `PUT /v1/apps/{app_id}/webhook`: sets the endpoint; a new signing secret is
    /// generated every time and returned once. With an idempotency key, a retry within 10
    /// minutes returns the same secret instead of generating another.
    pub async fn set_webhook(
        &self,
        url: &str,
        idempotency_key: Option<&str>,
    ) -> Result<AppWebhook> {
        let body = json!({ "url": url.trim() });
        self.send(
            Method::PUT,
            self.app_url(&["webhook"]),
            Some(&body),
            idempotency_key,
        )
        .await?
        .json()
    }

    /// Gets the current webhook endpoint, secret-present marker and chosen update events.
    pub async fn webhook_settings(&self) -> Result<Value> {
        self.get(self.app_url(&["webhook"])).await
    }

    /// Configures the Accounts-owned webhook with Silicon Apps update choices.
    /// Returns the new signing secret once; repeating an idempotency key returns the same result.
    pub async fn set_webhook_events(
        &self,
        url: &str,
        events: &[String],
        key: Option<&str>,
    ) -> Result<AppWebhook> {
        self.send(
            Method::PUT,
            self.app_url(&["webhook"]),
            Some(&json!({"url":url,"events":events,"preserve_secret":true})),
            key,
        )
        .await?
        .json()
    }

    /// Generates or replaces the webhook signing secret before or after an endpoint is set.
    /// `set_webhook_events` preserves it when saving endpoint and event preferences.
    pub async fn generate_webhook_secret(&self, key: Option<&str>) -> Result<WebhookSecret> {
        self.send(
            Method::POST,
            self.app_url(&["webhook", "generate-secret"]),
            Some(&json!({})),
            key,
        )
        .await?
        .json()
    }

    /// `DELETE /v1/apps/{app_id}/webhook`.
    pub async fn remove_webhook(&self) -> Result<()> {
        self.send(Method::DELETE, self.app_url(&["webhook"]), None, None)
            .await
            .map(|_| ())
    }

    /// `POST /v1/apps/{app_id}/webhook/rotate-secret`: the old secret stops signing
    /// immediately. With an idempotency key, a retry within 10 minutes returns the same new
    /// secret instead of rotating again.
    pub async fn rotate_webhook_secret(
        &self,
        idempotency_key: Option<&str>,
    ) -> Result<WebhookSecret> {
        self.send(
            Method::POST,
            self.app_url(&["webhook", "rotate-secret"]),
            None,
            idempotency_key,
        )
        .await?
        .json()
    }

    /// `POST /v1/apps/{app_id}/webhook/test`: queues a `ping` event. With an idempotency key,
    /// a retry queues no second ping.
    pub async fn test_webhook(&self, idempotency_key: Option<&str>) -> Result<WebhookTestResult> {
        let response = self
            .send(
                Method::POST,
                self.app_url(&["webhook", "test"]),
                None,
                idempotency_key,
            )
            .await?;
        Ok(serde_json::from_value(response.value()?).unwrap_or_default())
    }

    /// `GET /v1/apps/{app_id}/webhook/deliveries`.
    pub async fn deliveries(&self, query: &DeliveriesQuery) -> Result<Page<WebhookDelivery>> {
        let url = self.app_url_query(
            &["webhook", "deliveries"],
            &[
                ("status", query.status.clone()),
                ("limit", query.limit.map(|l| l.to_string())),
                ("cursor", query.cursor.clone()),
            ],
        );
        self.client.get_page(url, self.auth()).await
    }

    /// `GET /v1/apps/{app_id}/webhook/deliveries/{id}` with attempts and payload.
    pub async fn delivery(&self, delivery_id: &str) -> Result<DeliveryDetail> {
        self.get(self.app_url(&["webhook", "deliveries", delivery_id.trim()]))
            .await
    }

    /// `POST /v1/apps/{app_id}/webhook/replay`: re-queues deliveries (max 100).
    pub async fn replay(
        &self,
        request: &ReplayRequest,
        idempotency_key: Option<&str>,
    ) -> Result<ReplayResult> {
        request.check()?;
        let body = request.to_json();
        let response = self
            .send(
                Method::POST,
                self.app_url(&["webhook", "replay"]),
                Some(&body),
                idempotency_key,
            )
            .await?;
        Ok(serde_json::from_value(response.value()?).unwrap_or_default())
    }

    // ---- proofs ---------------------------------------------------------------------------

    /// `POST /v1/proofs/user-verification`: get a proof that this app may act at `receiving_app` on
    /// behalf of the account behind `subject_token` (an access token issued to this app,
    /// after the account consented in your own UI).
    pub async fn issue_user_verification(
        &self,
        request: &IssueUserVerification,
        idempotency_key: Option<&str>,
    ) -> Result<IssuedProof> {
        let auth = self.credentials("Issuing a User verification proof")?;
        let request = Request::new(
            Method::POST,
            self.client.endpoint(&["v1", "proofs", "user-verification"]),
            auth,
        )
        .json(request)?
        .idempotency_key(idempotency_key)?;
        self.client.execute(request).await?.json()
    }

    /// Issues an app verification proof for exactly one app (`request.receiving_app`). With app
    /// credentials this calls `POST /v1/proofs/app-verification`; through the owner's session it calls
    /// `POST /v1/apps/{app_id}/proofs/app-verification` (the app's App verification page). To talk to several apps,
    /// issue one proof per app: each app verifies its own.
    pub async fn issue_app_verification(
        &self,
        request: &IssueAppVerification,
        idempotency_key: Option<&str>,
    ) -> Result<IssuedProof> {
        let receiving_app = request.receiving_app.trim();
        if receiving_app.is_empty() {
            return Err(Error::invalid_input(
                "An app verification proof needs the one app that will verify it (receiving_app).",
                "Pass that app's id, e.g. remind; to talk to several apps, issue one proof per app.",
            ));
        }
        if receiving_app.contains(',') || receiving_app.chars().any(char::is_whitespace) {
            return Err(Error::invalid_input(
                format!(
                    "An app verification proof is for exactly one app, but receiving_app '{receiving_app}' names several."
                ),
                "Issue one proof per app: call issue_app_verification once for every app that should verify a proof from you.",
            ));
        }
        let mut request = request.clone();
        request.receiving_app = receiving_app.to_owned();
        let url = if self.is_owner_session() {
            self.app_url(&["proofs", "app-verification"])
        } else {
            self.client.endpoint(&["v1", "proofs", "app-verification"])
        };
        let request = Request::new(Method::POST, url, self.auth())
            .json(&request)?
            .idempotency_key(idempotency_key)?;
        self.client.execute(request).await?.json()
    }

    /// `POST /v1/proofs/refresh`: a new proof token and a rotated refresh token (reusing
    /// an old refresh token revokes the proof).
    pub async fn refresh_proof(
        &self,
        proof_refresh_token: &str,
        access_ttl_seconds: Option<u32>,
    ) -> Result<IssuedProof> {
        let auth = self.credentials("Refreshing a proof")?;
        let mut body = json!({ "proof_refresh_token": proof_refresh_token.trim() });
        if let Some(ttl) = access_ttl_seconds {
            body["access_ttl_seconds"] = json!(ttl);
        }
        let request = Request::new(
            Method::POST,
            self.client.endpoint(&["v1", "proofs", "refresh"]),
            auth,
        )
        .json(&body)?;
        self.client.execute(request).await?.json()
    }

    /// `POST /v1/proofs/verify`: is this proof token valid for this app right now?
    /// Anything else (unknown, expired, revoked, for another app, account removed…) is
    /// [`ProofVerification::Invalid`] by design.
    pub async fn verify_proof(&self, proof_token: &str) -> Result<ProofVerification> {
        let auth = self.credentials("Verifying a proof")?;
        let body = json!({ "proof_token": proof_token.trim() });
        let request = Request::new(
            Method::POST,
            self.client.endpoint(&["v1", "proofs", "verify"]),
            auth,
        )
        .json(&body)?;
        let response = self.client.execute(request).await?;
        let value = response.value()?;
        if value.get("valid").and_then(Value::as_bool) == Some(true) {
            Ok(ProofVerification::Valid(Box::new(
                response.json_from(value)?,
            )))
        } else {
            Ok(ProofVerification::Invalid)
        }
    }

    /// Revokes a proof this app issued. Through the owner's session only
    /// [`ProofRef::Id`] works (`DELETE /v1/apps/{app_id}/proofs/{id}`).
    pub async fn revoke_proof(&self, proof: &ProofRef) -> Result<()> {
        if let (AppAuth::Owner(_), ProofRef::Id(id)) = (&self.auth, proof) {
            return self
                .send(
                    Method::DELETE,
                    self.app_url(&["proofs", id.trim()]),
                    None,
                    None,
                )
                .await
                .map(|_| ());
        }
        let auth = self.credentials("Revoking a proof by token")?;
        let request = Request::new(
            Method::POST,
            self.client.endpoint(&["v1", "proofs", "revoke"]),
            auth,
        )
        .json(&proof.to_json())?;
        self.client.execute(request).await.map(|_| ())
    }

    /// `GET /v1/apps/{app_id}/proofs`: proofs this app issued.
    pub async fn proofs(&self, query: &ProofsQuery) -> Result<Page<AppProof>> {
        let url = self.app_url_query(
            &["proofs"],
            &[
                ("kind", query.kind.clone()),
                ("status", query.status.clone()),
                ("limit", query.limit.map(|l| l.to_string())),
                ("cursor", query.cursor.clone()),
            ],
        );
        self.client.get_page(url, self.auth()).await
    }

    // ---- lookup -------------------------------------------------------------------------

    /// `GET /v1/accounts/{uuid}`: the current public identity of an account.
    pub async fn lookup(&self, uuid: &str) -> Result<AccountSummary> {
        self.get(self.client.endpoint(&["v1", "accounts", uuid.trim()]))
            .await
    }

    /// `GET /v1/accounts/by-id/{id}` (current ids only).
    pub async fn lookup_by_id(&self, id: &str) -> Result<AccountSummary> {
        self.get(self.client.endpoint(&[
            "v1",
            "accounts",
            "by-id",
            &id.trim().to_ascii_lowercase(),
        ]))
        .await
    }

    /// Resolves a uuid or a `c:`/`si:` id.
    pub async fn resolve(&self, uuid_or_id: &str) -> Result<AccountSummary> {
        if AccountKind::of_id(uuid_or_id).is_some() {
            self.lookup_by_id(uuid_or_id).await
        } else {
            self.lookup(uuid_or_id).await
        }
    }
}

/// The refusal of an import whose request body is `size` bytes, over [`MAX_IMPORT_BYTES`].
fn import_too_large(input: &ImportInput, size: usize) -> Error {
    let rows = |n: usize| format!("{n} row{}", if n == 1 { "" } else { "s" });
    let body = match input {
        ImportInput::Csv(_) => "its CSV".to_owned(),
        ImportInput::Rows(list) => format!("its JSON body ({})", rows(list.len())),
        ImportInput::Json(list) => format!("its JSON body ({})", rows(list.len())),
    };
    Error::PayloadTooLarge {
        message: format!(
            "The import is over the 50 MB limit: {body} is {size} bytes ({}), and one import accepts at most {MAX_IMPORT_BYTES} bytes, so it was not uploaded.",
            megabytes(size)
        ),
        hint: "Split it into files of at most 50 MB and 100,000 rows each, and import them one after another.".to_owned(),
        details: json!({ "size_bytes": size, "limit_bytes": MAX_IMPORT_BYTES }),
    }
}

/// `size` in MB (counted like the limit, 1 MB = 1,048,576 bytes) with one decimal, rounded
/// up so that a body over the limit never reads as "50.0 MB".
fn megabytes(size: usize) -> String {
    let tenths = (size as u64).saturating_mul(10).div_ceil(1024 * 1024);
    format!("{}.{} MB", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_in_megabytes_rounded_up() {
        assert_eq!(megabytes(MAX_IMPORT_BYTES), "50.0 MB");
        assert_eq!(megabytes(MAX_IMPORT_BYTES + 1), "50.1 MB");
        assert_eq!(megabytes(51 * 1024 * 1024), "51.0 MB");
        assert_eq!(megabytes(0), "0.0 MB");
    }

    #[test]
    fn an_oversized_import_says_what_why_and_how_to_fix_it() {
        let size = 51 * 1024 * 1024;
        let error = import_too_large(&ImportInput::Csv(bytes::Bytes::new()), size);
        assert_eq!(error.code(), "payload_too_large");
        assert_eq!(
            error.status(),
            None,
            "nothing was sent, so there is no HTTP status"
        );
        assert_eq!(
            error.message(),
            "The import is over the 50 MB limit: its CSV is 53477376 bytes (51.0 MB), and one import accepts at most 52428800 bytes, so it was not uploaded."
        );
        assert_eq!(
            error.hint().as_deref(),
            Some(
                "Split it into files of at most 50 MB and 100,000 rows each, and import them one after another."
            )
        );
        assert_eq!(
            error.details(),
            Some(&json!({ "size_bytes": 53_477_376, "limit_bytes": 52_428_800 }))
        );
        let one = import_too_large(&ImportInput::Json(vec![json!({})]), size);
        assert!(one.message().contains("its JSON body (1 row) is"), "{one}");
        let many = import_too_large(
            &ImportInput::Rows(vec![crate::types::ImportRow::default(); 2]),
            size,
        );
        assert!(
            many.message().contains("its JSON body (2 rows) is"),
            "{many}"
        );
    }
}
