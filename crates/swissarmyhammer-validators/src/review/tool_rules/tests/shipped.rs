use super::preconditions::require_tool_installed;
use super::*;

use std::path::PathBuf;

use swissarmyhammer_common::test_utils::CurrentDirGuard;

use crate::doctor::FIXTURE_TEMPLATE_SUFFIX;
use crate::review::scope::{FileWork, ProbeNames, RuleNames};
use crate::review::test_support::builtin_loader;
use crate::validators::types::FIXTURES_DIR_NAME;

/// The run `rule` planned, or a panic naming what the plan fell back to.
///
/// Every shipped-rule acceptance test REQUIRES its run. A rule that planned
/// none leaves a [`ToolFallback`] carrying the doctor's reason — the missing
/// tool, or the fixture pair that failed — so the panic names that reason
/// rather than reporting a bare absence. Returning early instead would leave
/// the test asserting nothing, and a test that cannot fail is not a gate.
fn required_run<'a>(plan: &'a ToolPlan, rule: &str) -> &'a ToolRun {
    plan.runs()
        .iter()
        .find(|run| run.rule() == rule)
        .unwrap_or_else(|| {
            panic!(
                "the shipped tool rule `{rule}` must plan a run; fallbacks: {:?}",
                plan.fallbacks()
            )
        })
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

/// A Python module with one statement stranded behind a `return`.
///
/// `__all__` names the one function, which is how Python declares an
/// exported surface and how vulture is told the function has callers
/// outside the module. Without it the run reports the function as unused
/// too, and the probe would measure two findings rather than the one
/// stranded statement it is built to measure.
const UNREACHABLE_MODULE_PY: &str = concat!(
    "\"\"\"A probe module for the shipped Python dead-code tool rule.\"\"\"\n\n",
    "__all__ = [\"stops_early\"]\n\n\n",
    "def stops_early():\n",
    "    \"\"\"Return a value, then strand the statement below it.\"\"\"\n",
    "    return 1\n",
    "    print(\"stranded\")\n",
);

/// The module path inside the probe repository, as the work-list holds it.
const UNREACHABLE_MODULE_PATH: &str = "src/stops_early.py";

/// A one-validator work-list over `files` for the builtin `code-hygiene`
/// set, naming both the `dead-code` prompt rule and the Python tool rule.
fn dead_code_work(path: &str, content: &str) -> WorkList {
    WorkList::new(
        "a statement behind a return",
        vec![ValidatorWork::new(
            CODE_HYGIENE_SET,
            RuleNames::new([
                DEAD_CODE_PROMPT_RULE.to_string(),
                PYTHON_DEAD_CODE_RULE.to_string(),
            ]),
            ProbeNames::new([]),
            [FileWork::new(path, vec![], vec![], content, vec![])],
        )],
    )
}

/// Acceptance: the shipped Python dead-code tool rule suppresses the
/// `dead-code` prompt rule, and reports the stranded statement through the
/// real vulture pipeline.
///
/// The suppression half is the load-bearing one: a healthy tool rule
/// suppresses whatever its `supersedes` names. Superseding is what makes
/// dead code objective for Python — the prompt rule's carve-outs for the
/// exported surface and for staged work become `__all__` and
/// `# noqa: V1xx`, which the tool reads.
///
/// The test states the tool as a precondition — [`require_tool_installed`]
/// names the missing tool and the command that installs it — and then
/// REQUIRES the run. A rule that planned no run fails the test and names
/// the plan's fallbacks, which carry the doctor's reason. Returning early
/// instead would leave the test asserting nothing, and a test that cannot
/// fail is not a gate.
#[test]
fn the_shipped_python_dead_code_tool_rule_reports_and_suppresses_dead_code() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(
        repo.path().join(UNREACHABLE_MODULE_PATH),
        UNREACHABLE_MODULE_PY,
    )
    .unwrap();
    let loader = builtin_loader();
    let project_types = ["python"];
    require_tool_installed(&loader, &project_types, PYTHON_DEAD_CODE_RULE);
    let work = dead_code_work(UNREACHABLE_MODULE_PATH, UNREACHABLE_MODULE_PY);

    let plan = plan_tool_rules(&work, &loader, &project_types, None);

    let run = required_run(&plan, PYTHON_DEAD_CODE_RULE);
    assert_eq!(run.files(), [UNREACHABLE_MODULE_PATH.to_string()]);
    assert!(
        plan.suppression()
            .suppressed_rules(CODE_HYGIENE_SET, UNREACHABLE_MODULE_PATH)
            .contains(DEAD_CODE_PROMPT_RULE),
        "a healthy dead-code tool rule must suppress the `dead-code` prompt rule, \
         so no LLM re-reads a question the tool already decided"
    );

    verify_run_reports_one_finding(
        run,
        repo.path(),
        UNREACHABLE_MODULE_PATH,
        CODE_HYGIENE_SET,
        PYTHON_DEAD_CODE_RULE,
        "unreachable code after",
    );
}

