use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Simple in-memory rate limiter
/// In production, consider using Redis or a more sophisticated solution
#[derive(Clone)]
pub struct RateLimiter {
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_seconds: u64) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window: Duration::from_secs(window_seconds),
        }
    }

    pub fn is_allowed(&self, key: &str) -> bool {
        let mut requests = self.requests.lock().unwrap();
        let now = Instant::now();
        
        // Get or create request history for this key
        let request_times = requests.entry(key.to_string()).or_insert_with(Vec::new);
        
        // Remove old requests outside the window
        request_times.retain(|&time| now.duration_since(time) < self.window);
        
        // Check if we're under the limit
        if request_times.len() < self.max_requests {
            request_times.push(now);
            true
        } else {
            false
        }
    }
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    rate_limiter: RateLimiter,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Use client IP as the rate limiting key
    // In production, you might want to use user ID or API key
    let client_ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|header| header.to_str().ok())
        .unwrap_or("unknown");

    if !rate_limiter.is_allowed(client_ip) {
        tracing::warn!("Rate limit exceeded for client: {}", client_ip);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    Ok(next.run(request).await)
}