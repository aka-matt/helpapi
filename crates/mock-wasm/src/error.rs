//! Error types for WASM bindings.
//!
//! These errors convert to JsValue for propagation to JavaScript.

use wasm_bindgen::JsValue;

/// Errors that can occur during WASM operations.
#[derive(Debug)]
pub enum WasmError {
    /// Configuration parsing/validation error.
    Config(String),
    /// Engine compilation error.
    Compile(String),
    /// Decision error.
    Decision(String),
    /// Transform error.
    Transform(String),
    /// JSON serialization/deserialization error.
    Serialization(String),
}

impl From<WasmError> for JsValue {
    fn from(err: WasmError) -> Self {
        match err {
            WasmError::Config(msg) => JsValue::from_str(&format!("Config error: {}", msg)),
            WasmError::Compile(msg) => JsValue::from_str(&format!("Compile error: {}", msg)),
            WasmError::Decision(msg) => JsValue::from_str(&format!("Decision error: {}", msg)),
            WasmError::Transform(msg) => JsValue::from_str(&format!("Transform error: {}", msg)),
            WasmError::Serialization(msg) => {
                JsValue::from_str(&format!("Serialization error: {}", msg))
            }
        }
    }
}

impl From<mock_config::ConfigError> for WasmError {
    fn from(err: mock_config::ConfigError) -> Self {
        WasmError::Config(err.to_string())
    }
}

impl From<mock_core::EngineError> for WasmError {
    fn from(err: mock_core::EngineError) -> Self {
        match err {
            mock_core::EngineError::CompileError(msg) => WasmError::Compile(msg),
            mock_core::EngineError::MatchError(msg) => WasmError::Decision(msg),
            mock_core::EngineError::TransformError(msg) => WasmError::Transform(msg),
            mock_core::EngineError::UnsupportedBodyType(msg) => WasmError::Transform(msg),
        }
    }
}

impl From<serde_json::Error> for WasmError {
    fn from(err: serde_json::Error) -> Self {
        WasmError::Serialization(err.to_string())
    }
}
