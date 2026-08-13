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
//!
//! EVERY MEASUREMENT THIS MODULE STATES WAS TAKEN WITH THE SHELL THE RULES
//! RUN. [`crate::doctor::run_shell`] spawns `bash -c <script> bash
//! <argument>...`, so each shape below was run that way, with the bash of the
//! machine that wrote this file: 3.2.57(1)-release. The name of the shell is
//! part of the measurement. `/bin/sh` is the same binary here, and it reads a
//! script in POSIX mode, where a `set` line that names no shell option stops
//! the whole run: measured, `set -y` above the guard exits 2 under `sh`, and
//! under `bash` writes `set: -y: invalid option` to stderr and carries on.

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

/// What a shell option word opens with. `set -e` sets an option, and
/// `set +e` clears it. A mark with no letter under it is a word of its own
/// that sets nothing: measured, `set -` and `set +` each exit 0, write
/// nothing, and let the tool line run.
const SHELL_OPTION_MARKS: [char; 2] = ['-', '+'];

/// The word that ends the options of a `set` line. Each word under it
/// becomes a positional parameter, which is the thing `$#` counts.
const SHELL_OPTION_END: &str = "--";

/// The letter that takes the name of a long shell option out of the word
/// under it. `set -o pipefail` and `set -euo pipefail` each write the name of
/// the option as a word of its own, so the word under such an option is a
/// name and not an option.
///
/// The letter takes its name wherever it stands in the word. Measured:
/// `set -oe pipefail` sets pipefail and errexit, and `set -oo pipefail
/// errexit` sets pipefail and errexit as well, so a word holds one name for
/// each `o` it writes.
const SHELL_LONG_OPTION: char = 'o';

/// Every letter a short shell option word can hold beside
/// [`SHELL_LONG_OPTION`].
///
/// Measured over the 52 ASCII letters: `set -X` sets a shell option for 23 of
/// them, and writes `set: -X: invalid option` with a usage line to stderr for
/// the other 29. A letter outside this list therefore sets nothing, and the
/// script runs on with the option its author asked for unset.
///
/// Two of the 23 are not here. [`SHELL_LONG_OPTION`] is one, because it takes
/// a name rather than setting an option of its own. `n` is the other: it is
/// noexec, and measured over a run of 1 argument the shell then reads the
/// whole script and runs none of it, so the tool line never runs and the rule
/// answers every review with no finding.
const SHELL_SHORT_OPTION_LETTERS: [char; 21] = [
    'a', 'b', 'e', 'f', 'h', 'i', 'k', 'm', 'p', 'r', 't', 'u', 'v', 'x', 'B', 'C', 'E', 'H', 'I',
    'P', 'T',
];

/// The quotation marks a shell removes from a word before it reads the word.
///
/// The scripts stand in the YAML front matter of a rule file, where a quoted
/// word is usual. Measured: `set -o "pipefail"` and `set -o 'pipefail'` each
/// name the same option `set -o pipefail` names. A word that opens a
/// quotation and never closes it is a syntax error instead: measured,
/// `set -o "pipefail` above the guard makes the run exit 2, and the tool line
/// never runs.
const SHELL_QUOTES: [char; 2] = ['"', '\''];

/// The word each prologue line that is no `set` line opens with.
///
/// Each of the 10 names a shell builtin that starts no tool, makes no
/// directory, writes no positional parameter and exits nowhere. Measured over
/// the lines of [`LINES_THAT_RUN_NOTHING`] these words open: each run with no
/// argument exits 0 and writes nothing to stdout or stderr, and each run with
/// one argument reaches the tool line.
///
/// `shift` is not here, and it never can be: it rewrites the positional
/// parameters `$#` counts, so the guard under it reads a `$#` the script made
/// for itself.
const SHELL_SETUP_HEADS: [&str; 10] = [
    ":", "alias", "export", "hash", "readonly", "shopt", "trap", "true", "umask", "unset",
];

