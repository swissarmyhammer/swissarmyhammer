//! Tool-rule planning and execution for the review engine.
//!
//! A tool rule is a rule whose frontmatter carries a `tool` block (see
//! `builtin/validators/README.md`). A language tool examines the code and
//! reports the findings — no LLM reads the code for a tool rule.
//!
//! The flow has two halves:
//!
//! - [`plan_tool_rules`] matches every tool rule against the scoped
//!   [`WorkList`] through the same `ValidatorMatch` path prompt rules use,
//!   checks the tool's health with the doctor logic
//!   ([`check_tool_rule`](crate::doctor)), and produces the [`ToolPlan`]: the
//!   healthy runs, the fallbacks (unhealthy tool → the superseded prompt rule
//!   runs as before), and the [`ToolSuppression`] map that tells the fleet
//!   which superseded prompt rules to skip per file.
//! - [`execute_tool_runs`] runs each planned script with bash at the workspace
//!   root. `scope: files` passes the matched changed files as the script's
//!   arguments (`"$@"`); `scope: workspace` runs once with no arguments and
//!   keeps only the findings in the matched changed files. Stdout is parsed by
//!   [`parse_tool_stdout`] — the only parsing the engine does. Exit 0 means the
//!   script judged the code; a nonzero exit is a [`ToolRunError`] carrying the
//!   raw stderr, and no findings are read.
//!
//! [`project_tool_rules`] is the third selection: the workspace-wide one, for
//! the surfaces that have no work-list — the doctor's tool-rule rows and the
//! `sah init` pre-install.
//!
//! Tool findings are deterministic, so they skip adversarial verification:
//! each one becomes a CONFIRMED [`VerifiedFinding`] directly.
//!
//! When a tool needs a configuration file, the rule's `run` script writes one
//! to a temporary path and passes it with a flag. The engine never changes the
//! project's own lint configuration — it has no configuration knowledge at all.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::path::Path;

use crate::doctor::{check_tool_rule, command_failure_detail, run_shell};
use crate::review::fleet::{emit_progress, ReviewProgressEvent, ReviewProgressSender};
use crate::review::scope::{ValidatorWork, WorkList};
use crate::review::tool_output::parse_tool_stdout;
use crate::review::types::{Finding, VerifiedFinding};
use crate::validators::types::{
    MatchContext, Rule, Supersedes, ToolScope, ToolSpec, ValidatorMatch,
};
use crate::validators::{RuleSet, ValidatorLoader};

/// Why a tool finding is confirmed without the adversarial verify pass.
const TOOL_FINDING_REASON: &str =
    "a deterministic tool reported this finding; tool findings skip adversarial verification";

/// The prompt rules the fleet must skip, per `(validator, file)`.
///
/// Built by [`plan_tool_rules`]: when a healthy tool rule matches a file, every
/// prompt rule its `supersedes` names is skipped for that file. When the tool
/// is missing or unhealthy nothing is suppressed — those prompt rules run as
/// before.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolSuppression(BTreeMap<String, BTreeMap<String, BTreeSet<String>>>);

impl ToolSuppression {
    /// Record that `rule` is superseded for `file` under `validator`.
    fn insert(&mut self, validator: &str, file: &str, rule: &str) {
        self.0
            .entry(validator.to_string())
            .or_default()
            .entry(file.to_string())
            .or_default()
            .insert(rule.to_string());
    }

    /// The prompt rule names suppressed for `(validator, file)`, sorted.
    ///
    /// Empty when nothing is suppressed for the pair.
    pub fn suppressed_rules(&self, validator: &str, file: &str) -> BTreeSet<String> {
        self.0
            .get(validator)
            .and_then(|files| files.get(file))
            .cloned()
            .unwrap_or_default()
    }

    /// Whether nothing at all is suppressed.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// One healthy tool rule's execution unit: run this script over these files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRun {
    /// The owning validator (RuleSet) name.
    validator: String,
    /// The tool rule's name.
    rule: String,
    /// The rule's `tool` block.
    spec: ToolSpec,
    /// The changed files this rule matched, repo-relative, in work-list order.
    files: Vec<String>,
}

impl ToolRun {
    /// The owning validator (RuleSet) name.
    pub fn validator(&self) -> &str {
        &self.validator
    }

    /// The tool rule's name.
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// The changed files this rule matched, repo-relative.
    pub fn files(&self) -> &[String] {
        &self.files
    }
}

/// A tool rule whose tool is missing or unhealthy: the superseded prompt rules
/// run as before, and the report notes the fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolFallback {
    /// The owning validator (RuleSet) name.
    validator: String,
    /// The tool rule's name.
    rule: String,
    /// The prompt rules that run instead, when the tool rule names any.
    supersedes: Supersedes,
    /// Why the tool rule is not usable.
    detail: String,
}

impl ToolFallback {
    /// The owning validator (RuleSet) name.
    pub fn validator(&self) -> &str {
        &self.validator
    }

    /// The tool rule's name.
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// The prompt rules that run instead, when the tool rule names any.
    pub fn supersedes(&self) -> &Supersedes {
        &self.supersedes
    }

