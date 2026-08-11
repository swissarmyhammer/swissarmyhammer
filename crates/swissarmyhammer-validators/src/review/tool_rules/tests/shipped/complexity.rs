//! Acceptance tests for the shipped complexity tool rules.
//!
//! One test holds the whole roster to its fixture pair and to the prompt rules
//! each tool decides. The tests under it drive Rust and TypeScript through
//! their real tools.

use super::*;

/// Acceptance: every shipped complexity tool rule passes its fixture pair
/// in doctor, and supersedes exactly the gates its own tool decides.
///
/// The `supersedes` assertion is the load-bearing half. `complexity-rust`
/// must name both prompt rules, because one `cargo clippy` run answers
/// both; naming only one would leave an agent re-reading the probe for the
/// gate the tool already decided. The two Python rules must name one each,
/// because ruff decides one gate per lint; naming both from either rule
/// would silence a gate no tool measures.
#[test]
fn every_shipped_complexity_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(
        SHIPPED_COMPLEXITY_RULES,
        COGNITIVE_COMPLEXITY_PROMPT_RULE,
    );
}

/// A cargo package holding one function over the nesting gate and nothing
/// else the four lints report. `[workspace]` keeps cargo inside the
/// temporary directory.
const COMPLEX_PACKAGE_MANIFEST: &str = concat!(
    "[package]\nname = \"complex-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[workspace]\n",
);

/// The library of [`COMPLEX_PACKAGE_MANIFEST`]. `fold_grid` is a free
/// function at control-flow depth 6, and a free function body is itself one
/// level, so its innermost block sits at nesting level 7 against the gate
/// of 6. The Rust tool rule must report it once. The body stays well under
/// the line gate and takes two arguments, so the same run reports nothing
/// else.
const COMPLEX_LIB_RS: &str = r#"//! A probe crate for the shipped Rust complexity tool rule.

/// Folds a grid of readings into one band, one nested block for each test.
pub fn fold_grid(grid: &[Vec<i32>], limit: i32) -> i32 {
    let mut band = 0;
    for row in grid {
        for cell in row {
            if *cell > 0 {
                if *cell < limit {
                    while band < *cell {
                        if band % 2 == 0 {
                            band += 2;
                        }
                        band += 1;
                    }
                }
            }
        }
    }
    band
}
"#;

/// The library path inside the complexity probe package, as the work-list
/// holds it.
const COMPLEX_LIB_PATH: &str = "src/lib.rs";

/// A one-validator work-list over `path` for the builtin `code-hygiene`
/// set, naming both complexity prompt rules and the tool rule `rule`.
///
/// `rule` is a parameter because two languages drive this shape end to end:
/// `complexity-rust` for the nesting gate, and `complexity-typescript` for
/// the test carve-out.
fn complexity_work(rule: &str, path: &str, content: &str) -> WorkList {
    WorkList::new(
        "a function over a complexity gate",
        vec![ValidatorWork::new(
            CODE_HYGIENE_SET,
            RuleNames::new([
                COGNITIVE_COMPLEXITY_PROMPT_RULE.to_string(),
                FUNCTION_LENGTH_PROMPT_RULE.to_string(),
                rule.to_string(),
            ]),
            ProbeNames::new([]),
            [FileWork::new(path, vec![], vec![], content, vec![])],
        )],
    )
}

/// Acceptance: the shipped Rust complexity tool rule reports an over-complex
/// function on a real cargo workspace, through the real clippy pipeline,
/// and suppresses both prompt rules it supersedes.
///
/// The suppression half is what a rule that supersedes two names buys. A
/// healthy `complexity-rust` must silence `cognitive-complexity` AND
/// `function-length` for the file, so no LLM re-reads a gate the tool
/// already decided.
///
/// The reporting half also proves the threshold reached clippy.
/// `excessive-nesting-threshold` defaults to `0`, which turns the lint off
/// altogether, so the probe reports only when the script's temporary
/// `clippy.toml` is the file `CLIPPY_CONF_DIR` names.
#[test]
fn the_shipped_rust_complexity_tool_rule_reports_an_over_complex_function() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("Cargo.toml"), COMPLEX_PACKAGE_MANIFEST).unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(repo.path().join(COMPLEX_LIB_PATH), COMPLEX_LIB_RS).unwrap();
    let loader = builtin_loader();
    let project_types = ["rust"];
    require_tool_installed(&loader, &project_types, RUST_COMPLEXITY_RULE);
    let work = complexity_work(RUST_COMPLEXITY_RULE, COMPLEX_LIB_PATH, COMPLEX_LIB_RS);

    let plan = plan_tool_rules(&work, &loader, &project_types, None);

    let run = required_run(&plan, RUST_COMPLEXITY_RULE);
    assert_eq!(run.files(), [COMPLEX_LIB_PATH.to_string()]);
    let suppressed = plan
        .suppression()
        .suppressed_rules(CODE_HYGIENE_SET, COMPLEX_LIB_PATH);
    for prompt_rule in SUPERSEDES_BOTH_COMPLEXITY_GATES {
        assert!(
            suppressed.contains(*prompt_rule),
            "a healthy tool rule that supersedes two prompt rules must suppress both; \
             `{prompt_rule}` is missing from {suppressed:?}"
        );
    }

    verify_run_reports_one_finding(
        run,
        repo.path(),
        COMPLEX_LIB_PATH,
        CODE_HYGIENE_SET,
        RUST_COMPLEXITY_RULE,
        "too nested",
    );
}

