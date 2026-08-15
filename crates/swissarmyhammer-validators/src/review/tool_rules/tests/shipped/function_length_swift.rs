//! Acceptance tests for the shipped `function-length-swift` tool rule.
//!
//! Each test drives the SHIPPED script over a probe Swift package and reads
//! what the real swiftlint reported.
//!
//! One module stands for each language of the family, because one file for
//! the whole family runs past the byte cap a review prompt holds.

use super::*;

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

/// A Swift `struct` named `name` holding one computed `var` whose body runs
/// [`LONG_BODY_LINES`] lines and holds no closure.
///
/// The body is a run of statements rather than a stack of view rows, so
/// neither the declaration gate nor the closure gate reaches it.
fn long_swift_computed_variable(name: &str) -> String {
    format!(
        "struct {name} {{\n    var total: Int {{\n{}    }}\n}}\n",
        LONG_BODY_LINE.repeat(LONG_BODY_LINES)
    )
}

/// The name the function every staged Swift position holds takes.
const SWIFT_STAGED_FUNCTION_NAME: &str = "staged";

/// The declarations every staged Swift position holds: one function whose body
/// runs [`LONG_BODY_LINES`] lines.
///
/// The gate the rule states is 250, and `function_body_length` reports only
/// over the gate, so 300 body lines are one finding and no more. Measured with
/// swiftlint 0.65.0: `Function body should span 250 lines or less excluding
/// comments and whitespace: currently spans 300 lines`.
///
/// The source is BUILT rather than written out, because 300 lines of Swift in
/// this file would run past the byte cap a review prompt holds. A `static`
/// carries it, so each probe below can name it as a `&'static str`.
static SWIFT_STAGED_SOURCE: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| long_swift_function("", SWIFT_STAGED_FUNCTION_NAME));

/// [`SWIFT_STAGED_SOURCE`] as the `&'static str` each probe field holds.
fn swift_staged_source() -> &'static str {
    SWIFT_STAGED_SOURCE.as_str()
}

/// Drives the shipped `function-length-swift` script over a probe repository
/// that holds `files`, and answers each finding it reported as `path:line`,
/// sorted.
///
/// The script is given every staged file. The findings are the SCRIPT's own,
/// before the engine keeps only the ones in the changed files.
fn swift_function_length_findings(files: &[(&str, &str)]) -> Vec<String> {
    let loader = builtin_loader();
    require_tool_installed(&loader, SWIFT_PROJECT_TYPES, SWIFT_FUNCTION_LENGTH_RULE);
    let paths: Vec<&str> = files.iter().map(|(path, _)| *path).collect();

    let reported = shipped_script_findings(&loader, SWIFT_FUNCTION_LENGTH_RULE, files, &paths)
        .expect("the shipped Swift function-length script must judge the probe files and exit 0");

    sorted_names(&reported)
}

/// The `path:line` entry [`shipped_script_findings`] answers a finding at row
/// `row` of `path` with.
fn swift_probe_row(path: &str, row: usize) -> String {
    format!("{path}:{row}")
}

/// The file of the one finding the staged Swift positions must report.
const SWIFT_LENGTH_STAGED_REPORTED: &[&str] = &[SWIFT_ORDINARY_POSITION.path];

/// The staged Swift positions, and the one of them the real swiftlint pipeline
/// must report.
fn swift_length_positions_probe() -> ShippedStagedPositions {
    ShippedStagedPositions {
        run: ShippedRun {
            project_types: SWIFT_PROJECT_TYPES,
            rule: SWIFT_FUNCTION_LENGTH_RULE,
            expected: SWIFT_LENGTH_STAGED_REPORTED,
        },
        prompt_rule: FUNCTION_LENGTH_PROMPT_RULE,
        change_purpose: "one function over the length gate, staged in two positions",
        declarations: swift_staged_source(),
        staged: SWIFT_EXCLUDE_POSITIONS,
        support: SWIFT_EXCLUDING_SUPPORT_FILES,
        reason: "the ordinary file reports its function, and the file under the project's \
                 excluded directory reports nothing",
    }
}

