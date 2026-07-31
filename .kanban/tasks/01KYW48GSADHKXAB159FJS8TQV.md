---
assignees:
- claude-code
position_column: todo
position_ordinal: c880
title: 'tool_registry.rs: prefix-match table, duplicate macros, manual register_file_tools'
---
Four shape cleanups in `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs`. Split out of ^6xjxebg — all pre-existing and outside that commit's hunks.

## Items

1. **The `cli_category` prefix match (~line 161) should be a table.** The default `cli_category()` derives a category by matching the tool-name prefix against a hardcoded list of arms. This is the mechanism that silently hid `agent` and `skill` from the CLI for as long as they existed: a tool registers fine, then has no `sah tool <name>` command, with no error anywhere.

   ^6xjxebg worked around it per-tool by overriding `cli_category()` on the two affected tools. The root fix is to stop deriving a category from a name prefix at all, or to drive it from one table that a test can assert covers every registered tool.

   This is the highest-value item on this card — the others are style.

2. **Two duplicate macros.** Identical bodies; collapse to one.

3. **`register_file_tools` is written out by hand** where its siblings use the macro. Use the macro.

4. **`&Arc<...>` parameters** where `&T` would do.

## Acceptance

- Adding a tool whose name prefix has no arm cannot silently produce a tool with no CLI command — either a test catches it, or the derivation is gone.
- One macro, not two.
- `register_file_tools` matches its siblings.
- `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Interacts with ^6xjxebg's parity test, which now checks CLI *visibility* and not just registration — that test is the guard for item 1 and must keep passing. Also related: ^pwaxzy2 (three copies of the registration list, only one guarded).