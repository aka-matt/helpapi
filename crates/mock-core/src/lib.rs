//! Core mock engine types and logic.
//!
//! This crate contains host-agnostic data types with zero dependencies
//! on tokio, axum, reqwest, ratatui, or wasm-bindgen.

pub mod body;
pub mod decision;
pub mod engine;
pub mod error;
pub mod forward;
pub mod matcher;
pub mod metadata;
pub mod request;
pub mod response;
pub mod transform;
pub mod validate;

// Re-exports for convenience
pub use body::BodyData;
pub use decision::Decision;
pub use error::EngineError;
pub use forward::ForwardPlan;
pub use matcher::{
    CompositeMatcher, HeaderMatcher, Matcher, MatcherResult, MethodMatcher, PathMatcher,
    QueryMatcher,
};
pub use metadata::DecisionMetadata;
pub use request::{MatchedRequestContext, RequestData};
pub use response::ResponseData;
// `Transform` is the trait; the typed enum lives at `transform::spec::Transform`.
pub use transform::TransformError;
// Re-export trait at the same name as before; enum goes via `mock_core::transform::spec`.
pub use transform::Transform;

// Re-export individual transforms for convenience
pub use transform::{
    RemoveHeader, RemoveJsonPointer, RemoveQuery, ReplaceBody, SetHeader, SetJsonPointer, SetQuery,
    SetStatus, TemplateSubst,
};

// Re-export Engine
pub use engine::Engine;
