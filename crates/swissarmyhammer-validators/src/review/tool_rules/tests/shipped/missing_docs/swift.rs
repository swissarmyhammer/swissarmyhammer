//! Acceptance tests for the shipped Swift missing-docs tool rule.

use super::*;

/// The materialized name of the `missing-docs-swift` fail fixture.
const SWIFT_MISSING_DOCS_FAIL_FIXTURE: &str = concat!(missing_docs_rule!(swift), ".fail.swift");

/// The materialized name of the `missing-docs-swift` pass fixture.
const SWIFT_MISSING_DOCS_PASS_FIXTURE: &str = concat!(missing_docs_rule!(swift), ".pass.swift");

/// Where the `missing-docs-swift` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const SWIFT_MISSING_DOCS_FIXTURE_PATH: &str = "Sources/MissingDocsSwiftFail.swift";

/// Where the `missing-docs-swift` pass fixture stands inside the probe
/// repository, as the work-list holds it.
const SWIFT_MISSING_DOCS_PASS_PATH: &str = "Sources/MissingDocsSwiftPass.swift";

/// Every declaration the `missing-docs-swift` fail fixture leaves
/// undocumented, trimmed as the fixture writes it.
///
/// A line, and not a claim, because the claim of a finding spells only the
/// NAME of the declaration, and a method and a function can share one.
const SWIFT_MISSING_DOCS_FAIL_LINES: &[&str] = &[
    "public struct UndocumentedStructure {",
    "public func undocumentedMethod() {}",
    "public func undocumentedFunction() {}",
];

/// The `missing-docs-swift` fail fixture, and every undocumented declaration
/// the real `swift format` pipeline must report inside it.
const SWIFT_MISSING_DOCS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_MISSING_DOCS_FAIL_LINES,
    },
    fixture: SWIFT_MISSING_DOCS_FAIL_FIXTURE,
    path: SWIFT_MISSING_DOCS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "line holding an undocumented public declaration",
};

/// Acceptance: the shipped Swift missing-docs tool rule reports every
/// undocumented declaration its fail fixture holds, through the real
/// `swift format` pipeline.
///
/// A declaration is held to the SOURCE LINE its finding stands on. The count
/// is the other half, and it is what a silent run cannot fake: the fixture
/// holds three undocumented declarations and one documented neighbor, and the
/// run reports exactly those three. A run that read no configuration of its
/// own reports none, because `AllPublicDeclarationsHaveDocumentation` is OFF
/// in the toolchain's default configuration.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reports_every_fail_fixture_line() {
    verify_shipped_fail_fixture_reports_each(
        &SWIFT_MISSING_DOCS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an undocumented public structure, method and function",
                CODE_HYGIENE_SET,
                [
                    MISSING_DOCS_PROMPT_RULE.to_string(),
                    SWIFT_MISSING_DOCS_RULE.to_string(),
                ],
                [(SWIFT_MISSING_DOCS_FIXTURE_PATH, content)],
            )
        },
        fail_fixture_source_line,
        |reported, expected| reported == expected,
    );
}

/// The name of the one rule this gate keeps.
const SWIFT_MISSING_DOCS_TOOL_RULE: &str = "AllPublicDeclarationsHaveDocumentation";

/// Where a probe stages the Swift file it measures the rule over.
const SWIFT_MISSING_DOCS_PROBE_PATH: &str = "Sources/Probe.swift";

/// Where a probe stages a SECOND Swift file of the same run.
const SWIFT_MISSING_DOCS_SECOND_PATH: &str = "Sources/Second.swift";

/// Both files of a two-file run, in the order a probe stages them.
const SWIFT_MISSING_DOCS_TWO_PATHS: [&str; 2] = [
    SWIFT_MISSING_DOCS_PROBE_PATH,
    SWIFT_MISSING_DOCS_SECOND_PATH,
];

