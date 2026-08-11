//! Acceptance tests for the shipped missing-docs tool rules.
//!
//! One test holds the whole roster to its fixture pair and to the prompt rule
//! it supersedes. The tests under it drive one language each through its real
//! tool, so each measures the shipped script rather than a copy.

use super::*;

/// Acceptance: every shipped missing-docs tool rule passes its fixture pair
/// in doctor, and supersedes the `missing-docs` prompt rule.
///
/// A tool that reads the whole public surface answers the documentation
/// question the prompt rule asks, so it replaces it for the files it covers.
/// [`verify_shipped_tool_rules_pass_fixtures`] carries the rest of the
/// contract, including what a machine without the tool proves.
#[test]
fn every_shipped_missing_docs_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(SHIPPED_MISSING_DOCS_RULES, MISSING_DOCS_PROMPT_RULE);
}

/// Acceptance: the shipped Rust tool rule reports an undocumented public
/// item on a real cargo workspace, through the real clippy pipeline.
///
/// No LLM reads the pair: the rule plans healthy, so the `missing-docs`
/// prompt rule is suppressed for the file, and the finding comes from the
/// script's stdout — [`execute_tool_runs`] never reaches an agent.
#[test]
fn the_shipped_rust_tool_rule_reports_an_undocumented_public_item() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(
        repo.path().join("Cargo.toml"),
        UNDOCUMENTED_PACKAGE_MANIFEST,
    )
    .unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(repo.path().join(UNDOCUMENTED_LIB_PATH), UNDOCUMENTED_LIB_RS).unwrap();
    let loader = builtin_loader();
    let project_types = ["rust"];
    require_tool_installed(&loader, &project_types, RUST_MISSING_DOCS_RULE);
    let work = code_hygiene_work(&[UNDOCUMENTED_LIB_PATH]);

    let plan = plan_tool_rules(&work, &loader, &project_types, None);

    let run = required_run(&plan, RUST_MISSING_DOCS_RULE);
    assert_eq!(run.files(), [UNDOCUMENTED_LIB_PATH.to_string()]);
    assert!(
        plan.suppression()
            .suppressed_rules(CODE_HYGIENE_SET, UNDOCUMENTED_LIB_PATH)
            .contains(MISSING_DOCS_PROMPT_RULE),
        "a healthy tool rule must suppress the prompt rule, so no LLM reads the pair"
    );

    verify_run_reports_one_finding(
        run,
        repo.path(),
        UNDOCUMENTED_LIB_PATH,
        CODE_HYGIENE_SET,
        RUST_MISSING_DOCS_RULE,
        "missing documentation",
    );
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
    "code-hygiene/no-commented-code-parsed",
    "code-hygiene/unused-code-go",
    "code-security/command-safety",
    "code-security/injection",
    "code-security/no-secrets",
    "completeness/case-sensitivity-coverage",
    "completeness/invariant-propagation",
    "completeness/inverse-operation-coverage",
    "completeness/public-output-contract",
    "duplication/duplication",
    "duplication/duplication-parsed",
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
const GO_UNPARSABLE_PROBE: ShippedBrokenRun = ShippedBrokenRun {
    run: ShippedRun {
        project_types: &["go"],
        rule: GO_MISSING_DOCS_RULE,
        expected: GO_UNPARSABLE_ERROR,
    },
    change_purpose: "a Go file the parser cannot read",
    path: GO_UNPARSABLE_PATH,
    source: Some(GO_UNPARSABLE_SOURCE),
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
const PYTHON_UNPARSABLE_PROBE: ShippedBrokenRun = ShippedBrokenRun {
    run: ShippedRun {
        project_types: &["python"],
        rule: PYTHON_MISSING_DOCS_RULE,
        expected: PYTHON_UNPARSABLE_ERROR,
    },
    change_purpose: "a Python file the parser cannot read",
    path: PYTHON_UNPARSABLE_PATH,
    source: Some(PYTHON_UNPARSABLE_SOURCE),
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
const PYTHON_ABSENT_PROBE: ShippedBrokenRun = ShippedBrokenRun {
    run: ShippedRun {
        project_types: &["python"],
        rule: PYTHON_MISSING_DOCS_RULE,
        expected: PYTHON_ABSENT_ERROR,
    },
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

/// What a run given no file must report: nothing.
const NO_FINDINGS: &[&str] = &[];

/// The `missing-docs-python` probe over a run that is given no file.
const PYTHON_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: &["python"],
        rule: PYTHON_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    staged: PYTHON_UNREAD_FILES,
    reason: "the script judges the files it is given and no other: given none, it reports none \
             and exits 0, and the staged tree stays unread",
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
/// finding and an exit status of 0.
#[test]
fn the_shipped_python_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&PYTHON_EMPTY_RUN_PROBE);
}

/// The manifest of the root package of the Rust workspace probe.
///
/// It names `shared` as a dependency AND as a build-dependency, so cargo
/// compiles `shared` two times and clippy writes its finding two times.
/// `lonely` is a member no package depends on, so cargo builds it only when
/// the command selects the whole workspace.
const RUST_WORKSPACE_ROOT_MANIFEST: &str = concat!(
    "[package]\nname = \"workspace-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[dependencies]\nshared = { path = \"shared\" }\n",
    "\n[build-dependencies]\nshared = { path = \"shared\" }\n",
    "\n[workspace]\nmembers = [\"shared\", \"lonely\"]\n",
);

/// The build script of the Rust workspace probe. It carries a crate comment,
/// because `missing_docs` asks each compiled target for one.
const RUST_WORKSPACE_BUILD_RS: &str =
    "//! The build script of the workspace probe.\n\nfn main() {}\n";

/// The manifest of the `shared` member of the Rust workspace probe.
const RUST_WORKSPACE_SHARED_MANIFEST: &str =
    "[package]\nname = \"shared\"\nversion = \"0.0.0\"\nedition = \"2021\"\n";

/// The manifest of the `lonely` member of the Rust workspace probe.
const RUST_WORKSPACE_LONELY_MANIFEST: &str =
    "[package]\nname = \"lonely\"\nversion = \"0.0.0\"\nedition = \"2021\"\n";

/// Every file of the Rust workspace probe the work-list does not name.
const RUST_WORKSPACE_SUPPORT_FILES: &[(&str, &str)] = &[
    ("Cargo.toml", RUST_WORKSPACE_ROOT_MANIFEST),
    ("build.rs", RUST_WORKSPACE_BUILD_RS),
    ("shared/Cargo.toml", RUST_WORKSPACE_SHARED_MANIFEST),
    ("lonely/Cargo.toml", RUST_WORKSPACE_LONELY_MANIFEST),
];

/// The library of the root package of the Rust workspace probe.
const RUST_WORKSPACE_ROOT_LIB_PATH: &str = "src/lib.rs";

/// The library of the `shared` member, the one cargo compiles two times.
const RUST_WORKSPACE_SHARED_LIB_PATH: &str = "shared/src/lib.rs";

/// The library of the `lonely` member, the one no package depends on.
const RUST_WORKSPACE_LONELY_LIB_PATH: &str = "lonely/src/lib.rs";

/// The one undocumented declaration every library of the Rust workspace probe
/// holds.
const RUST_WORKSPACE_DECLARATIONS: &str = "pub struct Undocumented;\n";

/// The three libraries of the Rust workspace probe. Each carries a crate
/// comment of its own, so the only undocumented item is the shared
/// declaration.
const RUST_WORKSPACE_STAGED_FILES: &[ShippedStagedFile] = &[
    ShippedStagedFile {
        path: RUST_WORKSPACE_ROOT_LIB_PATH,
        head: &["//! The root package of the workspace probe.\n\n"],
    },
    ShippedStagedFile {
        path: RUST_WORKSPACE_SHARED_LIB_PATH,
        head: &["//! The shared member of the workspace probe.\n\n"],
    },
    ShippedStagedFile {
        path: RUST_WORKSPACE_LONELY_LIB_PATH,
        head: &["//! The lonely member of the workspace probe.\n\n"],
    },
];

/// Each library of the Rust workspace probe, one time, in the order `sort -u`
/// leaves them.
///
/// `lonely/src/lib.rs` is what `--workspace` buys: cargo builds no member
/// nothing depends on, so a command without the flag never reads it.
/// `shared/src/lib.rs` standing one time is what `sort -u` buys: cargo
/// compiles that member two times, and clippy writes its finding two times.
const RUST_WORKSPACE_REPORTS: &[&str] = &[
    RUST_WORKSPACE_LONELY_LIB_PATH,
    RUST_WORKSPACE_SHARED_LIB_PATH,
    RUST_WORKSPACE_ROOT_LIB_PATH,
];

/// The three libraries of the Rust workspace probe, and what the real clippy
/// pipeline must report over them.
const RUST_MISSING_DOCS_WORKSPACE_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: &["rust"],
        rule: RUST_MISSING_DOCS_RULE,
        expected: RUST_WORKSPACE_REPORTS,
    },
    change_purpose: "one undocumented public struct in each package of a workspace",
    declarations: RUST_WORKSPACE_DECLARATIONS,
    staged: RUST_WORKSPACE_STAGED_FILES,
    support: RUST_WORKSPACE_SUPPORT_FILES,
    reason: "the rule declares `scope: workspace`, so the run reads every member one time: the \
             member no package depends on reports, and the member cargo compiles two times \
             reports one finding and not two",
};