/// A kind of file a builtin validator set ships beside its manifest.
///
/// Each kind names the one directory that holds the file and the one suffix
/// the set adds to the asked-for name, so one lookup serves every kind.
struct ShippedAssetKind {
    /// The directory under the set's base path that holds the file.
    dir: &'static str,
    /// What the set adds to the asked-for name on disk.
    suffix: &'static str,
    /// What to call the file in the failure message.
    label: &'static str,
}

/// The fixture template a set carries for a materialized file name.
///
/// A set stores `<name>.tmpl`, so a test that wants the shipped bytes asks
/// for `<name>` and gets the template beside it.
const FIXTURE_TEMPLATE_ASSET: ShippedAssetKind = ShippedAssetKind {
    dir: FIXTURES_DIR_NAME,
    suffix: FIXTURE_TEMPLATE_SUFFIX,
    label: "fixture template",
};

/// The rule source a set carries for a rule name, beside its `fixtures/`.
const RULE_SOURCE_ASSET: ShippedAssetKind = ShippedAssetKind {
    dir: "rules",
    suffix: ".md",
    label: "rule source",
};

/// The path of the `kind` file named `name`, inside whichever builtin
/// validator set carries it.
fn shipped_asset(loader: &ValidatorLoader, kind: &ShippedAssetKind, name: &str) -> PathBuf {
    loader
        .list_rulesets()
        .iter()
        .map(|ruleset| {
            ruleset
                .base_path
                .join(kind.dir)
                .join(format!("{name}{}", kind.suffix))
        })
        .find(|path| path.exists())
        .unwrap_or_else(|| panic!("a builtin validator set must ship a {name} {}", kind.label))
}

/// A shipped fail fixture, and what the real pipeline of one tool rule must
/// report over it.
///
/// The doctor fixture contract asks a fail fixture for ONE finding, so a rule
/// that reported one position and stayed silent on every other would still
/// pass it. A probe names each position, so each one is load-bearing on its
/// own, and the count then states what the tool does NOT read as a measured
/// fact rather than leaving it to be discovered.
struct ShippedFailFixture {
    /// The project types the rule is planned for.
    project_types: &'static [&'static str],

    /// The tool rule that must plan the run.
    rule: &'static str,

    /// The materialized name of the fail fixture the set ships.
    fixture: &'static str,

    /// Where the fixture stands inside the probe repository, as the work-list
    /// holds it.
    path: &'static str,

    /// Every other shipped fixture the probe repository needs beside the fail
    /// fixture, each with the name the set ships it under and the path it
    /// takes in the repository.
    ///
    /// A `files`-scope rule reads the files it is given and needs none. A
    /// `workspace`-scope rule loads a project rather than a file list, so the
    /// project manifest the set ships stands here.
    support: &'static [(&'static str, &'static str)],

    /// One entry for each finding the run must report in the fixture, and no
    /// other.
    expected: &'static [&'static str],

    /// What one entry of `expected` is, for the failure messages.
    noun: &'static str,
}

/// What a `files`-scope probe names for `support`: nothing. The tool reads the
/// files it is given, so the fail fixture alone is the whole repository.
const NO_SUPPORT_FIXTURES: &[(&str, &str)] = &[];