/// Acceptance: the shipped Swift function-length tool rule honours the
/// project's own `excluded:` list, through the real swiftlint pipeline.
///
/// `function-length`, the prompt rule this rule supersedes, carves out
/// generated code, and swiftlint holds no generated-code check of its own, so
/// the project's `excluded:` list is the whole carve-out for a generated file.
///
/// The two files hold the same bytes on purpose. The list is the only
/// difference between the file that reports and the file that stays silent.
#[test]
fn the_shipped_swift_function_length_tool_rule_reads_the_project_exclude_list() {
    verify_shipped_staged_positions_report(&swift_length_positions_probe());
}

/// The `function-length-swift` probe over a run whose every file the project's
/// `excluded:` list names.
fn swift_length_every_file_excluded_probe() -> ShippedStagedPositions {
    ShippedStagedPositions {
        run: ShippedRun {
            project_types: SWIFT_PROJECT_TYPES,
            rule: SWIFT_FUNCTION_LENGTH_RULE,
            expected: NO_STAGED_REPORTS,
        },
        prompt_rule: FUNCTION_LENGTH_PROMPT_RULE,
        change_purpose: "one function over the length gate under the project's excluded directory",
        declarations: swift_staged_source(),
        staged: SWIFT_EXCLUDED_POSITION_ONLY,
        support: SWIFT_EXCLUDING_SUPPORT_FILES,
        reason: "the project excludes every file of the run, so the run reports nothing and \
                 breaks nothing",
    }
}

/// Acceptance: the shipped Swift function-length tool rule reports nothing,
/// and breaks nothing, when the project excludes every file of the run,
/// through the real swiftlint pipeline.
///
/// swiftlint exits 1 with `Error: No lintable files found at paths` when
/// `--force-exclude` leaves it no file to read, and that status reads as a
/// broken tool. The script tests each file it is given for readability first,
/// so the message can carry one cause only, and it then exits 0 with no
/// finding.
#[test]
fn the_shipped_swift_function_length_tool_rule_answers_zero_when_the_project_excludes_every_file() {
    verify_shipped_staged_positions_report(&swift_length_every_file_excluded_probe());
}

/// The file of the one finding the `child_config:` probe must report.
///
/// The project excludes this directory, and the run drops that exclude list,
/// so the file reports.
const SWIFT_LENGTH_CHILD_CONFIG_REPORTED: &[&str] = &[SWIFT_GENERATED_POSITION.path];

/// The `function-length-swift` probe beside a project that names a child
/// configuration of its own.
fn swift_length_child_config_probe() -> ShippedStagedPositions {
    ShippedStagedPositions {
        run: ShippedRun {
            project_types: SWIFT_PROJECT_TYPES,
            rule: SWIFT_FUNCTION_LENGTH_RULE,
            expected: SWIFT_LENGTH_CHILD_CONFIG_REPORTED,
        },
        prompt_rule: FUNCTION_LENGTH_PROMPT_RULE,
        change_purpose: "one function over the length gate, beside a project child configuration",
        declarations: swift_staged_source(),
        staged: SWIFT_EXCLUDED_POSITION_ONLY,
        support: SWIFT_CHILD_CONFIG_SUPPORT_FILES,
        reason: "swiftlint cannot read that project configuration beside the rule's own, so the \
                 run measures with the rule's configuration alone and reports the staged function",
    }
}

/// Acceptance: the shipped Swift function-length tool rule measures beside a
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
fn the_shipped_swift_function_length_tool_rule_measures_beside_a_project_child_config() {
    verify_shipped_staged_positions_report(&swift_length_child_config_probe());
}

/// A project `.swiftlint.yml` that switches both rules off and raises the
/// declaration gate.
///
/// Each of the two settings silences the staged function on its own:
/// `disabled_rules` switches `function_body_length` off, and a `warning` of
/// 500 stands over the staged function's 300 body lines.
const SWIFT_LENGTH_OVERRIDING_CONFIG: &str = concat!(
    "disabled_rules:\n",
    "  - function_body_length\n",
    "  - closure_body_length\n",
    "function_body_length:\n",
    "  warning: 500\n",
);

