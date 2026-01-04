//! Logging utilities for SwissArmyHammer
//!
//! This module provides utilities for formatting and displaying log messages.

use std::fmt::Debug;

/// Wrapper for pretty-printing Debug types in logs
///
/// Use this in tracing statements to automatically format complex types
/// with multi-line indentation:
///
/// ```ignore
/// use swissarmyhammer_common::Pretty;
/// use tracing::info;
///
/// let config = MyConfig { /* ... */ };
/// info!("Config: {}", Pretty(&config));
/// ```
///
/// This will use the `{:#?}` formatting internally to produce readable output.
pub struct Pretty<T: Debug>(pub T);

impl<T: Debug> std::fmt::Display for Pretty<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self.0)
    }
}

impl<T: Debug> std::fmt::Debug for Pretty<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self.0)
    }
}
