//! Coverage guard: each shipped `files`-scope script answers a run that gives
//! it no file at once, with no finding and an exit status of 0.
//!
//! The `run` key of `builtin/validators/README.md` states the three lines of
//! the guard word for word, and it states where they stand. A `files`-scope
//! script judges the files it takes as arguments. Given none, a script that
//! hands `"$@"` straight to its tool hands the tool an empty argument list,
//! and the tool then reads a default target of its own, refuses to start, or
//! breaks the run. The first shape is the worst of the three, because the
//! script exits 0 and the answer reads as a measured result.
//!
//! The PLACE is half of the contract. A guard under the first `mktemp -d`
//! leaves a directory behind. A guard under the first tool call answers after
//! the tool already read the whole tree. A guard under an earlier `exit 0`, a
//! guard in a subshell, a guard in the body of a function nothing calls, and
//! a guard inside a `<<'EOF'` heredoc each never run at all. A guard under
//! `set -- $(find . -name '*.py')` reads a `$#` the script wrote for itself.
//! Each of those scripts holds the three lines, so a guard that reads the
//! text alone answers true for each.
//!
//! Four lists in this file write that split out, and each list reaches an
//! assertion. `SCRIPTS_THAT_MISS_THE_GUARD` and `SCRIPTS_THAT_HOLD_THE_GUARD`
//! write whole scripts, and `the_guard_reads_where_the_three_lines_stand`
//! reads both. `LINES_THAT_RUN` and `LINES_THAT_RUN_NOTHING` write one line at
//! a time, and two tests read both. So each count this module states is read
//! off a list this file holds.
//!
//! One acceptance test for each rule holds that rule alone, and each of those
//! tests is written by hand. A rule that ships with no guard and no test of
//! its own therefore goes green.
//!
//! This module reads the SHIPPED script of each rule instead, so the contract
//! is held for the rules that ship today and for the rules that ship next.

use super::*;

/// The line that opens the guard the contract states.
pub(super) const ZERO_ARGUMENT_TEST: &str = r#"if [ "$#" -eq 0 ]; then"#;

/// The line the contract states stands under the test.
pub(super) const ZERO_ARGUMENT_EXIT: &str = "exit 0";

/// The line the contract states closes the guard.
pub(super) const ZERO_ARGUMENT_END: &str = "fi";

/// What a shell comment opens with.
const SHELL_COMMENT: &str = "#";

/// The word a shell option line opens with.
const SHELL_OPTION_HEAD: &str = "set";

/// What a shell option opens with. `set -e` sets an option, and `set +e`
/// clears it.
const SHELL_OPTION_MARKS: [char; 2] = ['-', '+'];

/// The word that ends the options of a `set` line. Each word under it
/// becomes a positional parameter, which is the thing `$#` counts.
const SHELL_OPTION_END: &str = "--";

/// The letter a long shell option ends with. `set -o pipefail` and
/// `set -euo pipefail` each write the name of the option as a word of its
/// own, so the word under such an option is a name and not an option.
const SHELL_LONG_OPTION: char = 'o';

/// Every name a long shell option takes.
///
/// The list is the answer `sh -c 'set -o'` gave on the machine that wrote
/// this file, where `/bin/sh` is bash 3.2.57(1)-release in `sh` mode. A word
/// outside the list names no shell option, so the shell reads it as an error
/// or as a positional parameter. Measured: `set -o rm` writes
/// "set: rm: invalid option name" and exits 1.
const SHELL_LONG_OPTION_NAMES: &[&str] = &[
    "allexport",
    "braceexpand",
    "emacs",
    "errexit",
    "errtrace",
    "functrace",
    "hashall",
    "histexpand",
    "history",
    "ignoreeof",
    "interactive-comments",
    "keyword",
    "monitor",
    "noclobber",
    "noexec",
    "noglob",
    "nolog",
    "notify",
    "nounset",
    "onecmd",
    "physical",
    "pipefail",
    "posix",
    "privileged",
    "verbose",
    "vi",
    "xtrace",
];

/// What a line writes to run a second command, to read the answer of one, to
/// read or write a file, or to join the line under it.
///
/// A mark with space around it meets the `-` or `+` test, so only a mark
/// glued to a word reaches this list. Measured in `/bin/sh`: `set -e>gm.txt`
/// exits 0 and cuts an 11-byte file to 0 bytes; `set -e<missing.txt` exits 1;
/// and `set -e\` joins the line under it, so the run exits 2 with a syntax
/// error.
const SHELL_COMMAND_MARKS: [char; 8] = [';', '&', '|', '$', '`', '>', '<', '\\'];

