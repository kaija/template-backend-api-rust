use prometheus::{
    Gauge, Histogram, HistogramOpts, IntCounter, IntGauge, Opts, Registry,
};
use std::sync::Arc;
use tracing::{info, warn};

/// Application metrics collector
#[derive(Clone)]
pub struct AppMetrics {
    registry: Arc<Registry>,

    // HTTP metrics
    pub http_requests_total: IntCounter,
    pub http_request_duration_seconds: Histogram,
    pub http_requests_in_flight: IntGauge,

    // Database metrics
    pub database_connections_active: IntGauge,
    pub database_connections_idle: IntGauge,
    pub database_query_duration_seconds: Histogram,
    pub database_queries_total: IntCounter,
    pub database_errors_total: IntCounter,

    // External service metrics
    pub external_requests_total: IntCounter,
    pub external_request_duration_seconds: Histogram,
    pub external_errors_total: IntCounter,
    pub circuit_breaker_state: IntGauge,

    // Application metrics
    pub application_info: IntGauge,
    pub application_uptime_seconds: Gauge,
    pub memory_usage_bytes: Gauge,
    pub cpu_usage_percent: Gauge,
}

impl AppMetrics {
    /// Create a new metrics collector with all metrics registered
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Arc::new(Registry::new());

        // HTTP metrics
        let http_requests_total = IntCounter::with_opts(Opts::new(
            "http_requests_total",
            "Total number of HTTP requests processed"
        ).const_label("service", "rust-api"))?;