/// The materialized name of the `complexity-typescript` fail fixture.
const TYPESCRIPT_COMPLEXITY_FAIL_FIXTURE: &str = "complexity-typescript.fail.ts";

/// Where the fail fixture stands inside the probe repository, as the
/// work-list holds it.
const TYPESCRIPT_COMPLEXITY_FIXTURE_PATH: &str = "src/complexity-typescript-fail.ts";

/// The start of the source line each guard in the `complexity-typescript`
/// fail fixture is reported at.
///
/// Both gates report at the head of the function they measure, and for a
/// method and for an accessor that head is the member's NAME. Each entry is
/// therefore the text the gate points at, not the name alone.
const TYPESCRIPT_COMPLEXITY_FAIL_GUARDS: &[&str] = &[
    "function foldGrid(",
    "function mixState(",
    "foldRows(",
    "get band(",
    "context.run(",
    "context.each(rows)(",
    "context.for(rows)(",
    "step(\"build the grid\", (",
];

/// The `complexity-typescript` fail fixture, and every guard the real eslint
/// pipeline must measure inside it.
const TYPESCRIPT_COMPLEXITY_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: &["nodejs"],
        rule: TYPESCRIPT_COMPLEXITY_RULE,
        expected: TYPESCRIPT_COMPLEXITY_FAIL_GUARDS,
    },
    fixture: TYPESCRIPT_COMPLEXITY_FAIL_FIXTURE,
    path: TYPESCRIPT_COMPLEXITY_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "guard",
};

/// Acceptance: the shipped TypeScript complexity tool rule measures every
/// guard its fail fixture holds, through the real eslint pipeline.
///
/// A guard is held to the SOURCE LINE its finding stands on, because both
/// gates report at the head of the function they measure, and that head is a
/// member's name for a method and for an accessor.
///
/// Six of the eight are the shapes a carve-out too broad in one direction
/// loses in silence, and all six stand inside a `describe` block. A class
/// method and an accessor report at their NAME, which stands outside the
/// function node the gates measure, so a lookup over the function ranges
/// alone climbs to the test callback and exempts them. `context.run(...)`,
/// `context.each(rows)(...)` and `context.for(rows)(...)` are calls whose
/// root identifier is a test-framework name and which are not test-framework
/// calls: a mark that reads the root identifier alone exempts the first, and
/// a mark that reads any pair of a test name and a modifier name exempts the
/// other two. A bare `step(...)` is a call to a name only Playwright spells,
/// and Playwright spells it `test.step` alone, so a mark that takes `step`
/// with no root exempts a build step, a wizard step and a saga step.
#[test]
fn the_shipped_typescript_complexity_tool_rule_measures_every_fail_fixture_guard() {
    verify_shipped_fail_fixture_reports_each(
        &TYPESCRIPT_COMPLEXITY_FAIL_PROBE,
        |content| {
            complexity_work(
                TYPESCRIPT_COMPLEXITY_RULE,
                TYPESCRIPT_COMPLEXITY_FIXTURE_PATH,
                content,
            )
        },
        fail_fixture_source_line,
        |reported, guard| reported.starts_with(guard),
    );
}

/// The line of a `tool.run` script that resolves the node module tree.
const NODE_MODULES_LINE_PREFIX: &str = "modules=";

/// The here-document that opens the eslint config inside the `tool.run`
/// script, and the word that closes it.
const ESLINT_CONFIG_OPEN: &str = "<<'ESLINT_CONFIG'";

