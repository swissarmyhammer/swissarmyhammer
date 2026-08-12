//! Acceptance tests for the shipped complexity tool rules.
//!
//! One test holds the whole roster to its fixture pair and to the prompt rules
//! each tool decides. The tests under it drive Rust, TypeScript, Swift, Python
//! and Go through their real tools.

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

/// Where the probe cargo manifest stands inside a Rust probe repository.
const RUST_PROBE_MANIFEST_PATH: &str = "Cargo.toml";

/// The probe cargo manifest, staged beside a Rust probe file that the
/// work-list does NOT name.
///
/// A `workspace`-scope rule loads a project rather than a file list, so cargo
/// breaks on the staged file only when it finds a package to lint.
const RUST_PROBE_SUPPORT_FILES: &[(&str, &str)] =
    &[(RUST_PROBE_MANIFEST_PATH, COMPLEX_PACKAGE_MANIFEST)];

/// One line of the body of a function built to run over the length gate.
const LONG_FUNCTION_BODY_LINE: &str = "    let _ = 1;\n";

/// How many body lines carry a function over `too-many-lines-threshold = 250`.
///
/// Clippy counts the body lines and the two brace lines alike, so 300 body
/// lines answer 302 against the gate of 250.
const LONG_FUNCTION_BODY_LINES: usize = 300;

/// A Rust function named `name` whose body runs [`LONG_FUNCTION_BODY_LINES`]
/// lines, with `head` written above its `pub fn` line.
///
/// Every shape the length gate measures runs past 250 lines, and
/// [`ShippedStagedRows`] carries only `&'static` bytes, so a probe of that gate
/// builds its source here rather than writing 300 lines out for each function.
fn long_rust_function(head: &str, name: &str) -> String {
    format!(
        "{head}pub fn {name}() {{\n{}}}\n",
        LONG_FUNCTION_BODY_LINE.repeat(LONG_FUNCTION_BODY_LINES)
    )
}

/// Drives the shipped `complexity-rust` script over a probe cargo package that
/// holds `files`, and answers each finding it reported as `path:line`, sorted.
///
/// The manifest is staged beside `files` because cargo lints a package and
/// never a loose file. The findings are the SCRIPT's own, before the engine
/// keeps only the ones in the changed files.
fn rust_complexity_findings(files: &[(&str, &str)]) -> Vec<String> {
    let loader = builtin_loader();
    require_tool_installed(&loader, RUST_PROJECT_TYPES, RUST_COMPLEXITY_RULE);
    let mut staged: Vec<(&str, &str)> = vec![(RUST_PROBE_MANIFEST_PATH, COMPLEX_PACKAGE_MANIFEST)];
    staged.extend_from_slice(files);
    let paths: Vec<&str> = staged.iter().map(|(path, _)| *path).collect();

    let reported = shipped_script_findings(&loader, RUST_COMPLEXITY_RULE, &staged, &paths)
        .expect("the shipped Rust complexity script must judge the probe package and exit 0");

    sorted_names(&reported)
}

/// The annotation the rule states for a function the length gate reports.
const LENGTH_GATE_ANNOTATION: &str =
    "#[expect(clippy::too_many_lines, reason = \"one line for each field\")]\n";

/// The one row the annotation probe must report.
///
/// The bare function opens the probe library, so its `pub fn` line is row 1.
/// The annotated function holds the same 300 body lines under it, and a run
/// that reported it as well would name row 304.
const RUST_LENGTH_ANNOTATION_ROWS: &[&str] = &["src/lib.rs:1"];

/// Acceptance: the shipped Rust complexity tool rule drops a long function
/// that carries the length-gate annotation, and keeps the bare one beside it,
/// through the real clippy pipeline.
///
/// `function-length`, one of the two prompt rules this rule supersedes, exempts
/// "Functions that are mostly configuration/data (e.g., builder patterns with
/// many options)" and "Initialization functions that set many fields". Clippy
/// counts a data line like a code line, and its configuration holds no key that
/// tells the two apart, so the run cannot reproduce that carve-out. The
/// annotation is the whole answer, and this test holds it.
///
/// Both functions hold the same 300 body lines, so the annotation is the one
/// difference between the function that reports and the function that stays
/// silent.
#[test]
fn the_shipped_rust_complexity_tool_rule_answers_the_length_gate_annotation() {
    let source = format!(
        "{}{}",
        long_rust_function("", "bare_defaults"),
        long_rust_function(LENGTH_GATE_ANNOTATION, "annotated_defaults")
    );

    let reported = rust_complexity_findings(&[(COMPLEX_LIB_PATH, &source)]);

    assert_eq!(
        reported,
        sorted_names(&expected_script_findings(RUST_LENGTH_ANNOTATION_ROWS)),
        "the annotation is the author's answer to the data carve-out, so the annotated \
         function must stay silent and the bare one must report"
    );
}

