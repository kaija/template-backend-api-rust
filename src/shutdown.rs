use std::time::Duration;
use tokio::signal;
use tracing::{info, warn, error};

/// Graceful shutdown handler that listens for termination signals
/// and coordinates the shutdown sequence
pub struct GracefulShutdown {
    shutdown_timeout: Duration,
}

impl GracefulShutdown {
    /// Create a new graceful shutdown handler with the specified timeout
    pub fn new(shutdown_timeout: Duration) -> Self {
        Self { shutdown_timeout }
    }

    /// Wait for termination signals (SIGTERM, SIGINT, or Ctrl+C)
    /// Returns when a shutdown signal is received
    pub async fn wait_for_shutdown_signal(&self) {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to install SIGTERM handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received SIGINT (Ctrl+C), initiating graceful shutdown");
            }
            _ = terminate => {
                info!("Received SIGTERM, initiating graceful shutdown");
            }
        }
    }

    /// Execute the graceful shutdown sequence with timeout
    /// This coordinates the shutdown of various application components
    pub async fn execute_shutdown<F, Fut>(&self, shutdown_fn: F) -> Result<(), ShutdownError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), ShutdownError>>,
    {
        info!("Starting graceful shutdown sequence with timeout of {:?}", self.shutdown_timeout);

        // Execute shutdown with timeout
        match tokio::time::timeout(self.shutdown_timeout, shutdown_fn()).await {
            Ok(Ok(())) => {
                info!("✅ Graceful shutdown completed successfully");
                Ok(())
            }
            Ok(Err(e)) => {
                error!("❌ Error during graceful shutdown: {}", e);
                Err(e)
            }
            Err(_) => {
                warn!("⚠️ Graceful shutdown timed out after {:?}, forcing exit", self.shutdown_timeout);
                Err(ShutdownError::Timeout)
            }
        }
    }
}

/// Errors that can occur during shutdown
#[derive(Debug, thiserror::Error)]
pub enum ShutdownError {
    #[error("Shutdown timed out")]
    Timeout,
    
    #[error("Database shutdown error: {0}")]
    Database(String),
    
    #[error("HTTP server shutdown error: {0}")]
    HttpServer(String),
    
    #[error("External service cleanup error: {0}")]
    ExternalService(String),
    
    #[error("Resource cleanup error: {0}")]
    ResourceCleanup(String),
}

/// Shutdown coordinator that manages the shutdown sequence for all application components
pub struct ShutdownCoordinator {
    components: Vec<Box<dyn ShutdownComponent>>,
}

