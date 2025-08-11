use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use tokio::time::sleep;
use tracing::{info, warn, error, instrument};

/// External service error types
#[derive(Debug, thiserror::Error)]
pub enum ExternalServiceError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("Timeout error")]
    Timeout,
    
    #[error("Service unavailable")]
    ServiceUnavailable,
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("Circuit breaker open")]
    CircuitBreakerOpen,
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Retry attempts exhausted")]
    RetryExhausted,
    
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// External service trait for making HTTP calls
#[async_trait]
pub trait ExternalService: Send + Sync {
    async fn get(&self, url: &str) -> Result<Value, ExternalServiceError>;
    async fn post(&self, url: &str, body: Value) -> Result<Value, ExternalServiceError>;
    async fn put(&self, url: &str, body: Value) -> Result<Value, ExternalServiceError>;
    async fn delete(&self, url: &str) -> Result<(), ExternalServiceError>;
}

/// Circuit breaker state
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker implementation
#[derive(Debug)]
struct CircuitBreaker {
    state: CircuitBreakerState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    failure_threshold: u32,
    timeout: Duration,
    half_open_max_calls: u32,
}

impl CircuitBreaker {
    fn new(failure_threshold: u32, timeout_seconds: u64) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            failure_threshold,
            timeout: Duration::from_secs(timeout_seconds),
            half_open_max_calls: 3,
        }
    }

    fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                if let Some(last_failure) = self.last_failure_time {
                    if last_failure.elapsed() >= self.timeout {
                        self.state = CircuitBreakerState::HalfOpen;
                        self.success_count = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => self.success_count < self.half_open_max_calls,
        }
    }

    fn record_success(&mut self) {
        match self.state {
            CircuitBreakerState::Closed => {
                self.failure_count = 0;
            }
            CircuitBreakerState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.half_open_max_calls {
                    self.state = CircuitBreakerState::Closed;
                    self.failure_count = 0;
                }
            }
            CircuitBreakerState::Open => {
                // Should not happen
            }
        }
    }

    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitBreakerState::Closed => {
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitBreakerState::Open;
                }
            }
            CircuitBreakerState::HalfOpen => {
                self.state = CircuitBreakerState::Open;
            }
            CircuitBreakerState::Open => {
                // Already open
            }
        }
    }
}

/// HTTP client configuration
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub circuit_breaker_enabled: bool,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout_seconds: u64,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 30,
            max_retries: 3,
            retry_delay_ms: 1000,
            circuit_breaker_enabled: true,
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout_seconds: 60,
        }
    }
}

/// HTTP client wrapper with timeout, retry logic, and circuit breaker
pub struct HttpExternalService {
    client: Client,
    config: HttpClientConfig,
    circuit_breaker: Arc<Mutex<CircuitBreaker>>,
}

impl HttpExternalService {
    pub fn new(timeout_seconds: u64) -> Self {
        let config = HttpClientConfig {
            timeout_seconds,
            ..Default::default()
        };
        Self::with_config(config)
    }

    pub fn with_config(config: HttpClientConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .user_agent("rust-api-microservice/1.0")
            .build()
            .expect("Failed to create HTTP client");

        let circuit_breaker = if config.circuit_breaker_enabled {
            Arc::new(Mutex::new(CircuitBreaker::new(
                config.circuit_breaker_threshold,
                config.circuit_breaker_timeout_seconds,
            )))
        } else {
            Arc::new(Mutex::new(CircuitBreaker::new(u32::MAX, u64::MAX))) // Effectively disabled
        };

        Self {
            client,
            config,
            circuit_breaker,
        }
    }

