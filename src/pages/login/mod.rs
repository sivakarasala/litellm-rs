use crate::auth::clean_error;
use crate::auth::password::{LoginWithPassword, RegisterWithPassword};
use crate::db::Email;
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

    // Client-side validation signals
    let (login_email, set_login_email) = signal(String::new());
    let (reg_email, set_reg_email) = signal(String::new());
    let (reg_name, set_reg_name) = signal(String::new());
    let (reg_password, set_reg_password) = signal(String::new());

    let login_email_error = Memo::new(move |_| {
        let e = login_email.get();
        if e.is_empty() {
            return None;
        }
        Email::parse(e).err()
    });

    let reg_email_error = Memo::new(move |_| {
        let e = reg_email.get();
        if e.is_empty() {
            return None;
        }
        Email::parse(e).err()
    });

    let reg_name_error = Memo::new(move |_| {
        let n = reg_name.get();
        if n.is_empty() {
            return None;
        }
        let trimmed = n.trim();
        if trimmed.is_empty() {
            Some("Name is required".to_string())
        } else if trimmed.len() < 4 {
            Some("Name must be at least 4 characters".to_string())
        } else if trimmed.len() > 100 {
            Some("Name is too long".to_string())
        } else {
            None
        }
    });

    let reg_password_error = Memo::new(move |_| {
        let p = reg_password.get();
        if p.is_empty() {
            return None;
        }
        if p.len() < 8 {
            Some("Password must be at least 8 characters".to_string())
        } else if p.len() > 128 {
            Some("Password is too long".to_string())
        } else {
            None
        }
    });

    let login_has_errors =
        Memo::new(move |_| login_email.get().is_empty() || login_email_error.get().is_some());

    let reg_has_errors = Memo::new(move |_| {
        reg_email.get().is_empty()
            || reg_name.get().trim().is_empty()
            || reg_password.get().is_empty()
            || reg_email_error.get().is_some()
            || reg_name_error.get().is_some()
            || reg_password_error.get().is_some()
    });

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
                            <div class="field-group">
                                <input
                                    type="email"
                                    name="email"
                                    placeholder="Email"
                                    required
                                    on:input=move |ev| set_login_email.set(event_target_value(&ev))
                                />
                                {move || login_email_error.get().map(|e| view! {
                                    <span class="field-error">{e}</span>
                                })}
                            </div>
                            <input type="password" name="password" placeholder="Password" required />
                            <button
                                type="submit"
                                class="auth-submit"
                                class:auth-submit--success=move || login_success.get()
                                disabled=move || login_pending.get() || login_success.get() || login_has_errors.get()
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
                            <div class="field-group">
                                <input
                                    type="text"
                                    name="name"
                                    placeholder="Display name"
                                    required
                                    minlength="4"
                                    maxlength="100"
                                    on:input=move |ev| set_reg_name.set(event_target_value(&ev))
                                />
                                {move || reg_name_error.get().map(|e| view! {
                                    <span class="field-error">{e}</span>
                                })}
                            </div>
                            <div class="field-group">
                                <input
                                    type="email"
                                    name="email"
                                    placeholder="Email"
                                    required
                                    on:input=move |ev| set_reg_email.set(event_target_value(&ev))
                                />
                                {move || reg_email_error.get().map(|e| view! {
                                    <span class="field-error">{e}</span>
                                })}
                            </div>
                            <div class="field-group">
                                <input
                                    type="password"
                                    name="password"
                                    placeholder="Password (min 8 chars)"
                                    required
                                    minlength="8"
                                    on:input=move |ev| set_reg_password.set(event_target_value(&ev))
                                />
                                {move || reg_password_error.get().map(|e| view! {
                                    <span class="field-error">{e}</span>
                                })}
                            </div>
                            <button
                                type="submit"
                                class="auth-submit"
                                class:auth-submit--success=move || register_success.get()
                                disabled=move || register_pending.get() || register_success.get() || reg_has_errors.get()
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