/// Runs the shipped Swift missing-docs script over the staged files, and
/// answers the `path:line` rows it reports.
///
/// Every probe of this module that drives the script itself reaches it through
/// here: the loader, the toolchain precondition and the exit-0 expectation have
/// one implementation. `judged` names the files the run carries.
fn swift_missing_docs_script_findings(staged: &[(&str, &str)], judged: &[&str]) -> Vec<String> {
    let loader = builtin_loader();
    require_tool_installed(&loader, SWIFT_PROJECT_TYPES, SWIFT_MISSING_DOCS_RULE);

    shipped_script_findings(&loader, SWIFT_MISSING_DOCS_RULE, staged, judged).expect(
        "the shipped Swift missing-docs script must judge every file it is given and exit 0",
    )
}

/// Reads the shipped pass fixture, and answers the `path:line` rows the
/// shipped script reports over it.
fn swift_missing_docs_pass_fixture_findings() -> Vec<String> {
    let loader = builtin_loader();
    let shipped = shipped_asset(
        &loader,
        &FIXTURE_TEMPLATE_ASSET,
        SWIFT_MISSING_DOCS_PASS_FIXTURE,
    );
    let content = std::fs::read_to_string(shipped).expect("read the shipped pass fixture");

    swift_missing_docs_script_findings(
        &[(SWIFT_MISSING_DOCS_PASS_PATH, content.as_str())],
        &[SWIFT_MISSING_DOCS_PASS_PATH],
    )
}

/// Acceptance: the shipped Swift missing-docs tool rule reports NOTHING over
/// its own passing fixture.
///
/// The fixture is written with 4-space indentation, the house style of every
/// Swift fixture of this set, and `swift format` asks for 2. The raw tool run
/// over it writes one `[Indentation]` line for each member, and no key of the
/// configuration stops the pretty-printer that writes them. So the script must
/// read the tag off each output line and keep only
/// `AllPublicDeclarationsHaveDocumentation`. A script that drops that filter
/// reports layout here and fails this test by name.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reports_nothing_over_its_pass_fixture() {
    let rows = swift_missing_docs_pass_fixture_findings();

    assert!(
        rows.is_empty(),
        "the passing fixture documents every public declaration, so the gate must report \
         nothing over it, whatever the pretty-printer says about its layout; the run reported \
         {rows:?}"
    );
}

/// A Swift file that documents every public declaration and lays the code out
/// in ways the pretty-printer reports: a deep indent, a missing space around
/// an operator, and trailing whitespace.
const SWIFT_MISSING_DOCS_LAYOUT_SOURCE: &str = concat!(
    "/// A documented structure.\n",
    "public struct Laid {\n",
    "        /// A documented property.\n",
    "        public var total = 1+2   \n",
    "}\n",
);

/// Acceptance: the gate drops every layout tag the pretty-printer writes.
///
/// `swift format lint` always writes `[Indentation]`, `[Spacing]`,
/// `[TrailingWhitespace]` and the other layout tags, and no key of the
/// configuration turns them off. The probe documents every public declaration
/// and draws three of those tags, so the only honest answer is no finding.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_drops_every_layout_finding() {
    let reported = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_MISSING_DOCS_PROBE_PATH,
        SWIFT_MISSING_DOCS_LAYOUT_SOURCE,
        NO_SUPPORT_FILES,
    );

    assert!(
        reported.is_empty(),
        "a documented file with layout defects must draw no finding; the run reported \
         {reported:?}"
    );
}

/// A Swift file whose documentation comments draw the findings of the two
/// documentation-quality rules, beside one default-on rule and one real
/// missing comment.
///
/// - the summary runs to two sentences, which
///   `BeginDocumentationCommentWithOneLineSummary` reports once it is ON;
/// - the `Parameter` entry names no parameter, which
///   `ValidateDocumentationComments` reports once it is ON;
/// - `Bad_One` is a name `AlwaysUseLowerCamelCase` reports in the DEFAULT
///   configuration;
/// - the last property is the one missing comment.
const SWIFT_MISSING_DOCS_OTHER_RULES_SOURCE: &str = concat!(
    "/// Summary sentence one. Sentence two follows it.\n",
    "///\n",
    "/// - Parameter wrong: names no parameter.\n",
    "public func documented(right: Int) {}\n",
    "\n",
    "let Bad_One = 1\n",
    "\n",
    "/// A documented structure.\n",
    "public struct Bag {\n",
    "    public var items: Int\n",
    "}\n",
);

