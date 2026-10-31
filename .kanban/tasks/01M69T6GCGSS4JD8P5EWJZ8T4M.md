---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m69w8w3pea4qgpccbab7ep0a
  text: |-
    ### implement — changed
    - evidence: Search showed nothing but ShellConfig's own tests used it, and no Rust code wrote .shell. Files changed: crates/swissarmyhammer-directory/src/config.rs, directory.rs, lib.rs (ShellConfig, its tests and its export removed); crates/swissarmyhammer-common/src/logging.rs (docs and tests no longer name .shell/shell); .gitignore (`**/.shell/` entry and shell wording removed); standards/mcp.md (removed shell as a live example, used git/code_context instead). Kept `.shell` in warn_legacy_paths: it warns about old user dirs, so it must still detect them.
    - checks: cargo check --workspace --tests OK; cargo fmt --check OK; cargo clippy --workspace --all-targets -D warnings OK; nextest rdeps(swissarmyhammer-directory) 11848 of 11849 pass. The one failure, swissarmyhammer-treesitter index::tests::test_status_after_clear (27s under load), passed when run alone (10s). It is not related to this change. review working: 0 findings.
    - not touched: untracked crates/swissarmyhammer-tools/.shell, nested swissarmyhammer/, sibling dirs, doc/src/concepts/closed-write-surface.md, ideas/.
    - next: /review
  timestamp: 2026-10-31T21:01:27.030920+00:00
- actor: claude-code
  id: 01m69wechhxe7erv93mvkmqnya
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace --no-fail-fast: 13941 run, 13941 passed (30 slow), 0 failed, 0 skipped. cargo clippy --workspace --all-targets -- -D warnings: exit 0, no warnings. cargo fmt --check: exit 0.
    - note: swissarmyhammer-treesitter index::tests::test_status_after_clear passed in this run. The earlier failure was a timing fault under load.
    - next: review
  timestamp: 2026-10-31T21:04:27.697767+00:00
- actor: claude-code
  id: 01m69wjxkrqx6hcspmvpemqkd3
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD gave 0 findings (7 attempted, 0 failed). .gitignore and standards/mcp.md have no validator, so I checked them by hand. standards/mcp.md has no remaining "shell" word. All headings are intact. The new claims match the code: git has the default op "get changes", git uses an unsafe transmute, git has git_schema_config, and git and code_context do not use ExecutionResult. get_tool_description("files","read") is valid. No ShellConfig remains in crates, standards or .gitignore. The ~/.shell entry in the legacy dir list stays on purpose, because it detects old user dirs.
    - next: move to done
  timestamp: 2026-10-31T21:06:56.248037+00:00
- actor: claude-code
  id: 01m69wkfex8rr5rd42hn53mteh
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 6 files
    - test: green — nextest workspace 13941 passed, clippy and fmt clean
    - commit: aaed1a625
    - review: clean — zero new findings, task done
  timestamp: 2026-10-31T21:07:14.525565+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffcd80
project: remove-shelltool
title: Remove the unused ShellConfig and the .shell gitignore entries
---
The shell crate is gone. `ShellConfig` in `crates/swissarmyhammer-directory/src/config.rs` has no user outside its own tests. Remove `ShellConfig`, its export in `lib.rs`, and its tests in `directory.rs`. Remove `ShellConfig` from the legacy dir list in `directory.rs` only if the list goal allows it. Then remove `**/.shell/` from `.gitignore`. Update the `.shell` text in `crates/swissarmyhammer-common/src/logging.rs` docs. Card ^vjmg9ae left these in place because `ShellConfig` can still write `.shell`. #remove-shell