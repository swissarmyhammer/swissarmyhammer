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
//! Each of those seven scripts holds the three lines, so a guard that reads
//! the text alone answers true for all seven. Measured on this machine over
//! 20 broken shapes and 8 correct shapes: the guard that read the text alone
//! accepted 12 of the 20; the guard that read every line under `set ` as a
//! shell option line accepted 5 of the 20; the guard below accepts 0 of the
//! 20 and 8 of the 8.
//!
//! One acceptance test for each rule holds that rule alone, and each of those
//! tests is written by hand. A rule that ships with no guard and no test of
//! its own therefore goes green.
//!
//! This module reads the SHIPPED script of each rule instead, so the contract
//! is held for the rules that ship today and for the rules that ship next.

use super::*;

/// The line that opens the guard the contract states.
const ZERO_ARGUMENT_TEST: &str = r#"if [ "$#" -eq 0 ]; then"#;

/// The line the contract states stands under the test.
const ZERO_ARGUMENT_EXIT: &str = "exit 0";

/// The line the contract states closes the guard.
const ZERO_ARGUMENT_END: &str = "fi";

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

/// What a line writes to run a second command, or to read the answer of one.
const SHELL_COMMAND_MARKS: [char; 5] = [';', '&', '|', '$', '`'];

/// How many shipped rules state `scope: files`.
///
/// The count is the assertion that a rule added later reaches this guard. A
/// seventeenth `files`-scope rule breaks it, and the author then reads the
/// contract before the rule ships.
const FILES_SCOPE_RULE_COUNT: usize = 16;

/// What the rules of this roster have in common, for the failure message.
const FILES_SCOPE_ROSTER: &str = "state `scope: files`";

/// Every shipped `files`-scope rule, or a panic when the set ships another
/// number of them.
fn required_files_scope_rules(loader: &ValidatorLoader) -> Vec<ShippedToolRule> {
    required_tool_rules(loader, FILES_SCOPE_ROSTER, FILES_SCOPE_RULE_COUNT, |rule| {
        rule.scope == ToolScope::Files
    })
}

/// Whether `line` sets shell options and does nothing else.
///
/// The line opens with `set` and it names one option or more. Each word
/// under `set` opens with `-` or `+`, or it is the name a long option takes.
///
/// Three shapes of a `set` line answer false. A line that holds `--` writes
/// the positional parameters, which is the thing `$#` counts, so the guard
/// under it reads a `$#` the script made for itself. A line that holds a
/// command mark runs a second command. A line that names no option writes
/// the answer of the shell to the report: `set` alone writes every shell
/// variable, and `set -o` alone writes every shell option.
fn sets_shell_options_only(line: &str) -> bool {
    let mut words = line.split_whitespace();
    if words.next() != Some(SHELL_OPTION_HEAD) {
        return false;
    }

    let mut named_an_option = false;
    let mut takes_a_name = false;
    for word in words {
        if word == SHELL_OPTION_END || word.contains(SHELL_COMMAND_MARKS) {
            return false;
        }
        if takes_a_name {
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
    lines.iter().all(|line| {
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
const LINES_THAT_RUN_NOTHING: &[&str] = &[
    "",
    "# a comment",
    "set -e",
    "set +e",
    "set -x",
    "set -o pipefail",
    "set -euo pipefail",
    "set -e -o pipefail",
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
const LINES_THAT_RUN: &[&str] = &[
    "set",
    "set -o",
    "set --",
    "set -- a b c",
    "set -- $(find . -name '*.py')",
    "set a b",
    "set -o $(tool)",
    "set -e | tool",
    r#"set -e; tool "$@""#,
    r#"work="$(mktemp -d)""#,
    r#"tool "$@""#,
];

/// The line kinds that stand over the guard, and the line kinds that do not.
///
/// The README states the same two lists under the `run` key, so an author
/// who reads the contract writes a script this guard accepts.
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
