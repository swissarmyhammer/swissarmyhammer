---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffec80
project: local-review
title: 'Blanket Doctorable: every tool reports OK by default; sah doctor enumerates all tools'
---
## What
Make every tool `Doctorable` with a blanket "OK" so each one shows up in `sah doctor` even when it has no special checks — instead of today's `impl_empty_doctorable!` which returns `Vec::new()` (the tool is invisible in the report). `McpTool: Doctorable` is already a supertrait (`crates/swissarmyhammer-tools/src/mcp/tool_registry.rs`), so this is about the default behavior, not adding the bound.

1. **Default `run_health_checks` → one OK check.** Give `Doctorable::run_health_checks` (in `crates/swissarmyhammer-common/src/health.rs`) a default body returning `vec![HealthCheck::ok(self.name(), format!("{} available", self.name()), self.category())]`. `name()`/`category()` are already required trait methods, so the default can use them. Tools with real diagnostics override `run_health_checks` (as web/prompts already do).
2. **Retire the empty macro.** Replace `impl_empty_doctorable!` (which overrode `run_health_checks` to return empty) with `impl_default_doctorable!` that only wires `name()` (→ `McpTool::name`) and `category()` (→ "tools") and inherits the OK default. Update all call sites.
3. **`sah doctor` enumerates ALL tools.** `collect_all_health_checks()` (`crates/swissarmyhammer-tools/src/health_registry.rs`) currently registers a hand-picked subset (file/git/shell/kanban/questions/web/skill) and omits others (code_context, agent, etc.). Register every tool group so every tool surfaces at least an OK line. Keep `is_applicable()` honored for genuinely optional/platform-specific tools.

## Acceptance Criteria
- [x] `Doctorable::run_health_checks` has a default that returns a single OK `HealthCheck` built from `name()`/`category()`.
- [x] No tool returns an empty health-check vec just because it has no special checks; `impl_empty_doctorable!` is gone (replaced by the default-inheriting macro). (RalphTool's explicit empty `Vec::new()` override also removed — it now inherits the OK default.)
- [x] `collect_all_health_checks()` registers all tool groups (added code_context, ralph, agent to match the server's `register_all_tools`); `sah doctor` lists every registered tool (each at least OK).
- [x] Tools with real checks (web, prompts) still override and report their specific results unchanged.

## Tests
- [x] Trait test: a minimal `Doctorable` impl that only defines `name()`/`category()` yields exactly one OK check via the default. (`test_default_run_health_checks_yields_single_ok`)
- [x] `collect_all_health_checks()` test: the returned set contains a check for every registered tool group (asserts code_context (LSP), ralph (default OK "Ralph"), and agent (Agent library) — all previously missing — now appear). (`test_all_tool_groups_enumerated`)
- [x] `cargo test -p swissarmyhammer-common health` and `cargo test -p swissarmyhammer-tools health_registry` green.

## Workflow
- Use `/tdd` — write the default-OK trait test and the "all tools enumerated" registry test first. This is a small cross-cutting infra change; keep it data-driven (one default, not per-tool boilerplate). Independent of the engine work — can land early.