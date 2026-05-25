// Adapted from code originally in [CocoIndex](https://CocoIndex)
// Original code from CocoIndex is copyrighted by CocoIndex
// and licensed under the Apache-2.0 License.
// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2026 CocoIndex
//
// All modifications from the upstream for Marque are copyrighted by Knitli Inc.
// SPDX-FileCopyrightText: 2026 Knitli Inc. (Marque)
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! The workspace error type and the helpers that build on it.
//!
//! [`Error`] sorts every failure into one of four kinds — caller mistakes
//! ([`client`](Error::client)), internal faults ([`internal`](Error::internal)),
//! errors from an embedding host language ([`host`](Error::host)), and a
//! [`Context`](Error::Context) wrapper that records a human-readable trail. The
//! [`ContextExt`] / [`StdContextExt`] traits add `.context(..)` to `Result` and
//! `Option`, and the `client_*` / `internal_*` / `api_*` macros build-and-bail
//! in one line.
//!
//! A few supporting types round it out: [`SError`] adapts [`Error`] to
//! [`std::error::Error`] where a `'static` source is required, [`SharedError`]
//! shares one error across many waiters (degrading to a message after the first
//! takes ownership), and [`ApiError`] is the boundary type for API responses.

use std::{
    any::Any,
    backtrace::Backtrace,
    error::Error as StdError,
    fmt::{Debug, Display},
    sync::{Arc, Mutex},
};

/// Any foreign error that can be carried as an [`Error::HostLang`]. Blanket-
/// implemented for every `Send + Sync + 'static` standard error.
pub trait HostError: Any + StdError + Send + Sync + 'static {}
impl<T: Any + StdError + Send + Sync + 'static> HostError for T {}

/// The workspace error. Each variant marks where a failure originated.
pub enum Error {
    /// A message wrapping an inner error, forming a context trail.
    Context { msg: String, source: Box<SError> },
    /// An error surfaced from an embedding host language (e.g. Python).
    HostLang(Box<dyn HostError>),
    /// A caller mistake — bad input or a failed precondition. Carries a
    /// backtrace and renders as `Invalid Request: ..`.
    Client { msg: String, bt: Backtrace },
    /// An internal fault, holding an [`anyhow::Error`] for flexible context.
    Internal(anyhow::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.format_context(f)? {
            Error::Context { .. } => Ok(()),
            Error::HostLang(e) => write!(f, "{}", e),
            Error::Client { msg, .. } => write!(f, "Invalid Request: {}", msg),
            Error::Internal(e) => write!(f, "{}", e),
        }
    }
}
impl Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.format_context(f)? {
            Error::Context { .. } => Ok(()),
            Error::HostLang(e) => write!(f, "{:?}", e),
            Error::Client { msg, bt } => {
                write!(f, "Invalid Request: {msg}\n\n{bt}\n")
            }
            Error::Internal(e) => write!(f, "{e:?}"),
        }
    }
}

/// A [`Result`](std::result::Result) defaulting to the workspace [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Backwards-compatibility alias for [`Error`].
pub type CError = Error;
/// Backwards-compatibility alias for [`Result`].
pub type CResult<T> = Result<T>;

impl Error {
    /// Wraps a host-language error as [`Error::HostLang`].
    pub fn host(e: impl HostError) -> Self {
        Self::HostLang(Box::new(e))
    }

    /// Builds a [`Error::Client`] from a message, capturing a backtrace.
    pub fn client(msg: impl Into<String>) -> Self {
        Self::Client {
            msg: msg.into(),
            bt: Backtrace::capture(),
        }
    }

    /// Wraps any `Into<anyhow::Error>` as [`Error::Internal`].
    pub fn internal(e: impl Into<anyhow::Error>) -> Self {
        Self::Internal(e.into())
    }

    /// Builds an [`Error::Internal`] straight from a message.
    pub fn internal_msg(msg: impl Into<String>) -> Self {
        Self::Internal(anyhow::anyhow!("{}", msg.into()))
    }

