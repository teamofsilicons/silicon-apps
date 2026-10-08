//! Local verification of Silicon Accounts access tokens (EdDSA / Ed25519 JWTs).

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, TokenError};
use crate::types::{AccountKind, Jwks};

/// The claims of a Silicon Accounts access token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Claims {
    /// Issuer: the Silicon Accounts public URL.
    #[serde(default)]
    pub iss: String,
    /// The account uuid.
    pub sub: String,
    /// The app id(s) the token was issued to.
    #[serde(deserialize_with = "one_or_many")]
    pub aud: Vec<String>,
    /// Expiry (unix seconds).
    pub exp: i64,
    /// Issued at (unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iat: Option<i64>,
    /// Not before (unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,
    /// Token id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
    /// Carbon or Silicon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<AccountKind>,
    /// The account's public id when the token was issued (it may have changed since).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Membership id (`{app_id}:{uuid}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mid: Option<String>,
    /// Token family id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fid: Option<String>,
    /// Space-separated scopes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

impl Claims {
    /// Scopes as a list.
    pub fn scopes(&self) -> Vec<&str> {
        self.scope
            .as_deref()
            .map(|s| s.split_whitespace().collect())
            .unwrap_or_default()
    }

    /// True when the token carries `scope`.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes().contains(&scope)
    }
}

fn one_or_many<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<String>, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::String(s) => Ok(vec![s]),
        Value::Array(items) => items
            .into_iter()
            .map(|v| match v {
                Value::String(s) => Ok(s),
                _ => Err(de::Error::custom("aud entries must be strings")),
            })
            .collect(),
        _ => Err(de::Error::custom(
            "aud must be a string or a list of strings",
        )),
    }
}

/// What to check when verifying an access token locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyOptions {
    /// Accepted audiences (your app id). Must not be empty.
    pub audiences: Vec<String>,
    /// Accepted issuer (the Silicon Accounts public URL), when you want it checked.
    pub issuer: Option<String>,
    /// Allowed clock skew in seconds (default 30).
    pub leeway_seconds: u64,
}

impl VerifyOptions {
    /// Accept tokens issued to `app_id`.
    pub fn for_app(app_id: &str) -> Self {
        Self {
            audiences: vec![app_id.to_owned()],
            issuer: None,
            leeway_seconds: 30,
        }
    }

    /// Also require this issuer.
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());
        self
    }
}

/// Verifies an access token with a JWKS (from [`crate::AccountsClient::jwks`]): EdDSA
/// signature by a key in the set, `exp`/`nbf`, `aud` and (optionally) `iss`.
///
/// This cannot see revocation (signed-out accounts, removed access); tokens live at most
/// 30 minutes. Call `introspect` when you must know about revocation immediately.
pub fn verify_access_token(jwks: &Jwks, token: &str, options: &VerifyOptions) -> Result<Claims> {
    let token = token.trim();
    let header = decode_header(token).map_err(|e| TokenError::Malformed(describe(e.kind())))?;
    if header.alg != Algorithm::EdDSA {
        return Err(TokenError::UnsupportedAlgorithm(format!("{:?}", header.alg)).into());
    }
    let jwk = match header.kid.as_deref() {
        Some(kid) => jwks.find(kid).ok_or_else(|| TokenError::UnknownKey {
            kid: Some(kid.to_owned()),
        })?,
        None if jwks.keys.len() == 1 => &jwks.keys[0],
        None => return Err(TokenError::UnknownKey { kid: None }.into()),
    };
    if jwk.kty != "OKP" || jwk.crv.as_deref() != Some("Ed25519") {
        return Err(TokenError::InvalidKey(format!(
            "it has kty `{}` and crv `{}`, expected OKP / Ed25519",
            jwk.kty,
            jwk.crv.as_deref().unwrap_or("none")
        ))
        .into());
    }
    let x = jwk
        .x
        .as_deref()
        .ok_or_else(|| TokenError::InvalidKey("it has no `x` value".to_owned()))?;
    // jsonwebtoken slices the key to 32 bytes without checking; check first so a short key
    // is an error, not a panic.
    let raw = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        x.trim_end_matches('='),
    )
    .map_err(|e| TokenError::InvalidKey(format!("`x` is not base64url ({e})")))?;
    if raw.len() != 32 {
        return Err(TokenError::InvalidKey(format!(
            "`x` is {} bytes, an Ed25519 public key is 32",
            raw.len()
        ))
        .into());
    }
    let key = DecodingKey::from_ed_components(x)
        .map_err(|e| TokenError::InvalidKey(describe(e.kind())))?;

    if options.audiences.is_empty() {
        return Err(crate::Error::invalid_input(
            "Refusing to verify an access token without an expected audience: any app's token would pass.",
            "Pass your app id as the audience (VerifyOptions::for_app).",
        ));
    }
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.leeway = options.leeway_seconds;
    validation.validate_nbf = true;
    validation.set_audience(&options.audiences);
    let mut required = vec!["exp", "sub", "aud"];
    if let Some(issuer) = &options.issuer {
        validation.set_issuer(&[issuer]);
        required.push("iss");
    }
    validation.set_required_spec_claims(&required);

    match decode::<Claims>(token, &key, &validation) {
        Ok(data) => Ok(data.claims),
        Err(e) => Err(match e.kind() {
            ErrorKind::InvalidSignature => TokenError::BadSignature,
            ErrorKind::ExpiredSignature => TokenError::Expired {
                exp: unverified_exp(token),
            },
            ErrorKind::ImmatureSignature => TokenError::NotYetValid,
            ErrorKind::InvalidAudience => TokenError::WrongAudience {
                expected: options.audiences.clone(),
            },
            ErrorKind::InvalidIssuer => TokenError::WrongIssuer {
                expected: options.issuer.clone().unwrap_or_default(),
            },
            ErrorKind::MissingRequiredClaim(claim) | ErrorKind::InvalidClaimFormat(claim) => {
                TokenError::MissingClaim(claim.clone())
            }
            ErrorKind::InvalidAlgorithm => {
                TokenError::UnsupportedAlgorithm("a non-EdDSA algorithm".to_owned())
            }
            ErrorKind::InvalidEddsaKey | ErrorKind::InvalidKeyFormat => {
                TokenError::InvalidKey(describe(e.kind()))
            }
            other => TokenError::Malformed(describe(other)),
        }
        .into()),
    }
}