/// Every name a long shell option takes.
///
/// The list is the answer `set -o` gave under the bash the rules run with. A
/// word outside the list names no shell option, so [`SHELL_LONG_OPTION`] sets
/// nothing and the line does not do what its author wrote.
///
/// Measured as the FIRST LINE OF A SCRIPT, which is the shape the rules
/// write: `set -o rm` writes `bash: line 0: set: rm: invalid option name` to
/// stderr, writes nothing to stdout, sets no shell option, and lets every
/// line under it run, so the whole run exits 0. The `sh -c 'set -o rm'` form
/// exits 1 instead, and no rule runs that form.
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

/// What a word writes to run a second command, to read the answer of one, or
/// to read or write a file.
///
/// Measured: `set -e>gm.txt` exits 0 and cuts an 11-byte file to 0 bytes, and
/// `set -e<missing.txt` writes `bash: missing.txt: No such file or directory`
/// to stderr and leaves the `set` unrun.
const SHELL_COMMAND_MARKS: [char; 6] = [';', '&', '|', '`', '>', '<'];

/// What a word writes to run a command and read its answer.
///
/// `$` on its own is no mark. `$name` and `${name}` read a shell variable and
/// run nothing, and no word an expansion splits into can hold a mark of its
/// own, so `export PATH="$HOME/.local/bin:$PATH"` starts no command. `$(` is
/// the one spelling of `$` that starts one.
const SHELL_COMMAND_SUBSTITUTION: &str = "$(";

/// What a line writes to join the line under it.
///
/// Measured: `set -e\` above the guard joins the guard line to it, and the
/// run exits 2 with a syntax error. A backslash anywhere else quotes the
/// character under it and joins nothing, so `IFS=$'\n'` runs nothing at all.
const SHELL_LINE_JOIN: char = '\\';

/// How many shipped rules state `scope: files`.
///
/// The count is the assertion that a rule added later reaches this guard. A
/// fifteenth `files`-scope rule breaks it, and the author then reads the
/// contract before the rule ships.
pub(super) const FILES_SCOPE_RULE_COUNT: usize = 14;

/// What the rules of this roster have in common, for the failure message.
const FILES_SCOPE_ROSTER: &str = "state `scope: files`";

/// Every shipped `files`-scope rule, or a panic when the set ships another
/// number of them.
pub(super) fn required_files_scope_rules(loader: &ValidatorLoader) -> Vec<ShippedToolRule> {
    required_tool_rules(loader, FILES_SCOPE_ROSTER, FILES_SCOPE_RULE_COUNT, |rule| {
        rule.scope == ToolScope::Files
    })
}

/// The words of `line` the shell runs, with a trailing comment dropped.
///
/// A `#` word and every word under it are a comment, so `set -e # keep going`
/// sets a shell option and runs nothing. A blank line and a line that is a
/// comment whole give no word at all.
///
/// `split_whitespace` reads a line the same with or without an indent, so the
/// answer agrees with the shipped path, which trims every line before the
/// guard reads it.
fn words_that_run(line: &str) -> Vec<&str> {
    line.split_whitespace()
        .take_while(|word| !word.starts_with(SHELL_COMMENT))
        .collect()
}

/// Whether `word` runs a second command, reads the answer of one, or reads or
/// writes a file.
fn runs_a_command(word: &str) -> bool {
    word.contains(SHELL_COMMAND_MARKS) || word.contains(SHELL_COMMAND_SUBSTITUTION)
}

/// Whether the last word of `words` joins the line under it, so the two run
/// as one command.
fn joins_the_line_under_it(words: &[&str]) -> bool {
    words
        .last()
        .is_some_and(|word| word.ends_with(SHELL_LINE_JOIN))
}