    /// Returns the backtrace, if this error (or the error it wraps) captured
    /// one. Host-language errors have none.
    pub fn backtrace(&self) -> Option<&Backtrace> {
        match self {
            Error::Client { bt, .. } => Some(bt),
            Error::Internal(e) => Some(e.backtrace()),
            Error::Context { source, .. } => source.0.backtrace(),
            Error::HostLang(_) => None,
        }
    }

    /// Peels off any [`Context`](Error::Context) layers to reach the underlying
    /// error.
    pub fn without_contexts(&self) -> &Error {
        match self {
            Error::Context { source, .. } => source.0.without_contexts(),
            other => other,
        }
    }

    /// Returns this error's source, if any, for chain traversal.
    pub fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Context { source, .. } => Some(source.as_ref()),
            Error::HostLang(e) => Some(e.as_ref()),
            Error::Internal(e) => e.source(),
            Error::Client { .. } => None,
        }
    }

    /// Wraps this error in a [`Context`](Error::Context) layer carrying `context`.
    pub fn context<C: Into<String>>(self, context: C) -> Self {
        Self::Context {
            msg: context.into(),
            source: Box::new(SError(self)),
        }
    }

    /// Like [`context`](Self::context), but builds the message lazily — `f` runs
    /// only on the error path.
    pub fn with_context<C: Into<String>, F: FnOnce() -> C>(self, f: F) -> Self {
        Self::Context {
            msg: f().into(),
            source: Box::new(SError(self)),
        }
    }

    /// Wraps this error in an [`SError`] so it satisfies a `'static`
    /// [`std::error::Error`] bound.
    pub fn std_error(self) -> SError {
        SError(self)
    }

    fn format_context(&self, f: &mut std::fmt::Formatter<'_>) -> Result<&Error, std::fmt::Error> {
        let mut current = self;
        if matches!(current, Error::Context { .. }) {
            write!(f, "\nContext:\n")?;
            let mut next_id = 1;
            while let Error::Context { msg, source } = current {
                writeln!(f, "  {next_id}: {msg}")?;
                current = source.inner();
                next_id += 1;
            }
        }
        Ok(current)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source()
    }
}

// A blanket `From<E: Into<anyhow::Error>>` would collide with the reflexive
// `From<T> for T`, so the common conversions are spelled out one at a time
// below instead.
impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error::Internal(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Internal(e.into())
    }
}
#[cfg(any(
    feature = "concur_control",
    feature = "retryable",
    feature = "batching"
))]
impl From<tokio::task::JoinError> for Error {
    fn from(e: tokio::task::JoinError) -> Self {
        Error::Internal(e.into())
    }
}
#[cfg(any(
    feature = "concur_control",
    feature = "retryable",
    feature = "batching"
))]
impl From<tokio::sync::oneshot::error::RecvError> for Error {
    fn from(e: tokio::sync::oneshot::error::RecvError) -> Self {
        Error::Internal(e.into())
    }
}
#[cfg(feature = "fingerprint")]
impl From<base64::DecodeError> for Error {
    fn from(e: base64::DecodeError) -> Self {
        Error::Internal(e.into())
    }
}

impl From<ResidualError> for Error {
    fn from(e: ResidualError) -> Self {
        Error::Internal(anyhow::Error::from(e))
    }
}
#[cfg(feature = "fingerprint")]
impl From<crate::fingerprint::FingerprinterError> for Error {
    fn from(e: crate::fingerprint::FingerprinterError) -> Self {
        Error::Internal(anyhow::Error::new(e))
    }
}

impl From<ApiError> for Error {
    fn from(e: ApiError) -> Self {
        Error::Internal(e.err)
    }
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        Error::Internal(anyhow::anyhow!("Mutex poison error: {}", e))
    }
}
impl From<std::num::ParseIntError> for Error {
    fn from(e: std::num::ParseIntError) -> Self {
        Error::Internal(e.into())
    }
}

impl From<std::str::ParseBoolError> for Error {
    fn from(e: std::str::ParseBoolError) -> Self {
        Error::Internal(e.into())
    }
}

