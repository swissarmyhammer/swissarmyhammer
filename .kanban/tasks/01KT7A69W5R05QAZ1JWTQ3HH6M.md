---
assignees:
- claude-code
depends_on:
- 01KT7A3Z4FNVZX1GJCMMS65A0F
- 01KT7A4D44637D9Z1THZX6DASP
- 01KT7A4YM770GGFP11JN8ZYNEA
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe480
project: mirdan-install
title: 'Real-path tests: every profile init/deinit is consistent and round-trips'
---
Lock in the "one mechanism, no drift" guarantee with production-path tests across all consumers.

## Cases (drive the REAL init_profile/deinit_profile path, isolated to tempdirs/HOME — never write a real .claude/ or .skills/ into the repo)
1. **Each CLI profile installs the same way**: for sah, shelltool, kanban, code-context — `init_profile(scope)` produces the expected detected-agent skill symlinks (store + symlink, NOT copied files), the expected MCP server registration in the right settings file, and the expected agents; assert the mechanism is identical across all four (same store layout, same lockfile entries, same scope handling).
2. **Round-trip**: `init_profile` then `deinit_profile` leaves the agent config and skill dirs clean (symlinks removed, store entries removed, MCP server unregistered) for each profile.
3. **Explicit-root**: a profile installed with an explicit `root` targets that root and touches no CWD (proves the kanban-app path).
4. **Scope matrix**: Project / Local / User scope each land in the correct location for a representative profile.
5. **No divergent mechanism remains**: assert there is no copy-into-.sah/skills path anymore (the workspace-init mechanism is gone) — a profile deploy is always store+symlink (or the single agreed explicit-root mechanism).
6. **code-context local-scope regression**: code-context's MCP registration now lands in Claude's local scope correctly (the bug the hand-rolled loop had).

## Done when
- All cases pass against the production mirdan init_profile/deinit_profile path.
- A regression that reintroduced a per-app installer or the copy-vs-symlink fork would fail these.
- Tests are HOME/tempdir isolated (mirror the agent-builtins MIRDAN_AGENTS_CONFIG isolation pattern); no repo leakage.

Depends on the sah migration (3), the three-CLI migration (4), and the kanban-app/workspace-init removal (5).

## Review Findings (2026-06-04 04:40)

Scope: working-tree changes; reviewed `crates/mirdan/src/install.rs` `profile_consistency_tests` (the test module this card added) against the production `init_profile`/`deinit_profile` path and the four real CLI `profile(scope)` builders. All 4 consistency tests pass; `cargo clippy -p mirdan --lib --tests` clean. Cases 1-4 and 6 are covered directly; case 5 ("no divergent mechanism") is covered structurally by the `is_symlink()` assertion plus deletion of the copy-into-`.sah/skills` sources and the workspace-init crate.

### Warnings
- [x] `crates/mirdan/src/install.rs:5043-5095` — The consistency tests **reconstruct** each CLI's profile rather than calling the real `apps/*/src/commands/{registry,profile}.rs::profile(scope)`. mirdan cannot depend on the apps (apps → mirdan), so the reconstruction is the only option, but it can drift. RESOLVED: added a "Kept in sync with mirdan profile_consistency_tests::<name>_profile" cross-reference doc comment to each real `profile()`/`sah_profile()` source so an editor of the source sees the coupling — `apps/swissarmyhammer-cli/src/commands/profile.rs::sah_profile`, `apps/shelltool-cli/src/commands/registry.rs::profile`, `apps/kanban-cli/src/commands/registry.rs::profile`, `apps/code-context-cli/src/commands/registry.rs::profile`.

### Nits
- [x] `crates/mirdan/src/install.rs:5183` (`every_cli_profile_installs_by_store_symlink_and_round_trips`) — round-trip (case 2) is asserted only at `Project` scope here. RESOLVED: added a doc-comment note that `Local`-scope round-trip lives in the code-context regression test and per-scope landing in `scope_matrix_lands_artifacts_in_the_right_place`, so a reader does not assume case 2 covers every scope.

## Resolution (2026-06-04)
Both review findings addressed (documentation-only changes — no test/behavior change). `cargo test -p mirdan --lib profile_consistency_tests`: 4 passed. `cargo clippy -p swissarmyhammer-cli -p shelltool-cli -p kanban-cli -p code-context-cli -p mirdan --lib`: clean, zero warnings. Ready for re-review.