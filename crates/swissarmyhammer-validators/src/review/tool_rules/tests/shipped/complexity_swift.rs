//! Acceptance tests for the shipped `complexity-swift` tool rule.
//!
//! Each test drives the SHIPPED script over a probe Swift package and reads
//! what the real swiftlint reported.
//!
//! One module stands for each language of the family, because one file for
//! the whole family runs past the byte cap a review prompt holds.

use super::*;

/// The declarations every staged Swift position holds: one function whose
/// cyclomatic complexity is 16.
///
/// The gate the rule states is 15, and `cyclomatic_complexity` reports only
/// over the gate, so 16 `if` statements are one finding and no more. Measured:
/// swiftlint answers `Function should have complexity 15 or less; currently
/// complexity is 16`.
const SWIFT_COMPLEXITY_STAGED: &str = concat!(
    "func branchy(_ n: Int) -> Int {\n",
    "    var total = 0\n",
    "    if n > 1 { total += 1 }\n",
    "    if n > 2 { total += 1 }\n",
    "    if n > 3 { total += 1 }\n",
    "    if n > 4 { total += 1 }\n",
    "    if n > 5 { total += 1 }\n",
    "    if n > 6 { total += 1 }\n",
    "    if n > 7 { total += 1 }\n",
    "    if n > 8 { total += 1 }\n",
    "    if n > 9 { total += 1 }\n",
    "    if n > 10 { total += 1 }\n",
    "    if n > 11 { total += 1 }\n",
    "    if n > 12 { total += 1 }\n",
    "    if n > 13 { total += 1 }\n",
    "    if n > 14 { total += 1 }\n",
    "    if n > 15 { total += 1 }\n",
    "    if n > 16 { total += 1 }\n",
    "    return total\n",
    "}\n",
);

/// The file of the one finding the staged Swift positions must report.
const SWIFT_COMPLEXITY_STAGED_REPORTED: &[&str] = &[SWIFT_ORDINARY_POSITION.path];

/// The staged Swift positions, and the one of them the real swiftlint pipeline
/// must report.
const SWIFT_COMPLEXITY_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: SWIFT_COMPLEXITY_STAGED_REPORTED,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function over the complexity gate, staged in two positions",
    declarations: SWIFT_COMPLEXITY_STAGED,
    staged: SWIFT_EXCLUDE_POSITIONS,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the ordinary file reports its function, and the file under the project's \
             excluded directory reports nothing",
};

/// Acceptance: the shipped Swift complexity tool rule honours the project's
/// own `excluded:` list, through the real swiftlint pipeline.
///
/// Both prompt rules this rule supersedes carve out generated code, and
/// swiftlint holds no generated-code check of its own, so the project's
/// `excluded:` list is the whole carve-out for a generated file.
///
/// The two files hold the same bytes on purpose. The list is the only
/// difference between the file that reports and the file that stays silent.
#[test]
fn the_shipped_swift_complexity_tool_rule_reads_the_project_exclude_list() {
    verify_shipped_staged_positions_report(&SWIFT_COMPLEXITY_POSITIONS_PROBE);
}

/// The `complexity-swift` probe over a run whose every file the project's
/// `excluded:` list names.
const SWIFT_COMPLEXITY_EVERY_FILE_EXCLUDED_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: NO_STAGED_REPORTS,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function over the complexity gate under the project's excluded directory",
    declarations: SWIFT_COMPLEXITY_STAGED,
    staged: SWIFT_EXCLUDED_POSITION_ONLY,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the project excludes every file of the run, so the run reports nothing and \
             breaks nothing",
};

/// Acceptance: the shipped Swift complexity tool rule reports nothing, and
/// breaks nothing, when the project excludes every file of the run, through
/// the real swiftlint pipeline.
///
/// swiftlint exits 1 with `Error: No lintable files found at paths` when
/// `--force-exclude` leaves it no file to read, and that status reads as a
/// broken tool. The script tests each file it is given for readability first,
/// so the message can carry one cause only, and it then exits 0 with no
/// finding.
#[test]
fn the_shipped_swift_complexity_tool_rule_answers_zero_when_the_project_excludes_every_file() {
    verify_shipped_staged_positions_report(&SWIFT_COMPLEXITY_EVERY_FILE_EXCLUDED_PROBE);
}

/// The file of the one finding the `child_config:` probe must report.
///
/// The project excludes this directory, and the run drops that exclude list,
/// so the file reports.
const SWIFT_COMPLEXITY_CHILD_CONFIG_REPORTED: &[&str] = &[SWIFT_GENERATED_POSITION.path];

