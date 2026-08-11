//! Coverage guard: each shipped `files`-scope script answers a run that gives
//! it no file at once, with no finding and an exit status of 0.
//!
//! The `run` key of `builtin/validators/README.md` states the shape word for
//! word. A `files`-scope script judges the files it takes as arguments. Given
//! none, a script that hands `"$@"` straight to its tool hands the tool an
//! empty argument list, and the tool then reads a default target of its own,
//! refuses to start, or breaks the run. The first shape is the worst of the
//! three, because the script exits 0 and the answer reads as a measured
//! result.
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

/// Whether `script` holds the guard the contract states.
///
/// The three lines stand together, so no statement between them can run
/// before the script exits, and the guard cannot open a block that some other
/// line closes.
fn answers_a_run_with_no_file(script: &str) -> bool {
    let lines: Vec<&str> = script.lines().map(str::trim).collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == ZERO_ARGUMENT_TEST)
        .any(|(at, _)| {
            lines.get(at + 1) == Some(&ZERO_ARGUMENT_EXIT)
                && lines.get(at + 2) == Some(&ZERO_ARGUMENT_END)
        })
}

/// Coverage: each shipped `files`-scope script answers a run that gives it no
/// file with no finding and an exit status of 0.
///
/// The guard stands on the script rather than on the tool, because each tool
/// answers an empty argument list its own way and a rule author cannot see
/// which way from the rule. The one shape all 16 rules write is the shape
/// this guard reads.
#[test]
fn each_shipped_files_scope_script_answers_a_run_that_gives_it_no_file() {
    let loader = builtin_loader();
    let rules = required_files_scope_rules(&loader);

    let deviating =
        tool_rules_that_deviate(&rules, |rule| answers_a_run_with_no_file(&rule.script));

    assert!(
        deviating.is_empty(),
        "`{ZERO_ARGUMENT_TEST}`, `{ZERO_ARGUMENT_EXIT}` and `{ZERO_ARGUMENT_END}` must \
         stand together in each `files`-scope script; these rules answer for files the \
         review never gave them: {deviating:?}"
    );
}
