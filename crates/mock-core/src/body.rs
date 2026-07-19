//! Canonical request/response body data type.
//!
//! This is also re-exported from `mock_config::models::BodyData` so that
//! downstream crates continue to find the type under their existing import paths.

pub use crate::transform::spec::BodyData;
