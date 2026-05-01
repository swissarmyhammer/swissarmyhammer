---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff9180
title: 'Stack builtin commands per-crate: move kanban-specific YAMLs out of swissarmyhammer-commands'
---
## What

`swissarmyhammer-commands/src/lib.rs` states the crate is "consumer-agnostic — it knows nothing about kanban, tasks, or specific entity types." But `swissarmyhammer-commands/builtin/commands/` violates that contract — it ships kanban-specific YAML files (tasks, columns, tags, attachments, perspectives, boards). The crate should be fully generic, and each domain crate should contribute its own builtin command YAMLs; the app composes them at startup ("stacking").

The layering infrastructure already exists — `CommandsRegistry::from_yaml_sources(&sources)` takes an arbitrary slice, and `merge_yaml_sources` layers more on top. What's missing is: (a) domain crates don't yet expose their own `builtin_yaml_sources()` fn, and (b) the app caller at `kanban-app/src/state.rs` only loads `swissarmyhammer_commands::builtin_yaml_sources()`, so kanban-specific YAMLs have no other home.

### Scope of this task

Move the six clearly kanban-specific YAML files and establish the crate-local stacking convention so `swissarmyhammer-commands` becomes fully generic.

#### YAMLs to move from `swissarmyhammer-commands/builtin/commands/` → `swissarmyhammer-kanban/builtin/commands/`

- `task.yaml` (3 cmds: `task.move`, `task.untag`, `task.doThisNext`)
- `column.yaml` (1 cmd: `column.reorder`)
- `tag.yaml` (1 cmd: `tag.update`)
- `attachment.yaml` (2 cmds: `attachment.open`, `attachment.reveal`)
- `perspective.yaml` (15 perspective commands)
- `file.yaml` (4 cmds: `file.switchBoard`, `file.closeBoard`, `file.newBoard`, `file.openBoard` — "board" is a kanban concept)

#### YAMLs that STAY in `swissarmyhammer-commands/builtin/commands/` (generic)

- `app.yaml` (quit/undo/redo/about/help/command/palette/search/dismiss)
- `settings.yaml` (keymap.vim/cua/emacs)
- `entity.yaml` (entity.* cross-cutting CRUD — "entity" is a generic concept owned by `swissarmyhammer-entity`)
- `ui.yaml` (generic UI mechanics — inspector, palette, setFocus, mode.set, window.new)
- `drag.yaml` (generic drag mechanics — drag.start/cancel/complete)

#### Known follow-up (OUT of scope for this task — file as separate task)

`ui.yaml` still contains `ui.view.set` and `ui.perspective.set`, which implicitly know about kanban's View/Perspective concepts. Splitting those out and possibly moving them into `swissarmyhammer-views` / `swissarmyhammer-perspectives` is a separate cleanup that depends on this task establishing the stacking convention.

### Files to change

- [x] **`swissarmyhammer-kanban/builtin/commands/`** — new directory; move the six YAML files listed above into it (content unchanged).
- [x] **`swissarmyhammer-kanban/src/lib.rs`** — add `pub fn builtin_yaml_sources() -> Vec<(&'static str, &'static str)>` that mirrors the implementation in `swissarmyhammer-commands/src/registry.rs` (`include_dir!("$CARGO_MANIFEST_DIR/builtin/commands")` + the same flat-layout filter). Re-use the same doc-comment language describing the flat-layout invariant. `include_dir` is already a workspace dep in `swissarmyhammer-kanban/Cargo.toml`, no new dependencies required.
- [x] **`kanban-app/src/state.rs`** — at the two sites that call `swissarmyhammer_commands::builtin_yaml_sources()` (`with_ui_state_path` around line 454, and `reload_command_overrides` around line 706), also collect `swissarmyhammer_kanban::builtin_yaml_sources()` and pass both slices concatenated (commands crate first, then kanban, then user overrides) so later sources override earlier by the existing partial-merge semantics.
- [x] **`swissarmyhammer-commands/src/registry.rs`** — update the large `builtin_yaml_files_parse` test (count of 60 commands, spot-checks on `task.untag`/`file.closeBoard`/etc.) and delete the kanban-specific tests (`perspective_commands_all_registered`, `test_perspective_yaml_parses`) that now belong to the kanban crate. Revise `ui_yaml_arg_only_commands_are_hidden_from_palette` only if its ID list is affected (it should not be — `ui.yaml` stays).
- [x] **`swissarmyhammer-kanban/tests/` (new or extended)** — add a test mirroring the deleted `perspective_commands_all_registered` / `test_perspective_yaml_parses` but reading from `swissarmyhammer_kanban::builtin_yaml_sources()`. Ensures the 26 moved commands still parse, are registered when the app composes both sources, and retain their scope/keys/visible/undoable fields.
- [x] **`swissarmyhammer-kanban/src/commands/mod.rs`** — `all_yaml_ids()` (around line 1217) currently enumerates only `swissarmyhammer_commands::builtin_yaml_sources()`. Extend it to also pull `crate::builtin_yaml_sources()` so `test_all_yaml_commands_have_rust_implementations` still sees every YAML-declared command.

