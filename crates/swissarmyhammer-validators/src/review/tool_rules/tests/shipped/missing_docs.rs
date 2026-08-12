//! Acceptance tests for the shipped missing-docs tool rules.
//!
//! One test holds the whole roster to its fixture pair and to the prompt rule
//! it supersedes. The tests under it drive one language each through its real
//! tool, so each measures the shipped script rather than a copy.
//!
//! The Rust rule stands in `missing_docs_rust`, because the shapes `cargo
//! clippy` answers a broken run with are cargo's own.

use super::*;

/// Acceptance: every shipped missing-docs tool rule passes its fixture pair
/// in doctor, and supersedes the `missing-docs` prompt rule.
///
/// A tool that reads the whole public surface answers the documentation
/// question the prompt rule asks, so it replaces it for the files it covers.
/// [`verify_shipped_tool_rules_pass_fixtures`] carries the rest of the
/// contract, including what a machine without the tool proves.
#[test]
#[serial_test::serial(cwd)]
fn every_shipped_missing_docs_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(SHIPPED_MISSING_DOCS_RULES, MISSING_DOCS_PROMPT_RULE);
}

/// The materialized name of the `missing-docs-dart` fail fixture.
const DART_MISSING_DOCS_FAIL_FIXTURE: &str = "missing-docs-dart.fail.dart";

/// Where the `missing-docs-dart` fail fixture stands inside the probe
/// repository, as the work-list holds it.
///
/// It stands under `lib/`, because that is the one position
/// `public_member_api_docs` reads.
const DART_MISSING_DOCS_FIXTURE_PATH: &str = "lib/missing_docs_dart_fail.dart";

/// Every member the `missing-docs-dart` fail fixture leaves undocumented,
/// trimmed as the fixture writes it.
///
/// A line, and not a claim, because `public_member_api_docs` writes one
/// message — `Missing documentation for a public member.` — for every member,
/// so the claim never spells which one it read.
///
/// The getter and the setter are load-bearing. The `missing-docs` prompt rule
/// carves out "Simple getters/setters with self-explanatory names", and this
/// rule restores nothing, because the lint takes no option at all. The rule
/// body states that, and these two entries hold the tool to the statement.
const DART_MISSING_DOCS_FAIL_LINES: &[&str] = &[
    "class UndocumentedClass {",
    "void undocumentedMethod() {}",
    "int get undocumentedProperty => _value;",
    "set undocumentedProperty(int next) => _value = next;",
    "void undocumentedFunction() {}",
];

/// The `missing-docs-dart` fail fixture, and every undocumented public member
/// the real `dart analyze` pipeline must report inside it.
const DART_MISSING_DOCS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: &["flutter"],
        rule: DART_MISSING_DOCS_RULE,
        expected: DART_MISSING_DOCS_FAIL_LINES,
    },
    fixture: DART_MISSING_DOCS_FAIL_FIXTURE,
    path: DART_MISSING_DOCS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "line holding an undocumented public member",
};

/// Acceptance: the shipped Dart missing-docs tool rule reports every
/// undocumented public member its fail fixture holds, through the real
/// `dart analyze` pipeline.
///
/// A member is held to the SOURCE LINE its finding stands on, because
/// `public_member_api_docs` writes one message for every member and never
/// spells the member it read.
///
/// The count is the other half, and it is what a silent run cannot fake. The
/// script builds a probe package and runs `dart pub get` inside it, and a run
/// that reached neither the lint nor the package would report zero findings
/// and exit `0`. Holding this run to exactly these five lines states that the
/// analyzer recognized the package, read the configuration the script wrote,
/// and read each kind of member.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_reports_every_fail_fixture_line() {
    verify_shipped_fail_fixture_reports_each(
        &DART_MISSING_DOCS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an undocumented class, method, getter, setter and function",
                CODE_HYGIENE_SET,
                [
                    MISSING_DOCS_PROMPT_RULE.to_string(),
                    DART_MISSING_DOCS_RULE.to_string(),
                ],
                [(DART_MISSING_DOCS_FIXTURE_PATH, content)],
            )
        },
        fail_fixture_source_line,
        |reported, expected| reported == expected,
    );
}

/// One undocumented public class, and one undocumented method inside it.
///
/// Every staged position holds these same bytes, so the POSITION is the only
/// thing that can tell one file of the run from another.
const DART_STAGED_LIBRARY: &str =
    concat!("class StagedClass {\n", "  void stagedMethod() {}\n", "}\n");

/// The library position: a file under a package `lib/`.
///
/// This is the one position `public_member_api_docs` reads in the project
/// itself, so this is the one file of the three the run may report.
const DART_STAGED_LIBRARY_PATH: &str = "lib/staged.dart";

/// The test position. A Dart test lives under `test/`, never under `lib/`, so
/// the project's own analyzer never reads it. The probe stages every changed
/// file under a `lib/` of its own, so only the exclude list keeps this file
/// silent.
const DART_STAGED_TEST_PATH: &str = "test/staged_test.dart";

/// The generator position. `.g.dart` is the fixed output name of
/// `build_runner`, and the `missing-docs` prompt rule this tool rule replaces
/// carves generated code out, so only the exclude list keeps this file silent.
const DART_STAGED_GENERATED_PATH: &str = "lib/staged.g.dart";

/// The head a Dart staged file carries: none. `dart analyze` decides on the
/// path alone, so all three files hold the same bytes.
const DART_NO_HEAD: &[&str] = &[];