    /// Execute a request with retry logic and circuit breaker
    async fn execute_with_retry<F, Fut>(&self, operation: F) -> Result<Value, ExternalServiceError>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<Value, ExternalServiceError>> + Send,
    {
        // Check circuit breaker
        {
            let mut cb = self.circuit_breaker.lock().unwrap();
            if !cb.can_execute() {
                warn!("Circuit breaker is open, rejecting request");
                return Err(ExternalServiceError::CircuitBreakerOpen);
            }
        }

        let mut last_error = None;
        
        for attempt in 0..=self.config.max_retries {
            let start_time = Instant::now();
            
            match operation().await {
                Ok(response) => {
                    let duration = start_time.elapsed();
                    info!("External service call succeeded on attempt {} in {:?}", attempt + 1, duration);
                    
                    // Record success in circuit breaker
                    {
                        let mut cb = self.circuit_breaker.lock().unwrap();
                        cb.record_success();
                    }
                    
                    return Ok(response);
                }
                Err(e) => {
                    let duration = start_time.elapsed();
                    warn!("External service call failed on attempt {} after {:?}: {}", attempt + 1, duration, e);
                    
                    // Record failure in circuit breaker
                    {
                        let mut cb = self.circuit_breaker.lock().unwrap();
                        cb.record_failure();
                    }
                    
                    last_error = Some(e);
                    
                    // Don't retry on the last attempt
                    if attempt < self.config.max_retries {
                        let delay = Duration::from_millis(
                            self.config.retry_delay_ms * (2_u64.pow(attempt))
                        );
                        info!("Retrying in {:?} (attempt {} of {})", delay, attempt + 1, self.config.max_retries + 1);
                        sleep(delay).await;
                    }
                }
            }
        }

        error!("All retry attempts exhausted");
        Err(last_error.unwrap_or(ExternalServiceError::RetryExhausted))
    }
}