/// Copies the shipped fixture template named `fixture` into `repo` at `path`,
/// and answers the bytes it wrote.
///
/// The fixture is read where the set ships it, so a run over the copy measures
/// the SHIPPED bytes. A test that wrote its own copy would answer for the copy.
fn copy_shipped_fixture(
    loader: &ValidatorLoader,
    repo: &Path,
    fixture: &str,
    path: &str,
) -> String {
    let shipped = shipped_asset(loader, &FIXTURE_TEMPLATE_ASSET, fixture);
    let content = std::fs::read_to_string(&shipped).expect("read the shipped fixture template");
    let file = repo.join(path);
    std::fs::create_dir_all(file.parent().expect("the fixture path has a parent")).unwrap();
    std::fs::write(&file, &content).unwrap();
    content
}

/// Drives the shipped fail fixture of `probe` through the real tool pipeline,
/// and holds the run to exactly the entries the probe names.
///
/// The fixture, and every support fixture the probe names beside it, is copied
/// into a temporary repository by [`copy_shipped_fixture`].
///
/// Three callbacks carry what one language does not share with another.
/// `build_work` states the work-list, because the rule list a language names is
/// its own. `extract` reads the text one finding is held to, and takes the
/// fixture's source lines as well, because one language holds a finding to its
/// claim and another holds it to the source line it stands on. `matches` states
/// how an entry meets that text.
fn verify_shipped_fail_fixture_reports_each<W, E, M>(
    probe: &ShippedFailFixture,
    build_work: W,
    extract: E,
    matches: M,
) where
    W: FnOnce(&str) -> WorkList,
    E: Fn(&VerifiedFinding, &[&str]) -> String,
    M: Fn(&str, &str) -> bool,
{
    let loader = builtin_loader();
    require_tool_installed(&loader, probe.project_types, probe.rule);
    let repo = tempfile::tempdir().unwrap();
    let content = copy_shipped_fixture(&loader, repo.path(), probe.fixture, probe.path);
    for (fixture, path) in probe.support {
        copy_shipped_fixture(&loader, repo.path(), fixture, path);
    }
    // A tool prints the resolved path of each file it reads, and on macOS a
    // temporary directory stands behind a symbolic link. The engine strips the
    // repository root off each reported path, so the root it is given has to be
    // the resolved form or no path matches and every finding keeps an absolute
    // path.
    let repo_root = repo
        .path()
        .canonicalize()
        .expect("resolve the probe repository path");
    let work = build_work(&content);

    let plan = plan_tool_rules(&work, &loader, probe.project_types, None);

    let run = required_run(&plan, probe.rule);
    let outcome = execute_tool_runs(std::slice::from_ref(run), &repo_root, None);
    assert!(
        outcome.errors().is_empty(),
        "the shipped pipeline must not break; errors: {:?}",
        outcome.errors()
    );

    let source: Vec<&str> = content.lines().collect();
    let reported: Vec<String> = outcome
        .findings()
        .iter()
        .filter(|verified| verified.finding.file == probe.path)
        .map(|verified| extract(verified, &source))
        .collect();
    for entry in probe.expected {
        assert!(
            reported.iter().any(|text| matches(text, entry)),
            "the fail fixture must report the {} `{entry}`; the run reported {reported:?}",
            probe.noun
        );
    }
    assert_eq!(
        reported.len(),
        probe.expected.len(),
        "the fail fixture holds one finding for each {} and no other; got {reported:?}",
        probe.noun
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
    project_types: &["nodejs"],
    rule: TYPESCRIPT_COMPLEXITY_RULE,
    fixture: TYPESCRIPT_COMPLEXITY_FAIL_FIXTURE,
    path: TYPESCRIPT_COMPLEXITY_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    expected: TYPESCRIPT_COMPLEXITY_FAIL_GUARDS,
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
        |verified, source| {
            let line = verified.finding.line;
            source
                .get(line as usize - 1)
                .unwrap_or_else(|| panic!("line {line} stands past the end of the fixture"))
                .trim()
                .to_string()
        },
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

/// Sorted names, for a set comparison that does not depend on read order.
fn sorted_names(names: &[String]) -> Vec<String> {
    let mut sorted = names.to_vec();
    sorted.sort();
    sorted
}

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

/// Names the prompt rules a roster row expects, for a failure message.
fn supersedes_label(expected: &[&str]) -> String {
    match expected.is_empty() {
        true => "nothing".to_string(),
        false => expected.join(", "),
    }
}

/// The name of the SwiftPM manifest, and of the fixture template that
/// carries one.
const SWIFT_MANIFEST: &str = "Package.swift";

/// A Swift package root as the process working directory, held until the
/// returned pair drops.
///
/// The guard comes first and the directory second, because a tuple drops
/// its fields in declaration order. The guard therefore restores the
/// working directory before [`tempfile::TempDir`] removes the root. The
/// other order removes the root while the process still stands in it, and
/// `getcwd` fails for every call until the guard runs.
///
/// `dead-code-swift` checks `which periphery swift jq && test -f
/// Package.swift`, because periphery scans a built SPM package and reports
/// itself missing outside one. That half of the check is a working-directory
/// precondition rather than a tool, so no install command can satisfy it and
/// a roster test that requires every row has to supply it. The manifest is
/// the shipped fixture template, so the test states no manifest of its own.
///
/// The fixture runs are unaffected: doctor materializes each pair into its
/// own scratch directory, `Package.swift.tmpl` included, and runs the script
/// there.
fn swift_package_root(loader: &ValidatorLoader) -> (CurrentDirGuard, tempfile::TempDir) {
    let manifest = shipped_asset(loader, &FIXTURE_TEMPLATE_ASSET, SWIFT_MANIFEST);
    let root = tempfile::tempdir().expect("temp dir");
    std::fs::copy(&manifest, root.path().join(SWIFT_MANIFEST))
        .expect("copy the shipped Package.swift template");
    let guard = CurrentDirGuard::new(root.path()).expect("cwd guard");
    (guard, root)
}

/// The pair [`swift_package_root`] returns restores the working directory
/// before it removes that directory.
///
/// A tuple drops its fields in declaration order, so the first element runs
/// first. A `TempDir` in that position removes the package root while the
/// root is still the process working directory, and `getcwd` then fails for
/// the whole window until the guard runs. The guard therefore has to be the
/// first element.
#[test]
fn the_swift_package_root_restores_the_directory_before_it_removes_it() {
    let loader = builtin_loader();
    let outside = std::env::current_dir().expect("a working directory before the guard");

    let (first, second) = swift_package_root(&loader);
    assert_ne!(
        std::env::current_dir().expect("the guard entered the package root"),
        outside,
        "the guard must enter the package root"
    );

    drop(first);

    let restored = std::env::current_dir();
    assert_eq!(
        restored.as_ref().ok().map(PathBuf::as_path),
        Some(outside.as_path()),
        "the first element must restore the working directory; instead the working \
         directory was removed while the process still stood in it"
    );
    drop(second);
}

/// Drives every rule in `rules` through the real install, doctor and
/// fixture path, and holds each one to the fixture contract.
///
/// Each row names a project type, the tool rule that serves it, and the
/// prompt rules that rule must supersede — empty for a rule that must leave
/// its prompt rule running. For each row, the helper reads the doctor row
/// and asserts what the row supersedes. The list belongs to the row rather
/// than to the call because one roster — the complexity rules — mixes rules
/// that replace one prompt rule with a rule that replaces two.
///
/// Every row keeps one contract, the same one the single-rule acceptance
/// tests keep: [`require_tool_installed`] gets the tool through the rule's
/// own declared install commands, and the fixture assertion then runs for
/// every row rather than for the rows this machine happens to carry. A row
/// whose tool cannot be obtained fails the test, naming the binary and the
/// command that installs it.
///
/// The degradation contract — a missing tool falls the rule back to its
/// prompt rule and never blocks a review — is held by
/// [`plan_reports_a_fallback_when_the_tool_is_missing_and_suppresses_nothing`]
/// and [`a_missing_tool_whose_installs_all_fail_stays_on_the_prompt_fallback`],
/// which state it over built specs and need no tool at all.
///
/// `rule_kind` names the group in the failure messages — the prompt rule the
/// group is named for, whether the group replaces that rule or runs beside
/// it — so a failing run says which roster broke.
fn verify_shipped_tool_rules_pass_fixtures(rules: &[(&str, &str, &[&str])], rule_kind: &str) {
    let loader = builtin_loader();
    let _package_root = swift_package_root(&loader);

    for (project_type, rule_name, expected_supersedes) in rules {
        let project_types = [*project_type];
        require_tool_installed(&loader, &project_types, rule_name);

        let status = crate::doctor::check_review_engine_with(&loader, &project_types, None);
        let row = status
            .tool_rules
            .iter()
            .find(|row| row.rule_name == *rule_name)
            .unwrap_or_else(|| panic!("{rule_name} must be reported for a {project_type} project"));
        assert_eq!(
            row.supersedes.names(),
            *expected_supersedes,
            "{rule_name} must supersede {}, the contract every {rule_kind} tool rule keeps",
            supersedes_label(expected_supersedes)
        );
        assert!(
            row.usable(),
            "{rule_name}'s tool is installed, so its fixtures must pass; doctor says: {}",
            row.degraded_detail()
        );
    }
}

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

/// Acceptance: every shipped dead-code tool rule passes its fixture pair in
/// doctor, and supersedes the `dead-code` prompt rule.
///
/// The pass fixture is the load-bearing half. Each pass fixture holds the
/// same dead shapes its fail fixture holds, each behind the language's own
/// suppression marker, so a marker the tool stops reading makes the pair
/// fail. That marker is the whole staging contract: with it the tool
/// replaces the prompt rule's judgment, and without it the tool would
/// report staged work as dead.
#[test]
fn every_shipped_dead_code_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(SHIPPED_DEAD_CODE_RULES, DEAD_CODE_PROMPT_RULE);
}

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
    project_types: &["python"],
    rule: PYTHON_MAGIC_NUMBERS_RULE,
    fixture: PYTHON_MAGIC_NUMBERS_FAIL_FIXTURE,
    path: PYTHON_MAGIC_NUMBERS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    expected: PYTHON_MAGIC_NUMBERS_FAIL_VALUES,
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
    project_types: &["nodejs"],
    rule: TYPESCRIPT_MAGIC_NUMBERS_RULE,
    fixture: TYPESCRIPT_MAGIC_NUMBERS_FAIL_FIXTURE,
    path: TYPESCRIPT_MAGIC_NUMBERS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    expected: TYPESCRIPT_MAGIC_NUMBERS_FAIL_VALUES,
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
    project_types: &["go"],
    rule: GO_MAGIC_NUMBERS_RULE,
    fixture: GO_MAGIC_NUMBERS_FAIL_FIXTURE,
    path: GO_MAGIC_NUMBERS_FIXTURE_PATH,
    support: GO_MAGIC_NUMBERS_SUPPORT,
    expected: GO_MAGIC_NUMBERS_FAIL_VALUES,
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
    project_types: &["swift"],
    rule: SWIFT_MAGIC_NUMBERS_RULE,
    fixture: SWIFT_MAGIC_NUMBERS_FAIL_FIXTURE,
    path: SWIFT_MAGIC_NUMBERS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    expected: SWIFT_MAGIC_NUMBERS_FAIL_LINES,
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
        |verified, source| {
            let line = verified.finding.line;
            source
                .get(line as usize - 1)
                .unwrap_or_else(|| panic!("line {line} stands past the end of the fixture"))
                .trim()
                .to_string()
        },
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
    project_types: &["flutter"],
    rule: DART_MAGIC_NUMBERS_RULE,
    fixture: DART_MAGIC_NUMBERS_FAIL_FIXTURE,
    path: DART_MAGIC_NUMBERS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    expected: DART_MAGIC_NUMBERS_FAIL_LINES,
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
        |verified, source| {
            let line = verified.finding.line;
            source
                .get(line as usize - 1)
                .unwrap_or_else(|| panic!("line {line} stands past the end of the fixture"))
                .trim()
                .to_string()
        },
        |reported, expected| reported == expected,
    );
}

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

