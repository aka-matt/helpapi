//! TUI module for the mock API CLI.
//!
//! This module provides a Ratatui-based terminal user interface for
//! monitoring and interacting with the mock API runtime.

mod app;
mod event_handler;
mod state;
pub mod widgets;

pub use app::App;
