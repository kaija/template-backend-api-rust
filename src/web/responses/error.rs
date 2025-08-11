use axum::{
    http::{StatusCode, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};

use crate::models::ErrorResponse;
use crate::services::ServiceError;
use crate::repository::RepositoryError;
use super::context::{ErrorContext, ContextualErrorResponse};

/// Comprehensive application error type hierarchy that can be converted to HTTP responses
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // Configuration errors
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),
    
    // Database errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    // Repository errors
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),
    
    // Service errors
    #[error("Service error: {0}")]
    Service(#[from] ServiceError),
    
    // Validation errors
    #[error("Validation error: {0}")]
    Validation(String),
    
    // Authentication errors
    #[error("Authentication error: {0}")]
    Authentication(String),
    
    // Authorization errors
    #[error("Authorization error: {0}")]
    Authorization(String),
    
    // Not found errors
    #[error("Not found: {0}")]
    NotFound(String),
    
    // Conflict errors
    #[error("Conflict: {0}")]
    Conflict(String),
    
    // External service errors
    #[error("External service error: {0}")]
    ExternalService(String),
    
    // HTTP client errors
    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),
    
    // Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    // Timeout errors
    #[error("Timeout error: {0}")]
    Timeout(String),
    
    // Rate limiting errors
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),
    
    // Internal server errors
    #[error("Internal server error")]
    Internal,
    
    // Generic errors with context
    #[error("Error: {message}")]
    Generic { message: String },
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message, details, should_log_error) = match self {
            // Configuration errors - typically startup issues
            AppError::Config(ref e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error".to_string(), Some(e.to_string()), true)
            }
            
            // Database errors - log and return generic message
            AppError::Database(ref e) => {
                tracing::error!("Database error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Database unavailable".to_string(), None, true)
            }
            
            // Repository errors - handle specific cases
            AppError::Repository(RepositoryError::NotFound) => {
                (StatusCode::NOT_FOUND, "Resource not found".to_string(), None, false)
            }
            AppError::Repository(RepositoryError::DuplicateEmail(ref email)) => {
                (StatusCode::CONFLICT, "Resource already exists".to_string(), Some(format!("Email {} already exists", email)), false)
            }
            AppError::Repository(RepositoryError::Validation(ref msg)) => {
                (StatusCode::BAD_REQUEST, "Validation error".to_string(), Some(msg.clone()), false)
            }
            AppError::Repository(ref e) => {
                tracing::error!("Repository error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None, true)
            }
            
            // Service errors - handle specific cases
            AppError::Service(ServiceError::NotFound) => {
                (StatusCode::NOT_FOUND, "Resource not found".to_string(), None, false)
            }
            AppError::Service(ServiceError::AlreadyExists) => {
                (StatusCode::CONFLICT, "Resource already exists".to_string(), None, false)
            }
            AppError::Service(ServiceError::Validation(ref msg)) => {
                (StatusCode::BAD_REQUEST, "Validation failed".to_string(), Some(msg.clone()), false)
            }
            AppError::Service(ServiceError::Repository(ref e)) => {
                tracing::error!("Service repository error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None, true)
            }
            AppError::Service(ServiceError::ExternalService(ref msg)) => {
                tracing::warn!("External service error: {}", msg);
                (StatusCode::BAD_GATEWAY, "External service unavailable".to_string(), Some(msg.clone()), false)
            }
            
            // Validation errors - client errors
            AppError::Validation(ref msg) => {
                (StatusCode::BAD_REQUEST, "Validation error".to_string(), Some(msg.clone()), false)
            }
            
            // Authentication errors - client errors
            AppError::Authentication(ref msg) => {
                (StatusCode::UNAUTHORIZED, "Authentication failed".to_string(), Some(msg.clone()), false)
            }
            
            // Authorization errors - client errors
            AppError::Authorization(ref msg) => {
                (StatusCode::FORBIDDEN, "Access denied".to_string(), Some(msg.clone()), false)
            }
            
            // Not found errors - client errors
            AppError::NotFound(ref msg) => {
                (StatusCode::NOT_FOUND, "Resource not found".to_string(), Some(msg.clone()), false)
            }
            
            // Conflict errors - client errors
            AppError::Conflict(ref msg) => {
                (StatusCode::CONFLICT, "Conflict".to_string(), Some(msg.clone()), false)
            }
            
            // External service errors - dependency issues
            AppError::ExternalService(ref msg) => {
                tracing::warn!("External service error: {}", msg);
                (StatusCode::BAD_GATEWAY, "External service unavailable".to_string(), Some(msg.clone()), false)
            }
            
            // HTTP client errors - dependency issues
            AppError::HttpClient(ref e) => {
                tracing::warn!("HTTP client error: {:?}", e);
                (StatusCode::BAD_GATEWAY, "External service unavailable".to_string(), None, false)
            }
            
            // Serialization errors - typically internal issues
            AppError::Serialization(ref e) => {
                tracing::error!("Serialization error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None, true)
            }
            
            // IO errors - typically internal issues
            AppError::Io(ref e) => {
                tracing::error!("IO error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None, true)
            }
            
            // Timeout errors - service unavailable
            AppError::Timeout(ref msg) => {
                tracing::warn!("Timeout error: {}", msg);
                (StatusCode::REQUEST_TIMEOUT, "Request timeout".to_string(), Some(msg.clone()), false)
            }
            
            // Rate limiting errors - client errors
            AppError::RateLimit(ref msg) => {
                (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".to_string(), Some(msg.clone()), false)
            }
            
            // Internal server errors - log and capture
            AppError::Internal => {
                tracing::error!("Internal server error: {:?}", self);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None, true)
            }
            
            // Generic errors with context
            AppError::Generic { ref message } => {
                tracing::error!("Generic error: {}", message);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None, true)
            }
        };

        // Capture errors in Sentry for monitoring
        if should_log_error {
            sentry::capture_error(&self);
        }

        let error_response = match details {
            Some(details) => ErrorResponse::with_details(error_message, details),
            None => ErrorResponse::new(error_message),
        };

        (status, Json(error_response)).into_response()
    }
}

// Additional From trait implementations for better error conversion
impl From<validator::ValidationErrors> for AppError {
    fn from(errors: validator::ValidationErrors) -> Self {
        let error_messages: Vec<String> = errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                errors.iter().map(move |error| {
                    format!("{}: {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into()))
                })
            })
            .collect();
        
        AppError::Validation(error_messages.join(", "))
    }
}

impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        tracing::error!("Anyhow error converted to AppError: {:?}", error);
        AppError::Generic {
            message: error.to_string(),
        }
    }
}

impl From<tokio::time::error::Elapsed> for AppError {
    fn from(error: tokio::time::error::Elapsed) -> Self {
        AppError::Timeout(format!("Operation timed out: {}", error))
    }
}

// Helper methods for creating specific error types
impl AppError {
    /// Create a validation error with a custom message
    pub fn validation<S: Into<String>>(message: S) -> Self {
        AppError::Validation(message.into())
    }
    
    /// Create an authentication error with a custom message
    pub fn authentication<S: Into<String>>(message: S) -> Self {
        AppError::Authentication(message.into())
    }
    
    /// Create an authorization error with a custom message
    pub fn authorization<S: Into<String>>(message: S) -> Self {
        AppError::Authorization(message.into())
    }
    
    /// Create a not found error with a custom message
    pub fn not_found<S: Into<String>>(message: S) -> Self {
        AppError::NotFound(message.into())
    }
    
    /// Create a conflict error with a custom message
    pub fn conflict<S: Into<String>>(message: S) -> Self {
        AppError::Conflict(message.into())
    }
    
    /// Create an external service error with a custom message
    pub fn external_service<S: Into<String>>(message: S) -> Self {
        AppError::ExternalService(message.into())
    }
    
