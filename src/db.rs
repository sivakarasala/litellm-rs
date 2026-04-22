#[cfg(feature = "ssr")]
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(feature = "ssr")]
static POOL: std::sync::OnceLock<sqlx::PgPool> = std::sync::OnceLock::new();

/// Call once from main() to make the pool globally available.
#[cfg(feature = "ssr")]
pub fn init_pool(pool: sqlx::PgPool) {
    POOL.set(pool).expect("Pool already initialized");
}

#[cfg(feature = "ssr")]
pub async fn db() -> Result<sqlx::PgPool, AppError> {
    leptos::prelude::use_context::<sqlx::PgPool>()
        .or_else(|| POOL.get().cloned())
        .ok_or_else(|| AppError::Internal("Database pool not initialized".into()))
}

// ---- Newtypes ----

/// Generate a UUID-backed newtype with Display, SSR-only from_uuid/as_uuid.
macro_rules! uuid_newtype {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        #[cfg(feature = "ssr")]
        impl $name {
            pub fn from_uuid(id: uuid::Uuid) -> Self {
                Self(id.to_string())
            }

            pub fn as_uuid(&self) -> Result<uuid::Uuid, AppError> {
                self.0
                    .parse()
                    .map_err(|_| AppError::Internal(format!("Invalid {} format", $label)))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

uuid_newtype!(UserId, "user ID");
uuid_newtype!(ProviderKeyId, "provider key ID");
uuid_newtype!(VirtualKeyId, "virtual key ID");
uuid_newtype!(ApprovedEmailId, "approved email ID");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Email(String);

impl Email {
    /// Parse and validate an email address. Returns error if invalid.
    pub fn parse(s: String) -> Result<Self, String> {
        use validator::ValidateEmail;
        let s = s.trim().to_lowercase();
        if s.validate_email() {
            Ok(Self(s))
        } else {
            Err(format!("{} is not a valid email address.", s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Construct from a trusted source (e.g., database row) without re-validating.
    #[cfg(feature = "ssr")]
    pub fn from_trusted(email: String) -> Self {
        Self(email)
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// ---- Models ----

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum UserRole {
    #[serde(rename = "viewer")]
    Viewer,
    #[serde(rename = "admin")]
    Admin,
}

#[cfg(feature = "ssr")]
impl sqlx::Type<sqlx::Postgres> for UserRole {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("user_role")
    }
}

#[cfg(feature = "ssr")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for UserRole {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        match s {
            "viewer" => Ok(UserRole::Viewer),
            "admin" => Ok(UserRole::Admin),
            other => Err(format!("unknown user_role: {other}").into()),
        }
    }
}

#[cfg(feature = "ssr")]
impl sqlx::Encode<'_, sqlx::Postgres> for UserRole {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            UserRole::Viewer => "viewer",
            UserRole::Admin => "admin",
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode(s, buf)
    }
}
