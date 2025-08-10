use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use url::Url;

/// Configuration validation error
#[derive(Debug, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("Invalid server configuration: {0}")]
    Server(String),
    #[error("Invalid database configuration: {0}")]
    Database(String),
    #[error("Invalid logging configuration: {0}")]
    Logging(String),
    #[error("Invalid Sentry configuration: {0}")]
    Sentry(String),
    #[error("Invalid Vault configuration: {0}")]
    Vault(String),
}

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
    pub sentry: SentryConfig,
    pub vault: Option<VaultConfig>,
    #[serde(default)]
    pub environment: String,
}

impl AppConfig {
    /// Validate the entire configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        self.server.validate()?;
        self.database.validate()?;
        self.logging.validate()?;
        self.sentry.validate()?;
        
        if let Some(vault) = &self.vault {
            vault.validate()?;
        }
        
        Ok(())
    }
    
    /// Check if running in development environment
    pub fn is_development(&self) -> bool {
        self.environment == "development" || self.environment == "dev"
    }
    
    /// Check if running in production environment
    pub fn is_production(&self) -> bool {
        self.environment == "production" || self.environment == "prod"
    }
    
    /// Check if running in test environment
    pub fn is_test(&self) -> bool {
        self.environment == "test" || self.environment == "testing"
    }
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub timeout_seconds: u64,
    pub max_connections: usize,
    #[serde(default = "default_graceful_shutdown_timeout")]
    pub graceful_shutdown_timeout_seconds: u64,
}

impl ServerConfig {
    /// Validate server configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        // Validate host is a valid IP address or hostname
        if self.host.is_empty() {
            return Err(ConfigValidationError::Server("Host cannot be empty".to_string()));
        }
        
        // Try to parse as IP address, if that fails it should be a valid hostname
        if self.host != "localhost" && IpAddr::from_str(&self.host).is_err() {
            // Basic hostname validation - should not contain invalid characters
            if self.host.contains(' ') || self.host.contains('\t') {
                return Err(ConfigValidationError::Server("Invalid host format".to_string()));
            }
        }
        
        // Validate port range
        if self.port == 0 {
            return Err(ConfigValidationError::Server("Port cannot be 0".to_string()));
        }
        
        // Validate timeout values
        if self.timeout_seconds == 0 {
            return Err(ConfigValidationError::Server("Timeout must be greater than 0".to_string()));
        }
        
        if self.graceful_shutdown_timeout_seconds == 0 {
            return Err(ConfigValidationError::Server("Graceful shutdown timeout must be greater than 0".to_string()));
        }
        
        // Validate max connections
        if self.max_connections == 0 {
            return Err(ConfigValidationError::Server("Max connections must be greater than 0".to_string()));
        }
        
        Ok(())
    }
    
    /// Get the socket address for binding
    pub fn socket_addr(&self) -> Result<SocketAddr, ConfigValidationError> {
        let ip = if self.host == "localhost" {
            IpAddr::from_str("127.0.0.1").unwrap()
        } else {
            IpAddr::from_str(&self.host)
                .map_err(|_| ConfigValidationError::Server(format!("Invalid IP address: {}", self.host)))?
        };
        
        Ok(SocketAddr::new(ip, self.port))
    }
}

fn default_graceful_shutdown_timeout() -> u64 {
    30
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_seconds: u64,
    #[serde(default = "default_statement_timeout")]
    pub statement_timeout_seconds: u64,
}

impl DatabaseConfig {
    /// Validate database configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        // Validate URL format
        if self.url.is_empty() {
            return Err(ConfigValidationError::Database("Database URL cannot be empty".to_string()));
        }
        
        // Parse URL to ensure it's valid
        Url::parse(&self.url)
            .map_err(|e| ConfigValidationError::Database(format!("Invalid database URL: {}", e)))?;
        
        // Validate connection pool settings
        if self.max_connections == 0 {
            return Err(ConfigValidationError::Database("Max connections must be greater than 0".to_string()));
        }
        
        if self.min_connections > self.max_connections {
            return Err(ConfigValidationError::Database("Min connections cannot be greater than max connections".to_string()));
        }
        
        // Validate timeout values
        if self.acquire_timeout_seconds == 0 {
            return Err(ConfigValidationError::Database("Acquire timeout must be greater than 0".to_string()));
        }
        
        if self.idle_timeout_seconds == 0 {
            return Err(ConfigValidationError::Database("Idle timeout must be greater than 0".to_string()));
        }
        
        if self.connect_timeout_seconds == 0 {
            return Err(ConfigValidationError::Database("Connect timeout must be greater than 0".to_string()));
        }
        
        if self.statement_timeout_seconds == 0 {
            return Err(ConfigValidationError::Database("Statement timeout must be greater than 0".to_string()));
        }
        
        Ok(())
    }
    
    /// Get database name from URL
    pub fn database_name(&self) -> Result<String, ConfigValidationError> {
        let url = Url::parse(&self.url)
            .map_err(|e| ConfigValidationError::Database(format!("Invalid database URL: {}", e)))?;
        
        let path = url.path().trim_start_matches('/');
        if path.is_empty() {
            return Err(ConfigValidationError::Database("Database name not found in URL".to_string()));
        }
        
        Ok(path.to_string())
    }
}

fn default_connect_timeout() -> u64 {
    10
}

fn default_statement_timeout() -> u64 {
    30
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub include_location: bool,
    #[serde(default = "default_log_target")]
    pub target: String,
    #[serde(default)]
    pub file_path: Option<String>,
}

