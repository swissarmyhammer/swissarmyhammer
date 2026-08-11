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
