use async_trait::async_trait;
use std::sync::Arc;

use crate::models::{User, CreateUserRequest, UpdateUserRequest, NewUser, UserId};
use crate::repository::{UserRepository, RepositoryError};

/// Service error types
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("User not found")]
    NotFound,

    #[error("User already exists")]
    AlreadyExists,

    #[error("External service error: {0}")]
    ExternalService(String),
}

/// User service trait
#[async_trait]
pub trait UserService: Send + Sync {
    async fn create_user(&self, request: CreateUserRequest) -> Result<User, ServiceError>;
    async fn get_user(&self, id: UserId) -> Result<User, ServiceError>;
    async fn get_user_by_email(&self, email: &str) -> Result<User, ServiceError>;
    async fn update_user(&self, id: UserId, request: UpdateUserRequest) -> Result<User, ServiceError>;
    async fn delete_user(&self, id: UserId) -> Result<(), ServiceError>;
    async fn list_users(&self, limit: i64, offset: i64) -> Result<Vec<User>, ServiceError>;
}

/// User service implementation
pub struct UserServiceImpl {
    repository: Arc<dyn UserRepository>,
    external_service: Arc<dyn crate::services::ExternalService>,
}

impl UserServiceImpl {
    pub fn new(
        repository: Arc<dyn UserRepository>,
        external_service: Arc<dyn crate::services::ExternalService>
    ) -> Self {
        Self {
            repository,
            external_service,
        }
    }

    /// Notify external services about user creation
    async fn notify_user_created(&self, user: &User) -> Result<(), ServiceError> {
        let notification_payload = serde_json::json!({
            "event": "user_created",
            "user_id": user.id,
            "email": user.email,
            "name": user.name,
            "created_at": user.created_at,
            "timestamp": chrono::Utc::now()
        });

        // Example: Send to webhook endpoint
        if let Err(e) = self.external_service
            .post("https://api.example.com/webhooks/user-created", notification_payload)
            .await
        {
            tracing::warn!("Failed to send user creation notification: {}", e);
            return Err(ServiceError::ExternalService(format!("Notification failed: {}", e)));
        }

        Ok(())
    }

    /// Notify external services about user update
    async fn notify_user_updated(&self, old_user: &User, new_user: &User) -> Result<(), ServiceError> {
        let notification_payload = serde_json::json!({
            "event": "user_updated",
            "user_id": new_user.id,
            "changes": {
                "name": {
                    "old": old_user.name,
                    "new": new_user.name
                },
                "email": {
                    "old": old_user.email,
                    "new": new_user.email
                }
            },
            "updated_at": new_user.updated_at,
            "timestamp": chrono::Utc::now()
        });

        // Example: Send to webhook endpoint
        if let Err(e) = self.external_service
            .post("https://api.example.com/webhooks/user-updated", notification_payload)
            .await
        {
            tracing::warn!("Failed to send user update notification: {}", e);
            return Err(ServiceError::ExternalService(format!("Notification failed: {}", e)));
        }

        Ok(())
    }

    /// Notify external services about user deletion
    async fn notify_user_deleted(&self, user: &User) -> Result<(), ServiceError> {
        let notification_payload = serde_json::json!({
            "event": "user_deleted",
            "user_id": user.id,
            "email": user.email,
            "name": user.name,
            "deleted_at": chrono::Utc::now(),
            "timestamp": chrono::Utc::now()
        });

        // Example: Send to webhook endpoint
        if let Err(e) = self.external_service
            .post("https://api.example.com/webhooks/user-deleted", notification_payload)
            .await
        {
            tracing::warn!("Failed to send user deletion notification: {}", e);
            return Err(ServiceError::ExternalService(format!("Notification failed: {}", e)));
        }

        Ok(())
    }

    /// Create user with transaction handling for complex operations
    pub async fn create_user_with_transaction(&self, request: CreateUserRequest) -> Result<User, ServiceError> {
        tracing::info!("Creating user with transaction: {}", request.email);

        // Validate and normalize the request
        let normalized_request = match request.validate_and_normalize() {
            Ok(req) => req,
            Err(validation_errors) => {
                return Err(ServiceError::Validation(format!("{:?}", validation_errors)));
            }
        };

        // Begin transaction
        let mut tx = match self.repository.begin_transaction().await {
            Ok(tx) => tx,
            Err(e) => {
                tracing::error!("Failed to begin transaction: {}", e);
                return Err(ServiceError::Repository(e));
            }
        };

        // Create user within transaction
        let new_user = NewUser::from(normalized_request);
        let user = match tx.create(&new_user).await {
            Ok(user) => user,
            Err(e) => {
                tracing::error!("Failed to create user in transaction: {}", e);
                if let Err(rollback_err) = tx.rollback().await {
                    tracing::error!("Failed to rollback transaction: {}", rollback_err);
                }
                return match e {
                    RepositoryError::DuplicateEmail(_) => Err(ServiceError::AlreadyExists),
                    _ => Err(ServiceError::Repository(e)),
                };
            }
        };

        // Additional operations within the same transaction could go here
        // For example: creating audit logs, updating statistics, etc.

        // Commit transaction
        if let Err(e) = tx.commit().await {
            tracing::error!("Failed to commit transaction: {}", e);
            return Err(ServiceError::Repository(e));
        }

        tracing::info!("Successfully created user with transaction: {}", user.id);

        // External notifications happen after transaction commit
        if let Err(e) = self.notify_user_created(&user).await {
            tracing::warn!("Failed to notify external services: {}", e);
            // Don't fail the operation if external notification fails
        }

        Ok(user)
    }

