use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Deserializer};
use validator::{Validate, ValidationError};
use std::collections::HashMap;

use super::common::UserId;

/// User domain model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new user
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(length(min = 1, max = 255, message = "Name must be between 1 and 255 characters"))]
    #[validate(custom(function = "validate_name"))]
    #[serde(deserialize_with = "deserialize_trimmed_string")]
    pub name: String,

    #[validate(email(message = "Invalid email format"))]
    #[validate(length(max = 320, message = "Email must not exceed 320 characters"))]
    #[serde(deserialize_with = "deserialize_trimmed_lowercase_string")]
    pub email: String,
}

/// Request to update an existing user
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(min = 1, max = 255, message = "Name must be between 1 and 255 characters"))]
    #[serde(deserialize_with = "deserialize_optional_trimmed_string")]
    pub name: Option<String>,

    #[validate(email(message = "Invalid email format"))]
    #[validate(length(max = 320, message = "Email must not exceed 320 characters"))]
    #[serde(deserialize_with = "deserialize_optional_trimmed_lowercase_string")]
    pub email: Option<String>,
}

/// User for database insertion
#[derive(Debug)]
pub struct NewUser {
    pub name: String,
    pub email: String,
}

impl From<CreateUserRequest> for NewUser {
    fn from(request: CreateUserRequest) -> Self {
        Self {
            name: request.name,
            email: request.email,
        }
    }
}

/// User list response with pagination metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct UserListResponse {
    pub users: Vec<User>,
    pub pagination: PaginationMetadata,
}

/// Pagination metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationMetadata {
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub has_more: bool,
}

/// User statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct UserStats {
    pub total_users: i64,
    pub active_users: i64,
    pub inactive_users: i64,
    pub users_created_today: i64,
    pub users_created_this_week: i64,
    pub users_created_this_month: i64,
}

/// User search filters
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UserSearchFilters {
    #[validate(length(min = 1, max = 255, message = "Name filter must be between 1 and 255 characters"))]
    pub name: Option<String>,

    #[validate(email(message = "Invalid email format for filter"))]
    pub email: Option<String>,

    pub is_active: Option<bool>,

    #[validate(range(min = 1, max = 1000, message = "Limit must be between 1 and 1000"))]
    pub limit: Option<i64>,

    #[validate(range(min = 0, message = "Offset must be non-negative"))]
    pub offset: Option<i64>,
}

impl Default for UserSearchFilters {
    fn default() -> Self {
        Self {
            name: None,
            email: None,
            is_active: None,
            limit: Some(20),
            offset: Some(0),
        }
    }
}

/// User activation/deactivation request
#[derive(Debug, Serialize, Deserialize)]
pub struct UserStatusRequest {
    pub is_active: bool,
    pub reason: Option<String>,
}

/// Validation functions
fn validate_name(name: &str) -> Result<(), ValidationError> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err(ValidationError::new("Name cannot be empty or only whitespace"));
    }

    if trimmed.len() < 2 {
        return Err(ValidationError::new("Name must be at least 2 characters long"));
    }

    // Check for invalid characters
    if trimmed.chars().any(|c| c.is_control() || c == '\n' || c == '\r' || c == '\t') {
        return Err(ValidationError::new("Name contains invalid characters"));
    }

    // Check for excessive whitespace
    if trimmed.contains("  ") {
        return Err(ValidationError::new("Name cannot contain consecutive spaces"));
    }

    Ok(())
}



/// Custom deserializers for data cleaning
fn deserialize_trimmed_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s.trim().to_string())
}

fn deserialize_trimmed_lowercase_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s.trim().to_lowercase())
}

fn deserialize_optional_trimmed_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
}

fn deserialize_optional_trimmed_lowercase_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()))
}

/// User model extensions
impl User {
    /// Check if user is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Get user's display name (for UI purposes)
    pub fn display_name(&self) -> &str {
        &self.name
    }

    /// Get user's email domain
    pub fn email_domain(&self) -> Option<&str> {
        self.email.split('@').nth(1)
    }

    /// Check if user was created recently (within last 24 hours)
    pub fn is_recently_created(&self) -> bool {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.created_at);
        duration.num_hours() < 24
    }

    /// Check if user was updated recently (within last hour)
    pub fn is_recently_updated(&self) -> bool {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.updated_at);
        duration.num_minutes() < 60
    }

    /// Convert to a safe representation (without sensitive data)
    pub fn to_safe_user(&self) -> SafeUser {
        SafeUser {
            id: self.id,
            name: self.name.clone(),
            email: self.email.clone(),
            is_active: self.is_active,
            created_at: self.created_at,
        }
    }
}