/// The word that closes [`ESLINT_CONFIG_OPEN`].
const ESLINT_CONFIG_CLOSE: &str = "ESLINT_CONFIG";

/// How far the `tool.run` block indents the script inside the rule's YAML
/// front matter.
const RUN_BLOCK_INDENT: &str = "    ";

/// The shell line that resolves the node module tree, and the eslint config
/// body, both read out of `source`.
///
/// Reading them out of the rule keeps the probe on the SHIPPED text: a test
/// that wrote its own copy of either would answer for the copy.
fn shipped_eslint_config(source: &str) -> (String, String) {
    let resolve: Vec<&str> = source
        .lines()
        .filter(|line| line.trim_start().starts_with(NODE_MODULES_LINE_PREFIX))
        .collect();
    assert_eq!(
        resolve.len(),
        1,
        "the rule must resolve the node module tree on exactly one line; got {resolve:?}"
    );
    let opens = source
        .lines()
        .position(|line| line.trim_end().ends_with(ESLINT_CONFIG_OPEN))
        .expect("the rule must write its eslint config through a here-document");
    let body: Vec<&str> = source
        .lines()
        .skip(opens + 1)
        .take_while(|line| line.trim() != ESLINT_CONFIG_CLOSE)
        .map(|line| line.strip_prefix(RUN_BLOCK_INDENT).unwrap_or(line))
        .collect();
    assert!(
        !body.is_empty(),
        "the here-document must hold the eslint config"
    );
    (resolve[0].trim_start().to_string(), body.join("\n"))
}

/// The lines appended to the shipped eslint config so node prints what the
/// config's own read of the framework names answered.
const FRAMEWORK_NAME_REPORT: &str = concat!(
    "console.log(JSON.stringify({",
    "read: FRAMEWORK_NAMES.functions, readModern: FRAMEWORK_NAMES.modern, ",
    "mirror: MIRROR_FRAMEWORK_FUNCTION, mirrorModern: MIRROR_MODERN_FUNCTION, ",
    "rooted: Array.from(FRAMEWORK_CALL).filter((entry) => entry[1].rooted)",
    ".map((entry) => entry[0])",
    "}));",
);

/// The Mocha and Jest openers and hooks a hand-written framework list left
/// out, each read from `globals.mocha` or `globals.jest`.
///
/// `before` and `after` are Mocha's own suite hooks, and not the `beforeAll`
/// and `afterAll` Jest and Vitest spell.
const TYPESCRIPT_FRAMEWORK_GLOBALS: &[&str] = &[
    "before",
    "after",
    "setup",
    "teardown",
    "suiteSetup",
    "suiteTeardown",
    "specify",
    "xdescribe",
    "xcontext",
    "xit",
    "xspecify",
    "fit",
    "xtest",
];

/// The names `globals.mocha` and `globals.jest` hold that open no test, and
/// which the read must therefore drop.
///
/// `mocha` and `jest` are the framework namespace objects, `expect` is the
/// assertion entry, and `run` is Mocha's delayed-start runner. `step` opens
/// a test only under the `test` root, so it stands outside the read as well.
const TYPESCRIPT_NOT_AN_OPENER: &[&str] = &["mocha", "jest", "expect", "run", "step"];

/// The one framework function that needs a framework root before it.
const TYPESCRIPT_ROOTED_CALL: &[&str] = &["step"];