/// A probe crate root with nothing for the four lints to read, for a probe that
/// measures another file of the same package.
const EMPTY_PROBE_LIB_RS: &str = "//! A probe crate root with nothing to lint.\n";

/// Where the probe integration test stands inside the probe repository.
const RUST_PROBE_INTEGRATION_TEST_PATH: &str = "tests/it.rs";

/// The attribute that marks the probe's test function at its DEFINITION, which
/// is the mark `cognitive-complexity` states for its test carve-out.
const RUST_TEST_ATTRIBUTE: &str = "#[test]\n";

/// Both rows the test-carve-out probe must report.
///
/// The attribute stands on row 1, so the `pub fn` line of the test function is
/// row 2. The helper holds the same 300 body lines under it, so its `pub fn`
/// line is row 304.
const RUST_TEST_CARVE_OUT_ROWS: &[&str] = &["tests/it.rs:2", "tests/it.rs:304"];

/// Acceptance: the shipped Rust complexity tool rule REPORTS a long test
/// function, and the helper beside it, through the real clippy pipeline.
///
/// Both prompt rules this rule supersedes exempt a test, and
/// `cognitive-complexity` names the DEFINITION as the mark: "A complex helper
/// named `build_request` in a file called `foo_test.rs` is still a complex
/// function and is still listed."
///
/// Clippy holds no flag and no configuration key that reads `#[test]`, so the
/// run reproduces none of that carve-out and the author answers it with the
/// annotation. `--all-targets` is what puts the test target in front of the
/// gates. Dropping the flag would read the TARGET, which is the mark the prompt
/// rule forbids: it drops the helper beside the test, and it drops every
/// `#[cfg(test)]` module as well. This test holds both rows, so a run that
/// silenced either half answers another list.
#[test]
fn the_shipped_rust_complexity_tool_rule_reports_a_test_function_and_its_helper() {
    let source = format!(
        "{}{}",
        long_rust_function(RUST_TEST_ATTRIBUTE, "test_table"),
        long_rust_function("", "build_request")
    );

    let reported = rust_complexity_findings(&[
        (COMPLEX_LIB_PATH, EMPTY_PROBE_LIB_RS),
        (RUST_PROBE_INTEGRATION_TEST_PATH, &source),
    ]);

    assert_eq!(
        reported,
        sorted_names(&expected_script_findings(RUST_TEST_CARVE_OUT_ROWS)),
        "`--all-targets` puts the test target in front of the gates, and no clippy key \
         reads `#[test]`, so the test function and the helper beside it both report"
    );
}

/// The head a generated Rust file carries in the probe: one generator writes
/// the first line, another writes the second, and clippy reads neither.
const RUST_GENERATED_HEAD: &str = concat!(
    "// This file is @generated by prost-build.\n",
    "// Code generated by tool. DO NOT EDIT.\n",
);

/// The crate root of the generated-code probe.
///
/// It names two module files that hold the same bytes, so the annotation on the
/// second declaration is the one difference between them. The declaration
/// stands in this file, which the generator never writes again.
const RUST_GENERATED_ROOT_RS: &str = concat!(
    "//! A probe crate root that names two generated modules.\n",
    "pub mod bare;\n",
    "#[expect(clippy::too_many_lines, reason = \"the generator writes this file\")]\n",
    "pub mod annotated;\n",
);

/// Where the generated module file with no annotation stands inside the probe
/// repository.
const RUST_GENERATED_BARE_PATH: &str = "src/bare.rs";

/// Where the generated module file whose declaration carries the annotation
/// stands inside the probe repository.
const RUST_GENERATED_ANNOTATED_PATH: &str = "src/annotated.rs";

