---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffba80
project: kanban-mcp
title: 'kanban doctor: fix false-negative "Board Initialized" check'
---
## What

`KanbanDoctor::check_board_initialized` in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-cli/src/commands/doctor.rs` reports `Board Initialized: No .kanban/board.yaml found` even when the board is fully initialized. The check hard-codes a single layout (`<cwd>/.kanban/board.yaml`), but the canonical board file today lives at `<cwd>/.kanban/boards/board.yaml` — so `kanban doctor` emits a false warning inside this very repo.

The canonical "is this board initialized?" logic already exists in `swissarmyhammer-kanban`:

```rust
// swissarmyhammer-kanban/src/context.rs:260
pub fn is_initialized(&self) -> bool {
    self.root.join("boards").join("board.yaml").exists()
        || self.board_path().exists()          // <root>/board.yaml (legacy)
        || self.root.join("board.json").exists() // very old legacy
}
```

The fix: delegate to `KanbanContext::new(<cwd>/.kanban).is_initialized()` instead of the inlined single-path check. `swissarmyhammer-kanban` is already a direct dependency of `kanban-cli` (used by `commands/serve.rs`), and `KanbanContext::new` is a synchronous, zero-I/O constructor — safe to call from the doctor path.

While fixing, also update the warning message and the `fix:` suggestion so they don't mis-attribute the expected location: say "No kanban board found in .kanban/" and suggest `kanban init board` (the actual operation name — `kanban board init` is not the CLI path that creates a board; the schema-driven command is `kanban board init` under the noun-verb scheme — verify with `kanban --help` before writing the exact fix string).

## Files

- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-cli/src/commands/doctor.rs` — swap the `board_path.is_file()` branch for `KanbanContext::new(cwd.join(".kanban")).is_initialized()`; update the success-path message to report `<root>` rather than a specific filename; update the warning/fix text to match.

## Acceptance Criteria

- [x] Running `kanban doctor` from the root of this repo reports `Board Initialized: Ok` (not a warning), because `.kanban/boards/board.yaml` exists.
- [x] Running `kanban doctor` from a directory with no `.kanban/` still reports a warning.
- [x] The success message references the `.kanban/` root or the detected layout, not a hard-coded `.kanban/board.yaml` path.
- [x] The fix suggestion in the warning path matches the actual CLI verb that creates a board (verify via `kanban --help`).
- [x] `cargo test -p kanban-cli` passes.
- [x] `cargo clippy -p kanban-cli --tests -- -D warnings` clean.

## Tests

- [x] Update `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-cli/src/commands/doctor.rs` — add a new `#[serial]` test using `CurrentDirGuard` (per `feedback_test_isolation.md`) that:
  1. Creates a tempdir, `cd`s into it, writes `.kanban/boards/board.yaml` with minimal valid content.
  2. Calls `check_board_initialized()` on a fresh `KanbanDoctor`.
  3. Asserts exactly one check named "Board Initialized" with status `CheckStatus::Ok`.
- [x] Add a companion `#[serial]` test that creates an empty tempdir with no `.kanban/` and asserts `CheckStatus::Warning`.
- [x] Keep the existing `check_board_initialized_produces_one_check` test (shape-only assertion remains useful).
- [x] `cargo test -p kanban-cli --lib commands::doctor` — all pass.

## Workflow

- Use `/tdd` — write the new failing `#[serial]` tests first (they should fail against the current implementation because the success test will see `.kanban/boards/board.yaml` but the code only checks `.kanban/board.yaml`), then swap the check to delegate to `KanbanContext::is_initialized()`.

## Implementation Notes

- The `kanban-cli` crate is binary-only (`[[bin]]` only, no `[lib]`), so the test command in the Tests acceptance criterion was run as `cargo test -p kanban-cli --bin kanban commands::doctor` instead of `--lib`. All 10 doctor tests pass (8 original + 2 new `#[serial]` regression tests).
- Verified via `cargo run -p kanban-cli -- doctor` from repo root that `Board Initialized` now reports `Ok` with message `Found at .../.kanban`.
- `swissarmyhammer_common::test_utils::CurrentDirGuard` and `serial_test::serial` were already available — no new dependencies added.