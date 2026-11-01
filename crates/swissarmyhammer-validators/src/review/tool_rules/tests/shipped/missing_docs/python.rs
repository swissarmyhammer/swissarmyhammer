//! Acceptance tests for the shipped Python missing-docs tool rule.

use super::*;

/// The row ruff stands a whole-module finding on: the first line of the file.
///
/// `D100` names the module itself, which carries no declaration line of its
/// own, so ruff reports it at the head of the file rather than under a
/// definition. That is a different reason from
/// [`opening_declaration_row`], although both name the same line.
///
/// It stands in a macro for the reason [`opening_declaration_row`] states.
macro_rules! python_module_row {
    () => {
        "1"
    };
}

/// The materialized name of the `missing-docs-python` fail fixture.
const PYTHON_MISSING_DOCS_FAIL_FIXTURE: &str = concat!(missing_docs_rule!(python), ".fail.py");

/// Where the fail fixture stands inside the probe repository, as the work-list
/// holds it.
const PYTHON_MISSING_DOCS_FIXTURE_PATH: &str = "src/missing_docs_python_fail.py";

/// The definition line of each item the `missing-docs-python` fail fixture
/// leaves undocumented.
///
/// One entry for each of the five codes a loose file can hold — `D101`, `D106`,
/// `D107`, `D102` and `D103` — and one more `D102` for the property getter.
/// `D100` and `D104` stand outside the fixture: the doctor materializes one
/// loose file that carries a docstring of its own, and it cannot take the name
/// `__init__.py`.
///
/// The getter is the entry the rule body owes a measurement. ruff carves out no
/// getter and the prompt rule does, so this entry holds ruff to reporting it.
const PYTHON_MISSING_DOCS_FAIL_ITEMS: &[&str] = &[
    "class UndocumentedClass:",
    "class UndocumentedNested:",
    "def __init__(self, name: str) -> None:",
    "def name(self) -> str:",
    "def undocumented_method(self) -> None:",
    "def undocumented_function() -> None:",
];

/// The `missing-docs-python` fail fixture, and every undocumented item the real
/// ruff pipeline must report inside it.
const PYTHON_MISSING_DOCS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_MISSING_DOCS_RULE,
        expected: PYTHON_MISSING_DOCS_FAIL_ITEMS,
    },
    fixture: PYTHON_MISSING_DOCS_FAIL_FIXTURE,
    path: PYTHON_MISSING_DOCS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "undocumented item",
};

/// Acceptance: the shipped Python missing-docs tool rule reports every
/// undocumented item its fail fixture holds, through the real ruff pipeline.
///
/// An item is held to the SOURCE LINE its finding stands on, because ruff writes
/// one message for each code and never spells the name it read.
///
/// The count is the other half, and it is what a silent run cannot fake. The
/// pass fixture holds an undocumented `__str__`, `__repr__`, `__eq__`, property
/// setter, test class, test method and test function, so a run that reported one
/// of them would fail the pair; holding this run to exactly these six states the
/// same silence from the other side.
#[test]
fn the_shipped_python_missing_docs_tool_rule_reports_every_fail_fixture_item() {
    verify_shipped_fail_fixture_reports_each(
        &PYTHON_MISSING_DOCS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an undocumented class, nested class, constructor, getter, method and function",
                CODE_HYGIENE_SET,
                [
                    MISSING_DOCS_PROMPT_RULE.to_string(),
                    PYTHON_MISSING_DOCS_RULE.to_string(),
                ],
                [(PYTHON_MISSING_DOCS_FIXTURE_PATH, content)],
            )
        },
        fail_fixture_source_line,
        |reported, item| reported == item,
    );
}

/// The declarations every staged Python position holds, each one undocumented.
///
/// `TestShared`, `test_method` and `test_shared` carry the name pytest and
/// unittest collect by, so the rule must drop each one at every position.
/// `helper_shared` carries no such name, so the rule must report it at every
/// position — the test file included.
const PYTHON_STAGED_DECLARATIONS: &str = concat!(
    "class TestShared:\n",
    "    def test_method(self) -> None:\n",
    "        assert True\n",
    "\n",
    "\n",
    "def test_shared() -> None:\n",
    "    assert True\n",
    "\n",
    "\n",
    "def helper_shared() -> None:\n",
    "    return None\n",
);

