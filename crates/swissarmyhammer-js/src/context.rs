//! JS execution context for the Operation pattern
//!
//! Wraps JsState to satisfy the Execute<C, E> trait's context parameter.

use crate::JsState;

/// Context for JS operations
///
/// This is the `C` type parameter for `Execute<JsContext, JsError>`.
/// It wraps the global JsState and provides access to the JS runtime.
pub struct JsContext {
    state: JsState,
}

impl JsContext {
    /// Create a new context wrapping the global JsState
    pub fn new() -> Self {
        Self {
            state: JsState::global(),
        }
    }

    /// Get a reference to the underlying JsState
    pub fn state(&self) -> &JsState {
        &self.state
    }
}

impl Default for JsContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_context() {
        let ctx = JsContext::new();
        // Verify state is accessible
        let _state = ctx.state();
    }

    #[test]
    fn test_default_creates_context() {
        let ctx = JsContext::default();
        let _state = ctx.state();
    }

    #[tokio::test]
    async fn test_context_state_can_evaluate() {
        let ctx = JsContext::new();
        let result = ctx.state().get("1 + 1").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(2));
    }
}
