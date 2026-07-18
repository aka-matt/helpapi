//! Core mock engine types and logic.
//!
//! This crate contains host-agnostic data types with zero dependencies
//! on tokio, axum, reqwest, ratatui, or wasm-bindgen.

pub mod body;
pub mod decision;
pub mod error;
pub mod forward;
pub mod matcher;
pub mod metadata;
pub mod request;
pub mod response;
pub mod transform;

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
pub use transform::{Transform, TransformError};

// Re-export individual transforms for convenience
pub use transform::{
    RemoveHeader, RemoveJsonPointer, RemoveQuery, ReplaceBody, SetHeader, SetJsonPointer, SetQuery,
    SetStatus, TemplateSubst,
};

/// Core mock engine.
pub struct Engine;
