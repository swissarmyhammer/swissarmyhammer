//! Acceptance tests for the shipped `complexity-typescript` tool rule.
//!
//! Each test drives the SHIPPED script, or the SHIPPED eslint config, over
//! a probe repository and reads what the real eslint reported.
//!
//! One module stands for each language of the family, because one file for
//! the whole family runs past the byte cap a review prompt holds.

use super::complexity::complexity_work;
use super::*;

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