/// The module docstring a documented position carries above the shared
/// declarations.
const PYTHON_MODULE_DOCSTRING: &str = "\"\"\"A documented module.\"\"\"\n\n\n";

/// The head of a documented module: the docstring and nothing else.
const PYTHON_DOCUMENTED_HEAD: &[&str] = &[PYTHON_MODULE_DOCSTRING];

/// The head of an undocumented module: nothing at all.
const PYTHON_UNDOCUMENTED_HEAD: &[&str] = &[];

/// The ordinary position. It carries a module docstring, so the helper is its
/// one finding.
const PYTHON_STAGED_DOCUMENTED_PATH: &str = "documented.py";

/// The package position. An `__init__.py` with no docstring reports `D104`.
const PYTHON_STAGED_PACKAGE_PATH: &str = "pkg/__init__.py";

/// The test position. The directory and the file name are both what pytest
/// collects by, and the rule reads neither, so it reports the same finding the
/// ordinary position reports.
const PYTHON_STAGED_TEST_PATH: &str = "tests/test_documented.py";

/// The undocumented module position. It reports `D100` above the helper.
const PYTHON_STAGED_UNDOCUMENTED_PATH: &str = "undocumented.py";

/// Each position the shared declarations are staged at.
///
/// The ordinary position and the test position hold the same bytes, so their
/// PATHS are the only difference. The undocumented position drops the module
/// docstring, so that DOCSTRING is its only difference. The package position
/// drops the same docstring under the one file name Python reads as a package.
const PYTHON_STAGED_FILES: &[ShippedStagedFile] = &[
    ShippedStagedFile {
        path: PYTHON_STAGED_DOCUMENTED_PATH,
        head: PYTHON_DOCUMENTED_HEAD,
    },
    ShippedStagedFile {
        path: PYTHON_STAGED_PACKAGE_PATH,
        head: PYTHON_UNDOCUMENTED_HEAD,
    },
    ShippedStagedFile {
        path: PYTHON_STAGED_TEST_PATH,
        head: PYTHON_DOCUMENTED_HEAD,
    },
    ShippedStagedFile {
        path: PYTHON_STAGED_UNDOCUMENTED_PATH,
        head: PYTHON_UNDOCUMENTED_HEAD,
    },
];

/// The file of each finding the four staged positions must report, in the order
/// ruff writes them.
///
/// ruff sorts its report by path, and it holds one file's findings in row order,
/// so the package docstring stands above the helper of the same file. Measured:
/// the order does not move when the file arguments are shuffled.
const PYTHON_STAGED_REPORTS: &[&str] = &[
    PYTHON_STAGED_DOCUMENTED_PATH,
    PYTHON_STAGED_PACKAGE_PATH,
    PYTHON_STAGED_PACKAGE_PATH,
    PYTHON_STAGED_TEST_PATH,
    PYTHON_STAGED_UNDOCUMENTED_PATH,
    PYTHON_STAGED_UNDOCUMENTED_PATH,
];

/// The four staged Python positions, and what the real ruff pipeline must
/// report over them.
const PYTHON_MISSING_DOCS_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_MISSING_DOCS_RULE,
        expected: PYTHON_STAGED_REPORTS,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one test class, one test function and one helper at four positions",
    declarations: PYTHON_STAGED_DECLARATIONS,
    staged: PYTHON_STAGED_FILES,
    support: NO_SUPPORT_FILES,
    reason: "the rule reads the item's own name and never the path: the test class, the test \
             method and the test function are silent at every position, the helper reports at \
             every position, and a module or a package with no docstring reports one more",
};

/// Acceptance: the shipped Python missing-docs tool rule carves a test out by
/// the item's own NAME, through the real ruff pipeline.
///
/// The `missing-docs` prompt rule asks for exactly this test: "Identify test
/// items from the structural marker on the item itself ... not from the file
/// name or path." ruff has no filter on a name, and `--isolated` discards the
/// `per-file-ignores` entry a project holds for its own test tree, so the script
/// reads the definition line each finding stands on.
///
/// The four positions hold the same declarations, so the path and the module
/// docstring are the only things that differ. `tests/test_documented.py` carries
/// the directory and the file name pytest collects by, and it reports the same
/// helper the ordinary position reports — which is what a path-shaped carve-out
/// would lose in silence.
#[test]
fn the_shipped_python_missing_docs_tool_rule_reads_the_item_name_and_not_the_path() {
    verify_shipped_staged_positions_report(&PYTHON_MISSING_DOCS_POSITIONS_PROBE);
}