/// The one row the generated-code probe must report.
///
/// The head runs two lines, so the `pub fn` line of each module file is row 3.
/// The annotated module holds the same bytes, and a run that reported it as
/// well would name `src/annotated.rs:3`.
const RUST_GENERATED_ROWS: &[&str] = &["src/bare.rs:3"];

/// Acceptance: the shipped Rust complexity tool rule REPORTS a checked-in
/// generated file, and drops the one whose module declaration carries the
/// annotation, through the real clippy pipeline.
///
/// Both prompt rules this rule supersedes exempt generated code. Rust states no
/// generated-file header convention, and clippy reads no header: the two module
/// files here each carry the header two generators write, and the bare one
/// still reports. A header test in the script would name the first lines of one
/// generator and never a convention, which is why the sibling `complexity-go`
/// makes such a test and this rule does not.
///
/// The author answers this carve-out at the `mod` declaration, which stands in
/// the PARENT file and which the generator never writes again. The two module
/// files hold the same bytes, so the annotation is the one difference between
/// the file that reports and the file that stays silent.
#[test]
fn the_shipped_rust_complexity_tool_rule_reports_a_generated_file() {
    let generated = long_rust_function(RUST_GENERATED_HEAD, "fold_grid");

    let reported = rust_complexity_findings(&[
        (COMPLEX_LIB_PATH, RUST_GENERATED_ROOT_RS),
        (RUST_GENERATED_BARE_PATH, &generated),
        (RUST_GENERATED_ANNOTATED_PATH, &generated),
    ]);

    assert_eq!(
        reported,
        sorted_names(&expected_script_findings(RUST_GENERATED_ROWS)),
        "clippy reads no generated-file header, so the bare module reports; the annotation \
         on the other module's declaration is what silences it"
    );
}

/// A Rust library the compiler refuses: the body of `broken` answers a string
/// where its signature states an integer.
const RUST_COMPLEXITY_UNCOMPILABLE_SOURCE: &str = concat!(
    "//! A probe crate the compiler refuses.\n",
    "pub fn broken() -> i32 { \"not an integer\" }\n",
);

/// What the one error of a workspace cargo cannot lint must name: the script's
/// own line, and cargo's own words beside it.
const RUST_COMPLEXITY_UNCOMPILABLE_ERROR: &[&str] = &[
    "complexity-rust: cargo clippy could not lint the workspace",
    "could not compile",
];

/// The `complexity-rust` probe over a workspace that does not compile.
const RUST_COMPLEXITY_UNCOMPILABLE_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_COMPLEXITY_RULE,
        expected: RUST_COMPLEXITY_UNCOMPILABLE_ERROR,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "a Rust workspace the compiler refuses",
    path: COMPLEX_LIB_PATH,
    source: Some(RUST_COMPLEXITY_UNCOMPILABLE_SOURCE),
    support: RUST_PROBE_SUPPORT_FILES,
};

/// Acceptance: the shipped Rust complexity tool rule BREAKS on a workspace it
/// cannot compile, through the real clippy pipeline.
///
/// `cargo clippy` lints nothing when the workspace does not compile: it writes
/// its own errors to stderr, writes no lint message, and exits nonzero. An
/// earlier shape of this script was one pipe that ended in `sort -u`, and a
/// shell pipeline takes the status of its last command, so that shape answered
/// exit 0 with no finding and the engine read the whole tree as clean. The
/// script now writes the report to a file, tests the status, and exits 1 with a
/// line that names the rule.
#[test]
fn the_shipped_rust_complexity_tool_rule_breaks_on_a_workspace_it_cannot_compile() {
    verify_shipped_run_breaks(&RUST_COMPLEXITY_UNCOMPILABLE_PROBE);
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

/// A Python function whose innermost block stands 6 levels deep. complexipy
/// adds one for a block and one more for each level that block stands under,
/// so the cognitive complexity is 21 against the gate of 15.
///
/// ruff's `C901`, which this rule ran before, scores the same function 7 and
/// reports nothing: McCabe counts one decision point for each `if` and reads
/// no nesting at all. That measured difference is why the rule left ruff.
const PYTHON_COMPLEXITY_UNREAD_SOURCE: &str = r#"def branch(value):
    if value > 0:
        if value > 0:
            if value > 0:
                if value > 0:
                    if value > 0:
                        if value > 0:
                            value -= 1
    return value
"#;

/// Every Python file staged in the probe repository the complexity script is
/// given none of. A tool that walks a whole tree reaches the nested file as
/// readily as the one at the root.
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
/// it is given, through the real complexipy pipeline.
///
/// complexipy holds no default target. Measured with complexipy 7.0.0, given
/// no path: `You need to define paths in the CLI call arguments or in
/// complexipy.toml file`, and exit 1. A script that gave the tool that empty
/// argument list would answer a refusal, and the engine would read the refusal
/// as a tree with no finding in it.
///
/// The shipped script reaches no such run: it hands complexipy one file at a
/// time, so the tool takes no empty argument list. Measured over this probe
/// with the argument count removed: the run with no argument reported nothing
/// and exited 0, the same as the shipped script. The count stands because the
/// `run` key of `builtin/validators/README.md` states it, and because it
/// answers before `mktemp -d` runs.
#[test]
fn the_shipped_python_complexity_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&PYTHON_COMPLEXITY_EMPTY_RUN_PROBE);
}

