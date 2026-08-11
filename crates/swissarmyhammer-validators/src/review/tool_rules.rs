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
//!   ([`tool_rule_health`](crate::review::tool_health)), and produces the
//!   [`ToolPlan`]: the healthy runs, the fallbacks (unhealthy tool → the
//!   superseded prompt rule runs as before), and the [`ToolSuppression`] map
//!   that tells the fleet which superseded prompt rules to skip per file.
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

use swissarmyhammer_common::command::command_failure_detail;

use crate::doctor::run_shell;
use crate::review::fleet::{emit_progress, ReviewProgressEvent, ReviewProgressSender};
use crate::review::scope::{ValidatorWork, WorkList};
use crate::review::tool_health::{tool_rule_health, HealthProof, ToolHealthCache};
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
/// health check ([`tool_rule_health`](crate::review::tool_health)) runs ONCE,
/// reading `health` for the fixture verdict it already proved:
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
    health: Option<&ToolHealthCache>,
) -> ToolPlan {
    let mut plan = ToolPlan::default();
    for matched in matched_tool_rules(work, loader, project_types) {
        plan_rule_by_health(
            &mut plan,
            matched.ruleset,
            matched.rule,
            matched.spec,
            matched.files,
            health,
        );
    }
    plan
}

/// One tool rule the work-list matched, with the files it matched.
#[derive(Debug)]
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
#[derive(Debug)]
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

/// Run the health check ONCE for a matched tool rule and record the
/// verdict on the plan: a healthy rule becomes a [`ToolRun`] (plus one
/// suppression entry per matched file for each prompt rule its `supersedes`
/// names), an unhealthy one a [`ToolFallback`] that suppresses nothing.
///
/// The check reads presence and version fresh and takes the fixture verdict
/// `health` stored, so a review that changed neither the tool nor the rule
/// costs no fixture run.
fn plan_rule_by_health(
    plan: &mut ToolPlan,
    ruleset: &RuleSet,
    rule: &Rule,
    spec: &ToolSpec,
    files: Vec<String>,
    health: Option<&ToolHealthCache>,
) {
    let validator = ruleset.name();
    let status = tool_rule_health(health, HealthProof::Stored, ruleset, rule, spec);
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

/// Start every planned tool run and hand back a handle to its outcome.
///
/// The fan-out needs only the plan's suppression map, and that is already
/// decided by the time the runs start; the tool findings are needed at
/// synthesis and nowhere earlier. Starting the runs here therefore lets a
/// `cargo clippy` that takes a minute overlap the fleet instead of delaying
/// its first task.
///
/// The `run` scripts are blocking shell processes, so they run on the blocking
/// pool. A tool script that ran on an async task would hold a runtime worker
/// thread for its whole run and stall the fleet it is meant to overlap.
pub fn start_tool_runs(
    runs: Vec<ToolRun>,
    repo_root: &Path,
    progress: Option<&ReviewProgressSender>,
) -> ToolRunsInFlight {
    let identities = runs.iter().map(RunIdentity::of).collect();
    let repo_root = repo_root.to_path_buf();
    let progress = progress.cloned();
    let task = tokio::task::spawn_blocking(move || {
        execute_tool_runs(&runs, &repo_root, progress.as_ref())
    });
    ToolRunsInFlight { task, identities }
}

/// The names one tool run is reported under.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RunIdentity {
    /// The owning validator (RuleSet) name.
    validator: String,
    /// The tool rule's name.
    rule: String,
}

impl RunIdentity {
    /// The names `run` is reported under.
    fn of(run: &ToolRun) -> Self {
        Self {
            validator: run.validator.clone(),
            rule: run.rule.clone(),
        }
    }
}

/// The tool runs of one review, running while the fleet works.
#[derive(Debug)]
pub struct ToolRunsInFlight {
    /// The blocking task running the scripts.
    task: tokio::task::JoinHandle<ToolOutcome>,
    /// The names of every run the task carries, so a task that did not finish
    /// is still reported as a broken run for each one rather than as silence.
    identities: Vec<RunIdentity>,
}

