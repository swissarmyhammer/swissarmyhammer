---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8e80
title: Remove the `[N/M] <step>` numbered headers from `sah init` output
---
## What

The narrated-step work (card `01KSJ4871F1MEYXR4RE1Z3HJBD`) added `InitEvent::Step { number, total, name }` events that render as `[N/M] <step name>` headers before each applicable component's output. In practice this is noise — components run effectively independent / async-ish, idempotent steps that do nothing (e.g. Permissions / Statusline / Preamble / Lockfile on a re-init) print a header with no following Action line, which is worse than silent.

Revert the visible step narration:

- Delete `InitEvent::Step` from `crates/swissarmyhammer-common/src/reporter.rs` (the variant, its emit-styled / emit-plain rendering, and the tests that exercise it).
- Stop emitting the event in `InitRegistry::run_lifecycle` in `crates/swissarmyhammer-common/src/lifecycle.rs` — go back to running applicable components in priority order without a per-component header. Keep `is_applicable` filtering exactly as it is.
- Update / remove tests that assert on Step events: the lifecycle tests in `lifecycle.rs` and `test_run_all_init_emits_step_per_applicable_component_for_project_scope` in `apps/swissarmyhammer-cli/src/commands/registry.rs`.
- **Keep** the unrelated good parts of the earlier card: the re-spaced priorities (10/11/20/30/40/50/55/60/70/80), the per-component `display_name()` overrides, the `KanbanTool` display name, and the updated priority banner comments. Those are harmless infrastructure — only the user-visible numbered headers are unwanted.

The kept `Action` / `Skipped` / `Warning` lines are exactly what the user asked to keep ("the un-numbered lines").

## Acceptance Criteria
- [x] `sah init` / `sah init user` no longer prints `[N/M] <name>` headers; only `Action` (`✓ Installed …`), `Warning`, and `Error` lines surface.
- [x] `InitEvent::Step` variant is removed; no remaining call sites.
- [x] `display_name()`, the re-spaced priorities, and the updated banner comments stay (no churn there).
- [x] `cargo build` and `cargo clippy -p swissarmyhammer-common -p swissarmyhammer-cli -p swissarmyhammer-tools --all-targets -- -D warnings` clean.

## Tests
- [x] Delete or refactor the now-irrelevant Step tests; do NOT replace them with weaker assertions.
- [x] Keep coverage of `is_applicable` filtering through whatever existing tests already exercised it.
- [x] `cargo test -p swissarmyhammer-common -p swissarmyhammer-cli` green.

## Workflow
- Straight refactor; no `/tdd` needed since this is removal, not new behavior. #init-doctor

## Review Findings (2026-05-26 15:45)

### Nits
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:50` — Stale priority in the `register_all` doc comment: "`SkillDeployment` (priority 30)" should be "`SkillDeployment` (priority 60)". The actual priority is 60 (see `apps/swissarmyhammer-cli/src/commands/skill.rs:61-63` and the canonical priority table in `apps/swissarmyhammer-cli/src/commands/registry.rs:34`). This is the only banner/priority comment that did not get re-spaced.