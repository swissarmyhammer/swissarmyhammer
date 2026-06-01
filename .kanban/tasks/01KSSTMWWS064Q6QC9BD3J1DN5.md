---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb880
title: 'kanban-cli init/deinit: adopt mirdan per-agent strategy like shelltool/sah'
---
## What
`kanban init/deinit` still uses the pre-redesign pattern that `shelltool`/`sah` moved off of (commit 40834ce58). Bring it in line: the **tool** (`KanbanTool`) owns its MCP registration via an injected entry and delegates to `mirdan::install` per-agent strategies; the CLI injects the entry. This fixes kanban's Local-scope bug and removes the duplicated per-agent loop.

### Current state (the problem)
- `apps/kanban-cli/src/commands/registry.rs::KanbanMcpRegistration` is a bespoke component with its own `resolve_agent_targets` loop and `global = matches!(scope, InitScope::User)` — so `kanban init local` collapses to Project behavior (writes committed `.mcp.json`, never `~/.claude.json` `projects.<key>`), the same bug shelltool/sah had. It does NOT call `mirdan::install`.
- `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs::KanbanTool` implements `Initializable` but only registers/unregisters git **merge drivers** for `.kanban/` (its own non-agent config). It does not do MCP registration. `is_applicable` = Project|Local. `KanbanTool` is a **unit struct** (`pub struct KanbanTool;`).

### Reference (what shelltool/sah now do)
- `ShellExecuteTool` carries `mcp_server: Option<(String, McpServerEntry)>` + `with_mcp_server(name, entry)`; `is_applicable` = User|Local|Project; `init` calls `mirdan::install::register_mcp_server(scope, name, entry, reporter)` when `Some`, and gates its own-config step (`.shell/config.yaml`) to Project|Local; `deinit` mirrors with `unregister_mcp_server`. shelltool-cli `register_all` injects the entry; `mirdan::install` (crates/mirdan/src/install.rs:1692+) iterates detected agents → `strategy_for(agent)` → correct per-scope target.

## Changes
### 1. `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`
- Change `KanbanTool` from a unit struct to carry `mcp_server: Option<(String, mirdan::mcp_config::McpServerEntry)>`. Keep `KanbanTool::new()` (→ `None`) and add `with_mcp_server(name, entry)`. Update ALL construction sites (McpTool registration in the tool registry, `KanbanTool::new()` callers, sah's `register_all`, kanban-cli, tests) — grep `KanbanTool` and fix each; sah must construct it WITHOUT an entry (sah exposes kanban via `sah serve`, not a separate `kanban` server).
- `is_applicable` → User|Local|Project.
- `init(scope)`: if `Some((name, entry))` → extend results with `mirdan::install::register_mcp_server(*scope, name, entry, reporter)`; THEN run the existing merge-driver registration ONLY for Project|Local (skip User). Aggregate.
- `deinit(scope)`: if `Some` → `mirdan::install::unregister_mcp_server(*scope, name, reporter)`; merge-driver unregister gated to Project|Local.
- Kanban is NOT a shell — do NOT call `deny_tool`/`allow_tool`.

### 2. `apps/kanban-cli/src/commands/registry.rs`
- `register_all`: register `KanbanTool::new().with_mcp_server("kanban", McpServerEntry{command:"kanban".into(), args:vec!["serve".into()], env:BTreeMap::new()})` + `KanbanSkillDeployment`.
- DELETE `KanbanMcpRegistration`, `resolve_agent_targets`, `AgentMcpTarget`, and `MCP_SERVER_NAME` (now redundant). Update tests (registry.len stays 2; replace the init/deinit single-result tests with ones that drive `KanbanTool` or assert `register_all` composition — keep the IsolatedTestEnvironment + CurrentDirGuard + isolation).
- Note: kanban-cli init now also sets up merge drivers (via KanbanTool) — that's a correct, intended consequence of the tool owning its lifecycle.

## Acceptance Criteria
- [ ] `kanban init local` registers `kanban` under `~/.claude.json` `projects.<cwd>.mcpServers` (NOT a committed `.mcp.json`); `deinit local` removes it and prunes empty `mcpServers`
- [ ] `kanban init project` writes `.mcp.json`; `kanban init user` writes global `~/.claude.json` `mcpServers.kanban`; each reverts on deinit
- [ ] kanban MCP registration goes through `mirdan::install::{register_mcp_server, unregister_mcp_server}` (no bespoke per-agent loop remains in kanban-cli)
- [ ] sah behavior unchanged: it still registers `KanbanTool` WITHOUT a kanban MCP entry (no `kanban` server appears from `sah init`); merge-driver setup preserved
- [ ] `KanbanTool` construction sites all updated; builds clean
- [ ] `cargo clippy -p swissarmyhammer-tools -p kanban-cli -p swissarmyhammer-cli --all-targets` clean, zero warnings

## Tests
- [ ] kanban-cli registry tests updated (IsolatedTestEnvironment + CurrentDirGuard + `cargo nextest run -p kanban-cli` passes
- [ ] KanbanTool lifecycle test: with an injected entry, init/deinit at User/Local/Project hit the right MCP target via mirdan (synthetic agent through MIRDAN_AGENTS_CONFIG, mirroring the shell-tool lifecycle tests); without an entry (sah path), init does merge-drivers only and no MCP write occurs
- [ ] `cargo nextest run -p swissarmyhammer-tools kanban` + `cargo nextest run -p swissarmyhammer-cli install` green (sah unaffected)

## Workflow
- Use `/tdd` for the new KanbanTool lifecycle test (write it red against the injected-entry behavior first), then refactor.
- Watch for the parallel in-flight shell-tool changes — this task touches `kanban/mod.rs` and `kanban-cli`, which do not overlap, but re-verify before staging.

## Review Findings (2026-05-29 13:18)

Verified: clippy clean (`swissarmyhammer-tools`/`kanban-cli`/`swissarmyhammer-cli`, zero warnings); `kanban-cli` 102/102 pass; `swissarmyhammer-tools kanban` 63/63 pass (4 lifecycle tests incl. user/local/project + no-mcp-entry); `swissarmyhammer-cli install` 52/52 pass. Delegation, scope gating (User|Local|Project for MCP; Project|Local for merge drivers), sah no-entry parity, and the deletion of `KanbanMcpRegistration`/`resolve_agent_targets`/`AgentMcpTarget`/`MCP_SERVER_NAME` from kanban-cli all confirmed. The `tool_registry.rs` `KanbanTool` hits are the `test_tool!`-generated unit-struct shadow (not the production `#[derive(Default)]` type), correctly unchanged. `applier_error` is an acceptable small parallel of the shell helper (not worth cross-crate sharing).

### Blockers
_None._

### Warnings
_None._

### Nits
- [ ] `apps/kanban-cli/src/commands/mod.rs:15` — Stale doc comment: the `registry` module doc still says it "Exposes `register_all` and `KanbanMcpRegistration`", but `KanbanMcpRegistration` was deleted in this change. Update to reference the `KanbanTool` lifecycle / `with_mcp_server` injection instead.
- [ ] `apps/kanban-cli/src/commands/skill.rs:92` — Stale doc comment on `KanbanSkillDeployment::priority`: "runs after `KanbanMcpRegistration` (priority 10)". The component no longer exists and the priority is wrong — `KanbanTool` now owns MCP registration at priority 55 (skill deployment is 20, so it runs *before* the tool, not after). Reword to avoid the dangling reference and the inverted ordering claim.