//! Test-only helpers shared between this crate's own tests and integration
//! tests.
//!
//! Gated on `#[cfg(any(test, feature = "test-support"))]` at the lib level so
//! only test builds pay for these helpers. See [`crate::test_support`] in
//! `lib.rs` for details.

/// Stack the commands-crate and kanban-crate builtin YAMLs the same way
/// `kanban-app/src/state.rs` does at startup, so tests see the same command
/// registry the running app sees.
///
/// The ordering (generic commands → kanban-specific → user overrides at the
/// call site) is load-bearing: later sources override earlier via the
/// partial-merge-by-id semantics in `CommandsRegistry::merge_yaml_value`.
///
/// This helper replaces six byte-identical copies that had accumulated across
/// this crate's integration tests and `scope_commands` unit tests; keep it
/// here as the single source of truth for "what the app's builtin command
/// registry looks like at startup."
pub fn composed_builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
    let commands = swissarmyhammer_commands::builtin_yaml_sources();
    let kanban = crate::builtin_yaml_sources();
    let mut out = Vec::with_capacity(commands.len() + kanban.len());
    out.extend(commands);
    out.extend(kanban);
    out
}