/// Safe user representation (for public APIs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeUser {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// User creation validation
impl CreateUserRequest {
    /// Validate and normalize the request
    pub fn validate_and_normalize(mut self) -> Result<Self, HashMap<String, Vec<String>>> {
        // Normalize data
        self.name = self.name.trim().to_string();
        self.email = self.email.trim().to_lowercase();

        // Validate
        match self.validate() {
            Ok(()) => Ok(self),
            Err(validation_errors) => {
                let mut errors = HashMap::new();
                for (field, field_errors) in validation_errors.field_errors() {
                    let error_messages: Vec<String> = field_errors
                        .iter()
                        .map(|e| e.message.as_ref().unwrap_or(&std::borrow::Cow::Borrowed("Invalid value")).to_string())
                        .collect();
                    errors.insert(field.to_string(), error_messages);
                }
                Err(errors)
            }
        }
    }
}

/// User update validation
impl UpdateUserRequest {
    /// Validate and normalize the request
    pub fn validate_and_normalize(mut self) -> Result<Self, HashMap<String, Vec<String>>> {
        // Normalize data
        if let Some(name) = &mut self.name {
            *name = name.trim().to_string();
            if name.is_empty() {
                self.name = None;
            }
        }

        if let Some(email) = &mut self.email {
            *email = email.trim().to_lowercase();
            if email.is_empty() {
                self.email = None;
            }
        }

        // Validate
        match self.validate() {
            Ok(()) => Ok(self),
            Err(validation_errors) => {
                let mut errors = HashMap::new();
                for (field, field_errors) in validation_errors.field_errors() {
                    let error_messages: Vec<String> = field_errors
                        .iter()
                        .map(|e| e.message.as_ref().unwrap_or(&std::borrow::Cow::Borrowed("Invalid value")).to_string())
                        .collect();
                    errors.insert(field.to_string(), error_messages);
                }
                Err(errors)
            }
        }
    }

    /// Check if the request has any updates
    pub fn has_updates(&self) -> bool {
        self.name.is_some() || self.email.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_create_user_request_validation() {
        let valid_request = CreateUserRequest {
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        assert!(valid_request.validate().is_ok());
    }

    #[test]
    fn test_create_user_request_invalid_email() {
        let invalid_request = CreateUserRequest {
            name: "John Doe".to_string(),
            email: "invalid-email".to_string(),
        };

        assert!(invalid_request.validate().is_err());
    }

    #[test]
    fn test_create_user_request_empty_name() {
        let invalid_request = CreateUserRequest {
            name: "".to_string(),
            email: "john@example.com".to_string(),
        };

        assert!(invalid_request.validate().is_err());
    }

    #[test]
    fn test_update_user_request_validation() {
        let valid_request = UpdateUserRequest {
            name: Some("Jane Doe".to_string()),
            email: Some("jane@example.com".to_string()),
        };

        assert!(valid_request.validate().is_ok());
    }

    #[test]
    fn test_update_user_request_no_updates() {
        let request = UpdateUserRequest {
            name: None,
            email: None,
        };

        assert!(!request.has_updates());
    }

    #[test]
    fn test_user_model_methods() {
        let user = User {
            id: Uuid::new_v4(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(user.is_active());
        assert_eq!(user.display_name(), "Test User");
        assert_eq!(user.email_domain(), Some("example.com"));
        assert!(user.is_recently_created());
        assert!(user.is_recently_updated());
    }

    #[test]
    fn test_safe_user_conversion() {
        let user = User {
            id: Uuid::new_v4(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let safe_user = user.to_safe_user();
        assert_eq!(safe_user.id, user.id);
        assert_eq!(safe_user.name, user.name);
        assert_eq!(safe_user.email, user.email);
        assert_eq!(safe_user.is_active, user.is_active);
        assert_eq!(safe_user.created_at, user.created_at);
    }

    #[test]
    fn test_name_validation() {
        assert!(validate_name("John Doe").is_ok());
        assert!(validate_name("A").is_err()); // Too short
        assert!(validate_name("").is_err()); // Empty
        assert!(validate_name("  ").is_err()); // Only whitespace
        assert!(validate_name("John  Doe").is_err()); // Consecutive spaces
        assert!(validate_name("John\nDoe").is_err()); // Control character
    }

    #[test]
    fn test_user_search_filters_default() {
        let filters = UserSearchFilters::default();
        assert_eq!(filters.limit, Some(20));
        assert_eq!(filters.offset, Some(0));
        assert!(filters.name.is_none());
        assert!(filters.email.is_none());
        assert!(filters.is_active.is_none());
    }

    #[test]
    fn test_pagination_metadata() {
        let metadata = PaginationMetadata {
            total: 100,
            limit: 20,
            offset: 0,
            has_more: true,
        };

        assert_eq!(metadata.total, 100);
        assert_eq!(metadata.limit, 20);
        assert_eq!(metadata.offset, 0);
        assert!(metadata.has_more);
    }
}
