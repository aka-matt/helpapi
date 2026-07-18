//! Configuration model, parsing, and validation.
//!
//! This crate provides data models for the mock API configuration.
//! The types are serializable with serde and are consumed by mock-core and mock-cli.

pub mod error;
pub mod models;
pub mod parse;
pub mod schema;
pub mod validate;

pub use error::ConfigError;
pub use models::{
    Action, BodyData, Config, DefaultsConfig, MatchRule, Response, Route, ServerConfig, Transform,
    UnmatchedConfig, ValidationIssue, ValidationSeverity,
};
pub use parse::{parse_and_validate, parse_json};
pub use schema::generate_schema;
pub use validate::validate;
