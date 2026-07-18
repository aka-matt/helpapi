//! Compiled rule representation for the mock engine.
//!
//! This module re-exports [`CompiledRule`] from [`mock_core`].
//!
//! After a [`Config`](super::models::Config) is parsed and validated, routes are compiled
//! into [`CompiledRule`]s which contain pre-resolved matchers and transforms ready
//! for fast rule evaluation.
//!
//! Use [`Engine::compile`](mock_core::Engine::compile) to compile a JSON config
//! into an engine.

pub use mock_core::engine::{CompiledAction, CompiledRule};
