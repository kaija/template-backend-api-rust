use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::{User, NewUser, UserId};

/// Repository error types
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("User not found")]
    NotFound,
    
    #[error("Duplicate email")]
    DuplicateEmail,
}

/// User repository trait
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create(&self, user: &NewUser) -> Result<User, RepositoryError>;
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, RepositoryError>;
    async fn update(&self, id: UserId, name: Option<String>, email: Option<String>) -> Result<User, RepositoryError>;
    async fn delete(&self, id: UserId) -> Result<(), RepositoryError>;
    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<User>, RepositoryError>;
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
    async fn create(&self, user: &NewUser) -> Result<User, RepositoryError> {
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
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.constraint() == Some("users_email_key") {
                    return RepositoryError::DuplicateEmail;
                }
            }
            RepositoryError::Database(e)
        })?;
        
        Ok(user)
    }

    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, name, email, is_active, created_at, updated_at FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(user)
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, RepositoryError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, name, email, is_active, created_at, updated_at FROM users WHERE email = $1"
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(user)
    }

    async fn update(&self, id: UserId, name: Option<String>, email: Option<String>) -> Result<User, RepositoryError> {
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
        
        Ok(user)
    }

    async fn delete(&self, id: UserId) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        
        Ok(())
    }

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
        
        Ok(users)
    }
}