/// A Python file that does not parse: the parameter list of `broken` never
/// closes.
const PYTHON_UNPARSABLE_SOURCE: &str = "def broken(\n";

/// Where the unparsable file stands inside the probe repository.
const PYTHON_UNPARSABLE_PATH: &str = "broken.py";

/// Where the file the run CAN judge stands, beside each item it cannot judge.
const PYTHON_JUDGED_PATH: &str = "judged.py";

/// A Python file the run judges. The module carries a docstring and the
/// function under it carries none, so `D103` is its one finding.
///
/// The module docstring is what holds the count at one: a module with no
/// docstring reports `D100` beside the function, and one row states as much
/// about a lost finding as two do.
const PYTHON_JUDGED_SOURCE: &str = concat!(
    "\"\"\"A documented module.\"\"\"\n",
    "\n",
    "\n",
    "def undocumented_function() -> None:\n",
    "    return None\n",
);

/// The definition line the one finding of the judged file stands on.
const PYTHON_JUDGED_DECLARATION: &str = "def undocumented_function() -> None:";

/// The `path:line` row the run must report for the judged file.
fn python_missing_docs_judged_row() -> String {
    expected_row(
        PYTHON_JUDGED_PATH,
        PYTHON_JUDGED_SOURCE,
        PYTHON_JUDGED_DECLARATION,
    )
}

/// Acceptance: the shipped Python missing-docs tool rule DECLINES a Python
/// file it cannot parse, through the real ruff pipeline.
///
/// ruff writes a file it cannot parse onto the SAME report as a finding, under
/// `"code": "invalid-syntax"`. Measured with ruff 0.14.5 over `def broken(`
/// beside a module whose function carries no docstring: three rows on the
/// report — `D100` and `D103` of the file it read AND the parse failure — at
/// exit 1, and nothing on stderr. ruff judged the other file, so the finding is
/// there to lose.
///
/// A filter that selected the seven documentation codes alone dropped the parse
/// row, and the unparsable file then read as clean. An `exit 1` answers that no
/// better: it fails the WHOLE run, so the documentation findings of the file
/// ruff DID judge go away with the file it did not. Measured with the shipped
/// script before this fix over the same two files: nothing on stdout, one
/// unmarked line on stderr, exit 1. The parse failure is one declined item of a
/// sound run, so the script states it under the `sah-diagnostic:` marker at
/// exit 0.
#[test]
fn the_shipped_python_missing_docs_tool_rule_declines_a_file_it_cannot_parse() {
    let expected = python_missing_docs_judged_row();

    verify_unjudged_file_is_declined(
        PYTHON_PROJECT_TYPES,
        PYTHON_MISSING_DOCS_RULE,
        &[
            (PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE),
            (PYTHON_UNPARSABLE_PATH, PYTHON_UNPARSABLE_SOURCE),
        ],
        PYTHON_UNPARSABLE_PATH,
        &[&expected],
    );
}

/// Where the path the run cannot read stands inside the probe repository.
///
/// One name serves all three shapes: the same path holds no file, holds bytes
/// that are not UTF-8, or holds source nobody may read, so the way it refuses
/// is the one difference between the three probes.
const PYTHON_UNREADABLE_PATH: &str = "unreadable.py";

/// A Python file whose bytes are not UTF-8.
///
/// The module carries a docstring, so a run that DID read it would report
/// nothing; the string literal holds two bytes that open no UTF-8 sequence, so
/// a reader opens the file and cannot decode it.
const PYTHON_UNDECODABLE_SOURCE: &[u8] =
    b"\"\"\"A documented module.\"\"\"\n\nVALUE = '\xff\xfe'\n";

/// A Python file the tool could read if the mode let it.
///
/// The module carries a docstring and holds no other item, so a run that DID
/// read it would report no finding — which is the clean answer this rule must
/// not give for a file it never read.
const PYTHON_FORBIDDEN_SOURCE: &str = "\"\"\"A documented module.\"\"\"\n";

