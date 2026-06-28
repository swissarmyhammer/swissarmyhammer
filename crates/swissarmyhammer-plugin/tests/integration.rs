//! Integration test suite for the `@swissarmyhammer/plugin` SDK helpers.
//!
//! Each submodule covers one slice of the SDK convention helpers
//! (`ensureServices` and `registerCommands`) when bound to a real `PluginHost`
//! with the command service bootstrapped:
//!
//! - [`ensure_services_e2e`] — two probe plugins both call
//!   `ensureServices(this, ["commands"])` in `load()`; the platform's
//!   idempotent registration policy merges them into one shared `commands`
//!   server, each plugin's commands land on the registry, and unloading one
//!   plugin purges only its commands.
//! - [`command_sdk_e2e`] — a single probe plugin registers commands through
//!   both the `registerCommands` convention helper AND the direct
//!   `this.commands.command.command.register(...)` form; both produce the
//!   same observable state, and unload purges both via the per-plugin
//!   ledger.
//!
//! Shared test helpers live in [`support`]. Every submodule lives under
//! `tests/integration/`; the `#[path = ...]` attributes make this
//! crate-root file act as the directory's index without needing a
//! `mod.rs` next to the submodules.

#[path = "integration/support.rs"]
mod support;

#[path = "integration/command_sdk_e2e.rs"]
mod command_sdk_e2e;
#[path = "integration/ensure_services_e2e.rs"]
mod ensure_services_e2e;
