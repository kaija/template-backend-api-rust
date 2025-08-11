use async_trait::async_trait;
use sqlx::{PgPool, Transaction, Postgres};
use tracing::{info, warn, instrument};

use crate::models::{User, NewUser, UserId};

/// Repository error types
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("User not found")]
    NotFound,
    
    #[error("Duplicate email: {0}")]
    DuplicateEmail(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Transaction error: {0}")]
    Transaction(String),
    
    #[error("Connection error: {0}")]
    Connection(String),
}

/// User repository trait with comprehensive data access methods
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Create a new user
    async fn create(&self, user: &NewUser) -> Result<User, RepositoryError>;
    
    /// Create a new user within a transaction
    async fn create_tx(&self, tx: &mut Transaction<'_, Postgres>, user: &NewUser) -> Result<User, RepositoryError>;
    
    /// Find user by ID
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError>;
    
    /// Find user by email
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, RepositoryError>;
    
    /// Update user information
    async fn update(&self, id: UserId, name: Option<String>, email: Option<String>) -> Result<User, RepositoryError>;
    
    /// Update user within a transaction
    async fn update_tx(&self, tx: &mut Transaction<'_, Postgres>, id: UserId, name: Option<String>, email: Option<String>) -> Result<User, RepositoryError>;
    
    /// Soft delete user (set is_active to false)
    async fn soft_delete(&self, id: UserId) -> Result<(), RepositoryError>;
    
    /// Hard delete user (remove from database)
    async fn delete(&self, id: UserId) -> Result<(), RepositoryError>;
    
    /// List users with pagination
    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<User>, RepositoryError>;
    
    /// List active users only
    async fn list_active(&self, limit: i64, offset: i64) -> Result<Vec<User>, RepositoryError>;
    
    /// Count total users
    async fn count(&self) -> Result<i64, RepositoryError>;
    
    /// Count active users
    async fn count_active(&self) -> Result<i64, RepositoryError>;
    
    /// Check if email exists
    async fn email_exists(&self, email: &str) -> Result<bool, RepositoryError>;
    
    /// Check if email exists for different user
    async fn email_exists_for_other_user(&self, email: &str, user_id: UserId) -> Result<bool, RepositoryError>;
    
    /// Activate user
    async fn activate(&self, id: UserId) -> Result<(), RepositoryError>;
    
    /// Deactivate user
    async fn deactivate(&self, id: UserId) -> Result<(), RepositoryError>;
    
    /// Begin a new database transaction
    async fn begin_transaction(&self) -> Result<Box<dyn UserRepositoryTransaction>, RepositoryError>;
}

/// Transaction-aware user repository operations
#[async_trait]
pub trait UserRepositoryTransaction: Send + Sync {
    /// Create a new user within the transaction
    async fn create(&mut self, user: &NewUser) -> Result<User, RepositoryError>;
    
    /// Update user within the transaction
    async fn update(&mut self, id: UserId, name: Option<String>, email: Option<String>) -> Result<User, RepositoryError>;
    
    /// Commit the transaction
    async fn commit(self: Box<Self>) -> Result<(), RepositoryError>;
    
    /// Rollback the transaction
    async fn rollback(self: Box<Self>) -> Result<(), RepositoryError>;
}

/// SQLx implementation of UserRepository
pub struct SqlxUserRepository {
    pool: PgPool,
}

impl SqlxUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for SqlxUserRepository {
    #[instrument(skip(self, user), fields(email = %user.email))]
    async fn create(&self, user: &NewUser) -> Result<User, RepositoryError> {
        info!("Creating new user with email: {}", user.email);
        
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (name, email, is_active, created_at, updated_at)
            VALUES ($1, $2, true, NOW(), NOW())
            RETURNING id, name, email, is_active, created_at, updated_at
            "#
        )
        .bind(&user.name)
        .bind(&user.email)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            warn!("Failed to create user: {}", e);
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.constraint() == Some("users_email_key") {
                    return RepositoryError::DuplicateEmail(user.email.clone());
                }
            }
            RepositoryError::Database(e)
        })?;
        
        info!("Successfully created user with ID: {}", user.id);
        Ok(user)
    }

    #[instrument(skip(self, tx, user), fields(email = %user.email))]
    async fn create_tx(&self, tx: &mut Transaction<'_, Postgres>, user: &NewUser) -> Result<User, RepositoryError> {
        info!("Creating new user in transaction with email: {}", user.email);
        
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (name, email, is_active, created_at, updated_at)
            VALUES ($1, $2, true, NOW(), NOW())
            RETURNING id, name, email, is_active, created_at, updated_at
            "#
        )
        .bind(&user.name)
        .bind(&user.email)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| {
            warn!("Failed to create user in transaction: {}", e);
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.constraint() == Some("users_email_key") {
                    return RepositoryError::DuplicateEmail(user.email.clone());
                }
            }
            RepositoryError::Database(e)
        })?;
        
        info!("Successfully created user in transaction with ID: {}", user.id);
        Ok(user)
    }

    #[instrument(skip(self), fields(user_id = %id))]
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, name, email, is_active, created_at, updated_at FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        
        match &user {
            Some(u) => info!("Found user with ID: {} ({})", id, u.email),
            None => info!("User not found with ID: {}", id),
        }
        
        Ok(user)
    }

    #[instrument(skip(self), fields(email = %email))]
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, RepositoryError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, name, email, is_active, created_at, updated_at FROM users WHERE email = $1"
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        
        match &user {
            Some(u) => info!("Found user with email: {} (ID: {})", email, u.id),
            None => info!("User not found with email: {}", email),
        }
        
        Ok(user)
    }

    #[instrument(skip(self), fields(user_id = %id))]
    async fn update(&self, id: UserId, name: Option<String>, email: Option<String>) -> Result<User, RepositoryError> {
        info!("Updating user with ID: {}", id);
        
        // Check for email conflicts if email is being updated
        if let Some(ref new_email) = email {
            if self.email_exists_for_other_user(new_email, id).await? {
                return Err(RepositoryError::DuplicateEmail(new_email.clone()));
            }
        }
        
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users 
            SET name = COALESCE($2, name),
                email = COALESCE($3, email),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, email, is_active, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(name)
        .bind(email)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(RepositoryError::NotFound)?;
        
        info!("Successfully updated user with ID: {}", id);
        Ok(user)
    }

    #[instrument(skip(self, tx), fields(user_id = %id))]
    async fn update_tx(&self, tx: &mut Transaction<'_, Postgres>, id: UserId, name: Option<String>, email: Option<String>) -> Result<User, RepositoryError> {
        info!("Updating user in transaction with ID: {}", id);
        
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users 
            SET name = COALESCE($2, name),
                email = COALESCE($3, email),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, email, is_active, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(name)
        .bind(email)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(RepositoryError::NotFound)?;
        
        info!("Successfully updated user in transaction with ID: {}", id);
        Ok(user)
    }

    #[instrument(skip(self), fields(user_id = %id))]
    async fn soft_delete(&self, id: UserId) -> Result<(), RepositoryError> {
        info!("Soft deleting user with ID: {}", id);
        
        let result = sqlx::query(
            "UPDATE users SET is_active = false, updated_at = NOW() WHERE id = $1"
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        
        info!("Successfully soft deleted user with ID: {}", id);
        Ok(())
    }

    #[instrument(skip(self), fields(user_id = %id))]
    async fn delete(&self, id: UserId) -> Result<(), RepositoryError> {
        info!("Hard deleting user with ID: {}", id);
        
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        
        info!("Successfully hard deleted user with ID: {}", id);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<User>, RepositoryError> {
        let users = sqlx::query_as::<_, User>(
            r#"
            SELECT id, name, email, is_active, created_at, updated_at 
            FROM users 
            ORDER BY created_at DESC 
            LIMIT $1 OFFSET $2
            "#
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        
        info!("Retrieved {} users (limit: {}, offset: {})", users.len(), limit, offset);
        Ok(users)
    }

    #[instrument(skip(self))]
    async fn list_active(&self, limit: i64, offset: i64) -> Result<Vec<User>, RepositoryError> {
        let users = sqlx::query_as::<_, User>(
            r#"
            SELECT id, name, email, is_active, created_at, updated_at 
            FROM users 
            WHERE is_active = true
            ORDER BY created_at DESC 
            LIMIT $1 OFFSET $2
            "#
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        
        info!("Retrieved {} active users (limit: {}, offset: {})", users.len(), limit, offset);
        Ok(users)
    }

    #[instrument(skip(self))]
    async fn count(&self) -> Result<i64, RepositoryError> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        
        info!("Total user count: {}", count.0);
        Ok(count.0)
    }

    #[instrument(skip(self))]
    async fn count_active(&self) -> Result<i64, RepositoryError> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_active = true")
            .fetch_one(&self.pool)
            .await?;
        
        info!("Active user count: {}", count.0);
        Ok(count.0)
    }

    #[instrument(skip(self), fields(email = %email))]
    async fn email_exists(&self, email: &str) -> Result<bool, RepositoryError> {
        let exists: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)"
        )
        .bind(email)
        .fetch_one(&self.pool)
        .await?;
        
        info!("Email {} exists: {}", email, exists.0);
        Ok(exists.0)
    }

    #[instrument(skip(self), fields(email = %email, user_id = %user_id))]
    async fn email_exists_for_other_user(&self, email: &str, user_id: UserId) -> Result<bool, RepositoryError> {
        let exists: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 AND id != $2)"
        )
        .bind(email)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        
        info!("Email {} exists for other user (excluding {}): {}", email, user_id, exists.0);
        Ok(exists.0)
    }

    #[instrument(skip(self), fields(user_id = %id))]
    async fn activate(&self, id: UserId) -> Result<(), RepositoryError> {
        info!("Activating user with ID: {}", id);
        
        let result = sqlx::query(
            "UPDATE users SET is_active = true, updated_at = NOW() WHERE id = $1"
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        
        info!("Successfully activated user with ID: {}", id);
        Ok(())
    }

    #[instrument(skip(self), fields(user_id = %id))]
    async fn deactivate(&self, id: UserId) -> Result<(), RepositoryError> {
        info!("Deactivating user with ID: {}", id);
        
        let result = sqlx::query(
            "UPDATE users SET is_active = false, updated_at = NOW() WHERE id = $1"
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        
        info!("Successfully deactivated user with ID: {}", id);
        Ok(())
    }

    async fn begin_transaction(&self) -> Result<Box<dyn UserRepositoryTransaction>, RepositoryError> {
        let tx = self.pool.begin().await.map_err(|e| {
            warn!("Failed to begin transaction: {}", e);
            RepositoryError::Transaction(e.to_string())
        })?;
        
        Ok(Box::new(SqlxUserRepositoryTransaction { tx }))
    }
}

/// SQLx transaction implementation
pub struct SqlxUserRepositoryTransaction {
    tx: Transaction<'static, Postgres>,
}

#[async_trait]
impl UserRepositoryTransaction for SqlxUserRepositoryTransaction {
    async fn create(&mut self, user: &NewUser) -> Result<User, RepositoryError> {
        info!("Creating new user in transaction with email: {}", user.email);
        
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (name, email, is_active, created_at, updated_at)
            VALUES ($1, $2, true, NOW(), NOW())
            RETURNING id, name, email, is_active, created_at, updated_at
            "#
        )
        .bind(&user.name)
        .bind(&user.email)
        .fetch_one(&mut *self.tx)
        .await
        .map_err(|e| {
            warn!("Failed to create user in transaction: {}", e);
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.constraint() == Some("users_email_key") {
                    return RepositoryError::DuplicateEmail(user.email.clone());
                }
            }
            RepositoryError::Database(e)
        })?;
        
        info!("Successfully created user in transaction with ID: {}", user.id);
        Ok(user)
    }

    async fn update(&mut self, id: UserId, name: Option<String>, email: Option<String>) -> Result<User, RepositoryError> {
        info!("Updating user in transaction with ID: {}", id);
        
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users 
            SET name = COALESCE($2, name),
                email = COALESCE($3, email),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, email, is_active, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(name)
        .bind(email)
        .fetch_optional(&mut *self.tx)
        .await?
        .ok_or(RepositoryError::NotFound)?;
        
        info!("Successfully updated user in transaction with ID: {}", id);
        Ok(user)
    }

    async fn commit(self: Box<Self>) -> Result<(), RepositoryError> {
        self.tx.commit().await.map_err(|e| {
            warn!("Failed to commit transaction: {}", e);
            RepositoryError::Transaction(e.to_string())
        })
    }

    async fn rollback(self: Box<Self>) -> Result<(), RepositoryError> {
        self.tx.rollback().await.map_err(|e| {
            warn!("Failed to rollback transaction: {}", e);
            RepositoryError::Transaction(e.to_string())
        })
    }
}