    /// Create a timeout error with a custom message
    pub fn timeout<S: Into<String>>(message: S) -> Self {
        AppError::Timeout(message.into())
    }
    
    /// Create a rate limit error with a custom message
    pub fn rate_limit<S: Into<String>>(message: S) -> Self {
        AppError::RateLimit(message.into())
    }
    
    /// Create a generic error with a custom message
    pub fn generic<S: Into<String>>(message: S) -> Self {
        AppError::Generic {
            message: message.into(),
        }
    }
    
    /// Check if the error is a client error (4xx status codes)
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            AppError::Validation(_)
                | AppError::Authentication(_)
                | AppError::Authorization(_)
                | AppError::NotFound(_)
                | AppError::Conflict(_)
                | AppError::RateLimit(_)
                | AppError::Service(ServiceError::NotFound)
                | AppError::Service(ServiceError::AlreadyExists)
                | AppError::Service(ServiceError::Validation(_))
                | AppError::Repository(RepositoryError::NotFound)
                | AppError::Repository(RepositoryError::DuplicateEmail(_))
                | AppError::Repository(RepositoryError::Validation(_))
        )
    }
    
    /// Check if the error is a server error (5xx status codes)
    pub fn is_server_error(&self) -> bool {
        !self.is_client_error()
    }
    
    /// Get the error category for logging and monitoring
    pub fn category(&self) -> &'static str {
        match self {
            AppError::Config(_) => "configuration",
            AppError::Database(_) => "database",
            AppError::Repository(_) => "repository",
            AppError::Service(_) => "service",
            AppError::Validation(_) => "validation",
            AppError::Authentication(_) => "authentication",
            AppError::Authorization(_) => "authorization",
            AppError::NotFound(_) => "not_found",
            AppError::Conflict(_) => "conflict",
            AppError::ExternalService(_) => "external_service",
            AppError::HttpClient(_) => "http_client",
            AppError::Serialization(_) => "serialization",
            AppError::Io(_) => "io",
            AppError::Timeout(_) => "timeout",
            AppError::RateLimit(_) => "rate_limit",
            AppError::Internal => "internal",
            AppError::Generic { .. } => "generic",
        }
    }

    /// Convert error to HTTP response parts (status, message, details)
    pub fn to_http_response_parts(&self) -> (StatusCode, String, Option<String>) {
        match self {
            // Configuration errors - typically startup issues
            AppError::Config(ref e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error".to_string(), Some(e.to_string()))
            }
            
            // Database errors - log and return generic message
            AppError::Database(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Database unavailable".to_string(), None)
            }
            
            // Repository errors - handle specific cases
            AppError::Repository(RepositoryError::NotFound) => {
                (StatusCode::NOT_FOUND, "Resource not found".to_string(), None)
            }
            AppError::Repository(RepositoryError::DuplicateEmail(ref email)) => {
                (StatusCode::CONFLICT, "Resource already exists".to_string(), Some(format!("Email {} already exists", email)))
            }
            AppError::Repository(RepositoryError::Validation(ref msg)) => {
                (StatusCode::BAD_REQUEST, "Validation error".to_string(), Some(msg.clone()))
            }
            AppError::Repository(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None)
            }
            
            // Service errors - handle specific cases
            AppError::Service(ServiceError::NotFound) => {
                (StatusCode::NOT_FOUND, "Resource not found".to_string(), None)
            }
            AppError::Service(ServiceError::AlreadyExists) => {
                (StatusCode::CONFLICT, "Resource already exists".to_string(), None)
            }
            AppError::Service(ServiceError::Validation(ref msg)) => {
                (StatusCode::BAD_REQUEST, "Validation failed".to_string(), Some(msg.clone()))
            }
            AppError::Service(ServiceError::Repository(_)) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None)
            }
            AppError::Service(ServiceError::ExternalService(ref msg)) => {
                (StatusCode::BAD_GATEWAY, "External service unavailable".to_string(), Some(msg.clone()))
            }
            
            // Validation errors - client errors
            AppError::Validation(ref msg) => {
                (StatusCode::BAD_REQUEST, "Validation error".to_string(), Some(msg.clone()))
            }
            
            // Authentication errors - client errors
            AppError::Authentication(ref msg) => {
                (StatusCode::UNAUTHORIZED, "Authentication failed".to_string(), Some(msg.clone()))
            }
            
            // Authorization errors - client errors
            AppError::Authorization(ref msg) => {
                (StatusCode::FORBIDDEN, "Access denied".to_string(), Some(msg.clone()))
            }
            
            // Not found errors - client errors
            AppError::NotFound(ref msg) => {
                (StatusCode::NOT_FOUND, "Resource not found".to_string(), Some(msg.clone()))
            }
            
            // Conflict errors - client errors
            AppError::Conflict(ref msg) => {
                (StatusCode::CONFLICT, "Conflict".to_string(), Some(msg.clone()))
            }
            
            // External service errors - dependency issues
            AppError::ExternalService(ref msg) => {
                (StatusCode::BAD_GATEWAY, "External service unavailable".to_string(), Some(msg.clone()))
            }
            
            // HTTP client errors - dependency issues
            AppError::HttpClient(_) => {
                (StatusCode::BAD_GATEWAY, "External service unavailable".to_string(), None)
            }
            
            // Serialization errors - typically internal issues
            AppError::Serialization(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None)
            }
            
            // IO errors - typically internal issues
            AppError::Io(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None)
            }
            
            // Timeout errors - service unavailable
            AppError::Timeout(ref msg) => {
                (StatusCode::REQUEST_TIMEOUT, "Request timeout".to_string(), Some(msg.clone()))
            }
            
            // Rate limiting errors - client errors
            AppError::RateLimit(ref msg) => {
                (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".to_string(), Some(msg.clone()))
            }
            
            // Internal server errors - log and capture
            AppError::Internal => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None)
            }
            
            // Generic errors with context
            AppError::Generic { message: _ } => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None)
            }
        }
    }
}