impl From<std::fmt::Error> for Error {
    fn from(e: std::fmt::Error) -> Self {
        Error::Internal(e.into())
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Error::Internal(e.into())
    }
}

impl From<std::borrow::Cow<'_, str>> for Error {
    fn from(e: std::borrow::Cow<'_, str>) -> Self {
        Error::Internal(anyhow::anyhow!("{}", e))
    }
}
#[cfg(any(
    feature = "concur_control",
    feature = "retryable",
    feature = "batching"
))]
impl From<tokio::sync::AcquireError> for Error {
    fn from(e: tokio::sync::AcquireError) -> Self {
        Error::Internal(e.into())
    }
}
#[cfg(any(
    feature = "concur_control",
    feature = "retryable",
    feature = "batching"
))]
impl From<tokio::sync::watch::error::RecvError> for Error {
    fn from(e: tokio::sync::watch::error::RecvError) -> Self {
        Error::Internal(e.into())
    }
}

/// Adds `.context(..)` / `.with_context(..)` to [`Result<T>`] and [`Option<T>`].
///
/// On a `Result` it wraps the existing error; on an `Option` a `None` becomes a
/// [`client`](Error::client) error carrying the context message.
pub trait ContextExt<T> {
    /// Attaches `context` to the error path.
    fn context<C: Into<String>>(self, context: C) -> Result<T>;
    /// Attaches a lazily-built context message to the error path.
    fn with_context<C: Into<String>, F: FnOnce() -> C>(self, f: F) -> Result<T>;
}

impl<T> ContextExt<T> for Result<T> {
    fn context<C: Into<String>>(self, context: C) -> Result<T> {
        self.map_err(|e| e.context(context))
    }

    fn with_context<C: Into<String>, F: FnOnce() -> C>(self, f: F) -> Result<T> {
        self.map_err(|e| e.with_context(f))
    }
}

/// Adds `.context(..)` / `.with_context(..)` to a `Result` carrying any foreign
/// [`std::error::Error`], converting it to an [`Error::Internal`] along the way.
pub trait StdContextExt<T, E> {
    /// Converts the foreign error to [`Error`] and attaches `context`.
    fn context<C: Into<String>>(self, context: C) -> Result<T>;
    /// Converts the foreign error to [`Error`] and attaches a lazily-built
    /// context message.
    fn with_context<C: Into<String>, F: FnOnce() -> C>(self, f: F) -> Result<T>;
}

impl<T, E: StdError + Send + Sync + 'static> StdContextExt<T, E> for Result<T, E> {
    fn context<C: Into<String>>(self, context: C) -> Result<T> {
        self.map_err(|e| Error::internal(e).context(context))
    }

    fn with_context<C: Into<String>, F: FnOnce() -> C>(self, f: F) -> Result<T> {
        self.map_err(|e| Error::internal(e).with_context(f))
    }
}

impl<T> ContextExt<T> for Option<T> {
    fn context<C: Into<String>>(self, context: C) -> Result<T> {
        self.ok_or_else(|| Error::client(context))
    }

    fn with_context<C: Into<String>, F: FnOnce() -> C>(self, f: F) -> Result<T> {
        self.ok_or_else(|| Error::client(f()))
    }
}

/// Returns early with a formatted [`Error::client`].
#[macro_export]
macro_rules! client_bail {
    ( $fmt:literal $(, $($arg:tt)*)?) => {
        return Err($crate::error::Error::client(format!($fmt $(, $($arg)*)?)))
    };
}

/// Builds a formatted [`Error::client`] without returning.
#[macro_export]
macro_rules! client_error {
    ( $fmt:literal $(, $($arg:tt)*)?) => {
        $crate::error::Error::client(format!($fmt $(, $($arg)*)?))
    };
}

/// Returns early with a formatted [`Error::internal_msg`].
#[macro_export]
macro_rules! internal_bail {
    ( $fmt:literal $(, $($arg:tt)*)?) => {
        return Err($crate::error::Error::internal_msg(format!($fmt $(, $($arg)*)?)))
    };
}

