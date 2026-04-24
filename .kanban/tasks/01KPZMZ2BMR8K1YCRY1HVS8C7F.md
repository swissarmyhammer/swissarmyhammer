---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff9980
title: UIState must be self-loading without the GUI crate
---
## What

PR #40 review comment from @wballard on `kanban-app/src/state.rs:460`:

> I do not expect to see this here -- the UIState should be self loading without any specific UI at all

Ref: https://github.com/swissarmyhammer/swissarmyhammer/pull/40#discussion_r3137375132

Currently UIState is constructed inside `AppState::with_ui_state_path` in the GUI crate (`kanban-app/src/state.rs`). The loading logic — resolving the persistence path, reading the file, seeding defaults, wiring the commands-registry stacking — happens in that constructor. A CLI, MCP-only, or headless test that wants a live `UIState` must either depend on `kanban-app` (impossible; it's the top-level GUI crate) or re-implement the loading by hand.

The reviewer's architectural position: UIState should load itself. The GUI crate should receive an already-loaded `UIState` (or call a static factory on the trait/type), not host the loading logic.

### Scope

- Move the persistence-path resolution, file read, defaults seeding, and any startup hooks from `AppState::with_ui_state_path` into `UIState` itself (likely `swissarmyhammer-commands/src/ui_state.rs`). Expose something like `UIState::load(path: &Path) -> Result<Self>` or `UIState::load_or_default(...)`.
- The commands-registry stacking done in `with_ui_state_path` is a separate concern (the `swissarmyhammer_commands::builtin_yaml_sources()` + `swissarmyhammer_kanban::builtin_yaml_sources()` stacking). Decide whether that registry composition belongs on `UIState`, on `CommandsRegistry::from_stacked_defaults()`, or stays in `AppState`. A registry self-builder feels right — `AppState` then just asks for one.
- GUI crate should call `UIState::load(path)` (or equivalent) and pass the resulting state into its constructor; no domain knowledge about file formats or defaults should remain in `kanban-app`.

### Files to modify

- `kanban-app/src/state.rs::AppState::with_ui_state_path` — strip down to a thin wrapper that takes an already-loaded `UIState` and registry.
- `swissarmyhammer-commands/src/ui_state.rs` — add `UIState::load` / `UIState::load_or_default` (or equivalent) that encapsulates the loading semantics currently in `state.rs`.
- `swissarmyhammer-commands/src/registry.rs` — add a convenience like `CommandsRegistry::from_default_sources(include_kanban: bool)` or a builder that crates compose, so `AppState` no longer stacks sources by hand.
- Any test/CLI that constructs a `UIState` — simplify to `UIState::load(...)`.

## Acceptance Criteria

- [x] `kanban-app/src/state.rs` no longer contains file-reading or defaults logic for UIState; it delegates to the self-loading API.
- [x] A headless Rust integration test (in `swissarmyhammer-commands/tests/` or `swissarmyhammer-kanban/tests/`) can load a UIState from a fixture path WITHOUT depending on `kanban-app`, `tauri`, or any GUI crate.
- [x] Registry stacking is available as a builder method on `CommandsRegistry` (or a free fn in the commands crate), so consumers ask for "the default registry" instead of assembling it site by site.
- [x] No regression: the live app still loads the same UIState shape it did before, with the same defaults and the same file contents.

## Tests

- [x] Add `swissarmyhammer-commands/tests/ui_state_load.rs` (new) — construct a temp dir, write a UIState fixture, call the new self-loading API, assert the resulting UIState matches the fixture.
- [x] Add a companion test for the "no file exists yet" path — assert defaults are seeded correctly.
- [x] Extend `kanban-app` tests (if any construct `AppState`) so they now pass an already-loaded UIState.
- [x] `cargo test -p swissarmyhammer-commands -p swissarmyhammer-kanban -p kanban-app` passes.

## Implementation Notes (done)

New API surface:

- `UIState::load_from_xdg(app_subdir: &str) -> Self` in `swissarmyhammer-commands/src/ui_state.rs` — resolves `$XDG_CONFIG_HOME/sah/<app_subdir>/ui-state.yaml` via `swissarmyhammer-directory`, delegates to existing `UIState::load(path)`. Falls back to `./{app_subdir}/ui-state.yaml` on XDG error.
- `swissarmyhammer_kanban::default_builtin_yaml_sources()` — public non-test replacement for `test_support::composed_builtin_yaml_sources`; stacks generic + kanban YAMLs in the documented order.
- `swissarmyhammer_kanban::default_commands_registry() -> CommandsRegistry` — self-composing factory for the full default registry.
- `swissarmyhammer_kanban::default_commands_registry_with_overrides(&[(String, String)]) -> CommandsRegistry` — same, plus user overrides from `.kanban/commands/`.

`AppState::with_ui_state_path` → `AppState::with_ui_state(UIState)`; `AppState::new()` funnels through `UIState::load_from_xdg(CONFIG_APP_SUBDIR)`. `reload_command_overrides` delegates to `default_commands_registry_with_overrides`. The `ui_state_file_path` helper, the `UI_STATE_FILE_NAME` constant, and the `swissarmyhammer-directory` dep are all gone from `kanban-app`.

`test_support::composed_builtin_yaml_sources` now delegates to the public `default_builtin_yaml_sources` so tests and production exercise one composition path.

## Workflow

Use `/tdd`. Write a headless load test first (it'll fail to compile until `UIState::load` exists), then hoist the loading logic. Keep the API shape minimal — `load(path)` and `load_or_default(path)` are enough. #commands #refactor #architecture #uistate

## Review Findings (2026-04-24 07:20)

### Warnings

- [x] `swissarmyhammer-commands/Cargo.toml:18` — Adding `swissarmyhammer-directory` as a workspace dep on `swissarmyhammer-commands` violates the documented Tier 0 placement of this crate in `ARCHITECTURE.md` ("Tier 0 — Leaves: Zero workspace dependencies. ...the `Command` trait (`swissarmyhammer-commands`)..."). The crate now has its first sibling-crate dep, transitively pulling `swissarmyhammer-directory` into every consumer (`swissarmyhammer-entity`, `swissarmyhammer-kanban`, `kanban-app`). Pick one fix: (a) host `load_from_xdg` in a higher-tier crate (e.g. `swissarmyhammer-kanban` adds `swissarmyhammer-directory` and exposes `default_ui_state()`/equivalent that returns a loaded `UIState`); or (b) update ARCHITECTURE.md to acknowledge that `swissarmyhammer-commands` now sits at Tier 1 (Foundation) since it depends on `swissarmyhammer-directory`. Option (a) keeps the trait crate pure and matches the original PR-comment intent (UIState is self-loading from path; XDG resolution is a higher-tier concern).

  **Resolved**: Took option (a). Removed `swissarmyhammer-directory` from `swissarmyhammer-commands/Cargo.toml`; deleted `UIState::load_from_xdg`, `UIState::xdg_config_path`, and the `UI_STATE_FILE_NAME` const from `swissarmyhammer-commands/src/ui_state.rs`. Added `swissarmyhammer-directory` as a dep on `swissarmyhammer-kanban` and introduced `swissarmyhammer_kanban::default_ui_state(app_subdir: &str) -> UIState` plus a private `ui_state_xdg_config_path` helper in `swissarmyhammer-kanban/src/lib.rs`. `kanban-app/src/state.rs::AppState::new` now calls `swissarmyhammer_kanban::default_ui_state(CONFIG_APP_SUBDIR)`. `swissarmyhammer-commands` is back to Tier 0 — its only non-`std` imports are shared `serde`/`tracing`/`thiserror`/`include_dir` leaves.

- [x] `swissarmyhammer-commands/tests/ui_state_load.rs:91-131` — `load_from_xdg_reads_under_xdg_config_home` mutates `XDG_CONFIG_HOME` without `serial_test::serial` and without saving/restoring the developer's prior value. If a developer runs this test with `XDG_CONFIG_HOME` already set, the test silently destroys it (it does `remove_var` on cleanup, not a restore). The codebase already establishes the right pattern in `swissarmyhammer-directory/src/file_loader.rs:1107-1127` — `#[serial]` + capture original via `std::env::var(...).ok()` + restore original on cleanup. Apply the same pattern here: add `serial_test = { workspace = true }` to `swissarmyhammer-commands` dev-deps, mark the test `#[serial]`, and replace the bare `remove_var` with a save/restore block. Bonus: consider extracting the path-resolver into a `pub(crate)` helper that takes a base dir, so the test exercises path composition without env mutation at all.

  **Resolved**: Moved the XDG test into `swissarmyhammer-kanban/tests/default_ui_state.rs` (it had to move anyway — the helper it exercises now lives on `swissarmyhammer-kanban`). Added `serial_test = { workspace = true }` to `swissarmyhammer-kanban` dev-deps, tagged the test `#[serial]`, and replaced the bare `remove_var` with the save/restore pattern copied from `swissarmyhammer-directory/src/file_loader.rs`. Added a companion `default_ui_state_without_file_returns_defaults` test so the fresh-install branch is also covered through the XDG entry point.

### Nits

- [x] `swissarmyhammer-commands/tests/ui_state_load.rs:104-113` — The SAFETY comment for `unsafe { std::env::set_var(...) }` argues "the helper reads it synchronously on the same thread before any mutation could happen — safe in the single-threaded context of this test". This reasoning is wrong: env mutation is UB if *any* other thread (including std-library `getenv` calls from `tempfile`, the test runner, etc.) reads concurrently — it's not about the helper's read order. The test happens to pass because no other test in this binary touches XDG, but the SAFETY justification should reflect the real reason (binary is small, no parallel reader exists in this test set), not invent a thread-ordering argument. Consider deleting the comment entirely if the test gets `#[serial]`-protected per the warning above.

  **Resolved**: The old wrong SAFETY comment is gone (the test moved). The new XDG test in `swissarmyhammer-kanban/tests/default_ui_state.rs` carries a SAFETY comment that names the real reason: `#[serial]` serializes this test against every other env-mutating test in the binary, so no sibling test can hit libc `getenv` while the `set_var` is in flight.

- [x] `swissarmyhammer-commands/src/ui_state.rs:375` — `xdg_config_path` is private. If a CLI or downstream caller ever needs to display "where will UIState be saved?" without loading it, there's no API for that. Not a current need — flag only because the function is the only piece of the new surface that isn't exposed. Leave private until a caller needs it.

  **Resolved**: The function moved to `swissarmyhammer-kanban/src/lib.rs` as `ui_state_xdg_config_path` (`pub(crate)` — scope-narrowed further than the original `fn` on `UIState`). Kept private per the reviewer's "leave private until a caller needs it" guidance.