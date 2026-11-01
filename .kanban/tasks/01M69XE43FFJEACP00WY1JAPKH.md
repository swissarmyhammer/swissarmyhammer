---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6c6vm027tkysrc9ewqzmys5
  text: |-
    ### implement — changed
    - Added Profile.legacy_skills; remove_legacy_skills in the shared settings tail of init_profile and deinit_profile; goes through uninstall_skill_at (symlink and real dir). Skips names the profile still deploys; NotFound is ignored.
    - Profile already derives Default; the 3 literals without ..Default (profile_tests, profile_consistency_tests, CLI sah_profile) got the field explicitly. `shell` is in LEGACY_SKILLS in apps/swissarmyhammer-cli/src/commands/profile.rs.
    - Tests: new crates/mirdan/src/install/legacy_skill_tests.rs (9 tests, real init/deinit_profile, user + project scope, shell-notes stays). RED seen on 5 first.
    - Files: crates/mirdan/src/install/{profile.rs,mod.rs,profile_tests.rs,profile_consistency_tests.rs,legacy_skill_tests.rs}, apps/swissarmyhammer-cli/src/commands/profile.rs
    - Verify: nextest -p mirdan -p swissarmyhammer-cli -p code-context-cli: 1284 passed, 0 skipped; workspace build all-targets: 0 warnings; cargo fmt clean.
  timestamp: 2026-11-01T18:44:55.938300+00:00
- actor: claude-code
  id: 01m6c6wx9vk58apr0c9y98chgz
  text: |-
    ### commit — changed
    - evidence: 60beb26b6 feat(mirdan): remove retired skills on init and deinit (6 files: profile.rs, mod.rs, profile_tests.rs, profile_consistency_tests.rs, legacy_skill_tests.rs, apps/swissarmyhammer-cli/src/commands/profile.rs)
    - next: review
  timestamp: 2026-11-01T18:45:38.235109+00:00
- actor: claude-code
  id: 01m6c75wb55ej920ayq3cq8fps
  text: |-
    ### review — findings
    - scope: review sha 60beb26b6~1..60beb26b6 (6 files). counts: 1 finding, 1 confirmed, 1 refuted.
    - open finding: `crates/mirdan/src/install/profile.rs:1414` `reuse/reuse` — remove_legacy_skills hand-rolls the uninstall loop, warning emit, and per-name result that deinit_profile_items already provides. Route the legacy skill removal through deinit_profile_items, with a wrapper that maps Err(RegistryError::NotFound(_)) to Ok(()); delete the hand-written loop and manual count; keep the result message.
    - acceptance, met by the commit: legacy_skills field on Profile; init_profile and deinit_profile call remove_legacy_skills through profile_settings_tail_results; removal uses uninstall_skill_at (symlinks, store, nested store); `shell` is in the single LEGACY_SKILLS list in apps/swissarmyhammer-cli/src/commands/profile.rs; a name in Profile.skills is filtered out; `shell-notes` is kept (USER_SKILL in legacy_skill_tests.rs:23, asserted at user and project scope, init and deinit); 9 real-path tests, no mocks.
    - no test asserts on the text of skill, source or markdown files. SKILL.md is written as seed data only (legacy_skill_tests.rs:101); every assert checks presence with symlink_metadata.
    - UNMEASURED: the last Done-when line (`find ~ -maxdepth 4 -path '*/skills/shell'` after `sah init user`) was not run against the real home directory. Judgment: it follows in part. The tests drive init_profile at InitScope::User through the same remove_legacy_skills and uninstall_skill_at path, and `sah init user` builds sah_profile() (install/mod.rs:86), whose legacy_skills equals ["shell"] (asserted in commands/profile.rs tests). But no test runs sah_profile() through init_profile; the tests use their own retiring_profile(). The join of the two is by inspection only.
    - next: implementer fixes the open finding; re-run review.

    step: review
    outcome: findings
    evidence: 1 finding — crates/mirdan/src/install/profile.rs:1414; 9 tests inspected, 0 text assertions; real-home find unmeasured
    task: ^y1japkh
  timestamp: 2026-11-01T18:50:32.165683+00:00
- actor: claude-code
  id: 01m6c7d6w970p1t08ggapg01qz
  text: |-
    ### implement — changed
    - evidence: crates/mirdan/src/install/profile.rs, apps/swissarmyhammer-cli/src/commands/profile.rs. remove_legacy_skills now routes through deinit_profile_items. The helper closure returns Result<bool>: Ok(false) = absent, silent, not counted. Hand loop and manual count deleted. Result message "Removed N legacy skill(s)" is unchanged. Warning text now "Failed to uninstall {name} legacy skill" (shared text). Other loops: remove_legacy_mcp_servers is a flat_map over unregister_named_mcp_server, no loop/warn/count copy, left as is. deinit_profile_validators has a similar remove loop but uses remove_dir_all, a path warning and a Vec result; not routed (different error type, would change messages). New real-path test test_sah_init_removes_retired_skills_and_keeps_current runs sah_profile() through init_profile with temp dirs. legacy_skill_tests.rs untouched. Run: 1286 passed, 0 skipped, 0 warnings.
    - next: /review
  timestamp: 2026-11-01T18:54:32.329761+00:00
