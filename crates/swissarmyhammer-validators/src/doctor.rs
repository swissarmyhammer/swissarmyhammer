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
//!   — with the `install.commands` to fix it.
//!
//! Fixture checks run the rule's `run` script against the set's
//! `fixtures/<rule>.fail.*` and `fixtures/<rule>.pass.*` files: the fail
//! fixture must produce at least one finding, the pass fixture none. A rule
//! that fails its fixtures is reported and not used.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Output;

use swissarmyhammer_doctor::{Check, CheckStatus};

use crate::error::AvpError;
use crate::review::scope::detected_project_type_keys;
use crate::review::tool_output::parse_tool_stdout;
use crate::validators::types::{Rule, RuleSet, ToolScope, ToolSpec, ValidatorSource};
use crate::validators::ValidatorLoader;

/// The check name for the detected project types row.
pub const PROJECT_TYPES_CHECK_NAME: &str = "Validator Project Types";

/// The number of always-included rows [`to_checks`] emits — the one detected
/// project-types row.
const PROJECT_TYPES_ROWS: usize = 1;

/// The directory inside a validator set that carries tool-rule fixtures.
const FIXTURES_DIR_NAME: &str = "fixtures";

/// The fixture that must make the tool report at least one finding.
const FAIL_FIXTURE_KIND: &str = "fail";

/// The fixture that must make the tool report zero findings.
const PASS_FIXTURE_KIND: &str = "pass";

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

    /// The prompt rule this tool rule replaces when healthy.
    pub supersedes: Option<String>,
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
/// # Errors
///
/// Returns an [`AvpError`] when the validator stack fails to load.
pub fn check_review_engine(workspace_root: &Path) -> Result<ReviewEngineStatus, AvpError> {
    let loader = crate::load_rules()?;
    let project_types = detected_project_type_keys(workspace_root);
    Ok(check_review_engine_with(&loader, &project_types))
}

/// Produce the review-engine facts from an explicit loader and detected
/// project types.
///
/// This is the injectable core of [`check_review_engine`]: tests drive it
/// with a synthetic loader and type list, without depending on the host's
/// validator directories or workspace.
pub fn check_review_engine_with(
    loader: &ValidatorLoader,
    project_types: &[String],
) -> ReviewEngineStatus {
    let mut rulesets = loader.list_rulesets();
    rulesets.sort_by(|a, b| a.name().cmp(b.name()));

    let mut sets = Vec::with_capacity(rulesets.len());
    let mut tool_rules = Vec::new();
    for ruleset in rulesets {
        let applies = criteria_applies(ruleset.manifest.match_criteria.as_ref(), project_types);
        sets.push(SetStatus {
            name: ruleset.name().to_string(),
            source: ruleset.source.clone(),
            applies,
        });
        if !applies {
            continue;
        }

        for rule in &ruleset.rules {
            let Some(spec) = &rule.tool else {
                continue;
            };
            if !criteria_applies(rule.match_criteria.as_ref(), project_types) {
                continue;
            }
            tool_rules.push(check_tool_rule(ruleset, rule, spec));
        }
    }

    ReviewEngineStatus {
        project_types: project_types.to_vec(),
        sets,
        tool_rules,
    }
}

/// Whether an optional match criteria's project-type constraint fits the
/// detected project types. Absent criteria applies everywhere.
fn criteria_applies(
    criteria: Option<&crate::validators::types::ValidatorMatch>,
    project_types: &[String],
) -> bool {
    criteria.is_none_or(|c| c.project_types_match(project_types))
}

/// Produce the doctor facts for one tool rule: presence, version, fixtures.
///
/// Crate-visible so the review engine's tool-rule planner
/// ([`crate::review::tool_rules`]) reuses the same health decision doctor
/// reports — "healthy" can never mean two different things.
pub(crate) fn check_tool_rule(ruleset: &RuleSet, rule: &Rule, spec: &ToolSpec) -> ToolRuleStatus {
    let presence = check_presence(spec);
    let (version, fixtures) = match &presence {
        ToolPresence::Present => (check_version(spec), check_fixtures(ruleset, rule, spec)),
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
        supersedes: rule.supersedes.clone(),
    }
}

