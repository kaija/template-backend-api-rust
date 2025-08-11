use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::{info, warn};

use crate::{
    metrics::AppMetrics,
    web::router::AppState,
};

/// Metrics middleware to track HTTP request metrics
pub async fn metrics_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();

    // Increment in-flight requests if metrics are available
    if let Some(metrics) = &state.metrics {
        metrics.http_requests_in_flight.inc();
    }

    // Process the request
    let response = next.run(request).await;

    // Calculate request duration
    let duration = start_time.elapsed();
    let status = response.status();

    // Record metrics if available
    if let Some(metrics) = &state.metrics {
        // Decrement in-flight requests
        metrics.http_requests_in_flight.dec();

        // Record request metrics
        metrics.http_requests_total.inc();
        metrics.http_request_duration_seconds.observe(duration.as_secs_f64());

        info!(
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = duration.as_millis(),
            "HTTP request completed"
        );
    } else {
        warn!("Metrics not available for request tracking");
    }

    response
}

/// Middleware to track database query metrics
/// This would typically be integrated into the repository layer
pub struct DatabaseMetricsTracker {
    metrics: Option<AppMetrics>,
}

impl DatabaseMetricsTracker {
    pub fn new(metrics: Option<AppMetrics>) -> Self {
        Self { metrics }
    }

    /// Record a database query execution
    pub fn record_query(&self, duration: std::time::Duration, success: bool) {
        if let Some(metrics) = &self.metrics {
            metrics.record_database_query(duration.as_secs_f64(), success);
        }
    }
}

/// Middleware to track external service request metrics
/// This would typically be integrated into the external service layer
pub struct ExternalServiceMetricsTracker {
    metrics: Option<AppMetrics>,
}

impl ExternalServiceMetricsTracker {
    pub fn new(metrics: Option<AppMetrics>) -> Self {
        Self { metrics }
    }

    /// Record an external service request
    pub fn record_request(&self, duration: std::time::Duration, success: bool) {
        if let Some(metrics) = &self.metrics {
            metrics.record_external_request(duration.as_secs_f64(), success);
        }
    }

    /// Update circuit breaker state
    pub fn update_circuit_breaker_state(&self, state: i64) {
        if let Some(metrics) = &self.metrics {
            metrics.update_circuit_breaker_state(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::AppMetrics;

    #[test]
    fn test_database_metrics_tracker() {
        let metrics = AppMetrics::new().expect("Failed to create metrics");
        let tracker = DatabaseMetricsTracker::new(Some(metrics.clone()));

        // Record a successful query
        tracker.record_query(std::time::Duration::from_millis(100), true);
        assert_eq!(metrics.database_queries_total.get(), 1);
        assert_eq!(metrics.database_errors_total.get(), 0);

        // Record a failed query
        tracker.record_query(std::time::Duration::from_millis(200), false);
        assert_eq!(metrics.database_queries_total.get(), 2);
        assert_eq!(metrics.database_errors_total.get(), 1);
    }

    #[test]
    fn test_external_service_metrics_tracker() {
        let metrics = AppMetrics::new().expect("Failed to create metrics");
        let tracker = ExternalServiceMetricsTracker::new(Some(metrics.clone()));

        // Record a successful request
        tracker.record_request(std::time::Duration::from_millis(500), true);
        assert_eq!(metrics.external_requests_total.get(), 1);
        assert_eq!(metrics.external_errors_total.get(), 0);

        // Record a failed request
        tracker.record_request(std::time::Duration::from_millis(1000), false);
        assert_eq!(metrics.external_requests_total.get(), 2);
        assert_eq!(metrics.external_errors_total.get(), 1);

        // Update circuit breaker state
        tracker.update_circuit_breaker_state(1); // Open
        assert_eq!(metrics.circuit_breaker_state.get(), 1);
    }

    #[test]
    fn test_trackers_with_no_metrics() {
        let db_tracker = DatabaseMetricsTracker::new(None);
        let ext_tracker = ExternalServiceMetricsTracker::new(None);

        // Should not panic when metrics are not available
        db_tracker.record_query(std::time::Duration::from_millis(100), true);
        ext_tracker.record_request(std::time::Duration::from_millis(500), true);
        ext_tracker.update_circuit_breaker_state(0);
    }
}