/// The head of the plain Python position: none.
const PYTHON_PLAIN_HEAD: &[&str] = &[];

/// The head of the generated Python position: the header the protocol buffer
/// compiler writes, and the blank line under it.
///
/// Python states no generated-file header convention of its own. This is one
/// generator's first line, which is why the rule reproduces no carve-out from
/// it.
const PYTHON_GENERATED_HEAD: &[&str] =
    &["# Generated by the protocol buffer compiler.  DO NOT EDIT!\n\n"];

/// The plain position, which carries no header.
const PYTHON_PLAIN_POSITION: ShippedStagedFile = ShippedStagedFile {
    path: "plain/staged.py",
    head: PYTHON_PLAIN_HEAD,
};

/// The generated position, which carries the header and nothing else of its
/// own.
///
/// Neither position takes a path a generator is known by, so the path can
/// decide nothing and the header is the one difference between the two.
const PYTHON_GENERATED_POSITION: ShippedStagedFile = ShippedStagedFile {
    path: "marked/staged.py",
    head: PYTHON_GENERATED_HEAD,
};

/// Both Python positions, in the order the work-list holds them.
const PYTHON_GENERATED_POSITIONS: &[ShippedStagedFile] =
    &[PYTHON_PLAIN_POSITION, PYTHON_GENERATED_POSITION];

/// The positions the run must report, which is the whole roster.
const PYTHON_GENERATED_REPORTED: &[&str] = &["plain/staged.py", "marked/staged.py"];

/// The staged Python positions, and the two of them the real complexipy
/// pipeline must report.
const PYTHON_COMPLEXITY_GENERATED_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_COMPLEXITY_RULE,
        expected: PYTHON_GENERATED_REPORTED,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function over the complexity gate, staged in two positions",
    declarations: PYTHON_COMPLEXITY_UNREAD_SOURCE,
    staged: PYTHON_GENERATED_POSITIONS,
    support: NO_SUPPORT_FILES,
    reason: "a generated Python file reports like any other file, because complexipy reads \
             no header and Python states no header convention to read",
};

/// Acceptance: the shipped Python complexity tool rule REPORTS a generated
/// file, through the real complexipy pipeline.
///
/// `cognitive-complexity`, the prompt rule this rule supersedes, exempts
/// generated code. This rule reproduces none of that carve-out, and this test
/// holds the gap measured rather than left to be discovered.
///
/// complexipy reads no file header: measured over three files, the one whose
/// head carries the protocol-buffer header, the one whose head carries
/// `# @generated`, and the plain file each reported their function. Its one
/// file filter, `--exclude <glob>`, reads the PATH, and it reaches no file
/// named on the command line at all: measured over the same three files named
/// as arguments, `--exclude 'DO NOT EDIT'`, `--exclude '*_pb2.py'` and
/// `--exclude 'marked_pb2.py'` each dropped none of them. The sibling
/// `complexity-go` makes the header test itself, because Go states one
/// convention; a Python header test would name one generator instead.
#[test]
fn the_shipped_python_complexity_tool_rule_reports_a_generated_file() {
    verify_shipped_staged_positions_report(&PYTHON_COMPLEXITY_GENERATED_PROBE);
}