/// Acceptance: the shipped Rust missing-docs tool rule reports every member of
/// a workspace, one time each, through the real clippy pipeline.
///
/// Two parts of the command are load-bearing here, and the probe holds both.
/// `--workspace` selects every member; without it cargo builds the package the
/// working directory names and the packages that package depends on, so
/// `lonely/src/lib.rs` stays unread. `sort -u` collapses the repeat; without it
/// `shared/src/lib.rs` arrives two times, because the root package names that
/// member as a dependency and as a build-dependency and cargo therefore
/// compiles it two times.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_reports_every_workspace_member() {
    verify_shipped_staged_positions_report(&RUST_MISSING_DOCS_WORKSPACE_PROBE);
}

/// The manifest of the generated-code probe crate.
const RUST_GENERATED_MANIFEST: &str = concat!(
    "[package]\nname = \"generated-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[workspace]\n",
);

/// The build script of the generated-code probe crate. It writes one
/// undocumented public struct and one undocumented public function into
/// `OUT_DIR`, which is a directory under `target/` that no author edits.
const RUST_GENERATED_BUILD_RS: &str = r#"//! The build script of the generated-code probe.

use std::io::Write;

fn main() {
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let generated = std::path::Path::new(&out).join("generated.rs");
    let mut file = std::fs::File::create(generated).expect("create the generated file");
    writeln!(file, "pub struct GeneratedUndocumented;").expect("write the generated struct");
    writeln!(file, "pub fn generated_undocumented() {{}}").expect("write the generated function");
}
"#;

