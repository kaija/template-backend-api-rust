use sqlx::{postgres::PgPoolOptions, PgPool, Pool, Postgres};
use std::time::Duration;
use tracing::{info, warn};

use crate::config::settings::DatabaseConfig;

/// Database connection pool and related utilities
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Create a new database connection pool
    pub async fn new(config: &DatabaseConfig) -> Result<Self, DatabaseError> {
        info!("Initializing database connection pool");

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.acquire_timeout_seconds))
            .idle_timeout(Duration::from_secs(config.idle_timeout_seconds))
            .max_lifetime(Duration::from_secs(config.idle_timeout_seconds * 2)) // Set max lifetime to 2x idle timeout
            .test_before_acquire(true) // Test connections before use
            .connect(&config.url)
            .await
            .map_err(|e| DatabaseError::ConnectionFailed(e.to_string()))?;

        info!(
            "Database connection pool initialized with {} max connections",
            config.max_connections
        );

        Ok(Self { pool })
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get a clone of the connection pool
    pub fn pool_cloned(&self) -> PgPool {
        self.pool.clone()
    }

    /// Check database connectivity and health
    pub async fn health_check(&self) -> Result<DatabaseHealth, DatabaseError> {
        let start = std::time::Instant::now();

        // Test basic connectivity with a simple query
        let result = sqlx::query("SELECT 1 as health_check")
            .fetch_one(&self.pool)
            .await;

        let response_time = start.elapsed();

        match result {
            Ok(_) => {
                let pool_status = self.pool.size();
                Ok(DatabaseHealth {
                    connected: true,
                    response_time_ms: response_time.as_millis() as u64,
                    active_connections: pool_status as u32,
                    idle_connections: self.pool.num_idle() as u32,
                    max_connections: self.pool.options().get_max_connections(),
                })
            }
            Err(e) => {
                warn!("Database health check failed: {}", e);
                Err(DatabaseError::HealthCheckFailed(e.to_string()))
            }
        }
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<(), DatabaseError> {
        info!("Running database migrations");

        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| DatabaseError::MigrationFailed(e.to_string()))?;

        info!("Database migrations completed successfully");
        Ok(())
    }

    /// Close the database connection pool gracefully
    pub async fn close(&self) {
        info!("Closing database connection pool");
        self.pool.close().await;
        info!("Database connection pool closed");
    }

    /// Get database connection statistics
    pub fn connection_stats(&self) -> ConnectionStats {
        ConnectionStats {
            size: self.pool.size() as u32,
            idle: self.pool.num_idle() as u32,
            max_connections: self.pool.options().get_max_connections(),
            min_connections: self.pool.options().get_min_connections(),
        }
    }
}

/// Database health information
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatabaseHealth {
    pub connected: bool,
    pub response_time_ms: u64,
    pub active_connections: u32,
    pub idle_connections: u32,
    pub max_connections: u32,
}

/// Database connection statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionStats {
    pub size: u32,
    pub idle: u32,
    pub max_connections: u32,
    pub min_connections: u32,
}

/// Database-related errors
#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("Failed to connect to database: {0}")]
    ConnectionFailed(String),

    #[error("Database health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("Database migration failed: {0}")]
    MigrationFailed(String),

    #[error("Database query failed: {0}")]
    QueryFailed(String),

    #[error("Database transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Database pool error: {0}")]
    PoolError(String),
}

impl From<sqlx::Error> for DatabaseError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::PoolTimedOut => DatabaseError::PoolError("Connection pool timed out".to_string()),
            sqlx::Error::PoolClosed => DatabaseError::PoolError("Connection pool is closed".to_string()),
            _ => DatabaseError::QueryFailed(err.to_string()),
        }
    }
}

/// Type alias for the database pool
pub type DbPool = Pool<Postgres>;

/// Helper function to create a database connection pool
pub async fn create_pool(config: &DatabaseConfig) -> Result<PgPool, DatabaseError> {
    Database::new(config).await.map(|db| db.pool_cloned())
}

/// Helper function to run migrations
pub async fn run_migrations(pool: &PgPool) -> Result<(), DatabaseError> {
    info!("Running database migrations");

    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| DatabaseError::MigrationFailed(e.to_string()))?;

    info!("Database migrations completed successfully");
    Ok(())
}

/// Helper function to perform database health check
pub async fn health_check(pool: &PgPool) -> Result<DatabaseHealth, DatabaseError> {
    let start = std::time::Instant::now();

    let result = sqlx::query("SELECT 1 as health_check")
        .fetch_one(pool)
        .await;

    let response_time = start.elapsed();

    match result {
        Ok(_) => {
            let pool_status = pool.size();
            Ok(DatabaseHealth {
                connected: true,
                response_time_ms: response_time.as_millis() as u64,
                active_connections: pool_status as u32,
                idle_connections: pool.num_idle() as u32,
                max_connections: pool.options().get_max_connections(),
            })
        }
        Err(e) => {
            warn!("Database health check failed: {}", e);
            Err(DatabaseError::HealthCheckFailed(e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::DatabaseConfig;

    #[tokio::test]
    async fn test_database_config_validation() {
        let config = DatabaseConfig {
            url: "postgresql://localhost/test".to_string(),
            max_connections: 5,
            min_connections: 1,
            acquire_timeout_seconds: 30,
            idle_timeout_seconds: 600,
            connect_timeout_seconds: 10,
            statement_timeout_seconds: 30,
        };

        assert!(config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_invalid_database_config() {
        let config = DatabaseConfig {
            url: "".to_string(), // Invalid empty URL
            max_connections: 5,
            min_connections: 1,
            acquire_timeout_seconds: 30,
            idle_timeout_seconds: 600,
            connect_timeout_seconds: 10,
            statement_timeout_seconds: 30,
        };

        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_connection_stats_serialization() {
        let stats = ConnectionStats {
            size: 5,
            idle: 3,
            max_connections: 10,
            min_connections: 1,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"size\":5"));
        assert!(json.contains("\"idle\":3"));
    }

    #[tokio::test]
    async fn test_database_health_serialization() {
        let health = DatabaseHealth {
            connected: true,
            response_time_ms: 50,
            active_connections: 3,
            idle_connections: 2,
            max_connections: 10,
        };

        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("\"connected\":true"));
        assert!(json.contains("\"response_time_ms\":50"));
    }
}
