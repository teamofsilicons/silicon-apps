//! Typed errors. Every error says exactly what went wrong and why (`message`) and what to
//! do next (`hint`), so a Carbon or a Silicon reading it can fix the problem without
//! guessing. `Display` prints the message followed by the hint.

use std::fmt;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

/// Result alias used throughout the crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong when talking to Silicon Accounts.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The service answered with its standard error body
    /// `{"error":{"code","message","hint","details"}}` (or a non-JSON error response).
    #[error("{0}")]
    Api(Box<ApiError>),
    /// An OAuth endpoint (`/v1/oauth/*`, device polling) answered with an RFC 6749 error.
    #[error("{0}")]
    OAuth(Box<OAuthError>),
    /// The request never got a response: connection refused, DNS, TLS, timeout…
    #[error("{message} Hint: {hint}")]
    Http {
        /// What failed and why.
        message: String,
        /// What to do next.
        hint: String,
        /// The underlying transport error.
        #[source]
        source: reqwest::Error,
    },
    /// The service answered, but the body was not what this client expected.
    #[error("{message} Hint: {hint}")]
    Decode {
        /// What failed and why.
        message: String,
        /// What to do next.
        hint: String,
    },
    /// A value passed to the client was rejected before any request was sent.
    #[error("{message} Hint: {hint}")]
    InvalidInput {
        /// What is wrong with the input.
        message: String,
        /// How to fix it.
        hint: String,
    },
    /// A request body is larger than the service accepts (an import over
    /// [`crate::MAX_IMPORT_BYTES`]), so it was never sent. Its code is
    /// `payload_too_large`, the code the service answers with when it refuses a body
    /// itself, so `error.is_code("payload_too_large")` covers both.
    #[error("{message} Hint: {hint}")]
    #[non_exhaustive]
    PayloadTooLarge {
        /// What is too large, how large it is, and the limit.
        message: String,
        /// How to make it fit.
        hint: String,
        /// `{"size_bytes": …, "limit_bytes": …}` (as [`Error::details`] returns them).
        details: Value,
    },
    /// Local verification of an access token failed (see [`crate::verify_access_token`]).
    #[error("{0}")]
    Token(TokenError),
    /// A waiting helper (custodian decision, import job, device sign-in) gave up.
    #[error("{message} Hint: {hint}")]
    TimedOut {
        /// What was still pending.
        message: String,
        /// How to resume.
        hint: String,
    },
}

/// The standard Silicon Accounts error body, plus transport metadata.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct ApiError {
    /// HTTP status code.
    pub status: u16,
    /// Stable machine-readable code, e.g. `id_taken`, `custodian_pending`, `rate_limited`.
    pub code: String,
    /// What went wrong and why.
    pub message: String,
    /// What to do next.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Structured details (`fields`, `retry_after_seconds`, `suggestions`…).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    /// The `X-Request-Id` of the failed request; quote it when reporting a bug.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// How long to wait before retrying (from `Retry-After` or `details.retry_after_seconds`).
    #[serde(skip)]
    pub retry_after: Option<Duration>,
}

impl ApiError {
    /// Creates an error value (mostly useful in tests and mocks).
    pub fn new(status: u16, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
            hint: None,
            details: None,
            request_id: None,
            retry_after: None,
        }
    }

    /// Adds a hint.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Field-level validation messages from `details.fields` (422 responses), if any.
    pub fn field_errors(&self) -> Vec<(String, String)> {
        self.details
            .as_ref()
            .and_then(|d| d.get("fields"))
            .and_then(Value::as_object)
            .map(|fields| {
                fields
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            v.as_str().map_or_else(|| v.to_string(), str::to_owned),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)?;
        if let Some(hint) = self.hint.as_deref().filter(|h| !h.is_empty()) {
            write!(f, " Hint: {hint}")?;
        }
        Ok(())
    }
}

