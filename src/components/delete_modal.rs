use leptos::prelude::*;

/// Reusable confirmation modal for destructive actions.
#[component]
pub fn DeleteModal(
    show: RwSignal<bool>,
    #[prop(default = "Delete this item?")] title: &'static str,
    #[prop(default = "This cannot be undone.")] subtitle: &'static str,
    #[prop(default = "Delete")] confirm_label: &'static str,
    on_confirm: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    let on_confirm = StoredValue::new(on_confirm);

    view! {
        <div
            class="confirm-overlay"
            style=move || if show.get() { "display:flex" } else { "display:none" }
            on:click=move |_| show.set(false)
        >
            <div class="confirm-dialog" on:click=move |ev| { ev.stop_propagation(); }>
                <p class="confirm-msg">{title}</p>
                <p class="confirm-sub">{subtitle}</p>
                <div class="confirm-actions">
                    <button
                        class="confirm-cancel-btn"
                        on:click=move |_| show.set(false)
                    >"Cancel"</button>
                    <button
                        class="confirm-delete-btn"
                        on:click=move |_| {
                            on_confirm.with_value(|f| f());
                            show.set(false);
                        }
                    >{confirm_label}</button>
                </div>
            </div>
        </div>
    }
}
