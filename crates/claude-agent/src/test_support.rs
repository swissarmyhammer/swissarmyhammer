//! Shared fixtures for this crate's unit tests.
//!
//! Compiled only under `#[cfg(test)]` (see the module declaration in
//! `lib.rs`); integration tests under `tests/` are a separate crate target
//! and import the same fixtures from `agent_client_protocol_extras`
//! directly.

// The canonical XDG_STATE_HOME isolation guard lives next to the
// `SessionStore` it isolates; re-export it rather than carrying a per-crate
// copy. Callers must be `#[serial]` — see its docs.
pub(crate) use agent_client_protocol_extras::test_support::StateDirGuard;
