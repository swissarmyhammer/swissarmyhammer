---
assignees:
- claude-code
position_column: todo
position_ordinal: bc80
title: 'shell tool: name the Bash literal and build the operation list from SHELL_OPERATIONS'
---
Three naming/DRY items in `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`. Split out of ^1t92gnj, where the review engine surfaced them after that card touched this file with a single `use` line.

All three predate ^1t92gnj by two months — confirmed by `git blame`:

| Site | Commit | Date |
|---|---|---|
| `deny_tool(*scope, "Bash", reporter)` | `40834ce585` | 2026-05-29 |
| `allow_tool(*scope, "Bash", reporter)` | `40834ce585` | 2026-05-29 |
| `ToolCategory::Replacement { native: "Bash" }` | `4321207aea` | 2026-06-03 |
| `"Unknown operation '{}'. Valid operations: ..."` | `a7c1f58ed5` | 2026-05-29 |

## Items

1. The literal `"Bash"` is hardcoded in `deny_tool` / `allow_tool`. Introduce `const NATIVE_BASH_TOOL: &str = "Bash";`.
2. The literal `"Bash"` is hardcoded again in `ToolCategory::Replacement { native: "Bash" }` — same cause as item 1, second site. Use the same constant.
3. The operation names are hardcoded in the `Unknown operation` error text. Build that text from `SHELL_OPERATIONS` so the message cannot drift from the real roster.

Items 1 and 3 are one cause with two sites; fix once and apply at both.

## Warning on line numbers

The review engine cited lines 327, 359 and 374, but those hold doc comments about the subject, not the code. The real sites are lines 400, 445, 525 and 566. Do not trust the cited numbers — grep for the literals.

## Acceptance

- No bare `"Bash"` string literal remains in `shell/mod.rs` outside the constant's definition.
- The `Unknown operation` message is generated from `SHELL_OPERATIONS`; a test proves that adding an operation to the roster changes the message without editing the message.
- `cargo nextest run -p swissarmyhammer-tools`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` all clean. #refactor