/// Contextual application error that includes correlation and debugging information
#[derive(Debug)]
pub struct ContextualAppError {
    pub error: AppError,
    pub context: ErrorContext,
}

impl ContextualAppError {
    /// Create a new contextual error
    pub fn new(error: AppError, context: ErrorContext) -> Self {
        Self { error, context }
    }

    /// Create a contextual error with correlation ID
    pub fn with_correlation_id(error: AppError, correlation_id: String) -> Self {
        let context = ErrorContext::new().with_correlation_id(correlation_id);
        Self::new(error, context)
    }

    /// Add request context information
    pub fn with_request_context(
        mut self,
        path: Option<String>,
        method: Option<String>,
    ) -> Self {
        if let Some(path) = path {
            self.context = self.context.with_request_path(path);
        }
        if let Some(method) = method {
            self.context = self.context.with_request_method(method);
        }
        self
    }

    /// Add user context
    pub fn with_user_id<S: Into<String>>(mut self, user_id: S) -> Self {
        self.context = self.context.with_user_id(user_id);
        self
    }

    /// Add metadata
    pub fn with_metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.context = self.context.with_metadata(key, value);
        self
    }

    /// Add multiple metadata entries
    pub fn with_metadata_map(mut self, metadata: std::collections::HashMap<String, String>) -> Self {
        self.context = self.context.with_metadata_map(metadata);
        self
    }

    /// Log the error with full context
    pub fn log_error(&self) {
        // Create a structured log entry with appropriate level
        if self.error.is_client_error() {
            tracing::warn!(
                error = %self.error,
                error_category = self.error.category(),
                correlation_id = self.context.correlation_id().unwrap_or("unknown"),
                request_path = self.context.request_path.as_deref().unwrap_or("unknown"),
                request_method = self.context.request_method.as_deref().unwrap_or("unknown"),
                user_id = self.context.user_id.as_deref().unwrap_or("anonymous"),
                "Client error occurred"
            );
        } else {
            tracing::error!(
                error = %self.error,
                error_category = self.error.category(),
                correlation_id = self.context.correlation_id().unwrap_or("unknown"),
                request_path = self.context.request_path.as_deref().unwrap_or("unknown"),
                request_method = self.context.request_method.as_deref().unwrap_or("unknown"),
                user_id = self.context.user_id.as_deref().unwrap_or("anonymous"),
                "Server error occurred"
            );
        }
    }
}