/// The library of the generated-code probe crate. It reads the generated file
/// with an `include!`, which is how a crate takes code out of `OUT_DIR`, and it
/// holds one undocumented item of its own.
const RUST_GENERATED_LIB_RS: &str = concat!(
    "//! A probe crate for the generated-code step of the shipped Rust rule.\n",
    "\n",
    "include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n",
    "\n",
    "pub struct HandWritten;\n",
);

/// Every file of the generated-code probe crate.
const RUST_GENERATED_PROBE_FILES: &[(&str, &str)] = &[
    ("Cargo.toml", RUST_GENERATED_MANIFEST),
    ("build.rs", RUST_GENERATED_BUILD_RS),
    ("src/lib.rs", RUST_GENERATED_LIB_RS),
];

/// What the shipped script must name over the generated-code probe: the
/// hand-written item, and no generated one.
const RUST_GENERATED_REPORTS: &[&str] = &["src/lib.rs:5"];

/// Acceptance: the shipped Rust missing-docs tool rule names no generated file,
/// through the real clippy pipeline.
///
/// Cargo writes generated code under `OUT_DIR`, and clippy reports an item
/// there with the absolute path of a file the author cannot edit. The
/// `select(.file | startswith("/") | not)` step drops it. Measured over this
/// probe without the step: 3 findings, two of them at an absolute path under
/// `target/`.
///
/// The script's OWN findings are what this test reads, because the engine keeps
/// only the findings in the changed files and would drop a generated one on its
/// own. The step is what makes the script's answer equal the rule's answer.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_names_no_generated_file() {
    let loader = builtin_loader();
    let project_types = ["rust"];
    require_tool_installed(&loader, &project_types, RUST_MISSING_DOCS_RULE);

    let reported = shipped_script_findings(
        &loader,
        RUST_MISSING_DOCS_RULE,
        ToolScope::Workspace,
        RUST_GENERATED_PROBE_FILES,
    )
    .expect("the shipped script must judge the probe crate and exit 0");

    assert_eq!(
        reported,
        expected_script_findings(RUST_GENERATED_REPORTS),
        "the script must name the hand-written item and no file under `target/`"
    );
}