/// Each position the staged class is written to, in the order the work-list
/// holds them.
const DART_STAGED_POSITIONS: &[ShippedStagedFile] = &[
    ShippedStagedFile {
        path: DART_STAGED_LIBRARY_PATH,
        head: DART_NO_HEAD,
    },
    ShippedStagedFile {
        path: DART_STAGED_TEST_PATH,
        head: DART_NO_HEAD,
    },
    ShippedStagedFile {
        path: DART_STAGED_GENERATED_PATH,
        head: DART_NO_HEAD,
    },
];

/// The file of each finding the Dart run must report: the library file, once
/// for its class and once for its method.
const DART_STAGED_REPORTED: &[&str] = &[DART_STAGED_LIBRARY_PATH, DART_STAGED_LIBRARY_PATH];

/// The staged Dart positions, and the one of them the real `dart analyze`
/// pipeline must report.
const DART_MISSING_DOCS_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: &["flutter"],
        rule: DART_MISSING_DOCS_RULE,
        expected: DART_STAGED_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented public class, staged in three positions",
    declarations: DART_STAGED_LIBRARY,
    staged: DART_STAGED_POSITIONS,
    support: NO_SUPPORT_FILES,
    reason: "the file under `lib/` reports its class and its method, and the test \
             file and the generated file report nothing",
};

/// Acceptance: the shipped Dart missing-docs tool rule reports the file under
/// `lib/` and stays silent on the test file and the generated file, through
/// the real `dart analyze` pipeline.
///
/// This is the half the fixture pair cannot reach. The doctor materializes one
/// fixture as a loose file with no directory, so no fixture can carry a
/// position, and the probe's `analyzer: exclude:` list decides by position
/// alone.
///
/// The three files hold the same bytes on purpose. `public_member_api_docs`
/// reports a member only inside a package's `lib/`, and the probe stages every
/// changed file under a `lib/` of its own, so without the exclude list all
/// three would report the same two members. The difference between one file
/// reporting and three reporting is therefore the list and nothing else.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_reads_only_the_package_library() {
    verify_shipped_staged_positions_report(&DART_MISSING_DOCS_POSITIONS_PROBE);
}

/// The materialized name of the `missing-docs-go` fail fixture.
const GO_MISSING_DOCS_FAIL_FIXTURE: &str = "missing-docs-go.fail.go";

/// Where the `missing-docs-go` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const GO_MISSING_DOCS_FIXTURE_PATH: &str = "src/missing_docs_go_fail.go";

/// Every item the `missing-docs-go` fail fixture leaves undocumented, as
/// revive's `exported` rule spells it inside the message it reports.
///
/// Each entry carries the KIND word beside the name, because the message
/// carries it, so an entry states which declaration revive read and not only
/// what it is called.
///
/// The first five hold the five kinds the rule body claims — a type, a method,
/// a function, a constant and a variable.
///
/// `WrongCommentForm` and `OnlyDeprecated` hold the two shapes revive reads as
/// undocumented although a comment stands above them: a doc comment that does
/// not open with the item's own name, and a `Deprecated:` note standing alone.
///
/// The getter and the setter are the load-bearing pair. The `missing-docs`
/// prompt rule carves out "Simple getters/setters with self-explanatory
/// names", and revive takes no option that restores the carve-out:
/// `disableChecksOnMethods` turns off EVERY method check, which is far wider.
/// The rule body states that a getter and a setter each report, and these two
/// entries hold revive to the statement.
const GO_MISSING_DOCS_FAIL_ITEMS: &[&str] = &[
    "type UndocumentedType",
    "method UndocumentedType.UndocumentedMethod",
    "function UndocumentedFunction",
    "const UndocumentedConst",
    "var UndocumentedVar",
    "function WrongCommentForm",
    "method Accessors.Value",
    "method Accessors.SetValue",
    "function OnlyDeprecated",
];

/// The `missing-docs-go` fail fixture, and every undocumented exported item
/// the real revive pipeline must report inside it.
const GO_MISSING_DOCS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: &["go"],
        rule: GO_MISSING_DOCS_RULE,
        expected: GO_MISSING_DOCS_FAIL_ITEMS,
    },
    fixture: GO_MISSING_DOCS_FAIL_FIXTURE,
    path: GO_MISSING_DOCS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "undocumented exported item",
};

/// Acceptance: the shipped Go missing-docs tool rule reports every
/// undocumented exported item its fail fixture holds, through the real revive
/// pipeline.
///
/// An item is held to the CLAIM its finding carries, because revive spells the
/// kind and the name inside the message it reports.
///
/// The count is the other half, and it is what a silent run cannot fake. The
/// pass fixture holds six undocumented methods revive carves out by name, so a
/// run that reported one of them would fail the pair; holding this run to
/// exactly these nine states the same silence from the other side.
#[test]
fn the_shipped_go_missing_docs_tool_rule_reports_every_fail_fixture_item() {
    verify_shipped_fail_fixture_reports_each(
        &GO_MISSING_DOCS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an undocumented type, method, function, constant, variable, getter and setter",
                CODE_HYGIENE_SET,
                [
                    MISSING_DOCS_PROMPT_RULE.to_string(),
                    GO_MISSING_DOCS_RULE.to_string(),
                ],
                [(GO_MISSING_DOCS_FIXTURE_PATH, content)],
            )
        },
        |verified, _source| verified.finding.claim.clone(),
        |reported, item| reported.contains(item),
    );
}