impl ToolRunsInFlight {
    /// Wait for the runs to finish and take their outcome.
    ///
    /// A task that did not finish reports one [`ToolRunError`] per run it
    /// carried. The runs are never cancelled, so this can only be a panic in
    /// the run loop — a bug, and the reader has to see which rules it cost.
    pub async fn finish(self) -> ToolOutcome {
        match self.task.await {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    runs = self.identities.len(),
                    "the tool runs did not finish; reporting every run as broken"
                );
                let detail = format!("the tool run task did not finish: {error}");
                ToolOutcome {
                    findings: Vec::new(),
                    errors: self
                        .identities
                        .into_iter()
                        .map(|identity| ToolRunError {
                            validator: identity.validator,
                            rule: identity.rule,
                            detail: detail.clone(),
                        })
                        .collect(),
                }
            }
        }
    }
}

/// Why a tool-rule script run produced no findings.
///
/// A variant carries WHAT went wrong and never how the caller words it: the
/// doctor names the fixture it was proving, the review engine names the rule it
/// was running, and each writes its own sentence from the same three facts.
///
/// The `Display` states the one sentence no caller can improve on — a shell
/// that would not start — and the [`std::error::Error`] impl keeps that
/// failure's own error reachable through [`std::error::Error::source`] rather
/// than flattening the chain into a string.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ScriptFailure {
    /// The shell could not be started at all.
    #[error("the run script failed to start: {0}")]
    Start(#[from] std::io::Error),
    /// The script ran and exited nonzero. Carries
    /// [`command_failure_detail`] — the script's own stderr, or its status.
    #[error("the run script exited nonzero: {0}")]
    Exit(String),
    /// The script exited 0, but its stdout broke the finding contract.
    #[error("the run script broke the finding contract: {0}")]
    Contract(String),
}

/// The positional arguments a script of `scope` receives.
///
/// A `files`-scope script reads the paths it is handed. A `workspace`-scope
/// script reads the tree it runs in and is handed none, so the caller filters
/// its findings by path afterwards.
pub(crate) fn script_args<S: AsRef<OsStr>>(scope: ToolScope, files: &[S]) -> Vec<&OsStr> {
    match scope {
        ToolScope::Files => files.iter().map(AsRef::as_ref).collect(),
        ToolScope::Workspace => Vec::new(),
    }
}

/// Run a tool-rule script in `dir` with `args` as its positional parameters,
/// and parse its stdout into findings.
///
/// The ONE way a tool-rule script is run for its findings. The doctor's
/// fixture checks ([`crate::doctor`]) prove a rule works by calling this, and
/// [`run_tool_script`] uses the rule by calling this, so a rule can never pass
/// its fixtures under one interpretation of the contract and be used under
/// another.
///
/// Exit 0 means the script judged the code, whether or not it found anything;
/// a nonzero exit is a broken tool, not a clean run.
pub(crate) fn run_script_findings(
    script: &str,
    dir: &Path,
    args: &[&OsStr],
) -> Result<Vec<Finding>, ScriptFailure> {
    let output = run_shell(script, Some(dir), args).map_err(ScriptFailure::Start)?;
    if !output.status.success() {
        return Err(ScriptFailure::Exit(command_failure_detail(&output)));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_tool_stdout(&stdout).map_err(|e| ScriptFailure::Contract(e.to_string()))
}

/// Run one tool rule's script and parse its stdout into tagged findings.
///
/// The script runs with bash at `repo_root`. `scope: files` passes the run's
/// matched files as the script's arguments; `scope: workspace` passes none and
/// keeps only the findings in the run's matched files afterwards. Exit 0 means
/// the script judged the code; a nonzero exit (or stdout that violates the
/// contract) is the error string — the raw stderr, or the parse problem.
fn run_tool_script(run: &ToolRun, repo_root: &Path) -> Result<Vec<Finding>, String> {
    let args = script_args(run.spec.scope, &run.files);
    let mut findings =
        run_script_findings(&run.spec.run, repo_root, &args).map_err(|failure| match failure {
            ScriptFailure::Exit(detail) | ScriptFailure::Contract(detail) => detail,
            // The shell that would not start: its own `Display` says it best.
            start => start.to_string(),
        })?;

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
mod tests;