    /// Why the tool rule is not usable.
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// Build a fallback for tests, mirroring [`SkippedFile::for_test`]'s
    /// pattern so synthesis tests can render one without a real tool run.
    ///
    /// [`SkippedFile::for_test`]: crate::review::scope::SkippedFile
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_test(validator: &str, rule: &str, supersedes: &[&str], detail: &str) -> Self {
        Self {
            validator: validator.to_string(),
            rule: rule.to_string(),
            supersedes: supersedes.iter().copied().collect(),
            detail: detail.to_string(),
        }
    }
}

/// A fallback is a report fact, not an error — the review still ran, through
/// the prompt rule — so it carries `Display` without `std::error::Error`.
impl std::fmt::Display for ToolFallback {
    /// The fallback as one line: which rule fell back, in which validator, and
    /// why. The prompt rule that ran instead is a separate report fact.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tool rule `{}` in validator `{}` fell back: {}",
            self.rule, self.validator, self.detail
        )
    }
}

/// A tool run that broke: a nonzero exit or stdout that violates the contract.
///
/// This is a tool error, never findings and never a clean result. The raw
/// stderr (or the parse problem) rides in `detail` so the diagnosing agent
/// reads exactly what the tool said.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("tool rule `{rule}` in validator `{validator}` broke: {detail}")]
pub struct ToolRunError {
    /// The owning validator (RuleSet) name.
    validator: String,
    /// The tool rule's name.
    rule: String,
    /// The raw stderr of the broken run, or the stdout-contract parse problem.
    detail: String,
}

impl ToolRunError {
    /// The owning validator (RuleSet) name.
    pub fn validator(&self) -> &str {
        &self.validator
    }

    /// The tool rule's name.
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// The raw stderr of the broken run, or the stdout-contract parse problem.
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// Build a tool error for tests, mirroring [`SkippedFile::for_test`]'s
    /// pattern so synthesis tests can render one without a real tool run.
    ///
    /// [`SkippedFile::for_test`]: crate::review::scope::SkippedFile
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_test(validator: &str, rule: &str, detail: &str) -> Self {
        Self {
            validator: validator.to_string(),
            rule: rule.to_string(),
            detail: detail.to_string(),
        }
    }
}

/// The planned tool-rule work for one review run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolPlan {
    /// The healthy tool rules to execute, with their matched files.
    runs: Vec<ToolRun>,
    /// The tool rules whose tool is missing or unhealthy.
    fallbacks: Vec<ToolFallback>,
    /// The superseded prompt rules the fleet must skip, per `(validator, file)`.
    suppression: ToolSuppression,
}

impl ToolPlan {
    /// The healthy tool rules to execute, with their matched files.
    pub fn runs(&self) -> &[ToolRun] {
        &self.runs
    }

    /// The tool rules whose tool is missing or unhealthy.
    pub fn fallbacks(&self) -> &[ToolFallback] {
        &self.fallbacks
    }

    /// The superseded prompt rules the fleet must skip, per `(validator, file)`.
    pub fn suppression(&self) -> &ToolSuppression {
        &self.suppression
    }

    /// Consume the plan into `(runs, fallbacks, suppression)`.
    pub fn into_parts(self) -> (Vec<ToolRun>, Vec<ToolFallback>, ToolSuppression) {
        (self.runs, self.fallbacks, self.suppression)
    }
}

/// The result of executing every planned tool run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolOutcome {
    /// Every tool finding, already CONFIRMED — tool findings skip verify.
    findings: Vec<VerifiedFinding>,
    /// Every run that broke (nonzero exit or stdout-contract violation).
    errors: Vec<ToolRunError>,
}

impl ToolOutcome {
    /// Every tool finding, already CONFIRMED.
    pub fn findings(&self) -> &[VerifiedFinding] {
        &self.findings
    }

    /// Every run that broke.
    pub fn errors(&self) -> &[ToolRunError] {
        &self.errors
    }

    /// Consume the outcome into `(findings, errors)`.
    pub fn into_parts(self) -> (Vec<VerifiedFinding>, Vec<ToolRunError>) {
        (self.findings, self.errors)
    }
}

/// The tool-rule facts synthesis renders into the report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolReport {
    /// How many tool runs were executed.
    attempted: usize,
    /// Every run that broke — rendered as a tool error, never as findings.
    errors: Vec<ToolRunError>,
    /// Every tool rule on its prompt fallback — the report notes each one.
    fallbacks: Vec<ToolFallback>,
}

impl ToolReport {
    /// Assemble the report facts from the run's plan and outcome parts.
    pub fn new(attempted: usize, errors: Vec<ToolRunError>, fallbacks: Vec<ToolFallback>) -> Self {
        Self {
            attempted,
            errors,
            fallbacks,
        }
    }

    /// How many tool runs were executed.
    pub fn attempted(&self) -> usize {
        self.attempted
    }

    /// Every run that broke.
    pub fn errors(&self) -> &[ToolRunError] {
        &self.errors
    }

    /// Every tool rule on its prompt fallback.
    pub fn fallbacks(&self) -> &[ToolFallback] {
        &self.fallbacks
    }