/// One Python test method beside one module-level helper, each with an
/// innermost block 6 levels deep, so each scores 21 against the gate of 15.
///
/// The name of the method is the pytest and unittest convention at the
/// DEFINITION, which is the mark `cognitive-complexity` states for its test
/// carve-out. The helper carries no such name, and the prompt rule keeps it
/// listed: "A complex helper named `build_request` in a file called
/// `foo_test.rs` is still a complex function and is still listed."
const PYTHON_COMPLEXITY_TEST_SOURCE: &str = r#"class TestThing:
    def test_method(self, value):
        if value > 0:
            if value > 0:
                if value > 0:
                    if value > 0:
                        if value > 0:
                            if value > 0:
                                value -= 1
        return value


def build_request(value):
    if value > 0:
        if value > 0:
            if value > 0:
                if value > 0:
                    if value > 0:
                        if value > 0:
                            value -= 1
    return value
"#;

/// The one test file staged for the carve-out probe.
const PYTHON_COMPLEXITY_TEST_FILES: &[(&str, &str)] =
    &[("suite/staged_test.py", PYTHON_COMPLEXITY_TEST_SOURCE)];

/// The one row the run must report: the module-level helper, at the line its
/// `def` stands on.
///
/// The test method stands at row 2 of the same file, so a run that reported
/// the method as well, or reported it instead, answers another list.
const PYTHON_COMPLEXITY_TEST_ROWS: &[&str] = &["suite/staged_test.py:13"];

/// The staged Python test file, and the one row the real complexipy pipeline
/// must report over it.
const PYTHON_COMPLEXITY_TEST_PROBE: ShippedStagedRows = ShippedStagedRows {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_COMPLEXITY_RULE,
        expected: PYTHON_COMPLEXITY_TEST_ROWS,
    },
    staged: PYTHON_COMPLEXITY_TEST_FILES,
    reason: "the filter reads the NAME each finding carries, so the test method goes and the \
             helper beside it stays",
};

/// Acceptance: the shipped Python complexity tool rule drops a test function
/// and keeps the helper beside it, through the real complexipy pipeline.
///
/// `cognitive-complexity` exempts a test, and it names the DEFINITION as the
/// mark rather than the file name. Python states that convention twice: pytest
/// collects a function or method whose name starts with `test`
/// (`python_functions = ["test"]`, read from pytest 9.1.1), and unittest
/// collects a method whose name starts with `test`
/// (`unittest.TestLoader.testMethodPrefix`).
///
/// complexipy holds no flag that reads a function name, and its `--exclude`
/// glob reads the path the prompt rule refuses. So the filter reads the name
/// the SARIF report carries — the bare name of a function, and `Class::method`
/// for a method. Measured over this probe: the script without the filter
/// reported both rows, and the shipped script reports the helper alone.
#[test]
fn the_shipped_python_complexity_tool_rule_drops_a_test_function_and_keeps_its_helper() {
    verify_shipped_staged_rows_report(&PYTHON_COMPLEXITY_TEST_PROBE);
}

/// What the one error of a Python file complexipy could not read must name.
///
/// The script writes this text for a path that holds no file and for a file
/// the parser refuses alike, because the engine reads one broken run either
/// way and the agent needs the path.
const PYTHON_COMPLEXITY_UNREADABLE_ERROR_PREFIX: &str =
    "complexity-python: complexipy could not read";

/// Where the Python file that is never written stands inside the probe
/// repository.
const PYTHON_COMPLEXITY_ABSENT_PATH: &str = "absent.py";

/// What the one error of an absent Python file must name.
const PYTHON_COMPLEXITY_ABSENT_ERROR: &[&str] = &[
    PYTHON_COMPLEXITY_UNREADABLE_ERROR_PREFIX,
    PYTHON_COMPLEXITY_ABSENT_PATH,
];