/// How many shipped rules state `scope: files`.
///
/// The count is the assertion that a rule added later reaches this guard. A
/// seventeenth `files`-scope rule breaks it, and the author then reads the
/// contract before the rule ships.
pub(super) const FILES_SCOPE_RULE_COUNT: usize = 16;

/// What the rules of this roster have in common, for the failure message.
const FILES_SCOPE_ROSTER: &str = "state `scope: files`";

/// Every shipped `files`-scope rule, or a panic when the set ships another
/// number of them.
pub(super) fn required_files_scope_rules(loader: &ValidatorLoader) -> Vec<ShippedToolRule> {
    required_tool_rules(loader, FILES_SCOPE_ROSTER, FILES_SCOPE_RULE_COUNT, |rule| {
        rule.scope == ToolScope::Files
    })
}

/// Whether `line` sets shell options and does nothing else.
///
/// The line opens with `set` and it names one option or more. Each word
/// under `set` opens with `-` or `+`, or it is the name a long option takes,
/// or it opens a comment. A `#` word and every word under it are a comment,
/// so `set -e # keep going` sets a shell option and runs nothing.
///
/// Four shapes of a `set` line answer false. A line that holds `--` writes
/// the positional parameters, which is the thing `$#` counts, so the guard
/// under it reads a `$#` the script made for itself. A line that holds a
/// command mark runs a second command, reads or writes a file, or joins the
/// line under it. A line whose long option takes a word that names no shell
/// option breaks, because the shell reads that word as an error or as a
/// positional parameter. A line that names no option writes the answer of the
/// shell to the report: `set` alone writes every shell variable, and `set -o`
/// alone writes every shell option.
fn sets_shell_options_only(line: &str) -> bool {
    let mut words = line.split_whitespace();
    if words.next() != Some(SHELL_OPTION_HEAD) {
        return false;
    }

    let mut named_an_option = false;
    let mut takes_a_name = false;
    for word in words {
        if word.starts_with(SHELL_COMMENT) {
            break;
        }
        if word == SHELL_OPTION_END || word.contains(SHELL_COMMAND_MARKS) {
            return false;
        }
        if takes_a_name {
            if !SHELL_LONG_OPTION_NAMES.contains(&word) {
                return false;
            }
            takes_a_name = false;
            continue;
        }
        if !word.starts_with(SHELL_OPTION_MARKS) {
            return false;
        }
        takes_a_name = word.ends_with(SHELL_LONG_OPTION);
        named_an_option = true;
    }

    named_an_option && !takes_a_name
}

/// Whether every line of `lines` runs nothing.
///
/// Each line is trimmed first, because `trimmed_script_lines` trims every
/// line before the shipped guard reads it. The two answers therefore agree
/// for a line a test writes with its own indent.
///
/// A blank line and a comment run nothing at all. A shell option line sets
/// shell options: it starts no tool, makes no directory, writes no
/// positional parameter, and exits nowhere. Every other line can run
/// something, so a guard under it is not the first answer the script gives.
///
/// The same test holds the guard at the TOP LEVEL of the script. A subshell
/// opens with `(`, the body of a function opens with a `name() {` line, and a
/// heredoc opens with a `<<` line. None of the three runs nothing, so a guard
/// under any of them answers false.
fn nothing_runs_before(lines: &[&str]) -> bool {
    lines.iter().map(|line| line.trim()).all(|line| {
        line.is_empty() || line.starts_with(SHELL_COMMENT) || sets_shell_options_only(line)
    })
}

/// Whether `script` holds the guard the contract states, where the contract
/// states it.
///
/// The three lines stand together, so no statement between them can run
/// before the script exits, and the guard cannot open a block that some other
/// line closes. Nothing that runs stands above them, so the guard answers
/// before the script makes a directory and before it starts a tool.
///
/// `any` over no guard gives false, which is the answer a script with no
/// guard must give. The temporary-directory guard reads `all` over the same
/// helpers for the opposite reason.
fn answers_a_run_with_no_file(script: &str) -> bool {
    let lines = trimmed_script_lines(script);
    script_lines_that_read(&lines, ZERO_ARGUMENT_TEST)
        .into_iter()
        .any(|at| {
            script_lines_under(&lines, at, &[ZERO_ARGUMENT_EXIT, ZERO_ARGUMENT_END])
                && nothing_runs_before(&lines[..at])
        })
}