/// Acceptance: the shipped `complexity-typescript` config READS its framework
/// function names out of the resolved node module tree, and the written
/// mirror it falls back to says the same thing.
///
/// Three review rounds each found a hand-written list of framework spellings
/// wrong in one direction or the other. The list is therefore read: from
/// `TEST_FRAMEWORK_STRUCTURE_FUNCTIONS` in `eslint-plugin-sonarjs`, and from
/// `globals.mocha` and `globals.jest` in `globals`, which that plugin
/// declares as a dependency. Neither is a new dependency, and the rule's own
/// install command brings both.
///
/// The test runs the SHIPPED config under node, through the rule's own
/// resolution line, and holds three facts. The read answers without falling
/// back — an empty standard error proves that, because the fallback writes
/// the resolution error there. The mirror equals the read, so a `globals`
/// release that adds an opener fails here rather than in silence. And the
/// read holds every name the two review rounds named, holds none of the five
/// names that open no test, and marks `step` as needing a root.
#[test]
fn the_shipped_typescript_complexity_config_reads_its_framework_names() {
    let loader = builtin_loader();
    let project_types = ["nodejs"];
    require_tool_installed(&loader, &project_types, TYPESCRIPT_COMPLEXITY_RULE);
    let source = std::fs::read_to_string(shipped_asset(
        &loader,
        &RULE_SOURCE_ASSET,
        TYPESCRIPT_COMPLEXITY_RULE,
    ))
    .expect("read the shipped TypeScript complexity rule");
    let (resolve, config) = shipped_eslint_config(&source);
    let probe = tempfile::tempdir().unwrap();
    let script = probe.path().join("read-framework-names.cjs");
    std::fs::write(&script, format!("{config}\n{FRAMEWORK_NAME_REPORT}\n")).unwrap();

    let output = std::process::Command::new("bash")
        .arg("-c")
        .arg(format!("{resolve}\nNODE_PATH=\"$modules\" node \"$1\""))
        .arg("read-framework-names")
        .arg(&script)
        .output()
        .expect("run the shipped eslint config under node");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the shipped eslint config must load under node; stderr: {stderr}"
    );
    assert!(
        stderr.is_empty(),
        "the config must READ the framework names rather than fall back to its \
         mirror; it wrote: {stderr}"
    );
    let names: FrameworkNames = serde_json::from_slice(&output.stdout)
        .expect("the shipped eslint config must print its framework names");
    assert_eq!(
        sorted_names(&names.read),
        sorted_names(&names.mirror),
        "the mirror in the rule must say what the read says"
    );
    assert_eq!(
        sorted_names(&names.read_modern),
        sorted_names(&names.mirror_modern),
        "the mirror of the modern-modifier names must say what the read says"
    );
    for name in TYPESCRIPT_FRAMEWORK_GLOBALS {
        assert!(
            names.read.iter().any(|read| read == name),
            "`{name}` is a Mocha or Jest opener, so the read must hold it; got {:?}",
            names.read
        );
    }
    for name in TYPESCRIPT_NOT_AN_OPENER {
        assert!(
            !names.read.iter().any(|read| read == name),
            "`{name}` opens no test, so the read must drop it; got {:?}",
            names.read
        );
    }
    assert_eq!(
        names.rooted, TYPESCRIPT_ROOTED_CALL,
        "only a framework function no framework spells bare needs a root"
    );
}

/// What [`FRAMEWORK_NAME_REPORT`] prints out of the shipped eslint config.
#[derive(serde::Deserialize)]
struct FrameworkNames {
    /// The framework function names the config read out of the tree.
    read: Vec<String>,
    /// The read names that accept the Jest, Vitest and Playwright modifiers.
    #[serde(rename = "readModern")]
    read_modern: Vec<String>,
    /// The written mirror of `read`.
    mirror: Vec<String>,
    /// The written mirror of `read_modern`.
    #[serde(rename = "mirrorModern")]
    mirror_modern: Vec<String>,
    /// The framework functions that need a framework root before them.
    rooted: Vec<String>,
}

/// The declarations every staged Swift position holds: one function whose
/// cyclomatic complexity is 16.
///
/// The gate the rule states is 15, and `cyclomatic_complexity` reports only
/// over the gate, so 16 `if` statements are one finding and no more. Measured:
/// swiftlint answers `Function should have complexity 15 or less; currently
/// complexity is 16`.
const SWIFT_COMPLEXITY_STAGED: &str = concat!(
    "func branchy(_ n: Int) -> Int {\n",
    "    var total = 0\n",
    "    if n > 1 { total += 1 }\n",
    "    if n > 2 { total += 1 }\n",
    "    if n > 3 { total += 1 }\n",
    "    if n > 4 { total += 1 }\n",
    "    if n > 5 { total += 1 }\n",
    "    if n > 6 { total += 1 }\n",
    "    if n > 7 { total += 1 }\n",
    "    if n > 8 { total += 1 }\n",
    "    if n > 9 { total += 1 }\n",
    "    if n > 10 { total += 1 }\n",
    "    if n > 11 { total += 1 }\n",
    "    if n > 12 { total += 1 }\n",
    "    if n > 13 { total += 1 }\n",
    "    if n > 14 { total += 1 }\n",
    "    if n > 15 { total += 1 }\n",
    "    if n > 16 { total += 1 }\n",
    "    return total\n",
    "}\n",
);

/// The file of the one finding the staged Swift positions must report.
const SWIFT_COMPLEXITY_STAGED_REPORTED: &[&str] = &[SWIFT_ORDINARY_POSITION.path];

