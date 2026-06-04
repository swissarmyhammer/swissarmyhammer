---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: 'Consolidate duplicate skill-deployment path: workspace-init bypasses mirdan'
---
## What

There are **two** skill-deployment implementations. mirdan is supposed to be THE
skill/agent deployment system, but `swissarmyhammer-workspace-init` reimplements
its own. They share the frontmatter serializer (`deploy::format_skill_md`) but
nothing else — the copy/store/link mechanics are forked. This is the duplicated-
but-different code that let the `context`/`agent` frontmatter bug hide (a third
serializer also existed in the CLI and was removed; see history).

The two paths today:

1. **mirdan (canonical)** — `mirdan::install::deploy_skill_to_agents`
   (`crates/mirdan/src/install.rs:612`). Copies the skill into a **central store**
   (`store::skill_store_dir(global)` — `~/…/.skills` or CWD-relative `.skills`),
   then **symlinks** it into each detected agent's skill dir. Requires
   `agents::load_agents_config()` + agent detection. Rooted at CWD/HOME.
   Used by `sah init` via `apps/swissarmyhammer-cli/src/commands/skill.rs`
   (`SkillDeployment`, priority 60) → `write_skill_contents` → `deploy_skill_to_agents`.

2. **workspace-init (the duplicate to remove)** —
   `crates/swissarmyhammer-workspace-init/src/components.rs`
   (`SkillDeployment`, `deploy_builtin_skills`, `write_skill`). Writes builtin
   skills **directly** to `<root>/.sah/skills/<name>/SKILL.md` from an **explicit
   caller-supplied root**. No central store, no symlinks, no agent detection.
   Registered in `crates/swissarmyhammer-workspace-init/src/registry.rs:26`; the
   only production caller is the kanban desktop app
   (`apps/kanban-app/src/state.rs:1058` → `run_workspace_init`).

Why it was forked: mirdan is rooted at CWD/HOME and needs detected coding-agents;
the kanban app is a long-lived in-process agent that needs an **explicit root**
and writes a self-contained `.sah/skills/` it reads directly (no external agent
to symlink into). That requirement is real — but it is a *parameter*, not a
reason for a second implementation.

### Approach

Make mirdan the single skill-deployment implementation, parameterized by target,
and delete workspace-init's bespoke copy/write logic.

- In `crates/mirdan/src/install.rs`, add an explicit-root / self-contained deploy
  target so a caller can request "write skill `<name>` into `<root>/.sah/skills/`"
  without agent detection or symlinks. Options: a new `pub fn` (e.g.
  `deploy_skill_to_dir(name, source_dir, dest_skills_dir)`) that
  `deploy_skill_to_agents` and the workspace path both build on, OR a target enum
  threaded through one shared internal fn. Pick whichever removes the most
  duplication (the store-copy + safe-name + resource-subdir logic should live
  once).
- Rewrite `swissarmyhammer-workspace-init`'s `SkillDeployment::init` to call the
  new mirdan entry point instead of its private `deploy_builtin_skills`/`write_skill`.
  Delete `deploy_builtin_skills`, `write_skill`, `is_safe_skill_name`,
  `is_safe_relative_path` from `components.rs` once mirdan owns them (mirdan
  already has `sanitize_dir_name` / safe-path helpers — reuse, don't re-add).
- Keep `ProjectStructure` delegation as-is (already shared).
- Confirm `sah init` behavior is byte-identical before/after for the agent-symlink
  path; only the explicit-root path is new.

Sizing note: this likely lands as 2 tasks — (a) add the explicit-root target to
mirdan + reuse helpers, (b) switch workspace-init + kanban-app onto it and delete
the duplicate. Split with `depends_on` if it exceeds ~500 LOC / 5 files. Consider
`/plan` if research shows the kanban-app caller needs broader changes.

## Acceptance Criteria
- [ ] `crates/swissarmyhammer-workspace-init/src/components.rs` contains no
      skill copy/serialize/safe-path logic of its own — it delegates entirely to
      `mirdan::install`.
- [ ] mirdan exposes one shared code path that handles both the agent-symlink
      deploy (CWD/HOME-rooted) and the explicit-root `.sah/skills/` deploy.
- [ ] `sah init user` and `sah init` (project) still deploy skills to detected
      agents exactly as before (central store + symlinks), with `context`/`agent`
      and all frontmatter preserved.
- [ ] The kanban desktop app (`apps/kanban-app`) still produces a self-contained
      `<root>/.sah/skills/<name>/SKILL.md` for every builtin skill, idempotently.
- [ ] No second implementation of skill copying/serialization remains anywhere
      (grep for `format_skill_md`, `write_skill`, `copy_dir_recursive` shows a
      single owning module each).

## Tests
- [ ] Unit test in `crates/mirdan/src/install.rs` for the new explicit-root
      deploy: deploys a temp skill into `<root>/.sah/skills/` and asserts the
      `SKILL.md` exists with frontmatter intact, no symlinks, no agent detection
      required.
- [ ] Update `crates/swissarmyhammer-workspace-init/src/components.rs` tests
      (`test_skill_deployment_writes_builtin_skills_under_explicit_root`,
      `test_skill_deployment_is_idempotent`) to assert behavior is preserved
      through the mirdan delegation.
- [ ] Anti-drift regression test: deploy the same builtin skill through both the
      agent path and the explicit-root path and assert the written `SKILL.md`
      bytes are identical (prevents the serializers/copiers re-forking).
- [ ] `cargo test -p mirdan -p swissarmyhammer-workspace-init -p swissarmyhammer-cli` passes.
- [ ] End-to-end: `HOME=$tmp sah init user` deploys `.skills/explore/SKILL.md`
      containing `context: fork` / `agent: explorer` (regression guard for the
      original bug).

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#tech-debt #mirdan #init