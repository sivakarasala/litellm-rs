use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageLogEntry {
    pub id: String,
    pub model: String,
    pub endpoint: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub total_tokens: i32,
    pub cost: String,
    pub cached: bool,
    pub status_code: i32,
    pub latency_ms: i32,
    pub created_at: String,
}

#[server]
pub async fn list_usage_logs() -> Result<Vec<UsageLogEntry>, ServerFnError> {
    use crate::auth::session::require_admin;
    use crate::error::AppError;

    require_admin().await?;
    let pool = crate::db::db().await?;

    let rows = sqlx::query!(
        r#"SELECT id, model, endpoint, input_tokens, output_tokens, total_tokens,
                  cost_usd::text as "cost!: String", cached, status_code, latency_ms, created_at
           FROM usage_logs
           ORDER BY created_at DESC
           LIMIT 100"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| UsageLogEntry {
            id: r.id.to_string(),
            model: r.model,
            endpoint: r.endpoint,
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            total_tokens: r.total_tokens,
            cost: format!("${}", r.cost),
            cached: r.cached,
            status_code: r.status_code,
            latency_ms: r.latency_ms,
            created_at: r.created_at.to_string(),
        })
        .collect())
}

#[component]
pub fn UsagePage() -> impl IntoView {
    let usage = Resource::new(|| (), |_| list_usage_logs());

    view! {
        <div class="usage-page">
            <div class="page-header">
                <h1>"Usage"</h1>
                <p class="page-desc">"API request logs and token usage."</p>
            </div>

            <Suspense fallback=|| view! { <div class="skeleton-table">"Loading..."</div> }>
                {move || {
                    usage.get().map(|result| {
                        match result {
                            Ok(list) if list.is_empty() => view! {
                                <div class="empty-state">
                                    <p>"No requests recorded yet."</p>
                                    <p class="empty-state__hint">"Make your first API call to see usage data."</p>
                                </div>
                            }.into_any(),
                            Ok(list) => view! { <UsageTable entries=list/> }.into_any(),
                            Err(e) => view! {
                                <div class="alert alert--error">{format!("Failed to load: {}", e)}</div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn UsageTable(entries: Vec<UsageLogEntry>) -> impl IntoView {
    view! {
        <div class="data-table-wrap">
            <table class="data-table">
                <thead>
                    <tr>
                        <th>"Model"</th>
                        <th>"Endpoint"</th>
                        <th>"In"</th>
                        <th>"Out"</th>
                        <th>"Total"</th>
                        <th>"Cost"</th>
                        <th>"Status"</th>
                        <th>"Latency"</th>
                        <th>"Time"</th>
                    </tr>
                </thead>
                <tbody>
                    {entries.into_iter().map(|e| {
                        view! { <UsageRow entry=e/> }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

#[component]
fn UsageRow(entry: UsageLogEntry) -> impl IntoView {
    let status_class = if entry.status_code == 200 {
        "badge badge--active"
    } else {
        "badge badge--inactive"
    };
    let cached_badge = entry.cached;

    view! {
        <tr>
            <td class="cell--name">{entry.model}</td>
            <td>{entry.endpoint}</td>
            <td>{entry.input_tokens.to_string()}</td>
            <td>{entry.output_tokens.to_string()}</td>
            <td>{entry.total_tokens.to_string()}</td>
            <td>
                {entry.cost}
                {if cached_badge { " (cached)" } else { "" }}
            </td>
            <td><span class=status_class>{entry.status_code.to_string()}</span></td>
            <td>{format!("{}ms", entry.latency_ms)}</td>
            <td class="cell--date">{entry.created_at}</td>
        </tr>
    }
}
