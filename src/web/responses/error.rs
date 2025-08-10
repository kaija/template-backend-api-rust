use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use crate::models::ErrorResponse;
use crate::services::ServiceError;

/// Application error type that can be converted to HTTP responses
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Service error: {0}")]
    Service(#[from] ServiceError),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Authentication error: {0}")]
    Authentication(String),
    
    #[error("Authorization error: {0}")]
    Authorization(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Conflict: {0}")]
    Conflict(String),
    
    #[error("Internal server error")]
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message, details) = match self {
            AppError::Service(ServiceError::NotFound) => {
                (StatusCode::NOT_FOUND, "Resource not found".to_string(), None)
            }
            AppError::Service(ServiceError::AlreadyExists) => {
                (StatusCode::CONFLICT, "Resource already exists".to_string(), None)
            }
            AppError::Service(ServiceError::Validation(msg)) => {
                (StatusCode::BAD_REQUEST, "Validation failed".to_string(), Some(msg))
            }
            AppError::Service(ServiceError::Repository(e)) => {
                tracing::error!("Repository error: {:?}", e);
                sentry::capture_error(&e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None)
            }
            AppError::Validation(msg) => {
                (StatusCode::BAD_REQUEST, "Validation error".to_string(), Some(msg))
            }
            AppError::Authentication(msg) => {
                (StatusCode::UNAUTHORIZED, "Authentication failed".to_string(), Some(msg))
            }
            AppError::Authorization(msg) => {
                (StatusCode::FORBIDDEN, "Access denied".to_string(), Some(msg))
            }
            AppError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, "Resource not found".to_string(), Some(msg))
            }
            AppError::Conflict(msg) => {
                (StatusCode::CONFLICT, "Conflict".to_string(), Some(msg))
            }
            AppError::Internal => {
                tracing::error!("Internal server error: {:?}", self);
                sentry::capture_error(&self);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None)
            }
        };

        let error_response = match details {
            Some(details) => ErrorResponse::with_details(error_message, details),
            None => ErrorResponse::new(error_message),
        };

        (status, Json(error_response)).into_response()
    }
}