use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::models::{User, CreateUserRequest, UpdateUserRequest, UserId, ApiResponse};
use crate::services::{UserService, ServiceError};
use crate::web::responses::AppError;

/// Query parameters for listing users
#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

/// Create a new user
pub async fn create_user(
    State(user_service): State<Arc<dyn UserService>>,
    Json(request): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<ApiResponse<User>>), AppError> {
    let user = user_service.create_user(request).await?;
    
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(user, "User created successfully".to_string())),
    ))
}

/// Get a user by ID
pub async fn get_user(
    State(user_service): State<Arc<dyn UserService>>,
    Path(user_id): Path<UserId>,
) -> Result<Json<ApiResponse<User>>, AppError> {
    let user = user_service.get_user(user_id).await?;
    
    Ok(Json(ApiResponse::new(user)))
}

/// Update a user
pub async fn update_user(
    State(user_service): State<Arc<dyn UserService>>,
    Path(user_id): Path<UserId>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<User>>, AppError> {
    let user = user_service.update_user(user_id, request).await?;
    
    Ok(Json(ApiResponse::with_message(user, "User updated successfully".to_string())))
}

/// Delete a user
pub async fn delete_user(
    State(user_service): State<Arc<dyn UserService>>,
    Path(user_id): Path<UserId>,
) -> Result<StatusCode, AppError> {
    user_service.delete_user(user_id).await?;
    
    Ok(StatusCode::NO_CONTENT)
}

/// List users with pagination
pub async fn list_users(
    State(user_service): State<Arc<dyn UserService>>,
    Query(query): Query<ListUsersQuery>,
) -> Result<Json<ApiResponse<Vec<User>>>, AppError> {
    let users = user_service.list_users(query.limit, query.offset).await?;
    
    Ok(Json(ApiResponse::new(users)))
}