/// An RFC 6749 error from the token, revoke or introspect endpoints.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct OAuthError {
    /// HTTP status code.
    pub status: u16,
    /// The RFC 6749 error code, e.g. `invalid_grant`, `authorization_pending`.
    pub error: String,
    /// The service's precise description of what was wrong.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The `X-Request-Id` of the failed request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl OAuthError {
    /// Creates an error value.
    pub fn new(status: u16, error: impl Into<String>, description: Option<String>) -> Self {
        Self {
            status,
            error: error.into(),
            description,
            request_id: None,
        }
    }

    /// What went wrong: the service's description, or a precise default for the code.
    pub fn message(&self) -> String {
        if let Some(description) = self.description.as_deref().filter(|d| !d.trim().is_empty()) {
            return description.to_owned();
        }
        let text = match self.error.as_str() {
            "invalid_request" => "The token request is missing a parameter or has an invalid one.",
            "invalid_client" => {
                "The app credentials were rejected: the app_id is unknown, the app is disabled, or the app secret is wrong."
            }
            "invalid_grant" => {
                "The code, refresh token, short-lived token or device code was rejected: it was already used, it expired, it was revoked, or it was issued to another app."
            }
            "unauthorized_client" => "This app is not allowed to use this grant type.",
            "unsupported_grant_type" => "The service does not support this grant type.",
            "invalid_scope" => {
                "One of the requested scopes is unknown or not allowed for this app."
            }
            "authorization_pending" => "The sign-in has not been approved yet.",
            "slow_down" => "The device sign-in is being polled too often.",
            "access_denied" => "The sign-in was denied.",
            "expired_token" => "The device code expired before the sign-in was approved.",
            _ => {
                return format!(
                    "The OAuth request failed with error `{}` (HTTP {}).",
                    self.error, self.status
                );
            }
        };
        text.to_owned()
    }

    /// What to do next, derived from the error code.
    pub fn hint(&self) -> &'static str {
        match self.error.as_str() {
            "invalid_request" => {
                "Check the request parameters against the token endpoint docs (`accounts docs apps`)."
            }
            "invalid_client" => {
                "Use the app's own app_id and app secret (from Silicon Apps); for the first-party CLI the client_id is `accounts` with no secret."
            }
            "invalid_grant" => {
                "Start a new sign-in. Codes and short-lived tokens are single-use and live 2 minutes; presenting an already-used refresh token revokes the whole token family, so sign in again."
            }
            "unauthorized_client" => "Use a grant type this app is configured for.",
            "unsupported_grant_type" => {
                "Use authorization_code, refresh_token, urn:silicon:params:oauth:grant-type:slt or urn:ietf:params:oauth:grant-type:device_code."
            }
            "invalid_scope" => {
                "Request only profile, email, phone, dob, timezone, openid or offline_access."
            }
            "authorization_pending" => {
                "Keep polling at the interval the device authorization returned."
            }
            "slow_down" => "Wait 5 seconds longer between polls.",
            "access_denied" => {
                "The Carbon declined on the account site; start again if that was a mistake."
            }
            "expired_token" => "Start a new sign-in; device codes are valid for 10 minutes.",
            _ => {
                "Check the request and try again; quote the request id if you report a bug (`accounts report`)."
            }
        }
    }
}

impl fmt::Display for OAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Hint: {}", self.message(), self.hint())
    }
}

/// Why a locally verified access token was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TokenError {
    /// The value is not a JWT (three base64url parts) or its JSON is invalid.
    Malformed(String),
    /// The token is signed with an algorithm other than EdDSA.
    UnsupportedAlgorithm(String),
    /// No key in the JWKS matches the token's `kid`.
    UnknownKey {
        /// The token's key id.
        kid: Option<String>,
    },
    /// The matching JWKS entry is not a usable Ed25519 key.
    InvalidKey(String),
    /// The signature does not match the token's content.
    BadSignature,
    /// `exp` is in the past.
    Expired {
        /// The token's `exp` (unix seconds).
        exp: Option<i64>,
    },
    /// `nbf` is in the future.
    NotYetValid,
    /// `aud` is not one of the expected audiences.
    WrongAudience {
        /// The audiences that were accepted.
        expected: Vec<String>,
    },
    /// `iss` is not the expected issuer.
    WrongIssuer {
        /// The issuer that was accepted.
        expected: String,
    },
    /// A required claim is missing or has the wrong type.
    MissingClaim(String),
}

