//! PKCE, state/nonce generation and the hosted sign-in URL.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};
use url::Url;

use crate::client::AccountsClient;

/// A PKCE verifier and its S256 challenge (RFC 7636). Send `challenge` to `/authorize`,
/// keep `verifier` server-side (or in the session) and send it with the code exchange.
#[derive(Clone, PartialEq, Eq)]
pub struct PkcePair {
    /// 43-character base64url secret; never put it in a URL.
    pub verifier: String,
    /// `base64url(sha256(verifier))`.
    pub challenge: String,
}

impl PkcePair {
    /// Always `S256`.
    pub fn method(&self) -> &'static str {
        "S256"
    }
}

impl std::fmt::Debug for PkcePair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PkcePair")
            .field("challenge", &self.challenge)
            .finish_non_exhaustive()
    }
}

/// Generates a PKCE pair (32 random bytes → 43-character verifier, S256 challenge).
pub fn pkce_pair() -> PkcePair {
    let verifier = random_token(32);
    let challenge = pkce_challenge(&verifier);
    PkcePair {
        verifier,
        challenge,
    }
}

/// The S256 challenge of a verifier.
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// `bytes` random bytes from the OS-seeded CSPRNG, base64url without padding.
pub fn random_token(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes.max(1)];
    rand::rng().fill_bytes(&mut buffer);
    URL_SAFE_NO_PAD.encode(buffer)
}

/// A fresh `state` value for `/authorize` (check it on the callback to stop CSRF).
pub fn random_state() -> String {
    random_token(24)
}

/// A fresh OIDC `nonce`.
pub fn random_nonce() -> String {
    random_token(24)
}

/// Parameters of the hosted sign-in page (`{base}/authorize`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthorizeParams {
    /// Your app id.
    pub app_id: String,
    /// Must exactly match one of the app's registered redirect URIs.
    pub redirect_uri: String,
    /// Opaque value returned to you on the redirect; always set it.
    pub state: Option<String>,
    /// PKCE challenge (recommended for every app, required for public clients).
    pub code_challenge: Option<String>,
    /// `S256` (default when a challenge is set) or `plain`.
    pub code_challenge_method: Option<String>,
    /// Scopes: `profile` is implied; add `email`, `phone`, `dob`, `timezone`, `openid`.
    pub scope: Vec<String>,
    /// OIDC nonce (echoed in the id token).
    pub nonce: Option<String>,
    /// `login`, `consent`, `select_account` or `none`.
    pub prompt: Option<String>,
    /// Jump straight to a method — a direct `Continue with …` button on your site: `google` /
    /// `apple` (the hosted "Opening Google…" page, then the provider), `email` / `phone` (the
    /// hosted page opens on that method's empty entry field). There is deliberately no way to
    /// pass a Carbon's email or phone: they always type it on the hosted pages.
    pub method: Option<String>,
    /// `signin` (default) or `signup`: whether the hosted pages show the sign-in or the
    /// sign-up version ("Sign in to Briefcase" / "Create your Briefcase account"). The
    /// account logic is the same either way (a first-time Carbon signs up).
    pub intent: Option<String>,
}

impl AuthorizeParams {
    /// Parameters for `app_id` returning to `redirect_uri`.
    pub fn new(app_id: impl Into<String>, redirect_uri: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            redirect_uri: redirect_uri.into(),
            ..Self::default()
        }
    }

    /// Sets `state`.
    pub fn state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Sets the PKCE challenge from a pair.
    pub fn pkce(mut self, pair: &PkcePair) -> Self {
        self.code_challenge = Some(pair.challenge.clone());
        self.code_challenge_method = Some(pair.method().to_owned());
        self
    }

    /// Adds scopes.
    pub fn scopes<I, S>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.scope.extend(scopes.into_iter().map(Into::into));
        self
    }

    /// Sets the OIDC nonce.
    pub fn nonce(mut self, nonce: impl Into<String>) -> Self {
        self.nonce = Some(nonce.into());
        self
    }

    /// Sets `prompt`.
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Sets `method` (a direct method button: `google`, `apple`, `email` or `phone`).
    pub fn method(mut self, method: impl Into<String>) -> Self {
        self.method = Some(method.into());
        self
    }

    /// Sets `intent`: `signin` or `signup` (your site's `Sign in` / `Sign up` buttons).
    pub fn intent(mut self, intent: impl Into<String>) -> Self {
        self.intent = Some(intent.into());
        self
    }
}

