---
assignees:
- claude-code
position_column: todo
position_ordinal: d380
project: command-cutover
title: Delete orphaned board.yaml YAML command def + make the no-YAML guard check the filesystem
---
## Context / Bug
The Stage-4 command cut-over claims it "deleted every embedded `builtin/commands/*.yaml`" and moved all command metadata into the builtin command plugins dispatched through `CommandService`. But one YAML command-definition file was never actually removed from disk:

- `crates/swissarmyhammer-kanban/builtin/commands/board.yaml` (defines `update.board`).

It is **dead trash**:
- `crates/swissarmyhammer-kanban/src/commands_core/registry.rs::builtin_yaml_sources()` returns `Vec::new()` — the kanban crate does **not** `include_dir!`/`include_str!` this directory. Verified: no `include_dir!`/`include_str!` anywhere points at `builtin/commands/` or `board.yaml`.
- The file's own header says it exists "to keep the YAML ↔ Rust completeness guard green (`test_all_yaml_commands_have_rust_implementations` / `test_no_orphan_rust_commands_without_yaml`)" — but that guard was **deleted** in the cut-over (see the note at `crates/swissarmyhammer-kanban/src/commands/mod.rs:1316-1320`). So its stated reason to exist is gone.
- `update.board` is now provided in Rust via `register_commands()` (`UpdateBoardCmd`, `crates/swissarmyhammer-kanban/src/commands/mod.rs:101`, impl in `board_commands.rs`). Deleting the YAML changes no behavior.

**Why it slipped through:** the regression guard `test_no_builtin_yaml_command_sources_remain` (`crates/swissarmyhammer-kanban/src/commands/mod.rs:1321`) only asserts that the two `builtin_yaml_sources()` *functions* return empty. It never looks at the filesystem, so a stranded `builtin/commands/*.yaml` file survives undetected.

> Scope note: the sibling live file `crates/swissarmyhammer-focus/builtin/commands/nav.yaml` is **still actively embedded** (`swissarmyhammer_focus::builtin_yaml_sources()` returns the 9 `nav.*` commands, composed at `apps/kanban-app/src/state.rs:918`). Migrating that off YAML is owned by card `01KTCQFH7AEQDZD0QETSMCMGP0` ("plugins/command-service exist to REPLACE ... `nav.yaml`"). This task must NOT touch the focus crate or break that still-live path — keep the guard hardening scoped to the kanban crate only.

## What
- [ ] Delete `crates/swissarmyhammer-kanban/builtin/commands/board.yaml` and remove the now-empty `crates/swissarmyhammer-kanban/builtin/commands/` directory.
- [ ] Harden the existing guard `test_no_builtin_yaml_command_sources_remain` in `crates/swissarmyhammer-kanban/src/commands/mod.rs` (or add an adjacent `#[test]`) so it ALSO fails when a `*.yaml` file physically exists under `concat!(env!("CARGO_MANIFEST_DIR"), "/builtin/commands")` — i.e. assert that directory is absent or contains no `.yaml` files. This closes the filesystem gap that let `board.yaml` survive.
- [ ] Keep the change kanban-crate-scoped: do not alter `swissarmyhammer-focus`, `nav.yaml`, or any `compose_registry!` call site.

## Acceptance Criteria
- [ ] `crates/swissarmyhammer-kanban/builtin/commands/board.yaml` no longer exists; the `builtin/commands/` dir under that crate is gone.
- [ ] `cargo build -p swissarmyhammer-kanban` and the kanban app build succeed (proves nothing embedded the file).
- [ ] The hardened guard test FAILS if a `.yaml` file is re-added under the kanban crate's `builtin/commands/`, and PASSES on the cleaned tree.
- [ ] `update.board` still dispatches: existing `UpdateBoardCmd` registration/tests remain green.

## Tests
- [ ] Update/extend the guard in `crates/swissarmyhammer-kanban/src/commands/mod.rs` (the `test_no_builtin_yaml_command_sources_remain` module). Add a filesystem assertion over `CARGO_MANIFEST_DIR/builtin/commands`. Confirm RED first by temporarily leaving `board.yaml` in place (test must fail), then GREEN after deletion.
- [ ] `cargo test -p swissarmyhammer-kanban no_builtin_yaml` → passes after deletion.
- [ ] `cargo test -p swissarmyhammer-kanban update_board` (and any `board_commands` tests) → still green, proving `update.board` behavior is unaffected.
- [ ] `cargo build -p kanban-app` → succeeds.

## Workflow
- Use `/tdd` — write the failing filesystem guard first (RED with `board.yaml` present), then delete the file to go GREEN.