/// The overriding project configuration staged beside the ordinary position,
/// which the work-list does NOT name.
const SWIFT_LENGTH_OVERRIDING_SUPPORT: &[(&str, &str)] =
    &[(SWIFT_PROJECT_CONFIG_PATH, SWIFT_LENGTH_OVERRIDING_CONFIG)];

/// The `function-length-swift` probe over a project that states another gate.
fn swift_length_options_probe() -> ShippedStagedPositions {
    ShippedStagedPositions {
        run: ShippedRun {
            project_types: SWIFT_PROJECT_TYPES,
            rule: SWIFT_FUNCTION_LENGTH_RULE,
            expected: SWIFT_LENGTH_STAGED_REPORTED,
        },
        prompt_rule: FUNCTION_LENGTH_PROMPT_RULE,
        change_purpose: "one function a project gate would let through",
        declarations: swift_staged_source(),
        staged: SWIFT_ORDINARY_POSITION_ONLY,
        support: SWIFT_LENGTH_OVERRIDING_SUPPORT,
        reason: "the rule's own gate decides, so the staged function still reports",
    }
}

/// Acceptance: the shipped Swift function-length tool rule keeps its own gates
/// against a project that states other ones, through the real swiftlint
/// pipeline.
///
/// The script names the project's `.swiftlint.yml` as the PARENT of its own
/// configuration, so the project decides which files are read. It must not
/// decide what the rule measures. The script's own configuration states the
/// gate of each rule, and a child block replaces the parent's block whole.
#[test]
fn the_shipped_swift_function_length_tool_rule_keeps_its_own_gates() {
    verify_shipped_staged_positions_report(&swift_length_options_probe());
}

/// The `function-length-swift` probe beside a project that names a swiftlint
/// version that is not installed.
fn swift_length_version_mismatch_probe() -> ShippedNamedPath {
    ShippedNamedPath {
        run: ShippedRun {
            project_types: SWIFT_PROJECT_TYPES,
            rule: SWIFT_FUNCTION_LENGTH_RULE,
            expected: SWIFT_VERSION_MISMATCH_ERROR,
        },
        prompt_rule: FUNCTION_LENGTH_PROMPT_RULE,
        change_purpose: "one function over the gate beside a project version mismatch",
        path: SWIFT_ORDINARY_POSITION.path,
        source: Some(swift_staged_source().as_bytes()),
        support: SWIFT_VERSION_MISMATCH_SUPPORT_FILES,
    }
}

/// Acceptance: the shipped Swift function-length tool rule BREAKS beside a
/// project that names a swiftlint version that is not installed, through the
/// real swiftlint pipeline.
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
fn the_shipped_swift_function_length_tool_rule_breaks_beside_a_project_version_mismatch() {
    verify_shipped_run_breaks(&swift_length_version_mismatch_probe());
}

/// Where the Swift file that is never written stands inside the probe
/// repository.
const SWIFT_LENGTH_ABSENT_PATH: &str = "Sources/Absent.swift";

/// What the one error of an absent file must name.
const SWIFT_LENGTH_ABSENT_ERROR: &[&str] = &[
    "function-length-swift cannot read",
    SWIFT_LENGTH_ABSENT_PATH,
];

/// The `function-length-swift` probe over a path that holds no file.
const SWIFT_LENGTH_ABSENT_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_FUNCTION_LENGTH_RULE,
        expected: SWIFT_LENGTH_ABSENT_ERROR,
    },
    prompt_rule: FUNCTION_LENGTH_PROMPT_RULE,
    change_purpose: "a Swift file that is not there",
    path: SWIFT_LENGTH_ABSENT_PATH,
    source: None,
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift function-length tool rule BREAKS on a file it
/// cannot read, through the real swiftlint pipeline.
///
/// swiftlint exits 1 for a path that is not there and writes nothing to
/// stdout. A pipeline takes the exit status of its LAST command, and that
/// command was `jq`, so the earlier pipe exited 0 and reported nothing — a run
/// answering zero for a reason other than a clean file.
#[test]
fn the_shipped_swift_function_length_tool_rule_breaks_on_a_file_it_cannot_read() {
    verify_shipped_run_breaks(&SWIFT_LENGTH_ABSENT_PROBE);
}