    /// Batch update users with transaction handling
    pub async fn batch_update_users(&self, updates: Vec<(UserId, UpdateUserRequest)>) -> Result<Vec<User>, ServiceError> {
        if updates.is_empty() {
            return Ok(Vec::new());
        }

        tracing::info!("Performing batch update for {} users", updates.len());

        // Begin transaction
        let mut tx = match self.repository.begin_transaction().await {
            Ok(tx) => tx,
            Err(e) => {
                tracing::error!("Failed to begin batch update transaction: {}", e);
                return Err(ServiceError::Repository(e));
            }
        };

        let mut updated_users = Vec::new();

        // Process each update within the transaction
        for (user_id, update_request) in updates {
            // Validate request
            let normalized_request = match update_request.validate_and_normalize() {
                Ok(req) => req,
                Err(validation_errors) => {
                    tracing::error!("Validation failed for user {}: {:?}", user_id, validation_errors);
                    if let Err(rollback_err) = tx.rollback().await {
                        tracing::error!("Failed to rollback batch update transaction: {}", rollback_err);
                    }
                    return Err(ServiceError::Validation(format!("User {}: {:?}", user_id, validation_errors)));
                }
            };

            if !normalized_request.has_updates() {
                continue; // Skip users with no updates
            }

            // Update user within transaction
            match tx.update(user_id, normalized_request.name, normalized_request.email).await {
                Ok(user) => {
                    updated_users.push(user);
                },
                Err(e) => {
                    tracing::error!("Failed to update user {} in batch: {}", user_id, e);
                    if let Err(rollback_err) = tx.rollback().await {
                        tracing::error!("Failed to rollback batch update transaction: {}", rollback_err);
                    }
                    return Err(ServiceError::Repository(e));
                }
            }
        }

        // Commit transaction
        if let Err(e) = tx.commit().await {
            tracing::error!("Failed to commit batch update transaction: {}", e);
            return Err(ServiceError::Repository(e));
        }

        tracing::info!("Successfully completed batch update for {} users", updated_users.len());

        // Send notifications for all updated users (fire and forget)
        for user in &updated_users {
            if let Err(e) = self.notify_user_updated(user, user).await {
                tracing::warn!("Failed to notify external services about user {} update: {}", user.id, e);
            }
        }

        Ok(updated_users)
    }
}

#[async_trait]
impl UserService for UserServiceImpl {
    #[tracing::instrument(skip(self, request), fields(email = %request.email))]
    async fn create_user(&self, request: CreateUserRequest) -> Result<User, ServiceError> {
        tracing::info!("Creating user with email: {}", request.email);

        // Validate and normalize the request
        let normalized_request = match request.validate_and_normalize() {
            Ok(req) => req,
            Err(validation_errors) => {
                tracing::warn!("User creation validation failed: {:?}", validation_errors);
                return Err(ServiceError::Validation(format!("{:?}", validation_errors)));
            }
        };

        // Check if email already exists
        if self.repository.email_exists(&normalized_request.email).await? {
            tracing::warn!("Attempted to create user with existing email: {}", normalized_request.email);
            return Err(ServiceError::AlreadyExists);
        }

        let new_user = NewUser::from(normalized_request);

        // Create user with transaction for complex operations
        let user = match self.repository.create(&new_user).await {
            Ok(user) => {
                tracing::info!("Successfully created user with ID: {}", user.id);

                // Notify external services about user creation (fire and forget)
                if let Err(e) = self.notify_user_created(&user).await {
                    tracing::warn!("Failed to notify external services about user creation: {}", e);
                    // Don't fail the operation if external notification fails
                }

                user
            },
            Err(RepositoryError::DuplicateEmail(email)) => {
                tracing::warn!("Duplicate email detected during creation: {}", email);
                return Err(ServiceError::AlreadyExists);
            },
            Err(e) => {
                tracing::error!("Failed to create user: {}", e);
                return Err(ServiceError::Repository(e));
            }
        };

        Ok(user)
    }

    #[tracing::instrument(skip(self), fields(user_id = %id))]
    async fn get_user(&self, id: UserId) -> Result<User, ServiceError> {
        tracing::debug!("Fetching user with ID: {}", id);

        match self.repository.find_by_id(id).await? {
            Some(user) => {
                tracing::debug!("Found user: {} ({})", user.name, user.email);
                Ok(user)
            },
            None => {
                tracing::debug!("User not found with ID: {}", id);
                Err(ServiceError::NotFound)
            }
        }
    }

