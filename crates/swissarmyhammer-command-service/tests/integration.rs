//! Integration test suite for the command service's host bootstrap.
//!
//! Each submodule covers one slice of the bootstrap wiring:
//!
//! - [`builtin_task_commands_e2e`] — the committed `task-commands` builtin
//!   plugin (port of `task.yaml`) is discovered from a builtin layer,
//!   registers its three commands with 1:1 metadata fidelity, and moves a
//!   real task on the exposed `kanban` store through `task.move`.
//! - [`builtin_file_commands_e2e`] — the committed `file-commands` builtin
//!   plugin (port of `file.yaml`) is discovered from a builtin layer,
//!   registers its four board-file lifecycle commands with 1:1 metadata
//!   fidelity, and drives the `window` board-lifecycle verbs over a recording
//!   shell (with `file.newBoard` exercising the board-creation effect).
//! - [`host_bootstrap_e2e`] — the bootstrap exposes `commands` on the
//!   host's registry, and a host caller's `register command` lands on the
//!   override stack and surfaces through `list command`.
//! - [`unload_cleanup_e2e`] — a loaded plugin's registrations are purged
//!   automatically when the plugin is unloaded, with no zombie ledger
//!   state left behind.
//! - [`override_stack_e2e`] — the headline scenario: A registers, B
//!   overrides, B unloads → A re-emerges, A unloads → the host's original
//!   registration is active again.
//! - [`mcp_notifications_e2e`] — the notification bridge's four planes:
//!   one multi-write command's `store/changed` events share a `txn` and
//!   carry `origin:"user"`, the `commands/executed` action event is
//!   delivered, a perspective edit produces a `store/changed{store:
//!   "perspective"}` with no `changes`, a palette toggle produces
//!   `ui_state/changed`, an undo emits the inverse batch under a new `txn`
//!   with `origin:"undo"`, and an external client receives the same stream.
//!
//! Shared test helpers live in [`support`]. Every submodule lives under
//! `tests/integration/`; the `#[path = ...]` attributes make this
//! crate-root file act as the directory's index without needing a
//! `mod.rs` next to the submodules.

#[path = "integration/support.rs"]
mod support;

#[path = "integration/builtin_app_shell_commands_e2e.rs"]
mod builtin_app_shell_commands_e2e;
#[path = "integration/builtin_entity_commands_e2e.rs"]
mod builtin_entity_commands_e2e;
#[path = "integration/builtin_file_commands_e2e.rs"]
mod builtin_file_commands_e2e;
#[path = "integration/builtin_kanban_misc_e2e.rs"]
mod builtin_kanban_misc_e2e;
#[path = "integration/builtin_perspective_commands_e2e.rs"]
mod builtin_perspective_commands_e2e;
#[path = "integration/builtin_task_commands_e2e.rs"]
mod builtin_task_commands_e2e;
#[path = "integration/builtin_ui_commands_e2e.rs"]
mod builtin_ui_commands_e2e;
#[path = "integration/host_bootstrap_e2e.rs"]
mod host_bootstrap_e2e;
#[path = "integration/mcp_notifications_e2e.rs"]
mod mcp_notifications_e2e;
#[path = "integration/override_stack_e2e.rs"]
mod override_stack_e2e;
#[path = "integration/undo_redo_notifies_dependents_e2e.rs"]
mod undo_redo_notifies_dependents_e2e;
#[path = "integration/unload_cleanup_e2e.rs"]
mod unload_cleanup_e2e;
