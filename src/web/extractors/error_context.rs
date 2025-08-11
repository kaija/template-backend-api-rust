use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};

use crate::web::responses::{ErrorContext, RequestContextExtractor};

/// Axum extractor for error context
#[derive(Debug, Clone)]
pub struct ExtractedErrorContext(pub ErrorContext);

#[async_trait]
impl<S> FromRequestParts<S> for ExtractedErrorContext
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract correlation ID from extensions (set by request_id_middleware)
        let correlation_id = parts.extensions.get::<String>().cloned();

        // Extract request information
        let path = Some(parts.uri.path().to_string());
        let method = Some(parts.method.to_string());

        // Build error context
        let context = RequestContextExtractor::new()
            .with_correlation_id(correlation_id)
            .with_path(path)
            .with_method(method)
            .build();

        Ok(ExtractedErrorContext(context))
    }
}

impl std::ops::Deref for ExtractedErrorContext {
    type Target = ErrorContext;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<ExtractedErrorContext> for ErrorContext {
    fn from(extracted: ExtractedErrorContext) -> Self {
        extracted.0
    }
}

/// Helper macro for creating contextual errors in handlers
#[macro_export]
macro_rules! contextual_error {
    ($error:expr, $context:expr) => {
        $crate::web::responses::ContextualAppError::new($error, $context.clone())
    };
    ($error:expr, $context:expr, $($key:expr => $value:expr),+) => {{
        let mut contextual_error = $crate::web::responses::ContextualAppError::new($error, $context.clone());
        $(
            contextual_error = contextual_error.with_metadata($key, $value);
        )+
        contextual_error
    }};
}

/// Helper function to create a contextual error result
pub fn contextual_error_result<T>(
    error: crate::web::responses::AppError,
    context: ErrorContext,
) -> Result<T, crate::web::responses::ContextualAppError> {
    Err(crate::web::responses::ContextualAppError::new(error, context))
}

/// Helper function to create a contextual error result with metadata
pub fn contextual_error_with_metadata<T>(
    error: crate::web::responses::AppError,
    context: ErrorContext,
    metadata: std::collections::HashMap<String, String>,
) -> Result<T, crate::web::responses::ContextualAppError> {
    let contextual_error = crate::web::responses::ContextualAppError::new(error, context)
        .with_metadata_map(metadata);
    Err(contextual_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Method, Uri};
    use std::str::FromStr;

    #[tokio::test]
    async fn test_error_context_extraction() {
        let mut parts = Parts {
            method: Method::POST,
            uri: Uri::from_str("/api/users").unwrap(),
            version: axum::http::Version::HTTP_11,
            headers: axum::http::HeaderMap::new(),
            extensions: axum::http::Extensions::new(),
        };

        // Simulate correlation ID set by middleware
        parts.extensions.insert("test-correlation-123".to_string());

        let extracted = ExtractedErrorContext::from_request_parts(&mut parts, &())
            .await
            .unwrap();

        assert_eq!(extracted.correlation_id(), Some("test-correlation-123"));
        assert_eq!(extracted.request_path, Some("/api/users".to_string()));
        assert_eq!(extracted.request_method, Some("POST".to_string()));
    }
}
