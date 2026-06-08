//! Typed error returned from the data/IO operations.
//!
//! Returning a free-form `String` forces callers to pattern-match on prose
//! ("DB not initialized" vs "permission denied" vs "file not found") which is
//! brittle and locale-sensitive.
//!
//! `AppError` serializes to a structured object:
//! ```json
//! { "code": "db_uninitialized", "message": "Database not initialized" }
//! ```
//! so callers can branch on `code` without scraping the message text.

/// Structured, serializable error payload.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AppError {
    /// Machine-readable discriminant — frontend switches on this.
    pub code: String,
    /// Human-readable message, suitable for logs or fallback display.
    pub message: String,
    /// Optional structured detail (e.g. `{"status": 503}` for upstream errors).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: serde_json::Value) -> Self {
        self.detail = Some(detail);
        self
    }

    // ── Common constructors ──────────────────────────────────────────────────

    pub fn db_uninitialized() -> Self {
        Self::new("db_uninitialized", "Database not initialized")
    }

    pub fn not_found(what: &str) -> Self {
        Self::new("not_found", format!("{what} not found"))
    }

    pub fn permission_denied(what: &str) -> Self {
        Self::new("permission_denied", format!("Permission denied: {what}"))
    }

    pub fn invalid_input(what: &str) -> Self {
        Self::new("invalid_input", format!("Invalid input: {what}"))
    }

    pub fn io_error(e: std::io::Error) -> Self {
        Self::new("io_error", e.to_string())
    }

    pub fn parse_error(e: impl std::fmt::Display) -> Self {
        Self::new("parse_error", e.to_string())
    }

    pub fn auth_expired() -> Self {
        Self::new("auth_expired", "Authentication expired")
    }

    pub fn network_error(e: impl std::fmt::Display) -> Self {
        Self::new("network_error", e.to_string())
    }

    pub fn upstream_error(status: u16, body: impl Into<String>) -> Self {
        Self::new(format!("upstream_{status}"), body)
            .with_detail(serde_json::json!({ "status": status }))
    }

    pub fn internal(e: impl std::fmt::Display) -> Self {
        Self::new("internal", e.to_string())
    }
}

// ── From impls so the `?` operator works in command bodies ───────────────────

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::io_error(e)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::parse_error(e)
    }
}

impl From<crate::data::connectivity::ApiError> for AppError {
    fn from(e: crate::data::connectivity::ApiError) -> Self {
        use crate::data::connectivity::ApiError;
        match e {
            ApiError::Auth(_) => Self::auth_expired(),
            ApiError::Http { status, body } => Self::upstream_error(status, body),
            ApiError::Network(s) => Self::network_error(s),
            ApiError::RateLimited { retry_after } => Self::new(
                "rate_limited",
                format!("Rate limited; retry after {:?}", retry_after),
            ),
            ApiError::Parse(s) => Self::parse_error(s),
            ApiError::CircuitOpen => {
                Self::new("circuit_open", "Service temporarily unavailable")
            }
            ApiError::Timeout(d) => {
                Self::new("timeout", format!("Operation timed out after {:?}", d))
            }
            ApiError::NotSupported(s) => Self::new("not_supported", s),
            ApiError::ShuttingDown => {
                Self::new("shutting_down", "Application is shutting down")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_serializes_to_expected_shape() {
        let e = AppError::not_found("watchlist");
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "not_found");
        assert_eq!(json["message"], "watchlist not found");
        assert!(json.get("detail").is_none());

        let with_detail = AppError::upstream_error(503, "service unavailable");
        let json = serde_json::to_value(&with_detail).unwrap();
        assert_eq!(json["code"], "upstream_503");
        assert_eq!(json["detail"]["status"], 503);
        assert_eq!(json["message"], "service unavailable");
    }

    #[test]
    fn db_uninitialized_has_stable_code() {
        let e = AppError::db_uninitialized();
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "db_uninitialized");
    }

    #[test]
    fn auth_expired_has_stable_code() {
        let e = AppError::auth_expired();
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "auth_expired");
    }
}
