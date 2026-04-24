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
/// Delegates to [`crate::default_builtin_yaml_sources`] — the public
/// non-test version of this helper. Kept here so existing tests keep
/// compiling without a mass rename; new test code should prefer
/// `default_builtin_yaml_sources` so production and tests exercise the
/// same composition path.
pub fn composed_builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
    crate::default_builtin_yaml_sources()
}