/// The staged Swift positions, and the one of them the real swiftlint pipeline
/// must report.
const SWIFT_COMPLEXITY_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: SWIFT_COMPLEXITY_STAGED_REPORTED,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function over the complexity gate, staged in two positions",
    declarations: SWIFT_COMPLEXITY_STAGED,
    staged: SWIFT_EXCLUDE_POSITIONS,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the ordinary file reports its function, and the file under the project's \
             excluded directory reports nothing",
};

/// Acceptance: the shipped Swift complexity tool rule honours the project's
/// own `excluded:` list, through the real swiftlint pipeline.
///
/// Both prompt rules this rule supersedes carve out generated code, and
/// swiftlint holds no generated-code check of its own, so the project's
/// `excluded:` list is the whole carve-out for a generated file.
///
/// The two files hold the same bytes on purpose. The list is the only
/// difference between the file that reports and the file that stays silent.
#[test]
fn the_shipped_swift_complexity_tool_rule_reads_the_project_exclude_list() {
    verify_shipped_staged_positions_report(&SWIFT_COMPLEXITY_POSITIONS_PROBE);
}

/// The `complexity-swift` probe over a run whose every file the project's
/// `excluded:` list names.
const SWIFT_COMPLEXITY_EVERY_FILE_EXCLUDED_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: NO_STAGED_REPORTS,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function over the complexity gate under the project's excluded directory",
    declarations: SWIFT_COMPLEXITY_STAGED,
    staged: SWIFT_EXCLUDED_POSITION_ONLY,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the project excludes every file of the run, so the run reports nothing and \
             breaks nothing",
};

/// Acceptance: the shipped Swift complexity tool rule reports nothing, and
/// breaks nothing, when the project excludes every file of the run, through
/// the real swiftlint pipeline.
///
/// swiftlint exits 1 with `Error: No lintable files found at paths` when
/// `--force-exclude` leaves it no file to read, and that status reads as a
/// broken tool. The script tests each file it is given for readability first,
/// so the message can carry one cause only, and it then exits 0 with no
/// finding.
#[test]
fn the_shipped_swift_complexity_tool_rule_answers_zero_when_the_project_excludes_every_file() {
    verify_shipped_staged_positions_report(&SWIFT_COMPLEXITY_EVERY_FILE_EXCLUDED_PROBE);
}

/// The file of the one finding the `child_config:` probe must report.
///
/// The project excludes this directory, and the run drops that exclude list,
/// so the file reports.
const SWIFT_COMPLEXITY_CHILD_CONFIG_REPORTED: &[&str] = &[SWIFT_GENERATED_POSITION.path];

/// The `complexity-swift` probe beside a project that names a child
/// configuration of its own.
const SWIFT_COMPLEXITY_CHILD_CONFIG_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: SWIFT_COMPLEXITY_CHILD_CONFIG_REPORTED,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function over the complexity gate, beside a project child configuration",
    declarations: SWIFT_COMPLEXITY_STAGED,
    staged: SWIFT_EXCLUDED_POSITION_ONLY,
    support: SWIFT_CHILD_CONFIG_SUPPORT_FILES,
    reason: "swiftlint cannot read that project configuration beside the rule's own, so the run \
             measures with the rule's configuration alone and reports the staged function",
};

/// Acceptance: the shipped Swift complexity tool rule measures beside a
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
fn the_shipped_swift_complexity_tool_rule_measures_beside_a_project_child_config() {
    verify_shipped_staged_positions_report(&SWIFT_COMPLEXITY_CHILD_CONFIG_PROBE);
}

/// A project `.swiftlint.yml` that switches both rules off and raises the
/// complexity gate.
///
/// Each of the two settings silences the staged function on its own:
/// `disabled_rules` switches `cyclomatic_complexity` off, and a `warning` of
/// 30 stands over the staged function's score of 16.
const SWIFT_COMPLEXITY_OVERRIDING_CONFIG: &str = concat!(
    "disabled_rules:\n",
    "  - cyclomatic_complexity\n",
    "  - function_body_length\n",
    "cyclomatic_complexity:\n",
    "  warning: 30\n",
);

/// The overriding project configuration staged beside the ordinary position,
/// which the work-list does NOT name.
const SWIFT_COMPLEXITY_OVERRIDING_SUPPORT: &[(&str, &str)] = &[(
    SWIFT_PROJECT_CONFIG_PATH,
    SWIFT_COMPLEXITY_OVERRIDING_CONFIG,
)];