### Subtasks

- [x] Add `swissarmyhammer-kanban/builtin/commands/` directory (empty) and `swissarmyhammer_kanban::builtin_yaml_sources()` exposed from `lib.rs`. Add one unit test in the kanban crate asserting the function returns an empty vec when the dir has no YAML.
- [x] `git mv` the six kanban-specific YAML files from `swissarmyhammer-commands/builtin/commands/` to `swissarmyhammer-kanban/builtin/commands/`. Content must be byte-identical.
- [x] Update `kanban-app/src/state.rs` callers to chain `swissarmyhammer_commands::builtin_yaml_sources()` then `swissarmyhammer_kanban::builtin_yaml_sources()` before merging user overrides. Confirm the chaining order matches the existing "later overrides earlier" contract.
- [x] Update the commands-crate tests that still hard-code counts/IDs for the moved YAMLs; move equivalent assertions into `swissarmyhammer-kanban/tests/`.
- [x] Update `swissarmyhammer-kanban/src/commands/mod.rs::all_yaml_ids()` to read both source sets; verify `test_all_yaml_commands_have_rust_implementations` still passes.

## Acceptance Criteria

- [x] `swissarmyhammer-commands/builtin/commands/` contains only `app.yaml`, `settings.yaml`, `entity.yaml`, `ui.yaml`, `drag.yaml` — the five generic YAML files.
- [x] `swissarmyhammer-kanban/builtin/commands/` contains `task.yaml`, `column.yaml`, `tag.yaml`, `attachment.yaml`, `perspective.yaml`, `file.yaml`.
- [x] `swissarmyhammer_kanban::builtin_yaml_sources()` is a public function returning the embedded YAML sources.
- [x] `kanban-app`'s composed registry resolves every command id previously available (no IDs disappear from `CommandsRegistry::get` or `all_commands`).
- [x] `swissarmyhammer-commands`'s tests pass without referencing any of the moved commands (task.*, column.*, tag.*, attachment.*, perspective.*, file.*).
- [x] `swissarmyhammer-kanban`'s tests verify the kanban YAMLs parse and register all expected IDs.
- [x] `cargo test -p swissarmyhammer-commands`, `cargo test -p swissarmyhammer-kanban`, and `cargo test -p kanban-app` all pass.
- [x] `cargo check -p swissarmyhammer-commands --features ""` (or whatever the minimal feature set is) does not reference any kanban types — proves the crate is now fully generic.

## Tests

