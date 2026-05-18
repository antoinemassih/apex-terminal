//! Token refresh trait + `with_auth_retry` helper.
//!
//! The contract lets HTTP gateways react to 401 responses by calling
//! `refresh_token()` once, then re-issuing the original request with the new
//! bearer. Anything past one retry surfaces the original `ApiError::Auth`
//! upstream — callers should not loop on stale credentials.

use async_trait::async_trait;
use std::future::Future;

use super::error::{ApiError, AuthError};

#[async_trait]
pub trait Authenticated: Send + Sync {
    /// Force a token refresh and return the new bearer.
    ///
    /// Implementations should be idempotent against concurrent callers
    /// (deduplicate in-flight refreshes if possible).
    async fn refresh_token(&self) -> Result<String, AuthError>;

    /// Latest cached token, if any. `None` means "no credentials yet" —
    /// the caller should treat this as `AuthError::MissingCredentials`.
    fn current_token(&self) -> Option<String>;
}

/// Wrap an HTTP call so a single 401 triggers refresh + one retry.
///
/// `f` is invoked with the current bearer string. On `ApiError::Auth` or an
/// HTTP 401, we ask the provider to refresh and call `f` exactly once more
/// with the new token. Anything other than auth-failure propagates
/// immediately — this helper does NOT replace a general retry strategy.
#[tracing::instrument(skip(auth, f), level = "debug", fields(has_token))]
pub async fn with_auth_retry<T, F, Fut>(auth: &dyn Authenticated, f: F) -> Result<T, ApiError>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<T, ApiError>>,
{
    tracing::Span::current().record("has_token", auth.current_token().is_some());
    let token = auth
        .current_token()
        .ok_or_else(|| ApiError::Auth(AuthError::MissingCredentials))?;

    match f(token).await {
        Ok(v) => Ok(v),
        Err(e) if is_auth_failure(&e) => {
            // One refresh, one retry. If the refresh itself fails, surface that.
            let new_token = auth.refresh_token().await.map_err(ApiError::Auth)?;
            f(new_token).await
        }
        Err(other) => Err(other),
    }
}

fn is_auth_failure(e: &ApiError) -> bool {
    matches!(e, ApiError::Auth(_) | ApiError::Http { status: 401, .. })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    struct MockAuth {
        token: Arc<std::sync::Mutex<String>>,
        refresh_count: AtomicU32,
        refresh_should_fail: bool,
    }

    impl MockAuth {
        fn new(initial: &str) -> Self {
            Self {
                token: Arc::new(std::sync::Mutex::new(initial.to_string())),
                refresh_count: AtomicU32::new(0),
                refresh_should_fail: false,
            }
        }
    }

    #[async_trait]
    impl Authenticated for MockAuth {
        async fn refresh_token(&self) -> Result<String, AuthError> {
            self.refresh_count.fetch_add(1, Ordering::SeqCst);
            if self.refresh_should_fail {
                return Err(AuthError::RefreshFailed("test".into()));
            }
            let mut g = self.token.lock().unwrap();
            *g = "refreshed".to_string();
            Ok(g.clone())
        }
        fn current_token(&self) -> Option<String> {
            Some(self.token.lock().unwrap().clone())
        }
    }

    #[tokio::test]
    async fn retries_once_after_401_then_succeeds() {
        let auth = MockAuth::new("stale");
        let calls = AtomicU32::new(0);
        let result: Result<&str, ApiError> = with_auth_retry(&auth, |tok| {
            let calls = &calls;
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                if tok == "stale" {
                    Err(ApiError::Http { status: 401, body: "expired".into() })
                } else {
                    Ok("ok")
                }
            }
        }).await;
        assert_eq!(result.unwrap(), "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(auth.refresh_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn second_401_propagates() {
        let auth = MockAuth::new("stale");
        let calls = AtomicU32::new(0);
        let result: Result<&str, ApiError> = with_auth_retry(&auth, |_tok| {
            let calls = &calls;
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<&str, _>(ApiError::Http { status: 401, body: "still expired".into() })
            }
        }).await;
        assert!(matches!(result, Err(ApiError::Http { status: 401, .. })));
        // Original + 1 retry = 2 calls. No third attempt.
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn non_auth_error_does_not_retry() {
        let auth = MockAuth::new("good");
        let calls = AtomicU32::new(0);
        let result: Result<&str, ApiError> = with_auth_retry(&auth, |_tok| {
            let calls = &calls;
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<&str, _>(ApiError::Http { status: 500, body: "boom".into() })
            }
        }).await;
        assert!(matches!(result, Err(ApiError::Http { status: 500, .. })));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(auth.refresh_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn auth_error_variant_also_triggers_retry() {
        let auth = MockAuth::new("stale");
        let attempts = AtomicU32::new(0);
        let _: Result<&str, ApiError> = with_auth_retry(&auth, |_tok| {
            let attempts = &attempts;
            async move {
                let n = attempts.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Err::<&str, _>(ApiError::Auth(AuthError::TokenExpired))
                } else {
                    Ok("ok")
                }
            }
        }).await;
        assert_eq!(auth.refresh_count.load(Ordering::SeqCst), 1);
    }
}
