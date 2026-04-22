use crate::auth::clean_error;
use crate::components::DeleteModal;
use crate::keys::provider_keys::{list_provider_keys, ProviderKeyInfo};
use crate::keys::virtual_keys::{
    list_virtual_keys, CreateVirtualKey, DeleteVirtualKey, ToggleVirtualKey, VirtualKeyCreated,
    VirtualKeyInfo,
};
use leptos::prelude::*;

#[component]
pub fn KeysPage() -> impl IntoView {
    let create_action = ServerAction::<CreateVirtualKey>::new();
    let toggle_action = ServerAction::<ToggleVirtualKey>::new();
    let delete_action = ServerAction::<DeleteVirtualKey>::new();

    let keys = Resource::new(
        move || {
            (
                create_action.version().get(),
                toggle_action.version().get(),
                delete_action.version().get(),
            )
        },
        |_| list_virtual_keys(),
    );

    let provider_keys = Resource::new(|| (), |_| list_provider_keys());

    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let show_form = RwSignal::new(false);
    let created_key: RwSignal<Option<VirtualKeyCreated>> = RwSignal::new(None);
    let create_form: NodeRef<leptos::html::Form> = NodeRef::new();

    // Handle create result
    Effect::new(move |_| match create_action.value().get() {
        Some(Ok(key)) => {
            if let Some(form) = create_form.get() {
                form.reset();
            }
            show_form.set(false);
            error_msg.set(None);
            created_key.set(Some(key));
        }
        Some(Err(e)) => error_msg.set(Some(clean_error(&e))),
        None => {}
    });

    // Handle toggle result
    Effect::new(move |_| match toggle_action.value().get() {
        Some(Ok(_)) => error_msg.set(None),
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
        <div class="keys-page">
            <div class="page-header">
                <h1>"Virtual Keys"</h1>
                <p class="page-desc">"Create and manage virtual API keys for your clients."</p>
            </div>

            {move || error_msg.get().map(|msg| view! {
                <div class="alert alert--error">{msg}</div>
            })}

            // Show created key banner (shown once)
            {move || created_key.get().map(|key| view! {
                <CreatedKeyBanner key=key on_dismiss=move || created_key.set(None)/>
            })}

            <Show when=move || show_form.get()>
                <Suspense fallback=|| view! { <div class="skeleton-table">"Loading providers..."</div> }>
                    {move || {
                        provider_keys.get().map(|result| {
                            match result {
                                Ok(providers) if providers.is_empty() => view! {
                                    <div class="alert alert--warning">
                                        "No provider keys configured. "
                                        <a href="/settings">"Add one in Settings"</a>
                                        " first."
                                    </div>
                                }.into_any(),
                                Ok(providers) => view! {
                                    <CreateKeyForm
                                        providers=providers
                                        create_action=create_action
                                        form_ref=create_form
                                        on_cancel=move || show_form.set(false)
                                    />
                                }.into_any(),
                                Err(e) => view! {
                                    <div class="alert alert--error">{format!("Failed to load providers: {}", e)}</div>
                                }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </Show>

            <Show when=move || !show_form.get()>
                <button class="btn btn--primary" on:click=move |_| show_form.set(true)>
                    "+ Create Virtual Key"
                </button>
            </Show>

            <Suspense fallback=|| view! { <div class="skeleton-table">"Loading..."</div> }>
                {move || {
                    keys.get().map(|result| {
                        match result {
                            Ok(list) if list.is_empty() => view! {
                                <div class="empty-state">
                                    <p>"No virtual keys yet."</p>
                                    <p class="empty-state__hint">"Create your first key to start proxying API requests."</p>
                                </div>
                            }.into_any(),
                            Ok(list) => view! {
                                <VirtualKeyTable
                                    keys=list
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
fn CreatedKeyBanner(key: VirtualKeyCreated, on_dismiss: impl Fn() + 'static) -> impl IntoView {
    view! {
        <div class="alert alert--success created-key-banner">
            <div class="created-key-banner__header">
                <strong>"Key Created Successfully"</strong>
                <button class="btn btn--sm" on:click=move |_| on_dismiss()>"Dismiss"</button>
            </div>
            <p class="created-key-banner__warning">
                "Copy this key now — it will not be shown again."
            </p>
            <div class="created-key-banner__key">
                <code>{key.raw_key}</code>
            </div>
            <div class="created-key-banner__details">
                <span>"Name: " {key.name}</span>
                {key.expires_at.map(|exp| view! {
                    <span>" | Expires: " {exp}</span>
                })}
            </div>
        </div>
    }
}

#[component]
fn CreateKeyForm(
    providers: Vec<ProviderKeyInfo>,
    create_action: ServerAction<CreateVirtualKey>,
    form_ref: NodeRef<leptos::html::Form>,
    on_cancel: impl Fn() + 'static + Copy + Send,
) -> impl IntoView {
    view! {
        <div class="card card--form">
            <ActionForm action=create_action node_ref=form_ref>
                <div class="form-grid">
                    <div class="form-field">
                        <label>"Key Name"</label>
                        <input type="text" name="name" placeholder="e.g. Frontend App" required minlength="4" maxlength="100" />
                    </div>
                    <div class="form-field">
                        <label>"Provider Key"</label>
                        <select name="provider_key_id" required>
                            {providers.into_iter().map(|p| {
                                let id = p.id.to_string();
                                let label = format!("{} ({})", p.name, p.provider);
                                view! { <option value=id>{label}</option> }
                            }).collect_view()}
                        </select>
                    </div>
                    <div class="form-field">
                        <label>"Expiry"</label>
                        <select name="expiry">
                            <option value="Hours1">"1 Hour"</option>
                            <option value="Hours6">"6 Hours"</option>
                            <option value="Hours24">"24 Hours"</option>
                            <option value="Days7">"7 Days"</option>
                            <option value="Days30" selected>"30 Days"</option>
                            <option value="Days90">"90 Days"</option>
                            <option value="Never">"Never"</option>
                        </select>
                    </div>
                    <div class="form-field">
                        <label>"Max Budget (USD)"</label>
                        <input type="text" name="max_budget_usd" placeholder="e.g. 10.00 (optional)" />
                    </div>
                    <div class="form-field">
                        <label>"RPM Limit"</label>
                        <input type="number" name="rpm_limit" placeholder="Requests/min (optional)" />
                    </div>
                    <div class="form-field">
                        <label>"TPM Limit"</label>
                        <input type="number" name="tpm_limit" placeholder="Tokens/min (optional)" />
                    </div>
                </div>
                <div class="form-actions">
                    <button type="submit" class="btn btn--primary"
                        disabled=move || create_action.pending().get()
                    >
                        {move || if create_action.pending().get() { "Creating..." } else { "Create Key" }}
                    </button>
                    <button type="button" class="btn btn--secondary"
                        on:click=move |_| on_cancel()
                    >"Cancel"</button>
                </div>
            </ActionForm>
        </div>
    }
}

#[component]
fn VirtualKeyTable(
    keys: Vec<VirtualKeyInfo>,
    toggle_action: ServerAction<ToggleVirtualKey>,
    delete_action: ServerAction<DeleteVirtualKey>,
) -> impl IntoView {
    let show_delete = RwSignal::new(false);
    let pending_delete_id: RwSignal<Option<String>> = RwSignal::new(None);

    view! {
        <div class="data-table-wrap">
            <table class="data-table">
                <thead>
                    <tr>
                        <th>"Name"</th>
                        <th>"Key"</th>
                        <th>"Status"</th>
                        <th>"Expires"</th>
                        <th>"Budget"</th>
                        <th>"Limits"</th>
                        <th>"Actions"</th>
                    </tr>
                </thead>
                <tbody>
                    {keys.into_iter().map(|key| {
                        let id = key.id.to_string();
                        view! { <VirtualKeyRow key=key toggle_action=toggle_action
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
            title="Delete this virtual key?"
            subtitle="This will permanently remove the key. Any clients using it will lose access."
            on_confirm=move || {
                if let Some(id) = pending_delete_id.get() {
                    delete_action.dispatch(DeleteVirtualKey { key_id: id });
                }
            }
        />
    }
}

#[component]
fn VirtualKeyRow(
    key: VirtualKeyInfo,
    toggle_action: ServerAction<ToggleVirtualKey>,
    #[prop(into)] on_delete: Callback<()>,
) -> impl IntoView {
    let toggle_id = key.id.to_string();
    let is_active = key.is_active;

    let status_class = if key.is_active {
        "badge badge--active"
    } else {
        "badge badge--inactive"
    };
    let status_text = if key.is_active { "Active" } else { "Disabled" };

    let expires_display = key.expires_at.unwrap_or_else(|| "Never".to_string());

    let budget_display = key
        .max_budget_usd
        .map(|b| format!("${}", b))
        .unwrap_or_else(|| "Unlimited".to_string());

    let limits_display = match (key.rpm_limit, key.tpm_limit) {
        (Some(rpm), Some(tpm)) => format!("{} RPM / {} TPM", rpm, tpm),
        (Some(rpm), None) => format!("{} RPM", rpm),
        (None, Some(tpm)) => format!("{} TPM", tpm),
        (None, None) => "None".to_string(),
    };

    view! {
        <tr>
            <td class="cell--name">{key.name}</td>
            <td><code class="key-preview">{key.key_prefix}</code></td>
            <td><span class=status_class>{status_text}</span></td>
            <td>{expires_display}</td>
            <td>{budget_display}</td>
            <td>{limits_display}</td>
            <td class="cell--actions">
                <ActionForm action=toggle_action>
                    <input type="hidden" name="key_id" value=toggle_id/>
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
