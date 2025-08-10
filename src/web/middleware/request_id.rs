use axum::{
    extract::Request,
    http::HeaderValue,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

/// Middleware to generate and propagate correlation IDs
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    // Get or generate correlation ID
    let correlation_id = get_or_generate_correlation_id(request.headers());
    
    // Add correlation ID to request extensions for use in handlers and other middleware
    request.extensions_mut().insert(correlation_id.clone());
    
    // Add correlation ID to tracing span
    let span = tracing::Span::current();
    span.record("correlation_id", &correlation_id);
    
    // Process the request
    let mut response = next.run(request).await;
    
    // Add correlation ID to response headers
    if let Ok(header_value) = HeaderValue::from_str(&correlation_id) {
        response.headers_mut().insert("x-correlation-id", header_value.clone());
        response.headers_mut().insert("x-request-id", header_value);
    }
    
    response
}

/// Get existing correlation ID from headers or generate a new one
fn get_or_generate_correlation_id(headers: &axum::http::HeaderMap) -> String {
    // Check for existing correlation ID in various header formats
    let correlation_id = headers
        .get("x-correlation-id")
        .or_else(|| headers.get("x-request-id"))
        .or_else(|| headers.get("request-id"))
        .or_else(|| headers.get("correlation-id"))
        .and_then(|header| header.to_str().ok())
        .filter(|id| !id.is_empty() && is_valid_correlation_id(id));
    
    match correlation_id {
        Some(id) => {
            tracing::debug!("Using existing correlation ID: {}", id);
            id.to_string()
        }
        None => {
            let new_id = Uuid::new_v4().to_string();
            tracing::debug!("Generated new correlation ID: {}", new_id);
            new_id
        }
    }
}

/// Validate correlation ID format
fn is_valid_correlation_id(id: &str) -> bool {
    // Check if it's a valid UUID format
    if Uuid::parse_str(id).is_ok() {
        return true;
    }
    
    // Allow alphanumeric strings with hyphens and underscores (reasonable length)
    id.len() <= 64 
        && id.len() >= 8 
        && id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn test_valid_correlation_id() {
        assert!(is_valid_correlation_id("550e8400-e29b-41d4-a716-446655440000"));
        assert!(is_valid_correlation_id("abc123-def456"));
        assert!(is_valid_correlation_id("request_12345"));
    }

    #[test]
    fn test_invalid_correlation_id() {
        assert!(!is_valid_correlation_id(""));
        assert!(!is_valid_correlation_id("a")); // Too short
        assert!(!is_valid_correlation_id("a".repeat(65).as_str())); // Too long
        assert!(!is_valid_correlation_id("invalid@id")); // Invalid character
        assert!(!is_valid_correlation_id("invalid id")); // Space
    }

    #[test]
    fn test_get_existing_correlation_id() {
        let mut headers = HeaderMap::new();
        headers.insert("x-correlation-id", "test-123".parse().unwrap());
        
        let id = get_or_generate_correlation_id(&headers);
        assert_eq!(id, "test-123");
    }

    #[test]
    fn test_generate_new_correlation_id() {
        let headers = HeaderMap::new();
        let id = get_or_generate_correlation_id(&headers);
        
        // Should be a valid UUID
        assert!(Uuid::parse_str(&id).is_ok());
    }
}