//! Acceptance: the shipped `no-commented-code-parsed` tool rule, end to end.
//!
//! This is the one place the rule's whole chain runs as production runs it:
//! the shipped rule file, the doctor's fixture pair, a real `bash`, the real
//! `sah` binary this package builds, the `find commented_code` op inside it,
//! and the stdout contract the review engine parses.
//!
//! It lives in this package rather than beside the other shipped-rule
//! acceptance tests because the rule's tool IS `sah`. `cargo` defines
//! `CARGO_BIN_EXE_sah` only for this package's own integration tests, so this
//! is the only place a test can name the binary it just built. Under any other
//! test binary the engine's `SAH_BIN` resolution falls back to whatever `sah`
//! sits on `PATH`, which is an older copy with no such op.

use std::collections::BTreeSet;
use std::path::Path;

use swissarmyhammer_common::test_utils::EnvVarGuard;
use swissarmyhammer_validators::review::scope::WorkList;
use swissarmyhammer_validators::review::test_support::{builtin_loader, tool_rule_work};
use swissarmyhammer_validators::review::{
    execute_tool_runs, plan_tool_rules, prompt_rules_for, ToolFallback, ToolReport,
};

/// The validator set that carries the rule.
const CODE_HYGIENE_SET: &str = "code-hygiene";

/// The prompt rule the tool rule supersedes.
const PROMPT_RULE: &str = "no-commented-code";

/// The tool rule under test.
const TOOL_RULE: &str = "no-commented-code-parsed";

/// The environment variable the review engine exports to every tool-rule
/// script, naming the `sah` binary the script invokes.
const SAH_BINARY_ENV: &str = "SAH_BIN";

/// One probe file: its path in the repository, and its whole content.
struct ProbeFile {
    /// The path the work-list carries and the finding must land on.
    path: &'static str,
    /// The file's content — a commented-out block, and nothing else wrong.
    content: &'static str,
}

/// One probe file for each of the three languages the card names, each holding
/// a commented-out function of more than five lines and nothing else the rule
/// reports.
const PROBE_FILES: &[ProbeFile] = &[
    ProbeFile {
        path: "src/lib.rs",
        content: concat!(
            "//! A probe crate for the shipped commented-out-code tool rule.\n",
            "\n",
            "// fn folded_band(grid: &[Vec<i32>], limit: i32) -> i32 {\n",
            "//     let mut band = 0;\n",
            "//     for row in grid {\n",
            "//         band += row.iter().filter(|cell| **cell < limit).count() as i32;\n",
            "//     }\n",
            "//     band\n",
            "// }\n",
            "\n",
            "/// Reads the band a caller asked for.\n",
            "pub fn band(limit: i32) -> i32 {\n",
            "    limit\n",
            "}\n",
        ),
    },
    ProbeFile {
        path: "src/band.py",
        content: concat!(
            "\"\"\"A probe module for the shipped commented-out-code tool rule.\"\"\"\n",
            "\n",
            "# def folded_band(grid, limit):\n",
            "#     band = 0\n",
            "#     for row in grid:\n",
            "#         band += len([cell for cell in row if cell < limit])\n",
            "#     return band\n",
            "#\n",
            "\n",
            "def band(limit):\n",
            "    \"\"\"Read the band a caller asked for.\"\"\"\n",
            "    return limit\n",
        ),
    },
    ProbeFile {
        path: "src/band.ts",
        content: concat!(
            "// function foldedBand(grid: number[][], limit: number): number {\n",
            "//     let band = 0;\n",
            "//     for (const row of grid) {\n",
            "//         band += row.filter((cell) => cell < limit).length;\n",
            "//     }\n",
            "//     return band;\n",
            "// }\n",
            "\n",
            "/** Reads the band a caller asked for. */\n",
            "export function band(limit: number): number {\n",
            "    return limit;\n",
            "}\n",
        ),
    },
];

