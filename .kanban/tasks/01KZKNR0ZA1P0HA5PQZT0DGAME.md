---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzm38v5y1hsvn171wtc5bx0f
  text: |-
    Research. The line numbers on this card were stale — `^a4ebnw3` iteration 7 changed `tool_install.rs` after the card was written. Each finding was found by its content.

    - Finding 1: `ensure_tool_installed` held `InstallLock::acquire()` across `agent.install(&request).await`, with no bound of its own. `PROMPT_TURN_CEILING` is 45 minutes and `PROMPT_IDLE_TIMEOUT` is 300 s, against an `INSTALL_LOCK_WAIT` of 300 s.
    - Finding 2: `InstallLock::take` warned "installing unserialized" and returned `None` on the deadline branch, and both callers went ahead.
    - Finding 3: `acquire` returned `Option<Self>`; `None` meant a lock file that could not be opened, an `flock(2)` error, OR a holder that never let go.
    - Finding 4: `binary_present` built `format!("which {binary}")`, where `binary` is a word `checked_binaries` split out of a rule's `doctor.check_command`.

    npm verification. `/opt/homebrew/lib/node_modules/npm/npmrc` reads `prefix = /opt/homebrew`, and `/opt/homebrew/bin/npm config get prefix` answers `/opt/homebrew`. The nvm node on this machine answers a per-user path, so the prefix truly follows the node on `PATH`. Four shipped rules declare `npm install -g`: `magic-numbers-typescript`, `dead-code-typescript`, `complexity-typescript`, `missing-docs-typescript`.
  timestamp: 2026-08-09T20:26:17.662381+00:00
- actor: claude-code
  id: 01kzm39ttc9r5fd5fhvem9w06h
  text: |-
    ### implement — changed
    - evidence: 3 files — `crates/swissarmyhammer-validators/src/review/tool_install.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/preconditions.rs`, `builtin/validators/README.md`.
    - RED first, each failure seen: `a_checked_binary_name_reaches_which_as_one_argument` failed with "the name must reach `which` as one argument; the shell read it as a redirect instead" (the shell made the file); `a_contended_lock_is_told_apart_from_a_lock_the_machine_cannot_give` failed with `left: true, right: true` (both answers were `None`); `a_contended_install_lock_runs_no_install_command` failed with "a blocked install installed nothing, so it cannot report the tool present"; `an_install_agent_turn_that_never_answers_is_bounded` failed with "the lifecycle must bound the agent turn; it held the install lock past 2s".
    - gates: `cargo nextest run --workspace` 13995 passed, 0 failed, 0 skipped. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - HEAD at the end: `8aa14e66f`.
    - next: `/review`.
  timestamp: 2026-08-09T20:26:50.060163+00:00
- actor: claude-code
  id: 01kzm3w2r9jgmgyfngpkxfcmtc
  text: |-
    ### review — clean
    - scope: `review sha 9c0f5031f^..9c0f5031f` (explicit range, not HEAD-relative — a parallel session moves HEAD in this working tree). The commit is `fix(validators): bound install turn, tell timeout from idle lock (^t0dgame)`, 3 files, 405 insertions, 66 deletions.
    - evidence: counts — findings 0, confirmed 0, refuted 0, attempted 9, failed 0, skipped 0, skipped_files none. No file:line list, because the engine found nothing.
    - prior state: the 4 items under `## The findings` are all `- [x]`. This card holds no earlier `## Review Findings` section; the earlier evidence is on `^a4ebnw3`.
    - next: none. The card moves to `done`.
  timestamp: 2026-08-09T20:36:48.009455+00:00
- actor: claude-code
  id: 01kzm3x1adv8e2pxdc9f299wfr
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 3 files (`tool_install.rs`, `tool_rules/tests/preconditions.rs`, `builtin/validators/README.md`); all 4 findings fixed, RED seen for each before the fix
    - test: green — `cargo nextest run --workspace`, 13995 passed, 0 failed, 0 skipped; fmt clean; clippy clean
    - commit: 9c0f5031f
    - review: clean — `review sha 9c0f5031f^..9c0f5031f`, 0 findings, 9 validators attempted, 0 failed, 0 skipped
    - result: the card is in `done`. One iteration, no repeat finding, the guardrail did not apply.
  timestamp: 2026-08-09T20:37:19.309296+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffd380
