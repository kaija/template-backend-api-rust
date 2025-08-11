use std::sync::Arc;
use sqlx::PgPool;

use crate::repository::{UserRepository, SqlxUserRepository};
use crate::services::{
    UserService, UserServiceImpl,
    AuthService, AuthServiceImpl,
    ExternalService, HttpExternalService,
};

/// Service container for dependency injection
///
/// This container manages the lifecycle and dependencies of all services
/// in the application, providing a centralized way to configure and
/// access service instances.
#[derive(Clone)]
pub struct ServiceContainer {
    // Repository layer
    user_repository: Arc<dyn UserRepository>,

    // Service layer
    user_service: Arc<dyn UserService>,
    auth_service: Arc<dyn AuthService>,
    external_service: Arc<dyn ExternalService>,
}

impl ServiceContainer {
    /// Create a new service container with all dependencies configured
    ///
    /// # Arguments
    /// * `db_pool` - Database connection pool for repository layer
    /// * `external_timeout_seconds` - Timeout for external HTTP calls
    ///
    /// # Returns
    /// A fully configured service container with all dependencies wired
    pub fn new(db_pool: PgPool, external_timeout_seconds: u64) -> Self {
        // Initialize repository layer
        let user_repository = Arc::new(SqlxUserRepository::new(db_pool));

        // Initialize external service
        let external_service = Arc::new(HttpExternalService::new(external_timeout_seconds));

        // Initialize service layer with dependencies
        let user_service = Arc::new(UserServiceImpl::new(
            user_repository.clone(),
            external_service.clone(),
        ));

        let auth_service = Arc::new(AuthServiceImpl::new(
            user_repository.clone(),
        ));

        Self {
            user_repository,
            user_service,
            auth_service,
            external_service,
        }
    }

    /// Get user service instance
    pub fn user_service(&self) -> Arc<dyn UserService> {
        self.user_service.clone()
    }

    /// Get authentication service instance
    pub fn auth_service(&self) -> Arc<dyn AuthService> {
        self.auth_service.clone()
    }

    /// Get external service instance
    pub fn external_service(&self) -> Arc<dyn ExternalService> {
        self.external_service.clone()
    }

    /// Get user repository instance (for advanced use cases)
    pub fn user_repository(&self) -> Arc<dyn UserRepository> {
        self.user_repository.clone()
    }
}

/// Application state that holds the service container
///
/// This struct is used throughout the web layer to access services
/// and is typically stored in Axum's application state.
#[derive(Clone)]
pub struct AppState {
    pub services: ServiceContainer,
    pub config: crate::config::AppConfig,
}

impl AppState {
    /// Create new application state
    pub fn new(config: crate::config::AppConfig, db_pool: PgPool) -> Self {
        let external_timeout = config.external_service.timeout_seconds.unwrap_or(30);
        let services = ServiceContainer::new(db_pool, external_timeout);

        Self {
            services,
            config,
        }
    }

    /// Get user service
    pub fn user_service(&self) -> Arc<dyn UserService> {
        self.services.user_service()
    }

    /// Get auth service
    pub fn auth_service(&self) -> Arc<dyn AuthService> {
        self.services.auth_service()
    }

    /// Get external service
    pub fn external_service(&self) -> Arc<dyn ExternalService> {
        self.services.external_service()
    }
}

/// Service factory trait for creating service instances
///
/// This trait can be implemented for different service configurations
/// or testing scenarios where different implementations are needed.
pub trait ServiceFactory: Send + Sync {
    fn create_user_service(&self) -> Arc<dyn UserService>;
    fn create_auth_service(&self) -> Arc<dyn AuthService>;
    fn create_external_service(&self) -> Arc<dyn ExternalService>;
}

/// Default service factory implementation
pub struct DefaultServiceFactory {
    container: ServiceContainer,
}

impl DefaultServiceFactory {
    pub fn new(db_pool: PgPool, external_timeout_seconds: u64) -> Self {
        Self {
            container: ServiceContainer::new(db_pool, external_timeout_seconds),
        }
    }
}