/// Write every probe file under `root`.
fn write_probe_repository(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("create the probe source directory");
    for probe in PROBE_FILES {
        std::fs::write(root.join(probe.path), probe.content)
            .unwrap_or_else(|e| panic!("write {}: {e}", probe.path));
    }
}

/// A one-validator work-list over every probe file, naming both the prompt
/// rule and the tool rule.
fn commented_code_work() -> WorkList {
    tool_rule_work(
        "a commented-out function in three languages",
        CODE_HYGIENE_SET,
        [PROMPT_RULE.to_string(), TOOL_RULE.to_string()],
        PROBE_FILES.iter().map(|probe| (probe.path, probe.content)),
    )
}

/// Acceptance: the shipped rule reports the commented-out block in Rust,
/// Python and TypeScript, and no LLM reads any of the three.
///
/// Four claims, each checked rather than asserted:
///
/// 1. **The rule is healthy.** It appears in the plan's runs and in none of its
///    fallbacks. Planning runs the doctor, and the doctor runs the shipped
///    fixture pair through this same script, so a healthy plan IS the fixture
///    pair passing.
/// 2. **The prompt rule is suppressed for every matched file.**
/// 3. **Zero LLM validator calls for this rule on matched files.**
///    [`prompt_rules_for`] is the fan-out planner's own filter — the list of
///    rules an agent is given for a file. `no-commented-code` is absent from
///    it for all three files, so no task can carry it and no agent can read
///    it. The report's own `attempted` count says one tool run replaced them.
/// 4. **The finding is real and lands on the right file.** One per language,
///    confirmed, carrying the op's own message.
#[test]
#[serial_test::serial(env)]
fn the_shipped_commented_code_tool_rule_reports_three_languages_with_no_llm_call() {
    let _sah = EnvVarGuard::set(SAH_BINARY_ENV, env!("CARGO_BIN_EXE_sah"));
    let repo = tempfile::tempdir().expect("create a probe repository");
    write_probe_repository(repo.path());
    let loader = builtin_loader();
    let work = commented_code_work();

    let plan = plan_tool_rules(&work, &loader, &["rust", "python", "nodejs"], None);

    let run = plan
        .runs()
        .iter()
        .find(|run| run.rule() == TOOL_RULE)
        .unwrap_or_else(|| {
            panic!(
                "the shipped rule must plan a run, which means its fixtures passed; \
                 fallbacks: {:?}",
                plan.fallbacks()
            )
        });
    assert_eq!(
        run.files(),
        PROBE_FILES
            .iter()
            .map(|probe| probe.path.to_string())
            .collect::<Vec<String>>(),
        "the run must carry every matched file"
    );

    let ruleset = loader
        .get_ruleset(CODE_HYGIENE_SET)
        .expect("code-hygiene should be loaded");
    for probe in PROBE_FILES {
        let suppressed = plan
            .suppression()
            .suppressed_rules(CODE_HYGIENE_SET, probe.path);
        assert!(
            suppressed.contains(PROMPT_RULE),
            "a healthy tool rule must suppress `{PROMPT_RULE}` for {}",
            probe.path
        );
        let reading_list: BTreeSet<String> = prompt_rules_for(ruleset, &suppressed)
            .into_iter()
            .map(|rule| rule.name)
            .collect();
        assert!(
            !reading_list.contains(PROMPT_RULE),
            "no agent may be given `{PROMPT_RULE}` for {}; its reading list is {reading_list:?}",
            probe.path
        );
    }

    let outcome = execute_tool_runs(std::slice::from_ref(run), repo.path(), None);
    assert!(
        outcome.errors().is_empty(),
        "the shipped pipeline must not break; errors: {:?}",
        outcome.errors()
    );

    let runs_for_rule = plan
        .runs()
        .iter()
        .filter(|run| run.rule() == TOOL_RULE)
        .count();
    let fallbacks_for_rule: Vec<ToolFallback> = plan
        .fallbacks()
        .iter()
        .filter(|fallback| fallback.rule() == TOOL_RULE)
        .cloned()
        .collect();
    let report = ToolReport::new(runs_for_rule, outcome.errors().to_vec(), fallbacks_for_rule);
    assert_eq!(
        report.attempted(),
        1,
        "one tool run answers all three files, so the rule costs one process and no agent turn"
    );
    assert!(
        report.fallbacks().is_empty(),
        "a fallback would put the prompt rule back in front of an agent: {:?}",
        report.fallbacks()
    );

    for probe in PROBE_FILES {
        let findings: Vec<_> = outcome
            .findings()
            .iter()
            .filter(|verified| verified.finding.file == probe.path)
            .collect();
        assert_eq!(
            findings.len(),
            1,
            "exactly one finding must land on {}; got {:?}",
            probe.path,
            outcome.findings()
        );
        assert!(findings[0].confirmed);
        assert_eq!(findings[0].finding.validator, CODE_HYGIENE_SET);
        assert_eq!(findings[0].finding.rule.as_deref(), Some(TOOL_RULE));
        assert!(
            findings[0].finding.claim.contains("commented-out code"),
            "the claim must be the op's own message; got '{}'",
            findings[0].finding.claim
        );
    }
}