/// One undocumented exported type, and one undocumented method on it.
const GO_STAGED_DECLARATIONS: &str = concat!(
    "type StagedType struct{}\n",
    "\n",
    "func (s StagedType) StagedMethod() {}\n"
);

/// The package clause a library file carries.
const GO_STAGED_PACKAGE_CLAUSE: &str = "package staged\n\n";

/// The package clause a command file carries. revive's `exported` rule reads
/// no file of `package main`, because a command exports nothing to a caller
/// outside itself.
const GO_MAIN_PACKAGE_CLAUSE: &str = "package main\n\n";

/// The generated-code header the Go convention defines, with the blank line
/// that separates it from what follows.
///
/// This one line is the whole of what revive reads to know a file is
/// generated. The name of the file says nothing.
const GO_GENERATED_HEADER: &str = "// Code generated by the sah probe. DO NOT EDIT.\n\n";

/// The ordinary position: a library file with no generated header, and a name
/// that is not a test name. This is the one file of the four that must report.
const GO_STAGED_ORDINARY_PATH: &str = "staged.go";

/// The test position. revive's `exported` rule skips a file whose name ends in
/// `_test.go`, and it skips the whole file rather than the test functions in
/// it, so this file must stay silent.
const GO_STAGED_TEST_PATH: &str = "staged_test.go";

/// The generator position. The name carries the protobuf compiler's suffix and
/// the file carries the generated header, so this file must stay silent.
const GO_STAGED_GENERATED_PATH: &str = "staged.pb.go";

/// The command position. It stands in a directory of its own, which is where
/// a Go command stands, so one directory never holds two package names.
const GO_STAGED_MAIN_PATH: &str = "cmd/probe/main.go";

/// The head of the ordinary file and of the test file: the library package
/// clause and nothing else.
const GO_LIBRARY_HEAD: &[&str] = &[GO_STAGED_PACKAGE_CLAUSE];

/// The head of the generated file: the generated header, then the SAME library
/// package clause the two files above carry.
const GO_GENERATED_HEAD: &[&str] = &[GO_GENERATED_HEADER, GO_STAGED_PACKAGE_CLAUSE];

/// The head of the command file: the `main` package clause alone.
const GO_MAIN_HEAD: &[&str] = &[GO_MAIN_PACKAGE_CLAUSE];

/// Each position the staged type is written to, with the head that file
/// carries above the shared declarations.
///
/// The ordinary file and the test file hold the same bytes, so their NAMES are
/// the only difference. The generated file adds the header line and nothing
/// else, so that LINE is its only difference. The command file changes the
/// package clause and nothing else, so that CLAUSE is its only difference.
const GO_STAGED_POSITIONS: &[ShippedStagedFile] = &[
    ShippedStagedFile {
        path: GO_STAGED_ORDINARY_PATH,
        head: GO_LIBRARY_HEAD,
    },
    ShippedStagedFile {
        path: GO_STAGED_TEST_PATH,
        head: GO_LIBRARY_HEAD,
    },
    ShippedStagedFile {
        path: GO_STAGED_GENERATED_PATH,
        head: GO_GENERATED_HEAD,
    },
    ShippedStagedFile {
        path: GO_STAGED_MAIN_PATH,
        head: GO_MAIN_HEAD,
    },
];

/// The file of each finding the Go run must report: the ordinary file, once
/// for its type and once for its method.
const GO_STAGED_REPORTED: &[&str] = &[GO_STAGED_ORDINARY_PATH, GO_STAGED_ORDINARY_PATH];

/// The staged Go positions, and the one of them the real revive pipeline must
/// report.
const GO_MISSING_DOCS_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: &["go"],
        rule: GO_MISSING_DOCS_RULE,
        expected: GO_STAGED_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented exported type, staged in four positions",
    declarations: GO_STAGED_DECLARATIONS,
    staged: GO_STAGED_POSITIONS,
    support: NO_SUPPORT_FILES,
    reason: "the ordinary library file reports its type and its method, and the \
             test file, the generated file and the command file report nothing",
};

/// Acceptance: the shipped Go missing-docs tool rule reports the ordinary
/// library file and stays silent on the test file, the generated file and the
/// command file, through the real revive pipeline.
///
/// This is the half the fixture pair cannot reach. The doctor materializes one
/// fixture as a loose file under a name of its own, in a package of its own, so
/// no fixture can carry a test name, a generated header, or a `main` package
/// clause.
///
/// Each of the three carve-outs is a DEFAULT of revive, and a default is what a
/// later edit can take away. `ignoreGeneratedHeader = true` makes revive ignore
/// the header rather than honour it, so a config that states it reports every
/// exported item of every generated file. This test fails the moment the config
/// states it.
#[test]
fn the_shipped_go_missing_docs_tool_rule_reads_neither_a_generated_a_test_nor_a_command_file() {
    verify_shipped_staged_positions_report(&GO_MISSING_DOCS_POSITIONS_PROBE);
}

/// The detected project type a Go workspace carries, as the match context
/// holds it.
const GO_PROJECT_TYPE: &str = "go";

