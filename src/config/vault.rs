use crate::config::settings::{VaultConfig, ConfigValidationError};
use std::collections::HashMap;
#[cfg(feature = "vault")]
use std::time::Duration;

/// Vault client error
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Vault client error: {0}")]
    Client(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Secret not found: {0}")]
    NotFound(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigValidationError),
    #[error("Vault feature not enabled")]
    FeatureNotEnabled,
}

/// Vault client interface
#[async_trait::async_trait]
pub trait VaultClient: Send + Sync {
    /// Get a secret from Vault
    async fn get_secret(&self, path: &str) -> Result<HashMap<String, String>, VaultError>;
    
    /// Check if Vault is available and accessible
    async fn health_check(&self) -> Result<bool, VaultError>;
    
    /// Get multiple secrets at once
    async fn get_secrets(&self, paths: &[&str]) -> Result<HashMap<String, HashMap<String, String>>, VaultError>;
}

/// Mock Vault client for testing and when Vault is not available
#[derive(Debug, Clone)]
pub struct MockVaultClient {
    secrets: HashMap<String, HashMap<String, String>>,
}

impl MockVaultClient {
    pub fn new() -> Self {
        Self {
            secrets: HashMap::new(),
        }
    }
    
    pub fn with_secret(mut self, path: &str, key: &str, value: &str) -> Self {
        self.secrets
            .entry(path.to_string())
            .or_insert_with(HashMap::new)
            .insert(key.to_string(), value.to_string());
        self
    }
}

#[async_trait::async_trait]
impl VaultClient for MockVaultClient {
    async fn get_secret(&self, path: &str) -> Result<HashMap<String, String>, VaultError> {
        self.secrets
            .get(path)
            .cloned()
            .ok_or_else(|| VaultError::NotFound(path.to_string()))
    }
    
    async fn health_check(&self) -> Result<bool, VaultError> {
        Ok(true)
    }
    
    async fn get_secrets(&self, paths: &[&str]) -> Result<HashMap<String, HashMap<String, String>>, VaultError> {
        let mut result = HashMap::new();
        for path in paths {
            if let Some(secret) = self.secrets.get(*path) {
                result.insert(path.to_string(), secret.clone());
            }
        }
        Ok(result)
    }
}

/// Real Vault client implementation (only available with vault feature)
#[cfg(feature = "vault")]
pub struct HashiCorpVaultClient {
    client: vaultrs::client::VaultClient,
    mount_path: String,
}

#[cfg(feature = "vault")]
impl HashiCorpVaultClient {
    pub async fn new(config: &VaultConfig) -> Result<Self, VaultError> {
        config.validate()?;
        
        let mut client_builder = vaultrs::client::VaultClientSettingsBuilder::default();
        client_builder.address(&config.address);
        client_builder.token(&config.token);
        client_builder.timeout(Some(Duration::from_secs(config.timeout_seconds)));
        
        if config.tls_skip_verify {
            client_builder.verify(false);
        }
        
        if let Some(ca_cert_path) = &config.ca_cert_path {
            client_builder.ca_certs(vec![ca_cert_path.clone()]);
        }
        
        let client_settings = client_builder
            .build()
            .map_err(|e| VaultError::Config(ConfigValidationError::Vault(format!("Failed to build client settings: {}", e))))?;
        
        let client = vaultrs::client::VaultClient::new(client_settings)
            .map_err(|e| VaultError::Client(format!("Failed to create Vault client: {}", e)))?;
        
        Ok(Self {
            client,
            mount_path: config.mount_path.clone(),
        })
    }
}

#[cfg(feature = "vault")]
#[async_trait::async_trait]
impl VaultClient for HashiCorpVaultClient {
    async fn get_secret(&self, path: &str) -> Result<HashMap<String, String>, VaultError> {
        use vaultrs::kv2;
        
        let full_path = if path.starts_with('/') {
            path.trim_start_matches('/').to_string()
        } else {
            path.to_string()
        };
        
        let secret = kv2::read(&self.client, &self.mount_path, &full_path)
            .await
            .map_err(|e| match e {
                vaultrs::error::ClientError::APIError { code: 404, .. } => {
                    VaultError::NotFound(format!("Secret not found at path: {}", path))
                }
                vaultrs::error::ClientError::APIError { code: 403, .. } => {
                    VaultError::Auth("Access denied - check token permissions".to_string())
                }
                vaultrs::error::ClientError::APIError { code: 401, .. } => {
                    VaultError::Auth("Authentication failed - invalid token".to_string())
                }
                _ => VaultError::Client(format!("Failed to read secret: {}", e)),
            })?;
        
        // Convert the secret data to HashMap<String, String>
        let mut result = HashMap::new();
        if let Some(data) = secret.data {
            for (key, value) in data {
                // Convert serde_json::Value to String
                let string_value = match value {
                    serde_json::Value::String(s) => s,
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => serde_json::to_string(&value)
                        .map_err(|e| VaultError::Client(format!("Failed to serialize value: {}", e)))?,
                };
                result.insert(key, string_value);
            }
        }
        
        Ok(result)
    }
    
    async fn health_check(&self) -> Result<bool, VaultError> {
        use vaultrs::sys;
        
        match sys::health(&self.client).await {
            Ok(health) => Ok(health.initialized && !health.sealed),
            Err(e) => Err(VaultError::Network(format!("Health check failed: {}", e))),
        }
    }
    
