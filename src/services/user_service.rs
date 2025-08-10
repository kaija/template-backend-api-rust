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
}

impl UserServiceImpl {
    pub fn new(repository: Arc<dyn UserRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl UserService for UserServiceImpl {
    async fn create_user(&self, request: CreateUserRequest) -> Result<User, ServiceError> {
        // Validate the request
        if let Err(validation_errors) = validator::Validate::validate(&request) {
            return Err(ServiceError::Validation(format!("{:?}", validation_errors)));
        }

        let new_user = NewUser::from(request);
        
        match self.repository.create(&new_user).await {
            Ok(user) => Ok(user),
            Err(RepositoryError::DuplicateEmail) => Err(ServiceError::AlreadyExists),
            Err(e) => Err(ServiceError::Repository(e)),
        }
    }

    async fn get_user(&self, id: UserId) -> Result<User, ServiceError> {
        match self.repository.find_by_id(id).await? {
            Some(user) => Ok(user),
            None => Err(ServiceError::NotFound),
        }
    }

    async fn get_user_by_email(&self, email: &str) -> Result<User, ServiceError> {
        match self.repository.find_by_email(email).await? {
            Some(user) => Ok(user),
            None => Err(ServiceError::NotFound),
        }
    }

    async fn update_user(&self, id: UserId, request: UpdateUserRequest) -> Result<User, ServiceError> {
        // Validate the request
        if let Err(validation_errors) = validator::Validate::validate(&request) {
            return Err(ServiceError::Validation(format!("{:?}", validation_errors)));
        }

        match self.repository.update(id, request.name, request.email).await {
            Ok(user) => Ok(user),
            Err(RepositoryError::NotFound) => Err(ServiceError::NotFound),
            Err(RepositoryError::DuplicateEmail) => Err(ServiceError::AlreadyExists),
            Err(e) => Err(ServiceError::Repository(e)),
        }
    }

    async fn delete_user(&self, id: UserId) -> Result<(), ServiceError> {
        match self.repository.delete(id).await {
            Ok(()) => Ok(()),
            Err(RepositoryError::NotFound) => Err(ServiceError::NotFound),
            Err(e) => Err(ServiceError::Repository(e)),
        }
    }

    async fn list_users(&self, limit: i64, offset: i64) -> Result<Vec<User>, ServiceError> {
        let users = self.repository.list(limit, offset).await?;
        Ok(users)
    }
}