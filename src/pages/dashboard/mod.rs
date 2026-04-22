use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_requests: i64,
    pub total_tokens: i64,
    pub total_cost: String,
    pub active_keys: i64,
}

#[server]
pub async fn get_dashboard_stats() -> Result<DashboardStats, ServerFnError> {
    use crate::auth::session::require_admin;
    use crate::error::AppError;

    require_admin().await?;
    let pool = crate::db::db().await?;

    let usage = sqlx::query!(
        r#"SELECT
            COUNT(*) as "request_count!: i64",
            COALESCE(SUM(total_tokens), 0) as "total_tokens!: i64",
            COALESCE(SUM(cost_usd), 0)::text as "total_cost!: String"
           FROM usage_logs"#
    )
    .fetch_one(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    let keys = sqlx::query!(
        r#"SELECT COUNT(*) as "count!: i64" FROM virtual_keys WHERE is_active = true"#
    )
    .fetch_one(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    Ok(DashboardStats {
        total_requests: usage.request_count,
        total_tokens: usage.total_tokens,
        total_cost: format!("${}", usage.total_cost),
        active_keys: keys.count,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecentRequest {
    pub model: String,
    pub endpoint: String,
    pub total_tokens: i32,
    pub cost: String,
    pub status_code: i32,
    pub latency_ms: i32,
    pub created_at: String,
}

#[server]
pub async fn get_recent_requests() -> Result<Vec<RecentRequest>, ServerFnError> {
    use crate::auth::session::require_admin;
    use crate::error::AppError;

    require_admin().await?;
    let pool = crate::db::db().await?;

    let rows = sqlx::query!(
        r#"SELECT model, endpoint, total_tokens, cost_usd::text as "cost!: String",
                  status_code, latency_ms, created_at
           FROM usage_logs
           ORDER BY created_at DESC
           LIMIT 20"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| RecentRequest {
            model: r.model,
            endpoint: r.endpoint,
            total_tokens: r.total_tokens,
            cost: format!("${}", r.cost),
            status_code: r.status_code,
            latency_ms: r.latency_ms,
            created_at: r.created_at.to_string(),
        })
        .collect())
}

#[component]
pub fn DashboardPage() -> impl IntoView {
    let stats = Resource::new(|| (), |_| get_dashboard_stats());
    let recent = Resource::new(|| (), |_| get_recent_requests());

    view! {
        <div class="dashboard-page">
            <div class="page-header">
                <h1>"Dashboard"</h1>
            </div>

            <Suspense fallback=|| view! { <div class="skeleton-table">"Loading stats..."</div> }>
                {move || {
                    stats.get().map(|result| {
                        match result {
                            Ok(s) => view! { <StatsCards stats=s/> }.into_any(),
                            Err(e) => view! {
                                <div class="alert alert--error">{format!("Failed to load stats: {}", e)}</div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>

            <div class="dashboard-section">
                <h2>"Recent Requests"</h2>
                <Suspense fallback=|| view! { <div class="skeleton-table">"Loading..."</div> }>
                    {move || {
                        recent.get().map(|result| {
                            match result {
                                Ok(list) if list.is_empty() => view! {
                                    <div class="empty-state">
                                        <p>"No requests recorded yet."</p>
                                        <p class="empty-state__hint">"Make your first API call to see usage data."</p>
                                    </div>
                                }.into_any(),
                                Ok(list) => view! { <RecentRequestsTable requests=list/> }.into_any(),
                                Err(e) => view! {
                                    <div class="alert alert--error">{format!("Failed to load: {}", e)}</div>
                                }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </div>
        </div>
    }
}

#[component]
fn StatsCards(stats: DashboardStats) -> impl IntoView {
    view! {
        <div class="stats-grid">
            <div class="stat-card">
                <div class="stat-card__value">{stats.total_requests.to_string()}</div>
                <div class="stat-card__label">"Total Requests"</div>
            </div>
            <div class="stat-card">
                <div class="stat-card__value">{stats.total_tokens.to_string()}</div>
                <div class="stat-card__label">"Total Tokens"</div>
            </div>
            <div class="stat-card">
                <div class="stat-card__value">{stats.total_cost}</div>
                <div class="stat-card__label">"Total Cost"</div>
            </div>
            <div class="stat-card">
                <div class="stat-card__value">{stats.active_keys.to_string()}</div>
                <div class="stat-card__label">"Active Keys"</div>
            </div>
        </div>
    }
}

#[component]
fn RecentRequestsTable(requests: Vec<RecentRequest>) -> impl IntoView {
    view! {
        <div class="data-table-wrap">
            <table class="data-table">
                <thead>
                    <tr>
                        <th>"Model"</th>
                        <th>"Endpoint"</th>
                        <th>"Tokens"</th>
                        <th>"Cost"</th>
                        <th>"Status"</th>
                        <th>"Latency"</th>
                        <th>"Time"</th>
                    </tr>
                </thead>
                <tbody>
                    {requests.into_iter().map(|r| {
                        let status_class = if r.status_code == 200 { "badge badge--active" } else { "badge badge--inactive" };
                        view! {
                            <tr>
                                <td class="cell--name">{r.model}</td>
                                <td>{r.endpoint}</td>
                                <td>{r.total_tokens.to_string()}</td>
                                <td>{r.cost}</td>
                                <td><span class=status_class>{r.status_code.to_string()}</span></td>
                                <td>{format!("{}ms", r.latency_ms)}</td>
                                <td class="cell--date">{r.created_at}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}
