//! Acceptance tests for the shipped magic-numbers tool rules.
//!
//! One test holds the whole roster to its fixture pair and to the prompt rule
//! it supersedes. The tests under it drive one language each through its real
//! tool.

use super::*;

/// Acceptance: every shipped magic-numbers tool rule passes its fixture pair
/// in doctor, and supersedes the `magic-numbers` prompt rule.
///
/// The pass fixture is the load-bearing half here. Each of these tools will
/// report every inline literal at its default settings, and the rule's
/// configuration narrows it to the contexts the prompt rule names. The pass
/// fixture holds the carve-outs — `0`, `1`, `-1`, a value a declaration
/// already names — so a configuration that stopped applying makes the pair
/// fail. [`verify_shipped_tool_rules_pass_fixtures`] carries the rest of the
/// contract, including what a machine without the tool proves.
#[test]
#[serial_test::serial(cwd)]
fn every_shipped_magic_numbers_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(SHIPPED_MAGIC_NUMBERS_RULES, MAGIC_NUMBERS_PROMPT_RULE);
}

/// The materialized name of the `magic-numbers-python` fail fixture.
const PYTHON_MAGIC_NUMBERS_FAIL_FIXTURE: &str = "magic-numbers-python.fail.py";

/// Where the `magic-numbers-python` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const PYTHON_MAGIC_NUMBERS_FIXTURE_PATH: &str = "src/magic_numbers_python_fail.py";

/// Every literal the `magic-numbers-python` fail fixture leaves unnamed, as
/// `PLR2004` spells it inside the message it reports.
///
/// `100` is the load-bearing entry. The `magic-numbers` prompt rule carves out
/// "conventional values (a `<< 8`, `100` for percent)", and `ruff` exposes no
/// value allow-list that could restore that carve-out:
/// `lint.pylint.allow-magic-value-types` selects TYPES, and naming `int` there
/// silences every integer. The rule body therefore states that `100` reports,
/// and this entry holds `ruff` to the statement.
const PYTHON_MAGIC_NUMBERS_FAIL_VALUES: &[&str] = &["404", "4096", "10", "90", "100"];

/// The `magic-numbers-python` fail fixture, and every unnamed literal the real
/// ruff pipeline must report inside it.
const PYTHON_MAGIC_NUMBERS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: &["python"],
        rule: PYTHON_MAGIC_NUMBERS_RULE,
        expected: PYTHON_MAGIC_NUMBERS_FAIL_VALUES,
    },
    fixture: PYTHON_MAGIC_NUMBERS_FAIL_FIXTURE,
    path: PYTHON_MAGIC_NUMBERS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "unnamed literal",
};

/// Acceptance: the shipped Python magic-numbers tool rule reports every
/// unnamed literal its fail fixture holds, through the real ruff pipeline.
///
/// A literal is held to the CLAIM its finding carries, because `PLR2004`
/// spells the value inside the message it reports.
///
/// The count is the other half. `PLR2004` reads a comparison and nothing else,
/// so a repeated literal in a call argument, an operation, or a return is never
/// reported. Holding the run to exactly these five states that silence.
#[test]
fn the_shipped_python_magic_numbers_tool_rule_reports_every_fail_fixture_value() {
    verify_shipped_fail_fixture_reports_each(
        &PYTHON_MAGIC_NUMBERS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "a comparison against an unnamed literal",
                CODE_HYGIENE_SET,
                [
                    MAGIC_NUMBERS_PROMPT_RULE.to_string(),
                    PYTHON_MAGIC_NUMBERS_RULE.to_string(),
                ],
                [(PYTHON_MAGIC_NUMBERS_FIXTURE_PATH, content)],
            )
        },
        |verified, _source| verified.finding.claim.clone(),
        |reported, value| reported.contains(&format!("`{value}`")),
    );
}

/// The materialized name of the `magic-numbers-typescript` fail fixture.
const TYPESCRIPT_MAGIC_NUMBERS_FAIL_FIXTURE: &str = "magic-numbers-typescript.fail.ts";

/// Where the `magic-numbers-typescript` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const TYPESCRIPT_MAGIC_NUMBERS_FIXTURE_PATH: &str = "src/magic-numbers-typescript-fail.ts";

/// Every literal the `magic-numbers-typescript` fail fixture leaves unnamed, as
/// `no-magic-numbers` spells it inside the message it reports.
///
/// `8` is the load-bearing entry. The `magic-numbers` prompt rule carves out
/// "conventional values (a `<< 8`, `100` for percent)", and the rule restores
/// the percent half through `ignore`. The shift half has no such lever:
/// `ignore` selects a VALUE and never a position, so `8` in `ignore` would
/// silence `x === 8` beside `x << 8`, and eslint names no option for a shift
/// operand. The rule body therefore states that a shift operand reports, and
/// this entry holds eslint to the statement.
const TYPESCRIPT_MAGIC_NUMBERS_FAIL_VALUES: &[&str] = &["404", "4096", "250", "8"];

/// The `magic-numbers-typescript` fail fixture, and every unnamed literal the
/// real eslint pipeline must report inside it.
const TYPESCRIPT_MAGIC_NUMBERS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: &["nodejs"],
        rule: TYPESCRIPT_MAGIC_NUMBERS_RULE,
        expected: TYPESCRIPT_MAGIC_NUMBERS_FAIL_VALUES,
    },
    fixture: TYPESCRIPT_MAGIC_NUMBERS_FAIL_FIXTURE,
    path: TYPESCRIPT_MAGIC_NUMBERS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "unnamed literal",
};

/// What `no-magic-numbers` puts before the value in the message it reports.
/// The whole message reads `No magic number: 4096.`, so the value stands
/// after this separator and before the closing full stop.
const TYPESCRIPT_MAGIC_NUMBERS_CLAIM_SEPARATOR: &str = ": ";

/// Acceptance: the shipped TypeScript magic-numbers tool rule reports every
/// unnamed literal its fail fixture holds, through the real eslint pipeline.
///
/// A literal is held to the CLAIM its finding carries, because
/// `no-magic-numbers` spells the value at the end of the message it reports.
///
/// The count is the other half. The pass fixture holds `100` for percent and
/// every declaration position the configuration allows, so a run that reported
/// one of them would fail the pair; holding this run to exactly these four
/// states the same silence from the other side.
#[test]
fn the_shipped_typescript_magic_numbers_tool_rule_reports_every_fail_fixture_value() {
    verify_shipped_fail_fixture_reports_each(
        &TYPESCRIPT_MAGIC_NUMBERS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an unnamed literal in a comparison, an operation, and a call argument",
                CODE_HYGIENE_SET,
                [
                    MAGIC_NUMBERS_PROMPT_RULE.to_string(),
                    TYPESCRIPT_MAGIC_NUMBERS_RULE.to_string(),
                ],
                [(TYPESCRIPT_MAGIC_NUMBERS_FIXTURE_PATH, content)],
            )
        },
        |verified, _source| verified.finding.claim.clone(),
        |reported, value| {
            reported.ends_with(&format!(
                "{TYPESCRIPT_MAGIC_NUMBERS_CLAIM_SEPARATOR}{value}."
            ))
        },
    );
}

/// The materialized name of the `magic-numbers-go` fail fixture.
const GO_MAGIC_NUMBERS_FAIL_FIXTURE: &str = "magic-numbers-go.fail.go";

/// Where the `magic-numbers-go` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const GO_MAGIC_NUMBERS_FIXTURE_PATH: &str = "src/magic_numbers_go_fail.go";

/// The support fixture the Go probe repository needs beside the fail fixture.
///
/// `magic-numbers-go` runs at `workspace` scope over `./...`, because `mnd`
/// needs a loaded package to read. A directory holding one Go file and no
/// module manifest loads nothing, so the probe repository takes the manifest
/// the set already ships for its own Go fixtures.
const GO_MAGIC_NUMBERS_SUPPORT: &[(&str, &str)] = &[("go.mod", "go.mod")];

/// Every literal the `magic-numbers-go` fail fixture leaves unnamed, as `mnd`
/// spells it inside the message it reports.
///
/// `8` is the load-bearing entry. The `magic-numbers` prompt rule carves out
/// "conventional values (a `<< 8`, `100` for percent)", and the rule restores
/// the percent half through `ignored-numbers`. The shift half has no such
/// lever: `ignored-numbers` selects a VALUE and never a position, so `8` in the
/// list would silence `status == 8` beside `word << 8`, and no other `mnd`
/// setting names a shift operand. The rule body therefore states that a shift
/// operand reports, and this entry holds `mnd` to the statement.
const GO_MAGIC_NUMBERS_FAIL_VALUES: &[&str] = &["404", "20", "4096", "512", "250", "8"];