/// The `missing-docs-python` probe over a refusing path beside `judged.py`.
///
/// The judged file carries one undocumented function, so the run has a finding
/// to lose. Losing it is what a nonzero exit over a declined item costs, and
/// staying silent about the path is what reads that path as a clean file.
fn python_missing_docs_decline_probe() -> ShippedDeclineProbe {
    ShippedDeclineProbe {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_MISSING_DOCS_RULE,
        judged: vec![(PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE.to_string())],
        path: PYTHON_UNREADABLE_PATH,
        expected: vec![python_missing_docs_judged_row()],
    }
}

/// Acceptance: the shipped Python missing-docs tool rule DECLINES a path that
/// holds no file, through the real ruff pipeline.
///
/// Measured with ruff 0.14.5 over such a path, against the shipped command
/// line: the report holds the findings of the other file and nothing for this
/// path, `warning: Failed to lint absent.py: No such file or directory (os
/// error 2)` stands on stderr, and ruff exits as it would without the path. The
/// empty report reads exactly like a clean file, so the answer has to come from
/// what ruff itself said.
#[test]
fn the_shipped_python_missing_docs_tool_rule_declines_a_path_that_holds_no_file() {
    verify_unreadable_file_is_declined(
        &python_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Absent,
    );
}

/// Acceptance: the shipped Python missing-docs tool rule DECLINES a file whose
/// bytes are not UTF-8, through the real ruff pipeline.
///
/// Measured with ruff 0.14.5 over such a file, against the shipped command
/// line: the report holds the findings of the other file, `warning: Failed to
/// lint notutf8.py: stream did not contain valid UTF-8` stands on stderr, and
/// ruff exits as it would without the file.
///
/// This is the shape a readability test on the PATH admits — the mode lets the
/// tool open the file — so the run this replaced reported the other file,
/// exited 0, and said nothing an engine reads about this one. Measured with the
/// shipped script before this fix: two findings on stdout, ruff's own unmarked
/// `Failed to lint` line on stderr, exit 0, and the engine drops an unmarked
/// line as tool chatter. The file read as CLEAN.
#[test]
fn the_shipped_python_missing_docs_tool_rule_declines_a_file_it_cannot_decode() {
    verify_unreadable_file_is_declined(
        &python_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Undecodable(PYTHON_UNDECODABLE_SOURCE),
    );
}

/// Acceptance: the shipped Python missing-docs tool rule DECLINES a file it may
/// not read, through the real ruff pipeline.
///
/// Measured with ruff 0.14.5 over such a file, against the shipped command
/// line: the report holds the findings of the other file, `warning: Failed to
/// lint noread.py: Permission denied (os error 13)` stands on stderr, and ruff
/// exits as it would without the file.
///
/// The probe takes every permission off the file, which is a mode, so it runs
/// on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_python_missing_docs_tool_rule_declines_a_file_it_may_not_read() {
    verify_unreadable_file_is_declined(
        &python_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Forbidden(PYTHON_FORBIDDEN_SOURCE),
    );
}

/// Where the directory nobody may read stands inside the probe repository.
///
/// The name carries the Python extension, the way `Sources/Hollow.swift` does
/// for Swift: the engine hands a `files`-scope run the paths its work-list
/// holds, and a path the rule's own file pattern refuses reaches no run at
/// all.
const PYTHON_UNREADABLE_DIRECTORY: &str = "unread.py";

/// What ruff says for a directory it may not read, with its `warning: ` head
/// taken off.
///
/// The line names NO path. ruff walks the path it is given, and a directory it
/// may not open stops the walk before it reaches a file of its own to name.
const PYTHON_DIRECTORY_REFUSAL: &str = "Encountered error: Permission denied (os error 13)";

