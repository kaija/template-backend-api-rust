use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use crate::services::AuthError;
use crate::web::router::AppState;

/// Authentication middleware
/// Requires a valid Bearer token in the Authorization header
pub async fn auth_middleware(
    State(app_state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let correlation_id = request
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    // Extract and validate authorization header
    let token = match extract_bearer_token(request.headers()) {
        Some(token) => token,
        None => {
            tracing::warn!("Missing or invalid authorization header [correlation_id: {}]", correlation_id);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Validate token and get current user
    let current_user = match app_state.auth_service().validate_token(token).await {
        Ok(user) => {
            tracing::debug!("Authentication successful for user: {} [correlation_id: {}]", user.id, correlation_id);
            user
        }
        Err(AuthError::InvalidToken) => {
            tracing::warn!("Invalid token provided [correlation_id: {}]", correlation_id);
            return Err(StatusCode::UNAUTHORIZED);
        }
        Err(AuthError::TokenExpired) => {
            tracing::warn!("Expired token provided [correlation_id: {}]", correlation_id);
            return Err(StatusCode::UNAUTHORIZED);
        }
        Err(AuthError::Internal(msg)) => {
            tracing::error!("Authentication service error: {} [correlation_id: {}]", msg, correlation_id);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Err(_) => {
            tracing::error!("Unknown authentication error [correlation_id: {}]", correlation_id);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Add current user to request extensions for use in handlers
    request.extensions_mut().insert(current_user);

    // Continue processing
    Ok(next.run(request).await)
}

/// Optional authentication middleware (doesn't fail if no token provided)
/// If a valid token is provided, the user will be added to request extensions
pub async fn optional_auth_middleware(
    State(app_state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let correlation_id = request
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    // Try to extract and validate authorization header
    if let Some(token) = extract_bearer_token(request.headers()) {
        match app_state.auth_service().validate_token(token).await {
            Ok(current_user) => {
                tracing::debug!("Optional authentication successful for user: {} [correlation_id: {}]", current_user.id, correlation_id);
                request.extensions_mut().insert(current_user);
            }
            Err(AuthError::InvalidToken) => {
                tracing::debug!("Invalid token in optional auth [correlation_id: {}]", correlation_id);
            }
            Err(AuthError::TokenExpired) => {
                tracing::debug!("Expired token in optional auth [correlation_id: {}]", correlation_id);
            }
            Err(err) => {
                tracing::warn!("Optional authentication error: {:?} [correlation_id: {}]", err, correlation_id);
            }
        }
    }

    // Continue processing regardless of authentication status
    next.run(request).await
}

/// Extract Bearer token from Authorization header
fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
}

/// Authorization middleware for role-based access control
/// This middleware should be applied after authentication middleware
pub async fn require_role_middleware(
    required_role: &'static str,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, StatusCode>> + Send>> + Clone {
    move |request: Request, next: Next| {
        let required_role = required_role;
        Box::pin(async move {
            let correlation_id = request
                .extensions()
                .get::<String>()
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());

            // Get current user from request extensions (should be set by auth middleware)
            let current_user = request
                .extensions()
                .get::<crate::models::CurrentUser>()
                .ok_or_else(|| {
                    tracing::warn!("Role check failed: no authenticated user [correlation_id: {}]", correlation_id);
                    StatusCode::UNAUTHORIZED
                })?;

            // TODO: Implement role checking logic
            // For now, we'll assume all authenticated users have access
            // In a real implementation, you'd check user roles/permissions
            tracing::debug!("Role check passed for user: {} (required: {}) [correlation_id: {}]",
                current_user.id, required_role, correlation_id);

            Ok(next.run(request).await)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn test_extract_bearer_token_valid() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer abc123".parse().unwrap());

        let token = extract_bearer_token(&headers);
        assert_eq!(token, Some("abc123"));
    }

    #[test]
    fn test_extract_bearer_token_invalid() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Basic abc123".parse().unwrap());

        let token = extract_bearer_token(&headers);
        assert_eq!(token, None);
    }

    #[test]
    fn test_extract_bearer_token_empty() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer ".parse().unwrap());

        let token = extract_bearer_token(&headers);
        assert_eq!(token, None);
    }

    #[test]
    fn test_extract_bearer_token_missing() {
        let headers = HeaderMap::new();
        let token = extract_bearer_token(&headers);
        assert_eq!(token, None);
    }
}
