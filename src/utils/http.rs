use axum::http::{HeaderMap, HeaderValue};
use uuid::Uuid;

/// Generate a correlation ID for request tracing
pub fn generate_correlation_id() -> String {
    Uuid::new_v4().to_string()
}

/// Extract correlation ID from headers or generate a new one
pub fn get_or_generate_correlation_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(generate_correlation_id)
}

/// Create correlation ID header
pub fn create_correlation_header(correlation_id: &str) -> Result<HeaderValue, axum::http::header::InvalidHeaderValue> {
    HeaderValue::from_str(correlation_id)
}

/// Extract client IP from headers and connection info
pub fn extract_client_ip(headers: &HeaderMap, remote_addr: Option<std::net::SocketAddr>) -> String {
    // Check for forwarded headers first
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(first_ip) = forwarded_str.split(',').next() {
                return first_ip.trim().to_string();
            }
        }
    }
    
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            return ip_str.to_string();
        }
    }
    
    // Fall back to remote address
    remote_addr
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Create standard CORS headers
pub fn create_cors_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
    headers.insert("access-control-allow-methods", HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"));
    headers.insert("access-control-allow-headers", HeaderValue::from_static("content-type, authorization, x-correlation-id"));
    headers
}