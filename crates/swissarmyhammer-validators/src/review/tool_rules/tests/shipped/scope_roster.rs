//! Coverage guard: each shipped rule that carries a `run` script stands in
//! one of the two scope rosters, and the two rosters together are the whole
//! set.
//!
//! `scope` states which inputs the script receives, and it takes one of two
//! values. `files` hands the script the changed files as its arguments.
//! `workspace` hands the script no argument at all: [`script_args`] answers
//! an empty list for that scope, and the engine keeps the findings in the
//! changed files after the run.
//!
//! One roster stands for each value, and one guard stands for each roster.
//! `zero_argument` pins the `files` roster and holds each script of it to the
//! three lines the README states. Before this module, nothing read the
//! `workspace` roster, and nothing stated that the two rosters together are
//! the whole set. A rule that shipped with a `tool` block and a
//! `scope: workspace` therefore reached no guard: the `files` roster kept its
//! count, `temp_directory` read the rule only when the script called
//! `mktemp -d`, and the acceptance test of that rule is written by hand or
//! absent.
//!
//! This module closes that gap with three assertions.
//!
//! 1. The two rosters together count every rule that carries a `run` script,
//!    so a rule outside both breaks a test rather than shipping unread.
//! 2. No `workspace` script holds the zero-argument guard. This states WHY
//!    the `workspace` roster stands outside `zero_argument` rather than
//!    leaving the reason unwritten. A `workspace` script reads `$#` as 0 on
//!    each run, so the guard exits the script before the tool starts, and the
//!    rule then reports nothing for any code.
//! 3. No `workspace` script reads the argument list. The engine fills that
//!    list for a `files` script alone, so the name reads as empty and the
//!    line it stands on runs over nothing.
//!
//! This module reads the SHIPPED script of each rule, so the contract is held
//! for the rules that ship today and for the rules that ship next.

use super::*;

use super::zero_argument::{
    required_files_scope_rules, script_holds_the_three_lines, FILES_SCOPE_RULE_COUNT,
    ZERO_ARGUMENT_END, ZERO_ARGUMENT_EXIT, ZERO_ARGUMENT_TEST,
};

/// How many shipped rules carry a `run` script.
///
/// Measured over `builtin/validators/*/rules/*.md`: 27 rules carry a `run`
/// script, 16 state `scope: files` and 11 state `scope: workspace`. The count
/// is the assertion that a rule added later reaches one of the two rosters. A
/// twenty-eighth rule breaks it, and the author then reads the contract of the
/// scope the rule states.
const SHIPPED_TOOL_RULE_COUNT: usize = 27;

/// How many shipped rules state `scope: workspace`.
///
/// The count is the assertion that a rule added later reaches this guard. A
/// twelfth `workspace`-scope rule breaks it, and the author then reads the
/// contract before the rule ships.
const WORKSPACE_SCOPE_RULE_COUNT: usize = 11;

/// What the rules of this roster have in common, for the failure message.
const WORKSPACE_SCOPE_ROSTER: &str = "state `scope: workspace`";

/// What a script writes to read the arguments the run gives it.
const ARGUMENT_LIST: &str = r#""$@""#;

/// Every shipped `workspace`-scope rule, or a panic when the set ships another
/// number of them.
fn required_workspace_scope_rules(loader: &ValidatorLoader) -> Vec<ShippedToolRule> {
    required_tool_rules(
        loader,
        WORKSPACE_SCOPE_ROSTER,
        WORKSPACE_SCOPE_RULE_COUNT,
        |rule| rule.scope == ToolScope::Workspace,
    )
}

/// The name of each rule of `rules`.
fn tool_rule_names(rules: &[ShippedToolRule]) -> Vec<&str> {
    rules.iter().map(|rule| rule.name.as_str()).collect()
}

/// Coverage: the two scope rosters together hold every shipped rule that
/// carries a `run` script.
///
/// A rule outside both rosters carries a script that no guard reads. The
/// three assertions below close each door into that gap. The first names any
/// rule the two rosters miss. The second holds the size of the whole set, so
/// a rule added with a scope neither roster keeps breaks a test. The third
/// holds the two roster sizes to that same total, so a rule cannot answer for
/// two rosters and hide a third one that is empty.
#[test]
fn the_two_scope_rosters_together_hold_every_rule_that_ships_a_run_script() {
    let loader = builtin_loader();
    let shipped = shipped_tool_rules(&loader);
    let files = required_files_scope_rules(&loader);
    let workspace = required_workspace_scope_rules(&loader);

    let held: Vec<&str> = tool_rule_names(&files)
        .into_iter()
        .chain(tool_rule_names(&workspace))
        .collect();

    let outside = tool_rules_that_deviate(&shipped, |rule| held.contains(&rule.name.as_str()));

    assert!(
        outside.is_empty(),
        "each shipped rule that carries a `run` script must stand in the \
         `files` roster or in the `workspace` roster; no guard reads the \
         script of these rules: {outside:?}"
    );

    assert_eq!(
        shipped.len(),
        SHIPPED_TOOL_RULE_COUNT,
        "the set must ship {SHIPPED_TOOL_RULE_COUNT} rules that carry a `run` \
         script; it ships {}",
        shipped.len()
    );

    assert_eq!(
        held.len(),
        SHIPPED_TOOL_RULE_COUNT,
        "the {FILES_SCOPE_RULE_COUNT} `files`-scope rules and the \
         {WORKSPACE_SCOPE_RULE_COUNT} `workspace`-scope rules must together \
         count the whole set, and no rule stands in both rosters"
    );
}

/// Coverage: no shipped `workspace`-scope script holds the zero-argument
/// guard.
///
/// This is the reason the `workspace` roster stands outside the guard of
/// `zero_argument`, written as an assertion. [`script_args`] answers an empty
/// list for `scope: workspace`, so such a script reads `$#` as 0 on each run.
/// The three lines in that script exit 0 before the tool starts, and the rule
/// then reports nothing for any code.
#[test]
fn no_shipped_workspace_scope_script_holds_the_zero_argument_guard() {
    let loader = builtin_loader();
    let rules = required_workspace_scope_rules(&loader);

    let deviating =
        tool_rules_that_deviate(&rules, |rule| !script_holds_the_three_lines(&rule.script));

    assert!(
        deviating.is_empty(),
        "`{ZERO_ARGUMENT_TEST}`, `{ZERO_ARGUMENT_EXIT}` and `{ZERO_ARGUMENT_END}` \
         belong to a `files`-scope script alone; a `workspace`-scope script takes \
         an empty argument list on each run, so the guard exits it before the tool \
         starts and the rule reports nothing for any code: {deviating:?}"
    );
}

/// Coverage: no shipped `workspace`-scope script reads the argument list.
///
/// [`script_args`] fills the list for a `files`-scope run alone. A
/// `workspace`-scope script that writes the name reads it as empty, so the
/// line it stands on runs over nothing, and a reader of the rule takes the
/// script for one that judges the changed files.
#[test]
fn no_shipped_workspace_scope_script_reads_the_argument_list() {
    let loader = builtin_loader();
    let rules = required_workspace_scope_rules(&loader);

    let deviating = tool_rules_that_deviate(&rules, |rule| !rule.script.contains(ARGUMENT_LIST));

    assert!(
        deviating.is_empty(),
        "`{ARGUMENT_LIST}` names the arguments a run gives a `files`-scope \
         script; a `workspace`-scope script takes an empty list, so the name \
         reads as empty and the line it stands on runs over nothing: \
         {deviating:?}"
    );
}
