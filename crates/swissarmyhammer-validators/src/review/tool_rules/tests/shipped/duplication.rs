//! Acceptance tests for the shipped duplication tool rule.
//!
//! The tool is sah itself, so the script builds a `--files` list out of its
//! arguments. A list `sah` cannot read is what this module measures.

use super::*;

/// The shipped duplication tool rule, whose tool is the running sah binary.
const DUPLICATION_RULE: &str = "duplication-parsed";

/// A Rust file holding two functions that normalize to one stream. Each
/// definition carries 73 normalized tokens, which stands over the gate of 40,
/// and the two are 100 percent alike.
const DUPLICATION_UNREAD_SOURCE: &str = r#"fn first(value: i32) -> i32 {
    let step0 = value + 0;
    let step1 = value + 1;
    let step2 = value + 2;
    let step3 = value + 3;
    let step4 = value + 4;
    let step5 = value + 5;
    let step6 = value + 6;
    let step7 = value + 7;
    let step8 = value + 8;
    let step9 = value + 9;
    value
}

fn second(value: i32) -> i32 {
    let step0 = value + 0;
    let step1 = value + 1;
    let step2 = value + 2;
    let step3 = value + 3;
    let step4 = value + 4;
    let step5 = value + 5;
    let step6 = value + 6;
    let step7 = value + 7;
    let step8 = value + 8;
    let step9 = value + 9;
    value
}
"#;

/// Every Rust file staged in the probe repository the duplication script is
/// given none of.
const DUPLICATION_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.rs", DUPLICATION_UNREAD_SOURCE),
    ("deep/nested/other.rs", DUPLICATION_UNREAD_SOURCE),
];

/// Each finding the duplication script reports over the two files it is
/// given, as `path:line`.
const DUPLICATION_READ_FINDINGS: &[&str] = &[
    "top.rs:15",
    "deep/nested/other.rs:1",
    "deep/nested/other.rs:15",
];

/// The `duplication-parsed` probe over a run that is given no file.
const DUPLICATION_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: DUPLICATION_RULE,
        expected: NO_FINDINGS,
    },
    staged: DUPLICATION_UNREAD_FILES,
    with_files: DUPLICATION_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped duplication tool rule reads only the files it is
/// given, through the real sah pipeline.
///
/// A pair is a fact about the files handed in, so `files` is a required
/// parameter of the op and an empty list reaches no smaller run: `sah`
/// answers `missing required parameter 'files'` and exits 2. Measured over
/// this probe with no argument: without the guard the script exited 2 and the
/// engine reported a tool error; with the guard it reports no finding and
/// exits 0. The same script over the two staged files reports 3, one for each
/// definition after the first.
#[test]
fn the_shipped_duplication_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&DUPLICATION_EMPTY_RUN_PROBE);
}