/// The `magic-numbers-go` fail fixture, and every unnamed literal the real
/// golangci-lint pipeline must report inside it.
const GO_MAGIC_NUMBERS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: &["go"],
        rule: GO_MAGIC_NUMBERS_RULE,
        expected: GO_MAGIC_NUMBERS_FAIL_VALUES,
    },
    fixture: GO_MAGIC_NUMBERS_FAIL_FIXTURE,
    path: GO_MAGIC_NUMBERS_FIXTURE_PATH,
    support: GO_MAGIC_NUMBERS_SUPPORT,
    noun: "unnamed literal",
};

/// What `mnd` puts before the value in the message it reports. The whole
/// message reads `Magic number: 4096, in <operation> detected`, so the value
/// stands after this text.
const GO_MAGIC_NUMBERS_CLAIM_PREFIX: &str = "Magic number: ";

/// What `mnd` puts after the value in the message it reports. Holding the
/// value to both sides keeps `8` from matching the `4096` finding.
const GO_MAGIC_NUMBERS_CLAIM_SUFFIX: &str = ",";

/// Acceptance: the shipped Go magic-numbers tool rule reports every unnamed
/// literal its fail fixture holds, through the real golangci-lint pipeline.
///
/// A literal is held to the CLAIM its finding carries, because `mnd` spells the
/// value inside the message it reports.
///
/// The count is the other half. The pass fixture holds `100` for percent and
/// every declaration position the configuration allows, so a run that reported
/// one of them would fail the pair; holding this run to exactly these six
/// states the same silence from the other side.
#[test]
fn the_shipped_go_magic_numbers_tool_rule_reports_every_fail_fixture_value() {
    verify_shipped_fail_fixture_reports_each(
        &GO_MAGIC_NUMBERS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an unnamed literal in a condition, a switch case, an operation, \
                 a return, and a call argument",
                CODE_HYGIENE_SET,
                [
                    MAGIC_NUMBERS_PROMPT_RULE.to_string(),
                    GO_MAGIC_NUMBERS_RULE.to_string(),
                ],
                [(GO_MAGIC_NUMBERS_FIXTURE_PATH, content)],
            )
        },
        |verified, _source| verified.finding.claim.clone(),
        |reported, value| {
            reported.contains(&format!(
                "{GO_MAGIC_NUMBERS_CLAIM_PREFIX}{value}{GO_MAGIC_NUMBERS_CLAIM_SUFFIX}"
            ))
        },
    );
}

/// The materialized name of the `magic-numbers-swift` fail fixture.
const SWIFT_MAGIC_NUMBERS_FAIL_FIXTURE: &str = "magic-numbers-swift.fail.swift";

/// Where the `magic-numbers-swift` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const SWIFT_MAGIC_NUMBERS_FIXTURE_PATH: &str = "Sources/MagicNumbersSwiftFail.swift";

/// Every line the `magic-numbers-swift` fail fixture leaves unnamed, trimmed as
/// the fixture writes it.
///
/// A line, and not a value, because `no_magic_numbers` reports one message —
/// `Magic numbers should be replaced by named constants` — for every literal,
/// so the claim never spells which one it read.
///
/// `return word << 8 | 1` is the load-bearing entry. The `magic-numbers` prompt
/// rule carves out "conventional values (a `<< 8`, `100` for percent)", and this
/// is the one rule of the five that restores BOTH halves: `allowed_numbers`
/// carries `100`, and `swiftlint` reads the shift OPERATOR, so `word << 8` is
/// silent while `status == 8` still reports. The carve-out reaches a whole
/// shift and not a link of a longer unparenthesised chain, so this line is the
/// edge of it, and this entry holds `swiftlint` to the rule body's statement.
const SWIFT_MAGIC_NUMBERS_FAIL_LINES: &[&str] = &[
    "if status == 404 {",
    "case 20:",
    "return size * 4096",
    "return schedule(delayMillis: 250)",
    "return word << 8 | 1",
];

/// The `magic-numbers-swift` fail fixture, and every unnamed literal the real
/// swiftlint pipeline must report inside it.
const SWIFT_MAGIC_NUMBERS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: &["swift"],
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        expected: SWIFT_MAGIC_NUMBERS_FAIL_LINES,
    },
    fixture: SWIFT_MAGIC_NUMBERS_FAIL_FIXTURE,
    path: SWIFT_MAGIC_NUMBERS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "line holding an unnamed literal",
};

/// Acceptance: the shipped Swift magic-numbers tool rule reports every unnamed
/// literal its fail fixture holds, through the real swiftlint pipeline.
///
/// A literal is held to the SOURCE LINE its finding stands on, because
/// `no_magic_numbers` writes one message for every literal and never spells the
/// value it read.
///
/// The count is the other half. The pass fixture holds `100` for percent, every
/// declaration position the configuration allows, and `word << 8` beside
/// `word >> 8`, so a run that reported one of them would fail the pair; holding
/// this run to exactly these five states the same silence from the other side.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_reports_every_fail_fixture_line() {
    verify_shipped_fail_fixture_reports_each(
        &SWIFT_MAGIC_NUMBERS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an unnamed literal in a condition, a switch case, an operation, \
                 a call argument, and a shift inside a longer chain",
                CODE_HYGIENE_SET,
                [
                    MAGIC_NUMBERS_PROMPT_RULE.to_string(),
                    SWIFT_MAGIC_NUMBERS_RULE.to_string(),
                ],
                [(SWIFT_MAGIC_NUMBERS_FIXTURE_PATH, content)],
            )
        },
        fail_fixture_source_line,
        |reported, expected| reported == expected,
    );
}

/// The materialized name of the `magic-numbers-dart` fail fixture.
const DART_MAGIC_NUMBERS_FAIL_FIXTURE: &str = "magic-numbers-dart.fail.dart";

/// Where the `magic-numbers-dart` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const DART_MAGIC_NUMBERS_FIXTURE_PATH: &str = "lib/magic_numbers_dart_fail.dart";

/// Every line the `magic-numbers-dart` fail fixture leaves unnamed, trimmed as
/// the fixture writes it.
///
/// A line, and not a value, because `no_magic_number` reports one message —
/// `Avoid using magic numbers.Extract them to named constants or variables.` —
/// for every literal, so the claim never spells which one it read.
///
/// The last two entries are load-bearing. The `magic-numbers` prompt rule
/// carves out "conventional values (a `<< 8`, `100` for percent)", and this
/// rule restores neither: `solid_lints` 0.3.3 throws on any `allowed` list, so
/// `100` reports, and no value allow-list can state a shift POSITION, so
/// `word << 8` reports. The rule body states both, and these two entries hold
/// the tool to the statement.
const DART_MAGIC_NUMBERS_FAIL_LINES: &[&str] = &[
    "bool mayDrive(int age) => age > 18;",
    "schedule(3600);",
    "return 65535;",
    "int scale(int value) => value * 4096;",
    "int pack(int word) => word << 8;",
    "int share(int part) => part * 100;",
];

/// The `magic-numbers-dart` fail fixture, and every unnamed literal the real
/// `custom_lint` pipeline must report inside it.
const DART_MAGIC_NUMBERS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: &["flutter"],
        rule: DART_MAGIC_NUMBERS_RULE,
        expected: DART_MAGIC_NUMBERS_FAIL_LINES,
    },
    fixture: DART_MAGIC_NUMBERS_FAIL_FIXTURE,
    path: DART_MAGIC_NUMBERS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "line holding an unnamed literal",
};

