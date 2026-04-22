use crate::db::{Email, UserRole};
use crate::error::AppError;

/// Validate a display name: trimmed, 4-100 chars.
pub fn validate_name(name: &str) -> Result<String, AppError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("Name is required".into()));
    }
    if name.len() < 4 {
        return Err(AppError::Validation(
            "Name must be at least 4 characters".into(),
        ));
    }
    if name.len() > 100 {
        return Err(AppError::Validation("Name is too long".into()));
    }
    Ok(name)
}

/// Validate an email address using the `validator` crate (RFC 5322).
pub fn validate_email(email: &str) -> Result<Email, AppError> {
    Email::parse(email.to_string()).map_err(AppError::Validation)
}

/// Validate password length: 8-128 characters.
pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 8 {
        return Err(AppError::Validation(
            "Password must be at least 8 characters".into(),
        ));
    }
    if password.len() > 128 {
        return Err(AppError::Validation("Password is too long".into()));
    }
    Ok(())
}

/// Hash a password with Argon2 + random salt.
pub fn hash_password(password: &str) -> Result<String, AppError> {
    use argon2::{
        password_hash::{rand_core::OsRng, SaltString},
        Argon2, PasswordHasher,
    };

    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// First user is admin, rest are viewers.
pub async fn default_role_for_new_user(pool: &sqlx::PgPool) -> Result<UserRole, AppError> {
    let rec = sqlx::query!("SELECT COUNT(*) AS count FROM users")
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(if rec.count.unwrap_or(0) == 0 {
        UserRole::Admin
    } else {
        UserRole::Viewer
    })
}