impl std::fmt::Display for ContextualAppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl std::error::Error for ContextualAppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl IntoResponse for ContextualAppError {
    fn into_response(self) -> Response {
        // Log the error with full context
        self.log_error();

        // Capture error in Sentry with context
        if self.error.is_server_error() {
            sentry::with_scope(
                |scope| {
                    if let Some(correlation_id) = self.context.correlation_id() {
                        scope.set_tag("correlation_id", correlation_id);
                    }
                    if let Some(ref path) = self.context.request_path {
                        scope.set_tag("request_path", path);
                    }
                    if let Some(ref method) = self.context.request_method {
                        scope.set_tag("request_method", method);
                    }
                    if let Some(ref user_id) = self.context.user_id {
                        scope.set_user(Some(sentry::User {
                            id: Some(user_id.clone()),
                            ..Default::default()
                        }));
                    }
                    // Add context as extra data
                    if let Ok(context_value) = serde_json::to_value(&self.context) {
                        if let Some(context_map) = context_value.as_object() {
                            for (key, value) in context_map {
                                if let Some(value_str) = value.as_str() {
                                    scope.set_extra(key, value_str.into());
                                }
                            }
                        }
                    }
                },
                || sentry::capture_error(&self.error),
            );
        }

        // Convert to HTTP response
        let (status, error_message, details) = self.error.to_http_response_parts();
        
        // Get correlation ID before moving context
        let correlation_id = self.context.correlation_id().map(|s| s.to_string());
        
        // Create contextual response
        let contextual_response = match details {
            Some(details) => ContextualErrorResponse::with_details(error_message, details, self.context),
            None => ContextualErrorResponse::new(error_message, self.context),
        };

        // Create client-safe response
        let client_response = contextual_response.client_safe();

        // Build HTTP response
        let mut response = (status, Json(client_response)).into_response();

        // Add correlation ID to response headers
        if let Some(correlation_id) = correlation_id {
            if let Ok(header_value) = HeaderValue::from_str(&correlation_id) {
                response.headers_mut().insert("x-correlation-id", header_value.clone());
                response.headers_mut().insert("x-request-id", header_value);
            }
        }

        response
    }
}

/// Helper trait for converting AppError to ContextualAppError
pub trait IntoContextualError {
    fn with_context(self, context: ErrorContext) -> ContextualAppError;
    fn with_correlation_id(self, correlation_id: String) -> ContextualAppError;
}

impl IntoContextualError for AppError {
    fn with_context(self, context: ErrorContext) -> ContextualAppError {
        ContextualAppError::new(self, context)
    }

    fn with_correlation_id(self, correlation_id: String) -> ContextualAppError {
        ContextualAppError::with_correlation_id(self, correlation_id)
    }
}

/// Middleware for extracting error context from requests
pub async fn error_context_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    // Extract context information from request
    let correlation_id = request.extensions().get::<String>().cloned();
    let path = Some(request.uri().path().to_string());
    let method = Some(request.method().to_string());

    // Store context in request extensions for use in handlers
    let context = ErrorContext::from_request_parts(correlation_id, path, method);
    
    // Process the request
    let mut response = next.run(request).await;
    
    // Add correlation ID to response headers if available
    if let Some(correlation_id) = context.correlation_id() {
        if let Ok(header_value) = HeaderValue::from_str(correlation_id) {
            response.headers_mut().insert("x-correlation-id", header_value.clone());
            response.headers_mut().insert("x-request-id", header_value);
        }
    }
    
    response
}