/// Lines that run nothing, so the guard under one of them still gives the
/// first answer of the script.
///
/// A line of space alone and a comment under space are here because the
/// shipped path trims each line before the guard reads it, and this list
/// states the same contract for a line the test writes.
///
/// A `#` word closes a `set` line, so `set -e # keep going` sets a shell
/// option and runs nothing. Measured in `/bin/sh`: that line above the guard
/// exits 0 and the tool line never runs.
const LINES_THAT_RUN_NOTHING: &[&str] = &[
    "",
    "   ",
    "# a comment",
    "   # a comment under three spaces",
    "set -e",
    "set +e",
    "set -x",
    "set -o pipefail",
    "set -euo pipefail",
    "set -e -o pipefail",
    "set -e # keep going",
    "set -o pipefail # keep going",
];

/// Lines that run something, or that write the positional parameters `$#`
/// counts.
///
/// `set` alone writes every shell variable, and `set -o` alone writes every
/// shell option. A `set --` line and a `set a b` line write the positional
/// parameters, so the guard under one of them reads a `$#` the script made
/// for itself. A `set` line that holds a command mark runs a second command,
/// and the mark reaches the name of a long option as well: `set -o $(tool)`
/// runs a tool for the name it gives `-o`.
///
/// A redirection mark and a line-continuation backslash break the run as
/// well. Measured in `/bin/sh`: `set -e>out.txt` exits 0 and cuts an 11-byte
/// file to 0 bytes; `set -e<in.txt` exits 1 when the file is absent, and the
/// guard never runs; a line that ends with a backslash joins the next line,
/// and the run exits 2 with a syntax error. `set -o >x` writes the 27 shell
/// options into the file `x`, and `set -o rm` names no shell option and
/// exits 1.
///
/// A subshell opens with `(`, a function body opens with a `name() {` line,
/// and a heredoc opens with a `<<` line. Measured in `/bin/sh`: the guard
/// inside each of the three lets the tool line run.
const LINES_THAT_RUN: &[&str] = &[
    "set",
    "set -o",
    "set --",
    "set -- a b c",
    "set -- $(find . -name '*.py')",
    "set a b",
    "set -o $(tool)",
    "set -e | tool",
    "set -e & tool",
    "set -o `tool`",
    r#"set -e; tool "$@""#,
    "set -e>out.txt",
    "set -e<in.txt",
    "set -e\\",
    "set -o >x",
    "set -o pipefail>x",
    "set -o <x",
    "set -o pipefail\\",
    "set -x\\",
    "set -o rm",
    "(",
    "lint() {",
    "cat <<'EOF'",
    r#"work="$(mktemp -d)""#,
    r#"tool "$@""#,
];

/// Whole scripts that do not meet the contract.
///
/// Each script but the last writes the three lines of the guard in a PLACE
/// where the guard gives the wrong answer. The last script writes another
/// shape, which the README names as the counter-example. That shape gives
/// the correct answer in the shell, and the contract still rejects it,
/// because a guard the contract cannot read is a guard the coverage test
/// cannot hold.
///
/// Each shape was run in `/bin/sh` with no argument. The guard under
/// `mktemp -d` left 1 directory behind; the same script with the trap above
/// the guard left 0. The guard under the first tool call answered after `wc`
/// read the file. The guard under an earlier `exit 0` never ran, and the
/// script reached no tool for 1 argument. The guard in a subshell, the guard
/// in a function body and the guard in a heredoc each let the tool line run.
/// The guard under `set -- $(find . -name '*.sh')` read a `$#` the script
/// wrote for itself: the tool line ran over the files the script found, and
/// the argument the run gave it reached no tool.
const SCRIPTS_THAT_MISS_THE_GUARD: &[&str] = &[
    r#"
      work="$(mktemp -d)"
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      tool "$@"
    "#,
    r#"
      wc -l seen.txt
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      tool "$@"
    "#,
    r#"
      exit 0
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      tool "$@"
    "#,
    r#"
      (
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      )
      tool "$@"
    "#,
    r#"
      guard() {
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      }
      tool "$@"
    "#,
    r#"
      cat <<'EOF'
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      EOF
      tool "$@"
    "#,
    r#"
      set -- $(find . -name '*.sh')
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      tool "$@"
    "#,
    r#"
      set -e
      [ "$#" -eq 0 ] && exit 0
      tool "$@"
    "#,
];

/// Whole scripts that meet the contract.
///
/// Each was run in `/bin/sh`. With no argument, each exits 0 and the tool
/// line never runs. With 1 argument, each reaches the tool line.
const SCRIPTS_THAT_HOLD_THE_GUARD: &[&str] = &[
    r#"
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      tool "$@"
    "#,
    r#"
      set -e
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      tool "$@"
    "#,
    r#"
      #!/bin/sh
      # read the files the review gives

      set -euo pipefail
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      tool "$@"
    "#,
    r#"
      set -e # keep going
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      tool "$@"
    "#,
];

