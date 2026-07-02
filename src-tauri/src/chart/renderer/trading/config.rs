//! Runtime configuration for the ApexIB broker client.
//!
//! Precedence (highest → lowest):
//!   1. Runtime override via `set_apexib_url`
//!   2. Env var `APEXIB_URL`
//!   3. Compiled default

use std::sync::{RwLock, OnceLock};

const DEFAULT_URL: &str = "https://apexib-dev.xllio.com";

static URL: OnceLock<RwLock<String>> = OnceLock::new();

fn url_cell() -> &'static RwLock<String> {
    URL.get_or_init(|| RwLock::new(load_initial_url()))
}

fn load_initial_url() -> String {
    std::env::var("APEXIB_URL").unwrap_or_else(|_| DEFAULT_URL.into())
}

pub fn apexib_url() -> String {
    url_cell().read().map(|g| g.clone()).unwrap_or_else(|_| DEFAULT_URL.into())
}

pub fn set_apexib_url(url: impl Into<String>) {
    if let Ok(mut g) = url_cell().write() { *g = url.into(); }
}
