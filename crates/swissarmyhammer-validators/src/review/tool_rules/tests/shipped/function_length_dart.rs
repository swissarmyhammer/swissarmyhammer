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
/// It names `function-length` alone, which is the ONE size gate this set
/// states and the one rule `function-length-dart` supersedes.
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
/// The carve-out reads a PATH, which is the mark this set forbids, and the
/// rule file states why no other mark exists: a Dart test is
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

/// The code-line gate the shipped `function-length-dart` rule states.
const DART_LINE_GATE: usize = 250;

/// How many code lines a probe function stands above the gate.
///
/// A later `dart_code_linter` can count one line another way. A probe this far
/// above the gate stays above it when the new count is smaller by fewer lines
/// than this.
const DART_LINE_MARGIN: usize = 10;

/// The code-line count of a probe function that must report: over the gate,
/// plus a margin, so a later `dart_code_linter` that counts one line another
/// way does not move the probe across the gate.
const OVER_THE_GATE_LINES: usize = DART_LINE_GATE + DART_LINE_MARGIN;

/// The head of the Dart declaration of `name`, as the source writes it.
///
/// `dart_code_linter` anchors each finding on the head of the declaration it
/// measured, which it computes with `firstTokenAfterCommentAndMetadata`, so
/// this is the text [`expected_row`] reads the line from.
fn dart_declaration_head(name: &str) -> String {
    format!("int {name}(int seed) {{")
}

/// A Dart top-level function of `lines` code lines, one addition for each
/// step.
///
/// Every line of the body holds a token, so `source-lines-of-code` counts the
/// whole body.
fn dart_procedure(name: &str, lines: usize) -> String {
    let body: String = (0..lines)
        .map(|step| format!("  total += {step};\n"))
        .collect();
    let head = dart_declaration_head(name);
    format!("{head}\n  var total = seed;\n{body}  return total;\n}}\n")
}

/// The `path:line` entry the run must report for the declaration of `name` in
/// `source`, staged at `path`.
fn dart_expected_row(path: &str, source: &str, name: &str) -> String {
    expected_row(path, source, &dart_declaration_head(name))
}

/// The name of the probe declaration that stands over the gate.
const DART_LENGTH_JUDGED_DECLARATION: &str = "accumulate";

/// Where the file the run CAN measure stands, beside each item it cannot
/// measure.
///
/// The path stands under `lib/` and carries no mark of the carve-out: it names
/// no test directory, it does not end `_test.dart`, and it carries no
/// generator suffix.
const DART_LENGTH_JUDGED_PATH: &str = "lib/judged.dart";

/// Where the Dart file that does not parse stands inside the probe
/// repository.
const DART_LENGTH_UNPARSABLE_PATH: &str = "lib/unparsable.dart";

/// A Dart file that does not parse: the text holds no declaration at all.
const DART_LENGTH_UNPARSABLE_SOURCE: &str = "this is @@@ not (((  dart ]]]\n";

/// Where the path the run cannot read stands inside the probe repository.
///
/// One name serves all three shapes: the same path holds no file, holds bytes
/// that are not UTF-8, or holds source nobody may read, so the way it refuses
/// is the one difference between the three probes.
const DART_LENGTH_UNREADABLE_PATH: &str = "lib/unreadable.dart";

/// A Dart file whose bytes are not UTF-8.
///
/// The declaration parses; the comment holds two bytes that open no UTF-8
/// sequence, so a reader opens the file and cannot decode it.
const DART_LENGTH_UNDECODABLE_SOURCE: &[u8] = b"int shortFunction() => 1; // \xff\xfe\n";

/// A Dart file the tool could measure if the mode let it.
///
/// The source is ordinary and stands under the gate, so a run that DID measure
/// it would report no finding — which is the clean answer this rule must not
/// give for a file it never read.
const DART_LENGTH_FORBIDDEN_SOURCE: &str = "int shortFunction() => 1;\n";

