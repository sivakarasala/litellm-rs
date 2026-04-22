#[cfg(feature = "ssr")]
use crate::db::UserId;
#[cfg(feature = "ssr")]
use crate::error::AppError;
use leptos::prelude::*;

#[server]
pub async fn login_with_password(email: String, password: String) -> Result<(), ServerFnError> {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};

    let pool = crate::db::db().await?;

    let row = sqlx::query!(
        "SELECT id, password_hash FROM users WHERE email = LOWER($1)",
        email
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let row = match row {
        Some(r) => r,
        None => {
            crate::audit::log_audit(
                &pool,
                None,
                "auth.login_failed",
                Some("user"),
                None,
                Some(serde_json::json!({"email": email, "reason": "not_found"})),
                None,
            )
            .await;
            return Err(AppError::InvalidCredentials.into());
        }
    };

    let parsed =
        PasswordHash::new(&row.password_hash).map_err(|e| AppError::Internal(e.to_string()))?;

    if Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_err()
    {
        crate::audit::log_audit(
            &pool,
            Some(row.id),
            "auth.login_failed",
            Some("user"),
            Some(&row.id.to_string()),
            Some(serde_json::json!({"reason": "bad_password"})),
            None,
        )
        .await;
        return Err(AppError::InvalidCredentials.into());
    }

    let user_id = UserId::from_uuid(row.id);
    let session = super::session::get_session().await?;
    super::session::set_user_id(&session, &user_id).await?;

    crate::audit::log_audit(
        &pool,
        Some(row.id),
        "auth.login",
        Some("user"),
        Some(&row.id.to_string()),
        None,
        None,
    )
    .await;

    Ok(())
}

#[server]
pub async fn register_with_password(
    name: String,
    email: String,
    password: String,
) -> Result<(), ServerFnError> {
    use super::{
        default_role_for_new_user, hash_password, validate_email, validate_name, validate_password,
    };

    let name = validate_name(&name)?;
    let email = validate_email(&email)?;
    validate_password(&password)?;

    let pool = crate::db::db().await?;
    let hash = hash_password(&password)?;
    let role = default_role_for_new_user(&pool).await?;

    let rec = sqlx::query!(
        r#"INSERT INTO users (email, display_name, password_hash, role)
           VALUES (LOWER($1), $2, $3, $4)
           RETURNING id"#,
        email.as_str(),
        name,
        hash,
        role as crate::db::UserRole
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("unique") || msg.contains("duplicate") {
            AppError::DuplicateEmail
        } else {
            AppError::Internal("Failed to create account".into())
        }
    })?;

    let user_id = UserId::from_uuid(rec.id);
    let session = super::session::get_session().await?;
    super::session::set_user_id(&session, &user_id).await?;

    crate::audit::log_audit(
        &pool,
        Some(rec.id),
        "auth.register",
        Some("user"),
        Some(&rec.id.to_string()),
        Some(serde_json::json!({"role": format!("{:?}", role)})),
        None,
    )
    .await;

    Ok(())
}