/// Where the Swift file swiftlint cannot decode stands inside the probe
/// repository.
const SWIFT_LENGTH_UNDECODABLE_PATH: &str = "Sources/Latin1.swift";

/// The head of the file swiftlint cannot decode, written in Latin-1 rather
/// than in UTF-8.
///
/// The byte `0xE9` is `é` in Latin-1, and it is not a UTF-8 sequence.
/// swiftlint reads a file as UTF-8 and nothing else, so it cannot decode a
/// file that holds this line.
const SWIFT_LATIN1_HEAD: &[u8] = b"let name = \"caf\xe9\"\n";

/// A Swift file written in Latin-1 rather than in UTF-8.
///
/// The staged function stands under the Latin-1 line, and it runs over the
/// length gate, so a run that DID read the file reports it. The bytes are
/// BUILT rather than written out, for the reason [`SWIFT_STAGED_SOURCE`]
/// states.
static SWIFT_UNDECODABLE_SOURCE: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
    let mut source = SWIFT_LATIN1_HEAD.to_vec();
    source.extend_from_slice(swift_staged_source().as_bytes());
    source
});

/// [`SWIFT_UNDECODABLE_SOURCE`] as the `&'static [u8]` the probe field holds.
fn swift_undecodable_source() -> &'static [u8] {
    &SWIFT_UNDECODABLE_SOURCE
}

/// What the one error of a file swiftlint cannot decode must name: the rule's
/// own line, and swiftlint's own message, which carries the path.
const SWIFT_LENGTH_UNDECODABLE_ERROR: &[&str] = &[
    "function-length-swift: swiftlint could not read the contents of a file this run names",
    "Could not read contents of",
    "Latin1.swift",
];

/// The `function-length-swift` probe over a Swift file swiftlint cannot decode.
fn swift_length_undecodable_probe() -> ShippedNamedPath {
    ShippedNamedPath {
        run: ShippedRun {
            project_types: SWIFT_PROJECT_TYPES,
            rule: SWIFT_FUNCTION_LENGTH_RULE,
            expected: SWIFT_LENGTH_UNDECODABLE_ERROR,
        },
        prompt_rule: FUNCTION_LENGTH_PROMPT_RULE,
        change_purpose: "a Swift file that is not UTF-8",
        path: SWIFT_LENGTH_UNDECODABLE_PATH,
        source: Some(swift_undecodable_source()),
        support: NO_SUPPORT_FILES,
    }
}

/// Acceptance: the shipped Swift function-length tool rule BREAKS on a Swift
/// file swiftlint cannot decode, through the real swiftlint pipeline.
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
fn the_shipped_swift_function_length_tool_rule_breaks_on_a_file_it_cannot_decode() {
    verify_shipped_run_breaks(&swift_length_undecodable_probe());
}

/// The `function-length-swift` probe over a file whose name holds the words of
/// swiftlint's decode message.
fn swift_length_decode_name_probe() -> ShippedStagedPositions {
    ShippedStagedPositions {
        run: ShippedRun {
            project_types: SWIFT_PROJECT_TYPES,
            rule: SWIFT_FUNCTION_LENGTH_RULE,
            expected: NO_STAGED_REPORTS,
        },
        prompt_rule: FUNCTION_LENGTH_PROMPT_RULE,
        change_purpose: "a file whose name holds the words of swiftlint's decode message",
        declarations: swift_staged_source(),
        staged: SWIFT_DECODE_NAME_POSITION_ONLY,
        support: SWIFT_EXCLUDING_SUPPORT_FILES,
        reason: "the project excludes the file, so the run reports nothing and breaks nothing, \
                 whatever the file is named",
    }
}

