---
assignees:
- claude-code
position_column: todo
position_ordinal: fa80
title: Route every MCP boolean arg through bool_arg so a string flag is never silently dropped
---
# Problem

`bool_arg` in `crates/swissarmyhammer-tools/src/mcp/op_tool_helpers.rs` now
coerces the JSON string `"true"`/`"false"` and errors on a value it cannot read.
An MCP caller is usually an agent, and agents routinely send the string where the
schema declares a boolean. A dropped flag returns a success-shaped response that
is missing the thing the caller asked for, with nothing to show why.

Card ^ztx209w fixed the one call site it needed (`review` / `list validators`).
The same cause is still in place at five sibling sites that read the caller's
arguments with a bare `as_bool()`:

- `crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs` —
  `args.get("dependents").and_then(|v| v.as_bool())`. Today
  `{"op": "check working", "dependents": "false"}` folds the dependents in
  anyway and answers as if it obeyed.
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` —
  `include_source` (twice: `get definition`, `get type_definition`) and
  `include_declaration` (`get references`).
- `crates/swissarmyhammer-tools/src/mcp/tools/shell/grep_history/mod.rs` — the
  boolean read at the top of the argument parse.

`code_context/mod.rs` line ~2687 (`l["installed"].as_bool()`) reads an internal
JSON structure, not a caller argument. Leave it alone.

# Changes

- Replace each caller-argument `as_bool()` read above with `bool_arg`. The
  helper is `pub(crate)` and every site is in the same crate.
- Map the `Err` to `rmcp::ErrorData::invalid_params`, the same way the `review`
  tool does.

# Tests

- One wire-layer test per tool proving the JSON string `"true"` returns the same
  result the boolean does, and that an unreadable value errors instead of
  quietly reading as the default. Follow
  `list_validators_honors_a_string_rules_flag_from_an_agent` in
  `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`.

# Acceptance

- `cargo nextest run -p swissarmyhammer-tools` passes.
- No caller-argument `as_bool()` read is left under
  `crates/swissarmyhammer-tools/src/mcp/tools/`.