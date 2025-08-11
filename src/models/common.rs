use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Common ID types
pub type UserId = Uuid;

/// Common response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
    pub message: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            message: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_message(data: T, message: String) -> Self {
        Self {
            data,
            message: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Common error response
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ErrorResponse {
    pub fn new(error: String) -> Self {
        Self {
            error,
            details: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_details(error: String, details: String) -> Self {
        Self {
            error,
            details: Some(details),
            timestamp: chrono::Utc::now(),
        }
    }
}