    /// Whether no tool rule did anything this run — nothing attempted, no
    /// errors, no fallbacks. Used to keep the "Nothing in scope to review."
    /// line honest.
    pub fn is_inert(&self) -> bool {
        self.attempted == 0 && self.errors.is_empty() && self.fallbacks.is_empty()
    }
}

/// Plan the run's tool-rule work from the scoped work-list.
///
/// Pair matching is the same `ValidatorMatch` path prompt rules use: a tool
/// rule matches the intersection of its set's `match` and its own — evaluated
/// per file with the workspace's detected `project_types`. A tool rule with no
/// matched file contributes nothing. For each tool rule with matched files the
/// doctor health check ([`check_tool_rule`](crate::doctor)) runs ONCE:
///
/// - Healthy (tool present, fixtures pass) → a [`ToolRun`], plus one
///   [`ToolSuppression`] entry per matched file for each prompt rule the
///   `supersedes` list names.
/// - Missing or unhealthy → a [`ToolFallback`]; nothing is suppressed, so the
///   superseded prompt rules run as before.
pub fn plan_tool_rules(
    work: &WorkList,
    loader: &ValidatorLoader,
    project_types: &[&str],
) -> ToolPlan {
    let mut plan = ToolPlan::default();
    for matched in matched_tool_rules(work, loader, project_types) {
        plan_rule_by_health(
            &mut plan,
            matched.ruleset.name(),
            matched.ruleset,
            matched.rule,
            matched.spec,
            matched.files,
        );
    }
    plan
}

/// One tool rule the work-list matched, with the files it matched.
pub(crate) struct MatchedToolRule<'a> {
    /// The owning validator set. The doctor reads its `fixtures/` directory,
    /// and its name is the validator name the work-list keys on.
    pub ruleset: &'a RuleSet,
    /// The matched tool rule.
    pub rule: &'a Rule,
    /// The rule's `tool` block.
    pub spec: &'a ToolSpec,
    /// The changed files this rule matched, repo-relative, in work-list order.
    pub files: Vec<String>,
}

/// Every tool rule the work-list matches, with the files each one matched.
///
/// The ONE matching pass over the work-list: [`plan_tool_rules`] plans from it
/// and [`install_missing_tools`](crate::review::tool_install::install_missing_tools)
/// installs from it, so the engine can never install a tool for a rule it will
/// not run, or run a rule it never tried to install.
///
/// A tool rule with no matched file is left out entirely.
pub(crate) fn matched_tool_rules<'a>(
    work: &WorkList,
    loader: &'a ValidatorLoader,
    project_types: &[&str],
) -> Vec<MatchedToolRule<'a>> {
    let mut matched = Vec::new();
    for validator in work.validators() {
        let Some(ruleset) = loader.get_ruleset(validator.validator_name()) else {
            continue;
        };
        for rule in &ruleset.rules {
            let Some(spec) = &rule.tool else {
                continue;
            };
            let files = matched_rule_files(validator, ruleset, rule, project_types);
            if files.is_empty() {
                continue;
            }
            matched.push(MatchedToolRule {
                ruleset,
                rule,
                spec,
                files,
            });
        }
    }
    matched
}

/// The subset of the validator's files this tool rule matches: the same
/// per-file `ValidatorMatch` intersection (set `match` ∩ rule `match`) prompt
/// rules use, evaluated with the workspace's detected `project_types`.
fn matched_rule_files(
    validator: &ValidatorWork,
    ruleset: &RuleSet,
    rule: &Rule,
    project_types: &[&str],
) -> Vec<String> {
    validator
        .files()
        .iter()
        .map(|file| file.path().to_string())
        .filter(|path| {
            let ctx = MatchContext::new()
                .with_file(path.clone())
                .with_project_types(project_types.iter().copied().map(String::from));
            rule.matches(ruleset, &ctx)
        })
        .collect()
}

/// One tool rule that serves the detected project types, with its owning set.
pub(crate) struct ProjectToolRule<'a> {
    /// The owning validator set — the doctor reads its `fixtures/` directory.
    pub ruleset: &'a RuleSet,
    /// The tool rule.
    pub rule: &'a Rule,
    /// The rule's `tool` block.
    pub spec: &'a ToolSpec,
}

/// Every tool rule the loaded sets declare for `project_types`, in set-name
/// order.
///
/// The workspace-wide counterpart of [`matched_tool_rules`]: the surfaces that
/// call it have no work-list, so a rule is selected on project type alone. It
/// contributes only when its set's `match` AND its own fit the detected project
/// types — the same intersection the review engine matches on.
///
/// The ONE selection pass over the loader, and it lives here with the rest of
/// the tool-rule utilities:
/// [`check_review_engine_with`](crate::doctor::check_review_engine_with)
/// diagnoses these rules and
/// [`install_project_tool_rules`](crate::review::tool_install::install_project_tool_rules)
/// installs their tools, so doctor can never report a rule the installer
/// skipped.
pub(crate) fn project_tool_rules<'a>(
    loader: &'a ValidatorLoader,
    project_types: &[&str],
) -> Vec<ProjectToolRule<'a>> {
    let mut matched = Vec::new();
    for ruleset in loader.list_rulesets() {
        if !ValidatorMatch::criteria_applies(
            ruleset.manifest.match_criteria.as_ref(),
            project_types,
        ) {
            continue;
        }
        for rule in &ruleset.rules {
            let Some(spec) = &rule.tool else {
                continue;
            };
            if !ValidatorMatch::criteria_applies(rule.match_criteria.as_ref(), project_types) {
                continue;
            }
            matched.push(ProjectToolRule {
                ruleset,
                rule,
                spec,
            });
        }
    }
    matched
}