- [x] Add `swissarmyhammer-kanban/tests/builtin_commands.rs` (or extend an existing test file in that crate) with a test equivalent to the deleted `perspective_commands_all_registered` — builds a registry from `swissarmyhammer_kanban::builtin_yaml_sources()` and asserts the 15 perspective IDs + the 11 other kanban-specific IDs are present. Include an assertion that the total kanban-builtin count is 26.
- [x] Add to the same file a test that composes both builtin sources (`commands_builtins` + `kanban_builtins`) and asserts `all_commands().len() == 60` (same as today's `builtin_yaml_files_parse` count) — proves no command is lost in the move.
- [x] Update `swissarmyhammer-commands/src/registry.rs::builtin_yaml_files_parse` to assert the NEW post-move count (roughly 60 − 26 = 34 commands) with a comment listing the expected composition by file (app: 9, settings: 3, entity: 8, ui: 11, drag: 3).
- [x] Delete `swissarmyhammer-commands/src/registry.rs::test_perspective_yaml_parses` and `perspective_commands_all_registered` (their assertions move to the kanban crate).
- [x] Command to run: `cargo test -p swissarmyhammer-commands -p swissarmyhammer-kanban -p kanban-app` — every test passes.
- [x] Command to run: `cargo build -p swissarmyhammer-commands` — verifies the crate still compiles standalone with only generic YAMLs.

## Workflow

- Use `/tdd` — write the kanban-crate builtin test first (fails because `swissarmyhammer_kanban::builtin_yaml_sources()` doesn't exist), then add the function + `include_dir!` reference. Do the YAML `git mv` next and update the commands-crate test to the reduced count. Finally wire the chain in `kanban-app/src/state.rs` and verify the composed registry resolves every id.
- Preserve file history: use `git mv` on each YAML so the commit shows the files as renames, not deletes + creates.
- Every YAML move is content-identical — do not edit the YAML bodies as part of this task. #commands #organization #refactor

## Review Findings (2026-04-23 15:29)

Overall the task is well-executed. All six YAMLs show as pure renames with 0-line changes in the staged diff (`git diff --cached --stat` confirms 0 insertions / 0 deletions). The `swissarmyhammer_kanban::builtin_yaml_sources()` function is a clean mirror of the commands-crate implementation. Stacking order in `state.rs` is correct at both sites (commands → kanban → user overrides), and `CommandsRegistry::merge_yaml_value` provides the partial-merge-by-id semantics the task relies on. `cargo check -p swissarmyhammer-commands` is clean; 174 commands-crate tests and 1261 kanban-crate tests pass.

### Warnings
- [x] `swissarmyhammer-kanban/tests/{command_dispatch_integration,command_snapshots,command_surface_matrix,perspective_context_menu_integration,undo_cross_cutting}.rs` + `swissarmyhammer-kanban/src/scope_commands.rs:1020` — the `composed_builtin_yaml_sources()` test helper is copy-pasted into **six** files, byte-identical. Rust integration tests can't share helpers directly, but the standard pattern is a `tests/common/mod.rs` that each integration test `mod common;` into — or expose the helper as `pub fn` on the kanban crate behind `#[cfg(any(test, feature = "test-util"))]`. Pick one and drop the five duplicates. The `scope_commands.rs` in-tree copy should just use the same helper too.

### Nits
- [x] `swissarmyhammer-commands/src/types.rs:134,188,218,253` and `swissarmyhammer-commands/src/registry.rs:262,274,324` — the post-move "consumer-agnostic" crate still uses kanban-flavored identifiers (`task.add`, `task.untag`, `file.newBoard`, `column:todo`, `tag:bug`) as inline YAML test fixtures. These are synthetic test-only strings, not references to real commands, so they don't break the stacking contract — but if the crate's lib.rs docs claim it "knows nothing about kanban, tasks, or specific entity types," a grep-based smell test will still catch these. Prefer generic placeholders (`foo.bar`, `widget:42`) in inline test YAML.
- [x] `swissarmyhammer-commands/builtin/commands/entity.yaml` header comment around line 26 — the bulleted list of kanban-specific files (`task.yaml, column.yaml, attachment.yaml, perspective.yaml`, plus the separate `file.yaml` block) reads as bare filenames but lives *inside* the commands-crate's `entity.yaml`. A reader skimming the comment might still expect those files in the same directory. Small copy-edit: qualify each bullet with the crate it lives in (e.g. `kanban: task.yaml`) to match the preceding "Generic (in ...)" / "Kanban-specific (in ...)" section headers.

## Review Follow-up (2026-04-23)

All three review findings addressed:

1. **Helper extraction**: Added `test-support` feature to `swissarmyhammer-kanban/Cargo.toml` and introduced `swissarmyhammer_kanban::test_support::composed_builtin_yaml_sources()` behind `#[cfg(any(test, feature = "test-support"))]` (matches the existing pattern in `swissarmyhammer-entity`). Deleted six byte-identical copies from the five integration tests and `scope_commands.rs`.

2. **Generic test fixtures**: Renamed kanban-flavored identifiers in `swissarmyhammer-commands/src/types.rs` and `swissarmyhammer-commands/src/registry.rs` to generic placeholders (`foo.add`, `foo.remove`, `widget:42`, `gadget:99`, `entity:widget`). The `builtin_yaml_files_parse` negative assertions kept their real kanban IDs because that test is explicitly verifying those IDs are NOT present.

3. **entity.yaml comment clarity**: Rewrote the header comment to qualify each file with its owning crate (`commands:app.yaml`, `kanban:task.yaml`, etc.) and added an explicit note that the kanban files are NOT siblings of `entity.yaml`.

Verification: `cargo nextest run --workspace` → 13301 passed. `npm test -- --run` → 122 files, 1322 tests passed.