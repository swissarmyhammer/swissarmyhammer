//! Acceptance: the shipped `duplication-parsed` tool rule, end to end.
//!
//! This is the one place the rule's whole chain runs as production runs it:
//! the shipped rule file, the doctor's fixture pair, a real `bash`, the real
//! `sah` binary this package builds, the `find duplication` op inside it, and
//! the stdout contract the review engine parses.
//!
//! It lives in this package rather than beside the other shipped-rule
//! acceptance tests because the rule's tool IS `sah`. `cargo` defines
//! `CARGO_BIN_EXE_sah` only for this package's own integration tests, so this
//! is the only place a test can name the binary it just built. Under any other
//! test binary the engine's `SAH_BIN` resolution falls back to whatever `sah`
//! sits on `PATH`, which is an older copy with no such op.

use std::collections::BTreeSet;

use swissarmyhammer_common::test_utils::EnvVarGuard;
use swissarmyhammer_sem::parser::plugins::code::get_all_code_extensions;
use swissarmyhammer_validators::review::scope::WorkList;
use swissarmyhammer_validators::review::test_support::{builtin_loader, tool_rule_work};
use swissarmyhammer_validators::review::{
    execute_tool_runs, plan_tool_rules, prompt_rules_for, ToolFallback, ToolReport,
};

/// The validator set that carries the rule.
const DUPLICATION_SET: &str = "duplication";

/// The prompt rules the tool rule supersedes.
const PROMPT_RULES: &[&str] = &["duplication", "rust", "swift"];

/// The tool rule under test.
const TOOL_RULE: &str = "duplication-parsed";

/// The environment variable the review engine exports to every tool-rule
/// script, naming the `sah` binary the script invokes.
const SAH_BINARY_ENV: &str = "SAH_BIN";

/// The path the probe file takes in the probe repository.
const PROBE_PATH: &str = "src/lib.rs";

/// A Rust file whose only defect is one function copied and its variables
/// renamed.
///
/// The two copies share no run of tokens at all, because every mention of
/// the accumulator differs, so the rule this pair replaced reported nothing
/// here. Once each body is normalized the two streams are equal. The pair is
/// intra-file on purpose: that is the case a path glob can never reach.
const RENAMED_COPY_RS: &str = concat!(
    "//! A probe crate for the shipped near-duplicate tool rule.\n",
    "\n",
    "/// Folds the readings of one grid into a band.\n",
    "pub fn folded_band(grid: &[Vec<i32>], limit: i32) -> i32 {\n",
    "    let mut band = 0;\n",
    "    let mut seen = 0;\n",
    "    for row in grid {\n",
    "        for cell in row {\n",
    "            if *cell < limit {\n",
    "                band += *cell;\n",
    "                seen += 1;\n",
    "            } else {\n",
    "                band -= *cell;\n",
    "            }\n",
    "        }\n",
    "        if seen > limit {\n",
    "            band = limit;\n",
    "        }\n",
    "    }\n",
    "    band\n",
    "}\n",
    "\n",
    "/// Folds the readings of one mirrored grid into a band.\n",
    "pub fn mirrored_band(grid: &[Vec<i32>], limit: i32) -> i32 {\n",
    "    let mut total = 0;\n",
    "    let mut count = 0;\n",
    "    for row in grid {\n",
    "        for cell in row {\n",
    "            if *cell < limit {\n",
    "                total += *cell;\n",
    "                count += 1;\n",
    "            } else {\n",
    "                total -= *cell;\n",
    "            }\n",
    "        }\n",
    "        if count > limit {\n",
    "            total = limit;\n",
    "        }\n",
    "    }\n",
    "    total\n",
    "}\n",
);

/// The same two functions, with the marker comment on the second.
const MARKED_COPY_RS: &str = concat!(
    "//! A probe crate for the shipped near-duplicate tool rule.\n",
    "\n",
    "/// Folds the readings of one grid into a band.\n",
    "pub fn folded_band(grid: &[Vec<i32>], limit: i32) -> i32 {\n",
    "    let mut band = 0;\n",
    "    let mut seen = 0;\n",
    "    for row in grid {\n",
    "        for cell in row {\n",
    "            if *cell < limit {\n",
    "                band += *cell;\n",
    "                seen += 1;\n",
    "            } else {\n",
    "                band -= *cell;\n",
    "            }\n",
    "        }\n",
    "        if seen > limit {\n",
    "            band = limit;\n",
    "        }\n",
    "    }\n",
    "    band\n",
    "}\n",
    "\n",
    "// sah:allow duplication the mirrored reading forks when it gains its own limit\n",
    "/// Folds the readings of one mirrored grid into a band.\n",
    "pub fn mirrored_band(grid: &[Vec<i32>], limit: i32) -> i32 {\n",
    "    let mut total = 0;\n",
    "    let mut count = 0;\n",
    "    for row in grid {\n",
    "        for cell in row {\n",
    "            if *cell < limit {\n",
    "                total += *cell;\n",
    "                count += 1;\n",
    "            } else {\n",
    "                total -= *cell;\n",
    "            }\n",
    "        }\n",
    "        if count > limit {\n",
    "            total = limit;\n",
    "        }\n",
    "    }\n",
    "    total\n",
    "}\n",
);

/// A probe repository holding `contents` at [`PROBE_PATH`].
fn probe_repository(contents: &str) -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("create a probe repository");
    std::fs::create_dir_all(repo.path().join("src")).expect("create the source directory");
    std::fs::write(repo.path().join(PROBE_PATH), contents).expect("write the probe file");
    repo
}