/// Acceptance: the shipped Dart magic-numbers tool rule reports every unnamed
/// literal its fail fixture holds, through the real `custom_lint` pipeline.
///
/// A literal is held to the SOURCE LINE its finding stands on, because
/// `no_magic_number` writes one message for every literal and never spells the
/// value it read.
///
/// The count is the other half, and it is what a silent run cannot fake. The
/// script builds a probe package and runs `dart pub get` inside it, and a run
/// that reached neither the plugin nor the package would report zero findings
/// and exit `0`. Holding this run to exactly these six lines states that the
/// plugin loaded, read its configuration, and read each position.
#[test]
fn the_shipped_dart_magic_numbers_tool_rule_reports_every_fail_fixture_line() {
    verify_shipped_fail_fixture_reports_each(
        &DART_MAGIC_NUMBERS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an unnamed literal in a comparison, a call argument, a return, \
                 an operation, a shift, and a percent",
                CODE_HYGIENE_SET,
                [
                    MAGIC_NUMBERS_PROMPT_RULE.to_string(),
                    DART_MAGIC_NUMBERS_RULE.to_string(),
                ],
                [(DART_MAGIC_NUMBERS_FIXTURE_PATH, content)],
            )
        },
        fail_fixture_source_line,
        |reported, expected| reported == expected,
    );
}

/// Where the Dart library that uses a declaration newer than a fixed floor
/// stands inside the probe repository, as the work-list holds it.
///
/// It stands under `lib/`, which is where the probe package holds every file
/// it copies, regardless of the path the review handed over.
const DART_MAGIC_NUMBERS_LANGUAGE_VERSION_PATH: &str = "lib/magic_numbers_language_version.dart";

/// A Dart library whose first declaration is an `extension type`, holding two
/// unnamed literals, beside a plain top-level function holding a third.
///
/// `extension type` arrived in Dart 3.3. A probe package whose
/// `environment: sdk:` states a floor below that version gives the analyzer a
/// language version that refuses the declaration, and every literal inside it
/// goes off the report.
const DART_MAGIC_NUMBERS_LANGUAGE_VERSION_SOURCE: &str = concat!(
    "extension type Meters(int value) {\n",
    "  int get doubled => value + 4096;\n",
    "\n",
    "  void report() {\n",
    "    print(value * 250);\n",
    "  }\n",
    "}\n",
    "\n",
    "int scale(int input) => input * 65535;\n",
);

/// The text of each unnamed literal
/// [`DART_MAGIC_NUMBERS_LANGUAGE_VERSION_SOURCE`] holds, unique enough to
/// locate its own line and no other.
///
/// The first two stand inside the `extension type` and the third stands
/// inside the plain function beneath it.
const DART_MAGIC_NUMBERS_LANGUAGE_VERSION_HEADS: &[&str] = &["4096", "250", "65535"];

/// Acceptance: the shipped Dart magic-numbers tool rule reports every unnamed
/// literal of a library that uses a declaration newer than a fixed floor,
/// through the real `custom_lint` pipeline.
///
/// A package's language version is the LOWER bound of its `environment: sdk:`
/// constraint, and the analyzer refuses syntax newer than that version. A
/// fixed floor therefore hides real code as the language moves. The script
/// therefore reads the version out of `dart --version` and writes
/// `sdk: '^<version>'`, so the probe parses with the language version of the
/// installed SDK.
///
/// Measured on Dart SDK 3.11.0 over this source, with `no_magic_number` on:
///
/// | the probe constraint | lines reported |
/// |---|---|
/// | `>=3.0.0 <5.0.0`, a floor three versions under the declaration | 9 alone |
/// | `>=3.5.0 <4.0.0`, the earlier floor of this rule | 2, 5 and 9 |
/// | `^3.11.0`, derived from `dart --version` | 2, 5 and 9 |
///
/// The earlier floor of this rule does not lose these two literals TODAY —
/// `extension type` needs only Dart 3.3, comfortably under `3.5.0` — so the
/// middle row and the last row read the same. A floor further behind the
/// declaration, held here as the illustration `builtin/validators/README.md`
/// asks a tool rule to carry, DOES lose them: the first row answers 9 alone
/// and exits 0, reading exactly like a file with nothing left to name. This
/// test holds the run to all three lines, so a stale floor can never come
/// back unmeasured as Dart's language moves past `3.5.0`.
#[test]
fn the_shipped_dart_magic_numbers_tool_rule_reports_a_member_of_a_newer_declaration() {
    let expected: Vec<String> = DART_MAGIC_NUMBERS_LANGUAGE_VERSION_HEADS
        .iter()
        .map(|head| {
            expected_row(
                DART_MAGIC_NUMBERS_LANGUAGE_VERSION_PATH,
                DART_MAGIC_NUMBERS_LANGUAGE_VERSION_SOURCE,
                head,
            )
        })
        .collect();
    let expected: Vec<&str> = expected.iter().map(String::as_str).collect();

    verify_staged_rows_report(
        FLUTTER_PROJECT_TYPES,
        DART_MAGIC_NUMBERS_RULE,
        &[(
            DART_MAGIC_NUMBERS_LANGUAGE_VERSION_PATH,
            DART_MAGIC_NUMBERS_LANGUAGE_VERSION_SOURCE,
        )],
        &expected,
        "the probe package states the language version of the installed SDK, so the \
         analyzer parses the extension type and reports its two literals beside the \
         plain function's literal",
    );
}

/// The binary the shipped Dart magic-numbers script calls to derive its
/// probe's language version.
const DART_MAGIC_NUMBERS_BINARY_NAME: &str = "dart";

/// The word `dart --version` takes as its first argument.
const DART_MAGIC_NUMBERS_VERSION_SUBCOMMAND: &str = "--version";

/// Where the one file the no-version probe stages stands, as the work-list
/// holds it.
///
/// The script never reaches `custom_lint` over this file — the break happens
/// before the probe package is even built — so its content answers for
/// nothing beyond giving the run one file to judge.
const DART_MAGIC_NUMBERS_NO_VERSION_PATH: &str = "lib/no_version.dart";

/// [`DART_MAGIC_NUMBERS_NO_VERSION_PATH`]'s source.
const DART_MAGIC_NUMBERS_NO_VERSION_SOURCE: &str = "int scale(int value) => value * 4096;\n";

/// The one file the no-version probe stages.
const DART_MAGIC_NUMBERS_NO_VERSION_STAGED: &[(&str, &str)] = &[(
    DART_MAGIC_NUMBERS_NO_VERSION_PATH,
    DART_MAGIC_NUMBERS_NO_VERSION_SOURCE,
)];

/// The words the error of a `dart --version` that names no version must
/// carry.
const DART_MAGIC_NUMBERS_NO_VERSION_ERROR: &[&str] =
    &[DART_MAGIC_NUMBERS_RULE, "dart --version names no version"];

/// The probe of a `dart --version` that names no version, and the words its
/// error must carry.
const DART_MAGIC_NUMBERS_NO_VERSION_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MAGIC_NUMBERS_RULE,
        expected: DART_MAGIC_NUMBERS_NO_VERSION_ERROR,
    },
    staged: DART_MAGIC_NUMBERS_NO_VERSION_STAGED,
    reason: "a `dart --version` that names no version leaves the probe package unable to \
             state the language version it parses with, and the run must not guess one",
};

/// Acceptance: the shipped Dart magic-numbers tool rule BREAKS when
/// `dart --version` names no version.
///
/// The script reads the installed SDK's language version out of
/// `dart --version` because a fixed floor would hide real code as the
/// language moves. A `dart --version` this script cannot read a version out
/// of leaves it with no constraint to derive, and the run must not guess one:
/// it names the failure and exits, rather than writing a probe package whose
/// `environment: sdk:` states a version nobody measured.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_dart_magic_numbers_tool_rule_breaks_when_dart_version_names_no_version() {
    verify_shipped_tree_breaks_with_stub(
        &DART_MAGIC_NUMBERS_NO_VERSION_PROBE,
        DART_MAGIC_NUMBERS_BINARY_NAME,
        &format!(" && [ \"$1\" = \"{DART_MAGIC_NUMBERS_VERSION_SUBCOMMAND}\" ]"),
        "  printf '%s\\n' 'Dart CLI has no version line here'\n  exit 0",
    );
}

/// The declarations every staged Swift position holds: one unnamed literal in
/// a comparison.
///
/// `404` stands outside the `allowed_numbers` list the rule states, so
/// `no_magic_numbers` reports it once in every file it reads.
const SWIFT_MAGIC_NUMBERS_STAGED: &str = concat!(
    "public func check(_ status: Int) -> Bool {\n",
    "    return status == 404\n",
    "}\n",
);

/// The file of the one finding the staged Swift positions must report.
const SWIFT_MAGIC_NUMBERS_STAGED_REPORTED: &[&str] = &[SWIFT_ORDINARY_POSITION.path];

