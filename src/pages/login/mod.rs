use crate::auth::clean_error;
use crate::auth::password::{LoginWithPassword, RegisterWithPassword};
use leptos::prelude::*;

#[component]
pub fn LoginPage() -> impl IntoView {
    let login = ServerAction::<LoginWithPassword>::new();
    let register = ServerAction::<RegisterWithPassword>::new();

    let (show_register, set_show_register) = signal(false);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);

    let login_pending = login.pending();
    let register_pending = register.pending();
    let login_success = RwSignal::new(false);
    let register_success = RwSignal::new(false);

    let register_form: NodeRef<leptos::html::Form> = NodeRef::new();

    // Handle login result
    Effect::new(move |_| match login.value().get() {
        Some(Ok(_)) => {
            login_success.set(true);
            error_msg.set(None);
            #[cfg(feature = "hydrate")]
            let _ = js_sys::eval("setTimeout(() => { window.location.href = '/'; }, 500)");
        }
        Some(Err(e)) => error_msg.set(Some(clean_error(&e))),
        None => {}
    });

    // Handle register result
    Effect::new(move |_| match register.value().get() {
        Some(Ok(_)) => {
            if let Some(form) = register_form.get() {
                form.reset();
            }
            register_success.set(true);
            error_msg.set(None);
            #[cfg(feature = "hydrate")]
            let _ = js_sys::eval("setTimeout(() => { window.location.href = '/'; }, 500)");
        }
        Some(Err(e)) => error_msg.set(Some(clean_error(&e))),
        None => {}
    });

    view! {
        <div class="login-page">
            <div class="login-card">
                <h1 class="login-title">"litellm-rs"</h1>
                <p class="login-subtitle">"LLM Proxy with Token Management"</p>

                {move || error_msg.get().map(|msg| view! {
                    <div class="login-error">{msg}</div>
                })}

                <Show when=move || !show_register.get()>
                    <div class="auth-form">
                        <ActionForm action=login>
                            <input type="email" name="email" placeholder="Email" required />
                            <input type="password" name="password" placeholder="Password" required />
                            <button
                                type="submit"
                                class="auth-submit"
                                class:auth-submit--success=move || login_success.get()
                                disabled=move || login_pending.get() || login_success.get()
                            >
                                {move || if login_success.get() {
                                    "Signed in!"
                                } else if login_pending.get() {
                                    "Signing in..."
                                } else {
                                    "Sign in"
                                }}
                            </button>
                        </ActionForm>
                    </div>
                    <p class="auth-switch">
                        "No account? "
                        <button class="auth-link" on:click=move |_| {
                            set_show_register.set(true);
                            error_msg.set(None);
                        }>"Create one"</button>
                    </p>
                </Show>

                <Show when=move || show_register.get()>
                    <div class="auth-form">
                        <ActionForm action=register node_ref=register_form>
                            <input type="text" name="name" placeholder="Display name" required maxlength="100" />
                            <input type="email" name="email" placeholder="Email" required />
                            <input type="password" name="password" placeholder="Password (min 8 chars)" required minlength="8" />
                            <button
                                type="submit"
                                class="auth-submit"
                                class:auth-submit--success=move || register_success.get()
                                disabled=move || register_pending.get() || register_success.get()
                            >
                                {move || if register_success.get() {
                                    "Account created!"
                                } else if register_pending.get() {
                                    "Creating account..."
                                } else {
                                    "Create account"
                                }}
                            </button>
                        </ActionForm>
                    </div>
                    <p class="auth-switch">
                        "Have an account? "
                        <button class="auth-link" on:click=move |_| {
                            set_show_register.set(false);
                            error_msg.set(None);
                        }>"Sign in"</button>
                    </p>
                </Show>
            </div>
        </div>
    }
}