/// Every shipped rule that reads a `.go` file, as `<set>/<rule>` and sorted.
///
/// The list is what the matcher SELECTS, before any rule supersedes another,
/// so `code-hygiene/missing-docs` stands here beside the tool rule that
/// replaces it. Half the list carries no file criteria at all, which is how a
/// set-wide rule reads every language.
///
/// The list is here to hold one sentence of the `missing-docs-go` rule body:
/// no shipped rule owns a stuttering Go NAME. `missing-docs-go` turns revive's
/// stuttering check off, and no other rule reads a Go name, so the defect has
/// no owner today. Card ^6jzgb8v carries the gap.
///
/// A rule added to any set above fails this test. Read the new rule then: if
/// it owns a Go name, correct the `missing-docs-go` rule body and card
/// ^6jzgb8v with it. If it does not, add its name here.
const SHIPPED_RULES_THAT_READ_A_GO_FILE: &[&str] = &[
    "code-hygiene/cognitive-complexity",
    "code-hygiene/complexity-go",
    "code-hygiene/data-driven",
    "code-hygiene/dead-code",
    "code-hygiene/function-length",
    "code-hygiene/function-length-go",
    "code-hygiene/magic-numbers",
    "code-hygiene/magic-numbers-go",
    "code-hygiene/missing-docs",
    "code-hygiene/missing-docs-go",
    "code-hygiene/no-commented-code",
    "code-hygiene/unused-code-go",
    "code-security/command-safety",
    "code-security/injection",
    "code-security/no-secrets",
    "completeness/case-sensitivity-coverage",
    "completeness/invariant-propagation",
    "completeness/inverse-operation-coverage",
    "completeness/public-output-contract",
    "duplication/duplication",
    "duplication/rust",
    "duplication/swift",
    "reuse/reuse",
    "test-integrity/no-hard-code",
    "test-integrity/no-test-cheating",
];

/// Acceptance: the shipped rules that read a `.go` file are exactly the ones
/// [`SHIPPED_RULES_THAT_READ_A_GO_FILE`] names.
///
/// The `missing-docs-go` rule body states that no shipped rule owns a
/// stuttering Go name. That sentence is about every rule and not about one, so
/// only an enumeration can hold it. The enumeration runs the real matcher over
/// a `.go` path in a Go workspace, which is the same question the review scope
/// stage asks.
#[test]
fn the_shipped_rules_that_read_a_go_file_stay_the_stated_list() {
    let loader = builtin_loader();
    let context = MatchContext::new()
        .with_file(GO_STAGED_ORDINARY_PATH)
        .with_project_types([GO_PROJECT_TYPE.to_string()]);

    let mut reading: Vec<String> = Vec::new();
    for ruleset in loader.list_rulesets() {
        for rule in &ruleset.rules {
            if rule.matches(ruleset, &context) {
                reading.push(format!("{}/{}", ruleset.name(), rule.name));
            }
        }
    }
    reading.sort();

    assert_eq!(
        reading, SHIPPED_RULES_THAT_READ_A_GO_FILE,
        "the rules that read a Go file moved; a rule that owns a Go NAME makes \
         the `missing-docs-go` rule body wrong, because that body states the \
         stuttering name has no owner"
    );
}

/// A Go file that does not parse: the parameter list of `Broken` never closes.
const GO_UNPARSABLE_SOURCE: &str = concat!("package staged\n", "\n", "func Broken( {\n");

/// Where the unparsable file stands inside the probe repository.
const GO_UNPARSABLE_PATH: &str = "broken.go";

/// What revive puts at the front of the failure it writes for a file it could
/// not parse. The run's error detail must carry it, so the agent reading the
/// error learns which file broke.
const GO_INVALID_FILE_PREFIX: &str = "invalid file";

/// What the one error of an unparsable Go file must name.
const GO_UNPARSABLE_ERROR: &[&str] = &[GO_INVALID_FILE_PREFIX, GO_UNPARSABLE_PATH];

/// The `missing-docs-go` probe over a Go file revive cannot parse.
const GO_UNPARSABLE_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: &["go"],
        rule: GO_MISSING_DOCS_RULE,
        expected: GO_UNPARSABLE_ERROR,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "a Go file the parser cannot read",
    path: GO_UNPARSABLE_PATH,
    source: Some(GO_UNPARSABLE_SOURCE.as_bytes()),
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Go missing-docs tool rule BREAKS on a Go file it
/// cannot parse, through the real revive pipeline.
///
/// revive exits 0 for such a file and reports the failure with an empty
/// `RuleName`, under the `validity` category rather than under `exported`. A
/// pipe that selected the `exported` findings alone therefore dropped it, and
/// the file read as clean — a run answering zero for a reason other than a
/// clean file. The script counts the failures that belong to no rule, writes
/// each one to stderr, and exits nonzero.
#[test]
fn the_shipped_go_missing_docs_tool_rule_breaks_on_a_file_it_cannot_parse() {
    verify_shipped_run_breaks(&GO_UNPARSABLE_PROBE);
}

/// The materialized name of the `missing-docs-python` fail fixture.
const PYTHON_MISSING_DOCS_FAIL_FIXTURE: &str = "missing-docs-python.fail.py";

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
        project_types: &["python"],
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
        project_types: &["python"],
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

/// The code ruff writes for a Python file it cannot parse. The run's error
/// detail must carry it, so the agent reading the error learns what broke.
const PYTHON_INVALID_SYNTAX_CODE: &str = "invalid-syntax";

/// What the one error of an unparsable file must name.
const PYTHON_UNPARSABLE_ERROR: &[&str] = &[PYTHON_INVALID_SYNTAX_CODE, PYTHON_UNPARSABLE_PATH];