/// Run the doctor health check ONCE for a matched tool rule and record the
/// verdict on the plan: a healthy rule becomes a [`ToolRun`] (plus one
/// suppression entry per matched file for each prompt rule its `supersedes`
/// names), an unhealthy one a [`ToolFallback`] that suppresses nothing.
fn plan_rule_by_health(
    plan: &mut ToolPlan,
    validator: &str,
    ruleset: &RuleSet,
    rule: &Rule,
    spec: &ToolSpec,
    files: Vec<String>,
) {
    let status = check_tool_rule(ruleset, rule, spec);
    if !status.usable() {
        let detail = status.degraded_detail();
        tracing::warn!(
            validator = %validator,
            rule = %rule.name,
            detail = %detail,
            supersedes = ?rule.supersedes,
            "tool rule is not usable; the superseded prompt rules run instead"
        );
        plan.fallbacks.push(ToolFallback {
            validator: validator.to_string(),
            rule: rule.name.clone(),
            supersedes: rule.supersedes.clone(),
            detail,
        });
        return;
    }

    for superseded in rule.supersedes.names() {
        for file in &files {
            plan.suppression.insert(validator, file, superseded);
        }
    }
    tracing::info!(
        validator = %validator,
        rule = %rule.name,
        files = ?files,
        "tool rule is healthy; the tool reviews these files instead of an LLM"
    );
    plan.runs.push(ToolRun {
        validator: validator.to_string(),
        rule: rule.name.clone(),
        spec: spec.clone(),
        files,
    });
}

/// Execute every planned tool run at the workspace root.
///
/// Findings stream on the same progress channels the fleet uses: one
/// [`ReviewProgressEvent::Planned`] for the tool pairs, a `PairStarted` /
/// `PairDone` per `(validator, file)` pair, and one
/// [`ReviewProgressEvent::Findings`] per run that judged the code. A broken
/// run (nonzero exit, or stdout that violates the contract) becomes a
/// [`ToolRunError`] — never findings, never a clean result — and its pairs
/// still reach `PairDone`.
pub fn execute_tool_runs(
    runs: &[ToolRun],
    repo_root: &Path,
    progress: Option<&ReviewProgressSender>,
) -> ToolOutcome {
    let mut outcome = ToolOutcome::default();
    if runs.is_empty() {
        return outcome;
    }

    let total_pairs = runs.iter().map(|run| run.files.len()).sum();
    emit_progress(progress, ReviewProgressEvent::Planned { total_pairs });

    for run in runs {
        for file in &run.files {
            emit_progress(
                progress,
                ReviewProgressEvent::PairStarted {
                    validator: run.validator.clone(),
                    file: file.clone(),
                },
            );
        }

        match run_tool_script(run, repo_root) {
            Ok(findings) => {
                tracing::info!(
                    validator = %run.validator,
                    rule = %run.rule,
                    findings = findings.len(),
                    "tool run judged the code"
                );
                emit_progress(
                    progress,
                    ReviewProgressEvent::Findings {
                        validator: run.validator.clone(),
                        findings: findings.clone(),
                    },
                );
                outcome.findings.extend(findings.into_iter().map(confirm));
            }
            Err(detail) => {
                tracing::warn!(
                    validator = %run.validator,
                    rule = %run.rule,
                    detail = %detail,
                    "tool run broke; reporting a tool error and reading no findings"
                );
                outcome.errors.push(ToolRunError {
                    validator: run.validator.clone(),
                    rule: run.rule.clone(),
                    detail,
                });
            }
        }

        for file in &run.files {
            emit_progress(
                progress,
                ReviewProgressEvent::PairDone {
                    validator: run.validator.clone(),
                    file: file.clone(),
                },
            );
        }
    }
    outcome
}

