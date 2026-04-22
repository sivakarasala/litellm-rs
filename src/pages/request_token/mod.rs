use crate::auth::clean_error;
use crate::keys::approved_emails::{RequestToken, TokenRequestResult};
use leptos::prelude::*;

#[component]
pub fn RequestTokenPage() -> impl IntoView {
    let action = ServerAction::<RequestToken>::new();
    let form_ref: NodeRef<leptos::html::Form> = NodeRef::new();
    let created_token: RwSignal<Option<TokenRequestResult>> = RwSignal::new(None);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);

    Effect::new(move |_| match action.value().get() {
        Some(Ok(result)) => {
            if let Some(form) = form_ref.get() {
                form.reset();
            }
            error_msg.set(None);
            created_token.set(Some(result));
        }
        Some(Err(e)) => error_msg.set(Some(clean_error(&e))),
        None => {}
    });

    view! {
        <div class="request-token-page">
            <div class="request-token-card">
                <h1 class="request-token-title">"litellm-rs"</h1>
                <p class="request-token-subtitle">"Request an API Key"</p>

                {move || error_msg.get().map(|msg| view! {
                    <div class="alert alert--error">{msg}</div>
                })}

                {move || created_token.get().map(|token| view! {
                    <TokenCreatedCard token=token on_dismiss=move || created_token.set(None)/>
                })}

                <Show when=move || created_token.get().is_none()>
                    <ActionForm action=action node_ref=form_ref>
                        <div class="request-token-form">
                            <div class="form-field form-field--full">
                                <label>"Your Name"</label>
                                <input type="text" name="name" placeholder="Jane Doe" required />
                            </div>
                            <div class="form-field form-field--full">
                                <label>"Email Address"</label>
                                <input type="email" name="email" placeholder="jane@example.com" required />
                            </div>
                        </div>
                        <button type="submit" class="btn btn--primary request-token-submit"
                            disabled=move || action.pending().get()
                        >
                            {move || if action.pending().get() { "Requesting..." } else { "Request API Key" }}
                        </button>
                        <p class="request-token-hint">
                            "If your email is approved, your API key will appear below."
                        </p>
                    </ActionForm>
                </Show>
            </div>
        </div>
    }
}

#[component]
fn TokenCreatedCard(
    token: TokenRequestResult,
    on_dismiss: impl Fn() + 'static + Copy + Send,
) -> impl IntoView {
    view! {
        <div class="token-created">
            <h2 class="token-created__title">"Your API Key"</h2>
            <p class="token-created__warning">
                "Copy this key now — it will not be shown again."
            </p>
            <div class="token-created__key">
                <code>{token.raw_key}</code>
            </div>
            <div class="token-created__details">
                <p>"Name: " {token.name}</p>
                {token.expires_at.map(|exp| view! {
                    <p>"Expires: " {exp}</p>
                })}
                {token.max_budget_usd.map(|budget| view! {
                    <p>"Budget: $" {budget}</p>
                })}
            </div>
            <button class="btn btn--secondary" on:click=move |_| on_dismiss()>
                "Request Another Key"
            </button>
        </div>
    }
}
