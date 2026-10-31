---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m69cnjb1e1ry2zzzvys39s3d
  text: 'The deletion of `apps/shelltool-cli/` is already done in ^94s8tz3. That work also removed the workspace member entry, the `justfile` shelltool lines, and updated `Cargo.lock`. This card covers only the rest: completions, homebrew formula, docs pages, mirdan test cases, comment cleanup, and the old `shelltool` MCP entry cleanup.'
  timestamp: 2026-10-31T16:28:45.793728+00:00
- actor: claude-code
  id: 01m69skg3649vjt8bfzdkdya1j
  text: |-
    ### implement — changed
    - evidence: Deleted completions/_shelltool, shelltool.bash, shelltool.fish and doc/src/reference/shelltool-cli.md. Removed the SUMMARY.md line. homebrew-tap/ is an untracked sibling git repo, so it was NOT touched (Formula/shelltool-cli.rb stays there). Removed shelltool_profile and its case from profile_consistency_tests.rs. The scope-matrix test now uses the kanban profile. Changed the profile_tests.rs serve test to kanban. Fixed every shelltool comment in the files named on the card. New cleanup: Profile.legacy_mcp_servers field and LEGACY_SHELLTOOL_MCP_SERVER constant in crates/mirdan/src/install/profile.rs. init_profile and deinit_profile both remove each listed server from agent MCP configs, next to the legacy Bash deny cleanup. sah_profile (apps/swissarmyhammer-cli/src/commands/profile.rs) lists "shelltool". Real-path tests in profile_tests.rs seed a .mcp.json and prove removal on init and deinit, and that a profile without the list leaves the entry.
    - checks: cargo check --workspace --tests ok; cargo nextest run (rdeps mirdan | kanban-cli | cli-completions) 3005 passed; cargo fmt --check ok; cargo clippy --workspace --all-targets -D warnings ok. Repo grep for shelltool: only the new cleanup code remains.
    - review: review working found 3 findings (duplicate dispatch, hand-rolled error assertion twice). All fixed (unregister_named_mcp_server helper, assert_no_init_error). A whole-file review of profile*.rs reports 10 findings, all in code this task did not change; left alone as unrelated refactors.
    - next: /review
  timestamp: 2026-10-31T20:14:49.446698+00:00
- actor: claude-code
  id: 01m69ss9k2b6hndjqs4vjves41
  text: |-
    ### test — green
    - evidence: cargo nextest run --override-version-check --workspace --no-fail-fast: 14049 run, 14049 passed, 0 failed, 0 skipped. cargo clippy --workspace --all-targets -- -D warnings: exit 0, zero warnings. cargo fmt --check: exit 0.
    - next: review
  timestamp: 2026-10-31T20:17:59.394101+00:00
- actor: claude-code
  id: 01m69sypryeek58rjf87yxnf79
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (commit 7502aa15c). 0 findings, 0 confirmed, 1 refuted, 8 attempted, 0 failed. 5 files had no matching validator (README, shelltool completions, doc pages). 2 .kanban files ignored.
    - next: task moved to done.
  timestamp: 2026-10-31T20:20:56.734123+00:00
- actor: claude-code
  id: 01m69sz39jnr2tqjkzxfwz0mk6
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 24 files
    - test: green — nextest workspace 14049 passed, clippy and fmt clean
    - commit: 7502aa15c
    - review: clean — zero new findings, task done
  timestamp: 2026-10-31T20:21:09.554348+00:00
depends_on:
- 01M695D70TVS1PX9QF894S8TZ3
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffca80
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