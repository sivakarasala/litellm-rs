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
    .map_err(|e| ServerFnError::new(e.to_string()))?;

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
            return Err(ServerFnError::new("Invalid email or password"));
        }
    };

    let parsed =
        PasswordHash::new(&row.password_hash).map_err(|e| ServerFnError::new(e.to_string()))?;

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
        return Err(ServerFnError::new("Invalid email or password"));
    }

    let session = super::session::get_session().await?;
    super::session::set_user_id(&session, &row.id.to_string()).await?;

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

    let role_enum: crate::db::UserRole = if role == "admin" {
        crate::db::UserRole::Admin
    } else {
        crate::db::UserRole::Viewer
    };
    let rec = sqlx::query!(
        r#"INSERT INTO users (email, display_name, password_hash, role)
           VALUES (LOWER($1), $2, $3, $4)
           RETURNING id"#,
        email,
        name,
        hash,
        role_enum as crate::db::UserRole
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("unique") || msg.contains("duplicate") {
            ServerFnError::new("An account with this email already exists")
        } else {
            ServerFnError::new("Failed to create account")
        }
    })?;

    let session = super::session::get_session().await?;
    super::session::set_user_id(&session, &rec.id.to_string()).await?;

    crate::audit::log_audit(
        &pool,
        Some(rec.id),
        "auth.register",
        Some("user"),
        Some(&rec.id.to_string()),
        Some(serde_json::json!({"role": role})),
        None,
    )
    .await;

    Ok(())
}