/// Acceptance: the shipped Dart function-length tool rule DECLINES a Dart file
/// it cannot parse, through the real `dart_code_linter` pipeline.
///
/// Measured with `dart_code_linter` 4.2.0 over a file holding
/// `this is @@@ not (((  dart ]]]` beside one function of 264 code lines: two
/// records on the report — 0 functions for the file that does not parse and
/// the one function of the file it read — at exit 0, and 0 bytes on stderr.
/// The tool measured the other file, so the finding is there to lose.
///
/// An `exit 1` loses it: it fails the WHOLE run, so the finding of the file
/// `dart_code_linter` DID measure goes away with it. The file that does not
/// parse is one declined item of a sound run, so the script states it under
/// the `sah-diagnostic:` marker at exit 0.
#[test]
fn the_shipped_dart_function_length_tool_rule_declines_a_file_it_cannot_parse() {
    let judged = dart_procedure(DART_LENGTH_JUDGED_DECLARATION, OVER_THE_GATE_LINES);
    let expected = dart_expected_row(
        DART_LENGTH_JUDGED_PATH,
        &judged,
        DART_LENGTH_JUDGED_DECLARATION,
    );

    verify_unjudged_file_is_declined(
        FLUTTER_PROJECT_TYPES,
        DART_FUNCTION_LENGTH_RULE,
        &[
            (DART_LENGTH_JUDGED_PATH, &judged),
            (DART_LENGTH_UNPARSABLE_PATH, DART_LENGTH_UNPARSABLE_SOURCE),
        ],
        DART_LENGTH_UNPARSABLE_PATH,
        &[&expected],
    );
}

/// Holds the shipped Dart function-length run to measuring `lib/judged.dart`
/// and to stating the one path it could not read, through the real
/// `dart_code_linter` pipeline.
///
/// The measured file carries one function over the gate, so the run has a
/// finding to lose. Losing it is what a nonzero exit over a declined item
/// costs, and staying silent about the path is what reads that path as a clean
/// file.
fn verify_dart_length_declines(path: &str, unreadable: &ShippedUnreadableFile) {
    let judged = dart_procedure(DART_LENGTH_JUDGED_DECLARATION, OVER_THE_GATE_LINES);
    let expected = dart_expected_row(
        DART_LENGTH_JUDGED_PATH,
        &judged,
        DART_LENGTH_JUDGED_DECLARATION,
    );

    verify_unreadable_file_is_declined(
        FLUTTER_PROJECT_TYPES,
        DART_FUNCTION_LENGTH_RULE,
        &[(DART_LENGTH_JUDGED_PATH, &judged)],
        path,
        unreadable,
        &[&expected],
    );
}

/// Acceptance: the shipped Dart function-length tool rule DECLINES a path that
/// holds no file, through the real `dart_code_linter` pipeline.
///
/// The script tests each path it is given before it builds the probe package,
/// because the package holds a COPY of each file: a path it cannot read is a
/// file `cp` never writes, so the tool would never hear of it and the run would
/// read it as a clean file.
#[test]
fn the_shipped_dart_function_length_tool_rule_declines_a_path_that_holds_no_file() {
    verify_dart_length_declines(DART_LENGTH_UNREADABLE_PATH, &ShippedUnreadableFile::Absent);
}

/// Acceptance: the shipped Dart function-length tool rule DECLINES a file
/// whose bytes are not UTF-8, through the real `dart_code_linter` pipeline.
///
/// Measured with `dart_code_linter` 4.2.0 over such a file beside one function
/// of 264 code lines: two records on the report — 0 functions for the file that
/// does not decode and the one function of the file it read — at exit 0. So the
/// tool measures the other files, and 0 functions is the same answer an empty
/// Dart file gives.
#[test]
fn the_shipped_dart_function_length_tool_rule_declines_a_file_it_cannot_decode() {
    verify_dart_length_declines(
        DART_LENGTH_UNREADABLE_PATH,
        &ShippedUnreadableFile::Undecodable(DART_LENGTH_UNDECODABLE_SOURCE),
    );
}

/// Acceptance: the shipped Dart function-length tool rule DECLINES a file it
/// may not read, through the real `dart_code_linter` pipeline.
///
/// The probe takes every permission off the file, which is a mode, so it runs
/// on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_dart_function_length_tool_rule_declines_a_file_it_may_not_read() {
    verify_dart_length_declines(
        DART_LENGTH_UNREADABLE_PATH,
        &ShippedUnreadableFile::Forbidden(DART_LENGTH_FORBIDDEN_SOURCE),
    );
}