/// Reads `exp` without verifying anything, only to make an expiry message precise.
fn unverified_exp(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let bytes =
        base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, payload).ok()?;
    serde_json::from_slice::<Value>(&bytes)
        .ok()?
        .get("exp")?
        .as_i64()
}

fn describe(kind: &ErrorKind) -> String {
    match kind {
        ErrorKind::InvalidToken => {
            "it does not have three base64url parts separated by dots".to_owned()
        }
        ErrorKind::Base64(e) => format!("a part is not valid base64url ({e})"),
        ErrorKind::Json(e) => format!("a part is not valid JSON ({e})"),
        ErrorKind::Utf8(e) => format!("a part is not valid UTF-8 ({e})"),
        ErrorKind::InvalidEddsaKey => "the key is not a 32-byte Ed25519 public key".to_owned(),
        ErrorKind::InvalidKeyFormat => "the key has an invalid format".to_owned(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use ed25519_dalek::SigningKey;
    use ed25519_dalek::pkcs8::EncodePrivateKey;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::json;

    use super::*;
    use crate::error::Error;
    use crate::types::Jwk;

    fn keypair(seed: u8) -> (EncodingKey, Jwk) {
        let signing = SigningKey::from_bytes(&[seed; 32]);
        let der = signing.to_pkcs8_der().unwrap();
        let encoding = EncodingKey::from_ed_der(der.as_bytes());
        let x = URL_SAFE_NO_PAD.encode(signing.verifying_key().as_bytes());
        (encoding, Jwk::ed25519(format!("k{seed}"), x))
    }

    fn token(key: &EncodingKey, kid: &str, claims: &Value) -> String {
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(kid.to_owned());
        encode(&header, claims, key).unwrap()
    }

    fn claims(aud: &str, exp_offset: i64) -> Value {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        json!({
            "iss": "http://127.0.0.1:8590", "sub": "a8K", "aud": aud, "exp": now + exp_offset, "iat": now,
            "nbf": now, "jti": "j1", "kind": "silicon", "id": "si:scout", "mid": "remind:a8K", "fid": "f1",
            "scope": "profile timezone"
        })
    }

    #[test]
    fn verifies_a_valid_token() {
        let (key, jwk) = keypair(7);
        let jwks = Jwks::new(vec![jwk]);
        let t = token(&key, "k7", &claims("remind", 600));
        let verified = verify_access_token(
            &jwks,
            &t,
            &VerifyOptions::for_app("remind").with_issuer("http://127.0.0.1:8590"),
        )
        .unwrap();
        assert_eq!(verified.sub, "a8K");
        assert_eq!(verified.aud, vec!["remind".to_owned()]);
        assert_eq!(verified.kind, Some(AccountKind::Silicon));
        assert!(verified.has_scope("timezone"));
    }

    #[test]
    fn rejects_wrong_audience_expiry_unknown_key_and_tampering() {
        let (key, jwk) = keypair(7);
        let (other_key, _) = keypair(9);
        let jwks = Jwks::new(vec![jwk]);
        let options = VerifyOptions::for_app("remind");

        let wrong_aud = token(&key, "k7", &claims("briefcase", 600));
        let err = verify_access_token(&jwks, &wrong_aud, &options).unwrap_err();
        assert!(
            matches!(err, Error::Token(TokenError::WrongAudience { .. })),
            "{err}"
        );

        let expired = token(&key, "k7", &claims("remind", -3600));
        let err = verify_access_token(&jwks, &expired, &options).unwrap_err();
        assert!(
            matches!(err, Error::Token(TokenError::Expired { exp: Some(_) })),
            "{err}"
        );

        let unknown = token(&other_key, "k9", &claims("remind", 600));
        let err = verify_access_token(&jwks, &unknown, &options).unwrap_err();
        assert!(
            matches!(err, Error::Token(TokenError::UnknownKey { .. })),
            "{err}"
        );

        let forged = token(&other_key, "k7", &claims("remind", 600));
        let err = verify_access_token(&jwks, &forged, &options).unwrap_err();
        assert!(
            matches!(err, Error::Token(TokenError::BadSignature)),
            "{err}"
        );

        let err = verify_access_token(&jwks, "not-a-jwt", &options).unwrap_err();
        assert!(
            matches!(err, Error::Token(TokenError::Malformed(_))),
            "{err}"
        );
        assert_eq!(err.code(), "token_malformed");

        let err = verify_access_token(
            &jwks,
            &wrong_aud,
            &VerifyOptions {
                audiences: vec![],
                issuer: None,
                leeway_seconds: 0,
            },
        )
        .unwrap_err();
        assert_eq!(err.code(), "invalid_input");
    }
}