    async fn get_secrets(&self, paths: &[&str]) -> Result<HashMap<String, HashMap<String, String>>, VaultError> {
        let mut result = HashMap::new();
        
        // Get secrets concurrently
        let futures: Vec<_> = paths
            .iter()
            .map(|path| async move {
                let secret = self.get_secret(path).await;
                (path.to_string(), secret)
            })
            .collect();
        
        let results = futures::future::join_all(futures).await;
        
        for (path, secret_result) in results {
            match secret_result {
                Ok(secret) => {
                    result.insert(path, secret);
                }
                Err(VaultError::NotFound(_)) => {
                    // Skip missing secrets, don't fail the entire operation
                    tracing::warn!("Secret not found at path: {}", path);
                }
                Err(e) => return Err(e),
            }
        }
        
        Ok(result)
    }
}

/// Vault client factory
pub struct VaultClientFactory;

impl VaultClientFactory {
    /// Create a Vault client based on configuration
    pub async fn create(config: Option<&VaultConfig>) -> Result<Box<dyn VaultClient>, VaultError> {
        match config {
            Some(_vault_config) => {
                #[cfg(feature = "vault")]
                {
                    let client = HashiCorpVaultClient::new(vault_config).await?;
                    Ok(Box::new(client))
                }
                #[cfg(not(feature = "vault"))]
                {
                    tracing::warn!("Vault configuration provided but vault feature is not enabled. Using mock client.");
                    Ok(Box::new(MockVaultClient::new()))
                }
            }
            None => {
                tracing::debug!("No Vault configuration provided. Using mock client.");
                Ok(Box::new(MockVaultClient::new()))
            }
        }
    }
    
    /// Create a mock client for testing
    pub fn create_mock() -> Box<dyn VaultClient> {
        Box::new(MockVaultClient::new())
    }
}

/// Vault integration for configuration loading
pub struct VaultConfigLoader {
    client: Box<dyn VaultClient>,
}

impl VaultConfigLoader {
    pub async fn new(config: Option<&VaultConfig>) -> Result<Self, VaultError> {
        let client = VaultClientFactory::create(config).await?;
        Ok(Self { client })
    }
    
    /// Load configuration values from Vault
    pub async fn load_config_values(&self, secret_paths: &[&str]) -> Result<HashMap<String, String>, VaultError> {
        let secrets = self.client.get_secrets(secret_paths).await?;
        
        let mut config_values = HashMap::new();
        for (path, secret) in secrets {
            for (key, value) in secret {
                // Create a flattened key like "database_password" from path "database" and key "password"
                let config_key = if path.contains('/') {
                    let path_parts: Vec<&str> = path.split('/').collect();
                    let last_part = path_parts.last().copied().unwrap_or(&path);
                    format!("{}_{}", last_part, key)
                } else {
                    format!("{}_{}", path, key)
                };
                config_values.insert(config_key, value);
            }
        }
        
        Ok(config_values)
    }
    
    /// Check if Vault is healthy and accessible
    pub async fn health_check(&self) -> Result<bool, VaultError> {
        self.client.health_check().await
    }
    
    /// Get a specific secret
    pub async fn get_secret(&self, path: &str) -> Result<HashMap<String, String>, VaultError> {
        self.client.get_secret(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mock_vault_client() {
        let client = MockVaultClient::new()
            .with_secret("database", "password", "secret123")
            .with_secret("database", "username", "admin")
            .with_secret("api", "key", "api-key-123");
        
        // Test getting a single secret
        let db_secret = client.get_secret("database").await.unwrap();
        assert_eq!(db_secret.get("password"), Some(&"secret123".to_string()));
        assert_eq!(db_secret.get("username"), Some(&"admin".to_string()));
        
        // Test getting multiple secrets
        let secrets = client.get_secrets(&["database", "api"]).await.unwrap();
        assert_eq!(secrets.len(), 2);
        assert!(secrets.contains_key("database"));
        assert!(secrets.contains_key("api"));
        
        // Test health check
        assert!(client.health_check().await.unwrap());
        
        // Test missing secret
        let result = client.get_secret("missing").await;
        assert!(matches!(result, Err(VaultError::NotFound(_))));
    }
    
    #[tokio::test]
    async fn test_vault_config_loader() {
        let client = MockVaultClient::new()
            .with_secret("database", "password", "secret123")
            .with_secret("database", "username", "admin")
            .with_secret("sentry", "dsn", "https://sentry.example.com/123");
        
        let loader = VaultConfigLoader {
            client: Box::new(client),
        };
        
        let config_values = loader
            .load_config_values(&["database", "sentry"])
            .await
            .unwrap();
        
        assert_eq!(config_values.get("database_password"), Some(&"secret123".to_string()));
        assert_eq!(config_values.get("database_username"), Some(&"admin".to_string()));
        assert_eq!(config_values.get("sentry_dsn"), Some(&"https://sentry.example.com/123".to_string()));
    }
    
    #[tokio::test]
    async fn test_vault_client_factory() {
        // Test with no config
        let client = VaultClientFactory::create(None).await.unwrap();
        assert!(client.health_check().await.unwrap());
        
        // Test mock client
        let mock_client = VaultClientFactory::create_mock();
        assert!(mock_client.health_check().await.unwrap());
    }
}