#[async_trait]
impl ExternalService for HttpExternalService {
    #[instrument(skip(self), fields(url = %url))]
    async fn get(&self, url: &str) -> Result<Value, ExternalServiceError> {
        info!("Making GET request to: {}", url);
        
        let url_clone = url.to_string();
        let client = self.client.clone();
        
        self.execute_with_retry(|| {
            let url = url_clone.clone();
            let client = client.clone();
            
            async move {
                let response = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            ExternalServiceError::Timeout
                        } else if e.is_connect() {
                            ExternalServiceError::ServiceUnavailable
                        } else {
                            ExternalServiceError::Http(e)
                        }
                    })?;

                let status = response.status();
                info!("GET request to {} returned status: {}", url, status);

                if status.is_success() {
                    let json = response.json::<Value>().await
                        .map_err(|e| ExternalServiceError::InvalidResponse(e.to_string()))?;
                    Ok(json)
                } else if status.is_server_error() {
                    Err(ExternalServiceError::ServiceUnavailable)
                } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    Err(ExternalServiceError::RateLimitExceeded)
                } else {
                    Err(ExternalServiceError::InvalidResponse(format!("HTTP {}", status)))
                }
            }
        }).await
    }

    #[instrument(skip(self, body), fields(url = %url))]
    async fn post(&self, url: &str, body: Value) -> Result<Value, ExternalServiceError> {
        info!("Making POST request to: {}", url);
        
        let url_clone = url.to_string();
        let client = self.client.clone();
        
        self.execute_with_retry(|| {
            let url = url_clone.clone();
            let client = client.clone();
            let body = body.clone();
            
            async move {
                let response = client
                    .post(&url)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            ExternalServiceError::Timeout
                        } else if e.is_connect() {
                            ExternalServiceError::ServiceUnavailable
                        } else {
                            ExternalServiceError::Http(e)
                        }
                    })?;

                let status = response.status();
                info!("POST request to {} returned status: {}", url, status);

                if status.is_success() {
                    let json = response.json::<Value>().await
                        .map_err(|e| ExternalServiceError::InvalidResponse(e.to_string()))?;
                    Ok(json)
                } else if status.is_server_error() {
                    Err(ExternalServiceError::ServiceUnavailable)
                } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    Err(ExternalServiceError::RateLimitExceeded)
                } else {
                    Err(ExternalServiceError::InvalidResponse(format!("HTTP {}", status)))
                }
            }
        }).await
    }

    #[instrument(skip(self, body), fields(url = %url))]
    async fn put(&self, url: &str, body: Value) -> Result<Value, ExternalServiceError> {
        info!("Making PUT request to: {}", url);
        
        let url_clone = url.to_string();
        let client = self.client.clone();
        
        self.execute_with_retry(|| {
            let url = url_clone.clone();
            let client = client.clone();
            let body = body.clone();
            
            async move {
                let response = client
                    .put(&url)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            ExternalServiceError::Timeout
                        } else if e.is_connect() {
                            ExternalServiceError::ServiceUnavailable
                        } else {
                            ExternalServiceError::Http(e)
                        }
                    })?;

                let status = response.status();
                info!("PUT request to {} returned status: {}", url, status);

                if status.is_success() {
                    let json = response.json::<Value>().await
                        .map_err(|e| ExternalServiceError::InvalidResponse(e.to_string()))?;
                    Ok(json)
                } else if status.is_server_error() {
                    Err(ExternalServiceError::ServiceUnavailable)
                } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    Err(ExternalServiceError::RateLimitExceeded)
                } else {
                    Err(ExternalServiceError::InvalidResponse(format!("HTTP {}", status)))
                }
            }
        }).await
    }

    #[instrument(skip(self), fields(url = %url))]
    async fn delete(&self, url: &str) -> Result<(), ExternalServiceError> {
        info!("Making DELETE request to: {}", url);
        
        let url_clone = url.to_string();
        let client = self.client.clone();
        
        let _result = self.execute_with_retry(|| {
            let url = url_clone.clone();
            let client = client.clone();
            
            async move {
                let response = client
                    .delete(&url)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            ExternalServiceError::Timeout
                        } else if e.is_connect() {
                            ExternalServiceError::ServiceUnavailable
                        } else {
                            ExternalServiceError::Http(e)
                        }
                    })?;

                let status = response.status();
                info!("DELETE request to {} returned status: {}", url, status);

                if status.is_success() {
                    Ok(serde_json::json!({})) // Return empty JSON for consistency
                } else if status.is_server_error() {
                    Err(ExternalServiceError::ServiceUnavailable)
                } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    Err(ExternalServiceError::RateLimitExceeded)
                } else {
                    Err(ExternalServiceError::InvalidResponse(format!("HTTP {}", status)))
                }
            }
        }).await?;

        Ok(())
    }
}
impl HttpExternalService {
    /// Get circuit breaker status for monitoring
    pub fn circuit_breaker_status(&self) -> CircuitBreakerStatus {
        let cb = self.circuit_breaker.lock().unwrap();
        CircuitBreakerStatus {
            state: cb.state.clone(),
            failure_count: cb.failure_count,
            success_count: cb.success_count,
            last_failure_time: cb.last_failure_time,
        }
    }

    /// Reset circuit breaker (for administrative purposes)
    pub fn reset_circuit_breaker(&self) {
        let mut cb = self.circuit_breaker.lock().unwrap();
        cb.state = CircuitBreakerState::Closed;
        cb.failure_count = 0;
        cb.success_count = 0;
        cb.last_failure_time = None;
        info!("Circuit breaker has been reset");
    }

