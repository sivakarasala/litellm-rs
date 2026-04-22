use crate::auth::clean_error;
use crate::keys::provider_keys::{
    list_provider_keys, AddProviderKey, DeleteProviderKey, ProviderKeyInfo,
};
use leptos::prelude::*;

#[component]
pub fn SettingsPage() -> impl IntoView {
    let add_action = ServerAction::<AddProviderKey>::new();
    let delete_action = ServerAction::<DeleteProviderKey>::new();

    let keys = Resource::new(
        move || (add_action.version().get(), delete_action.version().get()),
        |_| list_provider_keys(),
    );

    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let show_form = RwSignal::new(false);
    let add_form: NodeRef<leptos::html::Form> = NodeRef::new();

    // Handle add result
    Effect::new(move |_| match add_action.value().get() {
        Some(Ok(_)) => {
            if let Some(form) = add_form.get() {
                form.reset();
            }
            show_form.set(false);
            error_msg.set(None);
        }
        Some(Err(e)) => error_msg.set(Some(clean_error(&e))),
        None => {}
    });

    // Handle delete result
    Effect::new(move |_| match delete_action.value().get() {
        Some(Ok(_)) => error_msg.set(None),
        Some(Err(e)) => error_msg.set(Some(clean_error(&e))),
        None => {}
    });

    view! {
        <div class="settings-page">
            <div class="page-header">
                <h1>"Settings"</h1>
            </div>

            <div class="settings-section">
                <div class="section-header">
                    <h2>"Provider Keys"</h2>
                    <p class="section-desc">"Manage your upstream API keys (OpenAI, etc.)"</p>
                </div>

                {move || error_msg.get().map(|msg| view! {
                    <div class="alert alert--error">{msg}</div>
                })}

                <Show when=move || show_form.get()>
                    <div class="card card--form">
                        <ActionForm action=add_action node_ref=add_form>
                            <div class="form-grid">
                                <div class="form-field">
                                    <label>"Name"</label>
                                    <input type="text" name="name" placeholder="e.g. Production OpenAI" required />
                                </div>
                                <div class="form-field">
                                    <label>"Provider"</label>
                                    <select name="provider">
                                        <option value="openai">"OpenAI"</option>
                                    </select>
                                </div>
                                <div class="form-field form-field--full">
                                    <label>"API Key"</label>
                                    <input type="password" name="api_key" placeholder="sk-..." required />
                                </div>
                                <div class="form-field form-field--full">
                                    <label>"Base URL"</label>
                                    <input type="text" name="base_url" placeholder="https://api.openai.com" />
                                </div>
                            </div>
                            <div class="form-actions">
                                <button type="submit" class="btn btn--primary"
                                    disabled=move || add_action.pending().get()
                                >
                                    {move || if add_action.pending().get() { "Adding..." } else { "Add Provider Key" }}
                                </button>
                                <button type="button" class="btn btn--secondary"
                                    on:click=move |_| show_form.set(false)
                                >"Cancel"</button>
                            </div>
                        </ActionForm>
                    </div>
                </Show>

                <Show when=move || !show_form.get()>
                    <button class="btn btn--primary" on:click=move |_| show_form.set(true)>
                        "+ Add Provider Key"
                    </button>
                </Show>

                <Suspense fallback=|| view! { <div class="skeleton-table">"Loading..."</div> }>
                    {move || {
                        keys.get().map(|result| {
                            match result {
                                Ok(list) if list.is_empty() => view! {
                                    <div class="empty-state">
                                        <p>"No provider keys configured."</p>
                                        <p class="empty-state__hint">"Add an OpenAI API key to start proxying requests."</p>
                                    </div>
                                }.into_any(),
                                Ok(list) => view! {
                                    <ProviderKeyTable keys=list delete_action=delete_action/>
                                }.into_any(),
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
fn ProviderKeyTable(
    keys: Vec<ProviderKeyInfo>,
    delete_action: ServerAction<DeleteProviderKey>,
) -> impl IntoView {
    view! {
        <div class="data-table-wrap">
            <table class="data-table">
                <thead>
                    <tr>
                        <th>"Name"</th>
                        <th>"Provider"</th>
                        <th>"API Key"</th>
                        <th>"Base URL"</th>
                        <th>"Status"</th>
                        <th>"Actions"</th>
                    </tr>
                </thead>
                <tbody>
                    {keys.into_iter().map(|key| {
                        view! { <ProviderKeyRow key=key delete_action=delete_action/> }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

#[component]
fn ProviderKeyRow(
    key: ProviderKeyInfo,
    delete_action: ServerAction<DeleteProviderKey>,
) -> impl IntoView {
    let key_id = key.id.to_string();
    let status_class = if key.is_active {
        "badge badge--active"
    } else {
        "badge badge--inactive"
    };
    let status_text = if key.is_active { "Active" } else { "Inactive" };

    view! {
        <tr>
            <td class="cell--name">{key.name}</td>
            <td>{key.provider}</td>
            <td><code class="key-preview">{key.key_preview}</code></td>
            <td class="cell--url">{key.base_url}</td>
            <td><span class=status_class>{status_text}</span></td>
            <td>
                <ActionForm action=delete_action>
                    <input type="hidden" name="key_id" value=key_id/>
                    <button type="submit" class="btn btn--danger btn--sm"
                        disabled=move || delete_action.pending().get()
                    >"Delete"</button>
                </ActionForm>
            </td>
        </tr>
    }
}
