//! Agent-agnostic review-engine status facts for doctor surfaces.
//!
//! Mirrors the `mirdan::status` pattern: this module produces plain fact
//! structs about the review engine for one workspace and converts them into
//! doctor [`Check`](swissarmyhammer_doctor::Check) rows. `sah doctor`
//! consumes the conversion; any other
//! surface can consume the facts directly.
//!
//! The contract is the Doctor section of `builtin/validators/README.md`.
//! For the current project the facts cover:
//!
//! - the detected project types (from the `PROJECT_TYPE_SPECS` detection),
//! - each validator set and whether it applies to this project,
//! - each tool rule for the detected project types: tool present or missing,
//!   tool version (`doctor.check_version_command`), and fixture result,
//! - each tool rule on its prompt fallback — tool missing or fixtures failed
//!   — with the `install.commands` to fix it, or the `doctor.fix_hint` when the
//!   tool ships with the language toolchain and there is nothing to install.
//!
//! Fixture checks run the rule's `run` script against the set's
//! `fixtures/<rule>.fail.*` and `fixtures/<rule>.pass.*` files: the fail
//! fixture must produce at least one finding, the pass fixture none. A rule
//! that fails its fixtures is reported and not used.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Output;

use tempfile::TempDir;

use swissarmyhammer_common::command::{command_failure_detail, shell_command, Shell};
use swissarmyhammer_doctor::{Check, CheckStatus};

use crate::error::AvpError;
use crate::review::scope::{as_borrowed_strings, detected_project_type_keys};
use crate::review::tool_health::{tool_rule_health, HealthProof, ToolHealthCache};
use crate::review::tool_rules::{
    normalize_tool_path, project_tool_rules, run_script_findings, script_args, ScriptFailure,
};
use crate::validators::types::{
    FixHint, Rule, RuleSet, Supersedes, ToolSpec, ValidatorMatch, ValidatorSource,
    FIXTURES_DIR_NAME,
};
use crate::validators::ValidatorLoader;

/// The check name for the detected project types row.
pub const PROJECT_TYPES_CHECK_NAME: &str = "Validator Project Types";

/// The number of always-included rows [`to_checks`] emits — the one detected
/// project-types row.
const PROJECT_TYPES_ROWS: usize = 1;

/// The suffix that marks a fixture file as a template rather than source.
///
/// A fixture carries the very defect its tool rule reports, so a fixture
/// stored under a real source extension is a file the review engine would
/// review — and every missing-docs rule would fire on the fixture built to
/// make it fire. The stored name therefore ends in `.tmpl`, which no language
/// owns and no file group matches. [`materialize_fixtures`] drops the suffix
/// when it copies the file, so the tool still sees the extension it needs.
pub(crate) const FIXTURE_TEMPLATE_SUFFIX: &str = ".tmpl";

/// The fixture that must make the tool report at least one finding.
const FAIL_FIXTURE_KIND: &str = "fail";

/// The fixture that must make the tool report zero findings.
const PASS_FIXTURE_KIND: &str = "pass";

/// The lead-in every "the tool is missing" fix shares, so a person reads one
/// sentence whether the rule ships install commands or a fix hint.
const FIX_LEAD_IN: &str = "Install the tool: ";

/// The check name for one validator set row.
pub fn set_check_name(set_name: &str) -> String {
    format!("Validator Set · {set_name}")
}

/// The check name for one tool rule row.
pub fn tool_rule_check_name(set_name: &str, rule_name: &str) -> String {
    format!("Validator Tool Rule · {set_name}/{rule_name}")
}

/// The review-engine facts for one workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEngineStatus {
    /// The detected project type keys (e.g. "rust"), sorted and distinct.
    pub project_types: Vec<String>,

    /// One row per loaded validator set, sorted by name.
    pub sets: Vec<SetStatus>,

    /// One row per tool rule that serves the detected project types,
    /// in set order.
    pub tool_rules: Vec<ToolRuleStatus>,
}

/// Whether one validator set applies to the workspace under diagnosis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetStatus {
    /// The set name.
    pub name: String,

    /// Where the set was loaded from (builtin, user, or project).
    pub source: ValidatorSource,

    /// Whether the set's `match.project_types` constraint fits the detected
    /// project types. A set with no project-type constraint always applies.
    pub applies: bool,
}

/// Whether a tool rule's tool is installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolPresence {
    /// `doctor.check_command` passed (or the rule declares no doctor block).
    Present,

    /// `doctor.check_command` failed — the tool is not usable.
    Missing {
        /// What the check command reported.
        detail: String,
    },
}

/// The result of running a tool rule against its fixtures.
///
/// The outcome is never stored. [`ToolHealthCache`](crate::review::ToolHealthCache)
/// keeps a PASS and nothing else, so an entry standing under a rule's key is
/// itself the statement that the rule passed, and every other outcome is
/// proved again on the next run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureOutcome {
    /// The fail fixture produced findings and the pass fixture none.
    Passed,

    /// A fixture run broke the contract — wrong finding count, a nonzero
    /// exit, or unparseable stdout. The rule is reported and not used.
    Failed {
        /// What went wrong.
        detail: String,
    },

    /// The set ships no fixture pair for this rule, so the rule cannot be
    /// proven healthy.
    MissingFixtures {
        /// Which fixture files were expected.
        detail: String,
    },

    /// The tool is missing, so no fixture run was attempted.
    Skipped,
}

/// The doctor facts for one tool rule of a detected project type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRuleStatus {
    /// The owning validator set.
    pub set_name: String,

    /// The rule name.
    pub rule_name: String,

    /// Whether the tool is installed.
    pub presence: ToolPresence,

    /// The tool version reported by `doctor.check_version_command`.
    pub version: Option<String>,

    /// The fixture result.
    pub fixtures: FixtureOutcome,

    /// The `install.commands` that fix a missing tool, in order of preference.
    pub install_commands: Vec<String>,

    /// The `doctor.fix_hint` the rule states: the command a person runs when
    /// the tool has no package to install. Reported, never run.
    pub fix_hint: Option<FixHint>,

    /// The prompt rules this tool rule replaces when healthy.
    pub supersedes: Supersedes,
}

impl ToolRuleStatus {
    /// Whether the review engine may use this tool rule: the tool is present
    /// and the fixtures passed.
    pub fn usable(&self) -> bool {
        self.presence == ToolPresence::Present && self.fixtures == FixtureOutcome::Passed
    }