/// The staged Swift positions, and the one of them the real swiftlint pipeline
/// must report.
const SWIFT_MAGIC_NUMBERS_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        expected: SWIFT_MAGIC_NUMBERS_STAGED_REPORTED,
    },
    prompt_rule: MAGIC_NUMBERS_PROMPT_RULE,
    change_purpose: "one unnamed literal, staged in two positions",
    declarations: SWIFT_MAGIC_NUMBERS_STAGED,
    staged: SWIFT_EXCLUDE_POSITIONS,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the ordinary file reports its literal, and the file under the project's \
             excluded directory reports nothing",
};

/// Acceptance: the shipped Swift magic-numbers tool rule honours the project's
/// own `excluded:` list, through the real swiftlint pipeline.
///
/// The `magic-numbers` prompt rule this rule supersedes carves out nothing by
/// path, and swiftlint holds no generated-code check of its own, so the
/// project's `excluded:` list is the whole carve-out for a generated file.
///
/// The two files hold the same bytes on purpose. The list is the only
/// difference between the file that reports and the file that stays silent.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_reads_the_project_exclude_list() {
    verify_shipped_staged_positions_report(&SWIFT_MAGIC_NUMBERS_POSITIONS_PROBE);
}

/// The `magic-numbers-swift` probe over a run whose every file the project's
/// `excluded:` list names.
const SWIFT_MAGIC_NUMBERS_EVERY_FILE_EXCLUDED_PROBE: ShippedStagedPositions =
    ShippedStagedPositions {
        run: ShippedRun {
            project_types: SWIFT_PROJECT_TYPES,
            rule: SWIFT_MAGIC_NUMBERS_RULE,
            expected: NO_STAGED_REPORTS,
        },
        prompt_rule: MAGIC_NUMBERS_PROMPT_RULE,
        change_purpose: "one unnamed literal under the project's excluded directory",
        declarations: SWIFT_MAGIC_NUMBERS_STAGED,
        staged: SWIFT_EXCLUDED_POSITION_ONLY,
        support: SWIFT_EXCLUDING_SUPPORT_FILES,
        reason: "the project excludes every file of the run, so the run reports nothing and \
                 breaks nothing",
    };

/// Acceptance: the shipped Swift magic-numbers tool rule reports nothing, and
/// breaks nothing, when the project excludes every file of the run, through
/// the real swiftlint pipeline.
///
/// swiftlint exits 1 with `Error: No lintable files found at paths` when
/// `--force-exclude` leaves it no file to read, and that status reads as a
/// broken tool. The script tests each file it is given for readability first,
/// so the message can carry one cause only, and it then exits 0 with no
/// finding.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_answers_zero_when_the_project_excludes_every_file() {
    verify_shipped_staged_positions_report(&SWIFT_MAGIC_NUMBERS_EVERY_FILE_EXCLUDED_PROBE);
}

/// The file of the one finding the `child_config:` probe must report.
///
/// The project excludes this directory, and the run drops that exclude list,
/// so the file reports.
const SWIFT_MAGIC_NUMBERS_CHILD_CONFIG_REPORTED: &[&str] = &[SWIFT_GENERATED_POSITION.path];

/// The `magic-numbers-swift` probe beside a project that names a child
/// configuration of its own.
const SWIFT_MAGIC_NUMBERS_CHILD_CONFIG_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        expected: SWIFT_MAGIC_NUMBERS_CHILD_CONFIG_REPORTED,
    },
    prompt_rule: MAGIC_NUMBERS_PROMPT_RULE,
    change_purpose: "one unnamed literal beside a project child configuration",
    declarations: SWIFT_MAGIC_NUMBERS_STAGED,
    staged: SWIFT_EXCLUDED_POSITION_ONLY,
    support: SWIFT_CHILD_CONFIG_SUPPORT_FILES,
    reason: "swiftlint cannot read that project configuration beside the rule's own, so the run \
             measures with the rule's configuration alone and reports the staged literal",
};

/// Acceptance: the shipped Swift magic-numbers tool rule measures beside a
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
fn the_shipped_swift_magic_numbers_tool_rule_measures_beside_a_project_child_config() {
    verify_shipped_staged_positions_report(&SWIFT_MAGIC_NUMBERS_CHILD_CONFIG_PROBE);
}

/// The `magic-numbers-swift` probe beside a project that states a warning
/// threshold of one finding.
const SWIFT_MAGIC_NUMBERS_WARNING_THRESHOLD_PROBE: ShippedStagedPositions =
    ShippedStagedPositions {
        run: ShippedRun {
            project_types: SWIFT_PROJECT_TYPES,
            rule: SWIFT_MAGIC_NUMBERS_RULE,
            expected: SWIFT_MAGIC_NUMBERS_STAGED_REPORTED,
        },
        prompt_rule: MAGIC_NUMBERS_PROMPT_RULE,
        change_purpose: "one unnamed literal beside a project warning threshold",
        declarations: SWIFT_MAGIC_NUMBERS_STAGED,
        staged: SWIFT_ORDINARY_POSITION_ONLY,
        support: SWIFT_WARNING_THRESHOLD_SUPPORT_FILES,
        reason: "the threshold makes swiftlint exit 2 with the whole report on stdout, and the \
             script reads that status as a measured run, so the staged literal reports",
    };

/// Acceptance: the shipped Swift magic-numbers tool rule measures beside a
/// project that states `warning_threshold:`, through the real swiftlint
/// pipeline.
///
/// Measured with swiftlint 0.65.0 over the staged literal, with
/// `warning_threshold: 1` in the project configuration: swiftlint writes 2
/// entries to stdout — the `no_magic_numbers` finding and one
/// `warning_threshold` entry of error severity — and exits 2. The script read
/// each nonzero status as a broken tool and reported 0 findings, so one line
/// in the project file switched the gate off.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_measures_beside_a_project_warning_threshold() {
    verify_shipped_staged_positions_report(&SWIFT_MAGIC_NUMBERS_WARNING_THRESHOLD_PROBE);
}

/// A project `.swiftlint.yml` that switches the rule off and states another
/// allow-list for it.
///
/// Each of the two settings silences the staged literal on its own:
/// `disabled_rules` switches `no_magic_numbers` off, and an `allowed_numbers`
/// list carrying `404` names the staged literal.
const SWIFT_MAGIC_NUMBERS_OVERRIDING_CONFIG: &str = concat!(
    "disabled_rules:\n",
    "  - no_magic_numbers\n",
    "no_magic_numbers:\n",
    "  allowed_numbers: [404]\n",
);

/// The overriding project configuration staged beside the ordinary position,
/// which the work-list does NOT name.
const SWIFT_MAGIC_NUMBERS_OVERRIDING_SUPPORT: &[(&str, &str)] = &[(
    SWIFT_PROJECT_CONFIG_PATH,
    SWIFT_MAGIC_NUMBERS_OVERRIDING_CONFIG,
)];

/// The `magic-numbers-swift` probe over a project that states another
/// allow-list for the rule.
const SWIFT_MAGIC_NUMBERS_OPTIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        expected: SWIFT_MAGIC_NUMBERS_STAGED_REPORTED,
    },
    prompt_rule: MAGIC_NUMBERS_PROMPT_RULE,
    change_purpose: "one unnamed literal a project allow-list names",
    declarations: SWIFT_MAGIC_NUMBERS_STAGED,
    staged: SWIFT_ORDINARY_POSITION_ONLY,
    support: SWIFT_MAGIC_NUMBERS_OVERRIDING_SUPPORT,
    reason: "the rule's own allow-list decides, so the staged literal still reports",
};

/// Acceptance: the shipped Swift magic-numbers tool rule keeps its own
/// allow-list against a project that states another one, through the real
/// swiftlint pipeline.
///
/// The script names the project's `.swiftlint.yml` as the PARENT of its own
/// configuration, so the project decides which files are read. It must not
/// decide what the rule measures. The script's own configuration states
/// `allowed_numbers`, and a child block replaces the parent's block whole.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_keeps_its_own_allowed_numbers() {
    verify_shipped_staged_positions_report(&SWIFT_MAGIC_NUMBERS_OPTIONS_PROBE);
}

/// The `magic-numbers-swift` probe beside a project that names a swiftlint
/// version that is not installed.
const SWIFT_MAGIC_NUMBERS_VERSION_MISMATCH_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        expected: SWIFT_VERSION_MISMATCH_ERROR,
    },
    prompt_rule: MAGIC_NUMBERS_PROMPT_RULE,
    change_purpose: "one unnamed literal beside a project version mismatch",
    path: SWIFT_ORDINARY_POSITION.path,
    source: Some(SWIFT_MAGIC_NUMBERS_STAGED.as_bytes()),
    support: SWIFT_VERSION_MISMATCH_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift magic-numbers tool rule BREAKS beside a