    #[tracing::instrument(skip(self), fields(email = %email))]
    async fn get_user_by_email(&self, email: &str) -> Result<User, ServiceError> {
        tracing::debug!("Fetching user with email: {}", email);

        // Normalize email for lookup
        let normalized_email = email.trim().to_lowercase();

        match self.repository.find_by_email(&normalized_email).await? {
            Some(user) => {
                tracing::debug!("Found user: {} (ID: {})", user.name, user.id);
                Ok(user)
            },
            None => {
                tracing::debug!("User not found with email: {}", normalized_email);
                Err(ServiceError::NotFound)
            }
        }
    }

    #[tracing::instrument(skip(self, request), fields(user_id = %id))]
    async fn update_user(&self, id: UserId, request: UpdateUserRequest) -> Result<User, ServiceError> {
        tracing::info!("Updating user with ID: {}", id);

        // Validate and normalize the request
        let normalized_request = match request.validate_and_normalize() {
            Ok(req) => req,
            Err(validation_errors) => {
                tracing::warn!("User update validation failed: {:?}", validation_errors);
                return Err(ServiceError::Validation(format!("{:?}", validation_errors)));
            }
        };

        // Check if the request has any updates
        if !normalized_request.has_updates() {
            tracing::warn!("Update request has no changes for user ID: {}", id);
            return Err(ServiceError::Validation("No updates provided".to_string()));
        }

        // Verify user exists before updating
        let existing_user = match self.repository.find_by_id(id).await? {
            Some(user) => user,
            None => {
                tracing::warn!("Attempted to update non-existent user: {}", id);
                return Err(ServiceError::NotFound);
            }
        };

        // Check for email conflicts if email is being updated
        if let Some(ref new_email) = normalized_request.email {
            if new_email != &existing_user.email && self.repository.email_exists_for_other_user(new_email, id).await? {
                tracing::warn!("Attempted to update user {} with existing email: {}", id, new_email);
                return Err(ServiceError::AlreadyExists);
            }
        }

        // Perform the update with transaction handling
        let updated_user = match self.repository.update(id, normalized_request.name, normalized_request.email).await {
            Ok(user) => {
                tracing::info!("Successfully updated user with ID: {}", id);

                // Notify external services about user update (fire and forget)
                if let Err(e) = self.notify_user_updated(&existing_user, &user).await {
                    tracing::warn!("Failed to notify external services about user update: {}", e);
                    // Don't fail the operation if external notification fails
                }

                user
            },
            Err(RepositoryError::NotFound) => {
                tracing::warn!("User not found during update: {}", id);
                return Err(ServiceError::NotFound);
            },
            Err(RepositoryError::DuplicateEmail(email)) => {
                tracing::warn!("Duplicate email detected during update: {}", email);
                return Err(ServiceError::AlreadyExists);
            },
            Err(e) => {
                tracing::error!("Failed to update user {}: {}", id, e);
                return Err(ServiceError::Repository(e));
            }
        };

        Ok(updated_user)
    }

    #[tracing::instrument(skip(self), fields(user_id = %id))]
    async fn delete_user(&self, id: UserId) -> Result<(), ServiceError> {
        tracing::info!("Deleting user with ID: {}", id);

        // Get user details before deletion for external notifications
        let user = match self.repository.find_by_id(id).await? {
            Some(user) => user,
            None => {
                tracing::warn!("Attempted to delete non-existent user: {}", id);
                return Err(ServiceError::NotFound);
            }
        };

        // Perform soft delete instead of hard delete for data integrity
        match self.repository.soft_delete(id).await {
            Ok(()) => {
                tracing::info!("Successfully soft deleted user with ID: {}", id);

                // Notify external services about user deletion (fire and forget)
                if let Err(e) = self.notify_user_deleted(&user).await {
                    tracing::warn!("Failed to notify external services about user deletion: {}", e);
                    // Don't fail the operation if external notification fails
                }

                Ok(())
            },
            Err(RepositoryError::NotFound) => {
                tracing::warn!("User not found during deletion: {}", id);
                Err(ServiceError::NotFound)
            },
            Err(e) => {
                tracing::error!("Failed to delete user {}: {}", id, e);
                Err(ServiceError::Repository(e))
            }
        }
    }

    #[tracing::instrument(skip(self))]
    async fn list_users(&self, limit: i64, offset: i64) -> Result<Vec<User>, ServiceError> {
        tracing::debug!("Listing users with limit: {}, offset: {}", limit, offset);

        // Validate pagination parameters
        if limit <= 0 || limit > 1000 {
            return Err(ServiceError::Validation("Limit must be between 1 and 1000".to_string()));
        }

        if offset < 0 {
            return Err(ServiceError::Validation("Offset must be non-negative".to_string()));
        }

        let users = self.repository.list_active(limit, offset).await?;
        tracing::debug!("Retrieved {} users", users.len());

        Ok(users)
    }
}
