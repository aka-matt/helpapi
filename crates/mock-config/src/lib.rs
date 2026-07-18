//! Configuration model, parsing, and validation.
//!
//! This crate provides data models for the mock API configuration.
//! The types are serializable with serde and are consumed by mock-core and mock-cli.

pub mod error;
pub mod models;

pub use error::ConfigError;
pub use models::{
    Action, BodyData, Config, DefaultsConfig, MatchRule, Response, Route, ServerConfig,
    Transform, UnmatchedConfig, ValidationIssue, ValidationSeverity,
};