/// Acceptance: the shipped Python missing-docs tool rule DECLINES a directory
/// it may not read, through the real ruff pipeline.
///
/// A directory refuses ruff under another head than a file does. Measured with
/// ruff 0.14.5 over `judged.py` beside a mode-000 directory: the report holds
/// the `D103` of the file it judged, ruff exits 1, and stderr carries
/// `warning: Encountered error: Permission denied (os error 13)` — a head that
/// is NOT `warning: Failed to lint `, and a line that carries no path.
///
/// A scan that read the one head therefore matched nothing. Measured with the
/// shipped script before this fix: exit 0, the `D103` reported, and 0 bytes on
/// stderr — the directory nobody read came back as a fully judged clean tree.
///
/// The probe takes every permission off the directory, which is a mode, so it
/// runs on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_python_missing_docs_tool_rule_declines_a_directory_it_may_not_read() {
    let expected = python_missing_docs_judged_row();
    let judged = [(PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE)];
    let prepare = |repo: &Path| forbid_probe_directory(&repo.join(PYTHON_UNREADABLE_DIRECTORY));
    let restore = |repo: &Path| restore_probe_directory(&repo.join(PYTHON_UNREADABLE_DIRECTORY));
    let staging = ShippedStaging {
        prepare: &prepare,
        restore: &restore,
        ..ShippedStaging::of(&judged)
    };

    verify_declined_item_is_stated(
        PYTHON_PROJECT_TYPES,
        PYTHON_MISSING_DOCS_RULE,
        &staging,
        &[PYTHON_JUDGED_PATH, PYTHON_UNREADABLE_DIRECTORY],
        PYTHON_DIRECTORY_REFUSAL,
        &[&expected],
    );
}

/// Where the file whose name holds a backslash stands inside the probe
/// repository.
///
/// A backslash is the character the earlier hand-off spelled wrong. The script
/// flattened ruff's report to TSV with `jq ... | @tsv`, and `@tsv` escapes a
/// backslash to `\\`, so every row of this file named a path that stands on no
/// disk.
const PYTHON_BACKSLASH_PATH: &str = "back\\slash.py";

/// A Python file with no module docstring whose one function carries none
/// either, so ruff reports `D100` and `D103` on it.
const PYTHON_BACKSLASH_SOURCE: &str = "def in_a_named_file() -> None:\n    return None\n";

/// The definition line the `D103` of the backslash file stands on. It opens
/// [`PYTHON_BACKSLASH_SOURCE`], so the row is read out of the staged bytes.
const PYTHON_BACKSLASH_DECLARATION: &str = "def in_a_named_file() -> None:";

/// Each `path:line` row a run over the judged file and the backslash file must
/// report.
///
/// ruff sorts its report by path, so `back\slash.py` stands above `judged.py`.
/// Both of its rows stand on the first line, for two different reasons: `D100`
/// names the module, which carries no definition line of its own, and the
/// function `D103` names opens the file.
fn python_backslash_rows() -> Vec<String> {
    vec![
        format!("{PYTHON_BACKSLASH_PATH}:{}", python_module_row!()),
        expected_row(
            PYTHON_BACKSLASH_PATH,
            PYTHON_BACKSLASH_SOURCE,
            PYTHON_BACKSLASH_DECLARATION,
        ),
        python_missing_docs_judged_row(),
    ]
}

/// Acceptance: the shipped Python missing-docs tool rule reports a path that
/// holds a backslash as the path it really is, through the real ruff pipeline.
///
/// The script used to hand its rows from `jq` to `awk` as TSV. `@tsv` escapes
/// a backslash, a tab and a newline alike, so the text of every field reached
/// awk changed. Measured with ruff 0.14.5 and jq 1.8.2 over this probe before
/// the fix: three findings at exit 0, two of them naming
/// `back\\slash.py` — a path the engine can attribute to no file — and one
/// diagnostic saying the definition-line scan could not read that same doubled
/// name.
///
/// The filter now reads ruff's JSON report itself, so no text hand-off stands
/// between the report and the scan. The probe holds the whole row list rather
/// than a count, because the ROW is what was wrong.
#[test]
fn the_shipped_python_missing_docs_tool_rule_reports_a_path_holding_a_backslash() {
    let loader = builtin_loader();
    require_tool_installed(&loader, PYTHON_PROJECT_TYPES, PYTHON_MISSING_DOCS_RULE);
    let staged = [
        (PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE),
        (PYTHON_BACKSLASH_PATH, PYTHON_BACKSLASH_SOURCE),
    ];
    let named = [PYTHON_JUDGED_PATH, PYTHON_BACKSLASH_PATH];
    let staging = ShippedStaging::of(&staged);
    let drive = |read: fn(&ScriptOutcome, &Path) -> Vec<String>| {
        drive_shipped_script(&loader, PYTHON_MISSING_DOCS_RULE, &staging, &named, read)
            .expect("a run over a path holding a backslash must judge both files and exit 0")
    };

    let reported = drive(finding_rows);
    let stated = drive(script_diagnostics);

    assert_eq!(
        reported,
        python_backslash_rows(),
        "every finding must name the path ruff read, with the backslash it really holds"
    );
    assert!(
        stated.is_empty(),
        "the run reads every file ruff reported, so it must decline no item; it stated {stated:?}"
    );
}

