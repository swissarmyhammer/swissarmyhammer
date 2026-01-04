//! Logging utilities for SwissArmyHammer
//!
//! This module provides utilities for formatting and displaying log messages.

use serde::Serialize;
use std::fmt::Debug;

/// Wrapper for pretty-printing types in logs as YAML
///
/// Use this in tracing statements to automatically format complex types
/// as YAML with a newline before the content:
///
/// ```ignore
/// use swissarmyhammer_common::Pretty;
/// use tracing::info;
///
/// let config = MyConfig { /* ... */ };
/// info!("Config: {}", Pretty(&config));
/// ```
///
/// Outputs YAML format with a leading newline. Types must implement Serialize + Debug.
/// Debug is used as a fallback if YAML serialization fails.
pub struct Pretty<T>(pub T);

impl<T: Serialize + Debug> std::fmt::Display for Pretty<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match serde_yaml::to_string(&self.0) {
            Ok(yaml) => write!(f, "\n{}", yaml),
            Err(_) => write!(f, "\n{:#?}", self.0),
        }
    }
}

impl<T: Serialize + Debug> std::fmt::Debug for Pretty<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match serde_yaml::to_string(&self.0) {
            Ok(yaml) => write!(f, "\n{}", yaml),
            Err(_) => write!(f, "\n{:#?}", self.0),
        }
    }
}
