// Adapted from code originally in [CocoIndex](https://CocoIndex)
// Original code from CocoIndex is copyrighted by CocoIndex
// and licensed under the Apache-2.0 License.
// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2026 CocoIndex
//
// All modifications from the upstream for Marque are copyrighted by Knitli Inc.
// SPDX-FileCopyrightText: 2026 Knitli Inc. (Marque)
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use std::{
    future::Future,
    time::{Duration, Instant},
};
use tracing::trace;

pub trait IsRetryable {
    fn is_retryable(&self) -> bool;
}

pub struct Error {
    pub error: crate::error::Error,
    pub is_retryable: bool,
}

pub const DEFAULT_RETRY_TIMEOUT: Duration = Duration::from_secs(10 * 60);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.error, f)
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.error, f)
    }
}

impl IsRetryable for Error {
    fn is_retryable(&self) -> bool {
        self.is_retryable
    }
}

impl Error {
    pub fn retryable<E: Into<crate::error::Error>>(error: E) -> Self {
        Self {
            error: error.into(),
            is_retryable: true,
        }
    }

    pub fn not_retryable<E: Into<crate::error::Error>>(error: E) -> Self {
        Self {
            error: error.into(),
            is_retryable: false,
        }
    }
}

impl From<crate::error::Error> for Error {
    fn from(error: crate::error::Error) -> Self {
        Self {
            error,
            is_retryable: false,
        }
    }
}

impl From<Error> for crate::error::Error {
    fn from(val: Error) -> Self {
        val.error
    }
}