/// The path a stubbed ruff reports a finding of, and that the definition-line
/// scan then cannot open.
const PYTHON_VANISHED_PATH: &str = "vanished.py";

/// The row the stubbed ruff stands the finding of the vanished path on.
///
/// The path holds no file, so no staged source can carry the row. The stub
/// therefore writes the head of a file, and
/// [`the_shipped_python_missing_docs_tool_rule_keeps_the_findings_its_scan_cannot_carve`]
/// reads the same number back, so this constant is what holds the report the
/// stub wrote and the row the run must place to one value.
const PYTHON_VANISHED_FINDING_ROW: &str = opening_declaration_row!();

/// The code ruff writes for a public function that carries no docstring.
const PYTHON_UNDOCUMENTED_FUNCTION_CODE: &str = "D103";

/// The message ruff writes under [`PYTHON_UNDOCUMENTED_FUNCTION_CODE`].
const PYTHON_UNDOCUMENTED_FUNCTION_MESSAGE: &str = "Missing docstring in public function";

/// One entry of a stubbed ruff report: a `D103` of `path` on `row`.
fn python_stub_report_entry(path: &str, row: &str) -> String {
    format!(
        "{{\"filename\": \"{path}\", \"location\": {{\"row\": {row}}}, \
         \"code\": \"{PYTHON_UNDOCUMENTED_FUNCTION_CODE}\", \
         \"message\": \"{PYTHON_UNDOCUMENTED_FUNCTION_MESSAGE}\"}}"
    )
}

/// A stubbed ruff that exits 1 and reports one `D103` of a path that holds no
/// file, beside the `D103` of the judged file.
///
/// The shipped pipeline reaches no such row of its own any more: the filter
/// opens each path ruff wrote on its report, exactly as ruff spelled it, and
/// ruff declines on stderr every path it could not open itself. So the fail-
/// open needs a ruff that reports a file no reader can follow it to.
///
/// The row of the judged entry is read out of [`PYTHON_JUDGED_SOURCE`] rather
/// than written as a number, because the test holds the run to the row
/// [`python_missing_docs_judged_row`] reads out of those same bytes, and the
/// two must never drift apart.
fn python_vanished_report_answer() -> String {
    let judged_row = declaration_line(PYTHON_JUDGED_SOURCE, PYTHON_JUDGED_DECLARATION);
    let vanished = python_stub_report_entry(PYTHON_VANISHED_PATH, PYTHON_VANISHED_FINDING_ROW);
    let judged = python_stub_report_entry(PYTHON_JUDGED_PATH, &judged_row.to_string());

    format!("  printf '[{vanished}, {judged}]'\n  exit 1")
}

