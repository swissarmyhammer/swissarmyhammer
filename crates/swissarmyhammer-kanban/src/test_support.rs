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
/// Mirrors the contributor list used by the app layer's
/// `swissarmyhammer_commands::compose_registry!` invocation
/// (`swissarmyhammer_commands` then this crate), so production and tests
/// exercise the same composition path. The macro can't be used directly
/// here because it expects a `::`-separated identifier path and this
/// crate's contribution lives at our crate root (the `crate` keyword
/// isn't an identifier).
pub fn composed_builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
    let mut sources: Vec<(&'static str, &'static str)> = Vec::new();
    sources.extend(swissarmyhammer_commands::builtin_yaml_sources());
    sources.extend(crate::builtin_yaml_sources());
    sources
}
