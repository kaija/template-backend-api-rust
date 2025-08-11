# Error Handling with Context and Correlation

This module provides comprehensive error handling with correlation IDs and structured context for debugging and monitoring.

## Features

- **Comprehensive Error Hierarchy**: All error types are covered with proper HTTP status code mapping
- **Correlation ID Support**: Automatic correlation ID propagation for request tracing
- **Structured Logging**: Rich context information in logs for debugging
- **Sentry Integration**: Automatic error capture with context for monitoring
- **Client-Safe Responses**: Sensitive information is filtered from client responses

## Basic Usage

### Using AppError (Standard)

```rust
use crate::web::responses::AppError;

// Simple error creation
let error = AppError::validation("Invalid email format");
let error = AppError::not_found("User not found");
let error = AppError::internal();

// From trait conversions
let db_error: sqlx::Error = /* ... */;
let app_error: AppError = db_error.into();
```

### Using ContextualAppError (Enhanced)

```rust
use crate::web::responses::{AppError, ContextualAppError, ErrorContext, IntoContextualError};

// Create error with context
let context = ErrorContext::new()
    .with_correlation_id("req-123")
    .with_request_path("/api/users")
    .with_request_method("POST")
    .with_user_id("user-456")
    .with_metadata("operation", "create_user");

let contextual_error = AppError::validation("Invalid input")
    .with_context(context);

// Or use the helper method
let contextual_error = AppError::validation("Invalid input")
    .with_correlation_id("req-123".to_string());
```

## Handler Example

```rust
use axum::{extract::State, response::Json};
use crate::web::responses::{AppError, ContextualAppError, ErrorContext, IntoContextualError};

pub async fn create_user_with_context(
    State(app_state): State<AppState>,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<User>, ContextualAppError> {
    // Extract correlation ID from request (set by middleware)
    let correlation_id = /* extract from request extensions */;

    // Create error context
    let context = ErrorContext::new()
        .with_correlation_id(correlation_id)
        .with_request_path("/api/users")
        .with_request_method("POST")
        .with_metadata("email", request.email.clone());

    // Use contextual error handling
    let user = app_state.user_service
        .create_user(request)
        .await
        .map_err(|e| AppError::Service(e).with_context(context.clone()))?;

    Ok(Json(user))
}
```

## Middleware Integration

The error context middleware automatically extracts correlation IDs and request information:

```rust
use crate::web::responses::error_context_middleware;

// Add to your router
let app = Router::new()
    .route("/api/users", post(create_user))
    .layer(axum::middleware::from_fn(error_context_middleware));
```

## Error Response Format

### Standard Error Response
```json
{
  "error": "Validation error",
  "details": "Email is required",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### Contextual Error Response (Client-Safe)
```json
{
  "error": "Validation error",
  "details": "Email is required",
  "context": {
    "correlation_id": "req-123",
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

## Logging Output

Contextual errors produce structured logs:

```json
{
  "level": "ERROR",
  "message": "Server error occurred",
  "error": "Database connection failed",
  "error_category": "database",
  "correlation_id": "req-123",
  "request_path": "/api/users",
  "request_method": "POST",
  "user_id": "user-456",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

## Sentry Integration

Server errors are automatically captured in Sentry with full context:

- Correlation ID as tag
- Request path and method as tags
- User ID in user context
- Full error context as extra data

## Error Categories

The system categorizes errors for monitoring:

- `configuration` - Configuration errors
- `database` - Database connectivity issues
- `repository` - Data access errors
- `service` - Business logic errors
- `validation` - Input validation errors
- `authentication` - Auth failures
- `authorization` - Permission errors
- `not_found` - Resource not found
- `conflict` - Resource conflicts
- `external_service` - External API errors
- `http_client` - HTTP client errors
- `serialization` - JSON/data serialization errors
- `io` - File system errors
- `timeout` - Operation timeouts
- `rate_limit` - Rate limiting errors
- `internal` - Internal server errors
- `generic` - Generic errors

## Best Practices

1. **Use correlation IDs**: Always include correlation IDs for request tracing
2. **Add relevant metadata**: Include operation-specific context (user ID, resource ID, etc.)
3. **Log at appropriate levels**: Client errors as WARN, server errors as ERROR
4. **Filter sensitive data**: Use client-safe responses to avoid data leaks
5. **Monitor error patterns**: Use error categories for alerting and metrics
