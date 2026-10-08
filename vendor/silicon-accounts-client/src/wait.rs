//! Polling helpers: device sign-in, custodian decisions, import jobs.

use std::time::Duration;

use crate::client::AccountsClient;
use crate::error::{Error, OAuthError, Result};
use crate::types::{CustodianRequestStatus, DeviceAuthorization, DevicePoll, TokenResponse};

/// How a waiting helper polls.
#[derive(Debug, Clone, PartialEq)]
pub struct WaitOptions {
    /// Delay before the second poll.
    pub initial_interval: Duration,
    /// Upper bound for the delay between polls.
    pub max_interval: Duration,
    /// Multiplier applied to the delay after every poll (1.0 = fixed interval).
    pub backoff: f64,
    /// Give up with [`Error::TimedOut`] after this long (`None` = never).
    pub timeout: Option<Duration>,
}

impl WaitOptions {
    /// Poll at a fixed interval.
    pub fn fixed(interval: Duration) -> Self {
        Self {
            initial_interval: interval,
            max_interval: interval,
            backoff: 1.0,
            timeout: None,
        }
    }

    /// Poll with exponential backoff (doubling) from `initial` up to `max`.
    pub fn backoff(initial: Duration, max: Duration) -> Self {
        Self {
            initial_interval: initial,
            max_interval: max,
            backoff: 2.0,
            timeout: None,
        }
    }

    /// Sets the overall timeout.
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// The delay after `current`.
    pub fn next_interval(&self, current: Duration) -> Duration {
        let next = current.mul_f64(self.backoff.max(1.0));
        next.min(self.max_interval.max(self.initial_interval))
    }

    /// The default for custodian decisions: 5 s doubling to 60 s, for up to 14 days.
    pub fn custodian_default() -> Self {
        Self::backoff(Duration::from_secs(5), Duration::from_secs(60))
            .with_timeout(Some(Duration::from_secs(14 * 24 * 3600)))
    }
}

/// What a waiting helper reports after each poll.
#[derive(Debug)]
#[non_exhaustive]
pub enum WaitEvent<'a, T> {
    /// A poll succeeded with this state.
    Polled(&'a T),
    /// A poll failed with a transient error (network, 5xx, rate limit); the helper keeps
    /// waiting and polls again after `retry_in`.
    TransientError {
        /// The error.
        error: &'a Error,
        /// Delay before the next poll.
        retry_in: Duration,
    },
}

/// Errors worth retrying while waiting: transport failures, 5xx and rate limits.
pub(crate) fn is_transient(error: &Error) -> bool {
    match error {
        Error::Http { .. } => true,
        Error::Api(api) => api.status >= 500 || api.status == 429,
        _ => false,
    }
}

impl AccountsClient {
    /// Polls a device sign-in until the Carbon approves it on the account site, honouring
    /// `interval` and `slow_down`, and returns the first-party tokens.
    ///
    /// Fails with an OAuth `access_denied` error when the Carbon denies the request and
    /// `expired_token` when the code expires (10 minutes).
    pub async fn wait_for_device_tokens(
        &self,
        authorization: &DeviceAuthorization,
        mut on_event: impl FnMut(WaitEvent<'_, DevicePoll>),
    ) -> Result<TokenResponse> {
        let started = tokio::time::Instant::now();
        let deadline = Duration::from_secs(authorization.expires_in.max(1));
        let mut interval = Duration::from_secs(authorization.interval.max(1));
        loop {
            tokio::time::sleep(interval).await;
            match self.device_poll(authorization.device_code.expose()).await {
                Ok(DevicePoll::Tokens(tokens)) => return Ok(*tokens),
                Ok(DevicePoll::Denied) => {
                    on_event(WaitEvent::Polled(&DevicePoll::Denied));
                    return Err(OAuthError::new(
                        400,
                        "access_denied",
                        Some(format!(
                            "The sign-in request {} was denied on the account site.",
                            authorization.user_code
                        )),
                    )
                    .into());
                }
                Ok(DevicePoll::Expired) => {
                    on_event(WaitEvent::Polled(&DevicePoll::Expired));
                    return Err(expired(authorization));
                }
                Ok(state) => {
                    if state == DevicePoll::SlowDown {
                        interval += Duration::from_secs(5);
                    }
                    on_event(WaitEvent::Polled(&state));
                }
                Err(error) if is_transient(&error) => {
                    on_event(WaitEvent::TransientError {
                        error: &error,
                        retry_in: interval,
                    });
                }
                Err(error) => return Err(error),
            }
            if started.elapsed() + interval > deadline {
                return Err(expired(authorization));
            }
        }
    }

    /// Polls a self-created Silicon's custodian request until the custodian decides (or
    /// the request expires), calling `on_event` after every poll. Transient errors are
    /// retried. Returns the final status: check [`CustodianRequestStatus::is_accepted`].
    pub async fn wait_for_custodian_decision(
        &self,
        request_id: &str,
        request_token: &str,
        options: &WaitOptions,
        mut on_event: impl FnMut(WaitEvent<'_, CustodianRequestStatus>),
    ) -> Result<CustodianRequestStatus> {
        let started = tokio::time::Instant::now();
        let mut interval = options.initial_interval;
        loop {
            match self.silicon_request_status(request_id, request_token).await {
                Ok(status) => {
                    on_event(WaitEvent::Polled(&status));
                    if !status.is_pending() {
                        return Ok(status);
                    }
                }
                Err(error) if is_transient(&error) => {
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
                        "The custodian has not decided on request {request_id} after {}s of waiting.",
                        started.elapsed().as_secs()
                    ),
                    hint: format!(
                        "The request stays open until it expires; check again with `accounts silicon request status {request_id}` (add --wait to keep waiting)."
                    ),
                });
            }
            tokio::time::sleep(interval).await;
            interval = options.next_interval(interval);
        }
    }
}

fn expired(authorization: &DeviceAuthorization) -> Error {
    OAuthError::new(
        400,
        "expired_token",
        Some(format!(
            "The sign-in code {} expired before it was approved (codes last {} minutes).",
            authorization.user_code,
            authorization.expires_in.div_ceil(60)
        )),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_up_to_the_cap() {
        let options = WaitOptions::backoff(Duration::from_secs(5), Duration::from_secs(60));
        let mut interval = options.initial_interval;
        let mut seen = vec![interval.as_secs()];
        for _ in 0..6 {
            interval = options.next_interval(interval);
            seen.push(interval.as_secs());
        }
        assert_eq!(seen, vec![5, 10, 20, 40, 60, 60, 60]);
        let fixed = WaitOptions::fixed(Duration::from_secs(2));
        assert_eq!(
            fixed.next_interval(Duration::from_secs(2)),
            Duration::from_secs(2)
        );
    }
}