    /// Whether the superseded prompt rule runs instead of this tool rule.
    pub fn on_prompt_fallback(&self) -> bool {
        !self.usable()
    }

    /// Why the rule is not usable, for a fallback note. Empty for a usable
    /// rule.
    pub fn degraded_detail(&self) -> String {
        match (&self.presence, &self.fixtures) {
            (ToolPresence::Missing { detail }, _) => format!("tool missing: {detail}"),
            (ToolPresence::Present, FixtureOutcome::Failed { detail }) => {
                format!("fixtures failed: {detail}")
            }
            (ToolPresence::Present, FixtureOutcome::MissingFixtures { detail }) => {
                format!("fixtures missing: {detail}")
            }
            (ToolPresence::Present, FixtureOutcome::Skipped) => "fixture check skipped".to_string(),
            (ToolPresence::Present, FixtureOutcome::Passed) => String::new(),
        }
    }
}

/// Produce the review-engine facts for the workspace at `workspace_root`.
///
/// Loads the full validator stack via [`crate::load_rules`] (builtin → user →
/// project precedence), resolves the detected project types for
/// `workspace_root`, and delegates to [`check_review_engine_with`].
///
/// `workspace_root` selects both halves of the answer — the project validator
/// layer that loads and the project types the sets are judged against — so the
/// facts describe the workspace the caller named and no other. The caller
/// resolves that root; this never falls back to the process current directory.
///
/// # Errors
///
/// Returns an [`AvpError`] when the validator stack fails to load.
pub fn check_review_engine(workspace_root: &Path) -> Result<ReviewEngineStatus, AvpError> {
    let loader = crate::load_rules(Some(workspace_root))?;
    let project_types = detected_project_type_keys(workspace_root);
    let health = ToolHealthCache::open(workspace_root);
    let status =
        check_review_engine_with(&loader, &as_borrowed_strings(&project_types), Some(&health));
    health.save();
    Ok(status)
}

/// Produce the review-engine facts from an explicit loader and detected
/// project types.
///
/// This is the injectable core of [`check_review_engine`]: tests drive it
/// with a synthetic loader and type list, without depending on the host's
/// validator directories or workspace.
///
/// `health` is the workspace's stored fixture verdicts, when one is open.
/// Doctor never reads a stored verdict — it proves every rule, stores the
/// pass, and drops what a rule no longer earns — so a review that follows
/// doctor reads doctor's own answer. The caller saves the cache afterwards,
/// which is what carries the drop to the next process. `None` proves every
/// rule and stores nothing.
pub fn check_review_engine_with(
    loader: &ValidatorLoader,
    project_types: &[&str],
    health: Option<&ToolHealthCache>,
) -> ReviewEngineStatus {
    let sets = loader
        .list_rulesets()
        .into_iter()
        .map(|ruleset| SetStatus {
            name: ruleset.name().to_string(),
            source: ruleset.source.clone(),
            applies: ValidatorMatch::criteria_applies(
                ruleset.manifest.match_criteria.as_ref(),
                project_types,
            ),
        })
        .collect();

    // Doctor is the ground truth, so it proves every rule for itself and
    // replaces whatever verdict `health` holds. A review that follows then
    // reads doctor's own answer rather than an older one.
    let tool_rules = project_tool_rules(loader, project_types)
        .into_iter()
        .map(|matched| {
            tool_rule_health(
                health,
                HealthProof::Fresh,
                matched.ruleset,
                matched.rule,
                matched.spec,
            )
        })
        .collect();

    ReviewEngineStatus {
        project_types: project_types.iter().copied().map(String::from).collect(),
        sets,
        tool_rules,
    }
}

/// Produce the doctor facts for one tool rule: presence, version, fixtures.
///
/// Crate-visible so the review engine's tool-rule planner
/// ([`crate::review::tool_rules`]) reuses the same health decision doctor
/// reports — "healthy" can never mean two different things.
pub(crate) fn check_tool_rule(ruleset: &RuleSet, rule: &Rule, spec: &ToolSpec) -> ToolRuleStatus {
    check_tool_rule_with(ruleset, rule, spec, |ruleset, rule, spec, _version| {
        check_fixtures(ruleset, rule, spec)
    })
}

/// Produce the doctor facts for one tool rule, with `fixture_check` deciding
/// the fixture half.
///
/// Presence and version are read fresh on every call — each is one cheap
/// command — and the version is handed to `fixture_check` because a stored
/// fixture verdict is keyed on it (see
/// [`ToolHealthCache`](crate::review::ToolHealthCache)). `fixture_check` runs
/// only when the tool is present, so a missing tool still costs one command.
///
/// This is the ONE place presence, version, and fixtures become a
/// [`ToolRuleStatus`], so the review engine's stored verdict and `sah doctor`'s
/// proved one describe a tool rule the same way.
pub(crate) fn check_tool_rule_with<F>(
    ruleset: &RuleSet,
    rule: &Rule,
    spec: &ToolSpec,
    fixture_check: F,
) -> ToolRuleStatus
where
    F: FnOnce(&RuleSet, &Rule, &ToolSpec, Option<&str>) -> FixtureOutcome,
{
    let presence = check_presence(spec);
    let (version, fixtures) = match &presence {
        ToolPresence::Present => {
            let version = check_version(spec);
            let fixtures = fixture_check(ruleset, rule, spec, version.as_deref());
            (version, fixtures)
        }
        ToolPresence::Missing { .. } => (None, FixtureOutcome::Skipped),
    };

    ToolRuleStatus {
        set_name: ruleset.name().to_string(),
        rule_name: rule.name.clone(),
        presence,
        version,
        fixtures,
        install_commands: spec
            .install
            .as_ref()
            .map(|install| install.commands.clone())
            .unwrap_or_default(),
        fix_hint: spec
            .doctor
            .as_ref()
            .and_then(|doctor| doctor.fix_hint.clone()),
        supersedes: rule.supersedes.clone(),
    }
}

/// Run `doctor.check_command` to decide whether the tool is installed.
///
/// A rule without a doctor block is treated as present — the fixture run is
/// then the only health evidence.
///
/// Crate-visible so the install lifecycle
/// ([`crate::review::tool_install`]) decides presence with the same function,
/// before and after every install attempt — "installed" can never mean two
/// things.
pub(crate) fn check_presence(spec: &ToolSpec) -> ToolPresence {
    let Some(doctor) = &spec.doctor else {
        return ToolPresence::Present;
    };

    match run_shell(&doctor.check_command, None, &[]) {
        Ok(output) if output.status.success() => ToolPresence::Present,
        Ok(output) => ToolPresence::Missing {
            detail: command_failure_detail(&output),
        },
        Err(e) => ToolPresence::Missing {
            detail: format!("check command failed to run: {e}"),
        },
    }
}