impl LoggingConfig {
    /// Validate logging configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.level.to_lowercase().as_str()) {
            return Err(ConfigValidationError::Logging(
                format!("Invalid log level '{}'. Valid levels: {}", self.level, valid_levels.join(", "))
            ));
        }
        
        // Validate log format
        let valid_formats = ["json", "pretty", "compact"];
        if !valid_formats.contains(&self.format.to_lowercase().as_str()) {
            return Err(ConfigValidationError::Logging(
                format!("Invalid log format '{}'. Valid formats: {}", self.format, valid_formats.join(", "))
            ));
        }
        
        // Validate target
        let valid_targets = ["stdout", "stderr", "file"];
        if !valid_targets.contains(&self.target.to_lowercase().as_str()) {
            return Err(ConfigValidationError::Logging(
                format!("Invalid log target '{}'. Valid targets: {}", self.target, valid_targets.join(", "))
            ));
        }
        
        // If target is file, file_path must be provided
        if self.target.to_lowercase() == "file" && self.file_path.is_none() {
            return Err(ConfigValidationError::Logging(
                "File path must be provided when target is 'file'".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Get the tracing level filter
    pub fn tracing_level(&self) -> tracing::Level {
        match self.level.to_lowercase().as_str() {
            "trace" => tracing::Level::TRACE,
            "debug" => tracing::Level::DEBUG,
            "info" => tracing::Level::INFO,
            "warn" => tracing::Level::WARN,
            "error" => tracing::Level::ERROR,
            _ => tracing::Level::INFO, // Default fallback
        }
    }
}

fn default_log_target() -> String {
    "stdout".to_string()
}

/// Sentry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryConfig {
    pub dsn: String,
    pub environment: String,
    pub traces_sample_rate: f32,
    pub enable_tracing: bool,
    #[serde(default = "default_release")]
    pub release: Option<String>,
    #[serde(default = "default_max_breadcrumbs")]
    pub max_breadcrumbs: usize,
    #[serde(default)]
    pub debug: bool,
}

impl SentryConfig {
    /// Validate Sentry configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        // DSN can be empty (disables Sentry), but if provided must be valid
        if !self.dsn.is_empty() {
            // Basic DSN format validation
            if !self.dsn.starts_with("https://") && !self.dsn.starts_with("http://") {
                return Err(ConfigValidationError::Sentry(
                    "DSN must be a valid URL starting with http:// or https://".to_string()
                ));
            }
        }
        
        // Validate environment name
        if self.environment.is_empty() {
            return Err(ConfigValidationError::Sentry("Environment cannot be empty".to_string()));
        }
        
        // Validate sample rate
        if self.traces_sample_rate < 0.0 || self.traces_sample_rate > 1.0 {
            return Err(ConfigValidationError::Sentry(
                "Traces sample rate must be between 0.0 and 1.0".to_string()
            ));
        }
        
        // Validate max breadcrumbs
        if self.max_breadcrumbs == 0 {
            return Err(ConfigValidationError::Sentry("Max breadcrumbs must be greater than 0".to_string()));
        }
        
        Ok(())
    }
    
    /// Check if Sentry is enabled (has a valid DSN)
    pub fn is_enabled(&self) -> bool {
        !self.dsn.is_empty()
    }
}

fn default_release() -> Option<String> {
    std::env::var("CARGO_PKG_VERSION").ok()
}

fn default_max_breadcrumbs() -> usize {
    100
}

/// Vault configuration (optional)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub address: String,
    pub token: String,
    pub mount_path: String,
    #[serde(default = "default_vault_timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub tls_skip_verify: bool,
    #[serde(default)]
    pub ca_cert_path: Option<String>,
}

impl VaultConfig {
    /// Validate Vault configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        // Validate address
        if self.address.is_empty() {
            return Err(ConfigValidationError::Vault("Address cannot be empty".to_string()));
        }
        
        // Parse address to ensure it's a valid URL
        Url::parse(&self.address)
            .map_err(|e| ConfigValidationError::Vault(format!("Invalid address URL: {}", e)))?;
        
        // Validate token
        if self.token.is_empty() {
            return Err(ConfigValidationError::Vault("Token cannot be empty".to_string()));
        }
        
        // Validate mount path
        if self.mount_path.is_empty() {
            return Err(ConfigValidationError::Vault("Mount path cannot be empty".to_string()));
        }
        
        // Validate timeout
        if self.timeout_seconds == 0 {
            return Err(ConfigValidationError::Vault("Timeout must be greater than 0".to_string()));
        }
        
        // If CA cert path is provided, it should exist (in production)
        if let Some(ca_path) = &self.ca_cert_path {
            if ca_path.is_empty() {
                return Err(ConfigValidationError::Vault("CA cert path cannot be empty if provided".to_string()));
            }
        }
        
        Ok(())
    }
}

fn default_vault_timeout() -> u64 {
    30
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            timeout_seconds: 30,
            max_connections: 1000,
            graceful_shutdown_timeout_seconds: default_graceful_shutdown_timeout(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost/myapp".to_string(),
            max_connections: 10,
            min_connections: 1,
            acquire_timeout_seconds: 30,
            idle_timeout_seconds: 600,
            connect_timeout_seconds: default_connect_timeout(),
            statement_timeout_seconds: default_statement_timeout(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
            include_location: false,
            target: default_log_target(),
            file_path: None,
        }
    }
}

impl Default for SentryConfig {
    fn default() -> Self {
        Self {
            dsn: "".to_string(),
            environment: "development".to_string(),
            traces_sample_rate: 0.1,
            enable_tracing: true,
            release: default_release(),
            max_breadcrumbs: default_max_breadcrumbs(),
            debug: false,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            logging: LoggingConfig::default(),
            sentry: SentryConfig::default(),
            vault: None,
            environment: "development".to_string(),
        }
    }
}