/// project that names a swiftlint version that is not installed, through the
/// real swiftlint pipeline.
///
/// swiftlint compares `swiftlint_version:` with the version it is. At a
/// difference it writes one warning line to stderr, writes 0 bytes to stdout,
/// runs no lint, and exits 2. Measured with swiftlint 0.65.0 over the staged
/// literal: a run with no project configuration reports 1 finding, and a run
/// beside `swiftlint_version: 99.0.0` reports 0. A script that reads every
/// status 2 as a measured run hands `jq` an empty report, reports 0 findings
/// and exits 0, so the engine reads a dirty file as clean. The script accepts
/// status 2 only when the report holds a JSON array of one entry or more.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_breaks_beside_a_project_version_mismatch() {
    verify_shipped_run_breaks(&SWIFT_MAGIC_NUMBERS_VERSION_MISMATCH_PROBE);
}

/// Where the Swift file the magic-numbers run CAN judge stands, beside each
/// refusing path.
const SWIFT_MAGIC_NUMBERS_JUDGED_PATH: &str = "Sources/Judged.swift";

/// Where the path the magic-numbers run cannot judge stands inside the probe
/// repository.
///
/// One name serves all three shapes: the same path holds no file, holds bytes
/// that are not UTF-8, or holds source nobody may read, so the way it refuses
/// is the one difference between the three probes.
const SWIFT_MAGIC_NUMBERS_UNREADABLE_PATH: &str = "Sources/Unreadable.swift";

/// The head of the line [`SWIFT_MAGIC_NUMBERS_STAGED`] states its unnamed
/// literal on.
const SWIFT_MAGIC_NUMBERS_LITERAL_HEAD: &str = "return status == 404";

/// A Swift file written in Latin-1 rather than in UTF-8.
///
/// The byte `0xE9` is `é` in Latin-1, and it is not a UTF-8 sequence.
/// swiftlint reads a file as UTF-8 and nothing else, so it cannot decode this
/// one. The staged literal stands under the string, so a run that DID read the
/// file reports it.
const SWIFT_MAGIC_NUMBERS_UNDECODABLE_SOURCE: &[u8] = b"let name = \"caf\xe9\"\n\
public func check(_ status: Int) -> Bool {\n\
return status == 404\n\
}\n";

/// A Swift file swiftlint could read if the mode let it.
///
/// The one literal it holds stands in the `allowed_numbers` list the rule
/// states, so a run that DID read this file would report no finding — which is
/// the clean answer this rule must not give for a file it never read.
const SWIFT_MAGIC_NUMBERS_FORBIDDEN_SOURCE: &str = concat!(
    "public func one(_ status: Int) -> Bool {\n",
    "    return status == 1\n",
    "}\n",
);

/// The `magic-numbers-swift` probe over a refusing path beside
/// `Sources/Judged.swift`.
///
/// The judged file carries one unnamed literal, so the run has a finding to
/// lose. Losing it is what a nonzero exit over a declined item costs, and
/// staying silent about the path is what reads that path as a clean file.
fn swift_magic_numbers_decline_probe() -> ShippedDeclineProbe {
    let literal = expected_row(
        SWIFT_MAGIC_NUMBERS_JUDGED_PATH,
        SWIFT_MAGIC_NUMBERS_STAGED,
        SWIFT_MAGIC_NUMBERS_LITERAL_HEAD,
    );

    ShippedDeclineProbe {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        judged: vec![(
            SWIFT_MAGIC_NUMBERS_JUDGED_PATH,
            SWIFT_MAGIC_NUMBERS_STAGED.to_string(),
        )],
        path: SWIFT_MAGIC_NUMBERS_UNREADABLE_PATH,
        expected: vec![literal],
    }
}

/// Acceptance: the shipped Swift magic-numbers tool rule DECLINES a path that
/// holds no file, through the real swiftlint pipeline.
///
/// Measured with swiftlint 0.65.0 over such a path beside one file that holds a
/// finding: 1 entry on stdout, 0 bytes on stderr, and exit 0. swiftlint says
/// NOTHING about the path it dropped — measured again with `--quiet` taken off,
/// it writes `Linting 'Judged.swift' (1/1)` and no word of the other path. So
/// the script tests the path itself, and states it under the marker.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_declines_a_path_that_holds_no_file() {
    verify_unreadable_file_is_declined(
        &swift_magic_numbers_decline_probe(),
        &ShippedUnreadableFile::Absent,
    );
}

/// Acceptance: the shipped Swift magic-numbers tool rule DECLINES a Swift file
/// swiftlint cannot decode, through the real swiftlint pipeline.
///
/// Measured with swiftlint 0.65.0 over this file beside one file that holds a
/// finding: swiftlint writes ``Could not read contents of `<path>` `` to
/// stderr, writes 1 entry to stdout, and exits 0 — the status and the report of
/// a healthy run. The child states `severity: warning`, so no finding of this
/// rule reaches error severity and swiftlint never exits 2. So neither the
/// status nor the report tells this file from a clean one, and the script reads
/// swiftlint's own message instead.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_declines_a_file_it_cannot_decode() {
    verify_unreadable_file_is_declined(
        &swift_magic_numbers_decline_probe(),
        &ShippedUnreadableFile::Undecodable(SWIFT_MAGIC_NUMBERS_UNDECODABLE_SOURCE),
    );
}

/// Acceptance: the shipped Swift magic-numbers tool rule DECLINES a Swift file
/// it may not read, through the real swiftlint pipeline.
///
/// Measured with swiftlint 0.65.0 over this file beside one file that holds a
/// finding: swiftlint writes the same ``Could not read contents of `<path>` ``
/// line, writes 1 entry, and exits 0. The mode and the decode reach swiftlint
/// as one message, so one reading of stderr answers both.
///
/// The probe takes every permission off the file, which is a mode, so it runs
/// on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_declines_a_file_it_may_not_read() {
    verify_unreadable_file_is_declined(
        &swift_magic_numbers_decline_probe(),
        &ShippedUnreadableFile::Forbidden(SWIFT_MAGIC_NUMBERS_FORBIDDEN_SOURCE),
    );
}

/// The `magic-numbers-swift` probe over a file whose name holds the words of
/// swiftlint's decode message.
const SWIFT_MAGIC_NUMBERS_DECODE_NAME_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        expected: NO_STAGED_REPORTS,
    },
    prompt_rule: MAGIC_NUMBERS_PROMPT_RULE,
    change_purpose: "a file whose name holds the words of swiftlint's decode message",
    declarations: SWIFT_MAGIC_NUMBERS_STAGED,
    staged: SWIFT_DECODE_NAME_POSITION_ONLY,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the project excludes the file, so the run reports nothing and breaks nothing, \
             whatever the file is named",
};

/// Acceptance: the shipped Swift magic-numbers tool rule MEASURES a run over a
/// file whose name holds the words of swiftlint's decode message, through the
/// real swiftlint pipeline.
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
/// measured correctly. The same run over `Generated/Plain.swift`, with the same
/// exclude list, reports no finding and exits 0.
///
/// swiftlint writes its own decode message at the START of a line, and it
/// writes the path echo after `Error: `. Measured, a pattern anchored on the
/// start of the line matches the decode message and does not match the path
/// echo, so the script anchors the test that way.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_measures_a_file_named_for_the_decode_message() {
    verify_shipped_staged_positions_report(&SWIFT_MAGIC_NUMBERS_DECODE_NAME_PROBE);
}

/// The `magic-numbers-swift` probe over a file whose name holds the words of
/// swiftlint's configuration message.
const SWIFT_MAGIC_NUMBERS_CONFIG_NAME_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        expected: NO_STAGED_REPORTS,
    },
    prompt_rule: MAGIC_NUMBERS_PROMPT_RULE,
    change_purpose: "a file whose name holds the words of swiftlint's configuration message",
    declarations: SWIFT_MAGIC_NUMBERS_STAGED,
    staged: SWIFT_CONFIG_NAME_POSITION_ONLY,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the project configuration is readable, so the run keeps the project exclude list \
             and reports nothing, whatever the file is named",
};

