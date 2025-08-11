use anyhow::Result;
use rust_api::{
    config, 
    database::Database,
    services::container::ServiceContainer,
    shutdown::{GracefulShutdown, ShutdownCoordinator, HttpServerShutdown, DatabaseShutdown, ExternalServiceShutdown, TracingShutdown, GeneralResourceCleanup},
    tracing as app_tracing, 
    web::{handlers::health_handlers, router::{create_router, AppState}},
};
use std::{net::SocketAddr, time::Duration};
use tracing::{info, error};
use tracing_appender::non_blocking::WorkerGuard;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Rust API Microservice Template");

    // Initialize health check start time for uptime tracking
    health_handlers::init_start_time();

    // Load configuration from multiple sources
    let config = config::AppConfig::load()?;

    // Initialize tracing with multiple layers
    let tracing_guard: Option<WorkerGuard> = app_tracing::init_tracing(&config)?;

    info!("Configuration loaded and tracing initialized");

    // Initialize Sentry integration
    let _sentry_guard = sentry::init((
        config.sentry.dsn.clone(),
        sentry::ClientOptions {
            release: config.sentry.release.clone().map(|r| r.into()).or_else(|| sentry::release_name!().map(|r| r.into())),
            environment: Some(config.sentry.environment.clone().into()),
            traces_sample_rate: config.sentry.traces_sample_rate,
            debug: config.sentry.debug,
            max_breadcrumbs: config.sentry.max_breadcrumbs,
            ..Default::default()
        },
    ));

    info!("Sentry integration initialized");

    // Setup database connection pool
    let database = Database::new(&config.database).await?;
    
    // Run database migrations
    database.migrate().await?;
    
    info!("Database connection pool initialized and migrations completed");

    // Create service container with dependencies
    let services = ServiceContainer::new(
        database.pool_cloned(), 
        config.external_service.timeout_seconds.unwrap_or(30)
    );

    // Clone services for shutdown coordinator before moving to app state
    let external_service_for_shutdown = services.external_service();

    // Create application state
    let app_state = AppState::new(config.clone(), services);

    // Build router with middleware
    let app = create_router(app_state);

    // Setup graceful shutdown handler
    let graceful_shutdown = GracefulShutdown::new(Duration::from_secs(
        config.server.graceful_shutdown_timeout_seconds
    ));

    // Create server address
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    
    info!("Starting HTTP server on {}", addr);

    // Create server handle for graceful shutdown
    let handle = axum_server::Handle::new();
    
    // Create server with handle
    let server = axum_server::bind(addr)
        .handle(handle.clone())
        .serve(app.into_make_service());

    // Setup shutdown coordinator with all components
    let mut shutdown_coordinator = ShutdownCoordinator::new();
    
    // Register shutdown components in reverse order of startup with configurable timeouts
    shutdown_coordinator.register(
        HttpServerShutdown::new(handle)
            .with_timeout(Duration::from_secs(config.server.connection_drain_timeout_seconds))
    );
    shutdown_coordinator.register(
        ExternalServiceShutdown::new(external_service_for_shutdown)
            .with_timeout(Duration::from_secs(config.external_service.timeout_seconds.unwrap_or(5)))
    );
    shutdown_coordinator.register(
        DatabaseShutdown::new(database)
            .with_timeout(Duration::from_secs(config.database.idle_timeout_seconds.min(10)))
    );
    
    // Add general resource cleanup
    shutdown_coordinator.register(
        GeneralResourceCleanup::new()
            .with_timeout(Duration::from_secs(config.server.resource_cleanup_timeout_seconds))
    );
    
    if let Some(guard) = tracing_guard {
        shutdown_coordinator.register(
            TracingShutdown::new(guard)
                .with_timeout(Duration::from_millis(1000))
        );
    }

    info!("✅ Server started successfully on {}", addr);

    // Run server and wait for shutdown signal concurrently
    tokio::select! {
        result = server => {
            if let Err(e) = result {
                error!("Server error: {}", e);
                return Err(e.into());
            }
        }
        _ = graceful_shutdown.wait_for_shutdown_signal() => {
            info!("Shutdown signal received, initiating graceful shutdown");
        }
    }

    // Execute graceful shutdown sequence
    let shutdown_result = graceful_shutdown.execute_shutdown(|| async {
        shutdown_coordinator.shutdown_all().await
    }).await;

    match shutdown_result {
        Ok(()) => {
            info!("✅ Application shutdown completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("❌ Error during shutdown: {}", e);
            Err(e.into())
        }
    }
}