/// The manifest of the probe crate cargo cannot compile.
const RUST_UNCOMPILABLE_MANIFEST: &str = concat!(
    "[package]\nname = \"uncompilable-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[workspace]\n",
);

/// Every file of the uncompilable probe crate the work-list does not name.
const RUST_UNCOMPILABLE_SUPPORT_FILES: &[(&str, &str)] =
    &[("Cargo.toml", RUST_UNCOMPILABLE_MANIFEST)];

/// A Rust library that does not compile: the struct declaration ends with no
/// semicolon.
const RUST_UNCOMPILABLE_SOURCE: &str = concat!(
    "//! A probe crate the compiler cannot build.\n",
    "\n",
    "pub struct Undocumented\n",
);

/// Where the library that does not compile stands inside the probe repository.
const RUST_UNCOMPILABLE_PATH: &str = "src/lib.rs";

/// What cargo puts at the front of the failure it writes for a crate it cannot
/// compile. The run's error detail must carry it, so the agent reading the
/// error learns what broke.
const RUST_CANNOT_COMPILE_MESSAGE: &str = "could not compile";

/// What the one error of a crate cargo cannot compile must name.
const RUST_UNCOMPILABLE_ERROR: &[&str] = &[RUST_CANNOT_COMPILE_MESSAGE, "uncompilable-probe"];

/// The `missing-docs-rust` probe over a crate cargo cannot compile.
const RUST_UNCOMPILABLE_PROBE: ShippedBrokenRun = ShippedBrokenRun {
    run: ShippedRun {
        project_types: &["rust"],
        rule: RUST_MISSING_DOCS_RULE,
        expected: RUST_UNCOMPILABLE_ERROR,
    },
    change_purpose: "a Rust crate the compiler cannot build",
    path: RUST_UNCOMPILABLE_PATH,
    source: Some(RUST_UNCOMPILABLE_SOURCE),
    support: RUST_UNCOMPILABLE_SUPPORT_FILES,
};

/// Acceptance: the shipped Rust missing-docs tool rule BREAKS on a crate cargo
/// cannot compile, through the real clippy pipeline.
///
/// `cargo clippy` exits 101 for such a crate and writes no `missing_docs`
/// diagnostic for it. A shell pipeline takes the exit status of its LAST
/// command, so the earlier pipe — which ended in `jq` — exited 0 and reported
/// nothing, a run answering zero for a reason other than a clean crate. The
/// script writes cargo's report to a file, and `set -e` makes cargo's own exit
/// status the exit status of the script.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_breaks_on_a_crate_that_does_not_compile() {
    verify_shipped_run_breaks(&RUST_UNCOMPILABLE_PROBE);
}