/// Builds a formatted [`Error::internal_msg`] without returning.
#[macro_export]
macro_rules! internal_error {
    ( $fmt:literal $(, $($arg:tt)*)?) => {
        $crate::error::Error::internal_msg(format!($fmt $(, $($arg)*)?))
    };
}

/// Wraps [`Error`] so it satisfies the [`std::error::Error`] trait where a
/// `'static` source is required (e.g. inside [`anyhow`]).
pub struct SError(Error);

impl SError {
    /// Borrows the wrapped [`Error`].
    pub fn inner(&self) -> &Error {
        &self.0
    }
}

impl Display for SError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Debug for SError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl std::error::Error for SError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

struct ResidualErrorData {
    message: String,
    debug: String,
}

/// A cheap, cloneable snapshot of an error's rendered text.
///
/// When the original error cannot be cloned but its message must reach several
/// recipients (e.g. every waiter on a failed batch), capture it once here and
/// hand out clones.
#[derive(Clone)]
pub struct ResidualError(Arc<ResidualErrorData>);

impl ResidualError {
    /// Captures the `Display` and `Debug` renderings of `err`.
    pub fn new<Err: Display + Debug>(err: &Err) -> Self {
        Self(Arc::new(ResidualErrorData {
            message: err.to_string(),
            debug: err.to_string(),
        }))
    }
}

impl Display for ResidualError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0.message)
    }
}

impl Debug for ResidualError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0.debug)
    }
}

impl StdError for ResidualError {}

enum SharedErrorState {
    Error(Error),
    ResidualErrorMessage(ResidualError),
}

/// One error shared across many holders.
///
/// The first caller to [`into_result`](SharedResultExt::into_result) takes the
/// fully-typed [`Error`]; the slot then degrades to a [`ResidualError`], so
/// later callers still get the message but as a generic internal error.
#[derive(Clone)]
pub struct SharedError(Arc<Mutex<SharedErrorState>>);

impl SharedError {
    /// Wraps `err` so it can be shared across clones.
    pub fn new(err: Error) -> Self {
        Self(Arc::new(Mutex::new(SharedErrorState::Error(err))))
    }

    fn extract_error(&self) -> Error {
        let mut state = self.0.lock().unwrap();
        let mut_state = &mut *state;

        let residual_err = match mut_state {
            SharedErrorState::ResidualErrorMessage(err) => {
                // Already extracted; return a generic internal error with the residual message.
                return Error::internal(err.clone());
            }
            SharedErrorState::Error(err) => ResidualError::new(err),
        };

        let orig_state = std::mem::replace(
            mut_state,
            SharedErrorState::ResidualErrorMessage(residual_err),
        );
        let SharedErrorState::Error(err) = orig_state else {
            panic!("Expected shared error state to hold Error");
        };
        err
    }
}

impl Debug for SharedError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let state = self.0.lock().unwrap();
        match &*state {
            SharedErrorState::Error(err) => Debug::fmt(err, f),
            SharedErrorState::ResidualErrorMessage(err) => Debug::fmt(err, f),
        }
    }
}

impl Display for SharedError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let state = self.0.lock().unwrap();
        match &*state {
            SharedErrorState::Error(err) => Display::fmt(err, f),
            SharedErrorState::ResidualErrorMessage(err) => Display::fmt(err, f),
        }
    }
}

impl From<Error> for SharedError {
    fn from(err: Error) -> Self {
        Self(Arc::new(Mutex::new(SharedErrorState::Error(err))))
    }
}

/// Constructs an `Ok` in a [`SharedResult`], so call sites need not name
/// [`SharedError`].
pub fn shared_ok<T>(value: T) -> std::result::Result<T, SharedError> {
    Ok(value)
}

/// A [`Result`](std::result::Result) whose error is a [`SharedError`].
pub type SharedResult<T> = std::result::Result<T, SharedError>;

/// Converts a [`SharedResult`] into a plain [`Result`] by extracting the shared
/// error. See [`SharedError`] for the degrade-after-first-take behavior.
pub trait SharedResultExt<T> {
    /// Takes ownership and returns the typed error (or value).
    fn into_result(self) -> Result<T>;
}

