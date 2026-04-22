use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::{Email, UserId, UserRole};
#[cfg(feature = "ssr")]
use crate::error::AppError;

/// Strip internal prefixes from ServerFnError messages for user-friendly display.
pub fn clean_error(e: &ServerFnError) -> String {
    let raw = e.to_string();
    raw.strip_prefix("error running server function: ")
        .or_else(|| raw.strip_prefix("ServerFnError: "))
        .unwrap_or(&raw)
        .to_string()
}

pub mod password;
#[cfg(feature = "ssr")]
pub mod session;
#[cfg(feature = "ssr")]
mod validation;
#[cfg(feature = "ssr")]
pub use validation::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: UserId,
    pub email: Email,
    pub display_name: String,
    pub role: UserRole,
}

impl AuthUser {
    pub fn initials(&self) -> String {
        self.display_name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase()
    }
}

#[server]
pub async fn get_me() -> Result<Option<AuthUser>, ServerFnError> {
    let result = session::get_current_user().await;
    match &result {
        Ok(Some(u)) => tracing::info!("get_me: authenticated as {}", u.email),
        Ok(None) => tracing::info!("get_me: no session found"),
        Err(e) => tracing::warn!("get_me: error: {}", e),
    }
    Ok(result?)
}

#[server]
pub async fn logout() -> Result<(), ServerFnError> {
    let session = session::get_session().await?;
    session
        .flush()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    leptos_axum::redirect("/login");
    Ok(())
}