/// Run `doctor.check_command` to decide whether the tool is installed.
///
/// A rule without a doctor block is treated as present — the fixture run is
/// then the only health evidence.
fn check_presence(spec: &ToolSpec) -> ToolPresence {
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
fn check_fixtures(ruleset: &RuleSet, rule: &Rule, spec: &ToolSpec) -> FixtureOutcome {
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
    let fixtures_dir = ruleset.base_path.join(FIXTURES_DIR_NAME);
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

/// Run the rule's script against one fixture file and count its findings.
///
/// The script runs with the fixture's directory as its working directory. A
/// `files`-scope script receives the fixture file name as `"$@"`; a
/// `workspace`-scope script runs with no arguments.
fn run_fixture(spec: &ToolSpec, fixture: &Path) -> Result<usize, String> {
    let fixture_dir = fixture.parent().unwrap_or_else(|| Path::new("."));
    let fixture_name = fixture
        .file_name()
        .ok_or_else(|| format!("fixture path has no file name: {}", fixture.display()))?;

    let args: Vec<&OsStr> = match spec.scope {
        ToolScope::Files => vec![fixture_name],
        ToolScope::Workspace => Vec::new(),
    };

    let output = run_shell(&spec.run, Some(fixture_dir), &args)
        .map_err(|e| format!("tool failed to run on {}: {e}", fixture_label(fixture)))?;
    if !output.status.success() {
        return Err(format!(
            "tool broke on {}: {}",
            fixture_label(fixture),
            command_failure_detail(&output),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let findings = parse_tool_stdout(&stdout).map_err(|e| {
        format!(
            "tool stdout on {} broke the contract: {e}",
            fixture_label(fixture)
        )
    })?;
    Ok(findings.len())
}

/// The fixture's file name for messages, falling back to the full path.
fn fixture_label(fixture: &Path) -> String {
    fixture
        .file_name()
        .and_then(OsStr::to_str)
        .map(str::to_string)
        .unwrap_or_else(|| fixture.display().to_string())
}

/// Summarize a failed command: its stderr when present, its exit status
/// otherwise.
pub(crate) fn command_failure_detail(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("exited with {}", output.status)
    } else {
        stderr
    }
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
/// every other degradation asks for the rule's install commands.
fn degraded_fix(rule: &ToolRuleStatus) -> Option<String> {
    match &rule.fixtures {
        FixtureOutcome::MissingFixtures { .. } => Some(format!(
            "Add {rule}.{FAIL_FIXTURE_KIND}.* and {rule}.{PASS_FIXTURE_KIND}.* to the set's {FIXTURES_DIR_NAME}/ directory",
            rule = rule.rule_name,
        )),
        _ => install_fix(rule),
    }
}

/// The "which prompt rule runs instead" suffix for degraded rows.
fn fallback_note(rule: &ToolRuleStatus) -> String {
    match &rule.supersedes {
        Some(prompt_rule) => format!("; prompt rule '{prompt_rule}' runs instead"),
        None => "; prompt fallback".to_string(),
    }
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
        "Install the tool: {}",
        rule.install_commands.join(" || ")
    ))
}

/// Run a tool-rule shell snippet the way the engine does: `bash -c <script>`
/// with `args` as the script's positional parameters (`"$@"`).
///
/// The ONE shell runner for tool-rule scripts — the doctor's fixture checks
/// and the review engine's tool runs ([`crate::review::tool_rules`]) both go
/// through it, so a script can never pass its fixtures under one shell and
/// run under another.
pub(crate) fn run_shell(
    script: &str,
    cwd: Option<&Path>,
    args: &[&OsStr],
) -> std::io::Result<Output> {
    let mut command = std::process::Command::new("bash");
    command.arg("-c").arg(script).arg("bash").args(args);
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

    /// A tool rule whose check command names a binary that cannot exist.
    const MISSING_TOOL_RULE: &str = r#"---
name: gone-check
description: A rule whose tool is not installed.
supersedes: missing-docs
tool:
  scope: files
  run: "definitely-not-a-real-tool-1f9c \"$@\""
  doctor:
    check_command: "definitely-not-a-real-tool-1f9c --version"
  install:
    commands:
      - "brew install definitely-not-a-real-tool-1f9c"
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

    const RUST_TYPES: &[&str] = &["rust"];

    fn rust_types() -> Vec<String> {
        RUST_TYPES.iter().map(|s| s.to_string()).collect()
    }

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
        let status = check_review_engine_with(&loader, &detected);
        assert_eq!(status.project_types, detected);
    }

    #[test]
    fn test_set_applicability_follows_project_types() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_ruleset(temp.path(), "plain-set", PLAIN_MANIFEST, &[], &[]);
        write_ruleset(temp.path(), "python-set", PYTHON_ONLY_MANIFEST, &[], &[]);
        let loader = loader_for(temp.path());

        let status = check_review_engine_with(&loader, &rust_types());

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

        let status = check_review_engine_with(&loader, &rust_types());

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

        let status = check_review_engine_with(&loader, &rust_types());

        assert_eq!(status.tool_rules.len(), 1, "one tool rule expected");
        let rule = &status.tool_rules[0];
        assert_eq!(rule.set_name, "tool-set");
        assert_eq!(rule.rule_name, "todo-check");
        assert_eq!(rule.presence, ToolPresence::Present);
        assert_eq!(rule.version.as_deref(), Some("tool 1.2.3"));
        assert_eq!(rule.fixtures, FixtureOutcome::Passed);
        assert_eq!(rule.supersedes.as_deref(), Some("missing-docs"));
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

        let status = check_review_engine_with(&loader, &rust_types());

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

        let status = check_review_engine_with(&loader, &rust_types());

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

        let status = check_review_engine_with(&loader, &rust_types());

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

        let status = check_review_engine_with(&loader, &rust_types());

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
        let status = check_review_engine_with(&loader, &rust_types());

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
        let status = check_review_engine_with(&loader, &rust_types());

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
            tool_row.message.contains("missing-docs"),
            "a missing tool row must name the prompt rule that runs instead; got '{}'",
            tool_row.message
        );
        let fix = tool_row.fix.as_deref().expect("install fix");
        assert!(
            fix.contains("brew install definitely-not-a-real-tool-1f9c"),
            "the fix must carry the install commands; got '{fix}'"
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
        let status = check_review_engine_with(&loader, &rust_types());

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
        let status = check_review_engine_with(&loader, &[]);

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
}