/// Acceptance: the shipped Swift magic-numbers tool rule MEASURES a run over a
/// file whose name holds the words of swiftlint's configuration message,
/// through the real swiftlint pipeline.
///
/// The same cause reaches the configuration test, and there it makes a WRONG
/// FINDING rather than a break. Measured with swiftlint 0.65.0 over this probe:
/// a test spelled `grep -qF 'Could not read configuration'` matched the path
/// echo, so the script wrote `swiftlint cannot read .swiftlint.yml beside this
/// rule`, ran swiftlint a second time with no project configuration, and
/// reported 1 finding on a file the project excludes.
///
/// The project configuration of this probe is the one every Swift probe of this
/// module stages, and swiftlint reads it without trouble, so the run must keep
/// the project's `excluded:` list and report nothing.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_measures_a_file_named_for_the_configuration_message() {
    verify_shipped_staged_positions_report(&SWIFT_MAGIC_NUMBERS_CONFIG_NAME_PROBE);
}

/// The `magic-numbers-swift` probe over a directory that holds no Swift file.
///
/// The probe writes no file at the path, and the one staged file under that
/// path makes the directory.
const SWIFT_MAGIC_NUMBERS_HOLLOW_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        expected: NO_FINDINGS,
    },
    prompt_rule: MAGIC_NUMBERS_PROMPT_RULE,
    change_purpose: SWIFT_HOLLOW_PURPOSE,
    path: SWIFT_HOLLOW_PATH,
    source: None,
    support: SWIFT_HOLLOW_FILES,
};

/// Acceptance: the shipped Swift magic-numbers tool rule answers CLEAN over a
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
fn the_shipped_swift_magic_numbers_tool_rule_stays_clean_over_a_hollow_directory() {
    verify_shipped_hollow_directory_answers_clean(&SWIFT_MAGIC_NUMBERS_HOLLOW_PROBE);
}

/// Every Swift file staged in the probe repository the magic-numbers script is
/// given none of.
const SWIFT_MAGIC_NUMBERS_UNREAD_FILES: &[(&str, &str)] = &[
    ("Top.swift", SWIFT_MAGIC_NUMBERS_STAGED),
    ("deep/nested/Other.swift", SWIFT_MAGIC_NUMBERS_STAGED),
];

/// Each finding the Swift magic-numbers script reports over the two files it
/// is given, as `path:line`.
const SWIFT_MAGIC_NUMBERS_READ_FINDINGS: &[&str] = &["Top.swift:2", "deep/nested/Other.swift:2"];

/// The `magic-numbers-swift` probe over a run that is given no file.
const SWIFT_MAGIC_NUMBERS_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        expected: NO_FINDINGS,
    },
    staged: SWIFT_MAGIC_NUMBERS_UNREAD_FILES,
    with_files: SWIFT_MAGIC_NUMBERS_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Swift magic-numbers tool rule reads only the files
/// it is given, through the real swiftlint pipeline.
///
/// `swiftlint lint` with no path argument walks the whole tree under the
/// working directory, and it exits 0, so the answer reads as a measured result
/// rather than a mistake. The script answers an empty argument list at once,
/// with no finding and an exit status of 0. The same script over the two
/// staged files reports 2.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&SWIFT_MAGIC_NUMBERS_EMPTY_RUN_PROBE);
}

/// A Python function that compares against an unnamed literal. `PLR2004`
/// reports the literal in the comparison and stays silent about the one the
/// `return` carries, so each file holds one finding.
const PYTHON_MAGIC_NUMBERS_JUDGED_SOURCE: &str = r#"def gate(value):
    if value == 42:
        return 7
    return 0
"#;

/// Every Python file staged in the probe repository the magic-numbers script
/// is given none of.
const PYTHON_MAGIC_NUMBERS_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.py", PYTHON_MAGIC_NUMBERS_JUDGED_SOURCE),
    ("deep/nested/other.py", PYTHON_MAGIC_NUMBERS_JUDGED_SOURCE),
];

/// Each finding the Python magic-numbers script reports over the two files it
/// is given, as `path:line`.
const PYTHON_MAGIC_NUMBERS_READ_FINDINGS: &[&str] = &["deep/nested/other.py:2", "top.py:2"];

/// The `magic-numbers-python` probe over a run that is given no file.
const PYTHON_MAGIC_NUMBERS_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_MAGIC_NUMBERS_RULE,
        expected: NO_FINDINGS,
    },
    staged: PYTHON_MAGIC_NUMBERS_UNREAD_FILES,
    with_files: PYTHON_MAGIC_NUMBERS_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Python magic-numbers tool rule reads only the
/// files it is given, through the real ruff pipeline.
///
/// `ruff check` with no path argument falls back to a default target of `.`.
/// Measured over this probe with no argument: without the guard the script
/// reported 2 findings and exited 0; with the guard it reports none and exits
/// 0. The same script over the two staged files reports 2.
#[test]
fn the_shipped_python_magic_numbers_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&PYTHON_MAGIC_NUMBERS_EMPTY_RUN_PROBE);
}

/// Where the Python file the magic-numbers run CAN judge stands, beside each
/// item it cannot judge.
const PYTHON_MAGIC_NUMBERS_JUDGED_PATH: &str = "judged.py";

/// The comparison the one finding of the judged file stands on.
const PYTHON_MAGIC_NUMBERS_COMPARISON: &str = "if value == 42:";

/// The `path:line` row the run must report for the judged file.
///
/// That row is what a nonzero exit over a declined item costs, so every probe
/// of a declined item holds the run to it.
fn python_magic_numbers_judged_row() -> String {
    expected_row(
        PYTHON_MAGIC_NUMBERS_JUDGED_PATH,
        PYTHON_MAGIC_NUMBERS_JUDGED_SOURCE,
        PYTHON_MAGIC_NUMBERS_COMPARISON,
    )
}

/// A Python file that does not parse: the parameter list of `broken` never
/// closes.
const PYTHON_MAGIC_NUMBERS_UNPARSABLE_SOURCE: &str = "def broken(\n";

/// Where the file ruff cannot parse stands inside the probe repository.
const PYTHON_MAGIC_NUMBERS_UNPARSABLE_PATH: &str = "broken.py";

/// Acceptance: the shipped Python magic-numbers tool rule DECLINES a Python
/// file it cannot parse, through the real ruff pipeline.
///
/// ruff writes a file it cannot parse onto the SAME report as a finding, under
/// `"code": "invalid-syntax"`. Measured with ruff 0.14.5 over `def broken(`
/// beside a function that compares against `42`: two rows on the report — the
/// `PLR2004` of the file it read AND the parse failure — at exit 1, and nothing
/// on stderr. ruff judged the other file, so the finding is there to lose.
///
/// The `.[]` of the pipe this rule replaced carried no `select`, so the parse
/// row became a magic-numbers FINDING that named a defect ruff never reported.
/// Measured with that pipe over the same two files: 2 findings at exit 0, one
/// of them `invalid-syntax unexpected EOF while parsing`. A filter that selects
/// `PLR2004` and drops the rest instead reads the unparsable file as clean, and
/// an `exit 1` fails the WHOLE run, so the finding of the file ruff DID judge
/// goes away with the file it did not. The parse failure is one declined item
/// of a sound run, so the script states it under the `sah-diagnostic:` marker
/// at exit 0.
#[test]
fn the_shipped_python_magic_numbers_tool_rule_declines_a_file_it_cannot_parse() {
    let expected = python_magic_numbers_judged_row();

    verify_unjudged_file_is_declined(
        PYTHON_PROJECT_TYPES,
        PYTHON_MAGIC_NUMBERS_RULE,
        &[
            (
                PYTHON_MAGIC_NUMBERS_JUDGED_PATH,
                PYTHON_MAGIC_NUMBERS_JUDGED_SOURCE,
            ),
            (
                PYTHON_MAGIC_NUMBERS_UNPARSABLE_PATH,
                PYTHON_MAGIC_NUMBERS_UNPARSABLE_SOURCE,
            ),
        ],
        PYTHON_MAGIC_NUMBERS_UNPARSABLE_PATH,
        &[&expected],
    );
}

/// Where the path the magic-numbers run cannot read stands inside the probe
/// repository.
///
/// One name serves all three shapes: the same path holds no file, holds bytes
/// that are not UTF-8, or holds source nobody may read, so the way it refuses
/// is the one difference between the three probes.
const PYTHON_MAGIC_NUMBERS_UNREADABLE_PATH: &str = "unreadable.py";

