---
assignees:
- claude-code
position_column: todo
position_ordinal: ca80
title: 'Remove profile-based skill selection: every consumer deploys all builtin skills'
---
Delete the init-profile mechanism for skills. Every consumer deploys all 23 builtin skills via `Selector::All`. Curation by profile goes away entirely — this is a deliberate product decision, not a simplification of an existing behavior.

## Today

| Consumer | Selector | Skills |
|---|---|---|
| `sah` | `All` | 23 |
| `kanban-cli`, `Kanban.app` | `Profile("kanban")` | 6 — finish, implement, issue, kanban, plan, review |
| `code-context-cli` | `Profile("code-context")` | 2 — code-context, explore |
| `shelltool-cli` | `Single("shell")` | 1 |

After: `Kanban.app` and `kanban-cli` ship `/ci`, `/coverage`, `/make-readme`, `/deduplicate` and the rest. That is the accepted outcome.

## Required change

1. **Call sites → `Selector::All`**
   - `apps/kanban-app/src/state.rs:1156`
   - `apps/kanban-cli/src/commands/registry.rs` — three sites, around lines 40, 84, 97
   - `apps/code-context-cli/src/commands/registry.rs::skills_selector` (and its reuse in `commands/skill.rs:30`)

2. **Delete the mechanism**
   - `Selector::Profile` variant and its arm in `Selector::select` (`crates/mirdan/src/install.rs:886`)
   - `KNOWN_PROFILES` (`install.rs:1048`) and the `debug_assert!` validating against it (~1105-1119)
   - `resolve_profile_skills` (`crates/swissarmyhammer-skills/src/deploy.rs`)
   - `Skill::profiles` field and the `profiles` field on both `SkillFrontmatter` structs (loader + deploy)

3. **`SAH_INTERNAL_FRONTMATTER_KEYS` becomes empty — delete it too.** It exists solely to keep `profiles` out of deployed SKILL.md. With `profiles` gone there is no internal key left, so the constant, its filter in `format_skill_md`, the `extra.retain` in the loader, and the three tests that loop over it all go. Check for a genuinely-internal key before deleting; if one turns up, keep the constant and say which.

4. **Strip `profiles:` from the 8 builtin SKILL.md files** that still declare it: code-context, explore, finish, implement, issue, kanban, plan, review. (`task` already lost its key.)

5. **Fix the two kanban-app deploy tests** that hardcode the kanban-profile roster — `workspace_init::opening_a_board_deploys_the_kanban_tool_skills` and `state::tests::test_open_board_deploys_kanban_tool_skills_at_board_folder`. They currently fail because `task` lost its `profiles:` key. Reassert against the full builtin set.

## Coordination

The user is editing `builtin/skills/**` in parallel to reduce duplication. Item 4 touches only the `profiles:` frontmatter key in 8 files — coordinate before editing, and re-read each file immediately before changing it.

## Acceptance

- No `Selector::Profile`, `KNOWN_PROFILES`, `resolve_profile_skills`, or `Skill::profiles` anywhere.
- No `profiles:` key in any `builtin/skills/*/SKILL.md`.
- A sandboxed `sah init project` (redirected HOME, never the real `~/.skills`) deploys all 23 skills.
- The same for a kanban-cli init — prove the count went 6 → 23.
- `cargo nextest run --workspace`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Related: ^t7ebyn8 added the frontmatter round-trip that `SAH_INTERNAL_FRONTMATTER_KEYS` guards; deleting the constant must not break the lossless round-trip for the 12 unmodeled Claude Code keys. #refactor #init #bug