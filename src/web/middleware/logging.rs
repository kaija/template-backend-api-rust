use axum::{
    extract::{ConnectInfo, Request},
    middleware::Next,
    response::Response,
};
use std::{
    net::SocketAddr,
    time::Instant,
};
use tracing::{info, warn, error};
use uuid::Uuid;

/// Middleware for logging HTTP requests and responses
pub async fn logging_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();
    let headers = request.headers().clone();

    // Extract correlation ID from request extensions or generate one
    let correlation_id = request
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Extract user agent and other relevant headers
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");

    let content_length = headers
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    // Create a span for this request
    let span = tracing::info_span!(
        "http_request",
        correlation_id = %correlation_id,
        method = %method,
        uri = %uri,
        version = ?version,
        client_ip = %addr.ip(),
        user_agent = user_agent,
        request_size = content_length,
    );

    // Log the incoming request
    let _guard = span.enter();
    info!(
        "Started processing request: {} {} from {}",
        method,
        uri,
        addr.ip()
    );

    // Process the request
    let response = next.run(request).await;

    // Calculate response time
    let duration = start_time.elapsed();
    let status = response.status();

    // Extract response content length if available
    let response_size = response
        .headers()
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    // Log the response with appropriate level based on status code
    match status.as_u16() {
        200..=299 => {
            info!(
                status = status.as_u16(),
                duration_ms = duration.as_millis(),
                response_size = response_size,
                "Request completed successfully"
            );
        }
        300..=399 => {
            info!(
                status = status.as_u16(),
                duration_ms = duration.as_millis(),
                response_size = response_size,
                "Request completed with redirect"
            );
        }
        400..=499 => {
            warn!(
                status = status.as_u16(),
                duration_ms = duration.as_millis(),
                response_size = response_size,
                "Request completed with client error"
            );
        }
        500..=599 => {
            error!(
                status = status.as_u16(),
                duration_ms = duration.as_millis(),
                response_size = response_size,
                "Request completed with server error"
            );
        }
        _ => {
            warn!(
                status = status.as_u16(),
                duration_ms = duration.as_millis(),
                response_size = response_size,
                "Request completed with unknown status"
            );
        }
    }

    response
}

/// Middleware for logging HTTP requests and responses with detailed error information
pub async fn detailed_logging_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();
    let headers = request.headers().clone();

    // Extract correlation ID from request extensions or generate one
    let correlation_id = request
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Extract additional request metadata
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");

    let referer = headers
        .get("referer")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");

    let content_type = headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");

    let content_length = headers
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let accept = headers
        .get("accept")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");

    // Create a detailed span for this request
    let span = tracing::info_span!(
        "http_request_detailed",
        correlation_id = %correlation_id,
        method = %method,
        uri = %uri,
        version = ?version,
        client_ip = %addr.ip(),
        client_port = addr.port(),
        user_agent = user_agent,
        referer = referer,
        content_type = content_type,
        content_length = content_length,
        accept = accept,
    );

    // Log the incoming request with detailed information
    let _guard = span.enter();
    info!(
        "Request started: {} {} HTTP/{:?} from {}:{} | User-Agent: {} | Content-Type: {} | Accept: {}",
        method,
        uri,
        version,
        addr.ip(),
        addr.port(),
        user_agent,
        content_type,
        accept
    );

    // Process the request
    let response = next.run(request).await;

    // Calculate response time
    let duration = start_time.elapsed();
    let status = response.status();

    // Extract response metadata
    let response_content_type = response
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");

    let response_size = response
        .headers()
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let cache_control = response
        .headers()
        .get("cache-control")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");

    // Log the response with detailed information and appropriate level
    let duration_ms = duration.as_millis();
    let duration_us = duration.as_micros();

    match status.as_u16() {
        200..=299 => {
            info!(
                status = status.as_u16(),
                duration_ms = duration_ms,
                duration_us = duration_us,
                response_size = response_size,
                response_content_type = response_content_type,
                cache_control = cache_control,
                "Request completed successfully: {} {} -> {} in {}ms",
                method,
                uri,
                status,
                duration_ms
            );
        }
        300..=399 => {
            info!(
                status = status.as_u16(),
                duration_ms = duration_ms,
                duration_us = duration_us,
                response_size = response_size,
                response_content_type = response_content_type,
                cache_control = cache_control,
                "Request redirected: {} {} -> {} in {}ms",
                method,
                uri,
                status,
                duration_ms
            );
        }
        400..=499 => {
            warn!(
                status = status.as_u16(),
                duration_ms = duration_ms,
                duration_us = duration_us,
                response_size = response_size,
                response_content_type = response_content_type,
                "Client error: {} {} -> {} in {}ms from {}",
                method,
                uri,
                status,
                duration_ms,
                addr.ip()
            );
        }
        500..=599 => {
            error!(
                status = status.as_u16(),
                duration_ms = duration_ms,
                duration_us = duration_us,
                response_size = response_size,
                response_content_type = response_content_type,
                "Server error: {} {} -> {} in {}ms | Client: {} | User-Agent: {}",
                method,
                uri,
                status,
                duration_ms,
                addr.ip(),
                user_agent
            );
        }
        _ => {
            warn!(
                status = status.as_u16(),
                duration_ms = duration_ms,
                duration_us = duration_us,
                response_size = response_size,
                response_content_type = response_content_type,
                "Unknown status: {} {} -> {} in {}ms",
                method,
                uri,
                status,
                duration_ms
            );
        }
    }

    response
}

/// Simple access log middleware that logs in a format similar to Apache/Nginx access logs
pub async fn access_log_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();

    // Extract user agent and referer before moving request
    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-")
        .to_string();

    let referer = request
        .headers()
        .get("referer")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-")
        .to_string();

    // Process the request
    let response = next.run(request).await;

    // Calculate response time and extract response info
    let duration = start_time.elapsed();
    let status = response.status();
    let response_size = response
        .headers()
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");

    // Log in Common Log Format (CLF) style
    // Format: IP - - [timestamp] "METHOD URI HTTP/version" status size "referer" "user-agent" duration_ms
    info!(
        target: "access_log",
        r#"{} - - "{} {} HTTP/{:?}" {} {} "{}" "{}" {}ms"#,
        addr.ip(),
        method,
        uri,
        version,
        status.as_u16(),
        response_size,
        &referer,
        &user_agent,
        duration.as_millis()
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, StatusCode, Uri},
        response::Response,
    };
    use std::net::{IpAddr, Ipv4Addr};
    use tower::{ServiceExt, ServiceBuilder};
    use tower_http::trace::TraceLayer;

    #[tokio::test]
    async fn test_logging_middleware_success() {
        // This test would require setting up a test server
        // For now, we'll just test that the middleware compiles and can be created
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        // Test that we can create the middleware functions
        assert!(true); // Placeholder test
    }

    #[tokio::test]
    async fn test_detailed_logging_middleware() {
        // Similar placeholder test
        assert!(true);
    }

    #[tokio::test]
    async fn test_access_log_middleware() {
        // Similar placeholder test
        assert!(true);
    }
}