/// The `missing-docs-python` probe over a Python file ruff cannot parse.
const PYTHON_UNPARSABLE_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: &["python"],
        rule: PYTHON_MISSING_DOCS_RULE,
        expected: PYTHON_UNPARSABLE_ERROR,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "a Python file the parser cannot read",
    path: PYTHON_UNPARSABLE_PATH,
    source: Some(PYTHON_UNPARSABLE_SOURCE.as_bytes()),
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Python missing-docs tool rule BREAKS on a Python
/// file it cannot parse, through the real ruff pipeline.
///
/// ruff reports such a file under the code `invalid-syntax`, beside the codes
/// the rule selects. A filter that selected the seven documentation codes alone
/// dropped that record, and the file read as clean — a run answering zero for a
/// reason other than a clean file. The script counts each record outside the
/// seven codes, writes each one to stderr, and exits nonzero.
#[test]
fn the_shipped_python_missing_docs_tool_rule_breaks_on_a_file_it_cannot_parse() {
    verify_shipped_run_breaks(&PYTHON_UNPARSABLE_PROBE);
}

/// Where the file that is never written stands inside the probe repository.
const PYTHON_ABSENT_PATH: &str = "absent.py";

/// What the script writes for a file it cannot read.
const PYTHON_CANNOT_READ_MESSAGE: &str = "missing-docs-python cannot read";

/// What the one error of an absent file must name.
const PYTHON_ABSENT_ERROR: &[&str] = &[PYTHON_CANNOT_READ_MESSAGE, PYTHON_ABSENT_PATH];

/// The `missing-docs-python` probe over a path that holds no file.
const PYTHON_ABSENT_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: &["python"],
        rule: PYTHON_MISSING_DOCS_RULE,
        expected: PYTHON_ABSENT_ERROR,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "a Python file that is not there",
    path: PYTHON_ABSENT_PATH,
    source: None,
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Python missing-docs tool rule BREAKS on a file it
/// cannot read, through the real ruff pipeline.
///
/// ruff answers a path that is not there with an empty report and an exit status
/// of 0, and it puts the failure on stderr alone. That report reads exactly like
/// a clean file. The script therefore tests each file it is given before it
/// starts, and exits nonzero with the name of the file it cannot read.
#[test]
fn the_shipped_python_missing_docs_tool_rule_breaks_on_a_file_it_cannot_read() {
    verify_shipped_run_breaks(&PYTHON_ABSENT_PROBE);
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
const PYTHON_READ_FINDINGS: &[&str] = &[
    "deep/nested/other.py:1",
    "deep/nested/other.py:1",
    "deep/nested/other.py:2",
    "top.py:1",
    "top.py:1",
];

/// The `missing-docs-python` probe over a run that is given no file.
const PYTHON_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: &["python"],
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

/// The materialized name of the `missing-docs-swift` fail fixture.
const SWIFT_MISSING_DOCS_FAIL_FIXTURE: &str = "missing-docs-swift.fail.swift";

/// Where the `missing-docs-swift` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const SWIFT_MISSING_DOCS_FIXTURE_PATH: &str = "Sources/MissingDocsSwiftFail.swift";

/// Every declaration the `missing-docs-swift` fail fixture leaves
/// undocumented, trimmed as the fixture writes it.
///
/// A line, and not a claim, because `missing_docs` writes one message —
/// `public declarations should be documented` — for every declaration, so the
/// claim never spells which one it read.
const SWIFT_MISSING_DOCS_FAIL_LINES: &[&str] = &[
    "public struct UndocumentedStructure {",
    "public func undocumentedMethod() {}",
    "public func undocumentedFunction() {}",
];

/// The `missing-docs-swift` fail fixture, and every undocumented declaration
/// the real swiftlint pipeline must report inside it.
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
/// undocumented declaration its fail fixture holds, through the real swiftlint
/// pipeline.
///
/// A declaration is held to the SOURCE LINE its finding stands on, because
/// `missing_docs` writes one message for every declaration and never spells
/// the declaration it read.
///
/// The count is the other half, and it is what a silent run cannot fake. The
/// script writes its own configuration and names the project's own
/// `.swiftlint.yml` beside it, and a run that reached neither would report
/// zero findings. Holding this run to exactly these three lines states that
/// swiftlint read the configuration the script wrote and reported each kind
/// the fixture holds.
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

/// One undocumented public structure, and one undocumented stored property
/// inside it.
///
/// Every staged position holds these same bytes, so the POSITION is the only
/// thing that can tell one file of the run from another.
const SWIFT_STAGED_DECLARATIONS: &str = concat!(
    "public struct StagedThing {\n",
    "    public var value: Int = 0\n",
    "}\n",
);

/// The file of each finding the Swift run must report: the ordinary file, once
/// for its structure and once for its stored property.
const SWIFT_STAGED_REPORTED: &[&str] =
    &[SWIFT_ORDINARY_POSITION.path, SWIFT_ORDINARY_POSITION.path];

/// The staged Swift positions, and the one of them the real swiftlint pipeline
/// must report.
const SWIFT_MISSING_DOCS_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_STAGED_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented public structure, staged in two positions",
    declarations: SWIFT_STAGED_DECLARATIONS,
    staged: SWIFT_EXCLUDE_POSITIONS,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the ordinary file reports its structure and its stored property, and the \
             file under the project's excluded directory reports nothing",
};

