use serde::{Deserialize, Serialize};
use validator::Validate;

/// Authentication request
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AuthRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}

/// Authentication response
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Current user context
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: crate::models::UserId,
    pub email: String,
    pub name: String,
}