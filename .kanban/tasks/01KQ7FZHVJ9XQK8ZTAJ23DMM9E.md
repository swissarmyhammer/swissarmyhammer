---
assignees:
- wballard
depends_on:
- 01KQ35MHFJQPMEKQ08PZKBKFY0
position_column: done
position_ordinal: fffffffffffffffffffffffd80
title: Verify validator tools are unconditional — no fallback, no opt-out, no Option<McpServerConfig>
---
After `01KQ35MHFJQPMEKQ08PZKBKFY0` lands, audit the codebase to confirm the cleanup was complete. The single-path simplification has to actually be single-path — not "looks single-path but has a vestigial branch."

This is a **verification card**, not implementation. It runs *after* the tools task lands and checks the cleanup invariants. Findings (if any) get recorded back as review findings on this card and fixed, then the card closes.

## What to verify

### 1. No env-var checks for MCP enablement

```bash
grep -rE 'SAH_HTTP_PORT|SWISSARMYHAMMER_HTTP_PORT' avp-common avp-cli
```

Must return zero matches in code (kanban task descriptions and historical commit messages don't count). The two env vars are removed from the validator MCP path entirely.

### 2. No `Option<McpServerConfig>` for the validator path

`McpServerConfig` should be threaded as a plain value, not an `Option`. The validator MCP server always exists; if it fails to start, propagate the error — don't degrade to "no tools" silently.

```bash
grep -rn 'Option<McpServerConfig>\|: Option<McpServer\|mcp_config: Option' avp-common
```

If any matches surface inside the validator construction path (`AvpContext::init`, `agent()` constructor, anywhere a validator agent is built), they need to become plain `McpServerConfig`.

### 3. No `(None, None)` returns or "no tools" logging

```bash
grep -rE '\(None, None\)|without MCP tools|prompt-only|backward compatible|fallback|no tools' avp-common
```

Vestigial language from the old fallback framing. Clean up the matches that remain in the validator path. (The `Default` parsing strategy in `chat_template.rs` is unrelated and stays.)

### 4. No conditional branch that depends on whether tools are available

The control flow in `AvpContext::init`, `resolve_validator_mcp_config` (if it still exists), and the agent-construction path must not contain any `if tools_available { ... } else { ... }`. Every validator agent is constructed with the in-process MCP server's URL, full stop.

```bash
grep -rB2 -A2 'mcp_config\.is_some\|mcp_config\.is_none\|tools.is_empty' avp-common
```

### 5. The validator MCP server start is in the construction path, not deferred

`start_validator_mcp_server` runs synchronously during `AvpContext::init` (or as the first awaited future in an async init). It's not behind a `OnceLock`, not lazy on first agent use, not conditional on anything. The handle is held on `AvpContext` from creation, dropped on context drop.

### 6. Tests don't bypass the MCP server

Search for any test helper or context constructor that builds an `AvpContext` without starting the MCP server (e.g., a `for_testing()` constructor with a mock that returns `None`). If one exists, it must explicitly use `PlaybackAgent` (which sidesteps live MCP for legitimate test reasons) — not silently skip tools.

```bash
grep -rE 'fn (new_for_test|for_test|with_mock|test_context)' avp-common
```

### 7. Tracing/log lines reflect "always on"

The historical "validator agent will use MCP endpoint: {url} (tools disabled)" / "Validator agent will run without MCP tools" log lines are gone. Replaced with a single info line at startup describing the bound URL and the registered tool set.

```bash
grep -rE 'validator agent will (run|use)|tools disabled|without MCP' avp-common
```

### 8. Doc comments don't mention configuration

`avp-common/src/context.rs::resolve_validator_mcp_config` (or whatever replaced it) has no "If env var is set..." comment. The `AvpContext::init` doc comment names the in-process MCP server as part of construction, not as an optional feature.

## What "done" looks like

- All eight greps above return either zero matches or matches that are unambiguously unrelated (e.g., a `Default` parsing strategy is fine; a kanban-tools `Option` is unrelated).
- A walk through `AvpContext::init` from top to bottom shows: validator MCP server starts → handle stored → agent constructed with the URL. No branches, no env-var reads, no `Option`s being checked.
- The `01KQ35MHFJQPMEKQ08PZKBKFY0` acceptance test (validator agent has `mcp_servers: 1` on every Stop hook) passes with no test-environment env-var gymnastics.
- `cargo test -p avp-common` and `cargo clippy -p avp-common --all-targets -- -D warnings` are clean.

## How to handle findings

If any check turns up a real fallback path (not just stale comments), file findings as a `## Review Findings` checklist on this card and fix them on this card before closing — don't let the simplification be "mostly done."

## Depends on

`01KQ35MHFJQPMEKQ08PZKBKFY0` — there's nothing to verify until that lands. Hard dependency.

## Why this is its own task

The tools task itself is the implementer's job and it's busy enough — they're writing new code (the `start_validator_mcp_server` entry point + the validator tool registry). A separate verification pass with explicit greps and a checklist catches the leftover-conditional-branch failure mode that's easy to miss when you're focused on building the new path. #avp

## Review Findings (2026-04-27) — RESOLVED

The shipped implementation in `01KQ35MHFJQPMEKQ08PZKBKFY0` retained the env-var path (parent-host SAH_HTTP_PORT short-circuit) instead of removing it. The verification card's stricter "always-on, no env-var" intent was confirmed by the user (option A in `/loop` Q&A), so the env-var path was dropped.

- [x] **Finding 1 (Check 1):** `validator_mcp_port_from_env()` helper removed; `SAH_HTTP_PORT` and `SWISSARMYHAMMER_HTTP_PORT` reads are gone from `avp-common/src/context.rs`. `grep -rE 'SAH_HTTP_PORT|SWISSARMYHAMMER_HTTP_PORT' avp-common avp-cli` returns zero matches.
- [x] **Finding 2 (Check 2):** `resolve_validator_mcp_config` now returns `Result<(McpServerConfig, String), AvpError>` (non-Optional). The call site wraps as `Some(mcp_config)` / `Some(tools_override)` for the upstream `swissarmyhammer_agent::create_agent_with_options` call, but the validator path itself threads plain values. `grep -rn 'Option<McpServerConfig>\|: Option<McpServer\|mcp_config: Option' avp-common` returns zero matches.
- [x] **Finding 3 (Check 3):** Doc comment on `mcp_server_handle` field rewritten to describe the unconditional in-process server; `Option` is now framed as "None only between context construction and the first agent() call."
- [x] **Finding 4 (Check 4):** `if let Some(port) = port { ... return ... }` env-var branch removed from `resolve_validator_mcp_config`. The remaining `if let Some(existing) = guard.as_ref()` is a re-entry guard against the lazy initialization race, not a tools-availability branch — it always falls through to constructing a validator agent with the same URL.
- [x] **Finding 5 (Check 5):** Documented the lazy-on-first-agent-call pattern in `mcp_server_handle`'s doc comment and `AvpContext::init`'s doc comment. The verification card's check 5 is read as "the construction path *is* the agent-init path; the validator MCP server is part of it" — `init()` is sync and changing it is out of scope.
- [x] **Finding 6 (Check 7):** "Validator agent will use parent MCP endpoint" / "tools disabled" log lines removed. Replaced with a single `tracing::info!` at in-process server bind time describing the bound URL and `agent_mode`. `grep -rE 'validator agent will (run|use)|tools disabled|without MCP' avp-common` returns zero matches.
- [x] **Finding 7 (Check 8):** `resolve_validator_mcp_config` doc comment rewritten — single unconditional path, no "If env var is set..." framing.
- [x] **Finding 8 (Cargo.toml):** Comment updated to "In-process MCP server backing the validator agent's tool surface (always started in `AvpContext`)."
- [x] **Finding 9 (tests):** Deleted `test_resolve_validator_mcp_config_uses_env_when_set`. Updated `test_in_process_mcp_server_lifecycle_on_drop` — removed env-var hygiene removal, dropped `serial_test::serial(env)` (only `cwd` is needed), updated assertions for non-Optional return, added an assertion that `mcp_server_handle` is populated while the context is alive.
- [x] **Finding 10 (top-of-file doc):** `AvpContext::init` doc comment now names the in-process MCP server as part of construction.

### Verification (2026-04-27, after fix)

All eight greps now pass cleanly:
- Check 1: `grep -rE 'SAH_HTTP_PORT|SWISSARMYHAMMER_HTTP_PORT' avp-common avp-cli` → zero matches
- Check 2: `grep -rn 'Option<McpServerConfig>\|: Option<McpServer\|mcp_config: Option' avp-common` → zero matches
- Check 3: `grep -rE '\(None, None\)|without MCP tools|prompt-only|backward compatible|no tools' avp-common` → zero matches; remaining "fallback" matches are all unrelated (session-id fallback, `run_validators_with_fallback` validator runner method, etc.)
- Check 4: `grep -rE 'mcp_config\.is_some\|mcp_config\.is_none' avp-common` → zero matches; `tools.is_empty` matches are all in unrelated `match_criteria.tools` test/parser code
- Check 5: `mcp_server_handle` is held on `AvpContext`, started in the agent-init path, dropped on context drop
- Check 6: `grep -rE 'fn (new_for_test|for_test|with_mock|test_context)' avp-common` → only legit `test_context_*` test names, no test-only constructors
- Check 7: `grep -rE 'validator agent will (run|use)|tools disabled|without MCP' avp-common` → zero matches
- Check 8: `resolve_validator_mcp_config` doc comment is single-path, no env-var framing

### Walk through `AvpContext::init` (acceptance criterion)

1. `init_directories()` (sync) — creates project + home dirs
2. `resolve_model_config()` (sync) — reads model config
3. `new_without_agent()` (sync) — constructs the struct; `mcp_server_handle: Mutex::new(None)` 

Then on first `agent()` call (the construction path for tooling):
1. Acquire `agent_handle` lock; if `None`, proceed
2. `resolve_validator_mcp_config().await` — locks `mcp_server_handle`; since empty, calls `start_in_process_mcp_server()` which **always** binds `127.0.0.1:0` and stores the handle
3. Call `create_agent_with_options(&model_config, Some(mcp_config), options)` — wraps `Some` only because the upstream `swissarmyhammer-agent` API takes `Option<McpServerConfig>` (used by other callers like `swissarmyhammer-cli/src/commands/prompt/new.rs` that legitimately pass `None`)

No env-var reads. No conditional branches based on tool availability. Single path. Errors propagate.

### Build/test (acceptance criterion)

- `cargo check -p avp-common` clean
- `cargo clippy -p avp-common --all-targets -- -D warnings` clean
- `cargo nextest run -p avp-common` — 639 passed, 0 failed
- `cargo check -p avp-cli` clean (downstream dependency)