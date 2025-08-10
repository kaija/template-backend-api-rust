use axum::{
    extract::Request,
    http::HeaderValue,
    middleware::Next,
    response::Response,
};

use crate::utils::http::{get_or_generate_correlation_id, create_correlation_header};

/// Middleware to generate and propagate correlation IDs
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    // Get or generate correlation ID
    let correlation_id = get_or_generate_correlation_id(request.headers());
    
    // Add correlation ID to request extensions for use in handlers
    request.extensions_mut().insert(correlation_id.clone());
    
    // Process the request
    let mut response = next.run(request).await;
    
    // Add correlation ID to response headers
    if let Ok(header_value) = create_correlation_header(&correlation_id) {
        response.headers_mut().insert("x-correlation-id", header_value);
    }
    
    response
}