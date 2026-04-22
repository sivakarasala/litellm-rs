#![recursion_limit = "1024"]

pub mod app;
#[cfg(feature = "ssr")]
pub mod audit;
pub mod auth;
pub mod components;
#[cfg(feature = "ssr")]
pub mod configuration;
pub mod db;
pub mod error;
pub mod keys;
pub mod pages;
#[cfg(feature = "ssr")]
pub mod routes;
#[cfg(feature = "ssr")]
pub mod telemetry;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
