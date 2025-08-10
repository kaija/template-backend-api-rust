use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;

use crate::models::{User, CreateUserRequest, UpdateUserRequest, UserId, ApiResponse};
use crate::web::{responses::AppError, router::AppState};

/// Query parameters for listing users
#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_active: Option<bool>,
}

fn default_limit() -> i64 {
    20
}

impl ListUsersQuery {
    /// Validate query parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.limit < 1 || self.limit > 100 {
            return Err("Limit must be between 1 and 100".to_string());
        }
        
        if self.offset < 0 {
            return Err("Offset must be non-negative".to_string());
        }
        
        if let Some(name) = &self.name {
            if name.trim().is_empty() {
                return Err("Name filter cannot be empty".to_string());
            }
            if name.len() > 255 {
                return Err("Name filter cannot exceed 255 characters".to_string());
            }
        }
        
        if let Some(email) = &self.email {
            if email.trim().is_empty() {
                return Err("Email filter cannot be empty".to_string());
            }
            if !email.contains('@') {
                return Err("Email filter must be a valid email format".to_string());
            }
        }
        
        Ok(())
    }
}

/// Create a new user
pub async fn create_user(
    State(app_state): State<AppState>,
    Json(request): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<ApiResponse<User>>), AppError> {
    tracing::info!("Creating new user with email: {}", request.email);
    
    // Validate and normalize the request
    let validated_request = match request.validate_and_normalize() {
        Ok(req) => req,
        Err(validation_errors) => {
            let error_message = validation_errors
                .into_iter()
                .map(|(field, errors)| format!("{}: {}", field, errors.join(", ")))
                .collect::<Vec<_>>()
                .join("; ");
            tracing::warn!("User creation validation failed: {}", error_message);
            return Err(AppError::Validation(error_message));
        }
    };
    
    let user = app_state.user_service.create_user(validated_request).await?;
    
    tracing::info!("Successfully created user with ID: {}", user.id);
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(user, "User created successfully".to_string())),
    ))
}

/// Get a user by ID
pub async fn get_user(
    State(app_state): State<AppState>,
    Path(user_id): Path<UserId>,
) -> Result<Json<ApiResponse<User>>, AppError> {
    tracing::debug!("Getting user with ID: {}", user_id);
    
    let user = app_state.user_service.get_user(user_id).await?;
    
    tracing::info!("Successfully retrieved user: {}", user_id);
    Ok(Json(ApiResponse::new(user)))
}

/// Update a user
pub async fn update_user(
    State(app_state): State<AppState>,
    Path(user_id): Path<UserId>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<User>>, AppError> {
    tracing::info!("Updating user with ID: {}", user_id);
    
    // Validate and normalize the request
    let validated_request = match request.validate_and_normalize() {
        Ok(req) => req,
        Err(validation_errors) => {
            let error_message = validation_errors
                .into_iter()
                .map(|(field, errors)| format!("{}: {}", field, errors.join(", ")))
                .collect::<Vec<_>>()
                .join("; ");
            tracing::warn!("User update validation failed for user {}: {}", user_id, error_message);
            return Err(AppError::Validation(error_message));
        }
    };
    
    // Check if there are any updates to apply
    if !validated_request.has_updates() {
        tracing::warn!("No updates provided for user: {}", user_id);
        return Err(AppError::Validation("No updates provided".to_string()));
    }
    
    let user = app_state.user_service.update_user(user_id, validated_request).await?;
    
    tracing::info!("Successfully updated user: {}", user_id);
    Ok(Json(ApiResponse::with_message(user, "User updated successfully".to_string())))
}

/// Delete a user
pub async fn delete_user(
    State(app_state): State<AppState>,
    Path(user_id): Path<UserId>,
) -> Result<StatusCode, AppError> {
    tracing::info!("Deleting user with ID: {}", user_id);
    
    app_state.user_service.delete_user(user_id).await?;
    
    tracing::info!("Successfully deleted user: {}", user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// List users with pagination
pub async fn list_users(
    State(app_state): State<AppState>,
    Query(query): Query<ListUsersQuery>,
) -> Result<Json<ApiResponse<Vec<User>>>, AppError> {
    tracing::debug!("Listing users with limit: {}, offset: {}", query.limit, query.offset);
    
    // Validate query parameters
    if let Err(validation_error) = query.validate() {
        tracing::warn!("Invalid query parameters for list users: {}", validation_error);
        return Err(AppError::Validation(validation_error));
    }
    
    let users = app_state.user_service.list_users(query.limit, query.offset).await?;
    
    tracing::info!("Successfully retrieved {} users", users.len());
    Ok(Json(ApiResponse::new(users)))
}