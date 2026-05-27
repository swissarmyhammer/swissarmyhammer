//! Integration test suite for the command service's host bootstrap.
//!
//! Each submodule covers one slice of the bootstrap wiring:
//!
//! - [`host_bootstrap_e2e`] — the bootstrap exposes `commands` on the
//!   host's registry, and a host caller's `register command` lands on the
//!   override stack and surfaces through `list command`.
//! - [`unload_cleanup_e2e`] — a loaded plugin's registrations are purged
//!   automatically when the plugin is unloaded, with no zombie ledger
//!   state left behind.
//! - [`override_stack_e2e`] — the headline scenario: A registers, B
//!   overrides, B unloads → A re-emerges, A unloads → the host's original
//!   registration is active again.
//!
//! Shared test helpers live in [`support`]. Every submodule lives under
//! `tests/integration/`; the `#[path = ...]` attributes make this
//! crate-root file act as the directory's index without needing a
//! `mod.rs` next to the submodules.

#[path = "integration/support.rs"]
mod support;

#[path = "integration/host_bootstrap_e2e.rs"]
mod host_bootstrap_e2e;
#[path = "integration/override_stack_e2e.rs"]
mod override_stack_e2e;
#[path = "integration/unload_cleanup_e2e.rs"]
mod unload_cleanup_e2e;
