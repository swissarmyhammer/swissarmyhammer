---
assignees:
- claude-code
depends_on:
- 01KT7A3Z4FNVZX1GJCMMS65A0F
- 01KT7A4D44637D9Z1THZX6DASP
- 01KT7A4YM770GGFP11JN8ZYNEA
position_column: todo
position_ordinal: '8880'
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