    /// Make a custom HTTP request with full control
    pub async fn custom_request(
        &self,
        method: reqwest::Method,
        url: &str,
        headers: Option<reqwest::header::HeaderMap>,
        body: Option<Value>,
    ) -> Result<Value, ExternalServiceError> {
        info!("Making custom {} request to: {}", method, url);
        
        let url_clone = url.to_string();
        let client = self.client.clone();
        
        self.execute_with_retry(|| {
            let url = url_clone.clone();
            let client = client.clone();
            let method = method.clone();
            let headers = headers.clone();
            let body = body.clone();
            
            async move {
                let mut request = client.request(method.clone(), &url);
                
                if let Some(headers) = headers {
                    request = request.headers(headers);
                }
                
                if let Some(body) = body {
                    request = request.json(&body);
                }
                
                let response = request
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            ExternalServiceError::Timeout
                        } else if e.is_connect() {
                            ExternalServiceError::ServiceUnavailable
                        } else {
                            ExternalServiceError::Http(e)
                        }
                    })?;

                let status = response.status();
                info!("{} request to {} returned status: {}", method, url, status);

                if status.is_success() {
                    let json = response.json::<Value>().await
                        .map_err(|e| ExternalServiceError::InvalidResponse(e.to_string()))?;
                    Ok(json)
                } else if status.is_server_error() {
                    Err(ExternalServiceError::ServiceUnavailable)
                } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    Err(ExternalServiceError::RateLimitExceeded)
                } else {
                    Err(ExternalServiceError::InvalidResponse(format!("HTTP {}", status)))
                }
            }
        }).await
    }

    /// Health check for external service
    pub async fn health_check(&self, url: &str) -> Result<ExternalServiceHealthStatus, ExternalServiceError> {
        let start_time = Instant::now();
        
        match self.get(url).await {
            Ok(_) => {
                let response_time = start_time.elapsed();
                Ok(ExternalServiceHealthStatus {
                    is_healthy: true,
                    response_time,
                    circuit_breaker_state: self.circuit_breaker_status().state,
                    error_message: None,
                })
            }
            Err(e) => {
                let response_time = start_time.elapsed();
                Ok(ExternalServiceHealthStatus {
                    is_healthy: false,
                    response_time,
                    circuit_breaker_state: self.circuit_breaker_status().state,
                    error_message: Some(e.to_string()),
                })
            }
        }
    }
}

/// Circuit breaker status for monitoring
#[derive(Debug, Clone)]
pub struct CircuitBreakerStatus {
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub success_count: u32,
    pub last_failure_time: Option<Instant>,
}

/// External service health status
#[derive(Debug, Clone)]
pub struct ExternalServiceHealthStatus {
    pub is_healthy: bool,
    pub response_time: Duration,
    pub circuit_breaker_state: CircuitBreakerState,
    pub error_message: Option<String>,
}

/// Specialized external service implementations
pub struct WebhookService {
    http_service: HttpExternalService,
    base_url: String,
}

impl WebhookService {
    pub fn new(base_url: String, config: HttpClientConfig) -> Self {
        Self {
            http_service: HttpExternalService::with_config(config),
            base_url,
        }
    }

    /// Send webhook notification
    pub async fn send_notification(&self, endpoint: &str, payload: Value) -> Result<(), ExternalServiceError> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), endpoint.trim_start_matches('/'));
        
        info!("Sending webhook notification to: {}", url);
        
        let headers = {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("Content-Type", "application/json".parse().unwrap());
            headers.insert("User-Agent", "rust-api-microservice-webhook/1.0".parse().unwrap());
            Some(headers)
        };

        self.http_service
            .custom_request(reqwest::Method::POST, &url, headers, Some(payload))
            .await?;

        info!("Webhook notification sent successfully");
        Ok(())
    }
}