/// The `complexity-swift` probe beside a project that names a child
/// configuration of its own.
const SWIFT_COMPLEXITY_CHILD_CONFIG_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: SWIFT_COMPLEXITY_CHILD_CONFIG_REPORTED,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function over the complexity gate, beside a project child configuration",
    declarations: SWIFT_COMPLEXITY_STAGED,
    staged: SWIFT_EXCLUDED_POSITION_ONLY,
    support: SWIFT_CHILD_CONFIG_SUPPORT_FILES,
    reason: "swiftlint cannot read that project configuration beside the rule's own, so the run \
             measures with the rule's configuration alone and reports the staged function",
};

/// Acceptance: the shipped Swift complexity tool rule measures beside a
/// project that names a child configuration of its own, through the real
/// swiftlint pipeline.
///
/// swiftlint reads a list of `--config` paths as one parent-child hierarchy. A
/// parent that names a child of its own makes that hierarchy ambiguous, and
/// swiftlint aborts with exit 134. The script read that as a broken tool, so a
/// project switched the gate off with a configuration swiftlint reads on its
/// own.
///
/// The script now runs a second time with its own configuration alone. The
/// project's `excluded:` list is dropped for that run, so the staged file under
/// the excluded directory reports.
#[test]
fn the_shipped_swift_complexity_tool_rule_measures_beside_a_project_child_config() {
    verify_shipped_staged_positions_report(&SWIFT_COMPLEXITY_CHILD_CONFIG_PROBE);
}

/// A project `.swiftlint.yml` that switches both rules off and raises the
/// complexity gate.
///
/// Each of the two settings silences the staged function on its own:
/// `disabled_rules` switches `cyclomatic_complexity` off, and a `warning` of
/// 30 stands over the staged function's score of 16.
const SWIFT_COMPLEXITY_OVERRIDING_CONFIG: &str = concat!(
    "disabled_rules:\n",
    "  - cyclomatic_complexity\n",
    "  - function_body_length\n",
    "cyclomatic_complexity:\n",
    "  warning: 30\n",
);

/// The overriding project configuration staged beside the ordinary position,
/// which the work-list does NOT name.
const SWIFT_COMPLEXITY_OVERRIDING_SUPPORT: &[(&str, &str)] = &[(
    SWIFT_PROJECT_CONFIG_PATH,
    SWIFT_COMPLEXITY_OVERRIDING_CONFIG,
)];

/// The `complexity-swift` probe over a project that states another gate.
const SWIFT_COMPLEXITY_OPTIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: SWIFT_COMPLEXITY_STAGED_REPORTED,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function a project gate would let through",
    declarations: SWIFT_COMPLEXITY_STAGED,
    staged: SWIFT_ORDINARY_POSITION_ONLY,
    support: SWIFT_COMPLEXITY_OVERRIDING_SUPPORT,
    reason: "the rule's own gate decides, so the staged function still reports",
};

/// Acceptance: the shipped Swift complexity tool rule keeps its own gates
/// against a project that states other ones, through the real swiftlint
/// pipeline.
///
/// The script names the project's `.swiftlint.yml` as the PARENT of its own
/// configuration, so the project decides which files are read. It must not
/// decide what the rule measures. The script's own configuration states the
/// gate of each rule, and a child block replaces the parent's block whole.
#[test]
fn the_shipped_swift_complexity_tool_rule_keeps_its_own_gates() {
    verify_shipped_staged_positions_report(&SWIFT_COMPLEXITY_OPTIONS_PROBE);
}

/// The `complexity-swift` probe beside a project that names a swiftlint
/// version that is not installed.
const SWIFT_COMPLEXITY_VERSION_MISMATCH_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: SWIFT_VERSION_MISMATCH_ERROR,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function over the gate beside a project version mismatch",
    path: SWIFT_ORDINARY_POSITION.path,
    source: Some(SWIFT_COMPLEXITY_STAGED),
    support: SWIFT_VERSION_MISMATCH_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift complexity tool rule BREAKS beside a project