/// The `complexity-swift` probe over a project that states another gate.
const SWIFT_COMPLEXITY_OPTIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: SWIFT_COMPLEXITY_STAGED_REPORTED,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function a project gate would let through",
    declarations: SWIFT_COMPLEXITY_STAGED,
    staged: SWIFT_ORDINARY_POSITION_ONLY,
    support: SWIFT_COMPLEXITY_OVERRIDING_SUPPORT,
    reason: "the rule's own gate decides, so the staged function still reports",
};

/// Acceptance: the shipped Swift complexity tool rule keeps its own gates
/// against a project that states other ones, through the real swiftlint
/// pipeline.
///
/// The script names the project's `.swiftlint.yml` as the PARENT of its own
/// configuration, so the project decides which files are read. It must not
/// decide what the rule measures. The script's own configuration states the
/// gate of each rule, and a child block replaces the parent's block whole.
#[test]
fn the_shipped_swift_complexity_tool_rule_keeps_its_own_gates() {
    verify_shipped_staged_positions_report(&SWIFT_COMPLEXITY_OPTIONS_PROBE);
}

/// The `complexity-swift` probe beside a project that names a swiftlint
/// version that is not installed.
const SWIFT_COMPLEXITY_VERSION_MISMATCH_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: SWIFT_VERSION_MISMATCH_ERROR,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function over the gate beside a project version mismatch",
    path: SWIFT_ORDINARY_POSITION.path,
    source: Some(SWIFT_COMPLEXITY_STAGED),
    support: SWIFT_VERSION_MISMATCH_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift complexity tool rule BREAKS beside a project
/// that names a swiftlint version that is not installed, through the real
/// swiftlint pipeline.
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
fn the_shipped_swift_complexity_tool_rule_breaks_beside_a_project_version_mismatch() {
    verify_shipped_run_breaks(&SWIFT_COMPLEXITY_VERSION_MISMATCH_PROBE);
}

/// Where the Swift file that is never written stands inside the probe
/// repository.
const SWIFT_COMPLEXITY_ABSENT_PATH: &str = "Sources/Absent.swift";

/// What the one error of an absent file must name.
const SWIFT_COMPLEXITY_ABSENT_ERROR: &[&str] =
    &["complexity-swift cannot read", SWIFT_COMPLEXITY_ABSENT_PATH];

/// The `complexity-swift` probe over a path that holds no file.
const SWIFT_COMPLEXITY_ABSENT_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: SWIFT_COMPLEXITY_ABSENT_ERROR,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "a Swift file that is not there",
    path: SWIFT_COMPLEXITY_ABSENT_PATH,
    source: None,
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift complexity tool rule BREAKS on a file it
/// cannot read, through the real swiftlint pipeline.
///
/// swiftlint exits 1 for a path that is not there and writes nothing to
/// stdout. A pipeline takes the exit status of its LAST command, and that
/// command was `jq`, so the earlier pipe exited 0 and reported nothing — a run
/// answering zero for a reason other than a clean file.
#[test]
fn the_shipped_swift_complexity_tool_rule_breaks_on_a_file_it_cannot_read() {
    verify_shipped_run_breaks(&SWIFT_COMPLEXITY_ABSENT_PROBE);
}

/// The `complexity-swift` probe over a directory that holds no Swift file.
///
/// The probe writes no file at the path, and the one staged file under that
/// path makes the directory.
const SWIFT_COMPLEXITY_HOLLOW_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: NO_FINDINGS,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: SWIFT_HOLLOW_PURPOSE,
    path: SWIFT_HOLLOW_PATH,
    source: None,
    support: SWIFT_HOLLOW_FILES,
};

/// Acceptance: the shipped Swift complexity tool rule answers CLEAN over a
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
fn the_shipped_swift_complexity_tool_rule_stays_clean_over_a_hollow_directory() {
    verify_shipped_hollow_directory_answers_clean(&SWIFT_COMPLEXITY_HOLLOW_PROBE);
}

/// Every Swift file staged in the probe repository the complexity script is
/// given none of.
const SWIFT_COMPLEXITY_UNREAD_FILES: &[(&str, &str)] = &[
    ("Top.swift", SWIFT_COMPLEXITY_STAGED),
    ("deep/nested/Other.swift", SWIFT_COMPLEXITY_STAGED),
];

/// Each finding the Swift complexity script reports over the two files it is
/// given, as `path:line`.
const SWIFT_COMPLEXITY_READ_FINDINGS: &[&str] = &["Top.swift:1", "deep/nested/Other.swift:1"];

