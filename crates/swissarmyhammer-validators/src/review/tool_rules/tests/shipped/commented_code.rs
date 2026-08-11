//! Acceptance tests for the shipped commented-code tool rule.
//!
//! The tool is sah itself, so the script builds a `--files` list out of its
//! arguments. A list `sah` cannot read is what this module measures.

use super::*;

/// The shipped commented-code tool rule, whose tool is the running sah
/// binary.
const COMMENTED_CODE_RULE: &str = "no-commented-code-parsed";

/// A Rust file that carries one commented-out block of 6 lines. The block
/// stands over the gate of 5 lines, it re-parses as 6 statements, and it
/// raises no error node, so the op reports one finding for each file.
const COMMENTED_CODE_UNREAD_SOURCE: &str = r#"fn live() -> i32 {
    1
}

// let first = compute(1);
// let second = compute(2);
// let third = compute(3);
// let fourth = compute(4);
// let fifth = compute(5);
// let sixth = compute(6);
"#;

/// Every Rust file staged in the probe repository the commented-code script
/// is given none of.
const COMMENTED_CODE_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.rs", COMMENTED_CODE_UNREAD_SOURCE),
    ("deep/nested/other.rs", COMMENTED_CODE_UNREAD_SOURCE),
];

/// Each finding the commented-code script reports over the two files it is
/// given, as `path:line`.
const COMMENTED_CODE_READ_FINDINGS: &[&str] = &["top.rs:5", "deep/nested/other.rs:5"];

/// The `no-commented-code-parsed` probe over a run that is given no file.
const COMMENTED_CODE_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: COMMENTED_CODE_RULE,
        expected: NO_FINDINGS,
    },
    staged: COMMENTED_CODE_UNREAD_FILES,
    with_files: COMMENTED_CODE_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped commented-code tool rule reads only the files it
/// is given, through the real sah pipeline.
///
/// `files` is a required parameter of the op, so an empty list is not a
/// smaller run: `sah` answers `missing required parameter 'files'` and exits
/// 2. Measured over this probe with no argument: without the guard the script
/// exited 2 and the engine reported a tool error; with the guard it reports
/// no finding and exits 0. The same script over the two staged files reports
/// 2.
#[test]
fn the_shipped_commented_code_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&COMMENTED_CODE_EMPTY_RUN_PROBE);
}
