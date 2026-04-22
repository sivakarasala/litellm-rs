use super::AuthUser;
use crate::db::{Email, UserId, UserRole};
use crate::error::AppError;
use tower_sessions::Session;

const USER_ID_KEY: &str = "user_id";

pub async fn get_session() -> Result<Session, AppError> {
    let session: Session = leptos_axum::extract()
        .await
        .map_err(|e| AppError::Internal(format!("Session extraction failed: {}", e)))?;
    Ok(session)
}

pub async fn get_current_user() -> Result<Option<AuthUser>, AppError> {
    let session = get_session().await?;
    let user_id: Option<String> = session
        .get(USER_ID_KEY)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let Some(uid) = user_id else {
        return Ok(None);
    };

    let pool = crate::db::db().await?;
    let user_uuid: uuid::Uuid = uid
        .parse()
        .map_err(|_| AppError::Internal("Invalid user ID in session".into()))?;

    let row = sqlx::query!(
        r#"SELECT id, email, display_name, role AS "role: UserRole" FROM users WHERE id = $1"#,
        user_uuid
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(row.map(|r| AuthUser {
        id: UserId::from_uuid(r.id),
        email: Email::from_trusted(r.email),
        display_name: r.display_name,
        role: r.role,
    }))
}

pub async fn require_auth() -> Result<AuthUser, AppError> {
    get_current_user().await?.ok_or(AppError::Unauthorized)
}

pub async fn require_admin() -> Result<AuthUser, AppError> {
    let user = require_auth().await?;
    if user.role == UserRole::Admin {
        Ok(user)
    } else {
        Err(AppError::Forbidden)
    }
}

/// Bundle require_auth + db() + uuid parse into one call.
pub async fn auth_context() -> Result<(AuthUser, sqlx::PgPool, uuid::Uuid), AppError> {
    let user = require_auth().await?;
    let pool = crate::db::db().await?;
    let user_uuid = user.id.as_uuid()?;
    Ok((user, pool, user_uuid))
}

pub async fn set_user_id(session: &Session, user_id: &UserId) -> Result<(), AppError> {
    session
        .insert(USER_ID_KEY, user_id.as_str().to_string())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}
