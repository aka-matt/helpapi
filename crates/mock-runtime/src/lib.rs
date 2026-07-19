//! Service lifecycle, state, and event dispatch for the mock API runtime.
//!
//! The [`Runtime`] struct orchestrates the HTTP server and core engine,
//! managing the full lifecycle: start, stop, reload with atomic config replacement,
//! and event subscription for monitoring.
//!
//! ## Usage
//!
//! ```ignore
//! use mock_runtime::Runtime;
//!
//! let runtime = Runtime::new(config_json).await?;
//! let mut events = runtime.subscribe();
//!
//! runtime.start().await?;
//!
//! // ... use runtime ...
//!
//! runtime.reload(new_config_json).await?;
//! runtime.stop().await?;
//! ```

pub mod error;
pub mod runtime;

pub use error::RuntimeError;
pub use runtime::{EventReceiver, Runtime, RuntimeStatus};
