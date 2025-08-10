use crate::config::settings::{AppConfig, LoggingConfig, SentryConfig};
use anyhow::Result;
use std::io;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};
use uuid::Uuid;

/// Correlation ID for request tracing
#[derive(Debug, Clone)]
pub struct CorrelationId(String);

impl CorrelationId {
    /// Generate a new correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from existing string
    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the correlation ID as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Initialize tracing subscriber with multiple layers
pub fn init_tracing(config: &AppConfig) -> Result<Option<WorkerGuard>> {
    let logging_config = &config.logging;
    let sentry_config = &config.sentry;
    
    // Initialize Sentry first if configured
    let _sentry_guard = init_sentry(sentry_config)?;
    
    // Create environment filter based on configuration
    let env_filter = create_env_filter(logging_config)?;
    
    // Initialize tracing - Sentry will capture errors through its global integration
    let guard = match logging_config.target.to_lowercase().as_str() {
        "stdout" => {
            init_stdout_tracing(logging_config, env_filter)?;
            None
        }
        "stderr" => {
            init_stderr_tracing(logging_config, env_filter)?;
            None
        }
        "file" => {
            let guard = init_file_tracing(logging_config, env_filter)?;
            Some(guard)
        }
        _ => {
            tracing::warn!("Unknown log target '{}', falling back to stdout", logging_config.target);
            init_stdout_tracing(logging_config, env_filter)?;
            None
        }
    };
    
    tracing::info!(
        "Tracing initialized with level: {}, format: {}, target: {}, sentry_enabled: {}",
        logging_config.level,
        logging_config.format,
        logging_config.target,
        sentry_config.is_enabled()
    );
    
    Ok(guard)
}

/// Initialize Sentry SDK with configuration
fn init_sentry(config: &SentryConfig) -> Result<Option<sentry::ClientInitGuard>> {
    if !config.is_enabled() {
        tracing::debug!("Sentry is disabled (no DSN provided)");
        return Ok(None);
    }

    let guard = sentry::init(sentry::ClientOptions {
        dsn: Some(config.dsn.parse()?),
        environment: Some(config.environment.clone().into()),
        release: config.release.clone().map(Into::into),
        traces_sample_rate: config.traces_sample_rate,
        max_breadcrumbs: config.max_breadcrumbs,
        debug: config.debug,
        ..Default::default()
    });

    // Set up Sentry context
    sentry::configure_scope(|scope| {
        scope.set_tag("service", "rust-api-microservice-template");
        scope.set_tag("version", env!("CARGO_PKG_VERSION"));
    });

    tracing::info!(
        "Sentry initialized with DSN: {}, environment: {}, traces_sample_rate: {}",
        mask_dsn(&config.dsn),
        config.environment,
        config.traces_sample_rate
    );

    Ok(Some(guard))
}

/// Mask sensitive parts of DSN for logging
fn mask_dsn(dsn: &str) -> String {
    if let Ok(parsed) = dsn.parse::<url::Url>() {
        format!("{}://***@{}", parsed.scheme(), parsed.host_str().unwrap_or("unknown"))
    } else {
        "***".to_string()
    }
}

/// Create Sentry tracing layer
fn create_sentry_layer() -> sentry::integrations::tracing::SentryLayer<tracing_subscriber::Registry> {
    sentry::integrations::tracing::layer()
        .event_filter(|md| match *md.level() {
            // Capture error level events as Sentry events
            // These are grouped into issues, representing high-severity errors to act upon
            tracing::Level::ERROR => sentry::integrations::tracing::EventFilter::Event,
            // Capture warn and info as breadcrumbs for context
            tracing::Level::WARN => sentry::integrations::tracing::EventFilter::Breadcrumb,
            tracing::Level::INFO => sentry::integrations::tracing::EventFilter::Breadcrumb,
            // Ignore trace level events, as they're too verbose
            tracing::Level::TRACE => sentry::integrations::tracing::EventFilter::Ignore,
            // Capture debug as breadcrumbs
            tracing::Level::DEBUG => sentry::integrations::tracing::EventFilter::Breadcrumb,
        })
}

/// Create environment filter for log level filtering
fn create_env_filter(config: &LoggingConfig) -> Result<EnvFilter> {
    // Start with the configured log level as default
    let default_level = &config.level;
    
    // Try to create from environment variable first, fall back to config
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default_level))
        .unwrap_or_else(|_| {
            // Final fallback to INFO level
            tracing::warn!("Invalid log level '{}', falling back to 'info'", default_level);
            EnvFilter::new("info")
        });
    
    Ok(filter)
}

/// Initialize tracing with stdout output
fn init_stdout_tracing(config: &LoggingConfig, env_filter: EnvFilter) -> Result<()> {
    let layer = fmt::layer()
        .with_writer(io::stdout)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(config.include_location)
        .with_line_number(config.include_location)
        .with_span_events(FmtSpan::CLOSE);

    match config.format.to_lowercase().as_str() {
        "json" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.json())
                .init();
        }
        "pretty" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.pretty())
                .init();
        }
        "compact" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.compact())
                .init();
        }
        _ => {
            tracing::warn!("Unknown log format '{}', falling back to json", config.format);
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.json())
                .init();
        }
    }

    Ok(())
}

/// Initialize tracing with stderr output
fn init_stderr_tracing(config: &LoggingConfig, env_filter: EnvFilter) -> Result<()> {
    let layer = fmt::layer()
        .with_writer(io::stderr)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(config.include_location)
        .with_line_number(config.include_location)
        .with_span_events(FmtSpan::CLOSE);

    match config.format.to_lowercase().as_str() {
        "json" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.json())
                .init();
        }
        "pretty" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.pretty())
                .init();
        }
        "compact" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.compact())
                .init();
        }
        _ => {
            tracing::warn!("Unknown log format '{}', falling back to json", config.format);
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.json())
                .init();
        }
    }

    Ok(())
}

