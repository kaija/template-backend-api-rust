use anyhow::Result;
use rust_api::{config, tracing as app_tracing, web::handlers::health_handlers};
use tracing_appender::non_blocking::WorkerGuard;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Rust API Microservice Template");

    // Initialize health check start time for uptime tracking
    health_handlers::init_start_time();

    // Load configuration from multiple sources
    let config = config::AppConfig::load()?;

    // Initialize tracing with multiple layers
    let _guard: Option<WorkerGuard> = app_tracing::init_tracing(&config)?;

    tracing::info!("Configuration loaded and tracing initialized");

    // TODO: Initialize Sentry integration
    // TODO: Setup database connection pool
    // TODO: Create application state
    // TODO: Build router with middleware
    // TODO: Start server with graceful shutdown

    tracing::info!("✅ Server started successfully");

    Ok(())
}