/// Acceptance: the shipped Swift function-length tool rule MEASURES a run over
/// a file whose name holds the words of swiftlint's decode message, through
/// the real swiftlint pipeline.
///
/// The script tests stderr for the message swiftlint writes when it cannot
/// decode a file. swiftlint writes the PATH of a file into stderr as well, so a
/// test that read all of stderr answered the file NAME.
///
/// Measured with swiftlint 0.65.0 over this probe: swiftlint writes
/// `Error: No lintable files found at paths: 'Generated/Could not read contents
/// of.swift'` to stderr, writes 0 bytes to stdout, and exits 1. A test spelled
/// `grep -qF 'Could not read contents of'` matched that path echo, and the
/// script then wrote its own tool-error line and exited 1 over a run that
/// measured correctly. The same run over `Generated/Staged.swift`, with the
/// same exclude list, reports no finding and exits 0.
///
/// swiftlint writes its own decode message at the START of a line, and it
/// writes the path echo after `Error: `. Measured, a pattern anchored on the
/// start of the line matches the decode message and does not match the path
/// echo, so the script anchors the test that way.
#[test]
fn the_shipped_swift_function_length_tool_rule_measures_a_file_named_for_the_decode_message() {
    verify_shipped_staged_positions_report(&swift_length_decode_name_probe());
}

/// The `function-length-swift` probe over a file whose name holds the words of
/// swiftlint's configuration message.
fn swift_length_config_name_probe() -> ShippedStagedPositions {
    ShippedStagedPositions {
        run: ShippedRun {
            project_types: SWIFT_PROJECT_TYPES,
            rule: SWIFT_FUNCTION_LENGTH_RULE,
            expected: NO_STAGED_REPORTS,
        },
        prompt_rule: FUNCTION_LENGTH_PROMPT_RULE,
        change_purpose: "a file whose name holds the words of swiftlint's configuration message",
        declarations: swift_staged_source(),
        staged: SWIFT_CONFIG_NAME_POSITION_ONLY,
        support: SWIFT_EXCLUDING_SUPPORT_FILES,
        reason: "the project configuration is readable, so the run keeps the project exclude \
                 list and reports nothing, whatever the file is named",
    }
}

/// Acceptance: the shipped Swift function-length tool rule MEASURES a run over
/// a file whose name holds the words of swiftlint's configuration message,
/// through the real swiftlint pipeline.
///
/// The same cause reaches the earlier stderr test, and it makes a WRONG FINDING
/// rather than a break. Measured with swiftlint 0.65.0 over this probe: a test
/// spelled `grep -qF 'Could not read configuration'` matched the path echo, so
/// the script wrote `swiftlint cannot read .swiftlint.yml beside this rule`,
/// ran swiftlint a second time with no project configuration, and reported 1
/// finding on a file the project excludes.
///
/// The project configuration of this probe is the one every Swift probe of this
/// module stages, and swiftlint reads it without trouble, so the run must keep
/// the project's `excluded:` list and report nothing.
#[test]
fn the_shipped_swift_function_length_tool_rule_measures_a_file_named_for_the_configuration_message()
{
    verify_shipped_staged_positions_report(&swift_length_config_name_probe());
}

/// The `function-length-swift` probe over a directory that holds no Swift file.
///
/// The probe writes no file at the path, and the one staged file under that
/// path makes the directory.
const SWIFT_LENGTH_HOLLOW_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_FUNCTION_LENGTH_RULE,
        expected: NO_FINDINGS,
    },
    prompt_rule: FUNCTION_LENGTH_PROMPT_RULE,
    change_purpose: SWIFT_HOLLOW_PURPOSE,
    path: SWIFT_HOLLOW_PATH,
    source: None,
    support: SWIFT_HOLLOW_FILES,
};