/// Acceptance: the shipped Swift missing-docs tool rule honours the project's
/// own `excluded:` list, through the real swiftlint pipeline.
///
/// This is the half the fixture pair cannot reach. The doctor materializes one
/// fixture as a loose file with no directory, so no fixture can carry a
/// position and no fixture can stand beside a project configuration.
///
/// The two files hold the same bytes on purpose. The project's `excluded:`
/// list is the only difference between the file that reports and the file that
/// stays silent. swiftlint applies that list to a file named as a command-line
/// argument only under `--force-exclude`, so this test fails the moment the
/// script drops the flag or stops naming the project configuration.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reads_the_project_exclude_list() {
    verify_shipped_staged_positions_report(&SWIFT_MISSING_DOCS_POSITIONS_PROBE);
}

/// The `missing-docs-swift` probe over a run whose every file the project's
/// `excluded:` list names.
const SWIFT_EVERY_FILE_EXCLUDED_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: NO_STAGED_REPORTS,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented public structure under the project's excluded directory",
    declarations: SWIFT_STAGED_DECLARATIONS,
    staged: SWIFT_EXCLUDED_POSITION_ONLY,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the project excludes every file of the run, so the run reports nothing and \
             breaks nothing",
};

/// Acceptance: the shipped Swift missing-docs tool rule reports nothing, and
/// breaks nothing, when the project excludes every file of the run, through
/// the real swiftlint pipeline.
///
/// swiftlint exits 1 with `Error: No lintable files found at paths` when
/// `--force-exclude` leaves it no file to read. That status reads as a broken
/// tool, so a run over a change that touched generated code alone would report
/// a tool error rather than a clean answer. The script tests each file it is
/// given for readability first, so the message can mean one thing only, and it
/// then exits 0 with no finding.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_answers_zero_when_the_project_excludes_every_file() {
    verify_shipped_staged_positions_report(&SWIFT_EVERY_FILE_EXCLUDED_PROBE);
}

/// The file of each finding the `child_config:` probe must report: the file
/// under the project's excluded directory, once for its structure and once for
/// its stored property.
///
/// The project excludes that directory, and the run drops that exclude list,
/// so the file reports.
const SWIFT_CHILD_CONFIG_REPORTED: &[&str] =
    &[SWIFT_GENERATED_POSITION.path, SWIFT_GENERATED_POSITION.path];

/// The `missing-docs-swift` probe beside a project that names a child
/// configuration of its own.
const SWIFT_CHILD_CONFIG_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_CHILD_CONFIG_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented public structure beside a project child configuration",
    declarations: SWIFT_STAGED_DECLARATIONS,
    staged: SWIFT_EXCLUDED_POSITION_ONLY,
    support: SWIFT_CHILD_CONFIG_SUPPORT_FILES,
    reason: "swiftlint cannot read that project configuration beside the rule's own, so the run \
             measures with the rule's configuration alone and reports the staged declarations",
};

/// Acceptance: the shipped Swift missing-docs tool rule measures beside a
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
fn the_shipped_swift_missing_docs_tool_rule_measures_beside_a_project_child_config() {
    verify_shipped_staged_positions_report(&SWIFT_CHILD_CONFIG_PROBE);
}

/// The `missing-docs-swift` probe beside a project that states a warning
/// threshold of one finding.
const SWIFT_WARNING_THRESHOLD_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_STAGED_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented public structure beside a project warning threshold",
    declarations: SWIFT_STAGED_DECLARATIONS,
    staged: SWIFT_ORDINARY_POSITION_ONLY,
    support: SWIFT_WARNING_THRESHOLD_SUPPORT_FILES,
    reason: "the threshold makes swiftlint exit 2 with the whole report on stdout, and the \
             script reads that status as a measured run, so the staged declarations report",
};

/// Acceptance: the shipped Swift missing-docs tool rule measures beside a
/// project that states `warning_threshold:`, through the real swiftlint
/// pipeline.
///
/// Measured with swiftlint 0.65.0 over the staged declarations, with
/// `warning_threshold: 1` in the project configuration: swiftlint writes 3
/// entries to stdout — the 2 `missing_docs` findings and one
/// `warning_threshold` entry of error severity — and exits 2. The script read
/// each nonzero status as a broken tool and reported 0 findings, so one line
/// in the project file switched the gate off.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_measures_beside_a_project_warning_threshold() {
    verify_shipped_staged_positions_report(&SWIFT_WARNING_THRESHOLD_PROBE);
}

/// The `missing-docs-swift` probe beside a project that names a swiftlint
/// version that is not installed.
const SWIFT_VERSION_MISMATCH_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_VERSION_MISMATCH_ERROR,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "undocumented public declarations beside a project version mismatch",
    path: SWIFT_ORDINARY_POSITION.path,
    source: Some(SWIFT_STAGED_DECLARATIONS.as_bytes()),
    support: SWIFT_VERSION_MISMATCH_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift missing-docs tool rule BREAKS beside a
/// project that names a swiftlint version that is not installed, through the
/// real swiftlint pipeline.
///
/// swiftlint compares `swiftlint_version:` with the version it is. At a
/// difference it writes one warning line to stderr, writes 0 bytes to stdout,
/// runs no lint, and exits 2. Measured with swiftlint 0.65.0 over the staged
/// declarations: a run with no project configuration reports 2 findings, and a
/// run beside `swiftlint_version: 99.0.0` reports 0. A script that reads every
/// status 2 as a measured run hands `jq` an empty report, reports 0 findings
/// and exits 0, so the engine reads a dirty file as clean. The script accepts
/// status 2 only when the report holds a JSON array of one entry or more.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_breaks_beside_a_project_version_mismatch() {
    verify_shipped_run_breaks(&SWIFT_VERSION_MISMATCH_PROBE);
}