/// The `complexity-python` probe over a path that holds no file.
const PYTHON_COMPLEXITY_ABSENT_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_COMPLEXITY_RULE,
        expected: PYTHON_COMPLEXITY_ABSENT_ERROR,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "a Python file that is not there",
    path: PYTHON_COMPLEXITY_ABSENT_PATH,
    source: None,
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Python complexity tool rule BREAKS on a file it
/// cannot read, through the real complexipy pipeline.
///
/// Measured with complexipy 7.0.0 over a path that holds no file: the tool
/// wrote a SARIF run holding no result and exited 1 — the same status it
/// writes for a finding. A script that read status 1 as a measured run would
/// report no finding and exit 0, and the engine would read a file the tool
/// never judged as a clean file. The `[ ! -r "$file" ]` test names the path
/// and exits 1 before complexipy runs.
#[test]
fn the_shipped_python_complexity_tool_rule_breaks_on_a_file_it_cannot_read() {
    verify_shipped_run_breaks(&PYTHON_COMPLEXITY_ABSENT_PROBE);
}

/// A Python file that does not parse: the body of `broken` holds no statement.
const PYTHON_COMPLEXITY_UNPARSABLE_SOURCE: &str =
    concat!("def broken(value):\n", "    if value > 0:\n",);

/// Where the unparsable file stands inside the probe repository.
const PYTHON_COMPLEXITY_UNPARSABLE_PATH: &str = "unparsable.py";

/// What the one error of an unparsable Python file must name.
const PYTHON_COMPLEXITY_UNPARSABLE_ERROR: &[&str] = &[
    PYTHON_COMPLEXITY_UNREADABLE_ERROR_PREFIX,
    PYTHON_COMPLEXITY_UNPARSABLE_PATH,
];

/// The `complexity-python` probe over a Python file complexipy cannot parse.
const PYTHON_COMPLEXITY_UNPARSABLE_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_COMPLEXITY_RULE,
        expected: PYTHON_COMPLEXITY_UNPARSABLE_ERROR,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "a Python file the parser cannot read",
    path: PYTHON_COMPLEXITY_UNPARSABLE_PATH,
    source: Some(PYTHON_COMPLEXITY_UNPARSABLE_SOURCE),
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Python complexity tool rule BREAKS on a Python file
/// it cannot parse, through the real complexipy pipeline.
///
/// The file is readable, so the readability test admits it and complexipy
/// reads it. Measured with complexipy 7.0.0: the tool wrote `error: Failed to
/// process <path> - Please check file/folder exists or check syntax` to
/// STDOUT, wrote 0 bytes to stderr, and exited 1 — the same status it writes
/// for a finding, so the status alone cannot tell the two apart. The script
/// accepts status 1 only beside a report holding one result or more, and this
/// run held none, so the script forwards the tool's own console text, names
/// the file and exits 1.
#[test]
fn the_shipped_python_complexity_tool_rule_breaks_on_a_file_it_cannot_parse() {
    verify_shipped_run_breaks(&PYTHON_COMPLEXITY_UNPARSABLE_PROBE);
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
/// gocognit holds no default target of its own. Measured with gocognit
/// v1.2.1, given no path: 52 lines of usage text on stderr, nothing on stdout,
/// and exit 2. A script that gave the tool that empty argument list would
/// answer a refusal, and the engine would read the refusal as a tree with no
/// finding in it.
///
/// The shipped script reaches no such run. It hands gocognit one file at a
/// time, inside the loop that drops a generated file, so the tool takes no
/// empty argument list. The head of the script counts its arguments, which is
/// the guard the `run` key of `builtin/validators/README.md` states for each
/// `files`-scope rule.
///
/// Measured over this probe with no argument: the script reported 0 findings,
/// wrote nothing to stderr, and exited 0. The same script over the two staged
/// files reports 2.
#[test]
fn the_shipped_go_complexity_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&GO_COMPLEXITY_EMPTY_RUN_PROBE);
}

/// The head of the plain Go position: none.
const GO_PLAIN_HEAD: &[&str] = &[];

/// The head of the generated Go position: the header `go generate` states, and
/// the blank line under it.
///
/// The convention is one line that matches `^// Code generated .* DO NOT EDIT\.$`
/// above the first text that is neither a comment nor blank.
const GO_GENERATED_HEAD: &[&str] = &["// Code generated by protoc-gen-go. DO NOT EDIT.\n\n"];