/// Repository utilities and helper functions
impl SqlxUserRepository {
    /// Begin a new database transaction
    pub async fn begin_transaction(&self) -> Result<Transaction<'_, Postgres>, RepositoryError> {
        self.pool.begin().await.map_err(|e| {
            warn!("Failed to begin transaction: {}", e);
            RepositoryError::Transaction(e.to_string())
        })
    }

    /// Get a reference to the underlying pool for advanced operations
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::NewUser;
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn setup_test_pool() -> PgPool {
        // This would typically use a test database
        // For now, we'll just create a mock setup
        todo!("Setup test database connection")
    }

    #[tokio::test]
    async fn test_create_user() {
        // Test user creation
        let pool = setup_test_pool().await;
        let repo = SqlxUserRepository::new(pool);
        
        let new_user = NewUser {
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
        };

        // This test would require a real database connection
        // let result = repo.create(&new_user).await;
        // assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_find_user_by_id() {
        // Test finding user by ID
        let pool = setup_test_pool().await;
        let repo = SqlxUserRepository::new(pool);
        
        let user_id = Uuid::new_v4();
        
        // This test would require a real database connection
        // let result = repo.find_by_id(user_id).await;
        // assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_find_user_by_email() {
        // Test finding user by email
        let pool = setup_test_pool().await;
        let repo = SqlxUserRepository::new(pool);
        
        // This test would require a real database connection
        // let result = repo.find_by_email("test@example.com").await;
        // assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_user() {
        // Test user update
        let pool = setup_test_pool().await;
        let repo = SqlxUserRepository::new(pool);
        
        let user_id = Uuid::new_v4();
        
        // This test would require a real database connection
        // let result = repo.update(user_id, Some("Updated Name".to_string()), None).await;
        // assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_email_exists() {
        // Test email existence check
        let pool = setup_test_pool().await;
        let repo = SqlxUserRepository::new(pool);
        
        // This test would require a real database connection
        // let result = repo.email_exists("test@example.com").await;
        // assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_user_count() {
        // Test user counting
        let pool = setup_test_pool().await;
        let repo = SqlxUserRepository::new(pool);
        
        // This test would require a real database connection
        // let result = repo.count().await;
        // assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_users_with_pagination() {
        // Test user listing with pagination
        let pool = setup_test_pool().await;
        let repo = SqlxUserRepository::new(pool);
        
        // This test would require a real database connection
        // let result = repo.list(10, 0).await;
        // assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_soft_delete_user() {
        // Test soft delete
        let pool = setup_test_pool().await;
        let repo = SqlxUserRepository::new(pool);
        
        let user_id = Uuid::new_v4();
        
        // This test would require a real database connection
        // let result = repo.soft_delete(user_id).await;
        // assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_transaction_operations() {
        // Test transaction-based operations
        let pool = setup_test_pool().await;
        let repo = SqlxUserRepository::new(pool);
        
        // This test would require a real database connection
        // let mut tx = repo.begin_transaction().await.unwrap();
        // 
        // let new_user = NewUser {
        //     name: "Transaction User".to_string(),
        //     email: "tx@example.com".to_string(),
        // };
        // 
        // let result = repo.create_tx(&mut tx, &new_user).await;
        // assert!(result.is_ok());
        // 
        // tx.commit().await.unwrap();
    }
}