/// A Python file whose bytes are not UTF-8.
///
/// The literal holds two bytes that open no UTF-8 sequence, so a reader opens
/// the file and cannot decode it. The assignment is no comparison, so a run
/// that DID read the file would report nothing.
const PYTHON_MAGIC_NUMBERS_UNDECODABLE_SOURCE: &[u8] = b"VALUE = '\xff\xfe'\n";

/// A Python file the tool could read if the mode let it.
///
/// The assignment names its own value and stands in no comparison, so a run
/// that DID read the file would report no finding — which is the clean answer
/// this rule must not give for a file it never read.
const PYTHON_MAGIC_NUMBERS_FORBIDDEN_SOURCE: &str = "LIMIT = 42\n";

/// The `magic-numbers-python` probe over a refusing path beside `judged.py`.
///
/// The judged file carries one unnamed comparison literal, so the run has a
/// finding to lose. Losing it is what a nonzero exit over a declined item
/// costs, and staying silent about the path is what reads that path as a clean
/// file.
fn python_magic_numbers_decline_probe() -> ShippedDeclineProbe {
    ShippedDeclineProbe {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_MAGIC_NUMBERS_RULE,
        judged: vec![(
            PYTHON_MAGIC_NUMBERS_JUDGED_PATH,
            PYTHON_MAGIC_NUMBERS_JUDGED_SOURCE.to_string(),
        )],
        path: PYTHON_MAGIC_NUMBERS_UNREADABLE_PATH,
        expected: vec![python_magic_numbers_judged_row()],
    }
}

/// Acceptance: the shipped Python magic-numbers tool rule DECLINES a path that
/// holds no file, through the real ruff pipeline.
///
/// Measured with ruff 0.14.5 over such a path beside `judged.py`, against the
/// shipped command line: the report holds the `PLR2004` of the other file and
/// nothing for this path, `warning: Failed to lint absent.py: No such file or
/// directory (os error 2)` stands on stderr, and ruff exits 1 as it would
/// without the path. The report reads exactly like a clean file, so the answer
/// has to come from what ruff itself said.
#[test]
fn the_shipped_python_magic_numbers_tool_rule_declines_a_path_that_holds_no_file() {
    verify_unreadable_file_is_declined(
        &python_magic_numbers_decline_probe(),
        &ShippedUnreadableFile::Absent,
    );
}

/// Acceptance: the shipped Python magic-numbers tool rule DECLINES a file whose
/// bytes are not UTF-8, through the real ruff pipeline.
///
/// Measured with ruff 0.14.5 over such a file beside `judged.py`, against the
/// shipped command line: the report holds the `PLR2004` of the other file,
/// `warning: Failed to lint notutf8.py: stream did not contain valid UTF-8`
/// stands on stderr, and ruff exits 1 as it would without the file.
///
/// The pipe this rule replaced read the report alone. Measured with that pipe
/// over the same two files: one finding at exit 0, and ruff's own UNMARKED
/// `Failed to lint` line on stderr, which the engine drops as tool chatter. The
/// file read as CLEAN.
#[test]
fn the_shipped_python_magic_numbers_tool_rule_declines_a_file_it_cannot_decode() {
    verify_unreadable_file_is_declined(
        &python_magic_numbers_decline_probe(),
        &ShippedUnreadableFile::Undecodable(PYTHON_MAGIC_NUMBERS_UNDECODABLE_SOURCE),
    );
}

/// Acceptance: the shipped Python magic-numbers tool rule DECLINES a file it
/// may not read, through the real ruff pipeline.
///
/// Measured with ruff 0.14.5 over such a file beside `judged.py`, against the
/// shipped command line: the report holds the `PLR2004` of the other file,
/// `warning: Failed to lint noread.py: Permission denied (os error 13)` stands
/// on stderr, and ruff exits 1 as it would without the file.
///
/// The probe takes every permission off the file, which is a mode, so it runs
/// on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_python_magic_numbers_tool_rule_declines_a_file_it_may_not_read() {
    verify_unreadable_file_is_declined(
        &python_magic_numbers_decline_probe(),
        &ShippedUnreadableFile::Forbidden(PYTHON_MAGIC_NUMBERS_FORBIDDEN_SOURCE),
    );
}

/// Where the directory nobody may read stands inside the probe repository.
///
/// The name carries the Python extension, because the engine hands a
/// `files`-scope run the paths its work-list holds, and a path the rule's own
/// file pattern refuses reaches no run at all.
const PYTHON_MAGIC_NUMBERS_UNREADABLE_DIRECTORY: &str = "unread.py";

/// What ruff says for a directory it may not read, with its `warning: ` head
/// taken off.
///
/// The line names NO path. ruff walks the path it is given, and a directory it
/// may not open stops the walk before it reaches a file of its own to name.
const PYTHON_MAGIC_NUMBERS_DIRECTORY_REFUSAL: &str =
    "Encountered error: Permission denied (os error 13)";

/// Acceptance: the shipped Python magic-numbers tool rule DECLINES a directory
/// it may not read, through the real ruff pipeline.
///
/// A directory refuses ruff under another head than a file does. Measured with
/// ruff 0.14.5 over `judged.py` beside a mode-000 directory: the report holds
/// the `PLR2004` of the file it judged, ruff exits 1, and stderr carries
/// `warning: Encountered error: Permission denied (os error 13)` — a head that
/// is NOT `warning: Failed to lint `, and a line that carries no path. That is
/// why the script forwards the WHOLE stderr channel rather than one head.
///
/// The probe takes every permission off the directory, which is a mode, so it
/// runs on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_python_magic_numbers_tool_rule_declines_a_directory_it_may_not_read() {
    let expected = python_magic_numbers_judged_row();
    let judged = [(
        PYTHON_MAGIC_NUMBERS_JUDGED_PATH,
        PYTHON_MAGIC_NUMBERS_JUDGED_SOURCE,
    )];
    let prepare =
        |repo: &Path| forbid_probe_directory(&repo.join(PYTHON_MAGIC_NUMBERS_UNREADABLE_DIRECTORY));
    let restore = |repo: &Path| {
        restore_probe_directory(&repo.join(PYTHON_MAGIC_NUMBERS_UNREADABLE_DIRECTORY))
    };
    let staging = ShippedStaging {
        prepare: &prepare,
        restore: &restore,
        ..ShippedStaging::of(&judged)
    };

    verify_declined_item_is_stated(
        PYTHON_PROJECT_TYPES,
        PYTHON_MAGIC_NUMBERS_RULE,
        &staging,
        &[
            PYTHON_MAGIC_NUMBERS_JUDGED_PATH,
            PYTHON_MAGIC_NUMBERS_UNREADABLE_DIRECTORY,
        ],
        PYTHON_MAGIC_NUMBERS_DIRECTORY_REFUSAL,
        &[&expected],
    );
}

/// The name the shipped Python magic-numbers script calls the linter by.
const PYTHON_MAGIC_NUMBERS_TOOL_BINARY_NAME: &str = "ruff";

/// A stubbed ruff that refuses its command line: it writes its own error to
/// stderr, writes no report at all, and exits 2.
///
/// This is the shape the real ruff takes for a selector it cannot read and for
/// an output format it cannot read — measured, each one wrote 0 bytes to stdout
/// and exited 2.
const PYTHON_MAGIC_NUMBERS_REFUSED_ANSWER: &str = concat!(
    "  printf 'error: invalid value for --select <RULE_CODE>\\n' >&2\n",
    "  exit 2"
);

/// What the run must say when ruff refuses its command line.
const PYTHON_MAGIC_NUMBERS_REFUSED_MESSAGE: &str = "ruff exited 2 and judged no code";

/// Why a status outside the two findings statuses breaks the run.
const PYTHON_MAGIC_NUMBERS_REFUSED_REASON: &str =
    "ruff writes no report for a command line it refuses, so a pipe ending in `jq` reads \
     that run as a clean tree";

