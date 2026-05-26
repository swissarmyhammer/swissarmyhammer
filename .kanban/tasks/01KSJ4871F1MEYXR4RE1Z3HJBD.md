---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
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
- [ ] `sah init` and `sah init user` print one `[N/M] <step name>` header per applicable component, in priority order, and the count `M` matches the applicable-component count for that scope.
- [ ] Each component has a human-readable `display_name`; internal `name()` slugs remain unchanged so log/test selectors that grep them still work.
- [ ] `cargo test -p swissarmyhammer-cli` and `cargo test -p swissarmyhammer-common` green.

## Tests
- [ ] Add a `CliReporter` test (or a `NullReporter` capturing variant) that runs `InitRegistry::run_all_init` with two stub components — one applicable, one not — under each scope and asserts the emitted `Step` events have monotonically increasing `number` from 1, `total` equals the applicable count, and the inapplicable component emits no Step.
- [ ] Update any test that asserts on reporter event sequence to include the new Step variant.

## Workflow
- Use `/tdd` — write the registry-step-event test first. #init-doctor