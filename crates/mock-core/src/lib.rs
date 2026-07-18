//! Core mock engine types and logic.
//!
//! This crate contains host-agnostic data types with zero dependencies
//! on tokio, axum, reqwest, ratatui, or wasm-bindgen.

pub mod body;
pub mod forward;
pub mod metadata;
pub mod request;
pub mod response;

// Re-exports for convenience
pub use body::BodyData;
pub use forward::ForwardPlan;
pub use metadata::DecisionMetadata;
pub use request::{MatchedRequestContext, RequestData};
pub use response::ResponseData;

/// Core mock engine.
pub struct Engine;
