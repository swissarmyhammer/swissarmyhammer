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
    source: Some(SWIFT_MAGIC_NUMBERS_STAGED),
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

/// Where the Swift file that is never written stands inside the probe
/// repository.
const SWIFT_MAGIC_NUMBERS_ABSENT_PATH: &str = "Sources/Absent.swift";

/// What the one error of an absent file must name.
const SWIFT_MAGIC_NUMBERS_ABSENT_ERROR: &[&str] = &[
    "magic-numbers-swift cannot read",
    SWIFT_MAGIC_NUMBERS_ABSENT_PATH,
];

/// The `magic-numbers-swift` probe over a path that holds no file.
const SWIFT_MAGIC_NUMBERS_ABSENT_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        expected: SWIFT_MAGIC_NUMBERS_ABSENT_ERROR,
    },
    prompt_rule: MAGIC_NUMBERS_PROMPT_RULE,
    change_purpose: "a Swift file that is not there",
    path: SWIFT_MAGIC_NUMBERS_ABSENT_PATH,
    source: None,
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift magic-numbers tool rule BREAKS on a file it
/// cannot read, through the real swiftlint pipeline.
///
/// swiftlint exits 1 for a path that is not there and writes nothing to
/// stdout. A pipeline takes the exit status of its LAST command, and that
/// command was `jq`, so the earlier pipe exited 0 and reported nothing — a run
/// answering zero for a reason other than a clean file.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_breaks_on_a_file_it_cannot_read() {
    verify_shipped_run_breaks(&SWIFT_MAGIC_NUMBERS_ABSENT_PROBE);
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

/// The `magic-numbers-swift` probe over a run that is given no file.
const SWIFT_MAGIC_NUMBERS_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MAGIC_NUMBERS_RULE,
        expected: NO_FINDINGS,
    },
    staged: SWIFT_MAGIC_NUMBERS_UNREAD_FILES,
    reason: "the script judges the files it is given and no other: given none, it reports none \
             and exits 0, and the staged tree stays unread",
};

/// Acceptance: the shipped Swift magic-numbers tool rule reads only the files
/// it is given, through the real swiftlint pipeline.
///
/// `swiftlint lint` with no path argument walks the whole tree under the
/// working directory, and it exits 0, so the answer reads as a measured result
/// rather than a mistake. The script answers an empty argument list at once,
/// with no finding and an exit status of 0.
#[test]
fn the_shipped_swift_magic_numbers_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&SWIFT_MAGIC_NUMBERS_EMPTY_RUN_PROBE);
}
