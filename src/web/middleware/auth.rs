use axum::{
    extract::{Request, State},
    http::{StatusCode, HeaderMap},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::models::CurrentUser;
use crate::services::{AuthService, AuthError};

/// Authentication middleware
pub async fn auth_middleware(
    State(auth_service): State<Arc<dyn AuthService>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract authorization header
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    // Validate token and get current user
    let current_user = match auth_service.validate_token(token).await {
        Ok(user) => user,
        Err(AuthError::InvalidToken) => return Err(StatusCode::UNAUTHORIZED),
        Err(AuthError::TokenExpired) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    // Add current user to request extensions
    request.extensions_mut().insert(current_user);

    // Continue processing
    Ok(next.run(request).await)
}

/// Optional authentication middleware (doesn't fail if no token provided)
pub async fn optional_auth_middleware(
    State(auth_service): State<Arc<dyn AuthService>>,
    mut request: Request,
    next: Next,
) -> Response {
    // Extract authorization header
    if let Some(auth_header) = request
        .headers()
        .get("authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
    {
        // Try to validate token
        if let Ok(current_user) = auth_service.validate_token(auth_header).await {
            request.extensions_mut().insert(current_user);
        }
    }

    // Continue processing regardless of authentication status
    next.run(request).await
}