/// Run one tool rule's script and parse its stdout into tagged findings.
///
/// The script runs with bash at `repo_root`. `scope: files` passes the run's
/// matched files as the script's arguments; `scope: workspace` passes none and
/// keeps only the findings in the run's matched files afterwards. Exit 0 means
/// the script judged the code; a nonzero exit (or stdout that violates the
/// contract) is the error string — the raw stderr, or the parse problem.
fn run_tool_script(run: &ToolRun, repo_root: &Path) -> Result<Vec<Finding>, String> {
    let args: Vec<&OsStr> = match run.spec.scope {
        ToolScope::Files => run.files.iter().map(OsStr::new).collect(),
        ToolScope::Workspace => Vec::new(),
    };

    let output = run_shell(&run.spec.run, Some(repo_root), &args)
        .map_err(|e| format!("the run script failed to start: {e}"))?;
    if !output.status.success() {
        return Err(command_failure_detail(&output));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut findings = parse_tool_stdout(&stdout).map_err(|e| e.to_string())?;

    if run.spec.scope == ToolScope::Workspace {
        let matched: BTreeSet<&str> = run.files.iter().map(String::as_str).collect();
        findings.retain(|finding| {
            matched.contains(normalize_tool_path(&finding.file, repo_root).as_str())
        });
    }

    for finding in &mut findings {
        finding.file = normalize_tool_path(&finding.file, repo_root);
        finding.validator = run.validator.clone();
        finding.rule = Some(run.rule.clone());
    }
    Ok(findings)
}

/// Normalize a tool-reported path onto the repo-relative form the work-list
/// uses: strip the workspace root from an absolute path and a leading `./`.
///
/// Crate-visible so the doctor's fixture checks
/// ([`crate::doctor`]) attribute a tool-reported path to a fixture with the
/// same normalization the engine attributes it to a changed file with — a path
/// can never mean two things.
pub(crate) fn normalize_tool_path(path: &str, repo_root: &Path) -> String {
    let p = Path::new(path);
    let relative = p.strip_prefix(repo_root).unwrap_or(p);
    let text = relative.to_string_lossy();
    text.strip_prefix("./").unwrap_or(&text).to_string()
}

/// Wrap one tool finding as CONFIRMED — deterministic tool output needs no
/// adversarial verification.
fn confirm(finding: Finding) -> VerifiedFinding {
    VerifiedFinding {
        finding,
        confirmed: true,
        reason: TOOL_FINDING_REASON.to_string(),
        decided_by: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use crate::doctor::ToolPresence;
    use crate::review::scope::{FileWork, ProbeNames, RuleNames};
    use crate::review::test_support::write_tool_rule_fixtures;
    use crate::validators::types::{Rule, RuleSet, ToolDoctor, ToolInstall, ValidatorMatch};

    /// A shell probe that always succeeds — "the tool is installed".
    const TOOL_PRESENT: &str = "true";

    /// A shell probe that always fails — "the tool is missing".
    const TOOL_MISSING: &str = "false";

    /// A `files`-scope script that reports one `path:line: message` finding
    /// per line containing `TODO`, and exits 0 whether or not it found any.
    const TODO_SCRIPT: &str = r#"for f in "$@"; do awk -v f="$f" '/TODO/ { print f ":" NR ": TODO left in code" }' "$f"; done"#;

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

    /// A run over `script` with `scope` and `files`, for the execute tests.
    fn run_of(script: &str, scope: ToolScope, files: &[&str]) -> ToolRun {
        ToolRun {
            validator: "docs".to_string(),
            rule: "docs-tool".to_string(),
            spec: ToolSpec {
                scope,
                run: script.to_string(),
                doctor: None,
                install: None,
            },
            files: files.iter().map(|f| f.to_string()).collect(),
        }
    }

    #[test]
    fn execute_passes_the_changed_files_as_arguments_and_tags_the_findings() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join("src")).unwrap();
        std::fs::write(repo.path().join("src/lib.rs"), "fn a() {}\n// TODO: fix\n").unwrap();
        let run = run_of(TODO_SCRIPT, ToolScope::Files, &["src/lib.rs"]);

        let outcome = execute_tool_runs(&[run], repo.path(), None);

        assert!(outcome.errors().is_empty());
        assert_eq!(outcome.findings().len(), 1);
        let verified = &outcome.findings()[0];
        assert!(
            verified.confirmed,
            "tool findings are confirmed by construction"
        );
        assert_eq!(verified.finding.file, "src/lib.rs");
        assert_eq!(verified.finding.line, 2);
        assert_eq!(verified.finding.validator, "docs");
        assert_eq!(verified.finding.rule.as_deref(), Some("docs-tool"));
    }

    #[test]
    fn execute_keeps_only_matched_file_findings_for_a_workspace_scope_run() {
        let repo = tempfile::tempdir().unwrap();
        // The script reports findings in a matched and an unmatched file.
        let script = r#"printf 'src/lib.rs:1: in scope\n./src/lib.rs:2: dot slash in scope\nsrc/unrelated.rs:1: out of scope\n'"#;
        let run = run_of(script, ToolScope::Workspace, &["src/lib.rs"]);

        let outcome = execute_tool_runs(&[run], repo.path(), None);

        assert!(outcome.errors().is_empty());
        let files: Vec<&str> = outcome
            .findings()
            .iter()
            .map(|v| v.finding.file.as_str())
            .collect();
        assert_eq!(files, ["src/lib.rs", "src/lib.rs"]);
    }

    #[test]
    fn execute_reports_a_nonzero_exit_as_a_tool_error_with_the_raw_stderr() {
        let repo = tempfile::tempdir().unwrap();
        // The script prints a well-formed finding line but exits nonzero: the
        // exit code wins — a tool error, no findings read.
        let script =
            r#"echo "src/lib.rs:1: would-be finding"; echo "the linter exploded" >&2; exit 3"#;
        let run = run_of(script, ToolScope::Files, &["src/lib.rs"]);

        let outcome = execute_tool_runs(&[run], repo.path(), None);

        assert!(outcome.findings().is_empty());
        assert_eq!(outcome.errors().len(), 1);
        assert_eq!(outcome.errors()[0].validator(), "docs");
        assert_eq!(outcome.errors()[0].rule(), "docs-tool");
        assert!(outcome.errors()[0].detail().contains("the linter exploded"));
    }

    #[test]
    fn execute_reports_contract_breaking_stdout_as_a_tool_error() {
        let repo = tempfile::tempdir().unwrap();
        let script = r#"echo "this is not a finding line""#;
        let run = run_of(script, ToolScope::Files, &["src/lib.rs"]);

        let outcome = execute_tool_runs(&[run], repo.path(), None);

        assert!(outcome.findings().is_empty());
        assert_eq!(outcome.errors().len(), 1);
        assert!(outcome.errors()[0]
            .detail()
            .contains("this is not a finding line"));
    }

    #[test]
    fn a_tool_run_error_displays_the_rule_the_validator_and_the_detail() {
        let error = ToolRunError::for_test("docs", "docs-tool", "the linter exploded");

        assert_eq!(
            error.to_string(),
            "tool rule `docs-tool` in validator `docs` broke: the linter exploded"
        );
    }

    #[test]
    fn a_tool_run_error_is_a_standard_error() {
        let error = ToolRunError::for_test("docs", "docs-tool", "the linter exploded");
        let standard: &dyn std::error::Error = &error;

        assert_eq!(standard.to_string(), error.to_string());
    }

    #[test]
    fn a_tool_fallback_displays_the_rule_the_validator_and_the_detail() {
        let fallback = ToolFallback::for_test(
            "docs",
            "docs-tool",
            &["missing-docs"],
            "the tool is not installed",
        );

        assert_eq!(
            fallback.to_string(),
            "tool rule `docs-tool` in validator `docs` fell back: the tool is not installed"
        );
    }

    #[test]
    fn execute_streams_planned_pair_and_findings_events() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join("src")).unwrap();
        std::fs::write(repo.path().join("src/lib.rs"), "// TODO: fix\n").unwrap();
        let run = run_of(TODO_SCRIPT, ToolScope::Files, &["src/lib.rs"]);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        execute_tool_runs(&[run], repo.path(), Some(&tx));

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        assert!(matches!(
            events[0],
            ReviewProgressEvent::Planned { total_pairs: 1 }
        ));
        assert!(
            matches!(&events[1], ReviewProgressEvent::PairStarted { validator, file }
            if validator == "docs" && file == "src/lib.rs")
        );
        assert!(
            matches!(&events[2], ReviewProgressEvent::Findings { validator, findings }
            if validator == "docs" && findings.len() == 1)
        );
        assert!(
            matches!(&events[3], ReviewProgressEvent::PairDone { validator, file }
            if validator == "docs" && file == "src/lib.rs")
        );
        assert_eq!(events.len(), 4);
    }

    /// The builtin `code-hygiene` set, the one that carries the shipped
    /// missing-docs tool rules.
    const CODE_HYGIENE_SET: &str = "code-hygiene";

    /// The prompt rule every shipped missing-docs tool rule supersedes.
    const MISSING_DOCS_PROMPT_RULE: &str = "missing-docs";

    /// A second prompt rule name, for the tool rule that supersedes two.
    const FUNCTION_LENGTH_PROMPT_RULE: &str = "function-length";

    /// The shipped missing-docs tool rule for Rust, the one the pipeline
    /// acceptance test drives end to end.
    const RUST_MISSING_DOCS_RULE: &str = "missing-docs-rust";

    /// Every shipped missing-docs tool rule, with the project type it serves.
    const SHIPPED_MISSING_DOCS_RULES: &[(&str, &str)] = &[
        ("rust", RUST_MISSING_DOCS_RULE),
        ("python", "missing-docs-python"),
        ("nodejs", "missing-docs-typescript"),
        ("go", "missing-docs-go"),
        ("swift", "missing-docs-swift"),
        ("flutter", "missing-docs-dart"),
    ];

    /// The prompt rule the dead-code tool rules run beside, never in place of.
    const DEAD_CODE_PROMPT_RULE: &str = "dead-code";

    /// The shipped dead-code tool rule for Python, the one the pipeline
    /// acceptance test drives end to end.
    const PYTHON_UNREACHABLE_CODE_RULE: &str = "unreachable-code-python";

    /// Every shipped dead-code tool rule, with the project type it serves.
    ///
    /// These rules supersede nothing. The `dead-code` prompt rule keeps its
    /// carve-outs — entry points, exported public API, work-in-process
    /// scaffolding — because those need judgment. Each tool rule here adds one
    /// deterministic check beside that rule, never in place of it.
    const SHIPPED_DEAD_CODE_RULES: &[(&str, &str)] = &[
        ("go", "unused-code-go"),
        ("python", PYTHON_UNREACHABLE_CODE_RULE),
    ];

    /// A cargo package holding one undocumented public item and one documented
    /// one. `[workspace]` keeps cargo inside the temporary directory.
    const UNDOCUMENTED_PACKAGE_MANIFEST: &str = concat!(
        "[package]\nname = \"undocumented-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        "\n[workspace]\n",
    );

    /// The library of [`UNDOCUMENTED_PACKAGE_MANIFEST`]. The undocumented
    /// struct is the finding the Rust tool rule must report.
    const UNDOCUMENTED_LIB_RS: &str = concat!(
        "//! A probe crate for the shipped Rust missing-docs tool rule.\n\n",
        "/// A documented public struct.\n",
        "pub struct Documented;\n\n",
        "pub struct Undocumented;\n",
    );

    /// The library path inside the probe package, as the work-list holds it.
    const UNDOCUMENTED_LIB_PATH: &str = "src/lib.rs";

    /// A loader carrying every shipped validator set.
    fn builtin_loader() -> ValidatorLoader {
        let mut loader = ValidatorLoader::new();
        crate::load_builtins(&mut loader);
        loader
    }

    /// A one-validator work-list over `files` for the builtin `code-hygiene`
    /// set, naming both the prompt rule and the Rust tool rule.
    fn code_hygiene_work(files: &[&str]) -> WorkList {
        let file_work = files
            .iter()
            .map(|path| FileWork::new(*path, vec![], vec![], UNDOCUMENTED_LIB_RS, vec![]));
        WorkList::new(
            "an undocumented public item",
            vec![ValidatorWork::new(
                CODE_HYGIENE_SET,
                RuleNames::new([
                    MISSING_DOCS_PROMPT_RULE.to_string(),
                    RUST_MISSING_DOCS_RULE.to_string(),
                ]),
                ProbeNames::new([]),
                file_work,
            )],
        )
    }

    /// Executes `run` over `repo_root` and holds it to the report contract every
    /// shipped tool rule keeps: the pipeline breaks nothing, and it reports
    /// exactly one finding in `path` — confirmed, attributed to the
    /// `code-hygiene` set and to `rule`, carrying `claim_fragment` of the tool's
    /// own message.
    ///
    /// This is the half every shipped-rule acceptance test shares. The half
    /// above it — the probe repository, the work-list, and what the plan must
    /// suppress — differs per rule and stays in the test.
    fn verify_run_reports_one_finding(
        run: &ToolRun,
        repo_root: &Path,
        path: &str,
        rule: &str,
        claim_fragment: &str,
    ) {
        let outcome = execute_tool_runs(std::slice::from_ref(run), repo_root, None);

        assert!(
            outcome.errors().is_empty(),
            "the shipped pipeline must not break; errors: {:?}",
            outcome.errors()
        );
        let findings: Vec<&VerifiedFinding> = outcome
            .findings()
            .iter()
            .filter(|verified| verified.finding.file == path)
            .collect();
        assert_eq!(
            findings.len(),
            1,
            "exactly one finding must be reported in {path}; got {:?}",
            outcome.findings()
        );
        assert!(findings[0].confirmed);
        assert_eq!(findings[0].finding.validator, CODE_HYGIENE_SET);
        assert_eq!(findings[0].finding.rule.as_deref(), Some(rule));
        assert!(
            findings[0].finding.claim.contains(claim_fragment),
            "the claim must be the tool's message carrying '{claim_fragment}'; got '{}'",
            findings[0].finding.claim
        );
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
        let work = code_hygiene_work(&[UNDOCUMENTED_LIB_PATH]);

        let plan = plan_tool_rules(&work, &loader, &["rust"]);

        let run = plan
            .runs()
            .iter()
            .find(|run| run.rule() == RUST_MISSING_DOCS_RULE)
            .unwrap_or_else(|| {
                panic!(
                    "the shipped Rust tool rule must plan a run; fallbacks: {:?}",
                    plan.fallbacks()
                )
            });
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
            RUST_MISSING_DOCS_RULE,
            "missing documentation",
        );
    }

    /// A Python module with one statement stranded behind a `return`.
    const UNREACHABLE_MODULE_PY: &str = concat!(
        "\"\"\"A probe module for the shipped Python unreachable-code tool rule.\"\"\"\n\n\n",
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
                    PYTHON_UNREACHABLE_CODE_RULE.to_string(),
                ]),
                ProbeNames::new([]),
                [FileWork::new(path, vec![], vec![], content, vec![])],
            )],
        )
    }

    /// Acceptance: the shipped Python dead-code tool rule never suppresses the
    /// `dead-code` prompt rule, and reports the stranded statement through the
    /// real vulture pipeline.
    ///
    /// The suppression half is asserted unconditionally, because it is the
    /// regression that matters most and it does not depend on the tool being
    /// installed: a healthy tool rule suppresses whatever its `supersedes`
    /// names, and this rule must name nothing. Were it to name `dead-code`, the
    /// prompt rule would stop reading these files and its carve-outs for entry
    /// points, exported public API, and work-in-process scaffolding would go
    /// with it.
    ///
    /// The reporting half runs only when the tool is installed, the same
    /// condition [`every_shipped_dead_code_tool_rule_passes_its_fixtures`]
    /// applies: a missing tool degrades the rule and never blocks a review.
    #[test]
    fn the_shipped_python_dead_code_tool_rule_reports_without_suppressing_dead_code() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join("src")).unwrap();
        std::fs::write(
            repo.path().join(UNREACHABLE_MODULE_PATH),
            UNREACHABLE_MODULE_PY,
        )
        .unwrap();
        let loader = builtin_loader();
        let work = dead_code_work(UNREACHABLE_MODULE_PATH, UNREACHABLE_MODULE_PY);

        let plan = plan_tool_rules(&work, &loader, &["python"]);

        assert!(
            !plan
                .suppression()
                .suppressed_rules(CODE_HYGIENE_SET, UNREACHABLE_MODULE_PATH)
                .contains(DEAD_CODE_PROMPT_RULE),
            "a dead-code tool rule must never suppress the `dead-code` prompt rule, \
             or its carve-outs stop protecting staged work"
        );

        let Some(run) = plan
            .runs()
            .iter()
            .find(|run| run.rule() == PYTHON_UNREACHABLE_CODE_RULE)
        else {
            return;
        };
        assert_eq!(run.files(), [UNREACHABLE_MODULE_PATH.to_string()]);

        verify_run_reports_one_finding(
            run,
            repo.path(),
            UNREACHABLE_MODULE_PATH,
            PYTHON_UNREACHABLE_CODE_RULE,
            "unreachable code after",
        );
    }

    /// Drives every rule in `rules` through the real pre-install and doctor
    /// path, and holds each one to the fixture contract.
    ///
    /// Each pair names a project type and the tool rule that serves it. For
    /// each, the helper runs the same pre-install `sah init` runs, reads the
    /// doctor row, and asserts the row supersedes `expected_supersedes` —
    /// `None` for a rule that must leave its prompt rule running.
    ///
    /// A rule whose tool the machine does not have — and whose install commands
    /// could not get it — is reported as degraded, which is the documented
    /// behavior: a missing tool falls the rule back to its prompt rule and never
    /// blocks a review. That state cannot run the fixtures, so the fixture
    /// assertion applies to the rules whose tool doctor found. The exercised
    /// count guards against every rule taking that branch and the caller
    /// asserting nothing.
    ///
    /// `rule_kind` names the group in the failure messages — the prompt rule the
    /// group is named for, whether the group replaces that rule or runs beside
    /// it — so a failing run says which roster came up empty.
    fn verify_shipped_tool_rules_pass_fixtures(
        rules: &[(&str, &str)],
        expected_supersedes: &[&str],
        rule_kind: &str,
    ) {
        let loader = builtin_loader();
        let mut exercised = 0;
        let expected_label = match expected_supersedes.is_empty() {
            true => "nothing".to_string(),
            false => expected_supersedes.join(", "),
        };

        for (project_type, rule_name) in rules {
            let project_types = [*project_type];
            crate::review::tool_install::install_project_tool_rules(&loader, &project_types);

            let status = crate::doctor::check_review_engine_with(&loader, &project_types);
            let row = status
                .tool_rules
                .iter()
                .find(|row| row.rule_name == *rule_name)
                .unwrap_or_else(|| {
                    panic!("{rule_name} must be reported for a {project_type} project")
                });
            assert_eq!(
                row.supersedes.names(),
                expected_supersedes,
                "{rule_name} must supersede {expected_label}, the contract every {rule_kind} \
                 tool rule keeps"
            );
            if row.presence == ToolPresence::Present {
                assert!(
                    row.usable(),
                    "{rule_name}'s tool is installed, so its fixtures must pass; \
                     doctor says: {}",
                    row.degraded_detail()
                );
                exercised += 1;
            }
        }

        assert!(
            exercised > 0,
            "no shipped {rule_kind} tool rule's tool was installed, so the fixture \
             pairs were never run and this test asserts nothing"
        );
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
        verify_shipped_tool_rules_pass_fixtures(
            SHIPPED_MISSING_DOCS_RULES,
            &[MISSING_DOCS_PROMPT_RULE],
            MISSING_DOCS_PROMPT_RULE,
        );
    }

    /// Acceptance: every shipped dead-code tool rule passes its fixture pair in
    /// doctor, and supersedes nothing.
    ///
    /// The `supersedes` assertion is the load-bearing half. A dead-code tool
    /// reads the code without the `callers` probe, so it cannot see the
    /// `dead-code` prompt rule's carve-outs and would report staged work as
    /// dead. Naming that prompt rule in `supersedes` would silence it for the
    /// files the tool covers, which is exactly the regression to prevent.
    #[test]
    fn every_shipped_dead_code_tool_rule_passes_its_fixtures() {
        verify_shipped_tool_rules_pass_fixtures(
            SHIPPED_DEAD_CODE_RULES,
            &[],
            DEAD_CODE_PROMPT_RULE,
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
}