/// Acceptance: the gate keeps the one rule it owns and no other.
///
/// `ValidateDocumentationComments` and
/// `BeginDocumentationCommentWithOneLineSummary` decide the bullets of the
/// `doc-parameter-naming` prompt rule, and `AlwaysUseLowerCamelCase` is ON in
/// the toolchain's default configuration. Measured over this probe with those
/// three rules ON, the tool reports each of them. The gate must report the
/// one missing comment of `Bag.items` and nothing else, and every finding it
/// reports must carry `AllPublicDeclarationsHaveDocumentation`.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_names_only_its_own_rule() {
    let reported = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_MISSING_DOCS_PROBE_PATH,
        SWIFT_MISSING_DOCS_OTHER_RULES_SOURCE,
        NO_SUPPORT_FILES,
    );

    assert_eq!(
        reported,
        vec![SWIFT_MISSING_DOCS_TOOL_RULE.to_string()],
        "the probe holds one missing comment and three defects of other rules, so the gate \
         must report one finding carrying only `{SWIFT_MISSING_DOCS_TOOL_RULE}`"
    );
}

/// Where a project states its own `swift format` settings.
const SWIFT_MISSING_DOCS_PROJECT_CONFIG_PATH: &str = ".swift-format";

/// A project `.swift-format` that turns the rule this gate owns OFF.
const SWIFT_MISSING_DOCS_PROJECT_CONFIG: &str =
    "{\"rules\":{\"AllPublicDeclarationsHaveDocumentation\":false}}\n";

/// Acceptance: a project cannot switch the gate off with its own
/// `.swift-format`.
///
/// `swift format` reads a project `.swift-format` on its own, and the
/// `--configuration` the script gives the tool takes that file out of the run.
/// So a project that states the rule OFF still gets the finding. To exempt a
/// declaration the author writes the `swift-format-ignore` directive in the
/// code, where a reader sees it.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_keeps_its_own_configuration() {
    let reported = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_STAGED_PATH,
        SWIFT_STAGED_DECLARATIONS,
        &[(
            SWIFT_MISSING_DOCS_PROJECT_CONFIG_PATH,
            SWIFT_MISSING_DOCS_PROJECT_CONFIG,
        )],
    );

    assert_eq!(
        reported.len(),
        2,
        "the project turns the rule OFF in its own `.swift-format`, and the gate must still \
         report the structure and the property; the run reported {reported:?}"
    );
}

/// Where the staged declarations stand for a probe of one file.
const SWIFT_STAGED_PATH: &str = "Sources/Staged.swift";

/// A public getter and a public protocol-conformance member inside types that
/// declare an inherited type.
///
/// The `missing-docs` prompt rule carves out "Simple getters/setters" and
/// "Obvious implementations". The toolchain rule keeps neither carve-out for
/// Swift: it reads the access level and the doc comment and nothing else.
const SWIFT_MISSING_DOCS_INHERITED_SOURCE: &str = concat!(
    "public struct Wide: Equatable {\n",
    "    public var name: String { \"\" }\n",
    "}\n",
    "\n",
    "extension Wide: CustomStringConvertible {\n",
    "    public var description: String { \"\" }\n",
    "}\n",
);

/// Acceptance: the gate reports an undocumented public getter and an
/// undocumented conformance member, inside a type that declares an inherited
/// type.
///
/// `missing-docs.md` states that Swift keeps neither the getter carve-out nor
/// the obvious-implementation carve-out. This probe is the measurement behind
/// that statement: the structure, the getter and the `description` member each
/// report.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reports_a_getter_in_a_type_with_an_inherited_type() {
    let reported = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_STAGED_PATH,
        SWIFT_MISSING_DOCS_INHERITED_SOURCE,
        NO_SUPPORT_FILES,
    );

    assert_eq!(
        reported.len(),
        3,
        "the structure, its getter and the conformance member are each public and each \
         undocumented, so the gate must report all three; the run reported {reported:?}"
    );
}