impl<T> SharedResultExt<T> for std::result::Result<T, SharedError> {
    fn into_result(self) -> Result<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(err.extract_error()),
        }
    }
}

/// Borrowing counterpart to [`SharedResultExt`] for `&SharedResult<T>`, yielding
/// a borrowed value on success.
pub trait SharedResultExtRef<'a, T> {
    /// Extracts the shared error, or borrows the value.
    fn into_result(self) -> Result<&'a T>;
}

impl<'a, T> SharedResultExtRef<'a, T> for &'a std::result::Result<T, SharedError> {
    fn into_result(self) -> Result<&'a T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(err.extract_error()),
        }
    }
}

/// Builds a generic "Invariance violation" error for an unreachable state that
/// nonetheless needs a value rather than a panic.
pub fn invariance_violation() -> anyhow::Error {
    anyhow::anyhow!("Invariance violation")
}

/// The error type returned at the API boundary, wrapping an [`anyhow::Error`].
#[derive(Debug)]
pub struct ApiError {
    pub err: anyhow::Error,
}

impl ApiError {
    /// Builds an API error from a message.
    pub fn new(message: &str) -> Self {
        Self {
            err: anyhow::anyhow!("{}", message),
        }
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Display::fmt(&self.err, f)
    }
}

impl StdError for ApiError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.err.source()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> ApiError {
        if err.is::<ApiError>() {
            return err.downcast::<ApiError>().unwrap();
        }
        Self { err }
    }
}
impl From<Error> for ApiError {
    fn from(err: Error) -> ApiError {
        ApiError {
            err: anyhow::Error::from(err.std_error()),
        }
    }
}
/// Returns early with a formatted [`ApiError`], converted into the caller's
/// error type via `.into()`.
#[macro_export]
macro_rules! api_bail {
    ( $fmt:literal $(, $($arg:tt)*)?) => {
        return Err($crate::error::ApiError::new(&format!($fmt $(, $($arg)*)?)).into())
    };
}