/// One undocumented type that declares an inherited type, and one that
/// declares none, each holding one undocumented stored property.
///
/// `excludes_inherited_types: true` is what keeps `Wide` and its property
/// silent, and `warning: [open, public]` is what makes `Plain` and its
/// property report. The two shapes stand together so one run measures both.
const SWIFT_OPTION_DECLARATIONS: &str = concat!(
    "public struct Wide: Equatable {\n",
    "    public var name: String = \"\"\n",
    "}\n",
    "\n",
    "public struct Plain {\n",
    "    public var value: Int = 0\n",
    "}\n",
);

/// A project `.swiftlint.yml` that switches the rule off and states other
/// options for it.
///
/// Each of the three settings changes the answer on its own: `disabled_rules`
/// switches `missing_docs` off, `warning: [open]` drops every `public`
/// declaration, and `excludes_inherited_types: false` adds the two rows of
/// `Wide`.
const SWIFT_OVERRIDING_PROJECT_CONFIG: &str = concat!(
    "disabled_rules:\n",
    "  - missing_docs\n",
    "missing_docs:\n",
    "  warning: [open]\n",
    "  excludes_inherited_types: false\n",
);

/// The overriding project configuration staged beside the two shapes, which
/// the work-list does NOT name.
const SWIFT_OVERRIDING_SUPPORT_FILES: &[(&str, &str)] =
    &[(SWIFT_PROJECT_CONFIG_PATH, SWIFT_OVERRIDING_PROJECT_CONFIG)];

/// The file of each finding the run must report: the staged file, once for
/// `Plain` and once for its stored property.
const SWIFT_OPTION_REPORTED: &[&str] =
    &[SWIFT_ORDINARY_POSITION.path, SWIFT_ORDINARY_POSITION.path];

/// The `missing-docs-swift` probe over a project that states other options for
/// the rule.
const SWIFT_RULE_OPTIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_OPTION_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented type with an inherited type and one without",
    declarations: SWIFT_OPTION_DECLARATIONS,
    staged: SWIFT_ORDINARY_POSITION_ONLY,
    support: SWIFT_OVERRIDING_SUPPORT_FILES,
    reason: "the rule's own options decide: the type with no inherited type reports its \
             declaration and its property, and the type with one reports nothing",
};

/// Acceptance: the shipped Swift missing-docs tool rule keeps its own rule
/// options against a project that states other ones, through the real
/// swiftlint pipeline.
///
/// The script names the project's `.swiftlint.yml` as the PARENT of its own
/// configuration, so the project decides which files are read. It must not
/// decide what the rule measures. The script's own configuration states every
/// `missing_docs` option, and a child block replaces the parent's block whole.
///
/// Each setting in the staged project configuration moves the count on its
/// own, so this run tells the three apart: 0 findings if the project switched
/// the rule off or dropped `public`, and 4 if it widened the rule to a type
/// that declares an inherited type.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_keeps_its_own_rule_options() {
    verify_shipped_staged_positions_report(&SWIFT_RULE_OPTIONS_PROBE);
}

/// Where the Swift file that is never written stands inside the probe
/// repository.
const SWIFT_ABSENT_PATH: &str = "Sources/Absent.swift";

/// What the script writes for a file it cannot read.
const SWIFT_CANNOT_READ_MESSAGE: &str = "missing-docs-swift cannot read";

/// What the one error of an absent file must name.
const SWIFT_ABSENT_ERROR: &[&str] = &[SWIFT_CANNOT_READ_MESSAGE, SWIFT_ABSENT_PATH];

/// The `missing-docs-swift` probe over a path that holds no file.
const SWIFT_ABSENT_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_ABSENT_ERROR,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "a Swift file that is not there",
    path: SWIFT_ABSENT_PATH,
    source: None,
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift missing-docs tool rule BREAKS on a file it
/// cannot read, through the real swiftlint pipeline.
///
/// swiftlint exits 1 for a path that is not there and writes nothing to
/// stdout. A pipeline takes the exit status of its LAST command, and that
/// command was `jq`, so the earlier pipe exited 0 and reported nothing — a run
/// answering zero for a reason other than a clean file. The script tests each
/// file it is given before it starts, and exits 1 with the name of the file it
/// cannot read.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_breaks_on_a_file_it_cannot_read() {
    verify_shipped_run_breaks(&SWIFT_ABSENT_PROBE);
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
fn the_shipped_swift_missing_docs_tool_rule_stays_clean_over_a_hollow_directory() {
    verify_shipped_hollow_directory_answers_clean(&SWIFT_HOLLOW_PROBE);
}

/// An undocumented Swift file at the root of the probe repository.
const SWIFT_UNREAD_TOP_SOURCE: &str = "public struct Top {\n    public var value: Int = 0\n}\n";

/// The same, nested three directories deep. swiftlint walks a whole tree, so a
/// default target reaches this file as readily as the one at the root.
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
const SWIFT_READ_FINDINGS: &[&str] = &[
    "deep/nested/Other.swift:1",
    "deep/nested/Other.swift:2",
    "Top.swift:1",
    "Top.swift:2",
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
/// it is given, through the real swiftlint pipeline.
///
/// `swiftlint lint` with no path argument falls back to a default target of
/// the working directory, and it walks that whole tree. A script that hands
/// `"$@"` straight to swiftlint therefore answers for every Swift file under
/// the repository root when the run carries no file, and it exits 0, so the
/// answer reads as a measured result rather than a mistake.
///
/// The script therefore answers an empty argument list at once, with no
/// finding and an exit status of 0. The same script over the two staged files
/// reports 4.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&SWIFT_EMPTY_RUN_PROBE);
}