/// Two undocumented public functions, the first under the directive that
/// exempts one declaration.
const SWIFT_MISSING_DOCS_DIRECTIVE_SOURCE: &str = concat!(
    "// swift-format-ignore: AllPublicDeclarationsHaveDocumentation\n",
    "public func exempted() {}\n",
    "\n",
    "public func reported() {}\n",
);

/// The same two functions under the directive that exempts the whole file.
const SWIFT_MISSING_DOCS_FILE_DIRECTIVE_SOURCE: &str = concat!(
    "// swift-format-ignore-file\n",
    "public func exempted() {}\n",
    "\n",
    "public func alsoExempted() {}\n",
);

/// Acceptance: the directives an author writes in the code exempt what they
/// name, and the gate reads them.
///
/// The line form exempts ONE declaration, so the second function still
/// reports. The file form exempts the whole file, which is how generated code
/// leaves the gate: the `missing-docs` prompt rule carves generated code out,
/// and the tool carries no generated-code check of its own.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reads_the_exemption_directives() {
    let one = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_MISSING_DOCS_PROBE_PATH,
        SWIFT_MISSING_DOCS_DIRECTIVE_SOURCE,
        NO_SUPPORT_FILES,
    );
    assert_eq!(
        one,
        vec![SWIFT_MISSING_DOCS_TOOL_RULE.to_string()],
        "the line directive exempts one declaration, so the second function alone must report"
    );

    let whole = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_MISSING_DOCS_PROBE_PATH,
        SWIFT_MISSING_DOCS_FILE_DIRECTIVE_SOURCE,
        NO_SUPPORT_FILES,
    );
    assert!(
        whole.is_empty(),
        "the file directive exempts every declaration of the file; the run reported {whole:?}"
    );
}

/// Acceptance: a run over two files that both report gives the findings of
/// both.
///
/// The script hands `swift format` one path for each run, and a file that
/// reports makes the tool exit 1. A loop that stopped at the first file that
/// reported, or that let `set -e` end the run on that status, would report the
/// first file alone and the second would read as clean.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reports_every_file_that_reports() {
    let rows = swift_missing_docs_script_findings(
        &[
            (SWIFT_MISSING_DOCS_PROBE_PATH, SWIFT_STAGED_DECLARATIONS),
            (SWIFT_MISSING_DOCS_SECOND_PATH, SWIFT_STAGED_DECLARATIONS),
        ],
        &SWIFT_MISSING_DOCS_TWO_PATHS,
    );

    for path in SWIFT_MISSING_DOCS_TWO_PATHS {
        assert_eq!(
            rows.iter().filter(|row| row.starts_with(path)).count(),
            2,
            "a run over two files that both report must state the structure and the property \
             of `{path}`; the run reported {rows:?}"
        );
    }
}

/// One undocumented public structure, and one undocumented stored property
/// inside it.
const SWIFT_STAGED_DECLARATIONS: &str = concat!(
    "public struct StagedThing {\n",
    "    public var value: Int = 0\n",
    "}\n",
);

/// Where the path the missing-docs run cannot judge stands inside the probe
/// repository.
///
/// One name serves all three shapes: the same path holds no file, holds bytes
/// that are not UTF-8, or holds source nobody may read, so the way it refuses
/// is the one difference between the three probes.
const SWIFT_MISSING_DOCS_UNREADABLE_PATH: &str = "Sources/Unreadable.swift";

/// Where the file the run CAN judge stands.
const SWIFT_MISSING_DOCS_JUDGED_PATH: &str = "Sources/Judged.swift";

/// The head of the undocumented public structure
/// [`SWIFT_STAGED_DECLARATIONS`] declares.
const SWIFT_STAGED_STRUCTURE_HEAD: &str = "public struct StagedThing";