/// Run `doctor.check_version_command` and return the first stdout line.
fn check_version(spec: &ToolSpec) -> Option<String> {
    let command = spec.doctor.as_ref()?.check_version_command.as_ref()?;
    let output = run_shell(command, None, &[]).ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?.trim();
    (!first_line.is_empty()).then(|| first_line.to_string())
}

/// Run the rule's script against its fail and pass fixtures.
///
/// The fail fixture must produce at least one finding; the pass fixture must
/// produce none. Any broken run — nonzero exit, unparseable stdout — is a
/// failure with its detail.
///
/// This is the expensive half of a health check, and the only half a stored
/// verdict replaces. Crate-visible so
/// [`ToolHealthCache`](crate::review::ToolHealthCache) proves a rule by
/// calling exactly what doctor calls.
pub(crate) fn check_fixtures(ruleset: &RuleSet, rule: &Rule, spec: &ToolSpec) -> FixtureOutcome {
    match verify_fixture_contract(ruleset, rule, spec) {
        Ok(()) => FixtureOutcome::Passed,
        Err(outcome) => outcome,
    }
}

/// The fixture contract as a fallible check: `Ok` means both fixtures
/// behaved, `Err` carries the degraded outcome to report.
fn verify_fixture_contract(
    ruleset: &RuleSet,
    rule: &Rule,
    spec: &ToolSpec,
) -> Result<(), FixtureOutcome> {
    let fixtures_dir = ruleset.fixtures_dir();
    let fail_fixture = find_fixture(&fixtures_dir, &rule.name, FAIL_FIXTURE_KIND);
    let pass_fixture = find_fixture(&fixtures_dir, &rule.name, PASS_FIXTURE_KIND);

    let (Some(fail_fixture), Some(pass_fixture)) = (fail_fixture, pass_fixture) else {
        return Err(FixtureOutcome::MissingFixtures {
            detail: format!(
                "expected {name}.{FAIL_FIXTURE_KIND}.* and {name}.{PASS_FIXTURE_KIND}.* under {dir}",
                name = rule.name,
                dir = fixtures_dir.display(),
            ),
        });
    };

    let fail_count = run_and_count_fixture(spec, &fail_fixture)?;
    if fail_count == 0 {
        return Err(FixtureOutcome::Failed {
            detail: format!(
                "the fail fixture {} produced no findings; at least one is required",
                fixture_label(&fail_fixture),
            ),
        });
    }

    let pass_count = run_and_count_fixture(spec, &pass_fixture)?;
    if pass_count != 0 {
        return Err(FixtureOutcome::Failed {
            detail: format!(
                "the pass fixture {} produced {pass_count} finding(s); none are allowed",
                fixture_label(&pass_fixture),
            ),
        });
    }

    Ok(())
}

/// Run one fixture and count its findings, mapping a broken run to the
/// `Failed` outcome so the caller can use `?`.
fn run_and_count_fixture(spec: &ToolSpec, fixture: &Path) -> Result<usize, FixtureOutcome> {
    run_fixture(spec, fixture).map_err(|detail| FixtureOutcome::Failed { detail })
}

/// Find the fixture file named `<rule>.<kind>.<any extension>` in `dir`.
fn find_fixture(dir: &Path, rule_name: &str, kind: &str) -> Option<PathBuf> {
    let prefix = format!("{rule_name}.{kind}.");
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| name.starts_with(&prefix))
        })
}

/// Run the rule's script against one fixture file and count the findings it
/// reported ABOUT that fixture.
///
/// The script runs against a scratch copy of the fixtures directory, never the
/// directory in the validator set — see [`materialize_fixtures`]. A
/// `files`-scope script receives the materialized fixture file name as `"$@"`;
/// a `workspace`-scope script runs with no arguments.
///
/// A `workspace`-scope script reads the whole fixtures directory, so it sees
/// the fail fixture and the pass fixture on every run. Only the findings whose
/// path is the fixture under test count — the same attribution
/// [`execute_tool_runs`](crate::review::execute_tool_runs) makes when it keeps
/// only the findings in the changed files. Without it a `workspace`-scope rule
/// could never pass the pair, because both runs report the same findings.
fn run_fixture(spec: &ToolSpec, fixture: &Path) -> Result<usize, String> {
    let source_dir = fixture.parent().unwrap_or_else(|| Path::new("."));
    let fixture_name = materialized_name(fixture)?;

    let scratch = materialize_fixtures(source_dir)?;
    let fixture_dir = scratch.path();

    let args = script_args(spec.scope, std::slice::from_ref(&fixture_name));
    let findings =
        run_script_findings(&spec.run, fixture_dir, &args).map_err(|failure| match failure {
            ScriptFailure::Start(e) => {
                format!("tool failed to run on {}: {e}", fixture_label(fixture))
            }
            ScriptFailure::Exit(detail) => {
                format!("tool broke on {}: {detail}", fixture_label(fixture))
            }
            ScriptFailure::Contract(detail) => format!(
                "tool stdout on {} broke the contract: {detail}",
                fixture_label(fixture)
            ),
        })?;
    let about_fixture = findings
        .iter()
        .filter(|finding| {
            Path::new(&normalize_tool_path(&finding.file, fixture_dir)).file_name()
                == Some(fixture_name.as_os_str())
        })
        .count();
    Ok(about_fixture)
}

/// The name a fixture template takes once it is materialized: its own file
/// name with a trailing [`FIXTURE_TEMPLATE_SUFFIX`] removed. A name without
/// the suffix is its own materialized name.
fn materialized_name(fixture: &Path) -> Result<OsString, String> {
    let name = fixture
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("fixture path has no file name: {}", fixture.display()))?;
    Ok(OsString::from(
        name.strip_suffix(FIXTURE_TEMPLATE_SUFFIX).unwrap_or(name),
    ))
}

