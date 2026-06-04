---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe580
title: 'Consolidate duplicate skill-deployment path: workspace-init bypasses mirdan'
---
## What

There are **two** skill-deployment implementations. mirdan is supposed to be THE
skill/agent deployment system, but `swissarmyhammer-workspace-init` reimplements
its own. They share the frontmatter serializer (`deploy::format_skill_md`) but
nothing else — the copy/store/link mechanics are forked. This is the duplicated-
but-different code that let the `context`/`agent` frontmatter bug hide (a third
serializer also existed in the CLI and was removed; see history).

(Original problem statement and approach retained below the resolution.)

## RESOLUTION — Closed as superseded (2026-06-04)

Superseded in full by the **mirdan-install** project (cards 01KT7A30… through 01KT7A69…),
which consolidated init/install in mirdan outright. Verified against the current tree
before closing:

- `swissarmyhammer-workspace-init` is **deleted entirely** (crate dir + workspace member +
  all deps) — card 01KT7A4YM77…. Stronger than this card's "delegate to mirdan": there is
  no second deployer left to delegate. grep for the crate name returns only `.kanban/` text.
- mirdan exposes **one** shared deploy path parameterized by target: `init_profile` /
  `deinit_profile` with an optional explicit `root` (threads through `stage_and_deploy_rendered`
  with no CWD access) — cards 01KT7A3G6K… (installer) and 01KT7A30… (edge inversion).
- `sah init` / `sah init user` still deploy via central store + symlinks with frontmatter
  intact — card 01KT7A3Z4F… (sah Profile migration), covered by real-pipeline tests.
- Kanban desktop app deploys self-contained + idempotently through mirdan's explicit-root
  installer — card 01KT7A4YM77…. **Design delta:** the board reads/writes `<root>/.skills/`
  (the store location `SkillResolver` actually overlays), NOT the `<root>/.sah/skills/` this
  card assumed. The old `.sah/skills/` copy was an unread, divergent mechanism; card 5
  resolved the deploy-semantics question to the single `.skills/` store. The AC wording here
  ("`.sah/skills/`") is therefore intentionally not met — it was the wrong target.
- Single serializer / single copier: `swissarmyhammer-skills::deploy::format_skill_md` and
  `mirdan::install::copy_dir_recursive` each have exactly one owner. `is_safe_skill_name` /
  `deploy_builtin_skills` / `write_skill` (prod) / `run_workspace_init` are all gone. Safe-name
  validation unified on `mirdan::store::is_safe_name` — card 01KT7A5SYG…
- The original `context`/`agent` frontmatter regression is guarded at the serializer level by
  `swissarmyhammer-skills::deploy::test_format_skill_md_round_trips_context_and_agent`
  (asserts `context: fork` / `agent: explorer` survive the round-trip).

Not added (consciously declined per close-as-superseded): a full `HOME=$tmp sah init user`
end-to-end `.skills/explore/SKILL.md` frontmatter guard, and an explicit cross-path
byte-identical anti-drift test. There is now only one deploy code path (root is a parameter),
so the "two paths must match" anti-drift concern is structurally moot; the consistency tests in
`mirdan::install::profile_consistency_tests` (card 01KT7A69…) assert store+symlink-not-copy and
clean round-trip across all four CLI profiles. If the e2e explore-skill guard is wanted later,
file a fresh small test-only card rather than reopening this one.

---

(Original approach, retained for history:)

### Approach
Make mirdan the single skill-deployment implementation, parameterized by target,
and delete workspace-init's bespoke copy/write logic. [Done via mirdan-install; see resolution.]

## Acceptance Criteria
- [x] `swissarmyhammer-workspace-init` contains no skill copy/serialize/safe-path logic of its own — crate deleted entirely.
- [x] mirdan exposes one shared code path for both agent-symlink (CWD/HOME) and explicit-root deploy.
- [x] `sah init user` / `sah init` still deploy to detected agents (central store + symlinks), frontmatter preserved.
- [x] Kanban desktop app produces a self-contained, idempotent builtin-skill deploy (to `.skills/`, the resolver's store — superseding the `.sah/skills/` assumption).
- [x] No second implementation of skill copying/serialization remains (format_skill_md / copy_dir_recursive single-owner).

## Tests
- [x] Explicit-root deploy unit test (mirdan `init_profile_explicit_root_targets_given_root` + kanban-app `workspace_init.rs`).
- [x] workspace-init component tests — N/A, crate deleted; replaced by CLI `create_workspace_structure` tests + kanban-app tests.
- [~] Anti-drift cross-path byte-identical test — structurally moot (single code path); consistency covered by `profile_consistency_tests`. Declined.
- [x] `cargo test -p mirdan -p swissarmyhammer-cli` passes (workspace-init crate no longer exists).
- [~] E2E `sah init user` → `.skills/explore/SKILL.md` with `context: fork`/`agent: explorer` — declined; serializer-level round-trip test covers the regression.

#tech-debt #mirdan #init