/// The head of the undocumented stored property
/// [`SWIFT_STAGED_DECLARATIONS`] declares.
const SWIFT_STAGED_PROPERTY_HEAD: &str = "public var value";

/// A Swift file written in Latin-1 rather than in UTF-8.
///
/// The byte `0xE9` is `é` in Latin-1, and it is not a UTF-8 sequence, so the
/// tool cannot decode this file. The staged declarations stand under the
/// string, so a run that DID read the file reports them.
const SWIFT_MISSING_DOCS_UNDECODABLE_SOURCE: &[u8] = b"let name = \"caf\xe9\"\n\
public struct StagedThing {\n\
public var value: Int = 0\n\
}\n";

/// A Swift file the tool could read if the mode let it.
///
/// Every declaration is internal, so a run that DID read this file would
/// report no finding, which is the clean answer this rule must not give for a
/// file it never read.
const SWIFT_MISSING_DOCS_FORBIDDEN_SOURCE: &str = concat!(
    "struct InternalThing {\n",
    "    var value: Int = 0\n",
    "}\n"
);

/// The `path:line` rows of the structure and the property that
/// `Sources/Judged.swift` holds undocumented.
///
/// Every decline probe expects these two rows, so the row logic has one home.
fn swift_missing_docs_judged_rows() -> Vec<String> {
    [SWIFT_STAGED_STRUCTURE_HEAD, SWIFT_STAGED_PROPERTY_HEAD]
        .into_iter()
        .map(|head| {
            expected_row(
                SWIFT_MISSING_DOCS_JUDGED_PATH,
                SWIFT_STAGED_DECLARATIONS,
                head,
            )
        })
        .collect()
}

/// The `missing-docs-swift` probe over a refusing path beside
/// `Sources/Judged.swift`.
///
/// The judged file carries one undocumented public structure and one
/// undocumented stored property, so the run has two findings to lose. Losing
/// them is what a nonzero exit over a declined item costs, and staying silent
/// about the path is what reads that path as a clean file.
fn swift_missing_docs_decline_probe() -> ShippedDeclineProbe {
    ShippedDeclineProbe {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        judged: vec![(
            SWIFT_MISSING_DOCS_JUDGED_PATH,
            SWIFT_STAGED_DECLARATIONS.to_string(),
        )],
        path: SWIFT_MISSING_DOCS_UNREADABLE_PATH,
        expected: swift_missing_docs_judged_rows(),
    }
}

/// Acceptance: the shipped Swift missing-docs tool rule DECLINES a path that
/// holds no file, through the real `swift format` pipeline.
///
/// Measured with Apple Swift 6.4: `swift format lint --strict` over such a
/// path exits 0 and writes NOTHING, which reads exactly like a clean pass over
/// a file the run never opened. So the script tests the path itself, and
/// states it under the marker.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_declines_a_path_that_holds_no_file() {
    verify_unreadable_file_is_declined(
        &swift_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Absent,
    );
}

/// Acceptance: the shipped Swift missing-docs tool rule DECLINES a Swift file
/// the tool cannot decode, through the real `swift format` pipeline.
///
/// The mode lets the script open this file, so a readability test on the path
/// admits it. The answer has to come from what the tool itself said.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_declines_a_file_it_cannot_decode() {
    verify_unreadable_file_is_declined(
        &swift_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Undecodable(SWIFT_MISSING_DOCS_UNDECODABLE_SOURCE),
    );
}

/// Acceptance: the shipped Swift missing-docs tool rule DECLINES a Swift file
/// it may not read, through the real `swift format` pipeline.
///
/// The probe takes every permission off the file, which is a mode, so it runs
/// on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_swift_missing_docs_tool_rule_declines_a_file_it_may_not_read() {
    verify_unreadable_file_is_declined(
        &swift_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Forbidden(SWIFT_MISSING_DOCS_FORBIDDEN_SOURCE),
    );
}