/// that names a swiftlint version that is not installed, through the real
/// swiftlint pipeline.
///
/// swiftlint compares `swiftlint_version:` with the version it is. At a
/// difference it writes one warning line to stderr, writes 0 bytes to stdout,
/// runs no lint, and exits 2. Measured with swiftlint 0.65.0 over the staged
/// function: a run with no project configuration reports 1 finding, and a run
/// beside `swiftlint_version: 99.0.0` reports 0. A script that reads every
/// status 2 as a measured run hands `jq` an empty report, reports 0 findings
/// and exits 0, so the engine reads a dirty file as clean. The script accepts
/// status 2 only when the report holds a JSON array of one entry or more.
#[test]
fn the_shipped_swift_complexity_tool_rule_breaks_beside_a_project_version_mismatch() {
    verify_shipped_run_breaks(&SWIFT_COMPLEXITY_VERSION_MISMATCH_PROBE);
}

/// Where the Swift file that is never written stands inside the probe
/// repository.
const SWIFT_COMPLEXITY_ABSENT_PATH: &str = "Sources/Absent.swift";

/// What the one error of an absent file must name.
const SWIFT_COMPLEXITY_ABSENT_ERROR: &[&str] =
    &["complexity-swift cannot read", SWIFT_COMPLEXITY_ABSENT_PATH];

/// The `complexity-swift` probe over a path that holds no file.
const SWIFT_COMPLEXITY_ABSENT_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: SWIFT_COMPLEXITY_ABSENT_ERROR,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "a Swift file that is not there",
    path: SWIFT_COMPLEXITY_ABSENT_PATH,
    source: None,
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift complexity tool rule BREAKS on a file it
/// cannot read, through the real swiftlint pipeline.
///
/// swiftlint exits 1 for a path that is not there and writes nothing to
/// stdout. A pipeline takes the exit status of its LAST command, and that
/// command was `jq`, so the earlier pipe exited 0 and reported nothing — a run
/// answering zero for a reason other than a clean file.
#[test]
fn the_shipped_swift_complexity_tool_rule_breaks_on_a_file_it_cannot_read() {
    verify_shipped_run_breaks(&SWIFT_COMPLEXITY_ABSENT_PROBE);
}

/// The `complexity-swift` probe over a directory that holds no Swift file.
///
/// The probe writes no file at the path, and the one staged file under that
/// path makes the directory.
const SWIFT_COMPLEXITY_HOLLOW_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: NO_FINDINGS,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: SWIFT_HOLLOW_PURPOSE,
    path: SWIFT_HOLLOW_PATH,
    source: None,
    support: SWIFT_HOLLOW_FILES,
};

/// Acceptance: the shipped Swift complexity tool rule answers CLEAN over a
/// directory that holds no Swift file, through the real swiftlint pipeline.
///
/// The `[ ! -r "$file" ]` guard tests each path for reading, and a directory
/// is readable, so the guard admits it and swiftlint reads it. Measured with
/// swiftlint 0.65.0 over such a directory: swiftlint writes 0 bytes to stdout,
/// writes `Error: No lintable files found at paths: ...` to stderr, and
/// exits 1. The script reads that stderr, reports no finding, and exits 0. A
/// guard that tested for a FILE would stop the directory instead, and the run
/// would answer one tool error over a path swiftlint reads without trouble.
#[test]
fn the_shipped_swift_complexity_tool_rule_stays_clean_over_a_hollow_directory() {
    verify_shipped_hollow_directory_answers_clean(&SWIFT_COMPLEXITY_HOLLOW_PROBE);
}

/// Every Swift file staged in the probe repository the complexity script is
/// given none of.
const SWIFT_COMPLEXITY_UNREAD_FILES: &[(&str, &str)] = &[
    ("Top.swift", SWIFT_COMPLEXITY_STAGED),
    ("deep/nested/Other.swift", SWIFT_COMPLEXITY_STAGED),
];

/// Each finding the Swift complexity script reports over the two files it is
/// given, as `path:line`.
const SWIFT_COMPLEXITY_READ_FINDINGS: &[&str] = &["Top.swift:1", "deep/nested/Other.swift:1"];

/// The `complexity-swift` probe over a run that is given no file.
const SWIFT_COMPLEXITY_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: NO_FINDINGS,
    },
    staged: SWIFT_COMPLEXITY_UNREAD_FILES,
    with_files: SWIFT_COMPLEXITY_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Swift complexity tool rule reads only the files it
/// is given, through the real swiftlint pipeline.
///
/// `swiftlint lint` with no path argument walks the whole tree under the
/// working directory, and it exits 0, so the answer reads as a measured result
/// rather than a mistake. The script answers an empty argument list at once,
/// with no finding and an exit status of 0. The same script over the two
/// staged files reports 2.
#[test]
fn the_shipped_swift_complexity_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&SWIFT_COMPLEXITY_EMPTY_RUN_PROBE);
}