impl TokenError {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Malformed(_) => "token_malformed",
            Self::UnsupportedAlgorithm(_) => "token_unsupported_algorithm",
            Self::UnknownKey { .. } => "token_unknown_key",
            Self::InvalidKey(_) => "token_invalid_key",
            Self::BadSignature => "token_bad_signature",
            Self::Expired { .. } => "token_expired",
            Self::NotYetValid => "token_not_yet_valid",
            Self::WrongAudience { .. } => "token_wrong_audience",
            Self::WrongIssuer { .. } => "token_wrong_issuer",
            Self::MissingClaim(_) => "token_missing_claim",
        }
    }

    /// What is wrong with the token.
    pub fn message(&self) -> String {
        match self {
            Self::Malformed(why) => format!("The access token is not a valid JWT: {why}."),
            Self::UnsupportedAlgorithm(alg) => format!(
                "The access token is signed with {alg}, but Silicon Accounts only signs access tokens with EdDSA (Ed25519)."
            ),
            Self::UnknownKey { kid: Some(kid) } => {
                format!("The access token was signed with key `{kid}`, which is not in the JWKS you passed.")
            }
            Self::UnknownKey { kid: None } => {
                "The access token has no `kid` header and the JWKS holds more than one key, so the signing key is ambiguous.".to_owned()
            }
            Self::InvalidKey(why) => format!("The JWKS entry for this token is not a usable Ed25519 key: {why}."),
            Self::BadSignature => "The access token's signature does not match its content: it was modified or signed by someone else.".to_owned(),
            Self::Expired { exp: Some(exp) } => format!("The access token expired (exp {exp}); access tokens live 30 minutes."),
            Self::Expired { exp: None } => "The access token expired; access tokens live 30 minutes.".to_owned(),
            Self::NotYetValid => "The access token is not valid yet (its nbf is in the future).".to_owned(),
            Self::WrongAudience { expected } => format!(
                "The access token was issued to a different app; expected audience {}.",
                expected.join(" or ")
            ),
            Self::WrongIssuer { expected } => format!("The access token was not issued by {expected}."),
            Self::MissingClaim(claim) => format!("The access token is missing the `{claim}` claim or it has the wrong type."),
        }
    }

    /// What to do next.
    pub fn hint(&self) -> &'static str {
        match self {
            Self::Malformed(_) | Self::MissingClaim(_) => {
                "Pass the access_token from the token response exactly as received (not the refresh token or an SLT)."
            }
            Self::UnsupportedAlgorithm(_) | Self::BadSignature => {
                "Reject the request; never trust a token whose signature doesn't verify."
            }
            Self::UnknownKey { .. } | Self::InvalidKey(_) => {
                "Fetch the JWKS again from /.well-known/jwks.json (keys rotate) and retry once; reject the token if it still fails."
            }
            Self::Expired { .. } => {
                "Refresh it with the refresh token (grant_type=refresh_token) and retry."
            }
            Self::NotYetValid => {
                "Check this machine's clock; the token becomes valid within seconds."
            }
            Self::WrongAudience { .. } => {
                "Only accept tokens issued to your own app_id; another app's token must go through a proof (OBO) instead."
            }
            Self::WrongIssuer { .. } => {
                "Make sure the token comes from the Silicon Accounts instance you trust."
            }
        }
    }
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Hint: {}", self.message(), self.hint())
    }
}

impl std::error::Error for TokenError {}

impl From<TokenError> for Error {
    fn from(value: TokenError) -> Self {
        Self::Token(value)
    }
}

