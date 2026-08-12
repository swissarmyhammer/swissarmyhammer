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

/// One body line of a Swift declaration built to run over the length gate.
const LONG_BODY_LINE: &str = "    let _ = 1\n";

/// How many body lines carry a declaration over `function_body_length` at 250.
///
/// swiftlint counts the code lines of the body and leaves the signature line
/// out, so 300 body lines answer 300 against the gate of 250.
const LONG_BODY_LINES: usize = 300;

/// A Swift `func` named `name` whose body runs [`LONG_BODY_LINES`] lines, with
/// `head` written above its `func` line.
///
/// Every shape the length gate measures runs past 250 lines, so a probe of
/// that gate builds its source here rather than writing 300 lines out for each
/// declaration.
fn long_swift_function(head: &str, name: &str) -> String {
    format!(
        "{head}func {name}() {{\n{}}}\n",
        LONG_BODY_LINE.repeat(LONG_BODY_LINES)
    )
}

/// A Swift `struct` named `name` holding one `init` whose body runs
/// [`LONG_BODY_LINES`] lines, with `head` written above the `init` line.
///
/// `function-length` exempts "Initialization functions that set many fields",
/// so a probe of that carve-out measures an `init` rather than a `func`.
fn long_swift_initializer(head: &str, name: &str) -> String {
    format!(
        "struct {name} {{\n{head}    init() {{\n{}    }}\n}}\n",
        LONG_BODY_LINE.repeat(LONG_BODY_LINES)
    )
}

/// A Swift `func` named `name` whose cyclomatic complexity is 16, with `head`
/// written above its `func` line.
fn branchy_swift_function(head: &str, name: &str) -> String {
    let branches: String = (1..=SWIFT_STAGED_BRANCHES)
        .map(|step| format!("    if n > {step} {{ total += 1 }}\n"))
        .collect();
    format!("{head}func {name}(_ n: Int) -> Int {{\n    var total = 0\n{branches}    return total\n}}\n")
}

/// How many `if` statements carry [`branchy_swift_function`] over the
/// complexity gate of 15.
const SWIFT_STAGED_BRANCHES: usize = 16;

/// Drives the shipped `complexity-swift` script over a probe repository that
/// holds `files`, and answers each finding it reported as `path:line`, sorted.
///
/// The script is given every staged file. The findings are the SCRIPT's own,
/// before the engine keeps only the ones in the changed files.
fn swift_complexity_findings(files: &[(&str, &str)]) -> Vec<String> {
    let loader = builtin_loader();
    require_tool_installed(&loader, SWIFT_PROJECT_TYPES, SWIFT_COMPLEXITY_RULE);
    let paths: Vec<&str> = files.iter().map(|(path, _)| *path).collect();

    let reported = shipped_script_findings(&loader, SWIFT_COMPLEXITY_RULE, files, &paths)
        .expect("the shipped Swift complexity script must judge the probe files and exit 0");

    sorted_names(&reported)
}

/// The `path:line` entry [`shipped_script_findings`] answers a finding at row
/// `row` of `path` with.
fn swift_probe_row(path: &str, row: usize) -> String {
    format!("{path}:{row}")
}

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
    source: Some(SWIFT_COMPLEXITY_STAGED.as_bytes()),
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

/// Where the Swift file swiftlint cannot decode stands inside the probe
/// repository.
const SWIFT_COMPLEXITY_UNDECODABLE_PATH: &str = "Sources/Latin1.swift";

/// A Swift file written in Latin-1 rather than in UTF-8.
///
/// The byte `0xE9` is `é` in Latin-1, and it is not a UTF-8 sequence.
/// swiftlint reads a file as UTF-8 and nothing else, so it cannot decode this
/// one. The staged function stands under the string, and it holds one function
/// over the complexity gate, so a run that DID read the file reports it.
const SWIFT_COMPLEXITY_UNDECODABLE_SOURCE: &[u8] = b"let name = \"caf\xe9\"\n\
func branchy(_ n: Int) -> Int {\n\
    var total = 0\n\
    if n > 1 { total += 1 }\n\
    if n > 2 { total += 1 }\n\
    if n > 3 { total += 1 }\n\
    if n > 4 { total += 1 }\n\
    if n > 5 { total += 1 }\n\
    if n > 6 { total += 1 }\n\
    if n > 7 { total += 1 }\n\
    if n > 8 { total += 1 }\n\
    if n > 9 { total += 1 }\n\
    if n > 10 { total += 1 }\n\
    if n > 11 { total += 1 }\n\
    if n > 12 { total += 1 }\n\
    if n > 13 { total += 1 }\n\
    if n > 14 { total += 1 }\n\
    if n > 15 { total += 1 }\n\
    if n > 16 { total += 1 }\n\
    return total\n\
}\n";

