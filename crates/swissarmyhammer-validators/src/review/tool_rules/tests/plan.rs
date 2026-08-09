use super::*;

use std::path::PathBuf;

use crate::review::scope::{FileWork, ProbeNames, RuleNames};
use crate::review::test_support::write_tool_rule_fixtures;
use crate::validators::types::{Rule, RuleSet, ToolDoctor, ToolInstall, ValidatorMatch};

const TOOL_PRESENT: &str = "true";

/// A shell probe that always fails — "the tool is missing".
const TOOL_MISSING: &str = "false";

/// The placeholder install command the planner tests never run — they only
/// assert on the plan, never on the install lifecycle.
const UNUSED_INSTALL_COMMAND: &str = "brew install fake-tool@1.0.0";

/// A tool rule named `docs-tool` superseding `missing-docs`, with the
/// given run script and doctor check command.
fn tool_rule(run: &str, check_command: &str, match_criteria: Option<ValidatorMatch>) -> Rule {
    tool_rule_with_install(
        run,
        check_command,
        match_criteria,
        vec![UNUSED_INSTALL_COMMAND.to_string()],
    )
}

/// A tool rule named `docs-tool` superseding `missing-docs`, with the given
/// run script, doctor check command, and install commands.
fn tool_rule_with_install(
    run: &str,
    check_command: &str,
    match_criteria: Option<ValidatorMatch>,
    install_commands: Vec<String>,
) -> Rule {
    Rule {
        name: "docs-tool".to_string(),
        description: "docs by tool".to_string(),
        body: "TOOL RULE BODY — an LLM must never read this".to_string(),
        supersedes: Supersedes::from_iter(["missing-docs"]),
        match_criteria,
        tool: Some(ToolSpec {
            scope: ToolScope::Files,
            run: run.to_string(),
            doctor: Some(ToolDoctor {
                check_command: check_command.to_string(),
                check_version_command: None,
                fix_hint: None,
            }),
            install: Some(ToolInstall {
                commands: install_commands,
            }),
        }),
        ..Rule::default()
    }
}

/// The prompt rule the tool rule supersedes.
fn prompt_rule() -> Rule {
    Rule {
        name: "missing-docs".to_string(),
        description: "docs by prompt".to_string(),
        body: "Report public items without docs.".to_string(),
        ..Rule::default()
    }
}

/// A ruleset named `docs` matching `*.rs`, holding the given rules, based
/// at `base` (where the doctor looks for `fixtures/`).
fn docs_ruleset(base: &Path, rules: Vec<Rule>) -> RuleSet {
    let mut ruleset = crate::review::test_support::ruleset("docs", "*.rs", &[]);
    ruleset.rules = rules;
    ruleset.base_path = PathBuf::from(base);
    ruleset
}

/// A loader holding exactly `ruleset`.
fn loader_of(ruleset: RuleSet) -> ValidatorLoader {
    let mut loader = ValidatorLoader::new();
    loader.add_builtin_ruleset(ruleset);
    loader
}

/// A one-validator work-list over `files` for the `docs` validator.
fn docs_work(files: &[&str]) -> WorkList {
    let file_work = files
        .iter()
        .map(|path| FileWork::new(*path, vec![], vec![], "fn undocumented() {}\n", vec![]));
    WorkList::new(
        "test change",
        vec![ValidatorWork::new(
            "docs",
            RuleNames::new(["missing-docs".to_string(), "docs-tool".to_string()]),
            ProbeNames::new([]),
            file_work,
        )],
    )
}

#[test]
fn plan_includes_a_healthy_tool_rule_and_suppresses_the_superseded_rule_per_file() {
    let base = tempfile::tempdir().unwrap();
    write_tool_rule_fixtures(base.path(), "docs-tool");
    let loader = loader_of(docs_ruleset(
        base.path(),
        vec![prompt_rule(), tool_rule(TODO_SCRIPT, TOOL_PRESENT, None)],
    ));
    let work = docs_work(&["src/lib.rs"]);

    let plan = plan_tool_rules(&work, &loader, &[]);

    assert_eq!(plan.runs().len(), 1);
    assert_eq!(plan.runs()[0].validator(), "docs");
    assert_eq!(plan.runs()[0].rule(), "docs-tool");
    assert_eq!(plan.runs()[0].files(), ["src/lib.rs".to_string()]);
    assert!(plan.fallbacks().is_empty());
    assert!(plan
        .suppression()
        .suppressed_rules("docs", "src/lib.rs")
        .contains("missing-docs"));
}