/// Acceptance: every shipped unused-dependency tool rule passes its fixture
/// pair in doctor, and supersedes nothing.
///
/// The pass fixture is the load-bearing half. It declares the same unused
/// dependency its fail fixture declares, and the only difference between
/// the two is `[package.metadata.cargo-machete] ignored`, so a machete
/// release that stopped reading that key — or stopped reading it through
/// the trailing comment the entry carries — makes the pair fail and takes
/// the rule out of the review.
#[test]
fn every_shipped_unused_dependency_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(
        SHIPPED_UNUSED_DEPENDENCY_RULES,
        UNUSED_DEPENDENCIES_RULE_KIND,
    );
}

/// A cargo package that uses one dependency and declares a second one no
/// source names. `[workspace]` keeps cargo inside the temporary directory.
const UNUSED_DEPENDENCY_PACKAGE_MANIFEST: &str = concat!(
    "[package]\nname = \"unused-dependency-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[dependencies]\nlibc = \"0.2\"\nserde = \"1\"\n",
    "\n[workspace]\n",
);

/// The library of [`UNUSED_DEPENDENCY_PACKAGE_MANIFEST`]. It names `libc`
/// and never `serde`, so `serde` is the one finding the rule must report.
const UNUSED_DEPENDENCY_LIB_RS: &str = concat!(
    "//! A probe crate for the shipped Rust unused-dependency tool rule.\n\n",
    "/// The system page size, read through the one dependency this file names.\n",
    "pub fn page_size() -> i64 {\n",
    "    unsafe { libc::sysconf(libc::_SC_PAGESIZE) }\n",
    "}\n",
);