/// Acceptance: the shipped Python missing-docs tool rule keeps every finding
/// of a file its definition-line scan could not read.
///
/// The scan re-reads each file ruff reported, to read the definition line the
/// test carve-out needs. That read can fail where ruff's own read did not, and
/// a failed scan is one more declined item: the finding is real, and only the
/// carve-out is unanswerable. So the scan states the file under the marker and
/// reads no line for it, and every row of that file keeps its finding.
///
/// Measured with the shipped script before the fail-open, over a file the scan
/// could not open: nothing on stdout, one unmarked line on stderr, exit 1 —
/// the finding of the file the scan DID read lost with it.
///
/// The probe leads `PATH` with a stub, which is process state, so it stands
/// under `#[serial_test::serial(env)]`.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_python_missing_docs_tool_rule_keeps_the_findings_its_scan_cannot_carve() {
    let run = drive_shipped_script_with_stub(
        PYTHON_PROJECT_TYPES,
        PYTHON_MISSING_DOCS_RULE,
        &[(PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE)],
        &[PYTHON_JUDGED_PATH],
        PYTHON_TOOL_BINARY_NAME,
        &python_vanished_report_answer(),
    );
    let outcome = run
        .outcome
        .expect("a scan that cannot read one file must keep every finding and exit 0");

    assert_eq!(
        run.placed,
        vec![
            format!("{PYTHON_VANISHED_PATH}:{PYTHON_VANISHED_FINDING_ROW}"),
            python_missing_docs_judged_row(),
        ],
        "the finding of the file the scan could not read must stand beside the one it read"
    );
    assert_eq!(
        outcome.diagnostics,
        vec![format!(
            "missing-docs-python could not read {PYTHON_VANISHED_PATH}, so every finding of \
             that file stands"
        )],
        "the run must state the one file its scan could not read"
    );
}

/// The name ruff calls the linter of Python code by.
const PYTHON_TOOL_BINARY_NAME: &str = "ruff";

/// What the run must say when its filter could not read what ruff wrote.
///
/// A fragment of the line rather than the whole of it: the filter names the
/// reason Python gave beside these words, and that reason moves with the
/// Python release.
const PYTHON_FILTER_BROKEN_MESSAGE: &str =
    "missing-docs-python: the filter could not read the ruff report";

/// Why a report the filter cannot read must break the run rather than pass it.
const PYTHON_FILTER_BROKEN_REASON: &str =
    "a report the filter could not read leaves the run with no measurement at all, so it \
     must break in the rule's own words rather than exit on the filter's status";

/// A stubbed ruff that exits 1 and writes a report that stops in the middle of
/// its first entry.
///
/// Status 1 is the status ruff exits with for a file that HAS findings, so the
/// broken-run gate lets it through. The report is what the filter then cannot
/// read.
const PYTHON_TRUNCATED_REPORT_ANSWER: &str =
    "  printf '[\\n  {\\n    \"code\": \"D103\"'\n  exit 1";

/// Acceptance: the shipped Python missing-docs tool rule BREAKS when ruff
/// writes a report the filter cannot read.
///
/// The broken-run gate reads the STATUS, and ruff keeps status 1 for a file
/// with findings, so a malformed report at status 1 passes the gate. The
/// filter then fails, and a filter that reads the report with no guard takes
/// the whole script down with its own status. Measured with a stubbed ruff
/// over a filter with no guard: the script exited 1, wrote nothing to stdout,
/// and wrote a Python traceback naming the temporary copy of the filter — no
/// marker, and no word about which rule broke.
///
/// `missing-docs-rust.md` and `function-length-rust.md` hold the worked
/// answer, and this rule now holds it too: the filter catches the report it
/// cannot read, and breaks in the rule's own words.
///
/// The probe leads `PATH` with a stub, which is process state, so it stands
/// under `#[serial_test::serial(env)]`.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_python_missing_docs_tool_rule_breaks_on_a_report_the_filter_cannot_read() {
    let run = drive_shipped_script_with_stub(
        PYTHON_PROJECT_TYPES,
        PYTHON_MISSING_DOCS_RULE,
        &[(PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE)],
        &[PYTHON_JUDGED_PATH],
        PYTHON_TOOL_BINARY_NAME,
        PYTHON_TRUNCATED_REPORT_ANSWER,
    );
    let failure = run.outcome.expect_err(PYTHON_FILTER_BROKEN_REASON);

    assert_shipped_break(
        &failure,
        run.status,
        &[PYTHON_FILTER_BROKEN_MESSAGE],
        PYTHON_FILTER_BROKEN_REASON,
    );
    assert!(
        run.placed.is_empty(),
        "a run that breaks must place no finding; it placed {:?}: {PYTHON_FILTER_BROKEN_REASON}",
        run.placed
    );
}

/// The path a stubbed ruff declines on a line that carries no closing newline.
const PYTHON_UNTERMINATED_PATH: &str = "unterminated.py";