        let http_request_duration_seconds = Histogram::with_opts(HistogramOpts::new(
            "http_request_duration_seconds",
            "HTTP request duration in seconds"
        ).const_label("service", "rust-api")
        .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]))?;

        let http_requests_in_flight = IntGauge::with_opts(Opts::new(
            "http_requests_in_flight",
            "Number of HTTP requests currently being processed"
        ).const_label("service", "rust-api"))?;

        // Database metrics
        let database_connections_active = IntGauge::with_opts(Opts::new(
            "database_connections_active",
            "Number of active database connections"
        ).const_label("service", "rust-api"))?;

        let database_connections_idle = IntGauge::with_opts(Opts::new(
            "database_connections_idle",
            "Number of idle database connections"
        ).const_label("service", "rust-api"))?;

        let database_query_duration_seconds = Histogram::with_opts(HistogramOpts::new(
            "database_query_duration_seconds",
            "Database query duration in seconds"
        ).const_label("service", "rust-api")
        .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]))?;

        let database_queries_total = IntCounter::with_opts(Opts::new(
            "database_queries_total",
            "Total number of database queries executed"
        ).const_label("service", "rust-api"))?;

        let database_errors_total = IntCounter::with_opts(Opts::new(
            "database_errors_total",
            "Total number of database errors"
        ).const_label("service", "rust-api"))?;

        // External service metrics
        let external_requests_total = IntCounter::with_opts(Opts::new(
            "external_requests_total",
            "Total number of external service requests"
        ).const_label("service", "rust-api"))?;

        let external_request_duration_seconds = Histogram::with_opts(HistogramOpts::new(
            "external_request_duration_seconds",
            "External service request duration in seconds"
        ).const_label("service", "rust-api")
        .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]))?;

        let external_errors_total = IntCounter::with_opts(Opts::new(
            "external_errors_total",
            "Total number of external service errors"
        ).const_label("service", "rust-api"))?;

        let circuit_breaker_state = IntGauge::with_opts(Opts::new(
            "circuit_breaker_state",
            "Circuit breaker state (0=closed, 1=open, 2=half-open)"
        ).const_label("service", "rust-api"))?;

        // Application metrics
        let application_info = IntGauge::with_opts(Opts::new(
            "application_info",
            "Application information"
        ).const_label("service", "rust-api")
        .const_label("version", env!("CARGO_PKG_VERSION"))
        .const_label("rust_version", env!("RUSTC_VERSION"))
        .const_label("build_timestamp", env!("BUILD_TIMESTAMP")))?;

        let application_uptime_seconds = Gauge::with_opts(Opts::new(
            "application_uptime_seconds",
            "Application uptime in seconds"
        ).const_label("service", "rust-api"))?;

        let memory_usage_bytes = Gauge::with_opts(Opts::new(
            "memory_usage_bytes",
            "Memory usage in bytes"
        ).const_label("service", "rust-api"))?;

        let cpu_usage_percent = Gauge::with_opts(Opts::new(
            "cpu_usage_percent",
            "CPU usage percentage"
        ).const_label("service", "rust-api"))?;

        // Register all metrics
        registry.register(Box::new(http_requests_total.clone()))?;
        registry.register(Box::new(http_request_duration_seconds.clone()))?;
        registry.register(Box::new(http_requests_in_flight.clone()))?;
        registry.register(Box::new(database_connections_active.clone()))?;
        registry.register(Box::new(database_connections_idle.clone()))?;
        registry.register(Box::new(database_query_duration_seconds.clone()))?;
        registry.register(Box::new(database_queries_total.clone()))?;
        registry.register(Box::new(database_errors_total.clone()))?;
        registry.register(Box::new(external_requests_total.clone()))?;
        registry.register(Box::new(external_request_duration_seconds.clone()))?;
        registry.register(Box::new(external_errors_total.clone()))?;
        registry.register(Box::new(circuit_breaker_state.clone()))?;
        registry.register(Box::new(application_info.clone()))?;
        registry.register(Box::new(application_uptime_seconds.clone()))?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;
        registry.register(Box::new(cpu_usage_percent.clone()))?;

        // Set application info to 1 (constant)
        application_info.set(1);

        info!("Metrics registry initialized with {} metrics", registry.gather().len());

        Ok(Self {
            registry,
            http_requests_total,
            http_request_duration_seconds,
            http_requests_in_flight,
            database_connections_active,
            database_connections_idle,
            database_query_duration_seconds,
            database_queries_total,
            database_errors_total,
            external_requests_total,
            external_request_duration_seconds,
            external_errors_total,
            circuit_breaker_state,
            application_info,
            application_uptime_seconds,
            memory_usage_bytes,
            cpu_usage_percent,
        })
    }

    /// Get the Prometheus registry for metrics collection
    pub fn registry(&self) -> Arc<Registry> {
        self.registry.clone()
    }

    /// Update system metrics (memory, CPU, uptime)
    pub fn update_system_metrics(&self, uptime_seconds: f64) {
        self.application_uptime_seconds.set(uptime_seconds);

        // Update memory usage (simplified - in production you'd use a proper system metrics library)
        if let Ok(memory_info) = get_memory_usage() {
            self.memory_usage_bytes.set(memory_info as f64);
        }

        // Update CPU usage (simplified - in production you'd use a proper system metrics library)
        if let Ok(cpu_usage) = get_cpu_usage() {
            self.cpu_usage_percent.set(cpu_usage);
        }
    }

    /// Update database connection metrics
    pub fn update_database_metrics(&self, active: i64, idle: i64) {
        self.database_connections_active.set(active);
        self.database_connections_idle.set(idle);
    }

    /// Record a database query
    pub fn record_database_query(&self, duration_seconds: f64, success: bool) {
        self.database_queries_total.inc();
        self.database_query_duration_seconds.observe(duration_seconds);

        if !success {
            self.database_errors_total.inc();
        }
    }

    /// Record an external service request
    pub fn record_external_request(&self, duration_seconds: f64, success: bool) {
        self.external_requests_total.inc();
        self.external_request_duration_seconds.observe(duration_seconds);

        if !success {
            self.external_errors_total.inc();
        }
    }

    /// Update circuit breaker state (0=closed, 1=open, 2=half-open)
    pub fn update_circuit_breaker_state(&self, state: i64) {
        self.circuit_breaker_state.set(state);
    }

    /// Get metrics as Prometheus text format
    pub fn gather(&self) -> String {
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();

        match encoder.encode_to_string(&metric_families) {
            Ok(output) => output,
            Err(e) => {
                warn!("Failed to encode metrics: {}", e);
                String::new()
            }
        }
    }
}

impl Default for AppMetrics {
    fn default() -> Self {
        Self::new().expect("Failed to create default metrics")
    }
}

