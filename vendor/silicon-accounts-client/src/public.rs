//! Endpoints that need no account or app credentials: meta, id availability, keys,
//! Silicon login and self-create, CLI sign-in for Carbons, reports, telemetry.

use reqwest::Method;
use serde_json::json;

use crate::app::AppClient;
use crate::client::{AccountsClient, Auth, DEVELOPER_APP_ID, FIRST_PARTY_APP_ID, Request};
use crate::error::{Error, Result};
use crate::session::AccountSession;
use crate::types::{
    AppPublic, CliLoginChallenge, Contact, CustodianRequestStatus, DeviceAuthorization, DevicePoll,
    IdAvailability, Jwks, Meta, OidcDiscovery, ReportReceipt, SiliconSelfCreate,
    SiliconSelfCreated, TelemetryEvent, TokenResponse,
};

/// The OAuth grant type for exchanging a short-lived token.
pub const SLT_GRANT_TYPE: &str = "urn:silicon:params:oauth:grant-type:slt";
/// The OAuth grant type for device sign-in.
pub const DEVICE_CODE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// Refuses a client id that isn't one of Silicon Accounts' public clients before anything
/// is sent (apps have secrets: use [`crate::AppClient`]).
fn check_public_client(client_id: &str) -> Result<()> {
    if client_id == FIRST_PARTY_APP_ID || client_id == DEVELOPER_APP_ID {
        return Ok(());
    }
    Err(Error::invalid_input(
        format!(
            "'{client_id}' is not a public client: only Silicon Accounts' own '{FIRST_PARTY_APP_ID}' and '{DEVELOPER_APP_ID}' sign in without a secret."
        ),
        "Apps use their own credentials: AccountsClient::as_app(app_id, app_secret).refresh(…) / .revoke(…).",
    ))
}

