use axum::{
    http::StatusCode,
    middleware,
    response::{IntoResponse, Json},
    routing::{get, post, put, delete},
    Router,
};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
    timeout::TimeoutLayer,
    compression::CompressionLayer,
    request_id::{MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
};
use uuid::Uuid;

use crate::{
    config::AppConfig,
    services::{AuthService, UserService},
    web::{
        handlers::{health_handlers, user_handlers},
        middleware::request_id_middleware,
    },
};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub user_service: Arc<dyn UserService>,
    pub auth_service: Arc<dyn AuthService>,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        user_service: Arc<dyn UserService>,
        auth_service: Arc<dyn AuthService>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            user_service,
            auth_service,
        }
    }
}

/// Custom request ID generator using UUID v4
#[derive(Clone, Default)]
pub struct UuidMakeRequestId;

impl MakeRequestId for UuidMakeRequestId {
    fn make_request_id<B>(&mut self, _request: &axum::http::Request<B>) -> Option<RequestId> {
        let request_id = Uuid::new_v4().to_string().parse().ok()?;
        Some(RequestId::new(request_id))
    }
}

/// Create the main application router with middleware stack
pub fn create_router(state: AppState) -> Router {
    // Create API routes
    let api_routes = create_api_routes();
    
    // Create health check routes
    let health_routes = create_health_routes();
    
    // Build the main router with nested routes and middleware
    Router::new()
        .nest("/api/v1", api_routes)
        .nest("/health", health_routes)
        .layer(
            ServiceBuilder::new()
                // Request ID generation and propagation (outermost)
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(SetRequestIdLayer::x_request_id(UuidMakeRequestId::default()))
                
                // Custom request ID middleware for correlation
                .layer(middleware::from_fn(request_id_middleware))
                
                // Tracing layer for request/response logging
                .layer(TraceLayer::new_for_http())
                
                // Response compression
                .layer(CompressionLayer::new())
                
                // Request timeout (30 seconds)
                .layer(TimeoutLayer::new(std::time::Duration::from_secs(30)))
                
                // CORS handling
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_methods(Any)
                        .allow_headers(Any)
                )
                
                // Note: Rate limiting is handled at the load balancer level
        )
        .with_state(state)
        .fallback(not_found_handler)
}

/// Create API v1 routes
fn create_api_routes() -> Router<AppState> {
    Router::new()
        .nest("/users", create_user_routes())
        // Add more API route groups here as needed
}

/// Create user management routes
fn create_user_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(user_handlers::create_user))
        .route("/", get(user_handlers::list_users))
        .route("/:id", get(user_handlers::get_user))
        .route("/:id", put(user_handlers::update_user))
        .route("/:id", delete(user_handlers::delete_user))
        // Note: Authentication middleware will be applied at the router level
        // Individual routes can use the CurrentUser extractor to require authentication
}

/// Create health check routes
fn create_health_routes() -> Router<AppState> {
    Router::new()
        .route("/live", get(health_handlers::liveness))
        .route("/ready", get(health_handlers::readiness))
        .route("/", get(health_handlers::health))
}



/// Fallback handler for 404 responses
pub async fn not_found_handler() -> impl IntoResponse {
    let error_response = json!({
        "error": "Not Found",
        "message": "The requested resource was not found",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    (StatusCode::NOT_FOUND, Json(error_response))
}

/// Handle middleware errors
pub async fn handle_middleware_error(error: tower::BoxError) -> impl IntoResponse {
    tracing::error!("Middleware error: {:?}", error);
    
    let error_response = json!({
        "error": "Internal Server Error",
        "message": "An internal error occurred while processing the request",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    (StatusCode::INTERNAL_SERVER_ERROR, Json(error_response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use std::sync::Arc;
    
    // TODO: Implement mock services for testing
    // For now, we'll skip the tests that require services
    
    #[tokio::test]
    async fn test_not_found_handler() {
        let response = not_found_handler().await;
        // This test doesn't require the full router setup
        // Just testing the handler function directly
    }
}