impl Error {
    pub(crate) fn invalid_input(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
            hint: hint.into(),
        }
    }

    pub(crate) fn decode(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::Decode {
            message: message.into(),
            hint: hint.into(),
        }
    }

    /// Stable machine-readable code: the service's error code, the OAuth error, or a
    /// client-side code (`connection_failed`, `request_timeout`, `unexpected_response`,
    /// `invalid_input`, `payload_too_large`, `timed_out`, `token_*`).
    pub fn code(&self) -> &str {
        match self {
            Self::Api(e) => &e.code,
            Self::OAuth(e) => &e.error,
            Self::Http { source, .. } if source.is_timeout() => "request_timeout",
            Self::Http { .. } => "connection_failed",
            Self::Decode { .. } => "unexpected_response",
            Self::InvalidInput { .. } => "invalid_input",
            Self::PayloadTooLarge { .. } => "payload_too_large",
            Self::Token(e) => e.code(),
            Self::TimedOut { .. } => "timed_out",
        }
    }

    /// What went wrong and why (without the hint).
    pub fn message(&self) -> String {
        match self {
            Self::Api(e) => e.message.clone(),
            Self::OAuth(e) => e.message(),
            Self::Http { message, .. }
            | Self::Decode { message, .. }
            | Self::InvalidInput { message, .. }
            | Self::PayloadTooLarge { message, .. }
            | Self::TimedOut { message, .. } => message.clone(),
            Self::Token(e) => e.message(),
        }
    }

    /// What to do next, when known.
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::Api(e) => e.hint.clone().filter(|h| !h.is_empty()),
            Self::OAuth(e) => Some(e.hint().to_owned()),
            Self::Http { hint, .. }
            | Self::Decode { hint, .. }
            | Self::InvalidInput { hint, .. }
            | Self::PayloadTooLarge { hint, .. }
            | Self::TimedOut { hint, .. } => Some(hint.clone()),
            Self::Token(e) => Some(e.hint().to_owned()),
        }
    }

    /// HTTP status of the failed response, if there was one.
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Api(e) => Some(e.status),
            Self::OAuth(e) => Some(e.status),
            _ => None,
        }
    }

    /// The service's `X-Request-Id` for the failed request, if any.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Api(e) => e.request_id.as_deref(),
            Self::OAuth(e) => e.request_id.as_deref(),
            _ => None,
        }
    }

    /// Structured error details (`details` in the service's error body, or the size and
    /// the limit of a body refused before it was sent).
    pub fn details(&self) -> Option<&Value> {
        match self {
            Self::Api(e) => e.details.as_ref(),
            Self::PayloadTooLarge { details, .. } => Some(details),
            _ => None,
        }
    }

    /// How long the service asked to wait before retrying (429 / 423 responses).
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Api(e) => e.retry_after,
            _ => None,
        }
    }

    /// The service error body, if this is an API error.
    pub fn as_api(&self) -> Option<&ApiError> {
        match self {
            Self::Api(e) => Some(e),
            _ => None,
        }
    }

    /// The OAuth error, if this is one.
    pub fn as_oauth(&self) -> Option<&OAuthError> {
        match self {
            Self::OAuth(e) => Some(e),
            _ => None,
        }
    }

    /// True when the service answered with this error code.
    pub fn is_code(&self, code: &str) -> bool {
        self.code() == code
    }

    /// True for HTTP 404 responses.
    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }

    /// True when the credentials were missing, expired or rejected (HTTP 401 or an
    /// `invalid_grant` / `invalid_client` OAuth error).
    pub fn is_unauthenticated(&self) -> bool {
        match self {
            Self::Api(e) => e.status == 401,
            Self::OAuth(e) => {
                matches!(e.error.as_str(), "invalid_grant" | "invalid_client") || e.status == 401
            }
            _ => false,
        }
    }

    /// True when the request could not reach the service at all.
    pub fn is_transport(&self) -> bool {
        matches!(self, Self::Http { .. })
    }
}

impl From<ApiError> for Error {
    fn from(value: ApiError) -> Self {
        Self::Api(Box::new(value))
    }
}

impl From<OAuthError> for Error {
    fn from(value: OAuthError) -> Self {
        Self::OAuth(Box::new(value))
    }
}

/// Why a webhook delivery failed verification. Reject the delivery (answer 400/401) in
/// every case; Silicon Accounts retries failed deliveries with backoff.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WebhookError {
    /// The signing secret passed to the verifier is empty.
    #[error(
        "The webhook signing secret is empty. Hint: Pass the whsec_… secret you received when the webhook endpoint was set."
    )]
    EmptySecret,
    /// A required header was absent or empty.
    #[error(
        "The webhook request has no {0} header. Hint: Only accept requests that carry X-Accounts-Timestamp and X-Accounts-Signature; anything else did not come from Silicon Accounts."
    )]
    MissingHeader(&'static str),
    /// `X-Accounts-Timestamp` is not unix seconds.
    #[error(
        "X-Accounts-Timestamp is `{0}`, which is not a unix timestamp in seconds. Hint: Pass the header value exactly as received."
    )]
    InvalidTimestamp(String),
    /// The delivery is older (or further in the future) than the tolerance allows.
    #[error(
        "The webhook timestamp is {age_seconds}s away from this machine's clock, more than the {tolerance_seconds}s tolerance, so it may be a replay. Hint: Check this machine's clock; genuine retries are signed again with a fresh timestamp."
    )]
    TimestampOutOfTolerance {
        /// Absolute difference between the timestamp and now, in seconds.
        age_seconds: u64,
        /// The allowed difference.
        tolerance_seconds: u64,
    },
    /// `X-Accounts-Signature` does not contain a `v1=<hex>` signature.
    #[error(
        "X-Accounts-Signature is not in the form `v1=<hex>`. Hint: Pass the header value exactly as received."
    )]
    InvalidSignatureFormat,
    /// No signature matched.
    #[error(
        "The webhook signature does not match the body. Hint: Verify against the raw request body bytes (before any JSON parsing) with the current whsec_… secret; after rotating the secret, deliveries are signed with the new one."
    )]
    SignatureMismatch,
    /// The body verified but is not a webhook event.
    #[error(
        "The webhook body is not a valid event: {0}. Hint: Pass the raw request body exactly as received."
    )]
    InvalidBody(String),
}
