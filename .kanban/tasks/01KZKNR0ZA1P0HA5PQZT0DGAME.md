---
assignees:
- claude-code
position_column: todo
position_ordinal: ffae80
title: 'tool install lock: bound the agent turn, tell a timeout from an idle lock, cover the npm shared prefix'
---
Carried out of `^a4ebnw3` when that card reached `done` on its own subject. These four findings are about install concurrency, not duplication. The review evidence is on `^a4ebnw3` under `## Review Findings (2026-08-09 10:55)`.

`InstallLock` was added in `^a4ebnw3` and is real: an exclusive `flock(2)` held while install commands run, released on an unwinding panic through `Drop` and on `SIGKILL` through the kernel closing the descriptor. The double-check around it closes the race. These findings are what a directed review found after that.

## The findings

- [ ] `crates/swissarmyhammer-validators/src/review/tool_install.rs:77` — the 300 s deadline is justified in its own doc by the slowest DECLARED install, but `ensure_tool_installed` holds the same lock across `agent.install(&request).await`. The pool's `PROMPT_TURN_CEILING` is 2700 s and `PROMPT_IDLE_TIMEOUT` is 300 s, so a waiter burns its whole deadline while the holder sits in a turn the pool still calls healthy. The timeout becomes the ordinary outcome, not the exceptional one. Either bound the agent turn under the lock, or set a deadline the agent half can meet, or state in the doc that a waiter behind an agent turn is expected to time out.
- [ ] `crates/swissarmyhammer-validators/src/review/tool_install.rs:354` — on the deadline branch the code warns and installs UNSERIALIZED. That is exactly the race the lock exists to prevent, and a timeout means another process is probably mid-install. Failing is safer here, and it is not a new degradation: `plan.rs:154` and `plan.rs:310` already hold the prompt fallback for a rule whose tool is unusable.
- [ ] `crates/swissarmyhammer-validators/src/review/tool_install.rs:315` — `acquire` returns `None` both when no holder exists (harmless) and when a holder held throughout (a live race), and the caller cannot tell the two apart. The presence re-check in `run_declared_install_commands` covers only a holder that FINISHED. Return a verdict the caller can read.
- [ ] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/preconditions.rs:49` — a binary name read back out of a rule's `doctor.check_command` is interpolated into a shell command string. Escape it, or pass arguments as a list rather than embedding them in the command.

## One destination falls outside BOTH locks

The `^a4ebnw3` doc correction states that `$TMPDIR` covers every per-user destination and names Homebrew as the exception, because Homebrew locks itself. The Homebrew half was verified in Homebrew's own source, not accepted: `FormulaInstaller#install` calls `lock` (`formula_installer.rb:550`), `Formula#lock` builds a `FormulaLock` (`formula.rb:1897`), and `FormulaLock < LockFile` takes `flock(File::LOCK_EX | File::LOCK_NB)` (`lock_file.rb:44`) under the machine-shared `/opt/homebrew/var/homebrew/locks`.

**But npm is covered by neither.** `/opt/homebrew/lib/node_modules/npm/npmrc` sets `prefix = /opt/homebrew`, so under a Homebrew node `npm install -g` writes the shared prefix — and takes no brew lock, because it is not a brew operation. Four shipped rules declare `npm install -g`. Two users installing at once on one machine write the same tree with nothing serializing them.

Decide this with the rest: it may be the argument for a shared lock file after all, or for accepting the exposure and stating it.

#tool-validators #objectivity