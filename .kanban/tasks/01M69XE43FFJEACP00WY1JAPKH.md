---
assignees:
- claude-code
position_column: todo
position_ordinal: ffff9080
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