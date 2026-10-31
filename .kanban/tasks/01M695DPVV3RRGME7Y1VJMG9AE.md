---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m69t6ja8z88hj5mkrc07d4fh
  text: |-
    ### implement — changed
    - evidence: removed crates/swissarmyhammer-shell/ and builtin/shell/ (git rm). Removed the crate from Cargo.toml members and [workspace.dependencies]. `cargo update -w` dropped 18 lines from Cargo.lock. Search for swissarmyhammer-shell, swissarmyhammer_shell and builtin/shell finds nothing outside .kanban and the stray nested dir. Checks: cargo check --workspace --tests OK, cargo build --workspace OK, cargo nextest run --workspace 13943 passed 0 failed, cargo fmt --check OK, cargo clippy --workspace --all-targets -D warnings OK. No Rust file was changed, so no review file run.
    - notes: `.gitignore` entry `**/.shell/` is kept. `ShellConfig` in swissarmyhammer-directory still can write `.shell`. A new card removes ShellConfig and then the entries. The untracked dir crates/swissarmyhammer-tools/.shell exists (a log file and a .gitignore); left alone. An untracked nested copy `swissarmyhammer/` inside the repo root still holds old shell code; left alone, it is not part of the workspace.
    - next: /review
  timestamp: 2026-10-31T20:25:14.312399+00:00
- actor: claude-code
  id: 01m69tbrah5emetksdgxr9kprb
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace --no-fail-fast: 13943 run, 13943 passed, 0 failed, 0 skipped (1 leaky, 30 slow, all passed). cargo clippy --workspace --all-targets -- -D warnings: exit 0, 0 warnings. cargo fmt --check: exit 0.
    - next: review
  timestamp: 2026-10-31T20:28:04.305728+00:00
- actor: claude-code
  id: 01m69tfktzbtv6nb7672akaqz0
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD, 0 findings (attempted 7, failed 0). Cargo.lock and builtin/shell/config.yaml had no matching validator.
    - next: none. Task moved to done.
  timestamp: 2026-10-31T20:30:10.783831+00:00
- actor: claude-code
  id: 01m69tfy3c3pke4zw494j0smaa
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — crate and builtin config deleted
    - test: green — nextest workspace 13943 passed, clippy and fmt clean
    - commit: cbd009bbd
    - review: clean — zero new findings, task done
  timestamp: 2026-10-31T20:30:21.292295+00:00
depends_on:
- 01M695D70TVS1PX9QF894S8TZ3
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffcb80
project: remove-shelltool
title: Delete the swissarmyhammer-shell crate and shell config
---
Delete the crate that only the shell tool used.

Work:
- Delete `crates/swissarmyhammer-shell/` (security, performance, config, tests).
- Delete `builtin/shell/config.yaml`.
- Remove the crate from the workspace `Cargo.toml` and from `crates/swissarmyhammer/Cargo.toml`.
- Check that no other crate uses `swissarmyhammer_shell` (only the tools crate and shelltool-cli did).
- Remove the `.shell` dot-folder from `.gitignore` files if sah created it.
- Run `cargo update -w`.

Depends on the card that removes the MCP tool.
Done when: `cargo build --workspace` passes and a search for `swissarmyhammer-shell` finds nothing. #remove-shell