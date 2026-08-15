//! Acceptance tests for the shipped `function-length-dart` tool rule.
//!
//! Each test drives the SHIPPED script over a probe repository and reads what
//! the real `dart_code_linter` reported.
//!
//! One module stands for each language of the family, because one file for
//! the whole family runs past the byte cap a review prompt holds.

use super::*;

/// The materialized name of the `function-length-dart` fail fixture.
const DART_FUNCTION_LENGTH_FAIL_FIXTURE: &str = "function-length-dart.fail.dart";

/// Where the fail fixture stands inside the probe repository, as the
/// work-list holds it.
///
/// The path stands under `lib/`, which is where a Dart package keeps the code
/// this gate reads. It carries no mark of the carve-out below: it names no
/// test directory, it does not end `_test.dart`, and it carries no generator
/// suffix.
const DART_FUNCTION_LENGTH_FIXTURE_PATH: &str = "lib/function_length_fail.dart";

/// Where the SAME fixture stands for the carve-out test, as the work-list
/// holds it.
const DART_FUNCTION_LENGTH_TEST_PATH: &str = "test/function_length_fail_test.dart";

/// The source line each declaration of the `function-length-dart` fail
/// fixture is reported at.
///
/// The gate reports at the head of the declaration it measures, and
/// `dart_code_linter` computes that head with
/// `firstTokenAfterCommentAndMetadata`, so each entry is the signature line
/// rather than the doc comment above it.
///
/// The four are the four kinds `source-lines-of-code` reads. A tool upgrade
/// that stopped reading a whole kind still reports the other three, so the
/// list is the whole answer rather than a count.
const DART_FUNCTION_LENGTH_FAIL_DECLARATIONS: &[&str] = &[
    "int accumulateAtTopLevel(int seed) {",
    "Accumulator(int seed) {",
    "int accumulateInAMethod(int seed) {",
    "int get accumulatedTotal {",
];

/// What one entry of a `function-length-dart` run's `expected` is.
const DART_DECLARATION_NOUN: &str = "declaration";

/// The `function-length-dart` fail fixture, and every declaration the real
/// `dart_code_linter` pipeline must measure inside it.
const DART_FUNCTION_LENGTH_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_FUNCTION_LENGTH_RULE,
        expected: DART_FUNCTION_LENGTH_FAIL_DECLARATIONS,
    },
    fixture: DART_FUNCTION_LENGTH_FAIL_FIXTURE,
    path: DART_FUNCTION_LENGTH_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: DART_DECLARATION_NOUN,
};

/// The same fixture, staged under `test/`, where the run must report nothing.
const DART_FUNCTION_LENGTH_TEST_PATH_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_FUNCTION_LENGTH_RULE,
        expected: NO_FINDINGS,
    },
    fixture: DART_FUNCTION_LENGTH_FAIL_FIXTURE,
    path: DART_FUNCTION_LENGTH_TEST_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: DART_DECLARATION_NOUN,
};

/// A one-validator work-list over `path` for the builtin `code-hygiene` set,
/// naming the `function-length` prompt rule and the Dart tool rule.
///
/// It names `function-length` alone. `function-length-dart` supersedes that
/// rule and no other, because no Dart metric reproduces either gate of
/// `cognitive-complexity`; naming the branching rule here would state a
/// supersession the rule does not declare.
fn dart_function_length_work(path: &str, content: &str) -> WorkList {
    WorkList::new(
        "a function over the length gate",
        vec![ValidatorWork::new(
            CODE_HYGIENE_SET,
            RuleNames::new([
                FUNCTION_LENGTH_PROMPT_RULE.to_string(),
                DART_FUNCTION_LENGTH_RULE.to_string(),
            ]),
            ProbeNames::new([]),
            [FileWork::new(path, vec![], vec![], content, vec![])],
        )],
    )
}

/// Acceptance: the shipped Dart function-length tool rule reports every
/// declaration its fail fixture holds, through the real `dart_code_linter`
/// pipeline.
///
/// The four declarations are a top-level function, a constructor, a method
/// and a getter, which are the four kinds `source-lines-of-code` reads. Each
/// body runs 252 or 253 code lines against the gate of 250, and the pass
/// fixture holds the same four kinds at 247 or 248, so the pair pins the gate
/// as well as the kinds.
///
/// A constructor and a getter are the two rows that make this test worth
/// writing. `solid_lints` 0.3.3, the other Dart tool that computes a length
/// metric, registers `addFunctionDeclaration`, `addMethodDeclaration` and
/// `addFunctionExpression`, and reads no constructor at all — so a swap to
/// that tool would pass a fixture pair that held a plain function alone, and
/// would silently stop measuring every Dart constructor.
#[test]
fn the_shipped_dart_function_length_tool_rule_reports_every_fail_fixture_declaration() {
    verify_shipped_fail_fixture_reports_each(
        &DART_FUNCTION_LENGTH_FAIL_PROBE,
        |content| dart_function_length_work(DART_FUNCTION_LENGTH_FIXTURE_PATH, content),
        fail_fixture_source_line,
        |reported, declaration| reported.starts_with(declaration),
    );
}

/// Acceptance: the shipped Dart function-length tool rule reports nothing for
/// the same bytes staged under `test/`, through the real pipeline.
///
/// `dart_code_linter` folds a closure into the function that holds it, and a
/// Dart test file is one `main` holding every `group` and `test` closure of
/// the file, so `main` in a test file measures the WHOLE FILE. Measured over
/// 3931 files of `dart-lang/http`, `dart-lang/shelf` and `flutter/packages`:
/// at the gate of 250 the corpus reports 400 findings, 376 of them in test
/// files and 369 of those on the file's own `main`.
///
/// The carve-out reads a PATH, which is the mark `cognitive-complexity`
/// forbids, and the rule file states why no other mark exists: a Dart test is
/// an anonymous closure handed to `test(...)`, so it carries no definition to
/// read, and `dart_code_linter` 4.2.0 excludes by glob alone.
///
/// This test drives the FAIL fixture, so the same bytes that report four
/// findings at the path above report none here. A carve-out that stopped
/// firing would report those four, and a script that answered a tool error for
/// a run whose every file it excludes would break the pipeline instead — the
/// assertion inside the helper holds both.
#[test]
fn the_shipped_dart_function_length_tool_rule_drops_a_file_under_a_test_directory() {
    verify_shipped_fail_fixture_reports_each(
        &DART_FUNCTION_LENGTH_TEST_PATH_PROBE,
        |content| dart_function_length_work(DART_FUNCTION_LENGTH_TEST_PATH, content),
        fail_fixture_source_line,
        |reported, declaration| reported.starts_with(declaration),
    );
}