/// The plain position, which carries no header.
const GO_PLAIN_POSITION: ShippedStagedFile = ShippedStagedFile {
    path: "plain/staged.go",
    head: GO_PLAIN_HEAD,
};

/// The generated position, which carries the header and nothing else of its
/// own.
///
/// Neither position takes a path a generator is known by, so the path can
/// decide nothing and the header is the one difference between the two.
const GO_GENERATED_POSITION: ShippedStagedFile = ShippedStagedFile {
    path: "marked/staged.go",
    head: GO_GENERATED_HEAD,
};

/// Both Go positions, in the order the work-list holds them.
const GO_GENERATED_POSITIONS: &[ShippedStagedFile] = &[GO_PLAIN_POSITION, GO_GENERATED_POSITION];

/// The one position the run must report.
const GO_GENERATED_REPORTED: &[&str] = &["plain/staged.go"];

/// The staged Go positions, and the one of them the real gocognit pipeline
/// must report.
const GO_COMPLEXITY_GENERATED_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_COMPLEXITY_RULE,
        expected: GO_GENERATED_REPORTED,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one function over the complexity gate, staged in two positions",
    declarations: GO_COMPLEXITY_UNREAD_SOURCE,
    staged: GO_GENERATED_POSITIONS,
    support: NO_SUPPORT_FILES,
    reason: "the plain file reports its function, and the file whose head carries the \
             generated header reports nothing",
};

/// Acceptance: the shipped Go complexity tool rule skips a generated file,
/// through the real gocognit pipeline.
///
/// `cognitive-complexity`, the prompt rule this rule supersedes, exempts
/// generated code. gocognit reads no such header, and its one file filter,
/// `-ignore <regexp>`, reads the path and never the content: measured over
/// three files, `-ignore 'DO NOT EDIT'` dropped none of them. So the script
/// reads the head of each file it is given and drops the file that carries the
/// header.
///
/// Measured over this probe: the script without the head test reported both
/// positions, and the shipped script reports the plain position alone. The two
/// positions hold the same declarations, so the header is the one difference
/// between them.
#[test]
fn the_shipped_go_complexity_tool_rule_skips_a_generated_file() {
    verify_shipped_staged_positions_report(&GO_COMPLEXITY_GENERATED_PROBE);
}

/// One Go test function whose innermost block stands 6 levels deep, so its
/// cognitive complexity is 21 against the gate of 15.
///
/// The name and the signature are the Go test-framework convention at the
/// DEFINITION, which is the mark `cognitive-complexity` states for its test
/// carve-out.
const GO_COMPLEXITY_TEST_SOURCE: &str = r#"package probe

import "testing"

// TestBranch narrows a value, one nested block for each test.
func TestBranch(t *testing.T) {
    value := 6
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
    t.Log(value)
}
"#;

/// The test-file position, which the rule measures like every other file.
const GO_TEST_POSITION: ShippedStagedFile = ShippedStagedFile {
    path: "suite/staged_test.go",
    head: GO_PLAIN_HEAD,
};

/// The test-file position alone, which is then every file of the run.
const GO_TEST_POSITIONS: &[ShippedStagedFile] = &[GO_TEST_POSITION];

/// The position the run must report, which is the whole roster.
const GO_TEST_REPORTED: &[&str] = &["suite/staged_test.go"];

/// The staged Go test file, which the real gocognit pipeline must report.
const GO_COMPLEXITY_TEST_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_COMPLEXITY_RULE,
        expected: GO_TEST_REPORTED,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "one test function over the complexity gate",
    declarations: GO_COMPLEXITY_TEST_SOURCE,
    staged: GO_TEST_POSITIONS,
    support: NO_SUPPORT_FILES,
    reason: "a test function stands over the gate like any other function, because \
             gocognit reads no function name and the rule states no path filter",
};

/// Acceptance: the shipped Go complexity tool rule reports a complex test
/// function, through the real gocognit pipeline.
///
/// `cognitive-complexity` exempts a test, and it names the DEFINITION as the
/// mark rather than the file name, so a complex helper in a test file stays
/// listed. gocognit holds no flag that reads a function name, and its `-test`
/// flag filters a directory walk alone: measured, `-test=false` over a NAMED
/// `_test.go` path reported the test function again.
///
/// `-ignore '_test\.go$'` would silence the file, and it would silence every
/// helper in it as well. This test holds the rule to stating no such
/// expression: a run that reported nothing here would have traded a true
/// finding for the carve-out.
#[test]
fn the_shipped_go_complexity_tool_rule_reports_a_test_file() {
    verify_shipped_staged_positions_report(&GO_COMPLEXITY_TEST_PROBE);
}