/// Initialize tracing with file output
fn init_file_tracing(config: &LoggingConfig, env_filter: EnvFilter) -> Result<WorkerGuard> {
    let file_path = config
        .file_path
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("File path is required when target is 'file'"))?;

    // Extract directory and filename
    let path = std::path::Path::new(file_path);
    let directory = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid file path: {}", file_path))?;
    let filename = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", file_path))?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid filename encoding: {}", file_path))?;

    // Create directory if it doesn't exist
    std::fs::create_dir_all(directory)?;

    // Create non-blocking file appender
    let file_appender = tracing_appender::rolling::daily(directory, filename);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let layer = fmt::layer()
        .with_writer(non_blocking)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(config.include_location)
        .with_line_number(config.include_location)
        .with_span_events(FmtSpan::CLOSE);

    match config.format.to_lowercase().as_str() {
        "json" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.json())
                .init();
        }
        "pretty" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.pretty())
                .init();
        }
        "compact" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.compact())
                .init();
        }
        _ => {
            tracing::warn!("Unknown log format '{}', falling back to json", config.format);
            tracing_subscriber::registry()
                .with(env_filter)
                .with(layer.json())
                .init();
        }
    }

    Ok(guard)
}



/// Macro for creating spans with correlation ID
#[macro_export]
macro_rules! trace_span {
    ($level:expr, $name:expr, $correlation_id:expr) => {
        tracing::span!($level, $name, correlation_id = %$correlation_id)
    };
    ($level:expr, $name:expr, $correlation_id:expr, $($field:tt)*) => {
        tracing::span!($level, $name, correlation_id = %$correlation_id, $($field)*)
    };
}

/// Macro for creating info spans with correlation ID
#[macro_export]
macro_rules! info_span {
    ($name:expr, $correlation_id:expr) => {
        $crate::trace_span!(tracing::Level::INFO, $name, $correlation_id)
    };
    ($name:expr, $correlation_id:expr, $($field:tt)*) => {
        $crate::trace_span!(tracing::Level::INFO, $name, $correlation_id, $($field)*)
    };
}

/// Macro for creating debug spans with correlation ID
#[macro_export]
macro_rules! debug_span {
    ($name:expr, $correlation_id:expr) => {
        $crate::trace_span!(tracing::Level::DEBUG, $name, $correlation_id)
    };
    ($name:expr, $correlation_id:expr, $($field:tt)*) => {
        $crate::trace_span!(tracing::Level::DEBUG, $name, $correlation_id, $($field)*)
    };
}

/// Macro for creating error spans with correlation ID
#[macro_export]
macro_rules! error_span {
    ($name:expr, $correlation_id:expr) => {
        $crate::trace_span!(tracing::Level::ERROR, $name, $correlation_id)
    };
    ($name:expr, $correlation_id:expr, $($field:tt)*) => {
        $crate::trace_span!(tracing::Level::ERROR, $name, $correlation_id, $($field)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::{AppConfig, LoggingConfig};

    fn create_test_config(level: &str, format: &str, target: &str) -> AppConfig {
        let mut config = AppConfig::default();
        config.logging = LoggingConfig {
            level: level.to_string(),
            format: format.to_string(),
            include_location: false,
            target: target.to_string(),
            file_path: None,
        };
        config
    }

    #[test]
    fn test_correlation_id_generation() {
        let id1 = CorrelationId::new();
        let id2 = CorrelationId::new();
        
        assert_ne!(id1.as_str(), id2.as_str());
        assert!(!id1.as_str().is_empty());
        assert!(!id2.as_str().is_empty());
    }

    #[test]
    fn test_correlation_id_from_string() {
        let test_id = "test-correlation-id";
        let id = CorrelationId::from_string(test_id.to_string());
        
        assert_eq!(id.as_str(), test_id);
        assert_eq!(id.to_string(), test_id);
    }

    #[test]
    fn test_create_env_filter_valid_levels() {
        let levels = ["trace", "debug", "info", "warn", "error"];
        
        for level in &levels {
            let config = LoggingConfig {
                level: level.to_string(),
                format: "json".to_string(),
                include_location: false,
                target: "stdout".to_string(),
                file_path: None,
            };
            
            let result = create_env_filter(&config);
            assert!(result.is_ok(), "Failed to create filter for level: {}", level);
        }
    }

    #[test]
    fn test_create_env_filter_invalid_level() {
        let config = LoggingConfig {
            level: "invalid".to_string(),
            format: "json".to_string(),
            include_location: false,
            target: "stdout".to_string(),
            file_path: None,
        };
        
        let result = create_env_filter(&config);
        assert!(result.is_ok()); // Should fallback to info level
    }

    #[tokio::test]
    async fn test_init_tracing_stdout() {
        let config = create_test_config("info", "json", "stdout");
        let result = init_tracing(&config);
        
        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // No guard for stdout
    }

    #[tokio::test]
    async fn test_init_tracing_stderr() {
        let config = create_test_config("debug", "pretty", "stderr");
        let result = init_tracing(&config);
        
        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // No guard for stderr
    }

    #[tokio::test]
    async fn test_init_tracing_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_file = temp_dir.path().join("test.log");
        
        let mut config = create_test_config("warn", "compact", "file");
        config.logging.file_path = Some(log_file.to_string_lossy().to_string());
        
        let result = init_tracing(&config);
        
        assert!(result.is_ok());
        assert!(result.unwrap().is_some()); // Should have guard for file
    }
}