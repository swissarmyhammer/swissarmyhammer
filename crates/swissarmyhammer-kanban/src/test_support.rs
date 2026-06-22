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
/// `crate::commands_core::compose_registry!` invocation
/// (`swissarmyhammer_commands` then this crate), so production and tests
/// exercise the same composition path. The macro can't be used directly
/// here because it expects a `::`-separated identifier path and this
/// crate's contribution lives at our crate root (the `crate` keyword
/// isn't an identifier).
pub fn composed_builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
    let mut sources: Vec<(&'static str, &'static str)> = Vec::new();
    sources.extend(crate::commands_core::builtin_yaml_sources());
    sources.extend(crate::builtin_yaml_sources());
    sources
}

/// Create a temporary, initialized board and return its `(TempDir, KanbanContext)`.
///
/// The `TempDir` is returned alongside the context so the caller keeps it alive
/// for the duration of the test — dropping it deletes the backing `.kanban`
/// directory. The board is initialized with `InitBoard::new("Test")`, which
/// creates the default columns, so the returned context is ready for command
/// execution.
///
/// This is the single source of truth for the `setup()` helper that the per-op
/// test modules previously duplicated verbatim.
pub async fn setup() -> (tempfile::TempDir, crate::KanbanContext) {
    use crate::board::InitBoard;
    use swissarmyhammer_operations::Execute;

    let temp = tempfile::TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = crate::KanbanContext::new(kanban_dir);

    InitBoard::new("Test")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    (temp, ctx)
}