- actor: claude-code
  id: 01m6c7e4rxkfr37j7jbfshnvce
  text: |-
    ### commit — changed
    - evidence: fe5a5d935 refactor(mirdan): route legacy skill removal through the shared deinit helper; files: crates/mirdan/src/install/profile.rs, apps/swissarmyhammer-cli/src/commands/profile.rs
    - next: review
  timestamp: 2026-11-01T18:55:02.941801+00:00
- actor: claude-code
  id: 01m6c7k8zhd853jgyeejkrd213
  text: |-
    ### review — clean
    - evidence: review sha fe5a5d935~1..fe5a5d935: 0 findings (0 confirmed, 2 refuted, 7 attempted, 0 failed). The board reads the review 1 finding at profile.rs:1414 as `- [x]`. legacy_skill_tests.rs is not in the commit (2 files changed: crates/mirdan/src/install/profile.rs, apps/swissarmyhammer-cli/src/commands/profile.rs); it still has its 9 tests. New test test_sah_init_removes_retired_skills_and_keeps_current (apps/swissarmyhammer-cli/src/commands/profile.rs) asserts only on path existence, not on file text. Implementer run: 1286 passed, 0 skipped.
    - last card line: the test runs the real sah_profile() legacy list through init_profile at user scope in temp dirs. It seeds a store entry and an agent symlink for each retired name, then asserts both are gone and that the current skill "commit" stays. So the store and agent-folder removal at user scope follows from the test. The literal `find ~ -maxdepth 4` on the real home is not measured, by instruction. It follows, because the code path and the retired list are the real ones.
    - next: done.
  timestamp: 2026-11-01T18:57:51.089481+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffe780
project: remove-shelltool
title: Install removes the retired shell skill from the store and every agent
---
The `shell` skill is gone from `builtin/skills/`. But `sah init` and `sah deinit` never remove a skill that left the builtin set. Proof from `sah init user` run on 2026-10-31:
- `~/.skills/shell/SKILL.md` is still there (real copy in the skill store).
- `~/.claude/skills/shell` is a symlink to `../../.skills/shell`.
- The zed agent skill folder and `.github/copilot/skills/shell` hold the same symlinks. Old project installs (`.claude/skills/shell`, `.zed/skills/shell`) hold copies too.

Do:
- Add a `legacy_skills: Vec<String>` field to mirdan `Profile` (next to `legacy_mcp_servers` in `crates/mirdan/src/install/profile.rs`). A retired skill name goes in this list.
- `init_profile` and `deinit_profile` remove each legacy skill from the skill store (global and project scope as the profile scope says) and from every detected agent skill folder, including the symlinks. Use the same helper path that removes a current skill, so a symlink and a real directory both go.
- Put `shell` in the legacy list of the `sah` profile. Make it easy to add the next retired skill: one list, one place.
- Never remove a skill that the profile still deploys. Never remove a user-made skill with a different name.
- Real-path tests (no mocks of the install path): seed a store with `shell` plus agent symlinks, run `init_profile`, assert all are gone and the current skills stay. Run `deinit_profile` with the same seed and assert the same. Add a test that a user skill named `shell-notes` stays.
- Do not edit other people's repos. The copies in `andrewbusey/hence`, `calcutron` and `tmp/s` are outside this repo. Do not touch them.

Done when: after `sah init user`, `find ~ -maxdepth 4 -path '*/skills/shell'` finds nothing at user scope, and `cargo nextest run --workspace` is green. #remove-shell

## Review Findings (2026-11-01 12:45)

> Scope: `review sha 60beb26b6~1..60beb26b6` — reviewed the diffs only — lines this change added or modified. 6 file(s) reviewed, 0 not reviewed.

- [x] `crates/mirdan/src/install/profile.rs:1414` `reuse/reuse` — remove_legacy_skills hand-rolls the uninstall loop, warning emit, and per-name result that deinit_profile_items already provides. It duplicates the existing shared helper instead of extending it. The one real difference is that NotFound is silently skipped, and that difference is what should be parameterized. Two copies of this loop will diverge in warning text, counting, and result shape. Route the legacy skill removal through deinit_profile_items. Add a parameter, or a wrapper that maps Err(RegistryError::NotFound(_)) to Ok(()) before the call, so a retired name that is not installed stays silent. Pass the filtered retired names and the same ComponentName and ItemKind. Delete the hand-written loop and the manual count, and keep the result message the same.
