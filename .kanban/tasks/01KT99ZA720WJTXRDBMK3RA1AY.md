---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe680
project: mirdan-install
title: Tool CLIs don't install skills at user scope; code-context omits explore
---
Two confirmed regressions from the Profile migration (card 01KT7A4D44637D9Z1THZX6DASP), reproduced in an isolated temp HOME/CWD against the built `shelltool`/`code-context` binaries.

## Bug 1 — `init user` installs NO skills (all three tool CLIs)
`shelltool init user` registers the MCP server and the Bash-deny but deploys zero skills; `~/.skills` and `~/.claude/skills` are empty. Cause: each CLI's `project_scoped_skills(scope)` returns `Some(selector)` for Project/Local but **`None` for User**. Added in card 4 "matching prior per-CLI behavior." At Project/Local scope skills DO deploy, so it looks inconsistent.

**Decision (user):** remove the gate — skills must deploy at User scope too (into the global store `~/.skills` + `~/.claude/skills`), making `init user` a full configuration. This brings the tool CLIs in line with sah's `Selector::All`.

### Fix
- Delete `project_scoped_skills`'s `InitScope::User => None` gate in all three tool CLIs; return the skill selector at every scope.
- Update each CLI's now-wrong unit test to assert skills ARE selected at User scope. Keep the local-scope test.

## Bug 2 — code-context CLI never ships the `explore` skill (any scope)
code-context's profile was `Named(["code-context","lsp"])` — `explore` absent.

**Decision (user):** code-context installs **code-context + explore + lsp + detected-projects**.

### Fix
- Change the code-context profile selector to `Named(["code-context","explore","lsp","detected-projects"])`. All four builtins exist under `builtin/skills/`.
- Update the code-context profile-shape unit test to assert the new four-skill set.

## Why the existing consistency test missed both
`mirdan::install::profile_consistency_tests` (a) only exercised Project/Local scope, never User, and (b) **reconstructed** each CLI's profile rather than importing the real `profile()` — so a missing skill is mirrored in the reconstruction and passes. The drift gap the card-7 reviewer flagged.

## Tests (TDD — write FIRST and watch fail)
Isolated, HOME+CWD-isolated production-path tests covering **both User and Project scope**:
1. User-scope deploy regression: skill lands as `~/.skills/<name>/SKILL.md` + `~/.claude/skills/<name>` symlink, MCP registered. Must FAIL before gate removal.
2. Project-scope deploy: same, rooted at explicit `<root>`, no CWD access.
3. code-context skill set: exactly `{code-context, explore, lsp, detected-projects}`. Must FAIL before selector change.
4. Close drift gap: drive the REAL `profile(scope)` as `apps/*/tests/`-style integration tests, not a reconstruction.

## Done when
- `init user` and `init project`/`local` for all three CLIs deploy their declared skills (store + symlink) and register MCP — verified by the new tests.
- code-context deploys exactly the four skills.
- New tests fail before the fix and pass after; they drive the REAL profile.
- `cargo build --workspace` green; clippy clean; full suite green.

#bug #mirdan #init #cli

## Review Findings (2026-06-04 08:38)

Both bugs correctly fixed and verified. Optional reporter mislabel also fixed (`install.rs:1082` emits `skill_count`). Drift gap closed at the app layer via real-`profile(scope)` integration tests.

### Warnings
- [x] `profile_consistency_tests` reconstructed `expected_skills` enumerations — RESOLVED: replaced with a single `probe_skill` exercising only the deploy mechanism; docs point skill-set authority at the per-CLI `registry.rs` tests.
- [x] `profile_consistency_tests` helper duplication — RESOLVED: deleted the stale in-crate `test_support` duplicate; module now reuses the public `crate::test_support::{write_single_agent_config, read_json, assert_no_init_error, MirdanConfigGuard}`.

## Review Findings (2026-06-04 09:18)

Re-review after both warnings resolved. Both bugs remain correctly fixed; all three CLI registries clean and well-documented; no dead code; reporter mislabel confirmed. One nit only.

### Nits
- [x] `crates/mirdan/src/test_support.rs:115` and `:156` — `UserScopeDeploy::assert` and `ProjectScopeDeploy::assert` are near-verbatim, differing only by base path / MCP location / message prefix. DECLINED (kept as two explicit structs): the reviewer rated this a 2-occurrence rule-of-three coincidence, not a pattern, and explicitly noted the explicit two-struct form is "defensibly clearer" than one parameterized helper. The two structs sit on a real user-global vs project-local axis (global `~/.skills` + global `.fake/mcp.json` vs explicit-root `<root>/.skills` + project `.mcp.json`); collapsing them now would obscure that axis. The follow-up is recorded in the finding itself: if a third scope variant ever appears, extract a shared `assert_skill_deployed(base, agent_dir, skill, label)` + `assert_mcp_registered(mcp_path, server, label)` pair then.