impl AccountsClient {
    /// The URL to send a browser to for signing into your app. After sign-in the browser
    /// returns to `redirect_uri?code=…&state=…`; exchange the code with
    /// [`crate::AppClient::exchange_code`].
    ///
    /// ```
    /// use silicon_accounts_client::{AccountsClient, AuthorizeParams, pkce_pair, random_state};
    /// let client = AccountsClient::new("https://accounts.teamofsilicons.com").unwrap();
    /// let pkce = pkce_pair();
    /// let url = client.authorize_url(
    ///     &AuthorizeParams::new("briefcase", "https://briefcase.example/callback")
    ///         .state(random_state())
    ///         .pkce(&pkce)
    ///         .scopes(["email"]),
    /// );
    /// assert!(url.as_str().starts_with("https://accounts.teamofsilicons.com/authorize?"));
    /// ```
    pub fn authorize_url(&self, params: &AuthorizeParams) -> Url {
        let mut url = self.endpoint(&["authorize"]);
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("response_type", "code");
            query.append_pair("app_id", &params.app_id);
            query.append_pair("redirect_uri", &params.redirect_uri);
            if let Some(state) = &params.state {
                query.append_pair("state", state);
            }
            if let Some(challenge) = &params.code_challenge {
                query.append_pair("code_challenge", challenge);
                query.append_pair(
                    "code_challenge_method",
                    params.code_challenge_method.as_deref().unwrap_or("S256"),
                );
            }
            if !params.scope.is_empty() {
                query.append_pair("scope", &params.scope.join(" "));
            }
            if let Some(nonce) = &params.nonce {
                query.append_pair("nonce", nonce);
            }
            if let Some(prompt) = &params.prompt {
                query.append_pair("prompt", prompt);
            }
            if let Some(method) = &params.method {
                query.append_pair("method", method);
            }
            if let Some(intent) = &params.intent {
                query.append_pair("intent", intent);
            }
        }
        url
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn s256_challenge_matches_a_known_vector() {
        // Independently computed: base64url(sha256("dBjftJeZ4CVP-mJ92K9xGb3iV8z7MOV6SA5mxS6uUHg")).
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mJ92K9xGb3iV8z7MOV6SA5mxS6uUHg"),
            "vLgV3FmJAD3ZJ1sQQtdNB0vZCQxRhhYgz_AmqjzpTi4"
        );
    }

    #[test]
    fn pairs_are_random_and_well_formed() {
        let a = pkce_pair();
        let b = pkce_pair();
        assert_ne!(a.verifier, b.verifier);
        assert_eq!(a.verifier.len(), 43);
        assert_eq!(a.challenge, pkce_challenge(&a.verifier));
        assert!(
            a.verifier
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        );
        assert!(!format!("{a:?}").contains(&a.verifier));
    }

    #[test]
    fn authorize_url_carries_every_parameter() {
        let client = AccountsClient::new("http://127.0.0.1:8590").unwrap();
        let pair = PkcePair {
            verifier: "v".into(),
            challenge: "c".into(),
        };
        let url = client.authorize_url(
            &AuthorizeParams::new("briefcase", "http://127.0.0.1:8593/briefcase/callback")
                .state("s1")
                .pkce(&pair)
                .scopes(["openid", "email"])
                .nonce("n1")
                .prompt("login")
                .method("google")
                .intent("signup"),
        );
        assert_eq!(url.path(), "/authorize");
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let get = |k: &str| {
            pairs
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("response_type"), Some("code"));
        assert_eq!(get("app_id"), Some("briefcase"));
        assert_eq!(
            get("redirect_uri"),
            Some("http://127.0.0.1:8593/briefcase/callback")
        );
        assert_eq!(get("state"), Some("s1"));
        assert_eq!(get("code_challenge"), Some("c"));
        assert_eq!(get("code_challenge_method"), Some("S256"));
        assert_eq!(get("scope"), Some("openid email"));
        assert_eq!(get("nonce"), Some("n1"));
        assert_eq!(get("prompt"), Some("login"));
        assert_eq!(
            get("login_hint"),
            None,
            "apps never pass a Carbon's email or phone"
        );
        assert_eq!(get("method"), Some("google"));
        assert_eq!(get("intent"), Some("signup"));
    }
}