/// The `complexity-swift` probe over a run that is given no file.
const SWIFT_COMPLEXITY_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_COMPLEXITY_RULE,
        expected: NO_FINDINGS,
    },
    staged: SWIFT_COMPLEXITY_UNREAD_FILES,
    with_files: SWIFT_COMPLEXITY_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Swift complexity tool rule reads only the files it
/// is given, through the real swiftlint pipeline.
///
/// `swiftlint lint` with no path argument walks the whole tree under the
/// working directory, and it exits 0, so the answer reads as a measured result
/// rather than a mistake. The script answers an empty argument list at once,
/// with no finding and an exit status of 0. The same script over the two
/// staged files reports 2.
#[test]
fn the_shipped_swift_complexity_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&SWIFT_COMPLEXITY_EMPTY_RUN_PROBE);
}

/// A Python function of 17 branches, which stands over the `C901` gate of
/// 15. ruff counts one decision point for each `if`, and it counts neither a
/// nested block nor a chain of `and` operators, so the branches are written
/// out one for each line.
const PYTHON_COMPLEXITY_UNREAD_SOURCE: &str = r#"def branch(value):
    if value == 2: return 2
    if value == 3: return 3
    if value == 4: return 4
    if value == 5: return 5
    if value == 6: return 6
    if value == 7: return 7
    if value == 8: return 8
    if value == 9: return 9
    if value == 10: return 10
    if value == 11: return 11
    if value == 12: return 12
    if value == 13: return 13
    if value == 14: return 14
    if value == 15: return 15
    if value == 16: return 16
    if value == 17: return 17
    if value == 18: return 18
    return 0
"#;

/// Every Python file staged in the probe repository the complexity script is
/// given none of. ruff walks a whole tree, so a default target reaches the
/// nested file as readily as the one at the root.
const PYTHON_COMPLEXITY_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.py", PYTHON_COMPLEXITY_UNREAD_SOURCE),
    ("deep/nested/other.py", PYTHON_COMPLEXITY_UNREAD_SOURCE),
];

/// Each finding the Python complexity script reports over the two files it is
/// given, as `path:line`.
const PYTHON_COMPLEXITY_READ_FINDINGS: &[&str] = &["deep/nested/other.py:1", "top.py:1"];

/// The `complexity-python` probe over a run that is given no file.
const PYTHON_COMPLEXITY_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_COMPLEXITY_RULE,
        expected: NO_FINDINGS,
    },
    staged: PYTHON_COMPLEXITY_UNREAD_FILES,
    with_files: PYTHON_COMPLEXITY_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Python complexity tool rule reads only the files
/// it is given, through the real ruff pipeline.
///
/// `ruff check` with no path argument falls back to a default target of `.`,
/// and it walks that whole tree. A script that hands `"$@"` straight to ruff
/// therefore answers for every Python file under the repository root when the
/// run carries no file, and it exits 0, so the answer reads as a measured
/// result rather than a mistake. Measured over this probe with no argument:
/// without the guard the script reported 2 findings and exited 0; with the
/// guard it reports none and exits 0. The same script over the two staged
/// files reports 2.
#[test]
fn the_shipped_python_complexity_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&PYTHON_COMPLEXITY_EMPTY_RUN_PROBE);
}

/// A Python function of 190 statements, which stands over the `PLR0915` gate
/// of 180. `PLR0915` counts a statement rather than a line, and a semicolon
/// separates one statement from the next, so 19 lines carry the whole count.
const PYTHON_LENGTH_UNREAD_SOURCE: &str = r#"def long_function():
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0; value = 0
    return 0
"#;

/// Every Python file staged in the probe repository the function-length
/// script is given none of.
const PYTHON_LENGTH_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.py", PYTHON_LENGTH_UNREAD_SOURCE),
    ("deep/nested/other.py", PYTHON_LENGTH_UNREAD_SOURCE),
];

/// Each finding the Python function-length script reports over the two files
/// it is given, as `path:line`.
const PYTHON_LENGTH_READ_FINDINGS: &[&str] = &["deep/nested/other.py:1", "top.py:1"];

/// The `function-length-python` probe over a run that is given no file.
const PYTHON_LENGTH_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_FUNCTION_LENGTH_RULE,
        expected: NO_FINDINGS,
    },
    staged: PYTHON_LENGTH_UNREAD_FILES,
    with_files: PYTHON_LENGTH_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Python function-length tool rule reads only the