impl AccountsClient {
    /// Acts as a signed-in Carbon or Silicon with a first-party access token
    /// (`aud = accounts`, from [`AccountsClient::silicon_login`], the device flow or
    /// [`AccountsClient::cli_login_verify`]). A developer platform token (`aud = developer`)
    /// works too, but only for reading the account and managing the apps it owns.
    pub fn with_token(&self, access_token: impl Into<String>) -> AccountSession<'_> {
        AccountSession::new(self, access_token.into())
    }

    /// Acts as an app with its own credentials (HTTP Basic `app_id:app_secret`).
    pub fn as_app(
        &self,
        app_id: impl Into<String>,
        app_secret: impl Into<String>,
    ) -> AppClient<'_> {
        AppClient::with_credentials(self, app_id.into(), app_secret.into())
    }

    /// `GET /v1/meta`: service name, version, environment and configured providers.
    pub async fn meta(&self) -> Result<Meta> {
        self.get(self.endpoint(&["v1", "meta"]), Auth::None).await
    }

    /// `GET /v1/ids/available`: whether a `c:` or `si:` id can be taken. Invalid ids are
    /// reported as unavailable with reason `invalid` and a precise message.
    pub async fn id_available(&self, id: &str) -> Result<IdAvailability> {
        let url = self.endpoint_with_query(
            &["v1", "ids", "available"],
            &[("id", Some(id.trim().to_owned()))],
        );
        self.get(url, Auth::None).await
    }

    /// `GET /v1/apps/{app_id}/public`: an app's public sign-in configuration.
    pub async fn app_public(&self, app_id: &str) -> Result<AppPublic> {
        self.get(self.endpoint(&["v1", "apps", app_id, "public"]), Auth::None)
            .await
    }

    /// `GET /.well-known/jwks.json`: the keys that sign access and id tokens. Cache it and
    /// fetch again when a token names an unknown `kid`.
    pub async fn jwks(&self) -> Result<Jwks> {
        self.get(self.endpoint(&[".well-known", "jwks.json"]), Auth::None)
            .await
    }

    /// `GET /.well-known/openid-configuration`.
    pub async fn oidc_discovery(&self) -> Result<OidcDiscovery> {
        self.get(
            self.endpoint(&[".well-known", "openid-configuration"]),
            Auth::None,
        )
        .await
    }

    /// `POST /v1/silicons/login`: a Silicon signs in with its si:id and STK and receives
    /// first-party tokens (`aud = accounts`).
    ///
    /// Errors: `invalid_credentials` (401, same for unknown id and wrong STK),
    /// `custodian_pending` / `custodian_declined` / `account_deleted` (403),
    /// `login_locked` (423 after 10 failures in a row, 1 minute).
    pub async fn silicon_login(
        &self,
        id: &str,
        stk: &str,
        client_label: Option<&str>,
    ) -> Result<TokenResponse> {
        let id = id.trim();
        let stk = stk.trim();
        if id.is_empty() || stk.is_empty() {
            return Err(Error::invalid_input(
                "A Silicon signs in with both its si:id and its STK, but one of them is empty.",
                "Pass the si:id (e.g. si:scout) and the STK (stk- followed by hex characters).",
            ));
        }
        let mut body = json!({ "id": id, "stk": stk });
        if let Some(label) = client_label {
            body["client_label"] = json!(label);
        }
        self.send_json(
            Method::POST,
            self.endpoint(&["v1", "silicons", "login"]),
            Auth::None,
            &body,
            None,
        )
        .await
    }

    /// `POST /v1/silicons`: a Silicon creates its own account and names its custodian.
    /// The account stays `pending_custodian` until the custodian accepts (14 days).
    /// Store `stk` and `webhook_secret` from the response now: they are shown once.
    /// Pass an idempotency key so a retry never creates a second request.
    pub async fn silicon_self_create(
        &self,
        request: &SiliconSelfCreate,
        idempotency_key: Option<&str>,
    ) -> Result<SiliconSelfCreated> {
        self.send_json(
            Method::POST,
            self.endpoint(&["v1", "silicons"]),
            Auth::None,
            request,
            idempotency_key,
        )
        .await
    }

    /// `GET /v1/silicons/requests/{id}`: the status of a self-created Silicon's custodian
    /// request, authenticated with the `sarq_…` request token.
    pub async fn silicon_request_status(
        &self,
        request_id: &str,
        request_token: &str,
    ) -> Result<CustodianRequestStatus> {
        self.get(
            self.endpoint(&["v1", "silicons", "requests", request_id]),
            Auth::Bearer(request_token),
        )
        .await
    }

    /// `POST /v1/device/authorize`: starts a Carbon CLI sign-in. Show `user_code` and
    /// `verification_uri`, then poll with [`AccountsClient::device_poll`] or
    /// [`AccountsClient::wait_for_device_tokens`].
    pub async fn device_authorize(
        &self,
        client_label: Option<&str>,
    ) -> Result<DeviceAuthorization> {
        let body = match client_label {
            Some(label) => json!({ "client_label": label }),
            None => json!({}),
        };
        self.send_json(
            Method::POST,
            self.endpoint(&["v1", "device", "authorize"]),
            Auth::None,
            &body,
            None,
        )
        .await
    }

    /// Polls the token endpoint once for a device sign-in.
    pub async fn device_poll(&self, device_code: &str) -> Result<DevicePoll> {
        let request = Request::new(
            Method::POST,
            self.endpoint(&["v1", "oauth", "token"]),
            Auth::None,
        )
        .form(&[
            ("grant_type", DEVICE_CODE_GRANT_TYPE),
            ("device_code", device_code),
            ("client_id", FIRST_PARTY_APP_ID),
        ]);
        match self.execute(request).await {
            Ok(response) => Ok(DevicePoll::Tokens(Box::new(response.json()?))),
            Err(Error::OAuth(err)) => match err.error.as_str() {
                "authorization_pending" => Ok(DevicePoll::Pending),
                "slow_down" => Ok(DevicePoll::SlowDown),
                "access_denied" => Ok(DevicePoll::Denied),
                "expired_token" => Ok(DevicePoll::Expired),
                _ => Err(Error::OAuth(err)),
            },
            Err(other) => Err(other),
        }
    }

    /// `POST /v1/cli/login/start`: sends a 6-digit code to an existing Carbon's email or
    /// phone. Unknown destinations fail with `account_not_found` (sign up on the site).
    pub async fn cli_login_start(&self, contact: &Contact) -> Result<CliLoginChallenge> {
        if contact.value().trim().is_empty() {
            return Err(Error::invalid_input(
                "The email address or phone number to send the code to is empty.",
                "Pass the email or phone number of your Carbon account.",
            ));
        }
        self.send_json(
            Method::POST,
            self.endpoint(&["v1", "cli", "login", "start"]),
            Auth::None,
            &contact.to_json(),
            None,
        )
        .await
    }

    /// `POST /v1/cli/login/verify`: finishes a code sign-in and returns first-party tokens.
    /// Errors: `invalid_code` (422, `details.remaining_attempts`), `code_expired` (410),
    /// `verification_locked` (423, `details.retry_after_seconds`).
    pub async fn cli_login_verify(
        &self,
        challenge_id: &str,
        code: &str,
        client_label: Option<&str>,
    ) -> Result<TokenResponse> {
        let code = code.trim();
        if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::invalid_input(
                format!("The verification code `{code}` is not 6 digits."),
                "Enter the 6-digit code from the email or SMS exactly as received.",
            ));
        }
        let mut body = json!({ "challenge_id": challenge_id, "code": code });
        if let Some(label) = client_label {
            body["client_label"] = json!(label);
        }
        self.send_json(
            Method::POST,
            self.endpoint(&["v1", "cli", "login", "verify"]),
            Auth::None,
            &body,
            None,
        )
        .await
    }

    /// Rotates a first-party refresh token (`client_id = accounts`, no secret). The old
    /// refresh token stops working; store the new one before using it.
    pub async fn refresh_first_party(&self, refresh_token: &str) -> Result<TokenResponse> {
        self.refresh_public_client(FIRST_PARTY_APP_ID, refresh_token)
            .await
    }

    /// Revokes a first-party token family (sign out). Always succeeds for unknown tokens
    /// (RFC 7009).
    pub async fn revoke_first_party(&self, token: &str) -> Result<()> {
        self.revoke_public_client(FIRST_PARTY_APP_ID, token).await
    }

    /// Rotates a refresh token of one of Silicon Accounts' public clients, which have no
    /// secret: [`FIRST_PARTY_APP_ID`] (`accounts`, the CLI) or [`DEVELOPER_APP_ID`]
    /// (`developer`, the developer platform). A public client only ever refreshes its own
    /// tokens. The old refresh token stops working; store the new one before using it.
    pub async fn refresh_public_client(
        &self,
        client_id: &str,
        refresh_token: &str,
    ) -> Result<TokenResponse> {
        check_public_client(client_id)?;
        let request = Request::new(
            Method::POST,
            self.endpoint(&["v1", "oauth", "token"]),
            Auth::None,
        )
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.trim()),
            ("client_id", client_id),
        ]);
        self.execute(request).await?.json()
    }

    /// Ends a sign-in of one of Silicon Accounts' public clients ([`FIRST_PARTY_APP_ID`] or
    /// [`DEVELOPER_APP_ID`]) with its refresh or access token. Always succeeds for unknown
    /// tokens (RFC 7009).
    pub async fn revoke_public_client(&self, client_id: &str, token: &str) -> Result<()> {
        check_public_client(client_id)?;
        let token = token.trim();
        let hint = if token.starts_with("sar_") {
            "refresh_token"
        } else {
            "access_token"
        };
        let request = Request::new(
            Method::POST,
            self.endpoint(&["v1", "oauth", "revoke"]),
            Auth::None,
        )
        .form(&[
            ("token", token),
            ("token_type_hint", hint),
            ("client_id", client_id),
        ]);
        self.execute(request).await.map(|_| ())
    }

    /// Exchanges an authorization code of the developer platform's public client
    /// ([`DEVELOPER_APP_ID`]): no secret, so PKCE with S256 is required — pass the
    /// `code_verifier` whose S256 challenge went to `/authorize`. The tokens it returns
    /// (`aud = developer`) read the signed-in Carbon (`GET /v1/me`, `GET /v1/session`) and
    /// manage the apps they own (`GET /v1/me/owned-apps`, `/v1/apps/{app_id}/…`, e.g. through
    /// [`crate::AccountSession::app`]); every other route answers 401 `token_wrong_audience`.
    pub async fn exchange_developer_code(
        &self,
        code: &str,
        redirect_uri: &str,
        code_verifier: &str,
    ) -> Result<TokenResponse> {
        if code_verifier.trim().is_empty() {
            return Err(Error::invalid_input(
                "The developer platform is a public client, so exchanging its code needs the PKCE code_verifier.",
                "Pass the verifier of the pkce_pair() whose challenge went to /authorize (code_challenge_method=S256).",
            ));
        }
        let request = Request::new(
            Method::POST,
            self.endpoint(&["v1", "oauth", "token"]),
            Auth::None,
        )
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code.trim()),
            ("redirect_uri", redirect_uri),
            ("code_verifier", code_verifier.trim()),
            ("client_id", DEVELOPER_APP_ID),
        ]);
        self.execute(request).await?.json()
    }

    /// `POST /v1/reports`: reports a bug to the Silicon Accounts maintainers, optionally
    /// with the link to a PR that fixes it. Signed-in reports carry the account (pass the
    /// access token); anonymous ones are allowed.
    pub async fn report(
        &self,
        message: &str,
        pr_url: Option<&str>,
        access_token: Option<&str>,
        idempotency_key: Option<&str>,
    ) -> Result<ReportReceipt> {
        let message = message.trim();
        if message.is_empty() || message.chars().count() > 10_000 {
            return Err(Error::invalid_input(
                format!(
                    "A report message must be 1 to 10000 characters; this one has {}.",
                    message.chars().count()
                ),
                "Describe what you did, what you expected and what happened instead.",
            ));
        }
        if let Some(pr) = pr_url
            && !pr.trim().starts_with("https://")
        {
            return Err(Error::invalid_input(
                format!("The PR link `{pr}` is not an https URL."),
                "Pass the full link, e.g. https://github.com/teamofsilicons/silicon-accounts/pull/42.",
            ));
        }
        let mut body = json!({ "message": message });
        if let Some(pr) = pr_url {
            body["pr_url"] = json!(pr.trim());
        }
        let auth = access_token.map_or(Auth::None, Auth::Bearer);
        self.send_json(
            Method::POST,
            self.endpoint(&["v1", "reports"]),
            auth,
            &body,
            idempotency_key,
        )
        .await
    }

    /// `POST /v1/telemetry/events`. Does nothing when telemetry is disabled.
    pub async fn send_telemetry(&self, events: &[TelemetryEvent]) -> Result<()> {
        if !self.telemetry_enabled() || events.is_empty() {
            return Ok(());
        }
        if events.len() > 50 {
            return Err(Error::invalid_input(
                format!(
                    "A telemetry batch holds at most 50 events; this one has {}.",
                    events.len()
                ),
                "Split the events into batches of 50.",
            ));
        }
        if let Some(bad) = events.iter().find(|e| !e.has_valid_name()) {
            return Err(Error::invalid_input(
                format!(
                    "The telemetry event name `{}` does not match ^[a-z0-9_.]{{1,64}}$.",
                    bad.name
                ),
                "Use lowercase letters, digits, `_` and `.` only.",
            ));
        }
        let body = json!({ "events": events });
        let request = Request::new(
            Method::POST,
            self.endpoint(&["v1", "telemetry", "events"]),
            Auth::None,
        )
        .json(&body)?
        .timeout(std::time::Duration::from_secs(3));
        self.execute(request).await.map(|_| ())
    }
}
