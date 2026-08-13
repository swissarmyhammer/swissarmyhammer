---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzttx7rjz9aa5r9ggmqd9m4p
  text: |-
    ## Measurements re-taken (not copied from the card)

    Machine: bash 3.2.57(1)-release. `/bin/sh` is the same binary here. Each line
    was run the way `crate::doctor::run_shell` runs a rule script:
    `bash -c <script> bash <argument>...`, where the script is the candidate line,
    then the three guard lines, then a tool line. Correct = exit 0 and nothing on
    stdout with no argument, AND the tool line runs with one argument.

    ### Roster (the card's numbers moved)

    - 14 shipped rules state `scope: files`, not 16. Commit 59bd9ae5c deleted two.
      `FILES_SCOPE_RULE_COUNT` already reads 14.
    - 7 of the 14 write the guard on the first line. 7 write exactly 1 line above
      it, and each of those 7 lines is `set -e`. So 7 prefix lines in all, of 1
      distinct shape — not the card's 5.

    ### The shell (item 4)

    `run_shell` calls `shell_command(Shell::Bash, script)`, which spawns
    `bash -c`. The two shells give the same 27 `set -o` names, so the name list is
    unchanged. They DISAGREE on errors, which is why the name matters:
    `set -y` above the guard exits 2 under `/bin/sh` (POSIX mode) and under bash
    writes `set: -y: invalid option` to stderr and carries on.

    ### Item 1 — short-option words

    - `set -oe` -> exit 0, 27 lines on stdout. `set -ox` -> the same. Accepted
      today.
    - `set -eo` -> 27 lines (rejected today).
    - `set -oo pipefail` -> 27 lines. `set -oo pipefail errexit` -> correct. So a
      word holds ONE name for each `o` it writes.
    - `set -o -e` -> 27 lines.
    - `set -oe pipefail` -> correct. `set -eou pipefail` -> correct (rejected
      today).
    - Over the 52 ASCII letters, `set -X` sets an option for 23 of them —
      `abefhikmnoprtuvxBCEHIPT` — and writes `set: -X: invalid option` plus a usage
      line for the other 29.
    - `set -n` is noexec: with 1 argument the tool line NEVER runs. It is dropped
      from the accepted letters.
    - `set -` and `set +` -> correct, no stderr.

    ### Item 2 — quoted names, and the other prologue heads

    - 4 quoted shapes, all correct, all rejected today: `set -o "pipefail"`,
      `set -o 'pipefail'`, `set -euo "pipefail"`, `set -o "pipefail" # keep going`.
    - `set -o "pipefail` (quotation never closed) -> exit 2, the tool line never
      runs. Must stay rejected.
    - All 16 prologue lines the card names are correct and all 16 are rejected
      today: `export LC_ALL=C`, `export FOO=bar`,
      `export PATH="$HOME/.local/bin:$PATH"`, `export LC_ALL=C LANG=C`, `LC_ALL=C`,
      `PATH="/usr/bin:$PATH"`, `readonly LIMIT=15`, `IFS=$'\n'`, `unset FOO`,
      `umask 022`, `shopt -s nullglob`, `hash -r`, `alias ll=ls`,
      `trap 'exit 1' INT`, `:`, `true`.
    - These stay rejected: `LC_ALL=C tool "$@"`, `export FOO=$(tool)`, `shift`,
      `umask 022; tool "$@"`.

    ### Item 3 — `set -o rm` as the first line of a SCRIPT

    Under bash: writes `bash: line 0: set: rm: invalid option name` to stderr,
    writes nothing to stdout, sets no option, the run exits 0, and the lines under
    it still run. `sh -c 'set -o rm'` — the form the doc comment states — exits 1.

    ### Measurements the shell name changes

    - `set -e<in.txt` with the file absent: under `/bin/sh` exit 1; under bash the
      redirection fails, `set` never runs, errexit stays off, and the run exits 0.
      The doc comment must say what bash does.
    - Unchanged under bash: `set -e>out.txt` exits 0 and cuts an 11-byte file to 0
      bytes; a trailing backslash joins the guard line and the run exits 2 with a
      syntax error; `set -o >x` writes the 27 options into `x`.
    - Whole scripts, re-run under bash: the guard under `mktemp -d` left 1
      directory, and 0 with the trap above the guard; `wc` read the file before the
      guard answered; the guard in a subshell, in a function body and in a heredoc
      each let the tool line run; `set -- $(find . -name '*.txt')` made the tool run
      over the files the script found.
  timestamp: 2026-08-12T11:14:49.746466+00:00
