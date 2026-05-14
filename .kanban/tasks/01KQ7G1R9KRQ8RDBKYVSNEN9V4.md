---
assignees:
- wballard
depends_on:
- 01KQ35MHFJQPMEKQ08PZKBKFY0
position_column: done
position_ordinal: fffffffffffffffffffffffe80
title: Verify validator MCP server registers only the validator tool subset (no shell, no edit, no kanban, no leaks)
---
After `01KQ35MHFJQPMEKQ08PZKBKFY0` lands, audit that the in-process validator MCP server exposes **exactly** the read-only validator tool set and nothing else. Validators are pure judgment processes — they read code and decide. Anything beyond that is a footgun: a model that can call `bash` to run `cargo test` from inside a validator is not a validator anymore, and a model that can `write_file` can corrupt the user's working tree as a "side effect" of a code-quality check.

This is a **verification card**, parallel to `01KQ7FZHVJ9XQK8ZTAJ23DMM9E` but focused on tool *content* rather than path *structure*.

## What "validator tool subset" means

Concretely, exactly these:

- **`FilesTool::read_only()`** (`tools/files/mod.rs:90`) — exposes `read file`, `glob files`, `grep files`. Rejects `write file` and `edit file` at the `execute` boundary.
- **All `code_context` tools** (`tools/code_context/mod.rs`) — the ones that already return `is_validator_tool() = true`. Cross-check what that returns at runtime.

Nothing else. Specifically excluded:
- `FilesTool::new()` (the all-operations variant — has write/edit).
- `register_shell_tools` (entire `tools/shell` module).
- `register_git_tools` (entire `tools/git` module).
- `register_kanban_tools`.
- `register_web_tools`.
- `register_questions_tools`.
- `register_ralph_tools`.
- `register_skill_tools`.
- `register_agent_tools` (the agent bundle — file editing, shell, grep, skills).
- Anything else not explicitly in the two-item list above.

## What to verify

### 1. Static audit — the registration helper is exhaustive

Locate `build_validator_tool_registry` (or whatever the tools task named it) in `swissarmyhammer-tools/src/mcp/`. Its body must consist of exactly:

```rust
let mut registry = ToolRegistry::new();
registry.register(FilesTool::read_only());
register_code_context_tools(&mut registry);
registry
```

No additional `register(...)` calls, no `register_*_tools` for any other category. The function should be small and obviously correct by inspection.

### 2. Static audit — no agent-bundle helpers reach validator construction

```bash
grep -rE 'agent_mode\s*[:=]\s*true|register_agent_tools|register_all_tools|create_fully_registered_tool_registry' \
  swissarmyhammer-tools/src/mcp/ avp-common avp-cli
```

After cleanup, none of these should appear inside the validator MCP construction path. They're fine in the *full* server path (`start_mcp_server_with_options`), but the validator path must never call them.

### 3. Runtime audit — list the MCP server's tools and assert the set

A unit/integration test in `avp-common` (or `swissarmyhammer-tools`):

1. Call `start_validator_mcp_server(...)` to get an `McpServerHandle`.
2. Open an MCP client connection to `handle.url()`.
3. Send `tools/list`.
4. Assert the returned tool names are exactly `{"files"} ∪ {<all code_context tool names>}` — no more, no less.

The test fails fast if anyone adds a `register_kanban_tools` call that "seemed harmless" — the runtime list won't match.

### 4. Runtime audit — `files` tool exposes only `read file`, `glob files`, `grep files`

Same test, drill into the `files` tool's schema (or call it with each operation):

- `read file` → succeeds (with a real file path).
- `glob files` → succeeds.
- `grep files` → succeeds.
- `write file` → returns `McpError::invalid_params` matching the message in `tools/files/mod.rs:148-153`.
- `edit file` → same rejection.

Lock in the read-only enforcement at the boundary, not just at registration time. The `FilesTool::read_only()` constructor already does this; the test confirms it didn't regress.

### 5. Trait-level audit — every registered tool returns `is_validator_tool() = true`

Defense in depth. After registration, iterate the registry and assert that for every tool, `tool.is_validator_tool() == true`. If any tool registers itself as validator-mode but returns `false` from the trait method, that's a registration bug. If any non-validator tool sneaks into the registry, this test catches it even if grep #2 missed.

### 6. Doc/test fixture audit — rule prompts don't advertise tools we don't supply

The rule prompts in `builtin/validators/**/*.md` advertise an "Available Tools" section listing `files` (read-only) and `code_context`. Confirm these match what the registry actually serves — no rule prompt mentions `bash`, `git`, `kanban`, etc.

```bash
grep -rE 'bash|shell|git|kanban|edit_file|write_file|file edit|file write' \
  builtin/validators/ \
  | grep -v -F '#'    # exclude comment lines if they mention these
```

If a rule prompt advertises a tool the validator can't actually call, the model wastes turns asking for it. Either the prompt is wrong (more likely) or the tool registry is wrong (less likely after the tools task lands).

### 7. Recording-fixture audit — past recordings don't show forbidden tool calls

Once `01KQ7FWFR4V364AYF29DGGBZ87` lands and recordings start accumulating in `.avp/recordings/`, scan a sample for any tool call other than the validator subset:

```bash
jq -r '.calls[] | .. | objects | select(has("name")) | .name' .avp/recordings/*.json \
  | grep -vE '^(files|get_symbol|search_symbol|list_symbols|grep_code|get_callgraph|get_blastradius|get_status|...)$'
```

(Adjust the allowlist to whatever code_context's `is_validator_tool() = true` set turns out to be.) Any output is a finding — a model successfully called a tool the validator shouldn't have offered.

## What "done" looks like

- All four greps and three runtime tests pass.
- A walk through `build_validator_tool_registry` shows exactly two function calls (one for the read-only files tool, one for code_context).
- The MCP server's `tools/list` response matches the expected set byte-for-byte (modulo ordering).
- `cargo test -p swissarmyhammer-tools -p avp-common` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.

## How to handle findings

If a forbidden tool turns up — even one — it's a real correctness issue. File as a `## Review Findings` checklist item on this card, fix on this card (it's a one-line registration delete in 99% of cases), and confirm the runtime test re-passes before closing.

## Depends on

`01KQ35MHFJQPMEKQ08PZKBKFY0` — until that lands there's no `start_validator_mcp_server` to audit. Hard dependency.

## Pairs with

`01KQ7FZHVJ9XQK8ZTAJ23DMM9E` (no fallback paths). One verifies the path topology (always-on, no branches), the other verifies the path content (right tools, no leaks). Different invariants, complementary audits.

## Why this is its own task

Easy to gloss over during implementation review: "tools task added the registry, looks fine." Without an explicit allowlist test, the next person who adds `register_git_tools(&mut registry)` because "validators might want to look at git history" will sail through review. The runtime allowlist test is the durable guard. #avp