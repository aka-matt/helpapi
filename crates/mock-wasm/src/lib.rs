//! WASM bindings for the mock engine.
//!
//! This crate provides WASM-safe bindings to the `mock-core` engine,
//! allowing it to be used in browser and Node.js environments.

mod engine;
mod error;

pub use engine::WasmMockEngine;
pub use error::WasmError;

use mock_config::parse_and_validate;
use wasm_bindgen::prelude::*;

/// Validates a JSON config string.
///
/// Returns a JSON string with `ok: true` on success, or `ok: false` with
/// an `error` field containing the validation message on failure.
#[wasm_bindgen]
pub fn validate_config(config_json: &str) -> String {
    match parse_and_validate(config_json) {
        Ok(_) => r#"{"ok":true}"#.to_string(),
        Err(e) => serde_json::json!({
            "ok": false,
            "error": e.to_string()
        })
        .to_string(),
    }
}