- actor: claude-code
  id: 01kztveg7brsft2p18emewpjqn
  text: |-
    ## Implementation landed

    Test-first. The two lists went in first and 3 tests failed for the stated
    reasons — `set -eou pipefail` rejected, `set -oe` accepted, the prologue script
    rejected — then the reader changed and all 5 pass.

    ### The reader now

    - `words_that_run` drops a trailing comment and gives the words the shell runs.
      A blank line and a comment-only line give no word, so `nothing_runs_before` no
      longer holds its own blank and comment branches.
    - `sets_shell_options_only` takes the WORDS UNDER `set`, not the whole line. It
      holds each letter of a cluster to `SHELL_SHORT_OPTION_LETTERS` (21 measured
      letters) and counts one owed name for each `o` the cluster writes, wherever
      the `o` stands. A line that owes a name and never takes one answers false, so
      `set -oe` and `set -oo pipefail` are rejected and `set -oe pipefail` and
      `set -oo pipefail errexit` are taken.
    - `shell_word_value` removes a matched quotation pair from a name word and
      answers `None` for a quotation that never closes, so `set -o "pipefail"` is
      taken and `set -o "pipefail` is rejected.
    - `starts_no_tool` is the new top level. It takes a `set` line, a line whose
      head is one of `SHELL_SETUP_HEADS` (`:`, `alias`, `export`, `hash`,
      `readonly`, `shopt`, `trap`, `true`, `umask`, `unset`), and a line whose every
      word writes a shell variable. `shift` is not a head, and
      `LC_ALL=C tool "$@"` is rejected because `tool` writes no variable.

    ### Two marks became exact rather than broad

    Both were needed for the 16 prologue lines, and neither lets any earlier shape
    through:

    - `$` left `SHELL_COMMAND_MARKS`; `SHELL_COMMAND_SUBSTITUTION` is `$(`. A
      parameter expansion runs no command and can produce no command mark, so
      `export PATH="$HOME/.local/bin:$PATH"` is safe. Every rejected shape that held
      `$` holds `$(` as well.
    - The backslash left `SHELL_COMMAND_MARKS`; `SHELL_LINE_JOIN` is tested at the
      END of the line only. A backslash elsewhere quotes the character under it, so
      `IFS=$'\n'` is safe. The three rejected shapes each END with a backslash.

    ### `set -n` is rejected, with a measurement

    `n` is a valid bash `set` letter, so a list of accepted letters had to decide
    it. Measured over a run of 1 argument, `set -n` above the guard makes the shell
    read the whole script and run none of it: the tool line never runs and the rule
    answers every review with no finding. That is the failure this module exists to
    catch, so `n` is out and `set -n` stands in `LINES_THAT_RUN`. That list's doc
    and its assertion message now state all four reasons a line reaches it.

    ### Beyond the four items, and why

    `builtin/validators/README.md` states this contract under the `run` key, and
    the module doc of the test file names the README as the statement of the rule
    it enforces. The README said "An option cluster that ends with `o` takes a
    name" and "Only a comment, a blank line and a shell option line stand over
    them". Both are now false of the reader, so the README was rewritten to state
    the same rule, with the shell named.

    ### Evidence

    - `cargo nextest run -p swissarmyhammer-validators`: 690 run, 690 passed, 0
      failed.
    - `cargo nextest run -p mirdan`: 503 run, 503 passed (it `include_str!`s the
      README).
    - `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D
      warnings` clean.
    - 3 files: `builtin/validators/README.md`,
      `crates/swissarmyhammer-validators/src/doctor.rs`,
      `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/zero_argument.rs`.

    ### implement — changed

    - evidence: 3 files — builtin/validators/README.md,
      crates/swissarmyhammer-validators/src/doctor.rs,
      crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/zero_argument.rs;
      690/690 validators tests pass, 503/503 mirdan tests pass, fmt and clippy clean
    - next: /review
  timestamp: 2026-08-12T11:24:15.467093+00:00