/// `word` with the quotation marks the shell removes before it reads the
/// word, or `None` when the word opens a quotation it never closes.
fn shell_word_value(word: &str) -> Option<&str> {
    for quote in SHELL_QUOTES {
        if let Some(inner) = word.strip_prefix(quote) {
            return inner.strip_suffix(quote);
        }
    }
    Some(word)
}

/// Whether `letter` names a shell option a short option word can hold.
fn sets_a_shell_option(letter: char) -> bool {
    letter == SHELL_LONG_OPTION || SHELL_SHORT_OPTION_LETTERS.contains(&letter)
}

/// Whether `word` is the name a long shell option takes.
fn names_a_long_option(word: &str) -> bool {
    shell_word_value(word).is_some_and(|name| SHELL_LONG_OPTION_NAMES.contains(&name))
}

/// Whether `word` writes a shell variable, as `LC_ALL=C` does.
///
/// The name stands before the first `=`, and a shell reads a name of ASCII
/// letters, digits and `_` that opens with a letter or `_`. A word with no
/// `=`, and a word whose name is empty, writes no variable.
fn writes_a_variable(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut letters = name.chars();
    letters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && letters.all(|letter| letter.is_ascii_alphanumeric() || letter == '_')
}

/// Whether the words under `set` name shell options and nothing else.
///
/// Each word opens with `-` or `+` and holds shell option letters, or it is
/// the name a long option under it takes. [`SHELL_LONG_OPTION`] takes its
/// name out of the WORD UNDER the one that writes it, wherever the letter
/// stands in that word, so a word owes one name for each `o` it writes.
///
/// Four shapes answer false. A word that reads `--` opens the positional
/// parameters, which is the thing `$#` counts, so the guard under it reads a
/// `$#` the script made for itself. A word that holds a letter outside
/// [`SHELL_SHORT_OPTION_LETTERS`] sets nothing and writes a usage error. A
/// name outside [`SHELL_LONG_OPTION_NAMES`], and a name whose quotation never
/// closes, set nothing either. And a `set` line that writes the answer of the
/// shell to the report answers false as well: `set` with no word under it
/// writes every shell variable, and a line that owes a name and never takes
/// one writes every shell option.
fn sets_shell_options_only(words: &[&str]) -> bool {
    let mut wrote_an_option_word = false;
    let mut names_owed: usize = 0;

    for word in words {
        if names_owed > 0 {
            if !names_a_long_option(word) {
                return false;
            }
            names_owed -= 1;
            continue;
        }
        if *word == SHELL_OPTION_END {
            return false;
        }
        let Some(letters) = word.strip_prefix(SHELL_OPTION_MARKS) else {
            return false;
        };
        if !letters.chars().all(sets_a_shell_option) {
            return false;
        }
        names_owed += letters.matches(SHELL_LONG_OPTION).count();
        wrote_an_option_word = true;
    }

    wrote_an_option_word && names_owed == 0
}

/// Whether `line` starts no tool.
///
/// A blank line and a comment run nothing at all. A `set` line sets shell
/// options. Every other line that answers true opens with one of
/// [`SHELL_SETUP_HEADS`], or it writes shell variables and nothing else. None
/// of those starts a tool, makes a directory, writes a positional parameter,
/// or exits, so a guard under one of them still gives the first answer of the
/// script.
///
/// A line that holds a command mark answers false wherever the mark stands,
/// and so does a line that joins the line under it. A variable a line writes
/// IN FRONT OF a command runs that command, so every word of a line that
/// opens with an assignment must be an assignment too: `LC_ALL=C` runs
/// nothing and `LC_ALL=C tool "$@"` starts the tool.
///
/// The same test holds the guard at the TOP LEVEL of the script. A subshell
/// opens with `(`, the body of a function opens with a `name() {` line, and a
/// heredoc opens with a `<<` line. None of the three heads reaches this test,
/// so a guard under any of them answers false.
fn starts_no_tool(line: &str) -> bool {
    let words = words_that_run(line);
    let Some((head, arguments)) = words.split_first() else {
        return true;
    };
    if joins_the_line_under_it(&words) || words.iter().any(|word| runs_a_command(word)) {
        return false;
    }
    if *head == SHELL_OPTION_HEAD {
        return sets_shell_options_only(arguments);
    }
    if writes_a_variable(head) {
        return arguments.iter().all(|word| writes_a_variable(word));
    }
    SHELL_SETUP_HEADS.contains(head)
}