impl ServiceFactory for DefaultServiceFactory {
    fn create_user_service(&self) -> Arc<dyn UserService> {
        self.container.user_service()
    }

    fn create_auth_service(&self) -> Arc<dyn AuthService> {
        self.container.auth_service()
    }

    fn create_external_service(&self) -> Arc<dyn ExternalService> {
        self.container.external_service()
    }
}

/// Service configuration for dependency injection
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub external_timeout_seconds: u64,
    pub auth_token_expiry_hours: u64,
    pub max_retry_attempts: u32,
    pub circuit_breaker_threshold: u32,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            external_timeout_seconds: 30,
            auth_token_expiry_hours: 24,
            max_retry_attempts: 3,
            circuit_breaker_threshold: 5,
        }
    }
}

/// Service health check trait
///
/// Services can implement this trait to provide health check functionality
/// for monitoring and operational purposes.
#[async_trait::async_trait]
pub trait ServiceHealthCheck: Send + Sync {
    async fn health_check(&self) -> Result<ServiceHealthStatus, ServiceHealthError>;
}

/// Service health status
#[derive(Debug, Clone)]
pub struct ServiceHealthStatus {
    pub service_name: String,
    pub is_healthy: bool,
    pub details: Option<String>,
    pub response_time_ms: u64,
}

/// Service health check error
#[derive(Debug, thiserror::Error)]
pub enum ServiceHealthError {
    #[error("Service unavailable: {0}")]
    Unavailable(String),

    #[error("Timeout during health check")]
    Timeout,

    #[error("Internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    // Mock implementations for testing
    struct MockUserRepository;

    #[async_trait::async_trait]
    impl UserRepository for MockUserRepository {
        async fn create(&self, _user: &crate::models::NewUser) -> Result<crate::models::User, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn create_tx(&self, _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, _user: &crate::models::NewUser) -> Result<crate::models::User, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn find_by_id(&self, _id: crate::models::UserId) -> Result<Option<crate::models::User>, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn find_by_email(&self, _email: &str) -> Result<Option<crate::models::User>, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn update(&self, _id: crate::models::UserId, _name: Option<String>, _email: Option<String>) -> Result<crate::models::User, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn update_tx(&self, _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, _id: crate::models::UserId, _name: Option<String>, _email: Option<String>) -> Result<crate::models::User, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn soft_delete(&self, _id: crate::models::UserId) -> Result<(), crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn delete(&self, _id: crate::models::UserId) -> Result<(), crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn list(&self, _limit: i64, _offset: i64) -> Result<Vec<crate::models::User>, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn list_active(&self, _limit: i64, _offset: i64) -> Result<Vec<crate::models::User>, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn count(&self) -> Result<i64, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn count_active(&self) -> Result<i64, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn email_exists(&self, _email: &str) -> Result<bool, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn email_exists_for_other_user(&self, _email: &str, _user_id: crate::models::UserId) -> Result<bool, crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn activate(&self, _id: crate::models::UserId) -> Result<(), crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }

        async fn deactivate(&self, _id: crate::models::UserId) -> Result<(), crate::repository::RepositoryError> {
            todo!("Mock implementation")
        }
    }

    #[test]
    fn test_service_config_default() {
        let config = ServiceConfig::default();
        assert_eq!(config.external_timeout_seconds, 30);
        assert_eq!(config.auth_token_expiry_hours, 24);
        assert_eq!(config.max_retry_attempts, 3);
        assert_eq!(config.circuit_breaker_threshold, 5);
    }

    #[test]
    fn test_service_health_status() {
        let status = ServiceHealthStatus {
            service_name: "test-service".to_string(),
            is_healthy: true,
            details: Some("All systems operational".to_string()),
            response_time_ms: 150,
        };

        assert_eq!(status.service_name, "test-service");
        assert!(status.is_healthy);
        assert_eq!(status.response_time_ms, 150);
    }

    // Note: Integration tests would require a real database connection
    // These would be implemented in the integration test suite
}