impl<E: IsRetryable + std::error::Error + Send + Sync + 'static> From<E> for Error {
    fn from(error: E) -> Self {
        Self {
            is_retryable: error.is_retryable(),
            error: anyhow::Error::from(error).into(),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[allow(non_snake_case)]
pub fn Ok<T>(value: T) -> Result<T> {
    Result::Ok(value)
}

pub struct RetryOptions {
    pub retry_timeout: Option<Duration>,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for RetryOptions {
    fn default() -> Self {
        Self {
            retry_timeout: Some(DEFAULT_RETRY_TIMEOUT),
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
        }
    }
}

pub static HEAVY_LOADED_OPTIONS: RetryOptions = RetryOptions {
    retry_timeout: Some(DEFAULT_RETRY_TIMEOUT),
    initial_backoff: Duration::from_secs(1),
    max_backoff: Duration::from_secs(60),
};

pub async fn run<
    Ok,
    Err: std::fmt::Display + IsRetryable,
    Fut: Future<Output = Result<Ok, Err>>,
    F: Fn() -> Fut,
>(
    f: F,
    options: &RetryOptions,
) -> Result<Ok, Err> {
    let deadline = options
        .retry_timeout
        .map(|timeout| Instant::now() + timeout);
    let mut backoff = options.initial_backoff;

    loop {
        match f().await {
            Result::Ok(result) => return Result::Ok(result),
            Result::Err(err) => {
                if !err.is_retryable() {
                    return Result::Err(err);
                }
                let mut sleep_duration = backoff;
                if let Some(deadline) = deadline {
                    let now = Instant::now();
                    if now >= deadline {
                        return Result::Err(err);
                    }
                    let remaining_time = deadline.saturating_duration_since(now);
                    sleep_duration = std::cmp::min(sleep_duration, remaining_time);
                }
                trace!(
                    "Will retry in {}ms for error: {}",
                    sleep_duration.as_millis(),
                    err
                );
                tokio::time::sleep(sleep_duration).await;
                if backoff < options.max_backoff {
                    backoff = std::cmp::min(
                        Duration::from_micros(
                            (backoff.as_micros() * u128::from(fastrand::u32(1618..=2000)) / 1000)
                                as u64,
                        ),
                        options.max_backoff,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Bare retryable error used to drive `run` without going through the
    /// crate error type's `std::error::Error` conversion path.
    struct TestErr {
        retryable: bool,
    }

    impl std::fmt::Display for TestErr {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "test error (retryable={})", self.retryable)
        }
    }

    impl IsRetryable for TestErr {
        fn is_retryable(&self) -> bool {
            self.retryable
        }
    }

    /// Implements `std::error::Error` + `IsRetryable` to exercise the blanket
    /// `From<E>` conversion that reads the source error's own retryable flag.
    #[derive(Debug)]
    struct StdRetryableErr;

    impl std::fmt::Display for StdRetryableErr {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "std retryable error")
        }
    }

    impl std::error::Error for StdRetryableErr {}

    impl IsRetryable for StdRetryableErr {
        fn is_retryable(&self) -> bool {
            true
        }
    }

    fn fast_options() -> RetryOptions {
        RetryOptions {
            retry_timeout: Some(Duration::from_secs(5)),
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(2),
        }
    }

    #[test]
    fn retryable_and_not_retryable_set_flag() {
        let e = Error::retryable(crate::error::Error::internal_msg("boom"));
        assert!(e.is_retryable());

        let e = Error::not_retryable(crate::error::Error::internal_msg("boom"));
        assert!(!e.is_retryable());
    }

    #[test]
    fn from_core_error_defaults_to_not_retryable() {
        let core = crate::error::Error::internal_msg("boom");
        let e: Error = core.into();
        assert!(!e.is_retryable());
    }

    #[test]
    fn into_core_error_unwraps_inner() {
        let e = Error::retryable(crate::error::Error::internal_msg("inner cause"));
        let core: crate::error::Error = e.into();
        assert_eq!(core.to_string(), "inner cause");
    }

    #[test]
    fn from_std_error_reads_its_retryable_flag() {
        let e: Error = StdRetryableErr.into();
        assert!(e.is_retryable());
    }

    #[test]
    fn display_and_debug_delegate_to_inner_error() {
        let e = Error::not_retryable(crate::error::Error::internal_msg("inner msg"));
        assert_eq!(format!("{e}"), "inner msg");
        assert!(format!("{e:?}").contains("inner msg"));
    }

    #[test]
    fn ok_helper_constructs_ok_variant() {
        let r: Result<i32> = Ok(42);
        assert!(matches!(r, Result::Ok(42)));
    }

    #[test]
    fn retry_options_default_matches_documented_values() {
        let o = RetryOptions::default();
        assert_eq!(o.retry_timeout, Some(DEFAULT_RETRY_TIMEOUT));
        assert_eq!(o.initial_backoff, Duration::from_millis(100));
        assert_eq!(o.max_backoff, Duration::from_secs(10));
    }

    #[test]
    fn heavy_loaded_options_use_longer_backoff() {
        assert_eq!(HEAVY_LOADED_OPTIONS.initial_backoff, Duration::from_secs(1));
        assert_eq!(HEAVY_LOADED_OPTIONS.max_backoff, Duration::from_secs(60));
    }

    #[tokio::test]
    async fn run_returns_immediately_on_success() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let opts = fast_options();

        let res: Result<i32, TestErr> = run(
            || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Result::Ok(7)
                }
            },
            &opts,
        )
        .await;

        assert!(matches!(res, Result::Ok(7)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn run_retries_until_success() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let opts = fast_options();

        let res: Result<i32, TestErr> = run(
            || {
                let c = c.clone();
                async move {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n < 2 {
                        Result::Err(TestErr { retryable: true })
                    } else {
                        Result::Ok(99)
                    }
                }
            },
            &opts,
        )
        .await;

        assert!(matches!(res, Result::Ok(99)));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn run_stops_on_non_retryable_error() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let opts = fast_options();

        let res: Result<i32, TestErr> = run(
            || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Result::Err(TestErr { retryable: false })
                }
            },
            &opts,
        )
        .await;

        assert!(res.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn run_gives_up_when_deadline_elapsed() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        // A zero timeout means the deadline is already in the past after the
        // first attempt, so a retryable error still terminates the loop.
        let opts = RetryOptions {
            retry_timeout: Some(Duration::ZERO),
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(1),
        };

        let res: Result<i32, TestErr> = run(
            || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Result::Err(TestErr { retryable: true })
                }
            },
            &opts,
        )
        .await;

        assert!(res.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn run_without_timeout_retries_then_succeeds() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let opts = RetryOptions {
            retry_timeout: None,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(1),
        };

        let res: Result<i32, TestErr> = run(
            || {
                let c = c.clone();
                async move {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n < 1 {
                        Result::Err(TestErr { retryable: true })
                    } else {
                        Result::Ok(5)
                    }
                }
            },
            &opts,
        )
        .await;

        assert!(matches!(res, Result::Ok(5)));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