/// What the one error of a Go file gocognit could not read must name.
///
/// The script writes this text for a path that holds no file and for a file
/// the parser refuses alike, because the engine reads one broken run either
/// way and the agent needs the path.
const GO_COMPLEXITY_UNREADABLE_ERROR_PREFIX: &str = "complexity-go: gocognit could not read";

/// Where the Go file that is never written stands inside the probe repository.
const GO_COMPLEXITY_ABSENT_PATH: &str = "absent.go";

/// What the one error of an absent Go file must name.
const GO_COMPLEXITY_ABSENT_ERROR: &[&str] = &[
    GO_COMPLEXITY_UNREADABLE_ERROR_PREFIX,
    GO_COMPLEXITY_ABSENT_PATH,
];

/// The `complexity-go` probe over a path that holds no file.
const GO_COMPLEXITY_ABSENT_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_COMPLEXITY_RULE,
        expected: GO_COMPLEXITY_ABSENT_ERROR,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "a Go file that is not there",
    path: GO_COMPLEXITY_ABSENT_PATH,
    source: None,
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Go complexity tool rule BREAKS on a file it cannot
/// read, through the real gocognit pipeline.
///
/// Measured with gocognit v1.2.1 over a path that holds no file: the tool
/// wrote nothing to stdout, wrote `gocognit: open ...: no such file or
/// directory` to stderr, and exited 1 — the same status it writes for a
/// finding. An earlier shape of the script ended in a pipe, so it exited 0 and
/// reported nothing, and the engine read a file the tool never judged as a
/// clean file. The `[ ! -r "$file" ]` test now names the path and exits 1
/// before gocognit runs.
#[test]
fn the_shipped_go_complexity_tool_rule_breaks_on_a_file_it_cannot_read() {
    verify_shipped_run_breaks(&GO_COMPLEXITY_ABSENT_PROBE);
}

/// A Go file that does not parse: the body of `Broken` never closes.
const GO_COMPLEXITY_UNPARSABLE_SOURCE: &str = concat!(
    "package staged\n",
    "\n",
    "func Broken(value int) int {\n",
    "\tif value > 0 {\n",
    "\t\treturn 1\n",
);

/// Where the unparsable file stands inside the probe repository.
const GO_COMPLEXITY_UNPARSABLE_PATH: &str = "unparsable.go";

/// What the one error of an unparsable Go file must name.
const GO_COMPLEXITY_UNPARSABLE_ERROR: &[&str] = &[
    GO_COMPLEXITY_UNREADABLE_ERROR_PREFIX,
    GO_COMPLEXITY_UNPARSABLE_PATH,
];

/// The `complexity-go` probe over a Go file gocognit cannot parse.
const GO_COMPLEXITY_UNPARSABLE_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_COMPLEXITY_RULE,
        expected: GO_COMPLEXITY_UNPARSABLE_ERROR,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "a Go file the parser cannot read",
    path: GO_COMPLEXITY_UNPARSABLE_PATH,
    source: Some(GO_COMPLEXITY_UNPARSABLE_SOURCE),
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Go complexity tool rule BREAKS on a Go file it
/// cannot parse, through the real gocognit pipeline.
///
/// The file is readable, so the readability test admits it and gocognit reads
/// it. Measured with gocognit v1.2.1: the tool wrote nothing to stdout, wrote
/// `gocognit: ...: expected '}', found 'EOF'` to stderr, and exited 1 — the
/// same status it writes for a finding, so the status alone cannot tell the
/// two apart. The script accepts status 1 only beside a report that is a JSON
/// array of one entry or more, and this run wrote 0 bytes, so the script names
/// the file and exits 1.
#[test]
fn the_shipped_go_complexity_tool_rule_breaks_on_a_file_it_cannot_parse() {
    verify_shipped_run_breaks(&GO_COMPLEXITY_UNPARSABLE_PROBE);
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
