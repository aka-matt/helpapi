//! Transform types and implementations.
//!
//! Transforms modify requests and responses according to configured rules.

mod body;
mod headers;
mod json;
mod query;
mod template;

pub use body::{ReplaceBody, SetStatus};
pub use headers::{RemoveHeader, SetHeader};
pub use json::{RemoveJsonPointer, SetJsonPointer};
pub use query::{RemoveQuery, SetQuery};
pub use template::TemplateSubst;

// Re-export TransformError at module level
pub use json::TransformError;

/// The Transform trait defines the interface for request and response transforms.
///
/// Implementors must be thread-safe (Send + Sync) to allow sharing across async tasks.
pub trait Transform: Send + Sync {
    /// Transform a request.
    ///
    /// Returns the transformed request or an error if transformation failed.
    fn transform_request(
        &self,
        request: crate::RequestData,
    ) -> Result<crate::RequestData, TransformError>;

    /// Transform a response.
    ///
    /// Returns the transformed response or an error if transformation failed.
    fn transform_response(
        &self,
        response: crate::ResponseData,
    ) -> Result<crate::ResponseData, TransformError>;

    /// Returns the name of this transform for debugging/logging purposes.
    fn name(&self) -> &str;
}
