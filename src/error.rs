use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppError {
    // Auth
    InvalidCredentials,
    Unauthorized,
    Forbidden,
    DuplicateEmail,

    // Validation
    Validation(String),

    // Infrastructure (not user-facing details)
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::InvalidCredentials => write!(f, "Invalid email or password"),
            AppError::Unauthorized => write!(f, "Unauthorized"),
            AppError::Forbidden => write!(f, "Insufficient permissions"),
            AppError::DuplicateEmail => write!(f, "An account with this email already exists"),
            AppError::Validation(msg) => write!(f, "{msg}"),
            AppError::Internal(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<AppError> for leptos::prelude::ServerFnError {
    fn from(e: AppError) -> Self {
        leptos::prelude::ServerFnError::new(e.to_string())
    }
}