- actor: claude-code
  id: 01kztw0gmbmyax7pmncbq80fkj
  text: |-
    ### review — clean
    - evidence: `review sha 9546522c8~1..9546522c8` — 0 findings, 0 confirmed, 0 refuted, 8 validators attempted, 0 failed, 0 skipped
    - next: task moved to done
  timestamp: 2026-08-12T11:34:05.707391+00:00
- actor: claude-code
  id: 01kztw12eyxea482g91f9t2yx3
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 3 files (tests/shipped/zero_argument.rs, doctor.rs, builtin/validators/README.md). All four card items done, with the measurements re-taken under bash rather than copied. The roster moved as the card warned: 14 `scope: files` rules, not 16, and 7 prefix lines, not 5, each of them `set -e`. Two corrections to the card's own numbers: under bash an invalid `set -X` does NOT stop the script (that was the /bin/sh answer), and `set -o rm` as a script's first line lets the lines under it run. One item beyond the card: `set -n` is newly rejected, because above the guard it makes the shell read the whole script and run none of it, so the rule answers every review with no finding. The `$` and backslash marks were made exact ($( and line-end only), never broader.
    - test: green — cargo nextest run --workspace, 14138 passed, 0 failed, 0 skipped. fmt and clippy clean. mirdan checked, since it include_str!s the README.
    - commit: 9546522c8
    - review: clean — 0 findings over 9546522c8~1..9546522c8, task moved to done
  timestamp: 2026-08-12T11:34:23.966102+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffed80
title: The zero-argument coverage guard reads a shell prologue line wrong in two ways
---
Split out of ^6585731. That card asked for a zero-argument guard on each
`files`-scope rule and a trap under each `mktemp -d`. Both landed, and two
coverage guards hold them over the shipped bytes. This card carries the
remaining work on the SHELL-LINE READER inside one of those guards.

The reader is `sets_shell_options_only` and `nothing_runs_before` in
`crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/zero_argument.rs`.
It decides which lines may stand above the guard.

**No shipped rule is at risk today.** 166 shapes were run in a shell, and 76
gave a wrong answer. 0 of the 76 stand in a shipped script. The 16
`files`-scope scripts hold 5 prefix lines in all, and each of the 5 is
`set -e`.

## Two structural causes

1. **A short-option word is judged by its first character and its last
   character alone.** 46 single letters were run. 26 of them make `set` stop
   the script at status 2 before the guard runs, and the reader accepts all
   26. The same cause accepts `set -oe`, which exits 0 and writes 27 lines of
   shell options to stdout, and the stdout of a rule script is its finding
   list. It also rejects `set -eou pipefail`, which is correct.
2. **`set` is the only prologue head the reader accepts.** 16 other prologue
   lines were run — `export LC_ALL=C`, `export PATH="$HOME/.local/bin:$PATH"`,
   `umask 022`, `trap 'exit 1' INT` and 12 more. All 16 run no tool and are
   correct, and the reader rejects all 16. A rule author who writes a `PATH`
   line above the guard breaks the coverage guard.

A quoted name, `set -o "pipefail"`, fails the same way. The scripts stand in
YAML front matter, where a quoted word is usual.

## Two smaller items

- The doc comment of `SHELL_LONG_OPTION_NAMES` states that `set -o rm` writes
  `set: rm: invalid option name` and exits 1. Measured two ways: `sh -c 'set
  -o rm'` exits 1; the same line as the first line of a SCRIPT exits 0, writes
  nothing to stdout, and the tool line does not run. The rules run a script.
  State the script measurement.
- `run_shell` in `crates/swissarmyhammer-validators/src/doctor.rs` calls
  `shell_command(Shell::Bash, script)`, and every measurement in the test file
  names `/bin/sh`. On this machine the two are both bash 3.2.57(1)-release and
  `set -o` gives the same 27 names for each, so 0 measurements change today.
  Name the shell the rules use.

The full finding text, with each measurement, stands in the comment
`01kzs0eyrenn7a0j2125m11jf5` of ^6585731.

#tool-validators #objectivity #tool-validators-objectivity