/// Acceptance: a file whose comments are documentation, prose and a short
/// snippet draws no finding from the same run.
///
/// The pass half of the pair, run through the production path rather than
/// through the doctor. Without it a rule that reported every comment block
/// would still satisfy the test above.
#[test]
#[serial_test::serial(env)]
fn the_shipped_commented_code_tool_rule_reports_nothing_on_exempt_comments() {
    let _sah = EnvVarGuard::set(SAH_BINARY_ENV, env!("CARGO_BIN_EXE_sah"));
    let repo = tempfile::tempdir().expect("create a probe repository");
    std::fs::create_dir_all(repo.path().join("src")).expect("create the source directory");
    let exempt = concat!(
        "/// Folds a grid of readings into one band.\n",
        "///\n",
        "/// ```\n",
        "/// fn folded_band(grid: &[Vec<i32>], limit: i32) -> i32 {\n",
        "///     let mut band = 0;\n",
        "///     for row in grid {\n",
        "///         band += row.iter().filter(|cell| **cell < limit).count() as i32;\n",
        "///     }\n",
        "///     band\n",
        "/// }\n",
        "/// ```\n",
        "pub fn documented_band(limit: i32) -> i32 {\n",
        "    limit\n",
        "}\n",
        "\n",
        "// TODO: fold the band over the whole grid rather than one row at a time.\n",
        "// The caller reads one row today, and the change wants the shape above.\n",
        "// It stays prose until the settings file grows a section for the limit,\n",
        "// because the limit is what decides how many rows a fold may touch.\n",
        "// See the design note beside this module for the order the rows take.\n",
        "// Until then this comment is a note to a reader and nothing more.\n",
        "/// Reads the band a caller asked for.\n",
        "pub fn band(limit: i32) -> i32 {\n",
        "    limit\n",
        "}\n",
    );
    std::fs::write(repo.path().join("src/lib.rs"), exempt).expect("write the probe file");

    let work = tool_rule_work(
        "documentation, prose and a short snippet",
        CODE_HYGIENE_SET,
        [PROMPT_RULE.to_string(), TOOL_RULE.to_string()],
        [("src/lib.rs", exempt)],
    );
    let loader = builtin_loader();

    let plan = plan_tool_rules(&work, &loader, &["rust"], None);
    let run = plan
        .runs()
        .iter()
        .find(|run| run.rule() == TOOL_RULE)
        .unwrap_or_else(|| panic!("the shipped rule must plan a run; {:?}", plan.fallbacks()));

    let outcome = execute_tool_runs(std::slice::from_ref(run), repo.path(), None);

    assert!(outcome.errors().is_empty(), "{:?}", outcome.errors());
    assert!(
        outcome.findings().is_empty(),
        "documentation, prose and a two-line snippet are all exempt; got {:?}",
        outcome.findings()
    );
}