title: 'tool install lock: bound the agent turn, tell a timeout from an idle lock, cover the npm shared prefix'
---
Carried out of `^a4ebnw3` when that card reached `done` on its own subject. These four findings are about install concurrency, not duplication. The review evidence is on `^a4ebnw3` under `## Review Findings (2026-08-09 10:55)`.

`InstallLock` was added in `^a4ebnw3` and is real: an exclusive `flock(2)` held while install commands run, released on an unwinding panic through `Drop` and on `SIGKILL` through the kernel closing the descriptor. The double-check around it closes the race. These findings are what a directed review found after that.

NOTE: the line numbers below are stale. `^a4ebnw3` iteration 7 changed `tool_install.rs` after this card was written. Find each item by its content.

## The findings

- [x] `tool_install.rs` `INSTALL_LOCK_WAIT` — the 300 s deadline is justified in its own doc by the slowest DECLARED install, but `ensure_tool_installed` holds the same lock across `agent.install(&request).await`. The pool's `PROMPT_TURN_CEILING` is 2700 s and `PROMPT_IDLE_TIMEOUT` is 300 s, so a waiter burns its whole deadline while the holder sits in a turn the pool still calls healthy. FIXED: `ensure_tool_installed` now bounds the turn with `tokio::time::timeout(INSTALL_AGENT_TURN_WAIT, ...)`, and the wait covers a whole holder — `SLOWEST_DECLARED_INSTALL` plus `INSTALL_AGENT_TURN_WAIT`.
- [x] `tool_install.rs` deadline branch — the code warned and installed UNSERIALIZED. FIXED: the branch now reports the new `ToolInstallOutcome::Blocked`, which runs no command. `tool_present()` is false for it, so the superseded prompt rule runs, which is step 4 of the lifecycle.
- [x] `tool_install.rs` `InstallLock::acquire` — it returned `None` both when no holder exists and when a holder held throughout. FIXED: `acquire`, `acquire_at` and `take` return `InstallLockVerdict::{Held, Blocked, Unlocked}`. `Blocked` stops the install; `Unlocked` installs unserialized, as before.
- [x] `tool_rules/tests/preconditions.rs` `binary_present` — a binary name read out of a rule's `doctor.check_command` was put into a shell command string. FIXED: the name rides in as the script's positional parameter, and the script reads `which "$@"`.

## One destination falls outside BOTH locks

The `^a4ebnw3` doc correction states that `$TMPDIR` covers every per-user destination and names Homebrew as the exception, because Homebrew locks itself. The Homebrew half was verified in Homebrew's own source, not accepted: `FormulaInstaller#install` calls `lock` (`formula_installer.rb:550`), `Formula#lock` builds a `FormulaLock` (`formula.rb:1897`), and `FormulaLock < LockFile` takes `flock(File::LOCK_EX | File::LOCK_NB)` (`lock_file.rb:44`) under the machine-shared `/opt/homebrew/var/homebrew/locks`.

**But npm is covered by neither.** `/opt/homebrew/lib/node_modules/npm/npmrc` sets `prefix = /opt/homebrew`, so under a Homebrew node `npm install -g` writes the shared prefix — and takes no brew lock, because it is not a brew operation. Four shipped rules declare `npm install -g`. Two users installing at once on one machine write the same tree with nothing serializing them.

DECISION: accept the exposure, and state it. The claim was verified on this machine: `/opt/homebrew/bin/npm config get prefix` answers `/opt/homebrew`, the nvm node answers a per-user path. A shared lock file was rejected because it must sit in a world-writable directory, where any local user can hold it — and a wait that ends now installs nothing, so one held file would stop every other user's installs on the machine. The narrow race is the smaller hazard. The `InstallLock` doc and `builtin/validators/README.md` now state the uncovered case in those words.

#tool-validators #objectivity