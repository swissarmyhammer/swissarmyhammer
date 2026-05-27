---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8c80
title: 'Init: narrate clear pipeline steps in the reporter (Step N/M …)'
---
## What

The install pipeline today runs nine `Initializable` components in priority order (`McpRegistration`, `ClaudeLocalScope`, `DenyBash`, `Statusline`, `ProjectStructure`, `ClaudeMd`, `SkillDeployment`, `AgentDeployment`, `LockfileCleanup`), but the user-facing reporter output is a flat stream of `Action` / `Skipped` / `Warning` lines with no narrative. The component `category()` strings (`configuration`/`structure`/`deployment`) are accurate but never surface to the user. Goal: when you run `sah init [user]`, you should see clearly enumerated steps that match a documented install pipeline.

Concrete changes:

- Add a new `InitEvent::Step { number, total, name }` variant in `swissarmyhammer_common::reporter` and render it in `CliReporter` as something like `[2/8] Permissions …`.
- In `InitRegistry::run_all_init` (and `run_all_deinit`) — `crates/swissarmyhammer-common/src/lifecycle.rs` — emit one `Step` event per **applicable** component (filtering by `is_applicable(scope)`), with `total` being the count of applicable components for that scope. The `name` is each component's `name()` (or a new `display_name()`).
- Rename component names where they read like internal slugs rather than steps:
  - `mcp-registration` → `Register MCP server`
  - `claude-local-scope` → `Register MCP (Claude local scope)`
  - `deny-bash` → `Permissions`
  - statusline component → `Statusline`
  - `project-structure` → `Project workspace`
  - `claude-md` → `Preamble`
  - `skill-deployment` → `Skills`
  - `agent-deployment` → `Subagents`
  - `lockfile-cleanup` → `Lockfile`
  Implement via an added `fn display_name(&self) -> &str` on `Initializable` with a default that returns `self.name()`, then override per component.
- Re-space priorities into clean 10s (10, 20, 30, …) so the order is obvious: keep MCP at 10, ClaudeLocalScope at 11 (sub-step of MCP), Permissions 20, Statusline 30, ProjectWorkspace 40, Preamble 50, Skills 60, Subagents 70, Lockfile 80.
- Update `commands::registry::register_all` docs accordingly. Verify the existing assertions in the registry tests (`registry.len() == 9` / `10`) still hold.

## Acceptance Criteria
- [x] `sah init` and `sah init user` print one `[N/M] <step name>` header per applicable component, in priority order, and the count `M` matches the applicable-component count for that scope.
- [x] Each component has a human-readable `display_name`; internal `name()` slugs remain unchanged so log/test selectors that grep them still work.
- [x] `cargo test -p swissarmyhammer-cli` and `cargo test -p swissarmyhammer-common` green.

## Tests
- [x] Add a `CliReporter` test (or a `NullReporter` capturing variant) that runs `InitRegistry::run_all_init` with two stub components — one applicable, one not — under each scope and asserts the emitted `Step` events have monotonically increasing `number` from 1, `total` equals the applicable count, and the inapplicable component emits no Step.
- [x] Update any test that asserts on reporter event sequence to include the new Step variant.

## Workflow
- Use `/tdd` — write the registry-step-event test first. #init-doctor

## Implementation Notes

- `InitEvent::Step { number, total, name }` added in `swissarmyhammer_common::reporter` with rendering in both `emit_plain` (`[N/M] name`) and `emit_styled` (bold cyan counter + bold name).
- `Initializable::display_name(&self) -> &str` default returns `self.name()`; overridden in the 9 install components with the human-readable strings from the card.
- `InitRegistry::run_lifecycle` extracted as a shared helper for `run_all_init` / `run_all_deinit`: counts applicable components once for `total`, then emits a `Step` per applicable component in execution order (forward for init, reverse for deinit). Inapplicable components produce a `Skipped` result without emitting a Step.
- Priorities re-spaced: McpRegistration=10, ClaudeLocalScope=11, DenyBash=20, Statusline=30, ProjectStructure=40, ClaudeMd=50, KanbanTool=55 (sits in the same relative position it held in the old ordering — between Preamble and SkillDeployment), SkillDeployment=60, AgentDeployment=70, LockfileCleanup=80.
- Doc table on `commands::registry::register_all` updated to reflect the new priorities and display names.
- Tests added: 5 new unit tests in `swissarmyhammer-common::lifecycle::tests` covering default `display_name`, override path, applicable-only Step emission for init and deinit, Step `total` counting, and silent Step-less path when nothing applies. Reporter tests cover Step serialization and the all-variant matrix. Integration test in `swissarmyhammer-cli::commands::registry::tests::test_run_all_init_emits_step_per_applicable_component_for_project_scope` asserts the real registry emits Steps numbered 1..N with stable `total` and that Local-only components are excluded under Project scope.
- Verified end-to-end with `sah init` (Project: `[1/9]`…`[9/9]`) and `sah init user` (User: `[1/7]`…`[7/7]`).

## Review Findings (2026-05-26 09:32)

### Nits
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:437` — Section header banner `// ── DenyBash (priority 15) ───` is stale; actual priority is 20 (line 467 and the priority-method docstring at line 465 are correct). Update banner to `(priority 20)` so the section landmarks match the code.
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:572` — Section header banner `// ── Statusline (priority 16) ───` is stale; actual priority is 30. Update banner to `(priority 30)`.
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:778` — Section header banner `// ── ProjectStructure (priority 20) ───` is stale; actual priority is 40. Update banner to `(priority 40)`.
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:951` — Section header banner `// ── AgentDeployment (priority 31) ───` is stale; actual priority is 70. Update banner to `(priority 70)`.
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:1184` — Section header banner `// ── LockfileCleanup (priority 32) ───` is stale; actual priority is 80. Update banner to `(priority 80)`.
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:1274` — Section header banner `// ── ClaudeMd (priority 22) ──` is stale; actual priority is 50. Update banner to `(priority 50)`.
- [x] `apps/swissarmyhammer-cli/src/commands/skill.rs:35` — Section header banner `// ── SkillDeployment (priority 30) ───` is stale; actual priority is 60 (line 62 and the docstring at line 60 are correct). Update banner to `(priority 60)`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:162` — `KanbanTool` does not override `display_name()`, so its `[N/M]` header renders as the MCP tool slug `kanban` while every other narrative step (`Register MCP server`, `Permissions`, `Statusline`, `Project workspace`, `Preamble`, `Skills`, `Subagents`, `Lockfile`) uses a human-readable label. For visual consistency in the pipeline narrative, add `fn display_name(&self) -> &str { "Kanban board" }` (or similar) so the `[7/9]` step reads cleanly alongside its siblings. Note: the doc table in `apps/swissarmyhammer-cli/src/commands/registry.rs:32` already references KanbanTool without parens, which is the only step in the table missing a display label.