use crate::db::{Email, UserRole};
use crate::error::AppError;

/// Validate a display name: trimmed, non-empty, max 100 chars.
pub fn validate_name(name: &str) -> Result<String, AppError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("Name is required".into()));
    }
    if name.len() > 100 {
        return Err(AppError::Validation("Name is too long".into()));
    }
    Ok(name)
}

/// Validate an email address: trimmed, lowercased, must contain @ and dot.
pub fn validate_email(email: &str) -> Result<Email, AppError> {
    let email = email.trim().to_lowercase();
    if email.is_empty() {
        return Err(AppError::Validation("Email is required".into()));
    }
    if !email.contains('@') || !email.contains('.') {
        return Err(AppError::Validation("Invalid email address".into()));
    }
    Ok(Email::from_trusted(email))
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
