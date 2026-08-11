//! Coverage guard: each shipped script that makes a temporary directory
//! names it `work`, removes it, and names `mktemp` for the doctor.
//!
//! The `run` key of `builtin/validators/README.md` states the shape word for
//! word: write `work="$(mktemp -d)"`, then `trap 'rm -rf "$work"' EXIT` under
//! it. A rule that makes a directory and arms no trap leaves one directory
//! behind for each run, and the acceptance test of that one rule cannot see
//! it, because the test reads the findings and not the temporary tree. A rule
//! added later carries the same defect, and every test of every other rule
//! stays green.
//!
//! This module reads the SHIPPED script of each rule instead, so the contract
//! is held for the rules that ship today and for the rules that ship next.

use super::*;

/// The one line the contract states a script makes its temporary directory
/// with.
const TEMP_DIRECTORY_ASSIGNMENT: &str = r#"work="$(mktemp -d)""#;

/// The line the contract states stands directly under the assignment.
const TEMP_DIRECTORY_TRAP: &str = r#"trap 'rm -rf "$work"' EXIT"#;

/// What a script writes to make a temporary directory.
const TEMP_DIRECTORY_COMMAND: &str = "mktemp -d";

/// The tool a script that makes a temporary directory needs, as
/// `doctor.check_command` names it.
const TEMP_DIRECTORY_TOOL: &str = "mktemp";

/// How many shipped rules make a temporary directory.
///
/// The count is the assertion that a rule added later reaches this guard. A
/// seventeenth such rule breaks it, and the author then reads the contract
/// before the rule ships.
const TEMP_DIRECTORY_RULE_COUNT: usize = 16;

/// What the rules of this roster have in common, for the failure message.
const TEMP_DIRECTORY_ROSTER: &str = "make a temporary directory";

/// Every shipped rule that makes a temporary directory, or a panic when the
/// set ships another number of them.
fn required_temp_directory_rules(loader: &ValidatorLoader) -> Vec<ShippedToolRule> {
    required_tool_rules(
        loader,
        TEMP_DIRECTORY_ROSTER,
        TEMP_DIRECTORY_RULE_COUNT,
        |rule| rule.script.contains(TEMP_DIRECTORY_COMMAND),
    )
}

/// Whether each `mktemp -d` of `script` stands in the assignment the contract
/// states.
fn names_the_directory_work(script: &str) -> bool {
    script
        .lines()
        .filter(|line| line.contains(TEMP_DIRECTORY_COMMAND))
        .all(|line| line.trim() == TEMP_DIRECTORY_ASSIGNMENT)
}

/// Whether the line directly under each temporary-directory assignment of
/// `script` is the trap that removes the directory.
///
/// `all` over no assignment gives true, which is the answer a script that
/// makes no temporary directory must give. The zero-argument guard reads
/// `any` over the same helpers for the opposite reason.
fn removes_the_directory_on_exit(script: &str) -> bool {
    let lines = trimmed_script_lines(script);
    script_lines_that_read(&lines, TEMP_DIRECTORY_ASSIGNMENT)
        .into_iter()
        .all(|at| script_lines_under(&lines, at, &[TEMP_DIRECTORY_TRAP]))
}

/// Whether `check_command` names the `mktemp` tool as a word of its own.
fn checks_for_mktemp(check_command: Option<&str>) -> bool {
    check_command.is_some_and(|command| {
        command
            .split_whitespace()
            .any(|word| word == TEMP_DIRECTORY_TOOL)
    })
}

/// Coverage: each shipped script that makes a temporary directory holds it in
/// `work`.
///
/// The README names the variable, so a script that names another one states a
/// second shape of the same contract, and a reader of two rules learns two
/// shapes. One name also lets the trap assertion below read the line under the
/// assignment rather than parse a name out of it.
#[test]
fn each_shipped_script_that_makes_a_temporary_directory_names_it_work() {
    let loader = builtin_loader();
    let rules = required_temp_directory_rules(&loader);

    let deviating = tool_rules_that_deviate(&rules, |rule| names_the_directory_work(&rule.script));

    assert!(
        deviating.is_empty(),
        "each `mktemp -d` must stand in `{TEMP_DIRECTORY_ASSIGNMENT}`, the line the \
         `run` key contract states; these rules write another line: {deviating:?}"
    );
}

/// Coverage: each shipped script that makes a temporary directory removes it.
///
/// The trap stands on the line directly under the assignment, so no statement
/// between the two can exit and leave the directory. The trap covers a clean
/// run, a run with findings and a broken run alike.
#[test]
fn each_shipped_script_that_makes_a_temporary_directory_removes_it() {
    let loader = builtin_loader();
    let rules = required_temp_directory_rules(&loader);

    let deviating =
        tool_rules_that_deviate(&rules, |rule| removes_the_directory_on_exit(&rule.script));

    assert!(
        deviating.is_empty(),
        "`{TEMP_DIRECTORY_TRAP}` must stand directly under each \
         `{TEMP_DIRECTORY_ASSIGNMENT}`; these rules leave the directory behind for \
         each run: {deviating:?}"
    );
}

/// Coverage: each shipped script that makes a temporary directory names
/// `mktemp` for the doctor.
///
/// `check_command` alone decides whether a rule is usable, and it names every
/// tool the script runs. A script that makes a temporary directory runs
/// `mktemp`, so a machine without `mktemp` breaks the run while the doctor
/// reports the rule ready.
#[test]
fn each_shipped_script_that_makes_a_temporary_directory_checks_for_mktemp() {
    let loader = builtin_loader();
    let rules = required_temp_directory_rules(&loader);

    let deviating = tool_rules_that_deviate(&rules, |rule| {
        checks_for_mktemp(rule.check_command.as_deref())
    });

    assert!(
        deviating.is_empty(),
        "`doctor.check_command` must name `{TEMP_DIRECTORY_TOOL}`, the tool the script \
         runs to make its directory; these rules leave it unnamed: {deviating:?}"
    );
}