/// A Swift file the parser cannot read.
const SWIFT_MISSING_DOCS_UNPARSABLE_SOURCE: &str = "public func oops( {\n";

/// Acceptance: the shipped Swift missing-docs tool rule DECLINES a file the
/// parser cannot read, and keeps the findings of the file beside it.
///
/// A file the parser cannot read is a file no rule judged. The tool writes
/// its parse errors untagged and exits 1, exactly as it exits for a finding,
/// so the script reads the SHAPE of each line: an untagged line is trouble,
/// and the file that wrote one is declined whole.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_declines_a_file_the_parser_cannot_read() {
    let expected = swift_missing_docs_judged_rows();

    verify_unjudged_file_is_declined(
        SWIFT_PROJECT_TYPES,
        SWIFT_MISSING_DOCS_RULE,
        &[
            (SWIFT_MISSING_DOCS_JUDGED_PATH, SWIFT_STAGED_DECLARATIONS),
            (
                SWIFT_MISSING_DOCS_UNREADABLE_PATH,
                SWIFT_MISSING_DOCS_UNPARSABLE_SOURCE,
            ),
        ],
        SWIFT_MISSING_DOCS_UNREADABLE_PATH,
        &expected.iter().map(String::as_str).collect::<Vec<_>>(),
    );
}

/// The `missing-docs-swift` probe over a directory that holds no Swift file.
///
/// The probe writes no file at the path, and the one staged file under that
/// path makes the directory.
const SWIFT_HOLLOW_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: SWIFT_HOLLOW_PURPOSE,
    path: SWIFT_HOLLOW_PATH,
    source: None,
    support: SWIFT_HOLLOW_FILES,
};

/// Acceptance: the shipped Swift missing-docs tool rule answers CLEAN over a
/// directory that holds no Swift file, through the real `swift format`
/// pipeline.
///
/// A path that names a directory makes `swift format` exit 64, and a shared
/// run would lose every finding beside it. The script tests that the path is a
/// file before the tool starts, declines the directory under the marker, and
/// exits 0.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_stays_clean_over_a_hollow_directory() {
    verify_shipped_hollow_directory_answers_clean(&SWIFT_HOLLOW_PROBE);
}

/// An undocumented Swift file at the root of the probe repository.
const SWIFT_UNREAD_TOP_SOURCE: &str = "public struct Top {\n    public var value: Int = 0\n}\n";

/// The same, nested three directories deep. A tool that walks a whole tree
/// reaches this file as readily as the one at the root.
const SWIFT_UNREAD_NESTED_SOURCE: &str =
    "public enum Other {\n    public static let value = 2\n}\n";

/// Every Swift file staged in the probe repository the script is given none
/// of.
const SWIFT_UNREAD_FILES: &[(&str, &str)] = &[
    ("Top.swift", SWIFT_UNREAD_TOP_SOURCE),
    ("deep/nested/Other.swift", SWIFT_UNREAD_NESTED_SOURCE),
];

/// Each finding the Swift missing-docs script reports over the two files it
/// is given, as `path:line`.
///
/// Each file answers two: the type head that opens it, and the member on the
/// line under that head.
const SWIFT_READ_FINDINGS: &[&str] = &[
    concat!("deep/nested/Other.swift:", opening_declaration_row!()),
    concat!("deep/nested/Other.swift:", nested_declaration_row!()),
    concat!("Top.swift:", opening_declaration_row!()),
    concat!("Top.swift:", nested_declaration_row!()),
];

/// The `missing-docs-swift` probe over a run that is given no file.
const SWIFT_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    staged: SWIFT_UNREAD_FILES,
    with_files: SWIFT_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Swift missing-docs tool rule reads only the files
/// it is given, through the real `swift format` pipeline.
///
/// The script answers an empty argument list at once, with no finding and an
/// exit status of 0, so a run that carries no file never answers for a file
/// the review did not give it. The same script over the two staged files
/// reports 4.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&SWIFT_EMPTY_RUN_PROBE);
}
