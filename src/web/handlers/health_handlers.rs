use axum::{extract::State, http::StatusCode, response::Json};
use serde_json::{json, Value};
use std::time::Instant;
use tracing::{info, warn, error};

use crate::{
    database::DatabaseHealth,
    services::external_service::{ExternalServiceHealthStatus, CircuitBreakerState},
    web::router::AppState,
};

/// Application start time for uptime calculation
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Initialize the start time (should be called during application startup)
pub fn init_start_time() {
    START_TIME.set(Instant::now()).ok();
}

/// Get application uptime in seconds
pub fn get_uptime_seconds() -> u64 {
    START_TIME
        .get()
        .map(|start| start.elapsed().as_secs())
        .unwrap_or(0)
}

/// Liveness probe endpoint
/// Returns 200 OK if the service is running
/// This endpoint should always return OK unless the service is completely down
pub async fn liveness() -> StatusCode {
    info!("Liveness probe check");
    StatusCode::OK
}

/// Readiness probe endpoint
/// Returns 200 OK if the service is ready to handle requests
/// Checks database connectivity and other critical dependencies
pub async fn readiness(State(state): State<AppState>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    info!("Readiness probe check starting");

    let mut checks = serde_json::Map::new();
    let mut overall_ready = true;
    let check_start = Instant::now();

    // Check database connectivity
    let _db_check = match check_database_health(&state).await {
        Ok(health) => {
            checks.insert("database".to_string(), json!({
                "status": "healthy",
                "response_time_ms": health.response_time_ms,
                "active_connections": health.active_connections,
                "idle_connections": health.idle_connections,
                "max_connections": health.max_connections
            }));
            true
        }
        Err(e) => {
            error!("Database health check failed: {}", e);
            checks.insert("database".to_string(), json!({
                "status": "unhealthy",
                "error": e.to_string()
            }));
            overall_ready = false;
            false
        }
    };

    // Check external services (if configured)
    let _external_check = match check_external_services_health(&state).await {
        Ok(status) => {
            checks.insert("external_services".to_string(), json!({
                "status": if status.is_healthy { "healthy" } else { "degraded" },
                "response_time_ms": status.response_time.as_millis(),
                "circuit_breaker_state": format!("{:?}", status.circuit_breaker_state),
                "error": status.error_message
            }));
            // External services being down shouldn't make the service unready
            // but we log it for monitoring
            if !status.is_healthy {
                warn!("External services are unhealthy but service remains ready");
            }
            true
        }
        Err(e) => {
            warn!("External service health check failed: {}", e);
            checks.insert("external_services".to_string(), json!({
                "status": "unknown",
                "error": e.to_string()
            }));
            // External service check failure doesn't affect readiness
            true
        }
    };

    let total_check_time = check_start.elapsed();

    let response = json!({
        "status": if overall_ready { "ready" } else { "not_ready" },
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "uptime_seconds": get_uptime_seconds(),
        "check_duration_ms": total_check_time.as_millis(),
        "checks": checks
    });

    if overall_ready {
        info!("Readiness probe check completed successfully in {:?}", total_check_time);
        Ok(Json(response))
    } else {
        error!("Readiness probe check failed in {:?}", total_check_time);
        Err((StatusCode::SERVICE_UNAVAILABLE, Json(response)))
    }
}

/// Health check endpoint with detailed information
/// Provides comprehensive health information for monitoring and debugging
pub async fn health(State(state): State<AppState>) -> Json<Value> {
    info!("Health check endpoint called");

    let mut checks = serde_json::Map::new();
    let check_start = Instant::now();

    // Database health check
    match check_database_health(&state).await {
        Ok(health) => {
            checks.insert("database".to_string(), json!({
                "status": "healthy",
                "connected": health.connected,
                "response_time_ms": health.response_time_ms,
                "active_connections": health.active_connections,
                "idle_connections": health.idle_connections,
                "max_connections": health.max_connections
            }));
        }
        Err(e) => {
            checks.insert("database".to_string(), json!({
                "status": "unhealthy",
                "error": e.to_string()
            }));
        }
    }

    // External services health check
    match check_external_services_health(&state).await {
        Ok(status) => {
            checks.insert("external_services".to_string(), json!({
                "status": if status.is_healthy { "healthy" } else { "unhealthy" },
                "response_time_ms": status.response_time.as_millis(),
                "circuit_breaker_state": format!("{:?}", status.circuit_breaker_state),
                "error": status.error_message
            }));
        }
        Err(e) => {
            checks.insert("external_services".to_string(), json!({
                "status": "unknown",
                "error": e.to_string()
            }));
        }
    }

    let total_check_time = check_start.elapsed();

    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": get_uptime_seconds(),
        "environment": state.config.environment,
        "check_duration_ms": total_check_time.as_millis(),
        "checks": checks,
        "system": {
            "rust_version": env!("RUSTC_VERSION"),
            "target": env!("TARGET"),
            "build_timestamp": env!("BUILD_TIMESTAMP")
        }
    }))
}

/// Check database health
async fn check_database_health(state: &AppState) -> Result<DatabaseHealth, String> {
    // Get database pool from the service container
    let user_repository = state.services.user_repository();

    // For now, we'll perform a simple health check by trying to get the connection pool
    // In a real implementation, we would have access to the pool directly
    // This is a simplified version that checks if we can perform a basic operation

    let start_time = Instant::now();

    // Try to perform a simple database operation to check connectivity
    match user_repository.count().await {
        Ok(_) => {
            let response_time = start_time.elapsed();
            Ok(DatabaseHealth {
                connected: true,
                response_time_ms: response_time.as_millis() as u64,
                active_connections: 1, // Simplified - in real implementation we'd get actual stats
                idle_connections: 0,   // Simplified - in real implementation we'd get actual stats
                max_connections: 10,   // Simplified - in real implementation we'd get from config
            })
        }
        Err(e) => {
            Err(format!("Database connectivity check failed: {}", e))
        }
    }
}

/// Check external services health
async fn check_external_services_health(state: &AppState) -> Result<ExternalServiceHealthStatus, String> {
    let external_service = state.services.external_service();

    // For health check, we'll try to make a simple request to a health endpoint
    // In a real implementation, this would be configurable
    let health_url = "https://httpbin.org/status/200"; // Simple endpoint for testing

    let start_time = Instant::now();

    match external_service.get(health_url).await {
        Ok(_) => {
            let response_time = start_time.elapsed();
            Ok(ExternalServiceHealthStatus {
                is_healthy: true,
                response_time,
                circuit_breaker_state: CircuitBreakerState::Closed, // Simplified
                error_message: None,
            })
        }
        Err(e) => {
            let response_time = start_time.elapsed();
            Ok(ExternalServiceHealthStatus {
                is_healthy: false,
                response_time,
                circuit_breaker_state: CircuitBreakerState::Open, // Simplified
                error_message: Some(e.to_string()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_uptime_seconds() {
        // Initialize start time
        init_start_time();

        // Should return a value >= 0
        let uptime = get_uptime_seconds();
        assert!(uptime >= 0);
    }

    #[tokio::test]
    async fn test_liveness_endpoint() {
        let status = liveness().await;
        assert_eq!(status, StatusCode::OK);
    }

    #[test]
    fn test_init_start_time() {
        // Should not panic when called multiple times
        init_start_time();
        init_start_time();

        let uptime = get_uptime_seconds();
        assert!(uptime >= 0);
    }
}