/// A TypeScript function that carries no JSDoc comment.
/// `jsdoc/require-jsdoc` reports the declaration, so each file holds one
/// finding.
const TYPESCRIPT_MISSING_DOCS_UNREAD_SOURCE: &str = r#"export function undocumented(value: number): number {
  return value;
}
"#;

/// Every TypeScript file staged in the probe repository the missing-docs
/// script is given none of.
const TYPESCRIPT_MISSING_DOCS_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.ts", TYPESCRIPT_MISSING_DOCS_UNREAD_SOURCE),
    (
        "deep/nested/other.ts",
        TYPESCRIPT_MISSING_DOCS_UNREAD_SOURCE,
    ),
];

/// Each finding the TypeScript missing-docs script reports over the two files
/// it is given, as `path:line`.
const TYPESCRIPT_MISSING_DOCS_READ_FINDINGS: &[&str] = &["deep/nested/other.ts:1", "top.ts:1"];

/// The `missing-docs-typescript` probe over a run that is given no file.
const TYPESCRIPT_MISSING_DOCS_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    staged: TYPESCRIPT_MISSING_DOCS_UNREAD_FILES,
    with_files: TYPESCRIPT_MISSING_DOCS_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped TypeScript missing-docs tool rule reads only the
/// files it is given, through the real eslint pipeline.
///
/// eslint with no path argument reads the working directory, and the config
/// this rule writes names `**/*.{js,jsx,mjs,cjs,ts,tsx}`. Measured over this
/// probe with no argument: without the guard the script reported 2 findings
/// and exited 0; with the guard it reports none and exits 0. The same script
/// over the two staged files reports 2.
#[test]
fn the_shipped_typescript_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&TYPESCRIPT_MISSING_DOCS_EMPTY_RUN_PROBE);
}

/// An exported Go function that carries no doc comment. revive's `exported`
/// rule reports the declaration, so each file holds one finding.
const GO_MISSING_DOCS_UNREAD_SOURCE: &str = r#"package probe

func Exported() int {
    return 0
}
"#;

/// Every Go file staged in the probe repository the missing-docs script is
/// given none of.
///
/// revive reads the package in the working directory, so the file at the root
/// stands inside its default target and the nested file stands outside it.
const GO_MISSING_DOCS_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.go", GO_MISSING_DOCS_UNREAD_SOURCE),
    ("deep/nested/other.go", GO_MISSING_DOCS_UNREAD_SOURCE),
];

/// Each finding the Go missing-docs script reports over the two files it is
/// given, as `path:line`.
const GO_MISSING_DOCS_READ_FINDINGS: &[&str] = &["top.go:3", "deep/nested/other.go:3"];

/// The `missing-docs-go` probe over a run that is given no file.
const GO_MISSING_DOCS_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    staged: GO_MISSING_DOCS_UNREAD_FILES,
    with_files: GO_MISSING_DOCS_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Go missing-docs tool rule reads only the files it
/// is given, through the real revive pipeline.
///
/// revive with no path argument reads the package standing in the working
/// directory. Measured over this probe with no argument: without the guard
/// the script reported 1 finding, on `top.go`, and exited 0; with the guard
/// it reports none and exits 0. The same script over the two staged files
/// reports 2, so the guard is the whole difference and the nested file is
/// what the default target leaves out.
#[test]
fn the_shipped_go_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&GO_MISSING_DOCS_EMPTY_RUN_PROBE);
}

/// A Dart class and method that carry no documentation comment.
/// `public_member_api_docs` reports each of the two, so each file holds two
/// findings.
const DART_MISSING_DOCS_UNREAD_SOURCE: &str = r#"class Widget {
  int gate(int value) {
    return value;
  }
}
"#;

/// Every Dart file staged in the probe repository the missing-docs script is
/// given none of.
const DART_MISSING_DOCS_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.dart", DART_MISSING_DOCS_UNREAD_SOURCE),
    ("deep/nested/other.dart", DART_MISSING_DOCS_UNREAD_SOURCE),
];

/// Each finding the Dart missing-docs script reports over the two files it is
/// given, as `path:line`.
const DART_MISSING_DOCS_READ_FINDINGS: &[&str] = &[
    "deep/nested/other.dart:1",
    "deep/nested/other.dart:2",
    "top.dart:1",
    "top.dart:2",
];

/// The `missing-docs-dart` probe over a run that is given no file.
const DART_MISSING_DOCS_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    staged: DART_MISSING_DOCS_UNREAD_FILES,
    with_files: DART_MISSING_DOCS_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Dart missing-docs tool rule reads only the files
/// it is given, through the real `dart analyze` pipeline.
///
/// This script names the package it makes as the one path `dart analyze`
/// reads, and it copies each file it is given under that package. A run with
/// no argument therefore hands the tool a package holding no Dart file.
/// Measured over this probe with no argument: the script reported 0 findings
/// and exited 0 both without the guard and with it, and the same script over
/// the two staged files reports 4. The guard is what keeps the script from
/// making that package and running the analyzer over it.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&DART_MISSING_DOCS_EMPTY_RUN_PROBE);
}