/// A stubbed ruff that judges nothing, and declines one path on a line with no
/// closing newline.
///
/// The report is an empty array, so the filter reads it and the run reports
/// nothing. The one decline stands on stderr, and the bytes stop where the
/// message stops.
const PYTHON_UNTERMINATED_ANSWER: &str = concat!(
    "  printf '[]'\n",
    "  printf 'warning: Failed to lint unterminated.py: No such file or directory (os error 2)' \
     >&2\n",
    "  exit 1"
);

/// Acceptance: the shipped Python missing-docs tool rule reads a decline line
/// that carries no closing newline.
///
/// `while IFS= read -r line` answers nonzero for a last line with no newline
/// under it, so a loop that reads the status alone never runs its body for
/// that line and the decline goes unsaid. Measured over two decline lines with
/// the shipped script before this fix: 2 diagnostics with the closing newline,
/// and 1 without it — the last decline lost, and that path then read as clean.
///
/// The probe leads `PATH` with a stub, which is process state, so it stands
/// under `#[serial_test::serial(env)]`.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_python_missing_docs_tool_rule_reads_a_decline_with_no_closing_newline() {
    let run = drive_shipped_script_with_stub(
        PYTHON_PROJECT_TYPES,
        PYTHON_MISSING_DOCS_RULE,
        NO_SUPPORT_FILES,
        &[PYTHON_UNTERMINATED_PATH],
        PYTHON_TOOL_BINARY_NAME,
        PYTHON_UNTERMINATED_ANSWER,
    );
    let outcome = run
        .outcome
        .expect("a script handed an item it cannot judge must judge the rest and exit 0");

    assert_eq!(
        outcome.diagnostics,
        vec![format!(
            "ruff declined an item and said: warning: Failed to lint {PYTHON_UNTERMINATED_PATH}: \
             No such file or directory (os error 2)"
        )],
        "the run must state the decline that stands on the last line of ruff's stderr, \
         whether or not a newline closes it"
    );
}

/// An undocumented Python module at the root of the probe repository. ruff
/// reports `D100` on the module and `D103` on the function.
const PYTHON_UNREAD_TOP_SOURCE: &str = "def top():\n    return 1\n";

/// The same, nested three directories deep. ruff walks a whole tree, so a
/// default target reaches this file as readily as the one at the root.
const PYTHON_UNREAD_NESTED_SOURCE: &str = "class Other:\n    def method(self):\n        return 2\n";

/// Every Python file staged in the probe repository the script is given none
/// of.
const PYTHON_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.py", PYTHON_UNREAD_TOP_SOURCE),
    ("deep/nested/other.py", PYTHON_UNREAD_NESTED_SOURCE),
];

/// Each finding the Python missing-docs script reports over the two files it
/// is given, as `path:line`.
///
/// The nested file answers three: `D100` for the module, `D101` for the class
/// that opens it, and `D102` for the method on the line under the class head.
/// The file at the root answers two: `D100` for the module, and `D103` for the
/// function that opens it.
const PYTHON_READ_FINDINGS: &[&str] = &[
    concat!("deep/nested/other.py:", python_module_row!()),
    concat!("deep/nested/other.py:", opening_declaration_row!()),
    concat!("deep/nested/other.py:", nested_declaration_row!()),
    concat!("top.py:", python_module_row!()),
    concat!("top.py:", opening_declaration_row!()),
];

/// The `missing-docs-python` probe over a run that is given no file.
const PYTHON_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    staged: PYTHON_UNREAD_FILES,
    with_files: PYTHON_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Python missing-docs tool rule reads only the files
/// it is given, through the real ruff pipeline.
///
/// `ruff check` with no path argument falls back to a default target of `.`,
/// and it walks that whole tree. A script that hands `"$@"` straight to ruff
/// therefore answers for every Python file under the repository root when the
/// run carries no file, and it exits 0, so the answer reads as a measured
/// result rather than a mistake. Measured over this probe before the guard:
/// 5 findings across `top.py` and `deep/nested/other.py`, neither of which the
/// script was given, and an exit status of 0.
///
/// The script therefore answers an empty argument list at once, with no
/// finding and an exit status of 0. The same script over the two staged files
/// reports 5.
#[test]
fn the_shipped_python_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&PYTHON_EMPTY_RUN_PROBE);
}