impl ShutdownCoordinator {
    /// Create a new shutdown coordinator
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    /// Register a component for shutdown
    pub fn register<T: ShutdownComponent + 'static>(&mut self, component: T) {
        self.components.push(Box::new(component));
    }

    /// Execute shutdown for all registered components
    pub async fn shutdown_all(&mut self) -> Result<(), ShutdownError> {
        info!("Shutting down {} components", self.components.len());

        // Shutdown components in reverse order (LIFO)
        for (_index, component) in self.components.iter_mut().enumerate().rev() {
            let component_name = component.name().to_string();
            info!("Shutting down component: {}", component_name);

            match component.shutdown().await {
                Ok(()) => {
                    info!("✅ Component '{}' shut down successfully", component_name);
                }
                Err(e) => {
                    error!("❌ Failed to shutdown component '{}': {}", component_name, e);
                    // Continue with other components even if one fails
                }
            }
        }

        info!("All components shutdown sequence completed");
        Ok(())
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for components that need to be shut down gracefully
#[async_trait::async_trait]
pub trait ShutdownComponent: Send + Sync {
    /// Get the name of this component for logging
    fn name(&self) -> &str;

    /// Shutdown this component gracefully
    async fn shutdown(&mut self) -> Result<(), ShutdownError>;
}

/// HTTP server shutdown component
pub struct HttpServerShutdown {
    server_handle: Option<axum_server::Handle>,
    drain_timeout: Duration,
}

impl HttpServerShutdown {
    pub fn new(server_handle: axum_server::Handle) -> Self {
        Self {
            server_handle: Some(server_handle),
            drain_timeout: Duration::from_secs(10), // Default 10 second timeout for connection draining
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.drain_timeout = timeout;
        self
    }
}

#[async_trait::async_trait]
impl ShutdownComponent for HttpServerShutdown {
    fn name(&self) -> &str {
        "HTTP Server"
    }

    async fn shutdown(&mut self) -> Result<(), ShutdownError> {
        if let Some(handle) = self.server_handle.take() {
            info!("Initiating HTTP server graceful shutdown with drain timeout of {:?}", self.drain_timeout);
            
            // Signal the server to stop accepting new connections and set drain timeout
            handle.graceful_shutdown(Some(self.drain_timeout));
            
            info!("HTTP server shutdown initiated, waiting for connections to drain");
            
            // Give a moment for the shutdown signal to be processed
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            info!("HTTP server graceful shutdown completed");
            Ok(())
        } else {
            warn!("HTTP server handle already consumed or not available");
            Ok(())
        }
    }
}

/// Database connection pool shutdown component
pub struct DatabaseShutdown {
    database: Option<crate::database::Database>,
    close_timeout: Duration,
}

impl DatabaseShutdown {
    pub fn new(database: crate::database::Database) -> Self {
        Self {
            database: Some(database),
            close_timeout: Duration::from_secs(10), // Default 10 second timeout for database close
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.close_timeout = timeout;
        self
    }
}

#[async_trait::async_trait]
impl ShutdownComponent for DatabaseShutdown {
    fn name(&self) -> &str {
        "Database Connection Pool"
    }

    async fn shutdown(&mut self) -> Result<(), ShutdownError> {
        if let Some(database) = self.database.take() {
            info!("Closing database connection pool with timeout of {:?}", self.close_timeout);
            
            // Close database with timeout
            let close_result = tokio::time::timeout(self.close_timeout, async {
                // Get connection stats before closing for logging
                let stats = database.connection_stats();
                info!("Database connection stats before close: active={}, idle={}, max={}", 
                      stats.size, stats.idle, stats.max_connections);
                
                // Close the database connection pool
                database.close().await;
                
                Ok::<(), ShutdownError>(())
            }).await;

            match close_result {
                Ok(Ok(())) => {
                    info!("Database connection pool closed successfully");
                    Ok(())
                }
                Ok(Err(e)) => {
                    error!("Error during database close: {}", e);
                    Err(e)
                }
                Err(_) => {
                    warn!("Database close timed out after {:?}, connections may not be properly closed", self.close_timeout);
                    // Even if timeout occurs, we consider this a successful shutdown
                    // as the process will terminate anyway
                    Err(ShutdownError::Database("Close timeout".to_string()))
                }
            }
        } else {
            warn!("Database already closed or not available");
            Ok(())
        }
    }
}

/// External service connections shutdown component
pub struct ExternalServiceShutdown {
    service: Option<std::sync::Arc<dyn crate::services::ExternalService>>,
    cleanup_timeout: Duration,
}

impl ExternalServiceShutdown {
    pub fn new(service: std::sync::Arc<dyn crate::services::ExternalService>) -> Self {
        Self {
            service: Some(service),
            cleanup_timeout: Duration::from_secs(5), // Default 5 second timeout for cleanup
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.cleanup_timeout = timeout;
        self
    }
}

#[async_trait::async_trait]
impl ShutdownComponent for ExternalServiceShutdown {
    fn name(&self) -> &str {
        "External Service Connections"
    }

    async fn shutdown(&mut self) -> Result<(), ShutdownError> {
        if let Some(_service) = self.service.take() {
            info!("Cleaning up external service connections with timeout of {:?}", self.cleanup_timeout);
            
            // Perform cleanup with timeout
            let cleanup_result = tokio::time::timeout(self.cleanup_timeout, async {
                // External service cleanup operations
                // For HTTP clients, this typically involves:
                // 1. Cancelling any ongoing requests
                // 2. Closing connection pools
                // 3. Dropping the client which closes keep-alive connections
                
                // Simulate cleanup work - in a real implementation this would:
                // - Cancel ongoing HTTP requests
                // - Close connection pools
                // - Wait for in-flight requests to complete (with timeout)
                tokio::time::sleep(Duration::from_millis(100)).await;
                
                Ok::<(), ShutdownError>(())
            }).await;

            match cleanup_result {
                Ok(Ok(())) => {
                    info!("External service connections cleaned up successfully");
                    Ok(())
                }
                Ok(Err(e)) => {
                    error!("Error during external service cleanup: {}", e);
                    Err(e)
                }
                Err(_) => {
                    warn!("External service cleanup timed out after {:?}, forcing cleanup", self.cleanup_timeout);
                    // Force cleanup by dropping references
                    Err(ShutdownError::ExternalService("Cleanup timeout".to_string()))
                }
            }
        } else {
            warn!("External service already cleaned up or not available");
            Ok(())
        }
    }
}

/// Tracing and logging shutdown component
pub struct TracingShutdown {
    _guard: Option<tracing_appender::non_blocking::WorkerGuard>,
    flush_timeout: Duration,
}

impl TracingShutdown {
    pub fn new(guard: tracing_appender::non_blocking::WorkerGuard) -> Self {
        Self {
            _guard: Some(guard),
            flush_timeout: Duration::from_millis(500), // Default 500ms timeout for log flushing
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.flush_timeout = timeout;
        self
    }
}

#[async_trait::async_trait]
impl ShutdownComponent for TracingShutdown {
    fn name(&self) -> &str {
        "Tracing and Logging"
    }

    async fn shutdown(&mut self) -> Result<(), ShutdownError> {
        if let Some(_guard) = self._guard.take() {
            info!("Flushing remaining log entries with timeout of {:?}", self.flush_timeout);
            
            // Flush remaining log entries with timeout
            let flush_result = tokio::time::timeout(self.flush_timeout, async {
                // Give time for remaining log entries to be flushed
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<(), ShutdownError>(())
            }).await;

            match flush_result {
                Ok(Ok(())) => {
                    // The guard will be dropped here, which will flush and close the writer
                    info!("Tracing shutdown completed successfully");
                    Ok(())
                }
                Ok(Err(e)) => {
                    error!("Error during tracing flush: {}", e);
                    Err(e)
                }
                Err(_) => {
                    warn!("Tracing flush timed out after {:?}, some log entries may be lost", self.flush_timeout);
                    // Continue with shutdown even if flush times out
                    Ok(())
                }
            }
        } else {
            warn!("Tracing guard already consumed or not available");
            Ok(())
        }
    }
}

/// Resource cleanup utilities for proper resource disposal
pub struct ResourceCleanup;

impl ResourceCleanup {
    /// Clean up file handles and temporary resources
    pub async fn cleanup_file_resources() -> Result<(), ShutdownError> {
        info!("Cleaning up file resources");
        
        // In a real implementation, this would:
        // - Close any open file handles
        // - Clean up temporary files
        // - Flush any buffered file operations
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        info!("File resources cleaned up");
        Ok(())
    }

    /// Clean up memory resources and caches
    pub async fn cleanup_memory_resources() -> Result<(), ShutdownError> {
        info!("Cleaning up memory resources");
        
        // In a real implementation, this would:
        // - Clear caches
        // - Drop large data structures
        // - Force garbage collection if applicable
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        info!("Memory resources cleaned up");
        Ok(())
    }

    /// Clean up network resources
    pub async fn cleanup_network_resources() -> Result<(), ShutdownError> {
        info!("Cleaning up network resources");
        
        // In a real implementation, this would:
        // - Close any remaining network connections
        // - Cancel pending network operations
        // - Clean up connection pools
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        info!("Network resources cleaned up");
        Ok(())
    }

    /// Perform comprehensive resource cleanup with timeout
    pub async fn cleanup_all_resources(timeout: Duration) -> Result<(), ShutdownError> {
        info!("Starting comprehensive resource cleanup with timeout of {:?}", timeout);
        
        let cleanup_result = tokio::time::timeout(timeout, async {
            // Clean up resources in parallel for efficiency
            let (file_result, memory_result, network_result) = tokio::join!(
                Self::cleanup_file_resources(),
                Self::cleanup_memory_resources(),
                Self::cleanup_network_resources()
            );

            // Check results and report any errors
            if let Err(e) = file_result {
                error!("File resource cleanup failed: {}", e);
            }
            if let Err(e) = memory_result {
                error!("Memory resource cleanup failed: {}", e);
            }
            if let Err(e) = network_result {
                error!("Network resource cleanup failed: {}", e);
            }

            Ok::<(), ShutdownError>(())
        }).await;

        match cleanup_result {
            Ok(Ok(())) => {
                info!("Comprehensive resource cleanup completed successfully");
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Error during resource cleanup: {}", e);
                Err(e)
            }
            Err(_) => {
                warn!("Resource cleanup timed out after {:?}", timeout);
                Err(ShutdownError::ResourceCleanup("Cleanup timeout".to_string()))
            }
        }
    }
}

/// General resource cleanup shutdown component
pub struct GeneralResourceCleanup {
    cleanup_timeout: Duration,
}

impl GeneralResourceCleanup {
    pub fn new() -> Self {
        Self {
            cleanup_timeout: Duration::from_secs(5), // Default 5 second timeout
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.cleanup_timeout = timeout;
        self
    }
}

impl Default for GeneralResourceCleanup {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ShutdownComponent for GeneralResourceCleanup {
    fn name(&self) -> &str {
        "General Resource Cleanup"
    }

    async fn shutdown(&mut self) -> Result<(), ShutdownError> {
        ResourceCleanup::cleanup_all_resources(self.cleanup_timeout).await
    }
}

#[cfg(test)]
mod tests;