/// Acceptance: the shipped Python magic-numbers tool rule BREAKS on a ruff that
/// refuses its command line.
///
/// ruff exits 0 for a file with no finding and 1 for a file with one. Every
/// other status is a run that judged nothing: measured with ruff 0.14.5,
/// `--select ZZ999` and `--output-format zzz` each wrote 0 bytes of report and
/// exited 2.
///
/// The shape this rule replaced was one pipe, and a pipeline takes the status
/// of its LAST command. Measured with a stub of this shape against that pipe:
/// exit 0 and 0 bytes on stdout — a broken tool reading as a clean tree.
///
/// The probe leads `PATH` with a stub, which is process state, so it stands
/// under `#[serial_test::serial(env)]`.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_python_magic_numbers_tool_rule_breaks_on_a_status_it_cannot_read() {
    let run = drive_shipped_script_with_stub(
        PYTHON_PROJECT_TYPES,
        PYTHON_MAGIC_NUMBERS_RULE,
        &[(
            PYTHON_MAGIC_NUMBERS_JUDGED_PATH,
            PYTHON_MAGIC_NUMBERS_JUDGED_SOURCE,
        )],
        &[PYTHON_MAGIC_NUMBERS_JUDGED_PATH],
        PYTHON_MAGIC_NUMBERS_TOOL_BINARY_NAME,
        PYTHON_MAGIC_NUMBERS_REFUSED_ANSWER,
    );
    let failure = run.outcome.expect_err(PYTHON_MAGIC_NUMBERS_REFUSED_REASON);

    assert_shipped_break(
        &failure,
        run.status,
        &[PYTHON_MAGIC_NUMBERS_REFUSED_MESSAGE],
        PYTHON_MAGIC_NUMBERS_REFUSED_REASON,
    );
    assert!(
        run.placed.is_empty(),
        "a run that breaks must place no finding; it placed {:?}: \
         {PYTHON_MAGIC_NUMBERS_REFUSED_REASON}",
        run.placed
    );
}

/// What the run must say when `jq` could not read what ruff wrote.
const PYTHON_MAGIC_NUMBERS_FILTER_BROKEN_MESSAGE: &str =
    "magic-numbers-python: jq could not read the ruff report";

/// Why a report `jq` cannot read must break the run rather than pass it.
const PYTHON_MAGIC_NUMBERS_FILTER_BROKEN_REASON: &str =
    "a report the filter could not read leaves the run with no measurement at all, so it \
     must break in the rule's own words rather than exit on the filter's status";

/// A stubbed ruff that exits 1 and writes a report that stops in the middle of
/// its first entry.
///
/// Status 1 is the status ruff exits with for a file that HAS findings, so the
/// broken-run gate lets it through. The report is what the filter then cannot
/// read.
const PYTHON_MAGIC_NUMBERS_TRUNCATED_REPORT_ANSWER: &str =
    "  printf '[\\n  {\\n    \"code\": \"PLR2004\"'\n  exit 1";

/// Acceptance: the shipped Python magic-numbers tool rule BREAKS when ruff
/// writes a report the filter cannot read.
///
/// The broken-run gate reads the STATUS, and ruff keeps status 1 for a file
/// with findings, so a malformed report at status 1 passes the gate. The filter
/// then fails, and a filter that runs bare under `set -e` takes the whole script
/// down with its own status, which names no rule at all. Each filter step
/// therefore reads its own status, and one gate states the break in the rule's
/// own words.
///
/// The probe leads `PATH` with a stub, which is process state, so it stands
/// under `#[serial_test::serial(env)]`.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_python_magic_numbers_tool_rule_breaks_on_a_report_the_filter_cannot_read() {
    let run = drive_shipped_script_with_stub(
        PYTHON_PROJECT_TYPES,
        PYTHON_MAGIC_NUMBERS_RULE,
        &[(
            PYTHON_MAGIC_NUMBERS_JUDGED_PATH,
            PYTHON_MAGIC_NUMBERS_JUDGED_SOURCE,
        )],
        &[PYTHON_MAGIC_NUMBERS_JUDGED_PATH],
        PYTHON_MAGIC_NUMBERS_TOOL_BINARY_NAME,
        PYTHON_MAGIC_NUMBERS_TRUNCATED_REPORT_ANSWER,
    );
    let failure = run
        .outcome
        .expect_err(PYTHON_MAGIC_NUMBERS_FILTER_BROKEN_REASON);

    assert_shipped_break(
        &failure,
        run.status,
        &[PYTHON_MAGIC_NUMBERS_FILTER_BROKEN_MESSAGE],
        PYTHON_MAGIC_NUMBERS_FILTER_BROKEN_REASON,
    );
    assert!(
        run.placed.is_empty(),
        "a run that breaks must place no finding; it placed {:?}: \
         {PYTHON_MAGIC_NUMBERS_FILTER_BROKEN_REASON}",
        run.placed
    );
}

/// A TypeScript function that compares against an unnamed literal and returns
/// another one. `@typescript-eslint/no-magic-numbers` reports both, so each
/// file holds two findings.
const TYPESCRIPT_MAGIC_NUMBERS_UNREAD_SOURCE: &str = r#"export function gate(value: number): number {
  if (value === 42) {
    return 7;
  }
  return 0;
}
"#;

/// Every TypeScript file staged in the probe repository the magic-numbers
/// script is given none of.
const TYPESCRIPT_MAGIC_NUMBERS_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.ts", TYPESCRIPT_MAGIC_NUMBERS_UNREAD_SOURCE),
    (
        "deep/nested/other.ts",
        TYPESCRIPT_MAGIC_NUMBERS_UNREAD_SOURCE,
    ),
];

/// Each finding the TypeScript magic-numbers script reports over the two
/// files it is given, as `path:line`.
const TYPESCRIPT_MAGIC_NUMBERS_READ_FINDINGS: &[&str] = &[
    "deep/nested/other.ts:2",
    "deep/nested/other.ts:3",
    "top.ts:2",
    "top.ts:3",
];

/// The `magic-numbers-typescript` probe over a run that is given no file.
const TYPESCRIPT_MAGIC_NUMBERS_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_MAGIC_NUMBERS_RULE,
        expected: NO_FINDINGS,
    },
    staged: TYPESCRIPT_MAGIC_NUMBERS_UNREAD_FILES,
    with_files: TYPESCRIPT_MAGIC_NUMBERS_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped TypeScript magic-numbers tool rule reads only the
/// files it is given, through the real eslint pipeline.
///
/// eslint with no path argument reads the working directory, and the config
/// this rule writes names `**/*.{js,jsx,mjs,cjs,ts,tsx}`. Measured over this
/// probe with no argument: without the guard the script reported 4 findings
/// and exited 0; with the guard it reports none and exits 0. The same script
/// over the two staged files reports 4.
#[test]
fn the_shipped_typescript_magic_numbers_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&TYPESCRIPT_MAGIC_NUMBERS_EMPTY_RUN_PROBE);
}

/// A Dart method that compares against an unnamed literal and returns another
/// one. `no_magic_number` reports both, so each file holds two findings.
const DART_MAGIC_NUMBERS_UNREAD_SOURCE: &str = r#"class Widget {
  int gate(int value) {
    if (value > 42) {
      return 7;
    }
    return 0;
  }
}
"#;

/// Every Dart file staged in the probe repository the magic-numbers script is
/// given none of.
const DART_MAGIC_NUMBERS_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.dart", DART_MAGIC_NUMBERS_UNREAD_SOURCE),
    ("deep/nested/other.dart", DART_MAGIC_NUMBERS_UNREAD_SOURCE),
];

/// Each finding the Dart magic-numbers script reports over the two files it
/// is given, as `path:line`.
const DART_MAGIC_NUMBERS_READ_FINDINGS: &[&str] = &[
    "deep/nested/other.dart:3",
    "deep/nested/other.dart:4",
    "top.dart:3",
    "top.dart:4",
];

/// The `magic-numbers-dart` probe over a run that is given no file.
const DART_MAGIC_NUMBERS_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MAGIC_NUMBERS_RULE,
        expected: NO_FINDINGS,
    },
    staged: DART_MAGIC_NUMBERS_UNREAD_FILES,
    with_files: DART_MAGIC_NUMBERS_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Dart magic-numbers tool rule reads only the files
/// it is given, through the real custom_lint pipeline.
///
/// This script copies each file it is given into a package it makes, and it
/// runs the tool inside that package, so the tool holds no default target
/// that could reach the probe tree. Measured over this probe with no
/// argument: the script reported 0 findings and exited 0 both without the
/// guard and with it, and the same script over the two staged files reports
/// 4. The guard is what keeps the script from making that package, resolving
/// its dependencies and running the tool for a run with nothing to judge.
#[test]
fn the_shipped_dart_magic_numbers_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&DART_MAGIC_NUMBERS_EMPTY_RUN_PROBE);
}
