use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};

use crate::models::CurrentUser;

/// Extractor for current authenticated user
#[async_trait]
impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<CurrentUser>()
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

/// Optional extractor for current user (returns None if not authenticated)
pub struct OptionalCurrentUser(pub Option<CurrentUser>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalCurrentUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let user = parts.extensions.get::<CurrentUser>().cloned();
        Ok(OptionalCurrentUser(user))
    }
}