/// Builds a formatted [`ApiError`] without returning.
#[macro_export]
macro_rules! api_error {
    ( $fmt:literal $(, $($arg:tt)*)?) => {
        $crate::error::ApiError::new(&format!($fmt $(, $($arg)*)?))
    };
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use std::backtrace::BacktraceStatus;
    use std::io;

    #[derive(Debug)]
    struct MockHostError(String);

    impl Display for MockHostError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "MockHostError: {}", self.0)
        }
    }

    impl StdError for MockHostError {}

    #[test]
    fn test_client_error_creation() {
        let err = Error::client("invalid input");
        assert!(matches!(&err, Error::Client { msg, .. } if msg == "invalid input"));
        assert!(matches!(err.without_contexts(), Error::Client { .. }));
    }

    #[test]
    fn test_internal_error_creation() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Internal { .. }));
    }

    #[test]
    fn test_internal_msg_error_creation() {
        let err = Error::internal_msg("something went wrong");
        assert!(matches!(err, Error::Internal { .. }));
        assert_eq!(err.to_string(), "something went wrong");
    }

    #[test]
    fn test_host_error_creation_and_detection() {
        let mock = MockHostError("test error".to_string());
        let err = Error::host(mock);
        assert!(matches!(err.without_contexts(), Error::HostLang(_)));

        if let Error::HostLang(host_err) = err.without_contexts() {
            let any: &dyn Any = host_err.as_ref();
            let downcasted = any.downcast_ref::<MockHostError>();
            assert!(downcasted.is_some());
            assert_eq!(downcasted.unwrap().0, "test error");
        } else {
            panic!("Expected HostLang variant");
        }
    }

    #[test]
    fn test_context_chaining() {
        let inner = Error::client("base error");
        let with_context: Result<()> = Err(inner);
        let wrapped = ContextExt::context(
            ContextExt::context(ContextExt::context(with_context, "layer 1"), "layer 2"),
            "layer 3",
        );

        let err = wrapped.unwrap_err();
        assert!(matches!(&err, Error::Context { msg, .. } if msg == "layer 3"));

        if let Error::Context { source, .. } = &err {
            assert!(
                matches!(source.as_ref(), SError(Error::Context { msg, .. }) if msg == "layer 2")
            );
        }
        assert_eq!(
            err.to_string(),
            "\nContext:\
             \n  1: layer 3\
             \n  2: layer 2\
             \n  3: layer 1\
             \nInvalid Request: base error"
        );
    }

    #[test]
    fn test_context_preserves_host_error() {
        let mock = MockHostError("original python error".to_string());
        let err = Error::host(mock);
        let wrapped: Result<()> = Err(err);
        let with_context = ContextExt::context(wrapped, "while processing request");

        let final_err = with_context.unwrap_err();
        assert!(matches!(final_err.without_contexts(), Error::HostLang(_)));

        if let Error::HostLang(host_err) = final_err.without_contexts() {
            let any: &dyn Any = host_err.as_ref();
            let downcasted = any.downcast_ref::<MockHostError>();
            assert!(downcasted.is_some());
            assert_eq!(downcasted.unwrap().0, "original python error");
        } else {
            panic!("Expected HostLang variant");
        }
    }

    #[test]
    fn test_backtrace_captured_for_client_error() {
        let err = Error::client("test");
        let bt = err.backtrace();
        assert!(bt.is_some());
        let status = bt.unwrap().status();
        assert!(
            status == BacktraceStatus::Captured
                || status == BacktraceStatus::Disabled
                || status == BacktraceStatus::Unsupported
        );
    }

    #[test]
    fn test_backtrace_captured_for_internal_error() {
        let err = Error::internal_msg("test internal");
        let bt = err.backtrace();
        assert!(bt.is_some());
    }

    #[test]
    fn test_backtrace_traverses_context() {
        let inner = Error::internal_msg("base");
        let wrapped: Result<()> = Err(inner);
        let with_context = ContextExt::context(wrapped, "context");

        let err = with_context.unwrap_err();
        let bt = err.backtrace();
        assert!(bt.is_some());
    }

    #[test]
    fn test_option_context_ext() {
        let opt: Option<i32> = None;
        let result = opt.context("value was missing");

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err.without_contexts(), Error::Client { .. }));
        assert!(matches!(&err, Error::Client { msg, .. } if msg == "value was missing"));
    }

    #[test]
    fn test_error_display_formats() {
        let client_err = Error::client("bad input");
        assert_eq!(client_err.to_string(), "Invalid Request: bad input");

        let internal_err = Error::internal_msg("db connection failed");
        assert_eq!(internal_err.to_string(), "db connection failed");

        let host_err = Error::host(MockHostError("py error".to_string()));
        assert_eq!(host_err.to_string(), "MockHostError: py error");
    }

    #[test]
    fn test_error_source_chain() {
        let inner = Error::internal_msg("root cause");
        let wrapped: Result<()> = Err(inner);
        let outer = ContextExt::context(wrapped, "outer context").unwrap_err();

        let source = outer.source();
        assert!(source.is_some());
    }

    #[test]
    fn test_internal_from_anyhow() {
        let err = Error::internal(anyhow::anyhow!("boom"));
        assert!(matches!(err, Error::Internal(_)));
        assert_eq!(err.to_string(), "boom");
    }

    #[test]
    fn test_source_per_variant() {
        assert!(Error::client("x").source().is_none());
        assert!(Error::host(MockHostError("x".into())).source().is_some());
    }

    #[test]
    fn test_backtrace_none_for_host_error() {
        let err = Error::host(MockHostError("x".into()));
        assert!(err.backtrace().is_none());
    }

    #[test]
    fn test_with_context_is_lazy() {
        let inner = Error::client("base");
        let wrapped: Result<()> = Err(inner);
        let err = ContextExt::with_context(wrapped, || "lazy ctx").unwrap_err();
        assert!(matches!(&err, Error::Context { msg, .. } if msg == "lazy ctx"));
    }

    #[test]
    fn test_option_with_context_is_lazy() {
        let opt: Option<i32> = None;
        let err = ContextExt::with_context(opt, || "missing value").unwrap_err();
        assert!(matches!(&err, Error::Client { msg, .. } if msg == "missing value"));
    }

    #[test]
    fn test_std_context_ext_wraps_foreign_error() {
        let r: std::result::Result<(), io::Error> = Err(io::Error::other("io failure"));
        let err = StdContextExt::context(r, "while doing io").unwrap_err();
        assert!(matches!(&err, Error::Context { msg, .. } if msg == "while doing io"));
        assert!(matches!(err.without_contexts(), Error::Internal(_)));
    }

    #[test]
    fn test_std_context_ext_with_context_is_lazy() {
        let r: std::result::Result<(), io::Error> = Err(io::Error::other("io failure"));
        let err = StdContextExt::with_context(r, || "lazy io ctx").unwrap_err();
        assert!(matches!(&err, Error::Context { msg, .. } if msg == "lazy io ctx"));
    }

    #[test]
    fn test_std_error_wrapper_roundtrip() {
        let serr = Error::client("boom").std_error();
        assert_eq!(serr.to_string(), "Invalid Request: boom");
        assert!(matches!(serr.inner(), Error::Client { .. }));
        assert!(format!("{serr:?}").contains("boom"));
    }

    #[test]
    fn test_invariance_violation_message() {
        assert_eq!(invariance_violation().to_string(), "Invariance violation");
    }

    // --- macros ------------------------------------------------------------

    #[test]
    fn test_client_macros() {
        fn bail() -> Result<()> {
            client_bail!("bad {}", 42);
        }
        assert!(matches!(&bail().unwrap_err(), Error::Client { msg, .. } if msg == "bad 42"));

        let e = client_error!("oops {}", 1);
        assert!(matches!(&e, Error::Client { msg, .. } if msg == "oops 1"));
    }

    #[test]
    fn test_internal_macros() {
        fn bail() -> Result<()> {
            internal_bail!("internal {}", 7);
        }
        let err = bail().unwrap_err();
        assert!(matches!(err, Error::Internal(_)));
        assert_eq!(err.to_string(), "internal 7");

        let e = internal_error!("ierr {}", 2);
        assert_eq!(e.to_string(), "ierr 2");
    }

    #[test]
    fn test_api_macros() {
        fn bail() -> std::result::Result<(), ApiError> {
            api_bail!("api {}", 9);
        }
        assert_eq!(bail().unwrap_err().to_string(), "api 9");
        assert_eq!(api_error!("aerr {}", 3).to_string(), "aerr 3");
    }

    // --- ResidualError -----------------------------------------------------

    #[test]
    fn test_residual_error_formats_and_converts() {
        let base = Error::client("residual base");
        let residual = ResidualError::new(&base);
        assert!(residual.to_string().contains("residual base"));
        assert!(format!("{residual:?}").contains("residual base"));

        let err: Error = residual.into();
        assert!(matches!(err, Error::Internal(_)));
    }

    // --- SharedError -------------------------------------------------------

    #[test]
    fn test_shared_error_degrades_after_first_extraction() {
        let shared = SharedError::new(Error::client("shared boom"));
        assert!(shared.to_string().contains("shared boom"));
        assert!(format!("{shared:?}").contains("shared boom"));

        // First extraction returns the real, fully-typed error.
        let first: SharedResult<()> = Err(shared.clone());
        let extracted = first.into_result().unwrap_err();
        assert!(matches!(extracted.without_contexts(), Error::Client { .. }));

        // After extraction the shared slot holds only the residual message, but
        // still renders the original text.
        assert!(shared.to_string().contains("shared boom"));
        let second: SharedResult<()> = Err(shared.clone());
        assert!(matches!(
            second.into_result().unwrap_err(),
            Error::Internal(_)
        ));
    }

    #[test]
    fn test_shared_ok_and_ref_extension() {
        assert_eq!(shared_ok::<i32>(5).into_result().unwrap(), 5);

        let ok: SharedResult<i32> = Ok(10);
        assert_eq!(*(&ok).into_result().unwrap(), 10);

        let errored: SharedResult<i32> = Err(SharedError::new(Error::client("e")));
        assert!((&errored).into_result().is_err());
    }

    #[test]
    fn test_from_error_for_shared_error() {
        let shared: SharedError = Error::internal_msg("x").into();
        assert!(shared.to_string().contains("x"));
    }

    // --- ApiError ----------------------------------------------------------

    #[test]
    fn test_api_error_new_display_and_into_error() {
        let api = ApiError::new("api boom");
        assert_eq!(api.to_string(), "api boom");

        let err: Error = api.into();
        assert!(matches!(err, Error::Internal(_)));
    }

    #[test]
    fn test_api_error_from_anyhow_passthrough_and_downcast() {
        let api: ApiError = anyhow::anyhow!("plain").into();
        assert_eq!(api.to_string(), "plain");

        // An anyhow error already wrapping an ApiError downcasts back to it.
        let any = anyhow::Error::new(ApiError::new("nested"));
        let api2: ApiError = any.into();
        assert_eq!(api2.to_string(), "nested");
    }

    #[test]
    fn test_api_error_from_core_error() {
        let api: ApiError = Error::client("bad request").into();
        assert!(api.to_string().contains("bad request"));
    }

    // --- From conversions --------------------------------------------------

    #[test]
    fn test_from_std_library_errors() {
        let _: Error = "5x".parse::<i32>().unwrap_err().into();
        let _: Error = "notbool".parse::<bool>().unwrap_err().into();
        let _: Error = std::fmt::Error.into();
        let _: Error = String::from_utf8(vec![0xff, 0xfe]).unwrap_err().into();

        let cow: std::borrow::Cow<str> = std::borrow::Cow::Borrowed("cow error");
        let e: Error = cow.into();
        assert!(e.to_string().contains("cow error"));
    }

    #[test]
    fn test_from_poison_error() {
        use std::sync::{Arc, Mutex};
        let m = Arc::new(Mutex::new(0));
        let m2 = m.clone();
        // Poison the mutex by panicking while the lock is held.
        let _ = std::thread::spawn(move || {
            let _guard = m2.lock().unwrap();
            panic!("intentional poison");
        })
        .join();

        let poison = m.lock().unwrap_err();
        let err: Error = poison.into();
        assert!(matches!(err, Error::Internal(_)));
        assert!(err.to_string().contains("Mutex poison"));
    }

    #[cfg(feature = "fingerprint")]
    #[test]
    fn test_from_base64_decode_error() {
        use base64::Engine as _;
        let decode_err = base64::prelude::BASE64_STANDARD.decode("a").unwrap_err();
        let err: Error = decode_err.into();
        assert!(matches!(err, Error::Internal(_)));
    }

    #[cfg(feature = "fingerprint")]
    #[test]
    fn test_from_fingerprinter_error() {
        use serde::ser::Error as _;
        let fe = crate::fingerprint::FingerprinterError::custom("fp boom");
        let err: Error = fe.into();
        assert!(matches!(err, Error::Internal(_)));
    }

    #[cfg(any(
        feature = "concur_control",
        feature = "retryable",
        feature = "batching"
    ))]
    #[tokio::test]
    async fn test_from_tokio_errors() {
        // oneshot RecvError
        let (tx, rx) = tokio::sync::oneshot::channel::<i32>();
        drop(tx);
        let _: Error = rx.await.unwrap_err().into();

        // AcquireError from a closed semaphore
        let sem = tokio::sync::Semaphore::new(1);
        sem.close();
        let _: Error = sem.acquire().await.unwrap_err().into();

        // JoinError from a panicking task
        let handle = tokio::spawn(async { panic!("boom") });
        let _: Error = handle.await.unwrap_err().into();

        // watch RecvError when all senders drop
        let (wtx, mut wrx) = tokio::sync::watch::channel(1);
        drop(wtx);
        let _: Error = wrx.changed().await.unwrap_err().into();
    }
}
