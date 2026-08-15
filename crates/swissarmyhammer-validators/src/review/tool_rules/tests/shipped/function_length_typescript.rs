//! Acceptance tests for the shipped `function-length-typescript` tool rule.
//!
//! Each test drives the SHIPPED script, or the SHIPPED eslint config, over
//! a probe repository and reads what the real eslint reported.
//!
//! One module stands for each language of the family, because one file for
//! the whole family runs past the byte cap a review prompt holds.

use super::function_length::function_length_work;
use super::*;

use std::sync::LazyLock;

/// The materialized name of the `function-length-typescript` fail fixture.
const TYPESCRIPT_LENGTH_FAIL_FIXTURE: &str = "function-length-typescript.fail.ts";

/// Where the fail fixture stands inside the probe repository, as the
/// work-list holds it.
const TYPESCRIPT_LENGTH_FIXTURE_PATH: &str = "src/function-length-typescript-fail.ts";

/// The start of the source line each guard in the `function-length-typescript`
/// fail fixture is reported at.
///
/// The gate reports at the head of the function it measures, and for a method
/// that head is the member's NAME. Each entry is therefore the text the gate
/// points at, and not the declared name alone.
const TYPESCRIPT_LENGTH_FAIL_GUARDS: &[&str] = &[
    "export function foldReadings(",
    "public fold(",
    "export const foldHeld = (",
];

/// The `function-length-typescript` fail fixture, and every guard the real
/// eslint pipeline must measure inside it.
const TYPESCRIPT_LENGTH_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_FUNCTION_LENGTH_RULE,
        expected: TYPESCRIPT_LENGTH_FAIL_GUARDS,
    },
    fixture: TYPESCRIPT_LENGTH_FAIL_FIXTURE,
    path: TYPESCRIPT_LENGTH_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "guard",
};

