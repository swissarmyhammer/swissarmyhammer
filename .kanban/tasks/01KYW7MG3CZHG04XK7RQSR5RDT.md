---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kywek7d4ew3myfv0eawdwxhp
  text: |-
    Picked up by /finish ^qsr5rdt. No prior attempts.

    State at pickup, HEAD aecbd1216:
    - All 8 `profiles:` keys still present — code-context, explore, finish, implement, issue, kanban, plan, review.
    - `task` already lost its key in the user's commit b6cfd88cb, which is what surfaced this whole thread: two kanban-app deploy tests went red because they hardcode `task` in the expected kanban-profile roster.
    - Four `builtin/_partials/*.md` deletions are UNCOMMITTED in the working tree (coding-standards, short-ids, skills, validators) — the user's parallel dedup work. Leave them alone; do not stage them, do not restore them.

    Design decision came from the user directly: "everything deploys everywhere". `Selector::All` for every consumer, curation removed. Kanban.app and kanban-cli will ship all 23 skills including /ci, /coverage, /make-readme, /deduplicate. That is the accepted outcome, not an oversight — do not reintroduce filtering under another name.

    Worth knowing before starting: deleting `Skill::profiles` also deletes `SAH_INTERNAL_FRONTMATTER_KEYS`, which I added two hours ago on ^t7ebyn8 for the sole purpose of keeping `profiles` out of deployed SKILL.md files. With `profiles` gone there is no internal key left, so the constant, its filter in `format_skill_md`, the `extra.retain` in the loader, and the three tests that loop over it all go. Verify no other key is genuinely internal before deleting rather than assuming — the loader/deploy set difference was exactly `{profiles}` when I checked.

    The lossless round-trip for the 12 unmodeled Claude Code keys (hooks, model, paths, etc.) must keep working after that removal. That is the point of ^t7ebyn8 and the reason the ralph Stop hook now survives deployment.
  timestamp: 2026-07-31T16:02:25.828863+00:00
- actor: claude-code
  id: 01kywj2vy200b5zdsya6s24ptc
  text: |-
    Implementation landed. Worked from HEAD aecbd1216.

    ## TDD order

    Flipped the two kanban-app deploy tests FIRST, with production still on `Selector::Profile("kanban")`, and watched them go RED for the right reason:
    - `workspace_init::opening_a_board_deploys_the_kanban_tool_skills` panicked on "builtin skill `map` must be deployed".
    - `state::tests::test_open_board_deploys_kanban_tool_skills_at_board_folder` panicked on "builtin skill `detected-projects` must be deployed".

    Both now GREEN.

    ## Call sites → Selector::All

    - `apps/kanban-app/src/state.rs::kanban_profile`
    - `apps/kanban-cli/src/commands/registry.rs::profile` (plus the `KANBAN_PROFILE` const, deleted)
    - `apps/code-context-cli/src/commands/registry.rs::skills_selector` (plus the `SKILL_NAMES` const, deleted) — reused unchanged by `commands/skill.rs`
    - `apps/kanban-app/tests/workspace_init.rs::kanban_profile` (test-local mirror)
    - The two reconstructed profiles in `mirdan::install::profile_consistency_tests` (`kanban_profile`, `code_context_profile`)

    ## Mechanism deleted

    - `Selector::Profile` variant + its arm in `Selector::select`
    - `KNOWN_PROFILES` + the `debug_assert!` that validated builtins against it
    - `resolve_profile_skills` (it had ZERO callers — dead since it was written)
    - `Skill::profiles`, plus the `profiles` field on both `SkillFrontmatter` structs (loader + deploy)
    - `SAH_INTERNAL_FRONTMATTER_KEYS`, its `format_skill_md` filter, the loader's `extra.retain`, and the three tests that looped over it
    - Two obsolete loader tests: `test_parse_skill_md_parses_profiles_list`, `test_parse_skill_md_profiles_default_empty_when_absent`

    `Selector::select` also changed shape: it took a `HashMap<String, Vec<String>>` name → profile-tags map purely so `Profile` could match tags. Six call sites built that map, four of them filling `Vec::new()` placeholders with "carries no profile tags" comments. It now takes `&HashSet<String>` of names. Leaving the tag map would have been dead plumbing and a profile-shaped vestige.

    ## No other key was internal

    Verified before deleting the constant: the loader's `SkillFrontmatter` names `name, description, license, compatibility, context, agent, metadata, allowed-tools, profiles`; the deploy `SkillFrontmatter` names the same set minus `profiles`. The difference was exactly `{profiles}`, so the internal set is now empty and the constant is gone. **No other key turned out internal.**

    ## ^t7ebyn8 not regressed

    `deploy.rs::test_format_skill_md_round_trips_unmodeled_frontmatter_keys` (the lossless round-trip over `hooks`/`model`/`paths`/`disable-model-invocation`) is untouched and passes, as is `test_format_skill_md_writes_each_modeled_key_once` (I only dropped the now-unmodeled `profiles:` from its fixture). `mirdan::install::init_profile_preserves_unmodeled_skill_frontmatter` got STRONGER: with no internal keys there is nothing to skip, so every source key must now equal the deployed value except Liquid-rendered `metadata`.

    Verified end to end in a sandbox (`HOME` + `XDG_CONFIG_HOME` redirected to a temp dir, real `~/.skills` mtime confirmed older than the run): the deployed `finish/SKILL.md` still carries its `hooks: Stop: command: sah tool ralph ralph check --` block, and no deployed SKILL.md carries a `profiles:` key.

    ## 6 → 23 proof

    Measured the before-count mechanically at HEAD: 6 builtins carried `- kanban` (finish, implement, issue, kanban, plan, review) and 2 carried `- code-context` (code-context, explore). After: sandboxed `kanban init project` logs "Deployed 23 skill(s) to claude-code" and `<root>/.skills` holds all 23. Sandboxed `sah init project` also lands 23.

    ## Interesting discovery — orphan file

    `crates/mirdan/src/plugin.rs` is NOT compiled. `crates/mirdan/src/lib.rs` never declares `mod plugin`, nothing references `mirdan::plugin`, and there is no `#[path]` attribute or extra bin target pulling it in. It carried a `Selector::Profile("kanban")` plugin spec and its own tag-map plumbing. I updated it to `Selector::All` + a name set so no stale `Selector::Profile` text survives anywhere, but the file is dead code that the compiler never checks — so it silently rots. Filed separately; it wants either wiring up or deleting.

    ## What did not work / notes

    - `cargo nextest run -p kanban-app` appears to hang: `kanban-app::ai_panel_e2e test_ai_panel_e2e_mcp_tool_reachable_in_session` is a REAL-MODEL qwen-0.6b test and ran past 9 minutes. 158/160 kanban-app tests pass, including both target tests; the two stragglers are the ai_panel_e2e pair. Not caused by this card — exclude `binary(ai_panel_e2e)` and the `llama-agent`/`claude-agent` packages when running the suite.
    - Added `swissarmyhammer-skills` to `apps/kanban-app` `[dev-dependencies]`. The reassertion reads the real `SkillResolver::new().resolve_builtins()` roster instead of transcribing 23 names, so the test cannot drift as builtins are added or removed. kanban-app had no skills dependency before, only `mirdan`, which does not re-export the resolver.
  timestamp: 2026-07-31T17:03:24.098100+00:00
position_column: doing
position_ordinal: '8280'
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