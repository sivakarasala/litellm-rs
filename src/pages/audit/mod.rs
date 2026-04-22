use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub details: Option<String>,
    pub created_at: String,
}

#[server]
pub async fn list_audit_logs() -> Result<Vec<AuditLogEntry>, ServerFnError> {
    use crate::auth::session::require_admin;
    use crate::error::AppError;

    require_admin().await?;
    let pool = crate::db::db().await?;

    let rows = sqlx::query!(
        r#"SELECT id, action, target_type, target_id, details, created_at
           FROM audit_log
           ORDER BY created_at DESC
           LIMIT 100"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| AuditLogEntry {
            id: r.id.to_string(),
            action: r.action,
            target_type: r.target_type,
            target_id: r.target_id,
            details: r.details.map(|d| d.to_string()),
            created_at: r.created_at.to_string(),
        })
        .collect())
}

#[component]
pub fn AuditPage() -> impl IntoView {
    let audit = Resource::new(|| (), |_| list_audit_logs());

    view! {
        <div class="audit-page">
            <div class="page-header">
                <h1>"Audit Log"</h1>
                <p class="page-desc">"Track all admin actions and system events."</p>
            </div>

            <Suspense fallback=|| view! { <div class="skeleton-table">"Loading..."</div> }>
                {move || {
                    audit.get().map(|result| {
                        match result {
                            Ok(list) if list.is_empty() => view! {
                                <div class="empty-state">
                                    <p>"No activity recorded yet."</p>
                                </div>
                            }.into_any(),
                            Ok(list) => view! { <AuditTable entries=list/> }.into_any(),
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
fn AuditTable(entries: Vec<AuditLogEntry>) -> impl IntoView {
    view! {
        <div class="data-table-wrap">
            <table class="data-table">
                <thead>
                    <tr>
                        <th>"Action"</th>
                        <th>"Target"</th>
                        <th>"Details"</th>
                        <th>"Time"</th>
                    </tr>
                </thead>
                <tbody>
                    {entries.into_iter().map(|e| {
                        let target = match (&e.target_type, &e.target_id) {
                            (Some(t), Some(id)) => format!("{}/{}", t, &id[..8.min(id.len())]),
                            (Some(t), None) => t.clone(),
                            _ => "-".to_string(),
                        };
                        let details = e.details.unwrap_or_else(|| "-".to_string());
                        view! {
                            <tr>
                                <td class="cell--name">{e.action}</td>
                                <td><code class="key-preview">{target}</code></td>
                                <td class="cell--url">{details}</td>
                                <td class="cell--date">{e.created_at}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}
