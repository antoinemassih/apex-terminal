//! Typed error hierarchy for the connectivity layer.
//!
//! Three layers:
//! - [`ApiError`] — anything a request/response surface can return
//! - [`AuthError`] — token-specific failures (subset of `ApiError::Auth`)
//! - [`ConnectionError`] — terminal lifecycle failures for `ConnectionState::Failed`
//!
//! All errors are `Clone` so the global error sink (which fans out to UI
//! observers) doesn't need to box or take ownership. `io::Error`s are
//! stringified at the boundary — losing the underlying `ErrorKind` is fine for
//! this layer; loggers / users only see the formatted message.

use std::time::Duration;
use thiserror::Error;

/// Anything an HTTP / WS / parsing call can hand back.
#[derive(Error, Debug, Clone)]
pub enum ApiError {
    /// Underlying network / IO failure (DNS, connect, broken pipe…).
    #[error("network: {0}")]
    Network(String),
    /// Non-2xx HTTP response, body trimmed to a usable size by the caller.
    #[error("http {status}: {body}")]
    Http { status: u16, body: String },
    /// Token / credential failure — usually triggers a refresh + retry.
    #[error("auth: {0}")]
    Auth(#[from] AuthError),
    /// 429 / explicit backpressure from the server.
    #[error("rate limited; retry after {retry_after:?}")]
    RateLimited { retry_after: Duration },
    /// Body deserialization / framing failure.
    #[error("parse: {0}")]
    Parse(String),
    /// Circuit breaker fast-fail (Wave 2 will add the breaker; the error
    /// variant ships now so call sites can pattern-match without churn).
    #[error("circuit breaker open")]
    CircuitOpen,
    /// Request did not complete within the configured deadline.
    #[error("timeout after {0:?}")]
    Timeout(Duration),
    /// Shutdown is in progress; new requests are rejected.
    #[error("shutdown in progress")]
    ShuttingDown,
    /// Provider does not implement this method (e.g. IB has no chain stream).
    #[error("not supported: {0}")]
    NotSupported(String),
}

/// Token lifecycle failure surface.
#[derive(Error, Debug, Clone)]
pub enum AuthError {
    #[error("token expired")]
    TokenExpired,
    #[error("token invalid")]
    TokenInvalid,
    #[error("refresh failed: {0}")]
    RefreshFailed(String),
    #[error("missing credentials")]
    MissingCredentials,
}

/// Terminal failure modes a `Connection` reports via `ConnectionState::Failed`.
#[derive(Error, Debug, Clone)]
pub enum ConnectionError {
    #[error("handshake failed: {0}")]
    HandshakeFailed(String),
    #[error("auth failed: {0}")]
    AuthFailed(#[from] AuthError),
    #[error("max reconnect attempts ({0}) exceeded")]
    MaxRetriesExceeded(u32),
    #[error("io: {0}")]
    Io(String),
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::Network(e.to_string())
    }
}

impl From<std::io::Error> for ConnectionError {
    fn from(e: std::io::Error) -> Self {
        ConnectionError::Io(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_renders_all_variants() {
        let _ = format!("{}", ApiError::Network("dns".into()));
        let _ = format!("{}", ApiError::Http { status: 500, body: "x".into() });
        let _ = format!("{}", ApiError::Auth(AuthError::TokenExpired));
        let _ = format!("{}", ApiError::RateLimited { retry_after: Duration::from_secs(1) });
        let _ = format!("{}", ApiError::Parse("eof".into()));
        let _ = format!("{}", ApiError::CircuitOpen);
        let _ = format!("{}", ApiError::Timeout(Duration::from_millis(50)));
        let _ = format!("{}", ApiError::ShuttingDown);

        let _ = format!("{}", ConnectionError::HandshakeFailed("oops".into()));
        let _ = format!("{}", ConnectionError::AuthFailed(AuthError::TokenInvalid));
        let _ = format!("{}", ConnectionError::MaxRetriesExceeded(8));
        let _ = format!("{}", ConnectionError::Io("broken pipe".into()));
    }

    #[test]
    fn from_impls_compile() {
        // AuthError → ApiError via #[from]
        let e: ApiError = AuthError::TokenExpired.into();
        assert!(matches!(e, ApiError::Auth(_)));

        // AuthError → ConnectionError via #[from]
        let e: ConnectionError = AuthError::MissingCredentials.into();
        assert!(matches!(e, ConnectionError::AuthFailed(_)));

        // io::Error → ApiError
        let io = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "x");
        let e: ApiError = io.into();
        assert!(matches!(e, ApiError::Network(_)));

        // io::Error → ConnectionError
        let io = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "x");
        let e: ConnectionError = io.into();
        assert!(matches!(e, ConnectionError::Io(_)));
    }
}