/// Acceptance: the shipped Swift function-length tool rule answers CLEAN over
/// a directory that holds no Swift file, through the real swiftlint pipeline.
///
/// The `[ ! -r "$file" ]` guard tests each path for reading, and a directory
/// is readable, so the guard admits it and swiftlint reads it. Measured with
/// swiftlint 0.65.0 over such a directory: swiftlint writes 0 bytes to stdout,
/// writes `Error: No lintable files found at paths: ...` to stderr, and
/// exits 1. The script reads that stderr, reports no finding, and exits 0. A
/// guard that tested for a FILE would stop the directory instead, and the run
/// would answer one tool error over a path swiftlint reads without trouble.
#[test]
fn the_shipped_swift_function_length_tool_rule_stays_clean_over_a_hollow_directory() {
    verify_shipped_hollow_directory_answers_clean(&SWIFT_LENGTH_HOLLOW_PROBE);
}

/// Where the first Swift file the script is given none of stands.
const SWIFT_UNREAD_TOP_PATH: &str = "Top.swift";

/// Where the second one stands, under a directory of its own.
const SWIFT_UNREAD_NESTED_PATH: &str = "deep/nested/Other.swift";

/// Every Swift file staged in the probe repository the length script is given
/// none of.
static SWIFT_UNREAD_FILES: std::sync::LazyLock<Vec<(&'static str, &'static str)>> =
    std::sync::LazyLock::new(|| {
        vec![
            (SWIFT_UNREAD_TOP_PATH, swift_staged_source()),
            (SWIFT_UNREAD_NESTED_PATH, swift_staged_source()),
        ]
    });

/// [`SWIFT_UNREAD_FILES`] as the `&'static [(&'static str, &'static str)]` the
/// probe field holds.
fn swift_unread_files() -> &'static [(&'static str, &'static str)] {
    &SWIFT_UNREAD_FILES
}

/// Each finding the Swift length script reports over the two files it is
/// given, as `path:line`.
///
/// Each file holds the staged function alone, and that function opens the file,
/// so each finding stands on row 1.
const SWIFT_LENGTH_READ_FINDINGS: &[&str] = &["Top.swift:1", "deep/nested/Other.swift:1"];

/// The `function-length-swift` probe over a run that is given no file.
fn swift_length_empty_run_probe() -> ShippedEmptyRun {
    ShippedEmptyRun {
        run: ShippedRun {
            project_types: SWIFT_PROJECT_TYPES,
            rule: SWIFT_FUNCTION_LENGTH_RULE,
            expected: NO_FINDINGS,
        },
        staged: swift_unread_files(),
        with_files: SWIFT_LENGTH_READ_FINDINGS,
        reason: READS_ONLY_ITS_ARGUMENTS,
    }
}

/// Acceptance: the shipped Swift function-length tool rule reads only the files
/// it is given, through the real swiftlint pipeline.
///
/// `swiftlint lint` with no path argument walks the whole tree under the
/// working directory, and it exits 0, so the answer reads as a measured result
/// rather than a mistake. The script answers an empty argument list at once,
/// with no finding and an exit status of 0. The same script over the two
/// staged files reports 2.
#[test]
fn the_shipped_swift_function_length_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&swift_length_empty_run_probe());
}

/// Where the probe test file stands inside the probe repository.
const SWIFT_TEST_FILE_PATH: &str = "Tests/StagedTests.swift";

/// What stands above the test method: the XCTest import, one blank line, and
/// the `XCTestCase` subclass line.
///
/// The subclass and the `test` name prefix are the XCTest convention at the
/// DEFINITION, which is the mark `function-length` states for its test
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
/// reports it at.
const SWIFT_TEST_FUNCTION_ROW: usize = SWIFT_TEST_CLASS_HEAD_LINES + 1;

/// The row the helper beside that test method stands on, which is the row
/// `function_body_length` reports it at.
const SWIFT_TEST_HELPER_ROW: usize =
    SWIFT_TEST_FUNCTION_ROW + LONG_BODY_LINES + SWIFT_TEST_CLASS_TAIL_LINES + 1;

