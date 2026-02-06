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
