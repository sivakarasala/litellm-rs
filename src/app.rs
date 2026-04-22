use crate::auth::{get_me, AuthUser, Logout};
use crate::pages::{
    AuditPage, DashboardPage, KeysPage, LoginPage, RequestTokenPage, SettingsPage, UsagePage,
};
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::use_location,
    StaticSegment,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <link rel="icon" type="image/png" href="/favicon.png"/>
                <link rel="preconnect" href="https://fonts.googleapis.com"/>
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous"/>
                <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    let user = Resource::new(|| (), |_| get_me());

    view! {
        <Stylesheet id="leptos" href="/pkg/litellm-rs.css"/>
        <Title text="litellm-rs"/>

        <Router>
            <Transition fallback=|| view! { <div class="loading-screen">"Loading..."</div> }>
                {move || {
                    user.get().map(|result| {
                        let auth_user = match result {
                            Ok(Some(u)) => Some(u),
                            _ => None,
                        };
                        let is_authed = auth_user.is_some();

                        view! {
                            <Show when=move || is_authed>
                                <Sidebar user=auth_user.clone().unwrap()/>
                            </Show>
                            <div class=move || if is_authed { "main-content" } else { "main-content--full" }>
                                <Routes fallback=|| "Page not found.".into_view()>
                                    <Route path=StaticSegment("login") view=LoginPage/>
                                    <Route path=StaticSegment("keys") view=KeysPage/>
                                    <Route path=StaticSegment("usage") view=UsagePage/>
                                    <Route path=StaticSegment("audit") view=AuditPage/>
                                    <Route path=StaticSegment("request-token") view=RequestTokenPage/>
                                    <Route path=StaticSegment("settings") view=SettingsPage/>
                                    <Route path=StaticSegment("") view=move || {
                                        if is_authed { view! { <DashboardPage/> }.into_any() }
                                        else { view! { <LoginPage/> }.into_any() }
                                    }/>
                                </Routes>
                            </div>
                        }
                        .into_any()
                    })
                }}
            </Transition>
        </Router>
    }
}

#[component]
fn Sidebar(user: AuthUser) -> impl IntoView {
    let pathname = use_location().pathname;
    let logout = ServerAction::<Logout>::new();

    // Redirect after logout
    Effect::new(move |_| {
        if let Some(Ok(_)) = logout.value().get() {
            #[cfg(feature = "hydrate")]
            let _ = js_sys::eval("window.location.href = '/login'");
        }
    });

    view! {
        <aside class="sidebar">
            <div class="sidebar__logo">"litellm-rs"</div>

            <nav class="sidebar__nav">
                <a href="/" class="sidebar__link" class:active=move || pathname.get() == "/">
                    "Dashboard"
                </a>
                <a href="/keys" class="sidebar__link" class:active=move || pathname.get().starts_with("/keys")>
                    "API Keys"
                </a>
                <a href="/usage" class="sidebar__link" class:active=move || pathname.get().starts_with("/usage")>
                    "Usage"
                </a>
                <a href="/audit" class="sidebar__link" class:active=move || pathname.get().starts_with("/audit")>
                    "Audit Log"
                </a>
                <a href="/settings" class="sidebar__link" class:active=move || pathname.get().starts_with("/settings")>
                    "Settings"
                </a>
            </nav>

            <div class="sidebar__footer">
                <div class="sidebar__user">
                    <span class="sidebar__avatar">{user.initials()}</span>
                    <span class="sidebar__email">{user.email.to_string()}</span>
                </div>
                <ActionForm action=logout>
                    <button type="submit" class="sidebar__logout">"Sign out"</button>
                </ActionForm>
            </div>
        </aside>
    }
}