/// Acceptance: the shipped Swift function-length tool rule REPORTS a test
/// method, and the helper beside it, through the real swiftlint pipeline.
///
/// `function-length`, the prompt rule this rule supersedes, exempts "Functions
/// explicitly marked as tests", and it names the DEFINITION as the mark: "A
/// complex helper named `build_request` in a file called `foo_test.rs` is
/// still a long function and is still listed."
///
/// `swiftlint rules function_body_length` and `swiftlint rules
/// closure_body_length` each name `warning` and `error`. No option of either
/// rule reads a declaration name or a superclass, so the run reproduces none of
/// that carve-out and the author answers it with the annotation.
///
/// The one alternative is the `excluded:` list, which reads the PATH. That
/// reads the file name, which is the mark the prompt rule forbids, and it
/// silences the helper beside the test as well.
///
/// Measured with swiftlint 0.65.0 over this probe: the 300-line
/// `func testEndToEnd()` and the 300-line `func buildRequest()` beside it each
/// report `Function body should span 250 lines or less excluding comments and
/// whitespace: currently spans 300 lines`.
#[test]
fn the_shipped_swift_function_length_tool_rule_reports_a_test_method_and_its_helper() {
    let source = format!(
        "{SWIFT_TEST_CLASS_HEAD}    func testEndToEnd() {{\n{}{SWIFT_TEST_CLASS_TAIL}{}",
        LONG_BODY_LINE.repeat(LONG_BODY_LINES),
        long_swift_function("", "buildRequest")
    );

    let reported = swift_function_length_findings(&[(SWIFT_TEST_FILE_PATH, &source)]);

    assert_eq!(
        reported,
        sorted_names(&[
            swift_probe_row(SWIFT_TEST_FILE_PATH, SWIFT_TEST_FUNCTION_ROW),
            swift_probe_row(SWIFT_TEST_FILE_PATH, SWIFT_TEST_HELPER_ROW),
        ]),
        "the gate does not read a test declaration, so the test method and the helper beside \
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

/// Acceptance: the shipped Swift function-length tool rule drops a long
/// initializer that carries the length-gate annotation, and keeps the bare one
/// beside it, through the real swiftlint pipeline.
///
/// `function-length`, the prompt rule this rule supersedes, exempts
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
fn the_shipped_swift_function_length_tool_rule_answers_the_length_gate_annotation() {
    let source = format!(
        "{}{}",
        long_swift_initializer("", "BareSettings"),
        long_swift_initializer(SWIFT_LENGTH_GATE_ANNOTATION, "AnnotatedSettings")
    );

    let reported = swift_function_length_findings(&[(SWIFT_INITIALIZER_FILE_PATH, &source)]);

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

/// The row the stack of the long SwiftUI view stands on, which is the row
/// `closure_body_length` reports.
///
/// The finding anchors on the opening line of the closure itself, and the
/// `VStack {` line is the last line of [`SWIFT_VIEW_HEAD`].
const SWIFT_VIEW_STACK_ROW: usize = SWIFT_VIEW_HEAD_LINES;

/// What stands above the rows of the short SwiftUI view, which follows the
/// long one in the same file: the `View` conformance, the `body` property, and
/// the stack that holds the rows.
const SWIFT_SHORT_VIEW_HEAD: &str = concat!(
    "struct Card: View {\n",
    "    var body: some View {\n",
    "        VStack {\n",
);

/// What stands under the rows of the short SwiftUI view: the closing brace of
/// the stack, of the property and of the struct.
const SWIFT_SHORT_VIEW_TAIL: &str = "        }\n    }\n}\n";

/// How many rows the short SwiftUI view holds.
///
/// The closure gate is 250, so a stack of 200 rows stays under it. The long
/// view beside it holds [`LONG_BODY_LINES`] rows and runs over.
const SHORT_CLOSURE_LINES: usize = 200;

/// Acceptance: the shipped Swift function-length tool rule reports a long
/// trailing closure, through the real swiftlint pipeline.
///
/// `function-length`, the prompt rule this rule supersedes, states "All
/// Function Types: Methods, closures, lambdas, standalone functions", and
/// `function_body_length` reads no closure at all. `closure_body_length` at
/// 250 is what carries that half of the prompt rule.
///
/// The gate does not fire on an idiomatic trailing closure. Measured with
/// swiftlint 0.65.0 over 894 `.swift` files — Alamofire, swift-nio and vapor at
/// HEAD — `closure_body_length` at 250 reports ONE closure, against 148 at
/// swiftlint's own default warning of 30.
///
/// Both views hold the same rows and differ only in how many, so the count is
/// the one difference between the view that reports and the view that stays
/// silent.
#[test]
fn the_shipped_swift_function_length_tool_rule_reports_a_long_trailing_closure() {
    let source = format!(
        "{SWIFT_VIEW_HEAD}{}{SWIFT_VIEW_TAIL}{SWIFT_SHORT_VIEW_HEAD}{}{SWIFT_SHORT_VIEW_TAIL}",
        SWIFT_VIEW_ROW_LINE.repeat(LONG_BODY_LINES),
        SWIFT_VIEW_ROW_LINE.repeat(SHORT_CLOSURE_LINES),
    );

    let reported = swift_function_length_findings(&[(SWIFT_VIEW_FILE_PATH, &source)]);

    assert_eq!(
        reported,
        sorted_names(&[swift_probe_row(SWIFT_VIEW_FILE_PATH, SWIFT_VIEW_STACK_ROW)]),
        "the closure gate reads the trailing closure of a SwiftUI body, so the long \
         stack reports and the short one beside it stays silent"
    );
}

/// Where the probe computed-variable file stands inside the probe repository.
const SWIFT_COMPUTED_VARIABLE_FILE_PATH: &str = "Sources/Totals.swift";

/// How many lines stand above the body of the computed variable: the `struct`
/// line and the `var` line.
const SWIFT_COMPUTED_VARIABLE_HEAD_LINES: usize = 2;

/// How many lines stand under that body: the closing brace of the variable and
/// the closing brace of the struct.
const SWIFT_COMPUTED_VARIABLE_TAIL_LINES: usize = 2;

/// The row the long function beside the computed variable stands on, which is
/// the one row the run reports.
const SWIFT_COMPUTED_VARIABLE_FUNCTION_ROW: usize =
    SWIFT_COMPUTED_VARIABLE_HEAD_LINES + LONG_BODY_LINES + SWIFT_COMPUTED_VARIABLE_TAIL_LINES + 1;

/// Acceptance: the shipped Swift function-length tool rule reads no computed
/// property body, through the real swiftlint pipeline.
///
/// `function_body_length` measures a `func`, an `init`, a `deinit`, a
/// `subscript` and an accessor of a subscript. It measures no computed
/// VARIABLE. Measured with swiftlint 0.65.0 over one body of 300 lines in each
/// shape: the `func`, the `init`, the `deinit`, the `subscript` and the
/// subscript `get` each reported; the computed `var`, the same `var` written
/// with an explicit `get`, and the `static var` each reported nothing.
///
/// `closure_body_length` reaches a closure INSIDE such a variable — the
/// acceptance test
/// `the_shipped_swift_function_length_tool_rule_reports_a_long_trailing_closure`
/// holds a SwiftUI `body` whose `VStack` reports. A body of straight
/// statements holds no closure, so it is the shape neither gate reaches.
/// Measured with swiftlint 0.65.0 over this probe: the computed variable of
/// 300 statement lines reports nothing.
///
/// The long function beside it holds the same 300 body lines, so the shape of
/// the declaration is the one difference between the two.
///
/// This test holds that gap measured rather than left to be discovered.
#[test]
fn the_shipped_swift_function_length_tool_rule_reads_no_computed_property_body() {
    let source = format!(
        "{}{}",
        long_swift_computed_variable("Totals"),
        long_swift_function("", "longFunction")
    );

    let reported = swift_function_length_findings(&[(SWIFT_COMPUTED_VARIABLE_FILE_PATH, &source)]);

    assert_eq!(
        reported,
        sorted_names(&[swift_probe_row(
            SWIFT_COMPUTED_VARIABLE_FILE_PATH,
            SWIFT_COMPUTED_VARIABLE_FUNCTION_ROW
        )]),
        "neither gate reads a computed property body that holds no closure, so the \
         computed variable stays silent and the function beside it reports"
    );
}
