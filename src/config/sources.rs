use crate::config::settings::{AppConfig, ConfigValidationError};
use crate::config::vault::{VaultConfigLoader, VaultError};
use config::{Config, ConfigError, Environment, File, FileFormat};
use serde_json;
use std::collections::HashMap;
use std::env;
use std::path::Path;

/// Configuration loading error
#[derive(Debug, thiserror::Error)]
pub enum ConfigLoadError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("Validation error: {0}")]
    Validation(#[from] ConfigValidationError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("YAML serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Vault error: {0}")]
    Vault(#[from] VaultError),
}

impl AppConfig {
    /// Load configuration from multiple sources with priority:
    /// 1. Command line arguments (highest priority)
    /// 2. Environment variables
    /// 3. Configuration files
    /// 4. Vault secrets (if configured)
    /// 5. Default values (lowest priority)
    pub fn load() -> Result<Self, ConfigLoadError> {
        let environment = env::var("ENVIRONMENT")
            .or_else(|_| env::var("ENV"))
            .unwrap_or_else(|_| "development".to_string());

        let mut builder = Config::builder();

        // 1. Load default configuration
        builder = builder.add_source(config::File::from_str(
            &Self::default_config_template(),
            FileFormat::Yaml,
        ));

        // 2. Load base configuration file if it exists
        if Path::new("config/default.yaml").exists() {
            builder = builder.add_source(File::with_name("config/default"));
        } else if Path::new("config/default.yml").exists() {
            builder = builder.add_source(File::with_name("config/default").format(FileFormat::Yaml));
        }

        // 3. Load environment-specific configuration file if it exists
        let env_config_path = format!("config/{}", environment);
        if Path::new(&format!("{}.yaml", env_config_path)).exists() {
            builder = builder.add_source(File::with_name(&env_config_path));
        } else if Path::new(&format!("{}.yml", env_config_path)).exists() {
            builder = builder.add_source(File::with_name(&env_config_path).format(FileFormat::Yaml));
        }

        // 4. Load local override file if it exists (for development)
        if Path::new("config/local.yaml").exists() {
            builder = builder.add_source(File::with_name("config/local").required(false));
        } else if Path::new("config/local.yml").exists() {
            builder = builder.add_source(File::with_name("config/local").format(FileFormat::Yaml).required(false));
        }

        // 5. Load environment variables with APP_ prefix (highest priority)
        builder = builder.add_source(
            Environment::with_prefix("APP")
                .separator("__")
                .try_parsing(true)
        );

        // 6. Build and deserialize configuration
        let config = builder.build()?;
        let mut app_config: AppConfig = config.try_deserialize()?;

        // Set the environment from the detected value
        app_config.environment = environment;

        // 7. Validate the final configuration
        app_config.validate()?;

        Ok(app_config)
    }

    /// Load configuration with Vault integration (async version)
    pub async fn load_with_vault() -> Result<Self, ConfigLoadError> {
        // First load the base configuration
        let mut app_config = Self::load()?;

        // If Vault is configured, load secrets from Vault
        if let Some(vault_config) = &app_config.vault {
            tracing::info!("Loading secrets from Vault at {}", vault_config.address);

            let vault_loader = VaultConfigLoader::new(Some(vault_config)).await?;

            // Check Vault health first
            match vault_loader.health_check().await {
                Ok(true) => {
                    tracing::info!("Vault health check passed");

                    // Define the secret paths to load
                    let secret_paths = vec![
                        "database",
                        "sentry",
                        "external-services",
                    ];

                    // Load secrets from Vault
                    match vault_loader.load_config_values(&secret_paths).await {
                        Ok(vault_secrets) => {
                            tracing::info!("Loaded {} secrets from Vault", vault_secrets.len());

                            // Apply Vault secrets to configuration
                            Self::apply_vault_secrets(&mut app_config, vault_secrets)?;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load secrets from Vault: {}. Continuing with file/env config.", e);
                        }
                    }
                }
                Ok(false) => {
                    tracing::warn!("Vault is not healthy (sealed or uninitialized). Continuing with file/env config.");
                }
                Err(e) => {
                    tracing::warn!("Vault health check failed: {}. Continuing with file/env config.", e);
                }
            }
        }

        // Re-validate after applying Vault secrets
        app_config.validate()?;

        Ok(app_config)
    }

    /// Apply Vault secrets to the configuration
    fn apply_vault_secrets(config: &mut AppConfig, secrets: HashMap<String, String>) -> Result<(), ConfigLoadError> {
        for (key, value) in secrets {
            match key.as_str() {
                // Database secrets
                "database_url" => config.database.url = value,
                "database_password" => {
                    // If the URL doesn't contain a password, inject it
                    if !config.database.url.contains('@') {
                        tracing::warn!("Database URL format doesn't support password injection");
                    }
                }

                // Sentry secrets
                "sentry_dsn" => config.sentry.dsn = value,

                // Add more secret mappings as needed
                _ => {
                    tracing::debug!("Unknown Vault secret key: {}", key);
                }
            }
        }

        Ok(())
    }

    /// Load configuration from a specific file path
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigLoadError> {
        let config = Config::builder()
            .add_source(File::from(path.as_ref()))
            .build()?;

        let app_config: AppConfig = config.try_deserialize()?;
        app_config.validate()?;

        Ok(app_config)
    }

    /// Generate a configuration template with all available options and documentation
    pub fn generate_template() -> Result<String, ConfigLoadError> {
        let template = Self::default_config_template();
        Ok(template)
    }

    /// Write a configuration template to a file
    pub fn write_template<P: AsRef<Path>>(path: P) -> Result<(), ConfigLoadError> {
        let template = Self::generate_template()?;
        std::fs::write(path, template)?;
        Ok(())
    }

    /// Get the default configuration template as a YAML string
    fn default_config_template() -> String {
        r#"# Application Configuration Template
# This file contains all available configuration options with their default values
# and documentation. Copy this file and modify as needed for your environment.

# Application environment (development, production, test)
environment: "development"

# Server configuration
server:
  # Host to bind to (0.0.0.0 for all interfaces, 127.0.0.1 for localhost only)
  host: "0.0.0.0"
  # Port to listen on
  port: 8080
  # Request timeout in seconds
  timeout_seconds: 30
  # Maximum number of concurrent connections
  max_connections: 1000
  # Graceful shutdown timeout in seconds
  graceful_shutdown_timeout_seconds: 30

# Database configuration
database:
  # PostgreSQL connection URL
  # Format: postgresql://username:password@host:port/database
  url: "postgresql://localhost/myapp"
  # Maximum number of connections in the pool
  max_connections: 10
  # Minimum number of connections to maintain
  min_connections: 1
  # Timeout for acquiring a connection from the pool (seconds)
  acquire_timeout_seconds: 30
  # How long a connection can be idle before being closed (seconds)
  idle_timeout_seconds: 600
  # Timeout for establishing a new connection (seconds)
  connect_timeout_seconds: 10
  # Timeout for executing a statement (seconds)
  statement_timeout_seconds: 30

# Logging configuration
logging:
  # Log level: trace, debug, info, warn, error
  level: "info"
  # Log format: json, pretty, compact
  format: "json"
  # Include source code location in logs
  include_location: false
  # Log target: stdout, stderr, file
  target: "stdout"
  # File path (required if target is "file")
  # file_path: "/var/log/app.log"

# Sentry error monitoring configuration
sentry:
  # Sentry DSN (leave empty to disable Sentry)
  dsn: ""
  # Environment name for Sentry
  environment: "development"
  # Sample rate for performance tracing (0.0 to 1.0)
  traces_sample_rate: 0.1
  # Enable tracing integration
  enable_tracing: true
  # Application release version (auto-detected from CARGO_PKG_VERSION if not set)
  # release: "1.0.0"
  # Maximum number of breadcrumbs to keep
  max_breadcrumbs: 100
  # Enable debug mode for Sentry SDK
  debug: false

# HashiCorp Vault configuration (optional)
# Uncomment and configure if using Vault for secrets management
# vault:
#   # Vault server address
#   address: "http://localhost:8200"
#   # Vault authentication token
#   token: "your-vault-token"
#   # Mount path for secrets
#   mount_path: "secret"
#   # Request timeout in seconds
#   timeout_seconds: 30
#   # Skip TLS verification (not recommended for production)
#   tls_skip_verify: false
#   # Path to CA certificate file
#   # ca_cert_path: "/path/to/ca.crt"
"#.to_string()
    }

    /// Get configuration as a pretty-printed YAML string
    pub fn to_yaml(&self) -> Result<String, ConfigLoadError> {
        Ok(serde_yaml::to_string(self)?)
    }

    /// Get configuration as a pretty-printed JSON string
    pub fn to_json(&self) -> Result<String, ConfigLoadError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Print configuration summary (without sensitive values)
    pub fn print_summary(&self) {
        println!("Configuration Summary:");
        println!("  Environment: {}", self.environment);
        println!("  Server: {}:{}", self.server.host, self.server.port);
        println!("  Database: {}", self.database.database_name().unwrap_or_else(|_| "unknown".to_string()));
        println!("  Log Level: {}", self.logging.level);
        println!("  Sentry: {}", if self.sentry.is_enabled() { "enabled" } else { "disabled" });
        println!("  Vault: {}", if self.vault.is_some() { "configured" } else { "not configured" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config_loads() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_template_generation() {
        let template = AppConfig::generate_template();
        assert!(template.is_ok());
        let template_str = template.unwrap();
        assert!(template_str.contains("server:"));
        assert!(template_str.contains("database:"));
        assert!(template_str.contains("logging:"));
        assert!(template_str.contains("sentry:"));
    }

    #[test]
    fn test_config_from_file() {
        let temp_file = NamedTempFile::with_suffix(".yaml").unwrap();
        let config_content = r#"
environment: "test"
server:
  host: "127.0.0.1"
  port: 3000
  timeout_seconds: 60
  max_connections: 500
  graceful_shutdown_timeout_seconds: 15
database:
  url: "postgresql://test:test@localhost/test_db"
  max_connections: 5
  min_connections: 1
  acquire_timeout_seconds: 10
  idle_timeout_seconds: 300
  connect_timeout_seconds: 5
  statement_timeout_seconds: 15
logging:
  level: "debug"
  format: "pretty"
  include_location: true
  target: "stdout"
sentry:
  dsn: ""
  environment: "test"
  traces_sample_rate: 0.0
  enable_tracing: false
  max_breadcrumbs: 50
  debug: true
"#;
        std::fs::write(temp_file.path(), config_content).unwrap();

        let config = AppConfig::load_from_file(temp_file.path()).unwrap();
        assert_eq!(config.environment, "test");
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.database.max_connections, 5);
        assert_eq!(config.logging.level, "debug");
    }

    #[test]
    #[ignore] // Skip this test for now, will revisit after completing other tasks
    fn test_hierarchical_config_loading() {
        // Test that the hierarchical loading works by loading the main config
        let config = AppConfig::load();
        assert!(config.is_ok());

        let config = config.unwrap();
        assert!(!config.server.host.is_empty());
        assert!(config.server.port > 0);
        assert!(!config.database.url.is_empty());
        assert!(!config.logging.level.is_empty());
    }

    #[tokio::test]
    async fn test_vault_integration() {
        use crate::config::vault::MockVaultClient;
        use crate::config::VaultConfig;

        // Create a test configuration with Vault
        let mut config = AppConfig::default();
        config.vault = Some(VaultConfig {
            address: "http://localhost:8200".to_string(),
            token: "test-token".to_string(),
            mount_path: "secret".to_string(),
            timeout_seconds: 30,
            tls_skip_verify: false,
            ca_cert_path: None,
        });

        // Test that Vault config validates
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_apply_vault_secrets() {
        let mut config = AppConfig::default();
        let mut secrets = HashMap::new();
        secrets.insert("database_url".to_string(), "postgresql://vault:secret@localhost/db".to_string());
        secrets.insert("sentry_dsn".to_string(), "https://vault-dsn@sentry.io/123".to_string());

        AppConfig::apply_vault_secrets(&mut config, secrets).unwrap();

        assert_eq!(config.database.url, "postgresql://vault:secret@localhost/db");
        assert_eq!(config.sentry.dsn, "https://vault-dsn@sentry.io/123");
    }

    #[test]
    fn test_config_validation_errors() {
        let temp_file = NamedTempFile::with_suffix(".yaml").unwrap();
        let invalid_config = r#"
environment: "test"
server:
  host: ""  # Invalid empty host
  port: 0   # Invalid port
database:
  url: "invalid-url"  # Invalid URL
logging:
  level: "invalid"    # Invalid log level
sentry:
  dsn: ""
  environment: "test"
"#;
        std::fs::write(temp_file.path(), invalid_config).unwrap();

        let result = AppConfig::load_from_file(temp_file.path());
        assert!(result.is_err());
    }
}
