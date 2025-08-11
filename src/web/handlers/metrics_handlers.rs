use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::time::Instant;
use tracing::{info, warn};

use crate::{
    web::{handlers::health_handlers, router::AppState},
};

/// Metrics endpoint for Prometheus scraping
/// Returns metrics in Prometheus text format
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    info!("Metrics endpoint called");

    let start_time = Instant::now();

    // Update system metrics before gathering
    let uptime = health_handlers::get_uptime_seconds() as f64;
    if let Some(metrics) = &state.metrics {
        metrics.update_system_metrics(uptime);

        // Update database metrics if available
        if let Ok(db_health) = check_database_metrics(&state).await {
            metrics.update_database_metrics(
                db_health.active_connections as i64,
                db_health.idle_connections as i64,
            );
        }

        // Gather all metrics
        let metrics_output = metrics.gather();
        let gather_duration = start_time.elapsed();

        info!("Metrics gathered in {:?}, {} bytes", gather_duration, metrics_output.len());

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(metrics_output)
            .unwrap()
    } else {
        warn!("Metrics not initialized");
        Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .header(header::CONTENT_TYPE, "text/plain")
            .body("Metrics not available".to_string())
            .unwrap()
    }
}

/// Health metrics endpoint with JSON format
/// Provides metrics in a more human-readable JSON format
pub async fn metrics_json(State(state): State<AppState>) -> impl IntoResponse {
    info!("JSON metrics endpoint called");

    let start_time = Instant::now();
    let uptime = health_handlers::get_uptime_seconds() as f64;

    let mut metrics_data = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "uptime_seconds": uptime,
        "service": "rust-api",
        "version": env!("CARGO_PKG_VERSION")
    });

    if let Some(metrics) = &state.metrics {
        // Update system metrics
        metrics.update_system_metrics(uptime);

        // Collect current metric values
        let http_requests = metrics.http_requests_total.get();
        let http_in_flight = metrics.http_requests_in_flight.get();
        let db_queries = metrics.database_queries_total.get();
        let db_errors = metrics.database_errors_total.get();
        let external_requests = metrics.external_requests_total.get();
        let external_errors = metrics.external_errors_total.get();
        let circuit_breaker_state = metrics.circuit_breaker_state.get();

        // Database metrics
        let db_metrics = if let Ok(db_health) = check_database_metrics(&state).await {
            metrics.update_database_metrics(
                db_health.active_connections as i64,
                db_health.idle_connections as i64,
            );

            serde_json::json!({
                "active_connections": db_health.active_connections,
                "idle_connections": db_health.idle_connections,
                "max_connections": db_health.max_connections,
                "queries_total": db_queries,
                "errors_total": db_errors
            })
        } else {
            serde_json::json!({
                "status": "unavailable",
                "queries_total": db_queries,
                "errors_total": db_errors
            })
        };

        metrics_data["metrics"] = serde_json::json!({
            "http": {
                "requests_total": http_requests,
                "requests_in_flight": http_in_flight
            },
            "database": db_metrics,
            "external_services": {
                "requests_total": external_requests,
                "errors_total": external_errors,
                "circuit_breaker_state": circuit_breaker_state
            },
            "system": {
                "memory_usage_bytes": metrics.memory_usage_bytes.get(),
                "cpu_usage_percent": metrics.cpu_usage_percent.get()
            }
        });
    } else {
        metrics_data["error"] = serde_json::json!("Metrics not initialized");
    }

    let gather_duration = start_time.elapsed();
    metrics_data["collection_duration_ms"] = serde_json::json!(gather_duration.as_millis());

    info!("JSON metrics gathered in {:?}", gather_duration);

    axum::Json(metrics_data)
}

/// Simple database metrics check
async fn check_database_metrics(state: &AppState) -> Result<DatabaseMetrics, String> {
    let user_repository = state.services.user_repository();

    let start_time = Instant::now();

    // Try to perform a simple database operation to check connectivity
    match user_repository.count().await {
        Ok(_) => {
            let _response_time = start_time.elapsed();
            Ok(DatabaseMetrics {
                active_connections: 1, // Simplified - in real implementation we'd get actual stats
                idle_connections: 0,   // Simplified - in real implementation we'd get actual stats
                max_connections: 10,   // Simplified - in real implementation we'd get from config
            })
        }
        Err(e) => {
            Err(format!("Database metrics check failed: {}", e))
        }
    }
}

/// Simplified database metrics structure
#[derive(Debug, Clone)]
struct DatabaseMetrics {
    active_connections: u32,
    idle_connections: u32,
    max_connections: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_metrics_creation() {
        let metrics = DatabaseMetrics {
            active_connections: 5,
            idle_connections: 3,
            max_connections: 10,
        };

        assert_eq!(metrics.active_connections, 5);
        assert_eq!(metrics.idle_connections, 3);
        assert_eq!(metrics.max_connections, 10);
    }
}
