use crate::auth::clean_error;
use crate::components::DeleteModal;
use crate::keys::approved_emails::{
    list_approved_emails, AddApprovedEmail, ApprovedEmailInfo, DeleteApprovedEmail,
    ToggleApprovedEmail,
};
use crate::keys::provider_keys::{
    list_provider_keys, AddProviderKey, DeleteProviderKey, ProviderKeyInfo,
};
use leptos::prelude::*;

#[component]
pub fn SettingsPage() -> impl IntoView {
    view! {
        <div class="settings-page">
            <div class="page-header">
                <h1>"Settings"</h1>
            </div>

            <ProviderKeysSection/>
            <ApprovedEmailsSection/>
        </div>
    }
}

// ─── Provider Keys Section ───

#[component]
fn ProviderKeysSection() -> impl IntoView {
    let add_action = ServerAction::<AddProviderKey>::new();
    let delete_action = ServerAction::<DeleteProviderKey>::new();

    let keys = Resource::new(
        move || (add_action.version().get(), delete_action.version().get()),
        |_| list_provider_keys(),
    );

    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let show_form = RwSignal::new(false);
    let add_form: NodeRef<leptos::html::Form> = NodeRef::new();

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

    Effect::new(move |_| match delete_action.value().get() {
        Some(Ok(_)) => error_msg.set(None),
        Some(Err(e)) => error_msg.set(Some(clean_error(&e))),
        None => {}
    });

    view! {
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
                                <input type="text" name="name" placeholder="e.g. Production OpenAI" required minlength="4" maxlength="100" />
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
    }
}

#[component]
fn ProviderKeyTable(
    keys: Vec<ProviderKeyInfo>,
    delete_action: ServerAction<DeleteProviderKey>,
) -> impl IntoView {
    let show_delete = RwSignal::new(false);
    let pending_delete_id: RwSignal<Option<String>> = RwSignal::new(None);

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
                        let id = key.id.to_string();
                        view! { <ProviderKeyRow key=key
                            on_delete=move || {
                                pending_delete_id.set(Some(id.clone()));
                                show_delete.set(true);
                            }
                        /> }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
        <DeleteModal
            show=show_delete
            title="Delete this provider key?"
            subtitle="Virtual keys using this provider will stop working."
            on_confirm=move || {
                if let Some(id) = pending_delete_id.get() {
                    delete_action.dispatch(DeleteProviderKey { key_id: id });
                }
            }
        />
    }
}

#[component]
fn ProviderKeyRow(key: ProviderKeyInfo, #[prop(into)] on_delete: Callback<()>) -> impl IntoView {
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
                <button class="btn btn--danger btn--sm"
                    on:click=move |_| on_delete.run(())
                >"Delete"</button>
            </td>
        </tr>
    }
}

// ─── Approved Emails Section ───