/// files it is given, through the real ruff pipeline.
///
/// The two Python rules of this roster share one tool and one default target,
/// so the length gate answers for the tree the same way the complexity gate
/// does. Measured over this probe with no argument: without the guard the
/// script reported 2 findings and exited 0; with the guard it reports none
/// and exits 0. The same script over the two staged files reports 2.
#[test]
fn the_shipped_python_function_length_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&PYTHON_LENGTH_EMPTY_RUN_PROBE);
}

/// A Go function whose innermost block stands 6 levels deep. gocognit adds
/// one for a block and one more for each level that block stands under, so
/// the cognitive complexity is 21 against the gate of 15.
const GO_COMPLEXITY_UNREAD_SOURCE: &str = r#"package probe

// Branch narrows a value, one nested block for each test.
func Branch(value int) int {
    if value > 0 {
        if value > 0 {
            if value > 0 {
                if value > 0 {
                    if value > 0 {
                        if value > 0 {
                            value--
                        }
                    }
                }
            }
        }
    }
    return value
}
"#;

/// Every Go file staged in the probe repository the complexity script is
/// given none of.
const GO_COMPLEXITY_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.go", GO_COMPLEXITY_UNREAD_SOURCE),
    ("deep/nested/other.go", GO_COMPLEXITY_UNREAD_SOURCE),
];

/// Each finding the Go complexity script reports over the two files it is
/// given, as `path:line`.
const GO_COMPLEXITY_READ_FINDINGS: &[&str] = &["deep/nested/other.go:4", "top.go:4"];

/// The `complexity-go` probe over a run that is given no file.
const GO_COMPLEXITY_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_COMPLEXITY_RULE,
        expected: NO_FINDINGS,
    },
    staged: GO_COMPLEXITY_UNREAD_FILES,
    with_files: GO_COMPLEXITY_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Go complexity tool rule reads only the files it is
/// given, through the real gocognit pipeline.
///
/// gocognit holds no default target of its own. Given no path it writes 39
/// lines of usage text to stderr and exits nonzero, and the pipe ends in
/// `jq`, so the script exits 0 with no finding. Measured over this probe with
/// no argument: the script reported 0 findings and exited 0 both without the
/// guard and with it, and the same script over the two staged files reports
/// 2. The guard makes that 0 the script's own answer rather than a tool
/// refusal a pipe hid, and it keeps the usage text off stderr.
#[test]
fn the_shipped_go_complexity_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&GO_COMPLEXITY_EMPTY_RUN_PROBE);
}

/// A TypeScript function whose innermost block stands 6 levels deep. The
/// published Sonar algorithm adds one for a block and one more for each level
/// that block stands under, so the cognitive complexity is 21 against the
/// gate of 15.
const TYPESCRIPT_COMPLEXITY_UNREAD_SOURCE: &str = r#"export function branch(value: number): number {
  if (value > 0) {
    if (value > 0) {
      if (value > 0) {
        if (value > 0) {
          if (value > 0) {
            if (value > 0) {
              value -= 1;
            }
          }
        }
      }
    }
  }
  return value;
}
"#;

/// Every TypeScript file staged in the probe repository the complexity script
/// is given none of.
const TYPESCRIPT_COMPLEXITY_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.ts", TYPESCRIPT_COMPLEXITY_UNREAD_SOURCE),
    ("deep/nested/other.ts", TYPESCRIPT_COMPLEXITY_UNREAD_SOURCE),
];

/// Each finding the TypeScript complexity script reports over the two files
/// it is given, as `path:line`.
const TYPESCRIPT_COMPLEXITY_READ_FINDINGS: &[&str] = &["deep/nested/other.ts:1", "top.ts:1"];

/// The `complexity-typescript` probe over a run that is given no file.
const TYPESCRIPT_COMPLEXITY_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_COMPLEXITY_RULE,
        expected: NO_FINDINGS,
    },
    staged: TYPESCRIPT_COMPLEXITY_UNREAD_FILES,
    with_files: TYPESCRIPT_COMPLEXITY_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped TypeScript complexity tool rule reads only the
/// files it is given, through the real eslint pipeline.
///
/// eslint with no path argument reads the working directory, and the config
/// this rule writes names `**/*.{js,jsx,mjs,cjs,ts,tsx}`, so the run reaches
/// every TypeScript file under the repository root. Measured over this probe
/// with no argument: without the guard the script reported 2 findings and
/// exited 0; with the guard it reports none and exits 0. The same script over
/// the two staged files reports 2.
#[test]
fn the_shipped_typescript_complexity_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&TYPESCRIPT_COMPLEXITY_EMPTY_RUN_PROBE);
}