/// Get current memory usage in bytes (simplified implementation)
fn get_memory_usage() -> Result<u64, std::io::Error> {
    // This is a simplified implementation
    // In production, you'd use a proper system metrics library like `sysinfo`
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let status = fs::read_to_string("/proc/self/status")?;
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        return Ok(kb * 1024); // Convert KB to bytes
                    }
                }
            }
        }
    }

    // Fallback for non-Linux systems or if reading fails
    Ok(0)
}

/// Get current CPU usage percentage (simplified implementation)
fn get_cpu_usage() -> Result<f64, std::io::Error> {
    // This is a simplified implementation that returns 0
    // In production, you'd use a proper system metrics library like `sysinfo`
    // or implement proper CPU usage calculation
    Ok(0.0)
}

/// Metrics middleware for HTTP requests
pub struct MetricsMiddleware {
    metrics: AppMetrics,
}

impl MetricsMiddleware {
    pub fn new(metrics: AppMetrics) -> Self {
        Self { metrics }
    }

    /// Record HTTP request metrics
    pub fn record_request(&self, duration_seconds: f64) {
        self.metrics.http_requests_total.inc();
        self.metrics.http_request_duration_seconds.observe(duration_seconds);
    }

    /// Increment in-flight requests
    pub fn increment_in_flight(&self) {
        self.metrics.http_requests_in_flight.inc();
    }

    /// Decrement in-flight requests
    pub fn decrement_in_flight(&self) {
        self.metrics.http_requests_in_flight.dec();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = AppMetrics::new().expect("Failed to create metrics");
        assert_eq!(metrics.http_requests_total.get(), 0);
        assert_eq!(metrics.application_info.get(), 1);
    }

    #[test]
    fn test_metrics_recording() {
        let metrics = AppMetrics::new().expect("Failed to create metrics");

        // Record some metrics
        metrics.record_database_query(0.1, true);
        metrics.record_external_request(0.5, false);

        assert_eq!(metrics.database_queries_total.get(), 1);
        assert_eq!(metrics.external_requests_total.get(), 1);
        assert_eq!(metrics.external_errors_total.get(), 1);
    }

    #[test]
    fn test_metrics_gathering() {
        let metrics = AppMetrics::new().expect("Failed to create metrics");
        let output = metrics.gather();

        // Should contain some metric names
        assert!(output.contains("http_requests_total"));
        assert!(output.contains("application_info"));
    }

    #[test]
    fn test_system_metrics_update() {
        let metrics = AppMetrics::new().expect("Failed to create metrics");
        metrics.update_system_metrics(123.45);

        assert_eq!(metrics.application_uptime_seconds.get(), 123.45);
    }

    #[test]
    fn test_database_metrics_update() {
        let metrics = AppMetrics::new().expect("Failed to create metrics");
        metrics.update_database_metrics(5, 3);

        assert_eq!(metrics.database_connections_active.get(), 5);
        assert_eq!(metrics.database_connections_idle.get(), 3);
    }

    #[test]
    fn test_circuit_breaker_state_update() {
        let metrics = AppMetrics::new().expect("Failed to create metrics");

        // Test different states
        metrics.update_circuit_breaker_state(0); // Closed
        assert_eq!(metrics.circuit_breaker_state.get(), 0);

        metrics.update_circuit_breaker_state(1); // Open
        assert_eq!(metrics.circuit_breaker_state.get(), 1);

        metrics.update_circuit_breaker_state(2); // Half-open
        assert_eq!(metrics.circuit_breaker_state.get(), 2);
    }

    #[test]
    fn test_metrics_middleware() {
        let metrics = AppMetrics::new().expect("Failed to create metrics");
        let middleware = MetricsMiddleware::new(metrics.clone());

        // Test in-flight tracking
        middleware.increment_in_flight();
        assert_eq!(metrics.http_requests_in_flight.get(), 1);

        middleware.decrement_in_flight();
        assert_eq!(metrics.http_requests_in_flight.get(), 0);

        // Test request recording
        middleware.record_request(0.25);
        assert_eq!(metrics.http_requests_total.get(), 1);
    }
}