/// A healthy tool rule that names two prompt rules suppresses BOTH of them
/// for every file it matched: one `cargo clippy` run answers more than one
/// prompt rule, so one entry per named rule per file is the contract.
#[test]
fn plan_suppresses_every_named_prompt_rule_per_file() {
    let base = tempfile::tempdir().unwrap();
    write_tool_rule_fixtures(base.path(), "docs-tool");
    let mut rule = tool_rule(TODO_SCRIPT, TOOL_PRESENT, None);
    rule.supersedes =
        Supersedes::from_iter([MISSING_DOCS_PROMPT_RULE, FUNCTION_LENGTH_PROMPT_RULE]);
    let loader = loader_of(docs_ruleset(base.path(), vec![prompt_rule(), rule]));
    let files = ["src/lib.rs", "src/main.rs"];
    let work = docs_work(&files);

    let plan = plan_tool_rules(&work, &loader, &[]);

    let expected = BTreeSet::from([
        MISSING_DOCS_PROMPT_RULE.to_string(),
        FUNCTION_LENGTH_PROMPT_RULE.to_string(),
    ]);
    for file in files {
        assert_eq!(
            plan.suppression().suppressed_rules("docs", file),
            expected,
            "both named prompt rules must be suppressed for {file}"
        );
    }
}

#[test]
fn plan_reports_a_fallback_when_the_tool_is_missing_and_suppresses_nothing() {
    let base = tempfile::tempdir().unwrap();
    write_tool_rule_fixtures(base.path(), "docs-tool");
    let loader = loader_of(docs_ruleset(
        base.path(),
        vec![prompt_rule(), tool_rule(TODO_SCRIPT, TOOL_MISSING, None)],
    ));
    let work = docs_work(&["src/lib.rs"]);

    let plan = plan_tool_rules(&work, &loader, &[]);

    assert!(plan.runs().is_empty());
    assert_eq!(plan.fallbacks().len(), 1);
    assert_eq!(plan.fallbacks()[0].rule(), "docs-tool");
    assert_eq!(plan.fallbacks()[0].supersedes().names(), ["missing-docs"]);
    assert!(!plan.fallbacks()[0].detail().is_empty());
    assert!(plan.suppression().is_empty());
}

#[test]
fn plan_reports_a_fallback_when_the_fixtures_are_missing() {
    let base = tempfile::tempdir().unwrap();
    // No fixtures written: the rule cannot be proven healthy.
    let loader = loader_of(docs_ruleset(
        base.path(),
        vec![prompt_rule(), tool_rule(TODO_SCRIPT, TOOL_PRESENT, None)],
    ));
    let work = docs_work(&["src/lib.rs"]);

    let plan = plan_tool_rules(&work, &loader, &[]);

    assert!(plan.runs().is_empty());
    assert_eq!(plan.fallbacks().len(), 1);
    assert!(plan.suppression().is_empty());
}

#[test]
fn plan_narrows_a_tool_rule_to_the_files_its_own_match_covers() {
    let base = tempfile::tempdir().unwrap();
    write_tool_rule_fixtures(base.path(), "docs-tool");
    // The set matches *.rs; the rule narrows to src/covered.rs only.
    let narrowed = ValidatorMatch {
        files: vec!["src/covered.rs".to_string()],
        ..ValidatorMatch::default()
    };
    let loader = loader_of(docs_ruleset(
        base.path(),
        vec![
            prompt_rule(),
            tool_rule(TODO_SCRIPT, TOOL_PRESENT, Some(narrowed)),
        ],
    ));
    let work = docs_work(&["src/covered.rs", "src/other.rs"]);

    let plan = plan_tool_rules(&work, &loader, &[]);

    assert_eq!(plan.runs().len(), 1);
    assert_eq!(plan.runs()[0].files(), ["src/covered.rs".to_string()]);
    assert!(plan
        .suppression()
        .suppressed_rules("docs", "src/covered.rs")
        .contains("missing-docs"));
    assert!(plan
        .suppression()
        .suppressed_rules("docs", "src/other.rs")
        .is_empty());
}

