---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6c22wyj8s2vn1z95vj7410e
  text: 'Trimmed dead parts after the removal of the files tool, shell tool and GUI app: removed the shell/grep_history site (directory gone). Moved the code_context sites to lsp_ops.rs and the extract_bool_param helper in support.rs. Fixed the internal installed read path to status.rs.'
  timestamp: 2026-11-01T17:21:31.602872+00:00
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
The same cause is still in place at sibling sites that read the caller's
arguments with a bare `as_bool()`:

- `crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs` —
  `args.get("dependents").and_then(|v| v.as_bool())`. Today
  `{"op": "check working", "dependents": "false"}` folds the dependents in
  anyway and answers as if it obeyed.
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/lsp_ops.rs` —
  `include_source` (twice: `get definition`, `get type_definition`) and
  `include_declaration` (`get references`). All three read through
  `extract_bool_param` in `code_context/support.rs`, which holds the bare
  `as_bool()` read.

`code_context/status.rs` (`l["installed"].as_bool()`) reads an internal
JSON structure, not a caller argument. Leave it alone.

# Changes

- Replace each caller-argument `as_bool()` read above with `bool_arg` (for the code_context sites, change `extract_bool_param`). The
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