/// Acceptance: the shipped TypeScript function-length tool rule measures every
/// guard its fail fixture holds, through the real eslint pipeline.
///
/// A guard is held to the SOURCE LINE its finding stands on. The tool names the
/// function it measured for two of the three and names none for the third —
/// measured, it writes `Arrow function has too many lines (304).` — so the
/// position is the only text that tells one finding from another.
///
/// The three guards are the three shapes the gate must reach, and the fixture
/// carries each one at 302 body lines against the gate of 250: an exported
/// function declaration, a class method, and an arrow function bound to a
/// `const`. The method is the shape a lookup too narrow loses in silence,
/// because it reports at its NAME, which stands before the `FunctionExpression`
/// the gate measures; a lookup over the function ranges alone therefore misses
/// it and climbs to whatever function stands around it. The pass fixture holds
/// the same three shapes UNDER the gate, so the pair answers for the gate
/// itself rather than for a shape the tool cannot read at all.
#[test]
fn the_shipped_typescript_function_length_tool_rule_measures_every_fail_fixture_guard() {
    verify_shipped_fail_fixture_reports_each(
        &TYPESCRIPT_LENGTH_FAIL_PROBE,
        |content| {
            function_length_work(
                TYPESCRIPT_FUNCTION_LENGTH_RULE,
                TYPESCRIPT_LENGTH_FIXTURE_PATH,
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

/// Acceptance: the shipped `function-length-typescript` config READS its
/// framework function names out of the resolved node module tree, and the
/// written mirror it falls back to says the same thing.
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
fn the_shipped_typescript_function_length_config_reads_its_framework_names() {
    let loader = builtin_loader();
    let project_types = ["nodejs"];
    require_tool_installed(&loader, &project_types, TYPESCRIPT_FUNCTION_LENGTH_RULE);
    let source = std::fs::read_to_string(shipped_asset(
        &loader,
        &RULE_SOURCE_ASSET,
        TYPESCRIPT_FUNCTION_LENGTH_RULE,
    ))
    .expect("read the shipped TypeScript function-length rule");
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

/// One body line of the probe function the length script is given no path to.
const TYPESCRIPT_LENGTH_UNREAD_BODY_LINE: &str = "  value += 1;\n";

/// How many body lines that probe function runs.
///
/// The last of them is the `return`, so the source repeats
/// [`TYPESCRIPT_LENGTH_UNREAD_BODY_LINE`] one time fewer. eslint counts the
/// signature line and the closing brace beside the body, so the function
/// answers 302 against the gate of 250 — the number the tool itself wrote,
/// measured: `Function 'branch' has too many lines (302).`
const TYPESCRIPT_LENGTH_UNREAD_BODY_LINES: usize = 300;

/// A TypeScript function of [`TYPESCRIPT_LENGTH_UNREAD_BODY_LINES`] body lines,
/// which stands over the length gate of 250.
///
/// The gate measures LINES, so the source is BUILT here rather than written
/// out. 300 lines of one statement carry no fact a reader needs, and they run
/// this module past the byte cap a review prompt holds.
static TYPESCRIPT_LENGTH_UNREAD_SOURCE: LazyLock<String> = LazyLock::new(|| {
    format!(
        "export function branch(value: number): number {{\n{}  return value;\n}}\n",
        TYPESCRIPT_LENGTH_UNREAD_BODY_LINE.repeat(TYPESCRIPT_LENGTH_UNREAD_BODY_LINES - 1)
    )
});

/// Where the probe repository holds its top-level TypeScript file.
const TYPESCRIPT_LENGTH_UNREAD_TOP_PATH: &str = "top.ts";

/// Where the probe repository holds its nested TypeScript file.
///
/// It stands under two directories, because a tool that reads a default target
/// of its own walks the whole tree rather than the root alone.
const TYPESCRIPT_LENGTH_UNREAD_NESTED_PATH: &str = "deep/nested/other.ts";

/// Every TypeScript file staged in the probe repository the function-length
/// script is given none of.
static TYPESCRIPT_LENGTH_UNREAD_FILES: LazyLock<Vec<(&str, &str)>> = LazyLock::new(|| {
    vec![
        (
            TYPESCRIPT_LENGTH_UNREAD_TOP_PATH,
            TYPESCRIPT_LENGTH_UNREAD_SOURCE.as_str(),
        ),
        (
            TYPESCRIPT_LENGTH_UNREAD_NESTED_PATH,
            TYPESCRIPT_LENGTH_UNREAD_SOURCE.as_str(),
        ),
    ]
});

/// Each finding the TypeScript function-length script reports over the two
/// files it is given, as `path:line`.
///
/// Each probe function opens on the first line of its own file, so each entry
/// names line 1.
const TYPESCRIPT_LENGTH_READ_FINDINGS: &[&str] = &["deep/nested/other.ts:1", "top.ts:1"];

/// The `function-length-typescript` probe over a run that is given no file.
///
/// The staged source is built at run time, so the probe is built here rather
/// than written as a constant.
fn typescript_length_empty_run_probe() -> ShippedEmptyRun {
    ShippedEmptyRun {
        run: ShippedRun {
            project_types: NODEJS_PROJECT_TYPES,
            rule: TYPESCRIPT_FUNCTION_LENGTH_RULE,
            expected: NO_FINDINGS,
        },
        staged: TYPESCRIPT_LENGTH_UNREAD_FILES.as_slice(),
        with_files: TYPESCRIPT_LENGTH_READ_FINDINGS,
        reason: READS_ONLY_ITS_ARGUMENTS,
    }
}

/// Acceptance: the shipped TypeScript function-length tool rule reads only the
/// files it is given, through the real eslint pipeline.
///
/// eslint with no path argument reads the working directory, and the config
/// this rule writes names `**/*.{js,jsx,mjs,cjs,ts,tsx}`, so the run reaches
/// every TypeScript file under the repository root. Measured over this probe
/// with no argument: without the guard the script reported 2 findings and
/// exited 0; with the guard it reports none and exits 0. The same script over
/// the two staged files reports 2.
#[test]
fn the_shipped_typescript_function_length_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&typescript_length_empty_run_probe());
}