/// How many scripts of `SCRIPTS_THAT_MISS_THE_GUARD` hold the three lines of
/// the guard word for word.
///
/// A guard that reads the TEXT alone accepts every one of them, because the
/// text is correct in each and the PLACE is not. The last script of the list
/// writes another shape, so the text test rejects that one as well.
const SCRIPTS_WITH_THE_TEXT_AND_THE_WRONG_PLACE: usize = 7;

/// The line kinds that stand over the guard, and the line kinds that do not.
///
/// `builtin/validators/README.md` states the RULE under the `run` key, and it
/// names some of these lines as examples. The two lists apply that rule to
/// each shape a shipped script can write, so a wrong answer breaks a test
/// here rather than a rule that ships.
#[test]
fn a_comment_a_blank_line_and_a_shell_option_line_are_the_lines_that_run_nothing() {
    for line in LINES_THAT_RUN_NOTHING {
        assert!(
            nothing_runs_before(&[line]),
            "`{line}` runs nothing, so the guard under it gives the first answer of \
             the script"
        );
    }

    for line in LINES_THAT_RUN {
        assert!(
            !nothing_runs_before(&[line]),
            "`{line}` runs something, or it writes the positional parameters `$#` \
             counts, so the guard under it answers too late"
        );
    }
}

/// One line that runs decides a whole prefix, wherever it stands in it.
///
/// The test holds the combinator of `nothing_runs_before`. `all` over a
/// prefix that holds one line that runs gives false, and `any` over the same
/// prefix gives true, so a run that answers true here reads the wrong
/// combinator.
#[test]
fn a_prefix_that_holds_one_line_that_runs_answers_false() {
    assert!(
        nothing_runs_before(LINES_THAT_RUN_NOTHING),
        "every line of `LINES_THAT_RUN_NOTHING` runs nothing, so the whole list \
         runs nothing"
    );

    for runs in LINES_THAT_RUN {
        for quiet in LINES_THAT_RUN_NOTHING {
            assert!(
                !nothing_runs_before(&[quiet, runs]),
                "`{runs}` runs, so the prefix `{quiet}` then `{runs}` runs"
            );
            assert!(
                !nothing_runs_before(&[runs, quiet]),
                "`{runs}` runs, so the prefix `{runs}` then `{quiet}` runs"
            );
        }
    }
}

/// The guard reads the PLACE of the three lines, and not the text alone.
#[test]
fn the_guard_reads_where_the_three_lines_stand() {
    for script in SCRIPTS_THAT_MISS_THE_GUARD {
        assert!(
            !answers_a_run_with_no_file(script),
            "this script answers a run with no file too late, or not at all: \
             {script}"
        );
    }

    for script in SCRIPTS_THAT_HOLD_THE_GUARD {
        assert!(
            answers_a_run_with_no_file(script),
            "this script writes the three lines above every line that runs: \
             {script}"
        );
    }

    let with_the_text = SCRIPTS_THAT_MISS_THE_GUARD
        .iter()
        .filter(|script| script_holds_the_three_lines(script))
        .count();

    assert_eq!(
        with_the_text, SCRIPTS_WITH_THE_TEXT_AND_THE_WRONG_PLACE,
        "a guard that reads the text alone accepts each script that holds the \
         three lines, so the count states how much the PLACE test is worth"
    );
}

/// Whether `script` writes the three lines of the guard, wherever they stand.
pub(super) fn script_holds_the_three_lines(script: &str) -> bool {
    let lines = trimmed_script_lines(script);
    [ZERO_ARGUMENT_TEST, ZERO_ARGUMENT_EXIT, ZERO_ARGUMENT_END]
        .iter()
        .all(|text| lines.contains(text))
}

/// Coverage: each shipped `files`-scope script answers a run that gives it no
/// file with no finding and an exit status of 0.
///
/// The guard stands on the script rather than on the tool, because each tool
/// answers an empty argument list its own way and a rule author cannot see
/// which way from the rule. The one shape all 16 rules write is the shape
/// this guard reads. Measured over the 16 shipped scripts: 11 write the guard
/// on the first line, and 5 write it under `set -e` alone.
#[test]
fn each_shipped_files_scope_script_answers_a_run_that_gives_it_no_file() {
    let loader = builtin_loader();
    let rules = required_files_scope_rules(&loader);

    let deviating =
        tool_rules_that_deviate(&rules, |rule| answers_a_run_with_no_file(&rule.script));

    assert!(
        deviating.is_empty(),
        "`{ZERO_ARGUMENT_TEST}`, `{ZERO_ARGUMENT_EXIT}` and `{ZERO_ARGUMENT_END}` must \
         stand together in each `files`-scope script, above every line that runs; \
         these rules answer for files the review never gave them, or answer too \
         late: {deviating:?}"
    );
}
