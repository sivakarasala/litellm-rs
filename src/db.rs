use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
static POOL: std::sync::OnceLock<sqlx::PgPool> = std::sync::OnceLock::new();

/// Call once from main() to make the pool globally available.
#[cfg(feature = "ssr")]
pub fn init_pool(pool: sqlx::PgPool) {
    POOL.set(pool).expect("Pool already initialized");
}

#[cfg(feature = "ssr")]
pub async fn db() -> Result<sqlx::PgPool, leptos::prelude::ServerFnError> {
    leptos::prelude::use_context::<sqlx::PgPool>()
        .or_else(|| POOL.get().cloned())
        .ok_or_else(|| leptos::prelude::ServerFnError::new("Database pool not initialized"))
}

// ---- Models ----

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct User {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub role: UserRole,
}
