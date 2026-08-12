---
assignees:
- claude-code
position_column: doing
position_ordinal: '8480'
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