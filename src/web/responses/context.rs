use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Error context for correlation and debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Correlation ID for request tracing
    pub correlation_id: Option<String>,
    /// Request path that caused the error
    pub request_path: Option<String>,
    /// HTTP method that caused the error
    pub request_method: Option<String>,
    /// User ID if available
    pub user_id: Option<String>,
    /// Additional context metadata
    pub metadata: HashMap<String, String>,
    /// Timestamp when the error occurred
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new() -> Self {
        Self {
            correlation_id: None,
            request_path: None,
            request_method: None,
            user_id: None,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create error context from request extensions
    pub fn from_request_parts(
        correlation_id: Option<String>,
        request_path: Option<String>,
        request_method: Option<String>,
    ) -> Self {
        Self {
            correlation_id,
            request_path,
            request_method,
            user_id: None,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Set correlation ID
    pub fn with_correlation_id<S: Into<String>>(mut self, correlation_id: S) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Set request path
    pub fn with_request_path<S: Into<String>>(mut self, path: S) -> Self {
        self.request_path = Some(path.into());
        self
    }

    /// Set request method
    pub fn with_request_method<S: Into<String>>(mut self, method: S) -> Self {
        self.request_method = Some(method.into());
        self
    }

    /// Set user ID
    pub fn with_user_id<S: Into<String>>(mut self, user_id: S) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Add metadata
    pub fn with_metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Add multiple metadata entries
    pub fn with_metadata_map(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata.extend(metadata);
        self
    }

    /// Get correlation ID for logging
    pub fn correlation_id(&self) -> Option<&str> {
        self.correlation_id.as_deref()
    }

    /// Create a structured log entry for this error context
    pub fn to_log_fields(&self) -> Vec<(&'static str, String)> {
        let mut fields = Vec::new();

        if let Some(ref correlation_id) = self.correlation_id {
            fields.push(("correlation_id", correlation_id.clone()));
        }

        if let Some(ref path) = self.request_path {
            fields.push(("request_path", path.clone()));
        }

        if let Some(ref method) = self.request_method {
            fields.push(("request_method", method.clone()));
        }

        if let Some(ref user_id) = self.user_id {
            fields.push(("user_id", user_id.clone()));
        }

        fields.push(("timestamp", self.timestamp.to_rfc3339()));

        fields
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Enhanced error response with context information
#[derive(Debug, Serialize, Deserialize)]
pub struct ContextualErrorResponse {
    /// Error message for the client
    pub error: String,
    /// Additional error details (optional)
    pub details: Option<String>,
    /// Error context for debugging and correlation
    pub context: ErrorContext,
}

impl ContextualErrorResponse {
    /// Create a new contextual error response
    pub fn new<S: Into<String>>(error: S, context: ErrorContext) -> Self {
        Self {
            error: error.into(),
            details: None,
            context,
        }
    }

    /// Create a contextual error response with details
    pub fn with_details<S: Into<String>, D: Into<String>>(
        error: S,
        details: D,
        context: ErrorContext,
    ) -> Self {
        Self {
            error: error.into(),
            details: Some(details.into()),
            context,
        }
    }

    /// Create a client-safe error response (removes sensitive context)
    pub fn client_safe(mut self) -> Self {
        // Keep only correlation ID and timestamp for client
        let safe_context = ErrorContext {
            correlation_id: self.context.correlation_id.clone(),
            request_path: None,
            request_method: None,
            user_id: None,
            metadata: HashMap::new(),
            timestamp: self.context.timestamp,
        };

        self.context = safe_context;
        self
    }

    /// Get correlation ID for response headers
    pub fn correlation_id(&self) -> Option<&str> {
        self.context.correlation_id()
    }
}

/// Trait for extracting error context from request
pub trait ErrorContextExtractor {
    /// Extract error context from the current request
    fn extract_error_context(&self) -> ErrorContext;
}

/// Helper for creating error context from Axum request parts
pub struct RequestContextExtractor {
    pub correlation_id: Option<String>,
    pub path: Option<String>,
    pub method: Option<String>,
    pub user_id: Option<String>,
}

impl RequestContextExtractor {
    pub fn new() -> Self {
        Self {
            correlation_id: None,
            path: None,
            method: None,
            user_id: None,
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: Option<String>) -> Self {
        self.correlation_id = correlation_id;
        self
    }

    pub fn with_path(mut self, path: Option<String>) -> Self {
        self.path = path;
        self
    }

    pub fn with_method(mut self, method: Option<String>) -> Self {
        self.method = method;
        self
    }

    pub fn with_user_id(mut self, user_id: Option<String>) -> Self {
        self.user_id = user_id;
        self
    }

    pub fn build(self) -> ErrorContext {
        ErrorContext::from_request_parts(self.correlation_id, self.path, self.method)
            .with_user_id(self.user_id.unwrap_or_else(|| "anonymous".to_string()))
    }
}

impl Default for RequestContextExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_context_creation() {
        let context = ErrorContext::new()
            .with_correlation_id("test-123")
            .with_request_path("/api/users")
            .with_request_method("POST")
            .with_user_id("user-456")
            .with_metadata("key", "value");

        assert_eq!(context.correlation_id(), Some("test-123"));
        assert_eq!(context.request_path, Some("/api/users".to_string()));
        assert_eq!(context.request_method, Some("POST".to_string()));
        assert_eq!(context.user_id, Some("user-456".to_string()));
        assert_eq!(context.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_contextual_error_response() {
        let context = ErrorContext::new().with_correlation_id("test-123");
        let response = ContextualErrorResponse::with_details(
            "Validation failed",
            "Name is required",
            context,
        );

        assert_eq!(response.error, "Validation failed");
        assert_eq!(response.details, Some("Name is required".to_string()));
        assert_eq!(response.correlation_id(), Some("test-123"));
    }

    #[test]
    fn test_client_safe_response() {
        let context = ErrorContext::new()
            .with_correlation_id("test-123")
            .with_request_path("/api/users")
            .with_user_id("user-456")
            .with_metadata("sensitive", "data");

        let response = ContextualErrorResponse::new("Error occurred", context);
        let safe_response = response.client_safe();

        assert_eq!(safe_response.correlation_id(), Some("test-123"));
        assert_eq!(safe_response.context.request_path, None);
        assert_eq!(safe_response.context.user_id, None);
        assert!(safe_response.context.metadata.is_empty());
    }

    #[test]
    fn test_log_fields() {
        let context = ErrorContext::new()
            .with_correlation_id("test-123")
            .with_request_path("/api/users")
            .with_request_method("POST");

        let fields = context.to_log_fields();

        assert!(fields.iter().any(|(k, v)| k == &"correlation_id" && v == "test-123"));
        assert!(fields.iter().any(|(k, v)| k == &"request_path" && v == "/api/users"));
        assert!(fields.iter().any(|(k, v)| k == &"request_method" && v == "POST"));
    }
}
