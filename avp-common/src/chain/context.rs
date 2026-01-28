//! Chain context for passing state through the chain.

use std::collections::HashMap;

use serde::{de::DeserializeOwned, Serialize};

use crate::error::ValidationError;

/// Context passed through the chain for state sharing between links.
#[derive(Debug, Default)]
pub struct ChainContext {
    /// Key-value store for passing arbitrary typed data between chain links.
    /// Links can use `set()` and `get()` to share state during processing.
    state: HashMap<String, serde_json::Value>,

    /// Validation errors accumulated during chain processing.
    /// Links can add errors via `add_validation_error()`.
    validation_errors: Vec<ValidationError>,

    /// Process exit code, defaulting to 0. Links can modify this via `set_exit_code()`
    /// to signal specific outcomes (e.g., validator blocking uses exit code 2).
    exit_code: i32,
}

impl ChainContext {
    /// Create a new empty chain context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a value in the context.
    pub fn set<T: Serialize>(&mut self, key: &str, value: T) {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.state.insert(key.to_string(), json_value);
        }
    }

    /// Get a value from the context.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.state
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Check if a key exists in the context.
    pub fn contains(&self, key: &str) -> bool {
        self.state.contains_key(key)
    }

    /// Remove a value from the context.
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.state.remove(key)
    }

    /// Add a validation error to the context.
    pub fn add_validation_error(&mut self, error: ValidationError) {
        self.validation_errors.push(error);
    }

    /// Get all validation errors.
    pub fn validation_errors(&self) -> &[ValidationError] {
        &self.validation_errors
    }

    /// Check if there are any validation errors.
    pub fn has_validation_errors(&self) -> bool {
        !self.validation_errors.is_empty()
    }

    /// Set the exit code.
    pub fn set_exit_code(&mut self, code: i32) {
        self.exit_code = code;
    }

    /// Get the exit code.
    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_set_get() {
        let mut ctx = ChainContext::new();
        ctx.set("key", "value");
        let value: Option<String> = ctx.get("key");
        assert_eq!(value, Some("value".to_string()));
    }

    #[test]
    fn test_context_set_get_complex() {
        let mut ctx = ChainContext::new();
        ctx.set("numbers", vec![1, 2, 3]);
        let value: Option<Vec<i32>> = ctx.get("numbers");
        assert_eq!(value, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_context_validation_errors() {
        let mut ctx = ChainContext::new();
        assert!(!ctx.has_validation_errors());

        ctx.add_validation_error(ValidationError::MissingField("test".to_string()));
        assert!(ctx.has_validation_errors());
        assert_eq!(ctx.validation_errors().len(), 1);
    }

    #[test]
    fn test_context_exit_code() {
        use crate::chain::VALIDATOR_BLOCK_EXIT_CODE;

        let mut ctx = ChainContext::new();
        assert_eq!(ctx.exit_code(), 0);

        ctx.set_exit_code(VALIDATOR_BLOCK_EXIT_CODE);
        assert_eq!(ctx.exit_code(), VALIDATOR_BLOCK_EXIT_CODE);
    }
}