/// Copy a validator set's fixture directory into a scratch directory, dropping
/// [`FIXTURE_TEMPLATE_SUFFIX`] from every name.
///
/// The set stores `missing-docs-rust.fail.rs.tmpl`; the tool under test needs
/// `missing-docs-rust.fail.rs`, so the runner writes that name here and runs
/// against this copy. The set's own directory is never the working directory,
/// so a tool that writes beside its input — a build cache, a lock file —
/// cannot dirty the repository.
///
/// The whole directory is copied, not only the fixture under test: a
/// `workspace`-scope tool reads the fixture's neighbours, such as a Cargo
/// manifest, a Go module file, or the `lib.rs` that declares the Rust
/// fixtures as modules. Sub-directories are skipped; a fixture directory
/// holds no fixture input below its top level, only caches a tool left behind.
fn materialize_fixtures(source_dir: &Path) -> Result<TempDir, String> {
    let scratch = tempfile::Builder::new()
        .prefix("sah-fixtures-")
        .tempdir()
        .map_err(|e| format!("could not create a fixture scratch directory: {e}"))?;

    let entries = std::fs::read_dir(source_dir)
        .map_err(|e| format!("could not read {}: {e}", source_dir.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = materialized_name(&path)?;
        std::fs::copy(&path, scratch.path().join(&name))
            .map_err(|e| format!("could not copy {}: {e}", path.display()))?;
    }

    Ok(scratch)
}

/// The fixture's file name for messages, falling back to the full path.
fn fixture_label(fixture: &Path) -> String {
    fixture
        .file_name()
        .and_then(OsStr::to_str)
        .map(str::to_string)
        .unwrap_or_else(|| fixture.display().to_string())
}

/// Convert the review-engine facts into doctor [`Check`] rows.
///
/// One row for the detected project types, one per validator set, one per
/// tool rule. A missing tool or a failing fixture is a Warning — a degraded
/// review never blocks — with the install commands as the fix.
pub fn to_checks(status: &ReviewEngineStatus) -> Vec<Check> {
    let mut checks =
        Vec::with_capacity(PROJECT_TYPES_ROWS + status.sets.len() + status.tool_rules.len());
    checks.push(project_types_check(&status.project_types));
    checks.extend(status.sets.iter().map(set_check));
    checks.extend(status.tool_rules.iter().map(tool_rule_check));
    checks
}

/// The detected-project-types row.
fn project_types_check(project_types: &[String]) -> Check {
    let message = if project_types.is_empty() {
        "none detected".to_string()
    } else {
        project_types.join(", ")
    };
    Check {
        name: PROJECT_TYPES_CHECK_NAME.to_string(),
        status: CheckStatus::Ok,
        message,
        fix: None,
    }
}

/// One validator set's applicability row. Informational either way — a set
/// that does not apply to this project is expected, not a problem.
fn set_check(set: &SetStatus) -> Check {
    let message = if set.applies {
        format!("applies to this project ({})", set.source)
    } else {
        format!("does not apply to this project ({})", set.source)
    };
    Check {
        name: set_check_name(&set.name),
        status: CheckStatus::Ok,
        message,
        fix: None,
    }
}

/// One tool rule's health row.
///
/// A degraded row's message is [`ToolRuleStatus::degraded_detail`] — the SAME
/// string the review engine renders for the rule's prompt fallback — plus the
/// [`fallback_note`] suffix, so doctor and the engine can never describe the
/// same degradation two different ways.
fn tool_rule_check(rule: &ToolRuleStatus) -> Check {
    let name = tool_rule_check_name(&rule.set_name, &rule.rule_name);
    if rule.usable() {
        return Check {
            name,
            status: CheckStatus::Ok,
            message: format!("tool present{}; fixtures pass", version_note(rule)),
            fix: None,
        };
    }
    Check {
        name,
        status: CheckStatus::Warning,
        message: format!("{}{}", rule.degraded_detail(), fallback_note(rule)),
        fix: degraded_fix(rule),
    }
}

/// The fix for a degraded row: missing fixtures ask for the fixture pair,
/// every other degradation asks for the rule's install commands, and a rule
/// with no install commands falls back to its `doctor.fix_hint`.
fn degraded_fix(rule: &ToolRuleStatus) -> Option<String> {
    match &rule.fixtures {
        FixtureOutcome::MissingFixtures { .. } => Some(format!(
            "Add {rule}.{FAIL_FIXTURE_KIND}.* and {rule}.{PASS_FIXTURE_KIND}.* to the set's {FIXTURES_DIR_NAME}/ directory",
            rule = rule.rule_name,
        )),
        _ => install_fix(rule).or_else(|| fix_hint_fix(rule)),
    }
}

/// The "which prompt rules run instead" suffix for degraded rows. Names every
/// prompt rule the tool rule supersedes.
fn fallback_note(rule: &ToolRuleStatus) -> String {
    if rule.supersedes.is_empty() {
        return "; prompt fallback".to_string();
    }
    format!(
        "; {} {} instead",
        rule.supersedes.prompt_rule_phrase(),
        rule.supersedes.runs_verb()
    )
}

/// The "; vX.Y.Z" suffix for healthy rows, empty when no version was read.
fn version_note(rule: &ToolRuleStatus) -> String {
    match &rule.version {
        Some(version) => format!(" ({version})"),
        None => String::new(),
    }
}

/// The install-command fix for a degraded row, `None` when the rule declares
/// no install commands.
fn install_fix(rule: &ToolRuleStatus) -> Option<String> {
    if rule.install_commands.is_empty() {
        return None;
    }
    Some(format!(
        "{FIX_LEAD_IN}{}",
        rule.install_commands.join(" || ")
    ))
}

/// The fix-hint fallback for a degraded row, `None` when the rule states no
/// hint.
///
/// A tool that ships with the language toolchain has no package to install, so
/// its rule names the command a person runs instead. The engine renders that
/// text and never runs it — [`FixHint`] is not a command string.
fn fix_hint_fix(rule: &ToolRuleStatus) -> Option<String> {
    let hint = rule.fix_hint.as_ref()?;
    Some(format!("{FIX_LEAD_IN}{hint}"))
}

/// Run a tool-rule shell snippet, with `args` as the script's positional
/// parameters (`"$@"`).
///
/// The ONE shell runner for tool-rule scripts — the doctor's fixture checks
/// and the review engine's tool runs ([`crate::review::tool_rules`]) both go
/// through it, so a script can never pass its fixtures under one shell and
/// run under another. The interpreter and the stream wiring come from
/// [`shell_command`], the same builder every other caller in the workspace
/// spawns a shell with. What this runner adds is its own: the tool-rule
/// contract's `"$@"`.
///
/// THE SHELL IS BASH, and never `sh`. Every rule script is written against
/// bash, and every measurement a tool-rule test states is taken with bash. The
/// two shells are not the same reader: `sh` reads a script in POSIX mode,
/// where a failed special builtin stops the whole run, and bash carries on
/// past it. `Shell::Bash` below is therefore part of the contract, not a
/// default.
pub(crate) fn run_shell(
    script: &str,
    cwd: Option<&Path>,
    args: &[&OsStr],
) -> std::io::Result<Output> {
    // `bash` is the `$0` a shell reads before `"$@"` begins, so the first real
    // argument is not swallowed as the script's own name.
    let mut command = shell_command(Shell::Bash, script);
    command.arg("bash").args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    command.output()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write one validator set directory: a VALIDATOR.md manifest, rule
    /// files under `rules/`, and fixture files under `fixtures/`.
    fn write_ruleset(
        root: &Path,
        dir_name: &str,
        manifest: &str,
        rules: &[(&str, &str)],
        fixtures: &[(&str, &str)],
    ) {
        let set_dir = root.join(dir_name);
        std::fs::create_dir_all(set_dir.join("rules")).expect("create rules dir");
        std::fs::write(set_dir.join("VALIDATOR.md"), manifest).expect("write manifest");
        for (name, content) in rules {
            std::fs::write(set_dir.join("rules").join(name), content).expect("write rule");
        }
        if !fixtures.is_empty() {
            std::fs::create_dir_all(set_dir.join(FIXTURES_DIR_NAME)).expect("create fixtures dir");
            for (name, content) in fixtures {
                std::fs::write(set_dir.join(FIXTURES_DIR_NAME).join(name), content)
                    .expect("write fixture");
            }
        }
    }

    /// Load every set under `root` as project-source rulesets.
    fn loader_for(root: &Path) -> ValidatorLoader {
        let mut loader = ValidatorLoader::new();
        loader
            .load_rulesets_directory(root, ValidatorSource::Project)
            .expect("load rulesets");
        loader
    }

    const PLAIN_MANIFEST: &str =
        "---\nname: plain-set\ndescription: A set with no project-type constraint.\n---\n";

    const PYTHON_ONLY_MANIFEST: &str = "---\nname: python-set\ndescription: A set keyed to python projects.\nmatch:\n  project_types:\n    - python\n---\n";

    const TOOL_SET_MANIFEST: &str =
        "---\nname: tool-set\ndescription: A set carrying tool rules.\n---\n";

    /// A working tool rule: grep for TODO markers, `path:line: message` output,
    /// deterministic doctor and version commands, one install command.
    const GREP_TOOL_RULE: &str = r#"---
name: todo-check
description: Flag TODO markers via grep.
supersedes: missing-docs
tool:
  scope: files
  run: |
    grep -n TODO "$@" /dev/null | awk -F: '{print $1 ":" $2 ": found TODO"}'
  doctor:
    check_command: "which grep awk"
    check_version_command: "echo tool 1.2.3"
  install:
    commands:
      - "brew install grep"
---
Grep for TODO markers.
"#;

    /// A tool rule whose check command names a binary that cannot exist. It
    /// declares an install command AND a fix hint, so the doctor row proves
    /// which of the two a person is shown.
    const MISSING_TOOL_RULE: &str = r#"---
name: gone-check
description: A rule whose tool is not installed.
supersedes: missing-docs
tool:
  scope: files
  run: "definitely-not-a-real-tool-1f9c \"$@\""
  doctor:
    check_command: "definitely-not-a-real-tool-1f9c --version"
    fix_hint: "ask the platform team for definitely-not-a-real-tool-1f9c"
  install:
    commands:
      - "brew install definitely-not-a-real-tool-1f9c"
---
Never runs.
"#;

    /// A tool rule that supersedes TWO prompt rules and whose tool cannot
    /// exist, so its degraded row must name both of them.
    const PAIR_MISSING_TOOL_RULE: &str = r#"---
name: pair-check
description: A rule that replaces two prompt rules.
supersedes:
  - function-length
  - missing-docs
tool:
  scope: files
  run: "definitely-not-a-real-tool-1f9c \"$@\""
  doctor:
    check_command: "definitely-not-a-real-tool-1f9c --version"
---
Never runs.
"#;

    /// A tool rule whose script never reports findings, so its fail fixture
    /// cannot pass.
    const SILENT_TOOL_RULE: &str = r#"---
name: silent-check
description: A rule whose tool reports nothing.
tool:
  scope: files
  run: "true"
  doctor:
    check_command: "true"
---
Always silent.
"#;

    /// A tool rule whose script is present but broken: it says why on stderr
    /// and exits nonzero. Exit 0 means the tool judged the code, so a nonzero
    /// exit is a broken tool rather than a clean run.
    const BROKEN_TOOL_RULE: &str = r#"---
name: broken-check
description: A rule whose tool fails whenever it runs.
tool:
  scope: files
  run: |
    echo "the analyzer could not load its grammar" >&2
    exit 4
  doctor:
    check_command: "true"
---
Always broken.
"#;

    /// A `workspace`-scope tool rule: the script reads the whole directory it
    /// runs in, never the fixture it is asked about, so both fixture runs see
    /// both fixture files and report the same findings.
    const WORKSPACE_TOOL_RULE: &str = r#"---
name: workspace-check
description: Flag TODO markers across the whole workspace.
supersedes: missing-docs
tool:
  scope: workspace
  run: |
    grep -rn TODO . | awk -F: '{print $1 ":" $2 ": found TODO"}'
  doctor:
    check_command: "which grep awk"
---
Grep the workspace for TODO markers.
"#;

    /// A tool rule scoped to python projects only.
    const PYTHON_TOOL_RULE: &str = r#"---
name: python-only-check
description: A rule serving python projects.
match:
  project_types:
    - python
tool:
  scope: files
  run: "true"
  doctor:
    check_command: "true"
---
Python only.
"#;

    /// The detected project types of a Rust workspace, in the borrowed form
    /// [`check_review_engine_with`] takes them.
    const RUST_TYPES: &[&str] = &["rust"];

    #[test]
    fn test_detected_project_types_flow_into_status() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"probe\"\nversion = \"0.0.1\"\n",
        )
        .expect("write Cargo.toml");
        std::fs::create_dir_all(temp.path().join("src")).expect("create src");
        std::fs::write(temp.path().join("src/main.rs"), "fn main() {}\n").expect("write main.rs");

        let detected = detected_project_type_keys(temp.path());
        assert!(
            detected.iter().any(|k| k == "rust"),
            "a Cargo.toml workspace must detect as rust; got {detected:?}"
        );

        let loader = ValidatorLoader::new();
        let detected_keys: Vec<&str> = detected.iter().map(String::as_str).collect();
        let status = check_review_engine_with(&loader, &detected_keys, None);
        assert_eq!(status.project_types, detected);
    }

    #[test]
    fn test_set_applicability_follows_project_types() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(temp.path(), "plain-set", PLAIN_MANIFEST, &[], &[]);
        write_ruleset(temp.path(), "python-set", PYTHON_ONLY_MANIFEST, &[], &[]);
        let loader = loader_for(temp.path());

        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        let applies: std::collections::BTreeMap<&str, bool> = status
            .sets
            .iter()
            .map(|s| (s.name.as_str(), s.applies))
            .collect();
        assert_eq!(applies.get("plain-set"), Some(&true));
        assert_eq!(applies.get("python-set"), Some(&false));
    }

    #[test]
    fn test_sets_are_sorted_by_name() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(temp.path(), "plain-set", PLAIN_MANIFEST, &[], &[]);
        write_ruleset(temp.path(), "python-set", PYTHON_ONLY_MANIFEST, &[], &[]);
        write_ruleset(temp.path(), "tool-set", TOOL_SET_MANIFEST, &[], &[]);
        let loader = loader_for(temp.path());

        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        let names: Vec<&str> = status.sets.iter().map(|s| s.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "sets must be sorted by name");
        assert_eq!(names.len(), 3, "all three sets must be reported");
    }

    #[test]
    fn test_tool_rule_present_with_version_and_passing_fixtures() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[("todo-check.md", GREP_TOOL_RULE)],
            &[
                ("todo-check.fail.txt", "a TODO marker\n"),
                ("todo-check.pass.txt", "all clean here\n"),
            ],
        );
        let loader = loader_for(temp.path());

        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        assert_eq!(status.tool_rules.len(), 1, "one tool rule expected");
        let rule = &status.tool_rules[0];
        assert_eq!(rule.set_name, "tool-set");
        assert_eq!(rule.rule_name, "todo-check");
        assert_eq!(rule.presence, ToolPresence::Present);
        assert_eq!(rule.version.as_deref(), Some("tool 1.2.3"));
        assert_eq!(rule.fixtures, FixtureOutcome::Passed);
        assert_eq!(rule.supersedes.names(), ["missing-docs"]);
        assert!(rule.usable());
        assert!(!rule.on_prompt_fallback());
    }

    #[test]
    fn test_missing_tool_reports_install_commands_and_skips_fixtures() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[("gone-check.md", MISSING_TOOL_RULE)],
            &[
                ("gone-check.fail.txt", "irrelevant\n"),
                ("gone-check.pass.txt", "irrelevant\n"),
            ],
        );
        let loader = loader_for(temp.path());

        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        assert_eq!(status.tool_rules.len(), 1, "one tool rule expected");
        let rule = &status.tool_rules[0];
        assert!(
            matches!(rule.presence, ToolPresence::Missing { .. }),
            "a nonexistent binary must report Missing; got {:?}",
            rule.presence
        );
        assert_eq!(rule.version, None);
        assert_eq!(rule.fixtures, FixtureOutcome::Skipped);
        assert_eq!(
            rule.install_commands,
            vec!["brew install definitely-not-a-real-tool-1f9c".to_string()]
        );
        assert!(!rule.usable());
        assert!(rule.on_prompt_fallback());
    }

    /// A script that exits nonzero is a BROKEN tool, never a clean run, and
    /// the row says what the script said on stderr.
    #[test]
    fn test_a_nonzero_exit_is_reported_as_a_broken_tool_with_its_own_words() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[("broken-check.md", BROKEN_TOOL_RULE)],
            &[
                ("broken-check.fail.txt", "a defect the tool must flag\n"),
                ("broken-check.pass.txt", "clean\n"),
            ],
        );
        let loader = loader_for(temp.path());

        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        assert_eq!(status.tool_rules.len(), 1, "one tool rule expected");
        let rule = &status.tool_rules[0];
        assert_eq!(rule.presence, ToolPresence::Present);
        let FixtureOutcome::Failed { detail } = &rule.fixtures else {
            panic!(
                "a script that exits nonzero must fail, got {:?}",
                rule.fixtures
            );
        };
        assert!(
            detail.contains("the analyzer could not load its grammar"),
            "the row must carry the script's own stderr, got {detail:?}"
        );
        assert!(!rule.usable());
        assert!(rule.on_prompt_fallback());
    }

    #[test]
    fn test_fixture_failure_marks_rule_unusable() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[("silent-check.md", SILENT_TOOL_RULE)],
            &[
                ("silent-check.fail.txt", "a defect the tool must flag\n"),
                ("silent-check.pass.txt", "clean\n"),
            ],
        );
        let loader = loader_for(temp.path());

        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        assert_eq!(status.tool_rules.len(), 1, "one tool rule expected");
        let rule = &status.tool_rules[0];
        assert_eq!(rule.presence, ToolPresence::Present);
        assert!(
            matches!(rule.fixtures, FixtureOutcome::Failed { .. }),
            "a silent tool must fail its fail fixture; got {:?}",
            rule.fixtures
        );
        assert!(!rule.usable());
        assert!(rule.on_prompt_fallback());
    }

    /// A `workspace`-scope script reads the whole fixtures directory, so both
    /// fixture runs report the fail fixture's defect. Only the findings of the
    /// fixture under test count, exactly as the review engine keeps only the
    /// findings in the changed files for a `workspace`-scope run.
    #[test]
    fn test_workspace_scope_fixtures_count_only_the_fixture_under_test() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[("workspace-check.md", WORKSPACE_TOOL_RULE)],
            &[
                ("workspace-check.fail.txt", "a TODO marker\n"),
                ("workspace-check.pass.txt", "all clean here\n"),
            ],
        );
        let loader = loader_for(temp.path());

        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        assert_eq!(status.tool_rules.len(), 1, "one tool rule expected");
        let rule = &status.tool_rules[0];
        assert_eq!(rule.presence, ToolPresence::Present);
        assert_eq!(
            rule.fixtures,
            FixtureOutcome::Passed,
            "the pass fixture holds no defect, so a workspace run must count zero findings for it"
        );
        assert!(rule.usable());
    }

    #[test]
    fn test_missing_fixture_files_are_reported() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[("silent-check.md", SILENT_TOOL_RULE)],
            &[],
        );
        let loader = loader_for(temp.path());

        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        assert_eq!(status.tool_rules.len(), 1, "one tool rule expected");
        let rule = &status.tool_rules[0];
        assert!(
            matches!(rule.fixtures, FixtureOutcome::MissingFixtures { .. }),
            "a rule with no fixtures must report MissingFixtures; got {:?}",
            rule.fixtures
        );
    }

    #[test]
    fn test_tool_rules_filtered_by_project_type() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[
                ("todo-check.md", GREP_TOOL_RULE),
                ("python-only-check.md", PYTHON_TOOL_RULE),
            ],
            &[
                ("todo-check.fail.txt", "a TODO marker\n"),
                ("todo-check.pass.txt", "all clean here\n"),
            ],
        );
        // A whole set keyed to python must contribute no tool rules either.
        write_ruleset(
            temp.path(),
            "python-set",
            PYTHON_ONLY_MANIFEST,
            &[("silent-check.md", SILENT_TOOL_RULE)],
            &[],
        );
        let loader = loader_for(temp.path());

        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        let names: Vec<&str> = status
            .tool_rules
            .iter()
            .map(|r| r.rule_name.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["todo-check"],
            "only rules serving the detected project types are reported"
        );
    }

    #[test]
    fn test_to_checks_reports_project_types_sets_and_tool_rules() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(temp.path(), "python-set", PYTHON_ONLY_MANIFEST, &[], &[]);
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[("todo-check.md", GREP_TOOL_RULE)],
            &[
                ("todo-check.fail.txt", "a TODO marker\n"),
                ("todo-check.pass.txt", "all clean here\n"),
            ],
        );
        let loader = loader_for(temp.path());
        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        let checks = to_checks(&status);

        let types_row = checks
            .iter()
            .find(|c| c.name == PROJECT_TYPES_CHECK_NAME)
            .expect("project types row");
        assert_eq!(types_row.status, CheckStatus::Ok);
        assert!(
            types_row.message.contains("rust"),
            "project types row must name the detected types; got '{}'",
            types_row.message
        );

        let set_row = checks
            .iter()
            .find(|c| c.name == set_check_name("python-set"))
            .expect("python-set row");
        assert_eq!(set_row.status, CheckStatus::Ok);
        assert!(
            set_row.message.contains("does not apply"),
            "a non-applying set must say so; got '{}'",
            set_row.message
        );

        let tool_row = checks
            .iter()
            .find(|c| c.name == tool_rule_check_name("tool-set", "todo-check"))
            .expect("tool rule row");
        assert_eq!(tool_row.status, CheckStatus::Ok);
        assert!(
            tool_row.message.contains("tool 1.2.3"),
            "a healthy tool rule row must show the version; got '{}'",
            tool_row.message
        );
        assert!(
            tool_row.message.contains("fixtures pass"),
            "a healthy tool rule row must show the fixture result; got '{}'",
            tool_row.message
        );
    }

    #[test]
    fn test_to_checks_missing_tool_is_warning_with_install_fix() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[("gone-check.md", MISSING_TOOL_RULE)],
            &[
                ("gone-check.fail.txt", "irrelevant\n"),
                ("gone-check.pass.txt", "irrelevant\n"),
            ],
        );
        let loader = loader_for(temp.path());
        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        let checks = to_checks(&status);

        let tool_row = checks
            .iter()
            .find(|c| c.name == tool_rule_check_name("tool-set", "gone-check"))
            .expect("tool rule row");
        assert_eq!(tool_row.status, CheckStatus::Warning);
        assert!(
            tool_row.message.contains("missing"),
            "a missing tool row must say the tool is missing; got '{}'",
            tool_row.message
        );
        assert!(
            tool_row
                .message
                .contains("prompt rule 'missing-docs' runs instead"),
            "a missing tool row must name the one prompt rule that runs instead, \
             with the noun and verb agreeing with that count; got '{}'",
            tool_row.message
        );
        let fix = tool_row.fix.as_deref().expect("install fix");
        assert!(
            fix.contains("brew install definitely-not-a-real-tool-1f9c"),
            "the fix must carry the install commands; got '{fix}'"
        );
    }

    /// A degraded row names EVERY prompt rule the tool rule supersedes, and
    /// the verb agrees with the count.
    #[test]
    fn test_to_checks_missing_tool_names_every_superseded_rule() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[("pair-check.md", PAIR_MISSING_TOOL_RULE)],
            &[
                ("pair-check.fail.txt", "irrelevant\n"),
                ("pair-check.pass.txt", "irrelevant\n"),
            ],
        );
        let loader = loader_for(temp.path());
        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        let checks = to_checks(&status);

        let tool_row = checks
            .iter()
            .find(|c| c.name == tool_rule_check_name("tool-set", "pair-check"))
            .expect("tool rule row");
        assert!(
            tool_row
                .message
                .contains("prompt rules 'function-length', 'missing-docs' run instead"),
            "a degraded row must name every superseded prompt rule; got '{}'",
            tool_row.message
        );
    }

    #[test]
    fn test_to_checks_fixture_failure_is_warning() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[("silent-check.md", SILENT_TOOL_RULE)],
            &[
                ("silent-check.fail.txt", "a defect the tool must flag\n"),
                ("silent-check.pass.txt", "clean\n"),
            ],
        );
        let loader = loader_for(temp.path());
        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        let checks = to_checks(&status);

        let tool_row = checks
            .iter()
            .find(|c| c.name == tool_rule_check_name("tool-set", "silent-check"))
            .expect("tool rule row");
        assert_eq!(tool_row.status, CheckStatus::Warning);
        assert!(
            tool_row.message.contains("fixtures failed"),
            "a fixture failure row must say the fixtures failed; got '{}'",
            tool_row.message
        );
        assert!(
            tool_row.message.contains("prompt fallback"),
            "a fixture failure row must carry the fallback note; got '{}'",
            tool_row.message
        );
        // The row's core detail is degraded_detail() verbatim, so doctor and
        // the review engine can never describe the same degradation two ways.
        let rule = &status.tool_rules[0];
        assert!(
            tool_row.message.starts_with(&rule.degraded_detail()),
            "the row message must start with degraded_detail(); got '{}'",
            tool_row.message
        );
    }

    #[test]
    fn test_to_checks_no_project_types_row_says_none() {
        let loader = ValidatorLoader::new();
        let status = check_review_engine_with(&loader, &[], None);

        let checks = to_checks(&status);

        let types_row = checks
            .iter()
            .find(|c| c.name == PROJECT_TYPES_CHECK_NAME)
            .expect("project types row");
        assert_eq!(types_row.status, CheckStatus::Ok);
        assert!(
            types_row.message.contains("none detected"),
            "with no detected types the row must say so; got '{}'",
            types_row.message
        );
    }

    /// The builtin set that carries the toolchain-component tool rule.
    const BUILTIN_TOOL_SET: &str = "code-hygiene";

    /// The builtin tool rule whose tool ships with the Rust toolchain: clippy
    /// is a `rustup` component, so the rule declares no install commands and
    /// states a fix hint instead.
    const TOOLCHAIN_COMPONENT_RULE: &str = "missing-docs-rust";

    /// The command a person runs when clippy is not on PATH.
    const CLIPPY_COMPONENT_FIX: &str = "rustup component add clippy";

    /// A doctor check command that always fails, standing in for a tool that is
    /// not on PATH.
    const TOOL_OFF_PATH_CHECK: &str = "exit 1";

    /// The doctor facts for one shipped builtin tool rule, with its tool forced
    /// off PATH.
    ///
    /// Everything the row reports — the fix hint, the install commands, the
    /// superseded prompt rule — comes from the shipped rule file, through the
    /// same [`check_tool_rule`] production uses. Only the doctor check command
    /// is replaced, because a test cannot uninstall the host's clippy.
    fn builtin_rule_with_tool_off_path(set_name: &str, rule_name: &str) -> ToolRuleStatus {
        let mut loader = ValidatorLoader::new();
        crate::load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset(set_name)
            .unwrap_or_else(|| panic!("the builtin {set_name} set must load"));
        let rule = ruleset
            .rules
            .iter()
            .find(|rule| rule.name == rule_name)
            .unwrap_or_else(|| panic!("the builtin {rule_name} rule must load"));
        let mut spec = rule
            .tool
            .clone()
            .unwrap_or_else(|| panic!("{rule_name} must carry a tool block"));
        spec.doctor
            .as_mut()
            .unwrap_or_else(|| panic!("{rule_name} must declare a doctor block"))
            .check_command = TOOL_OFF_PATH_CHECK.to_string();

        check_tool_rule(ruleset, rule, &spec)
    }

    /// The doctor rows for one tool rule on its own.
    fn checks_for(rule: ToolRuleStatus) -> Vec<Check> {
        to_checks(&ReviewEngineStatus {
            project_types: RUST_TYPES.iter().copied().map(String::from).collect(),
            sets: Vec::new(),
            tool_rules: vec![rule],
        })
    }

    /// With clippy off PATH the shipped `missing-docs-rust` row must name the
    /// command a person runs. The rule declares no install commands — a
    /// `rustup` component has no package version to pin — so the fix comes from
    /// its `doctor.fix_hint`.
    #[test]
    fn test_toolchain_component_row_names_its_fix_hint() {
        let rule = builtin_rule_with_tool_off_path(BUILTIN_TOOL_SET, TOOLCHAIN_COMPONENT_RULE);
        assert!(
            rule.install_commands.is_empty(),
            "{TOOLCHAIN_COMPONENT_RULE} must declare no install commands; got {:?}",
            rule.install_commands
        );

        let checks = checks_for(rule);

        let tool_row = checks
            .iter()
            .find(|c| c.name == tool_rule_check_name(BUILTIN_TOOL_SET, TOOLCHAIN_COMPONENT_RULE))
            .expect("tool rule row");
        assert_eq!(tool_row.status, CheckStatus::Warning);
        let fix = tool_row
            .fix
            .as_deref()
            .expect("a degraded row must carry a fix");
        assert!(
            fix.contains(CLIPPY_COMPONENT_FIX),
            "the row must name the command a person runs; got '{fix}'"
        );
    }

    /// A rule that declares both reports its install command: the engine can
    /// run that, so the hint stays the fallback for a rule with nothing to run.
    #[test]
    fn test_install_commands_win_over_a_fix_hint() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(
            temp.path(),
            "tool-set",
            TOOL_SET_MANIFEST,
            &[("gone-check.md", MISSING_TOOL_RULE)],
            &[
                ("gone-check.fail.txt", "irrelevant\n"),
                ("gone-check.pass.txt", "irrelevant\n"),
            ],
        );
        let loader = loader_for(temp.path());
        let status = check_review_engine_with(&loader, RUST_TYPES, None);

        let checks = to_checks(&status);

        let tool_row = checks
            .iter()
            .find(|c| c.name == tool_rule_check_name("tool-set", "gone-check"))
            .expect("tool rule row");
        let fix = tool_row
            .fix
            .as_deref()
            .expect("a degraded row must carry a fix");
        let rule = &status.tool_rules[0];
        assert!(
            fix.contains(&rule.install_commands[0]),
            "the fix must carry the install command; got '{fix}'"
        );
        let hint = rule.fix_hint.as_ref().expect("the rule states a fix hint");
        assert!(
            !fix.contains(&hint.to_string()),
            "an installable tool shows its install command, not its hint; got '{fix}'"
        );
    }

    /// A rule with no fixture pair asks for the fixture pair, whatever else it
    /// declares — a fix hint never displaces that.
    #[test]
    fn test_missing_fixtures_outrank_a_fix_hint() {
        let rule = ToolRuleStatus {
            fixtures: FixtureOutcome::MissingFixtures {
                detail: "no fixture pair".to_string(),
            },
            ..builtin_rule_with_tool_off_path(BUILTIN_TOOL_SET, TOOLCHAIN_COMPONENT_RULE)
        };

        let checks = checks_for(rule);

        let tool_row = checks
            .iter()
            .find(|c| c.name == tool_rule_check_name(BUILTIN_TOOL_SET, TOOLCHAIN_COMPONENT_RULE))
            .expect("tool rule row");
        let fix = tool_row
            .fix
            .as_deref()
            .expect("a degraded row must carry a fix");
        assert!(
            fix.contains(FIXTURES_DIR_NAME),
            "a rule with no fixture pair must be asked for one; got '{fix}'"
        );
        assert!(
            !fix.contains(CLIPPY_COMPONENT_FIX),
            "the fixture fix must not be replaced by the fix hint; got '{fix}'"
        );
    }
}
