---
position_column: done
position_ordinal: ffffffffffc780
title: 'STATUSLINE-1: Crate skeleton + input types'
---
## What
Create the `swissarmyhammer-statusline` crate with Cargo.toml, add to workspace members, and implement `src/input.rs` with serde structs for the Claude Code statusline JSON contract.

Key files:
- `swissarmyhammer-statusline/Cargo.toml` (new)
- `swissarmyhammer-statusline/src/lib.rs` (new)
- `swissarmyhammer-statusline/src/input.rs` (new)
- `Cargo.toml` (workspace members list)

The input struct must handle all fields from Claude's JSON stdin: model, workspace, cwd, cost, context_window, session_id, version, vim, agent, worktree. All fields `Option<T>` since Claude may not send everything on every update.

Dependencies: serde, serde_json, serde_yaml, tracing, git2, swissarmyhammer-directory, swissarmyhammer-kanban, swissarmyhammer-code-context.

## Acceptance Criteria
- [ ] `swissarmyhammer-statusline/Cargo.toml` exists with correct deps
- [ ] Crate listed in workspace `Cargo.toml` members
- [ ] `StatuslineInput` deserializes all known Claude Code JSON fields
- [ ] `cargo check -p swissarmyhammer-statusline` passes

## Tests
- [ ] Unit test: deserialize full JSON blob with all fields
- [ ] Unit test: deserialize minimal JSON blob (empty object)
- [ ] Unit test: deserialize JSON with only some fields present
- [ ] `cargo test -p swissarmyhammer-statusline`