/// The manifest path inside the probe repository, as the work-list holds
/// it. This is the file the finding must land on — not the source file that
/// fails to name the dependency.
const UNUSED_DEPENDENCY_MANIFEST_PATH: &str = "Cargo.toml";

/// The library path inside the probe package.
const UNUSED_DEPENDENCY_LIB_PATH: &str = "src/lib.rs";

/// A one-validator work-list over `path` for the builtin `manifests` set,
/// naming its one tool rule.
fn manifests_work(path: &str, content: &str) -> WorkList {
    WorkList::new(
        "a declared dependency no source names",
        vec![ValidatorWork::new(
            MANIFESTS_SET,
            RuleNames::new([RUST_UNUSED_DEPENDENCIES_RULE.to_string()]),
            ProbeNames::new([]),
            [FileWork::new(path, vec![], vec![], content, vec![])],
        )],
    )
}

/// Acceptance: the shipped Rust unused-dependency tool rule reports a
/// declared dependency no source names, on a real cargo package, through
/// the real `cargo machete` pipeline.
///
/// This is the production path the fixture pair cannot reach. A fixture is
/// a manifest under a fixture name, which machete refuses to read, so the
/// script normalizes it into a temporary package; a real manifest is named
/// `Cargo.toml` and is scanned where it lies. Only a run over a real
/// package exercises that half, and only it proves the finding lands on the
/// manifest rather than on the source file.
///
/// The test states the tool as a precondition — [`require_tool_installed`]
/// names the missing tool and the command that installs it — and then
/// REQUIRES the run. A rule that planned no run fails the test and names
/// the plan's fallbacks, which carry the doctor's reason. Returning early
/// instead would leave the test asserting nothing, and a test that cannot
/// fail is not a gate.
#[test]
fn the_shipped_rust_unused_dependency_tool_rule_reports_an_unused_dependency() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(
        repo.path().join(UNUSED_DEPENDENCY_MANIFEST_PATH),
        UNUSED_DEPENDENCY_PACKAGE_MANIFEST,
    )
    .unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(
        repo.path().join(UNUSED_DEPENDENCY_LIB_PATH),
        UNUSED_DEPENDENCY_LIB_RS,
    )
    .unwrap();
    let loader = builtin_loader();
    let project_types = ["rust"];
    require_tool_installed(&loader, &project_types, RUST_UNUSED_DEPENDENCIES_RULE);
    let work = manifests_work(
        UNUSED_DEPENDENCY_MANIFEST_PATH,
        UNUSED_DEPENDENCY_PACKAGE_MANIFEST,
    );

    let plan = plan_tool_rules(&work, &loader, &project_types, None);

    let run = required_run(&plan, RUST_UNUSED_DEPENDENCIES_RULE);
    assert_eq!(
        run.files(),
        [UNUSED_DEPENDENCY_MANIFEST_PATH.to_string()],
        "the run must carry the changed manifest, so the engine keeps the finding"
    );

    verify_run_reports_one_finding(
        run,
        repo.path(),
        UNUSED_DEPENDENCY_MANIFEST_PATH,
        MANIFESTS_SET,
        RUST_UNUSED_DEPENDENCIES_RULE,
        "unused dependency `serde`",
    );
}

#[test]
fn execute_emits_no_planned_event_when_there_are_no_runs() {
    let repo = tempfile::tempdir().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let outcome = execute_tool_runs(&[], repo.path(), Some(&tx));

    assert_eq!(outcome, ToolOutcome::default());
    assert!(rx.try_recv().is_err(), "no events for an empty plan");
}