/// A one-validator work-list over the probe file, naming every prompt rule the
/// tool rule supersedes and the tool rule itself.
fn duplication_work(contents: &'static str) -> WorkList {
    let rules = PROMPT_RULES
        .iter()
        .map(|rule| (*rule).to_string())
        .chain(std::iter::once(TOOL_RULE.to_string()));
    tool_rule_work(
        "one function copied and its variables renamed",
        DUPLICATION_SET,
        rules,
        [(PROBE_PATH, contents)],
    )
}

/// Acceptance: a review whose only defect is a copied function reports it, and
/// no LLM reads the duplication set for that file.
///
/// Four claims, each checked rather than asserted:
///
/// 1. **The rule is healthy.** It appears in the plan's runs and in none of its
///    fallbacks. Planning runs the doctor, and the doctor runs the shipped
///    fixture pair through this same script, so a healthy plan IS the fixture
///    pair passing.
/// 2. **Every superseded prompt rule is suppressed for the matched file.**
/// 3. **Zero LLM validator calls for the duplication set on matched files.**
///    [`prompt_rules_for`] is the fan-out planner's own filter — the list of
///    rules an agent is given for a file. It comes back EMPTY for the whole
///    set, so no task can carry a rule of it and no agent can read one. The
///    report's own `attempted` count says one tool run replaced them.
/// 4. **The finding is real and lands on the right file**, carrying the op's
///    own message.
#[test]
#[serial_test::serial(env)]
fn the_shipped_duplication_tool_rule_reports_a_renamed_copy_with_no_llm_call() {
    let _sah = EnvVarGuard::set(SAH_BINARY_ENV, env!("CARGO_BIN_EXE_sah"));
    let repo = probe_repository(RENAMED_COPY_RS);
    let loader = builtin_loader();
    let work = duplication_work(RENAMED_COPY_RS);

    let plan = plan_tool_rules(&work, &loader, &["rust"], None);

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
        vec![PROBE_PATH.to_string()],
        "the run must carry the matched file"
    );

    let ruleset = loader
        .get_ruleset(DUPLICATION_SET)
        .expect("duplication should be loaded");
    let suppressed = plan
        .suppression()
        .suppressed_rules(DUPLICATION_SET, PROBE_PATH);
    for prompt_rule in PROMPT_RULES {
        assert!(
            suppressed.contains(*prompt_rule),
            "a healthy tool rule must suppress `{prompt_rule}` for {PROBE_PATH}"
        );
    }
    let reading_list: BTreeSet<String> = prompt_rules_for(ruleset, &suppressed)
        .into_iter()
        .map(|rule| rule.name)
        .collect();
    assert!(
        reading_list.is_empty(),
        "no agent may be given any duplication rule for {PROBE_PATH}; \
         its reading list is {reading_list:?}"
    );

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
        "one tool run answers the whole set, so it costs one process and no agent turn"
    );
    assert!(
        report.fallbacks().is_empty(),
        "a fallback would put the prompt rules back in front of an agent: {:?}",
        report.fallbacks()
    );

    assert_eq!(
        outcome.findings().len(),
        1,
        "exactly one finding must land on {PROBE_PATH}; got {:?}",
        outcome.findings()
    );
    let finding = &outcome.findings()[0];
    assert!(finding.confirmed);
    assert_eq!(finding.finding.file, PROBE_PATH);
    assert_eq!(finding.finding.validator, DUPLICATION_SET);
    assert_eq!(finding.finding.rule.as_deref(), Some(TOOL_RULE));
    assert_eq!(finding.finding.line, 24);
    assert_eq!(
        finding.finding.claim,
        "fn `mirrored_band` is a near-duplicate of `folded_band` at src/lib.rs:4 \
         (61 tokens, 100% alike)",
        "the claim must be the op's own message, word for word"
    );
}

/// Acceptance: the same pair of functions, with the marker comment on the
/// second, draws no finding from the same run.
///
/// The pass half of the pair, run through the production path rather than
/// through the doctor. Without it a rule that reported every repeated block
/// would still satisfy the test above.
#[test]
#[serial_test::serial(env)]
fn the_shipped_duplication_tool_rule_reports_nothing_on_a_marked_copy() {
    let _sah = EnvVarGuard::set(SAH_BINARY_ENV, env!("CARGO_BIN_EXE_sah"));
    let repo = probe_repository(MARKED_COPY_RS);
    let loader = builtin_loader();
    let work = duplication_work(MARKED_COPY_RS);

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
        "the marker exempts the copy; got {:?}",
        outcome.findings()
    );
}

/// Acceptance: the rule's `match` names exactly the grammar roster's
/// extensions.
///
/// The op reports nothing for a file the roster does not claim, so a `match`
/// wider than the roster would silence the `duplication` prompt rule for files
/// no tool reads. A `match` narrower than the roster would leave a language
/// the tool can decide on the prompt path. This test is what holds the two
/// lists together as the roster grows.
#[test]
fn the_shipped_duplication_tool_rule_matches_the_whole_grammar_roster() {
    let loader = builtin_loader();
    let ruleset = loader
        .get_ruleset(DUPLICATION_SET)
        .expect("duplication should be loaded");
    let rule = ruleset
        .rules
        .iter()
        .find(|rule| rule.name == TOOL_RULE)
        .unwrap_or_else(|| panic!("the shipped set must carry `{TOOL_RULE}`"));

    let matched: BTreeSet<String> = rule
        .match_criteria
        .as_ref()
        .expect("the rule declares its own match")
        .files
        .iter()
        .map(|pattern| pattern.trim_start_matches("**/*").to_string())
        .collect();
    let roster: BTreeSet<String> = get_all_code_extensions()
        .iter()
        .map(|extension| (*extension).to_string())
        .collect();

    assert_eq!(matched, roster);
}
