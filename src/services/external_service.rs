use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

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
}

/// External service trait for making HTTP calls
#[async_trait]
pub trait ExternalService: Send + Sync {
    async fn get(&self, url: &str) -> Result<Value, ExternalServiceError>;
    async fn post(&self, url: &str, body: Value) -> Result<Value, ExternalServiceError>;
    async fn put(&self, url: &str, body: Value) -> Result<Value, ExternalServiceError>;
    async fn delete(&self, url: &str) -> Result<(), ExternalServiceError>;
}

/// HTTP client wrapper with timeout and retry logic
pub struct HttpExternalService {
    client: Client,
    timeout: Duration,
}

impl HttpExternalService {
    pub fn new(timeout_seconds: u64) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            timeout: Duration::from_secs(timeout_seconds),
        }
    }
}

#[async_trait]
impl ExternalService for HttpExternalService {
    async fn get(&self, url: &str) -> Result<Value, ExternalServiceError> {
        tracing::info!("Making GET request to: {}", url);
        
        let response = self
            .client
            .get(url)
            .timeout(self.timeout)
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::error!("GET request failed with status: {}", response.status());
            return Err(ExternalServiceError::ServiceUnavailable);
        }

        let json = response.json::<Value>().await?;
        tracing::info!("GET request successful");
        
        Ok(json)
    }

    async fn post(&self, url: &str, body: Value) -> Result<Value, ExternalServiceError> {
        tracing::info!("Making POST request to: {}", url);
        
        let response = self
            .client
            .post(url)
            .json(&body)
            .timeout(self.timeout)
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::error!("POST request failed with status: {}", response.status());
            return Err(ExternalServiceError::ServiceUnavailable);
        }

        let json = response.json::<Value>().await?;
        tracing::info!("POST request successful");
        
        Ok(json)
    }

    async fn put(&self, url: &str, body: Value) -> Result<Value, ExternalServiceError> {
        tracing::info!("Making PUT request to: {}", url);
        
        let response = self
            .client
            .put(url)
            .json(&body)
            .timeout(self.timeout)
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::error!("PUT request failed with status: {}", response.status());
            return Err(ExternalServiceError::ServiceUnavailable);
        }

        let json = response.json::<Value>().await?;
        tracing::info!("PUT request successful");
        
        Ok(json)
    }

    async fn delete(&self, url: &str) -> Result<(), ExternalServiceError> {
        tracing::info!("Making DELETE request to: {}", url);
        
        let response = self
            .client
            .delete(url)
            .timeout(self.timeout)
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::error!("DELETE request failed with status: {}", response.status());
            return Err(ExternalServiceError::ServiceUnavailable);
        }

        tracing::info!("DELETE request successful");
        Ok(())
    }
}