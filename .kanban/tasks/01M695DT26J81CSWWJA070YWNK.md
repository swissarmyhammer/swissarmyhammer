---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m69cnjb1e1ry2zzzvys39s3d
  text: 'The deletion of `apps/shelltool-cli/` is already done in ^94s8tz3. That work also removed the workspace member entry, the `justfile` shelltool lines, and updated `Cargo.lock`. This card covers only the rest: completions, homebrew formula, docs pages, mirdan test cases, comment cleanup, and the old `shelltool` MCP entry cleanup.'
  timestamp: 2026-10-31T16:28:45.793728+00:00
depends_on:
- 01M695D70TVS1PX9QF894S8TZ3
position_column: todo
position_ordinal: ffff8e80
project: remove-shelltool
title: Delete the shelltool CLI binary and its packaging
---
Delete the `shelltool` binary and everything that installs or documents it.

Work:
- Delete `apps/shelltool-cli/`.
- Remove it from the workspace `Cargo.toml:65`. Run `cargo update -w`.
- Delete the `justfile` lines 10 and 22-23.
- Delete `completions/shelltool.bash`, `shelltool.fish` and `_shelltool`.
- Delete `homebrew-tap/Formula/shelltool-cli.rb`.
- Delete `doc/src/reference/shelltool-cli.md` and its line in `doc/src/SUMMARY.md:23`.
- Remove `shelltool_profile` and the shelltool case from `crates/mirdan/src/install/profile_consistency_tests.rs` and `profile_tests.rs:351`. Keep the kanban and code-context cases.
- Fix comments that name shelltool in `kanban-cli` (doctor.rs, serve.rs, build.rs, README), `swissarmyhammer-cli/logging.rs`, `cli-completions`, `mirdan/tool_install.rs`, `skills/deploy.rs`, `operations/cli_gen.rs`, `build-support/doc_gen.rs`.
- Add a cleanup step: `sah deinit` or `sah doctor` removes an old `shelltool` entry from agent MCP configs.

Done when: `cargo build --workspace` and `cargo test -p mirdan` pass. #remove-shell