/// API client for external REST services
pub struct ApiClient {
    http_service: HttpExternalService,
    base_url: String,
    api_key: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: String, api_key: Option<String>, config: HttpClientConfig) -> Self {
        Self {
            http_service: HttpExternalService::with_config(config),
            base_url,
            api_key,
        }
    }

    /// Get authenticated headers
    fn get_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers.insert("Accept", "application/json".parse().unwrap());
        
        if let Some(api_key) = &self.api_key {
            headers.insert("Authorization", format!("Bearer {}", api_key).parse().unwrap());
        }
        
        headers
    }

    /// Make authenticated GET request
    pub async fn get(&self, endpoint: &str) -> Result<Value, ExternalServiceError> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), endpoint.trim_start_matches('/'));
        let headers = Some(self.get_headers());
        
        self.http_service
            .custom_request(reqwest::Method::GET, &url, headers, None)
            .await
    }

    /// Make authenticated POST request
    pub async fn post(&self, endpoint: &str, body: Value) -> Result<Value, ExternalServiceError> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), endpoint.trim_start_matches('/'));
        let headers = Some(self.get_headers());
        
        self.http_service
            .custom_request(reqwest::Method::POST, &url, headers, Some(body))
            .await
    }

    /// Make authenticated PUT request
    pub async fn put(&self, endpoint: &str, body: Value) -> Result<Value, ExternalServiceError> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), endpoint.trim_start_matches('/'));
        let headers = Some(self.get_headers());
        
        self.http_service
            .custom_request(reqwest::Method::PUT, &url, headers, Some(body))
            .await
    }

    /// Make authenticated DELETE request
    pub async fn delete(&self, endpoint: &str) -> Result<Value, ExternalServiceError> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), endpoint.trim_start_matches('/'));
        let headers = Some(self.get_headers());
        
        self.http_service
            .custom_request(reqwest::Method::DELETE, &url, headers, None)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_circuit_breaker_new() {
        let cb = CircuitBreaker::new(5, 60);
        assert_eq!(cb.state, CircuitBreakerState::Closed);
        assert_eq!(cb.failure_count, 0);
        assert_eq!(cb.failure_threshold, 5);
    }

    #[test]
    fn test_circuit_breaker_can_execute() {
        let mut cb = CircuitBreaker::new(2, 60);
        
        // Initially closed, should allow execution
        assert!(cb.can_execute());
        
        // Record failures to open circuit
        cb.record_failure();
        assert!(cb.can_execute()); // Still closed
        
        cb.record_failure();
        assert!(!cb.can_execute()); // Now open
    }

    #[test]
    fn test_circuit_breaker_success_recovery() {
        let mut cb = CircuitBreaker::new(2, 1); // 1 second timeout for testing
        
        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state, CircuitBreakerState::Open);
        
        // Wait for timeout (in real test, we'd need to wait)
        // For unit test, we'll manually set the time
        cb.last_failure_time = Some(Instant::now() - Duration::from_secs(2));
        
        // Should transition to half-open
        assert!(cb.can_execute());
        assert_eq!(cb.state, CircuitBreakerState::HalfOpen);
        
        // Record successes to close circuit
        cb.record_success();
        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state, CircuitBreakerState::Closed);
    }

    #[test]
    fn test_http_client_config_default() {
        let config = HttpClientConfig::default();
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 1000);
        assert!(config.circuit_breaker_enabled);
    }

    #[tokio::test]
    async fn test_external_service_creation() {
        let service = HttpExternalService::new(30);
        let status = service.circuit_breaker_status();
        assert_eq!(status.state, CircuitBreakerState::Closed);
        assert_eq!(status.failure_count, 0);
    }

    #[test]
    fn test_webhook_service_creation() {
        let config = HttpClientConfig::default();
        let webhook = WebhookService::new("https://api.example.com".to_string(), config);
        assert_eq!(webhook.base_url, "https://api.example.com");
    }

    #[test]
    fn test_api_client_creation() {
        let config = HttpClientConfig::default();
        let client = ApiClient::new(
            "https://api.example.com".to_string(),
            Some("test-api-key".to_string()),
            config,
        );
        assert_eq!(client.base_url, "https://api.example.com");
        assert_eq!(client.api_key, Some("test-api-key".to_string()));
    }

    #[test]
    fn test_api_client_headers() {
        let config = HttpClientConfig::default();
        let client = ApiClient::new(
            "https://api.example.com".to_string(),
            Some("test-key".to_string()),
            config,
        );
        
        let headers = client.get_headers();
        assert!(headers.contains_key("Content-Type"));
        assert!(headers.contains_key("Accept"));
        assert!(headers.contains_key("Authorization"));
    }
}