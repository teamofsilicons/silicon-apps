//! A string that must never end up in logs by accident.

use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// A secret value returned by Silicon Accounts: an access or refresh token, an STK, a
/// short-lived token, a webhook signing secret, a proof token…
///
/// `Debug` never prints the value (only its public prefix such as `sar_`), so a stray
/// `{:?}` in a log line cannot leak it. Call [`Secret::expose`] when you really need the
/// value, for example to send it in a header or to show an STK exactly once.
/// The memory is zeroed when the value is dropped.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    /// Wraps a secret value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the secret value. Only call this where the value must leave the program.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Consumes the wrapper and returns the secret value.
    pub fn into_inner(mut self) -> String {
        std::mem::take(&mut self.0)
    }

    /// The public, non-secret prefix of the value (`sar_`, `stk-`, `whsec_`…), if it has one.
    pub fn prefix(&self) -> Option<&str> {
        let end = self.0.find(['_', '-'])?;
        // Prefixes are short; anything longer is probably part of the secret itself.
        (end <= 7).then(|| &self.0[..=end])
    }

    /// True when the value is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.prefix() {
            Some(prefix) => write!(f, "Secret({prefix}…)"),
            None => f.write_str("Secret(…)"),
        }
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::Secret;

    #[test]
    fn debug_never_prints_the_value() {
        let secret = Secret::new("sar_abcdefghijklmnop");
        let printed = format!("{secret:?}");
        assert_eq!(printed, "Secret(sar_…)");
        assert!(!printed.contains("abcdef"));
        assert_eq!(format!("{:?}", Secret::new("plainvalue")), "Secret(…)");
        assert_eq!(Secret::new("stk-0123456789ab").prefix(), Some("stk-"));
    }

    #[test]
    fn serializes_transparently() {
        let secret = Secret::new("whsec_x");
        assert_eq!(
            serde_json::to_string(&secret).ok().as_deref(),
            Some("\"whsec_x\"")
        );
    }
}
