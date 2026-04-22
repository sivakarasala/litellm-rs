use super::AuthUser;
use crate::db::UserRole;
use leptos::prelude::*;
use tower_sessions::Session;

const USER_ID_KEY: &str = "user_id";

pub async fn get_session() -> Result<Session, ServerFnError> {
    let session: Session = leptos_axum::extract()
        .await
        .map_err(|e| ServerFnError::new(format!("Session extraction failed: {}", e)))?;
    Ok(session)
}

pub async fn get_current_user() -> Result<Option<AuthUser>, ServerFnError> {
    let session = get_session().await?;
    let user_id: Option<String> = session
        .get(USER_ID_KEY)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let Some(uid) = user_id else {
        return Ok(None);
    };

    let pool = crate::db::db().await?;
    let user_uuid: uuid::Uuid = uid
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let row = sqlx::query!(
        r#"SELECT id, email, display_name, role AS "role: UserRole" FROM users WHERE id = $1"#,
        user_uuid
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(row.map(|r| AuthUser {
        id: r.id.to_string(),
        email: r.email,
        display_name: r.display_name,
        role: r.role,
    }))
}

pub async fn require_auth() -> Result<AuthUser, ServerFnError> {
    get_current_user()
        .await?
        .ok_or_else(|| ServerFnError::new("Unauthorized"))
}

pub async fn require_admin() -> Result<AuthUser, ServerFnError> {
    let user = require_auth().await?;
    if user.role == UserRole::Admin {
        Ok(user)
    } else {
        Err(ServerFnError::new("Insufficient permissions"))
    }
}

/// Bundle require_auth + db() + uuid parse into one call.
pub async fn auth_context() -> Result<(AuthUser, sqlx::PgPool, uuid::Uuid), ServerFnError> {
    let user = require_auth().await?;
    let pool = crate::db::db().await?;
    let user_uuid: uuid::Uuid = user
        .id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    Ok((user, pool, user_uuid))
}

pub async fn set_user_id(session: &Session, user_id: &str) -> Result<(), ServerFnError> {
    session
        .insert(USER_ID_KEY, user_id.to_string())
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}