#[component]
fn ApprovedEmailsSection() -> impl IntoView {
    let add_action = ServerAction::<AddApprovedEmail>::new();
    let delete_action = ServerAction::<DeleteApprovedEmail>::new();
    let toggle_action = ServerAction::<ToggleApprovedEmail>::new();

    let emails = Resource::new(
        move || {
            (
                add_action.version().get(),
                delete_action.version().get(),
                toggle_action.version().get(),
            )
        },
        |_| list_approved_emails(),
    );

    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let show_form = RwSignal::new(false);
    let add_form: NodeRef<leptos::html::Form> = NodeRef::new();

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

    Effect::new(move |_| match delete_action.value().get() {
        Some(Ok(_)) => error_msg.set(None),
        Some(Err(e)) => error_msg.set(Some(clean_error(&e))),
        None => {}
    });

    Effect::new(move |_| match toggle_action.value().get() {
        Some(Ok(_)) => error_msg.set(None),
        Some(Err(e)) => error_msg.set(Some(clean_error(&e))),
        None => {}
    });

    view! {
        <div class="settings-section">
            <div class="section-header">
                <h2>"Approved Emails"</h2>
                <p class="section-desc">"Manage the email whitelist for self-service token requests at /request-token"</p>
            </div>

            {move || error_msg.get().map(|msg| view! {
                <div class="alert alert--error">{msg}</div>
            })}

            <Show when=move || show_form.get()>
                <div class="card card--form">
                    <ActionForm action=add_action node_ref=add_form>
                        <div class="form-grid">
                            <div class="form-field">
                                <label>"Email"</label>
                                <input type="email" name="email" placeholder="user@example.com" required />
                            </div>
                            <div class="form-field">
                                <label>"Display Name (optional)"</label>
                                <input type="text" name="display_name" placeholder="Jane Doe" />
                            </div>
                            <div class="form-field">
                                <label>"Max Budget (USD)"</label>
                                <input type="text" name="max_budget_usd" placeholder="e.g. 10.00" />
                            </div>
                            <div class="form-field">
                                <label>"Expiry (hours)"</label>
                                <input type="number" name="default_expiry_hours" placeholder="720 (30 days)" />
                            </div>
                            <div class="form-field">
                                <label>"RPM Limit"</label>
                                <input type="number" name="rpm_limit" placeholder="Optional" />
                            </div>
                            <div class="form-field">
                                <label>"TPM Limit"</label>
                                <input type="number" name="tpm_limit" placeholder="Optional" />
                            </div>
                        </div>
                        <div class="form-actions">
                            <button type="submit" class="btn btn--primary"
                                disabled=move || add_action.pending().get()
                            >
                                {move || if add_action.pending().get() { "Adding..." } else { "Add Email" }}
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
                    "+ Add Approved Email"
                </button>
            </Show>

            <Suspense fallback=|| view! { <div class="skeleton-table">"Loading..."</div> }>
                {move || {
                    emails.get().map(|result| {
                        match result {
                            Ok(list) if list.is_empty() => view! {
                                <div class="empty-state">
                                    <p>"No approved emails configured."</p>
                                    <p class="empty-state__hint">"Add emails to enable self-service token requests."</p>
                                </div>
                            }.into_any(),
                            Ok(list) => view! {
                                <ApprovedEmailTable
                                    emails=list
                                    toggle_action=toggle_action
                                    delete_action=delete_action
                                />
                            }.into_any(),
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
fn ApprovedEmailTable(
    emails: Vec<ApprovedEmailInfo>,
    toggle_action: ServerAction<ToggleApprovedEmail>,
    delete_action: ServerAction<DeleteApprovedEmail>,
) -> impl IntoView {
    let show_delete = RwSignal::new(false);
    let pending_delete_id: RwSignal<Option<String>> = RwSignal::new(None);

    view! {
        <div class="data-table-wrap">
            <table class="data-table">
                <thead>
                    <tr>
                        <th>"Email"</th>
                        <th>"Name"</th>
                        <th>"Budget"</th>
                        <th>"Expiry"</th>
                        <th>"Status"</th>
                        <th>"Actions"</th>
                    </tr>
                </thead>
                <tbody>
                    {emails.into_iter().map(|email| {
                        let id = email.id.to_string();
                        view! { <ApprovedEmailRow email=email toggle_action=toggle_action
                            on_delete=move || {
                                pending_delete_id.set(Some(id.clone()));
                                show_delete.set(true);
                            }
                        /> }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
        <DeleteModal
            show=show_delete
            title="Delete this approved email?"
            subtitle="This email will no longer be able to request self-service tokens."
            on_confirm=move || {
                if let Some(id) = pending_delete_id.get() {
                    delete_action.dispatch(DeleteApprovedEmail { email_id: id });
                }
            }
        />
    }
}

#[component]
fn ApprovedEmailRow(
    email: ApprovedEmailInfo,
    toggle_action: ServerAction<ToggleApprovedEmail>,
    #[prop(into)] on_delete: Callback<()>,
) -> impl IntoView {
    let toggle_id = email.id.to_string();
    let is_active = email.is_active;

    let status_class = if email.is_active {
        "badge badge--active"
    } else {
        "badge badge--inactive"
    };
    let status_text = if email.is_active {
        "Active"
    } else {
        "Disabled"
    };

    let budget_display = email
        .max_budget_usd
        .map(|b| format!("${}", b))
        .unwrap_or_else(|| "Unlimited".to_string());

    let expiry_display = email
        .default_expiry_hours
        .map(|h| format!("{}h", h))
        .unwrap_or_else(|| "30 days".to_string());

    let name_display = email.display_name.unwrap_or_default();

    view! {
        <tr>
            <td class="cell--name">{email.email}</td>
            <td>{name_display}</td>
            <td>{budget_display}</td>
            <td>{expiry_display}</td>
            <td><span class=status_class>{status_text}</span></td>
            <td class="cell--actions">
                <ActionForm action=toggle_action>
                    <input type="hidden" name="email_id" value=toggle_id/>
                    <input type="hidden" name="active" value=move || (!is_active).to_string()/>
                    <button type="submit" class="btn btn--secondary btn--sm"
                        disabled=move || toggle_action.pending().get()
                    >
                        {if is_active { "Disable" } else { "Enable" }}
                    </button>
                </ActionForm>
                <button class="btn btn--danger btn--sm"
                    on:click=move |_| on_delete.run(())
                >"Delete"</button>
            </td>
        </tr>
    }
}