/// What the one error of a file swiftlint cannot decode must name: the rule's
/// own line, and swiftlint's own message, which carries the path.
const SWIFT_COMPLEXITY_UNDECODABLE_ERROR: &[&str] = &[
    "complexity-swift: swiftlint could not read the contents of a file this run names",
    "Could not read contents of",
    "Latin1.swift",
];

/// The `complexity-swift` probe over a Swift file swiftlint cannot decode.
const SWIFT_COMPLEXITY_UNDECODABLE_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: SWIFT_COMPLEXITY_UNDECODABLE_ERROR,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "a Swift file that is not UTF-8",
    path: SWIFT_COMPLEXITY_UNDECODABLE_PATH,
    source: Some(SWIFT_COMPLEXITY_UNDECODABLE_SOURCE),
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift complexity tool rule BREAKS on a Swift file
/// swiftlint cannot decode, through the real swiftlint pipeline.
///
/// The file is readable, so the `[ ! -r "$file" ]` guard admits it and
/// swiftlint reads it. Measured with swiftlint 0.65.0 over this file:
/// swiftlint writes ``Could not read contents of `<path>` `` to stderr, writes
/// an empty JSON array to stdout, and exits 0 — the status and the report of a
/// clean file. So the script read a file swiftlint never read as a clean file,
/// and the one function over the gate reached the engine as a clean tree.
///
/// Measured over the same file beside one file that holds a finding: swiftlint
/// writes the same stderr line, writes the report of the other file, and exits
/// 2. So the shape reaches the script at status 0 and at status 2 alike, and
/// the script tests stderr rather than the status.
#[test]
fn the_shipped_swift_complexity_tool_rule_breaks_on_a_file_it_cannot_decode() {
    verify_shipped_run_breaks(&SWIFT_COMPLEXITY_UNDECODABLE_PROBE);
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

/// Where the probe test file stands inside the probe repository.
const SWIFT_TEST_FILE_PATH: &str = "Tests/StagedTests.swift";

/// What stands above the test method: the XCTest import, one blank line, and
/// the `XCTestCase` subclass line.
///
/// The subclass and the `test` name prefix are the XCTest convention at the
/// DEFINITION, which is the mark `cognitive-complexity` states for its test
/// carve-out.
const SWIFT_TEST_CLASS_HEAD: &str = "import XCTest\n\nfinal class StagedTests: XCTestCase {\n";

/// How many lines [`SWIFT_TEST_CLASS_HEAD`] runs.
const SWIFT_TEST_CLASS_HEAD_LINES: usize = 3;

/// What stands under the test method body: the closing brace of the method,
/// the closing brace of the class, and one blank line.
const SWIFT_TEST_CLASS_TAIL: &str = "    }\n}\n\n";

/// How many lines [`SWIFT_TEST_CLASS_TAIL`] runs.
const SWIFT_TEST_CLASS_TAIL_LINES: usize = 3;

/// The row the test method stands on, which is the row `function_body_length`
/// reports.
const SWIFT_TEST_FUNCTION_ROW: usize = SWIFT_TEST_CLASS_HEAD_LINES + 1;

/// The row the helper beside that test method stands on, which is the row
/// `cyclomatic_complexity` reports.
const SWIFT_TEST_HELPER_ROW: usize =
    SWIFT_TEST_FUNCTION_ROW + LONG_BODY_LINES + SWIFT_TEST_CLASS_TAIL_LINES + 1;

/// Acceptance: the shipped Swift complexity tool rule REPORTS a test method,
/// and the helper beside it, through the real swiftlint pipeline.
///
/// Both prompt rules this rule supersedes exempt a test. `function-length`
/// exempts "Functions explicitly marked as tests", and `cognitive-complexity`
/// names the DEFINITION as the mark: "A complex helper named `build_request`
/// in a file called `foo_test.rs` is still a complex function and is still
/// listed."
///
/// `swiftlint rules cyclomatic_complexity` names `warning`, `error` and
/// `ignores_case_statements`, and `swiftlint rules function_body_length` names
/// `warning` and `error`. No option of either rule reads a declaration name or
/// a superclass, so the run reproduces none of that carve-out and the author
/// answers it with the annotation.
///
/// The one alternative is the `excluded:` list, which reads the PATH. That
/// reads the file name, which is the mark the prompt rule forbids, and it
/// silences the helper beside the test as well — the trade `complexity-python`
/// refuses for a test path.
///
/// Measured with swiftlint 0.65.0 over this probe: the 300-line
/// `func testEndToEnd()` reports `Function body should span 250 lines or less
/// excluding comments and whitespace: currently spans 300 lines`, and the
/// helper reports `Function should have complexity 15 or less; currently
/// complexity is 16`.
#[test]
fn the_shipped_swift_complexity_tool_rule_reports_a_test_method_and_its_helper() {
    let source = format!(
        "{SWIFT_TEST_CLASS_HEAD}    func testEndToEnd() {{\n{}{SWIFT_TEST_CLASS_TAIL}{}",
        LONG_BODY_LINE.repeat(LONG_BODY_LINES),
        branchy_swift_function("", "buildRequest")
    );

    let reported = swift_complexity_findings(&[(SWIFT_TEST_FILE_PATH, &source)]);

    assert_eq!(
        reported,
        sorted_names(&[
            swift_probe_row(SWIFT_TEST_FILE_PATH, SWIFT_TEST_FUNCTION_ROW),
            swift_probe_row(SWIFT_TEST_FILE_PATH, SWIFT_TEST_HELPER_ROW),
        ]),
        "neither gate reads a test declaration, so the test method and the helper beside \
         it both report"
    );
}

/// Where the probe initializer file stands inside the probe repository.
const SWIFT_INITIALIZER_FILE_PATH: &str = "Sources/Settings.swift";

/// The annotation the rule states for a declaration the length gate reports.
///
/// It stands inside the `struct`, on the line directly above the `init` line.
const SWIFT_LENGTH_GATE_ANNOTATION: &str = "    // swiftlint:disable:next function_body_length\n";

/// The row the bare initializer stands on: the `struct` line opens the file,
/// and the `init` line stands under it.
const SWIFT_BARE_INITIALIZER_ROW: usize = 2;

/// Acceptance: the shipped Swift complexity tool rule drops a long initializer
/// that carries the length-gate annotation, and keeps the bare one beside it,
/// through the real swiftlint pipeline.
///
/// `function-length`, one of the two prompt rules this rule supersedes, exempts
/// "Initialization functions that set many fields" and "Functions that are
/// mostly configuration/data (e.g., builder patterns with many options)".
/// `function_body_length` counts a data line like a code line, and its whole
/// option list is `warning` and `error`, so the run reproduces neither
/// carve-out. Measured with swiftlint 0.65.0: an `init` of 260 body lines
/// reports `Initializer body should span 250 lines or less`; a builder chain of
/// 300 `.opt(n)` lines and a dictionary of 300 entries each report as well.
///
/// The annotation is the whole answer. Both structs hold the same 300 body
/// lines, so the annotation is the one difference between the initializer that
/// reports and the one that stays silent.
#[test]
fn the_shipped_swift_complexity_tool_rule_answers_the_length_gate_annotation() {
    let source = format!(
        "{}{}",
        long_swift_initializer("", "BareSettings"),
        long_swift_initializer(SWIFT_LENGTH_GATE_ANNOTATION, "AnnotatedSettings")
    );

    let reported = swift_complexity_findings(&[(SWIFT_INITIALIZER_FILE_PATH, &source)]);

    assert_eq!(
        reported,
        sorted_names(&[swift_probe_row(
            SWIFT_INITIALIZER_FILE_PATH,
            SWIFT_BARE_INITIALIZER_ROW
        )]),
        "the annotation is the author's answer to the initializer carve-out, so the \
         annotated initializer must stay silent and the bare one must report"
    );
}

/// Where the probe complexity file stands inside the probe repository.
const SWIFT_BRANCHY_FILE_PATH: &str = "Sources/Branchy.swift";

/// The annotation the rule states for a function the complexity gate reports.
const SWIFT_COMPLEXITY_GATE_ANNOTATION: &str = "// swiftlint:disable:next cyclomatic_complexity\n";

/// The row the bare branchy function stands on: it opens the probe file with
/// no head above it.
const SWIFT_BARE_BRANCHY_ROW: usize = 1;

/// Acceptance: the shipped Swift complexity tool rule drops a branchy function
/// that carries the complexity-gate annotation, and keeps the bare one beside
/// it, through the real swiftlint pipeline.
///
/// `cognitive-complexity` exempts "Configuration parsing with many options,
/// where the score comes from a long flat list of simple cases rather than from
/// nesting". `ignores_case_statements: true` reproduces that carve-out for a
/// `switch`, and nothing reproduces it for a flat `if` chain: measured, 16 flat
/// `if` statements score 16 against the gate of 15.
///
/// Both functions hold the same 16 `if` statements, so the annotation is the
/// one difference between the function that reports and the one that stays
/// silent.
#[test]
fn the_shipped_swift_complexity_tool_rule_answers_the_complexity_gate_annotation() {
    let source = format!(
        "{}{}",
        branchy_swift_function("", "bareBranchy"),
        branchy_swift_function(SWIFT_COMPLEXITY_GATE_ANNOTATION, "annotatedBranchy")
    );

    let reported = swift_complexity_findings(&[(SWIFT_BRANCHY_FILE_PATH, &source)]);

    assert_eq!(
        reported,
        sorted_names(&[swift_probe_row(
            SWIFT_BRANCHY_FILE_PATH,
            SWIFT_BARE_BRANCHY_ROW
        )]),
        "the annotation is the author's answer to the flat-list carve-out, so the \
         annotated function must stay silent and the bare one must report"
    );
}

/// Where the probe SwiftUI view stands inside the probe repository.
const SWIFT_VIEW_FILE_PATH: &str = "Sources/Panel.swift";

/// What stands above the rows of the SwiftUI view: the import, one blank line,
/// the `View` conformance, the `body` property, and the stack that holds the
/// rows.
const SWIFT_VIEW_HEAD: &str = concat!(
    "import SwiftUI\n",
    "\n",
    "struct Panel: View {\n",
    "    var body: some View {\n",
    "        VStack {\n",
);

/// How many lines [`SWIFT_VIEW_HEAD`] runs.
const SWIFT_VIEW_HEAD_LINES: usize = 5;

/// One row of the SwiftUI view body.
const SWIFT_VIEW_ROW_LINE: &str = "            Text(\"row\")\n";

/// What stands under the rows: the closing brace of the stack, of the property
/// and of the struct, and one blank line.
const SWIFT_VIEW_TAIL: &str = "        }\n    }\n}\n\n";

/// How many lines [`SWIFT_VIEW_TAIL`] runs.
const SWIFT_VIEW_TAIL_LINES: usize = 4;

/// The row the long function beside the view stands on, which is the one row
/// the run reports.
const SWIFT_VIEW_FUNCTION_ROW: usize =
    SWIFT_VIEW_HEAD_LINES + LONG_BODY_LINES + SWIFT_VIEW_TAIL_LINES + 1;

/// Acceptance: the shipped Swift complexity tool rule reads no computed
/// property body, through the real swiftlint pipeline.
///
/// `function_body_length` measures a `func`, an `init`, a `deinit`, a
/// `subscript` and an accessor of a subscript. It measures no computed
/// VARIABLE. Measured with swiftlint 0.65.0 over one body of 300 lines in each
/// shape: the `func`, the `init`, the `deinit`, the `subscript` and the
/// subscript `get` each reported; the computed `var`, the same `var` written
/// with an explicit `get`, the `static var` and the closure each reported
/// nothing.
///
/// A SwiftUI `body` is a computed variable, so a `body` of any length reaches
/// neither gate. The long function beside it holds the same 300 body lines, so
/// the shape of the declaration is the one difference between the two.
///
/// This test holds that gap measured rather than left to be discovered.
#[test]
fn the_shipped_swift_complexity_tool_rule_reads_no_computed_property_body() {
    let source = format!(
        "{SWIFT_VIEW_HEAD}{}{SWIFT_VIEW_TAIL}{}",
        SWIFT_VIEW_ROW_LINE.repeat(LONG_BODY_LINES),
        long_swift_function("", "longFunction")
    );

    let reported = swift_complexity_findings(&[(SWIFT_VIEW_FILE_PATH, &source)]);

    assert_eq!(
        reported,
        sorted_names(&[swift_probe_row(
            SWIFT_VIEW_FILE_PATH,
            SWIFT_VIEW_FUNCTION_ROW
        )]),
        "the length gate reads no computed property body, so the SwiftUI view stays \
         silent and the function beside it reports"
    );
}
