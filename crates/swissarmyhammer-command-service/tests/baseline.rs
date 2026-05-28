//! Plugin-catalog baseline test suite.
//!
//! Locks the builtin-command plugin catalog (`tests/baseline/plugins.yaml`) as
//! the cut-over contract for every plugin-port task. Submodules:
//!
//! - [`mod@baseline`] — the loader: parses `plugins.yaml` into typed
//!   [`PluginSpec`](baseline::PluginSpec) / [`CommandSpec`](baseline::CommandSpec)
//!   values, and pins the 12-file source-YAML set.
//! - [`catalog_self_check`] — catalog-internal invariants: tallies (7/12/62),
//!   id uniqueness, and backend-set membership.
//! - [`yaml_vs_catalog`] — the drift test over the pinned 12-file source set:
//!   positive (all 62 ids present), negative (no `nav.*`), and full per-command
//!   metadata fidelity.
//!
//! Run with `cargo test -p swissarmyhammer-command-service --test baseline`.
//!
//! The `#[path = ...]` attributes let this crate-root file index the
//! `tests/baseline/` directory without a `mod.rs` next to the submodules,
//! matching how `tests/integration.rs` aggregates `tests/integration/`.

#[path = "baseline/mod.rs"]
mod baseline;