/// Whether every line of `lines` runs nothing.
fn nothing_runs_before(lines: &[&str]) -> bool {
    lines.iter().all(|line| starts_no_tool(line))
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
/// option and runs nothing. Measured: that line above the guard exits 0 and
/// the tool line never runs.
///
/// `set -` and `set +` write a mark with no letter under it. Measured: each
/// exits 0, writes nothing to stdout or stderr, and lets the tool line run.
///
/// A cluster takes one name for each `o` it writes, and it takes that name
/// out of the word under it wherever the `o` stands. Measured:
/// `set -oe pipefail` and `set -oo pipefail errexit` each set every option
/// they name. A quotation around a name is removed before the shell reads
/// the name, and the scripts stand in YAML front matter where a quoted word
/// is usual.
///
/// The 16 lines under the `set` lines are the other prologue a rule author
/// writes: a locale, a `PATH`, a `umask`, a `trap`. Measured, each of the 16
/// with no argument exits 0 and writes nothing to stdout or stderr, and each
/// with one argument reaches the tool line.
const LINES_THAT_RUN_NOTHING: &[&str] = &[
    "",
    "   ",
    "# a comment",
    "   # a comment under three spaces",
    "set -e",
    "set +e",
    "set -x",
    "set -",
    "set +",
    "set -o pipefail",
    "set -euo pipefail",
    "set -eou pipefail",
    "set -e -o pipefail",
    "set -oe pipefail",
    "set -oo pipefail errexit",
    "set -o pipefail -o errexit",
    r#"set -o "pipefail""#,
    "set -o 'pipefail'",
    r#"set -euo "pipefail""#,
    "set -e # keep going",
    "set -o pipefail # keep going",
    r#"set -o "pipefail" # keep going"#,
    "export LC_ALL=C",
    "export FOO=bar",
    r#"export PATH="$HOME/.local/bin:$PATH""#,
    "export LC_ALL=C LANG=C",
    "LC_ALL=C",
    r#"PATH="/usr/bin:$PATH""#,
    "readonly LIMIT=15",
    r#"IFS=$'\n'"#,
    "unset FOO",
    "umask 022",
    "shopt -s nullglob",
    "hash -r",
    "alias ll=ls",
    "trap 'exit 1' INT",
    ":",
    "true",
];

/// Lines the guard cannot stand under.
///
/// A line reaches this list for one of four measured reasons: it runs
/// something, it writes the positional parameters `$#` counts, it stops the
/// script from answering at all, or it sets none of the options it names.
///
/// `set` alone writes every shell variable, and `set -o` alone writes every
/// shell option. A cluster that writes `o` takes its name out of the word
/// under it wherever the `o` stands, so `set -oe`, `set -ox`, `set -eo` and
/// `set -oo pipefail` each fall one name short: measured, each writes the
/// 27-line shell option table to stdout, and the stdout of a rule script is
/// its finding list. `set -o -e` falls short the same way, because `-e` is
/// no name.
///
/// A `set --` line and a `set a b` line write the positional parameters, so
/// the guard under one of them reads a `$#` the script made for itself. A
/// `set` line that holds a command mark runs a second command, and the mark
/// reaches the name of a long option as well: `set -o $(tool)` runs a tool
/// for the name it gives `-o`.
///
/// A letter that names no shell option and a name outside
/// [`SHELL_LONG_OPTION_NAMES`] set nothing. Measured: `set -y` and `set -q`
/// each write `set: -X: invalid option` and a usage line to stderr, and
/// `set -o rm` writes `set: rm: invalid option name`. The run carries on with
/// the option its author asked for unset. `set -n` is noexec, and it is
/// worse: measured over a run of 1 argument, the shell reads the whole script
/// and runs none of it, so the tool line never runs and the rule answers
/// every review with no finding.
///
/// A quotation a word never closes is a syntax error. Measured:
/// `set -o "pipefail` above the guard makes the run exit 2, and the tool line
/// never runs.
///
/// A redirection mark and a line-continuation backslash break the run as
/// well. Measured: `set -e>out.txt` exits 0 and cuts an 11-byte file to 0
/// bytes; `set -e<in.txt` with the file absent writes
/// `bash: in.txt: No such file or directory` to stderr and leaves errexit
/// off, so the `set` never runs; a line that ends with a backslash joins the
/// next line, and the run exits 2 with a syntax error. `set -o >x` writes the
/// 27 shell options into the file `x`.
///
/// A variable a line writes in front of a command runs that command with the
/// variable set, so `LC_ALL=C tool "$@"` starts the tool. `shift` rewrites
/// the positional parameters `$#` counts.
///
/// A subshell opens with `(`, a function body opens with a `name() {` line,
/// and a heredoc opens with a `<<` line. Measured: the guard inside each of
/// the three lets the tool line run.
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
    "set -oe",
    "set -ox",
    "set -eo",
    "set -oo pipefail",
    "set -o -e",
    "set -y",
    "set -q",
    "set -n",
    r#"set -o "pipefail"#,
    "shift",
    r#"LC_ALL=C tool "$@""#,
    "export FOO=$(tool)",
    r#"umask 022; tool "$@""#,
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
/// Each shape was run with no argument. The guard under `mktemp -d` left 1
/// directory behind; the same script with the trap above the guard left 0.
/// The guard under the first tool call answered after `wc` read the file. The
/// guard under an earlier `exit 0` never ran, and the script reached no tool
/// for 1 argument. The guard in a subshell, the guard in a function body and
/// the guard in a heredoc each let the tool line run. The guard under
/// `set -- $(find . -name '*.sh')` read a `$#` the script wrote for itself:
/// the tool line ran over the files the script found, and the argument the
/// run gave it reached no tool. The guard under `LC_ALL=C tool "$@"` answered
/// after that tool ran, because a variable a line writes in front of a
/// command runs the command.
const SCRIPTS_THAT_MISS_THE_GUARD: &[&str] = &[
    r#"
      work="$(mktemp -d)"
      if [ "$#" -eq 0 ]; then
        exit 0
      fi
      tool "$@"
    "#,
    r#"
      export LC_ALL=C
      LC_ALL=C tool "$@"
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
/// With no argument, each exits 0 and the tool line never runs. With 1
/// argument, each reaches the tool line.
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
      #!/bin/bash
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
    r#"
      export LC_ALL=C
      export PATH="$HOME/.local/bin:$PATH"
      umask 022
      set -eou pipefail
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
const SCRIPTS_WITH_THE_TEXT_AND_THE_WRONG_PLACE: usize = 8;

/// The line kinds that stand over the guard, and the line kinds that do not.
///
/// `builtin/validators/README.md` states the RULE under the `run` key, and it
/// names some of these lines as examples. The two lists apply that rule to
/// each shape a shipped script can write, so a wrong answer breaks a test
/// here rather than a rule that ships.
#[test]
fn a_comment_a_blank_line_and_a_prologue_line_are_the_lines_that_run_nothing() {
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
            "`{line}` runs something, writes the positional parameters `$#` counts, \
             stops the script from answering, or sets none of the options it names, \
             so the guard under it is not the first answer the script gives"
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
/// which way from the rule. The one shape all 14 rules write is the shape
/// this guard reads. Measured over the 14 shipped scripts: 7 write the guard
/// on the first line, and 7 write it under `set -e` alone.
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