/// The workspace-wide selection reports its rules in set-name order, and
/// that order does not depend on the order the sets were loaded in. It is
/// the order the doctor rows and the `sah init` pre-install both read, so
/// nothing along the way re-sorts.
#[test]
fn project_tool_rules_reports_the_sets_in_name_order() {
    let base = tempfile::tempdir().unwrap();
    let mut loader = ValidatorLoader::new();
    // Loaded last-name-first, so load order is not name order.
    for name in ["zeta-set", "alpha-set"] {
        let mut ruleset = crate::review::test_support::ruleset(name, "*.rs", &[]);
        ruleset.rules = vec![tool_rule(TODO_SCRIPT, TOOL_PRESENT, None)];
        ruleset.base_path = PathBuf::from(base.path());
        loader.add_builtin_ruleset(ruleset);
    }

    let selected = project_tool_rules(&loader, &[]);

    let sets: Vec<&str> = selected
        .iter()
        .map(|selected| selected.ruleset.name())
        .collect();
    assert_eq!(sets, ["alpha-set", "zeta-set"]);
}

/// A doctor check that passes only once `marker` exists — a missing tool
/// that an install command can make present.
fn marker_check_command(marker: &Path) -> String {
    format!("test -f '{}'", marker.display())
}

/// An install command that creates `marker`, standing in for a real one.
fn marker_install_command(marker: &Path) -> String {
    format!("touch '{}'", marker.display())
}

/// Acceptance: with the tool absent and a working install command, the
/// engine installs it and then plans the runner over the changed files.
#[tokio::test]
async fn a_missing_tool_with_a_working_install_command_is_installed_and_then_planned() {
    let base = tempfile::tempdir().unwrap();
    write_tool_rule_fixtures(base.path(), "docs-tool");
    let marker = base.path().join("installed-tool");
    let loader = loader_of(docs_ruleset(
        base.path(),
        vec![
            prompt_rule(),
            tool_rule_with_install(
                TODO_SCRIPT,
                &marker_check_command(&marker),
                None,
                vec![marker_install_command(&marker)],
            ),
        ],
    ));
    let work = docs_work(&["src/lib.rs"]);

    // Before the install stage the tool is missing, so the rule falls back.
    let before = plan_tool_rules(&work, &loader, &[]);
    assert!(before.runs().is_empty());
    assert_eq!(before.fallbacks().len(), 1);

    let installs =
        crate::review::tool_install::install_missing_tools(&work, &loader, &[], None).await;

    assert_eq!(installs.len(), 1);
    assert_eq!(installs[0].set_name(), "docs");
    assert_eq!(installs[0].rule_name(), "docs-tool");
    assert!(
        installs[0].outcome().tool_present(),
        "the install command must make the doctor check pass; got {:?}",
        installs[0].outcome()
    );

    // The planner re-runs the same doctor check, so the rule is now healthy.
    let after = plan_tool_rules(&work, &loader, &[]);
    assert_eq!(after.runs().len(), 1, "the installed tool must be planned");
    assert_eq!(after.runs()[0].rule(), "docs-tool");
    assert!(after.fallbacks().is_empty());
    assert!(after
        .suppression()
        .suppressed_rules("docs", "src/lib.rs")
        .contains("missing-docs"));
}

/// Acceptance: with every install command failing, the run completes on the
/// prompt fallback — the missing tool degrades the review, never blocks it.
#[tokio::test]
async fn a_missing_tool_whose_installs_all_fail_stays_on_the_prompt_fallback() {
    let base = tempfile::tempdir().unwrap();
    write_tool_rule_fixtures(base.path(), "docs-tool");
    let marker = base.path().join("never-installed");
    let loader = loader_of(docs_ruleset(
        base.path(),
        vec![
            prompt_rule(),
            tool_rule_with_install(
                TODO_SCRIPT,
                &marker_check_command(&marker),
                None,
                vec!["echo 'no such package' >&2; exit 1".to_string()],
            ),
        ],
    ));
    let work = docs_work(&["src/lib.rs"]);

    let installs =
        crate::review::tool_install::install_missing_tools(&work, &loader, &[], None).await;

    assert_eq!(installs.len(), 1);
    assert!(
        !installs[0].outcome().tool_present(),
        "every install command failed, so the tool stays missing"
    );

    let plan = plan_tool_rules(&work, &loader, &[]);
    assert!(plan.runs().is_empty());
    assert_eq!(plan.fallbacks().len(), 1);
    assert_eq!(plan.fallbacks()[0].supersedes().names(), ["missing-docs"]);
    assert!(
        plan.suppression().is_empty(),
        "the superseded prompt rule must still run for every file"
    );
}
