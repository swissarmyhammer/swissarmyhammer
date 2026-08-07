//! The install lifecycle for a tool rule's tool.
//!
//! The contract is the "Install lifecycle" section of
//! `builtin/validators/README.md`. When a review needs a tool rule and the tool
//! is missing:
//!
//! 1. Doctor runs `check_command`. If it passes, the tool rule is ready.
//! 2. The engine tries each entry in `install.commands`, in order. After each
//!    try, doctor runs again.
//! 3. If every command fails, an install agent gets the rule, the platform, and
//!    the error output. The agent has one goal: make `check_command` pass.
//!    Doctor confirms the result — the agent cannot assert success.
//! 4. If the tool is still missing, the superseded prompt rule runs instead, and
//!    doctor keeps a warning. A missing tool degrades the review. It never
//!    blocks the review.
//!
//! Steps 1 and 2 are [`install_tool_commands`]: deterministic, no LLM, so `sah
//! init` pre-installs the runner tools through the same code the review runs.
//! Step 3 is [`ensure_tool_installed`], which adds one bounded agent turn on top.
//! Step 4 needs no code here — [`install_missing_tools`] runs before
//! [`plan_tool_rules`](crate::review::tool_rules::plan_tool_rules), and the
//! planner re-runs the SAME doctor check, so a tool this module installed is
//! planned as healthy and a tool it could not install falls back on its own.
//!
//! Presence is decided by [`check_presence`](crate::doctor) throughout — the one
//! function doctor itself uses — so "installed" can never mean two things.
//!
//! Every command this module runs comes from `install.commands`. A rule's
//! `doctor.fix_hint` is text a person reads, and it is a
//! [`FixHint`](crate::validators::types::FixHint) rather than a command string,
//! so no step of the lifecycle can run it.

use futures::future::BoxFuture;
use regex::Regex;

use crate::doctor::{check_presence, command_failure_detail, run_shell, ToolPresence};
use crate::error::AvpError;
use crate::review::scope::WorkList;
use crate::review::tool_rules::matched_tool_rules;
use crate::validators::types::ToolSpec;
use crate::validators::{AgentPool, ValidatorLoader};

/// What an install command that exited 0 reported, for the attempt log.
const INSTALL_COMMAND_SUCCEEDED: &str = "the command exited 0";

/// The version-pin shapes [`install_command_pins_version`] accepts.
///
/// A pin is a version with at least one dot bound to the package by one of the
/// conventional separators — `pkg==1.2.3` (pip/uv), `pkg@1.2.3` (npm/go/brew),
/// `pkg:1.2.3` (container tags), `--version 1.2.3` (cargo), or `-v 1.2.3`
/// (gem). A bare major (`python@3`) is not a pin: the tool can still change its
/// rules underneath the gate.
const VERSION_PIN_PATTERN: &str = r"(?x)
      [=@:]\s*v?\d+(\.\d+)+
    | --version[=\s]\s*v?\d+(\.\d+)+
    | (^|\s)-v[=\s]\s*v?\d+(\.\d+)+
";

/// One install command that was tried, and what it reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallAttempt {
    /// The command that was run.
    command: String,
    /// Its stderr, or its exit status when it printed nothing.
    detail: String,
}

impl InstallAttempt {
    /// The command that was run.
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Its stderr, or its exit status when it printed nothing.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// What the install lifecycle achieved for one tool rule's tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolInstallOutcome {
    /// `check_command` already passed. Nothing was run.
    AlreadyPresent,

    /// An entry in `install.commands` made `check_command` pass.
    Installed {
        /// The command that worked.
        command: String,
    },

    /// The install agent made `check_command` pass after every command failed.
    InstalledByAgent,

    /// The tool is still missing. The superseded prompt rule runs instead.
    Failed {
        /// Every install command that was tried, in order.
        attempts: Vec<InstallAttempt>,
    },
}

impl ToolInstallOutcome {
    /// Whether the tool is usable now, however it got there.
    pub fn tool_present(&self) -> bool {
        !matches!(self, ToolInstallOutcome::Failed { .. })
    }
}

/// What the install agent is told: the rule, the platform, the goal, and every
/// error the deterministic commands produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallAgentRequest {
    /// The tool rule that needs the tool, as `<set>/<rule>`.
    rule: String,
    /// The command that proves the tool is installed.
    check_command: String,
    /// The host the tool must be installed on, as `<os>/<arch>`.
    platform: String,
    /// Every install command the engine already tried, in order.
    attempts: Vec<InstallAttempt>,
}

impl InstallAgentRequest {
    /// Build the request for `rule`, targeting the running host.
    pub fn new(rule: &str, check_command: &str, attempts: Vec<InstallAttempt>) -> Self {
        Self {
            rule: rule.to_string(),
            check_command: check_command.to_string(),
            platform: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
            attempts,
        }
    }

    /// The tool rule that needs the tool, as `<set>/<rule>`.
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// The command that proves the tool is installed.
    pub fn check_command(&self) -> &str {
        &self.check_command
    }

    /// The host the tool must be installed on, as `<os>/<arch>`.
    pub fn platform(&self) -> &str {
        &self.platform
    }

    /// Every install command the engine already tried, in order.
    pub fn attempts(&self) -> &[InstallAttempt] {
        &self.attempts
    }

    /// The prompt the bounded install agent receives.
    ///
    /// It names the goal as a command to make pass, never as a claim to report:
    /// the engine re-runs `check_command` itself afterwards.
    pub fn render_prompt(&self) -> String {
        let tried = if self.attempts.is_empty() {
            "The rule declares no install commands, so none were tried.".to_string()
        } else {
            let lines: Vec<String> = self
                .attempts
                .iter()
                .enumerate()
                .map(|(index, attempt)| {
                    format!(
                        "{n}. `{command}` — {detail}",
                        n = index + 1,
                        command = attempt.command,
                        detail = attempt.detail
                    )
                })
                .collect();
            format!(
                "These install commands were tried in order and all failed:\n\n{}",
                lines.join("\n")
            )
        };

        format!(
            "A code-review tool rule needs a command-line tool that is not installed \
on this machine.\n\n\
Tool rule: {rule}\n\
Platform: {platform}\n\
Goal: make this command exit 0 — `{check}`\n\n\
{tried}\n\n\
Install the tool with the package manager this platform provides. Change no \
file in the repository. Do not report success: the engine runs `{check}` \
itself, and that result is the only proof.",
            rule = self.rule,
            platform = self.platform,
            check = self.check_command,
        )
    }
}

/// A bounded agent that can install a missing tool.
///
/// The agent's answer is a transcript, never a verdict: [`ensure_tool_installed`]
/// re-runs the doctor check afterwards and that check alone decides.
pub trait ToolInstallAgent: Send + Sync {
    /// Run one bounded agent turn against `request` and return its transcript.
    ///
    /// # Errors
    ///
    /// Returns an [`AvpError`] when the turn could not be run at all. A turn
    /// that ran and helped nothing is `Ok` — the doctor check decides.
    fn install<'a>(
        &'a self,
        request: &'a InstallAgentRequest,
    ) -> BoxFuture<'a, Result<String, AvpError>>;
}

/// The production install agent: one bounded turn on the review's shared
/// [`AgentPool`](crate::validators::AgentPool).
///
/// Bounded by the pool itself — one turn, the pool's per-call token cap, its
/// idle window, and its absolute turn ceiling. The engine never loops on it.
pub struct PoolInstallAgent<'p> {
    /// The shared pool every review stage submits to.
    pool: &'p AgentPool,
}

impl std::fmt::Debug for PoolInstallAgent<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoolInstallAgent")
            .field("workers", &self.pool.worker_count())
            .finish()
    }
}

impl<'p> PoolInstallAgent<'p> {
    /// Build an install agent over the review's shared pool.
    pub fn new(pool: &'p AgentPool) -> Self {
        Self { pool }
    }
}

impl ToolInstallAgent for PoolInstallAgent<'_> {
    fn install<'a>(
        &'a self,
        request: &'a InstallAgentRequest,
    ) -> BoxFuture<'a, Result<String, AvpError>> {
        Box::pin(async move {
            match self.pool.submit(request.render_prompt()).await {
                Ok(Ok(response)) => Ok(response.content),
                Ok(Err(e)) => Err(AvpError::Agent(e.to_string())),
                Err(_) => Err(AvpError::Agent(
                    "the install agent turn was dropped before it answered".to_string(),
                )),
            }
        })
    }
}

/// Run the deterministic half of the install lifecycle for one tool.
///
/// Returns [`ToolInstallOutcome::AlreadyPresent`] without running anything when
/// the doctor check already passes. Otherwise it runs each entry in
/// `install.commands` in order and re-runs the doctor check after each, stopping
/// at the first command that makes the check pass. A command that exits 0 and
/// leaves the check failing is not a success — the check is the only proof.
///
/// No LLM and no agent: `sah init` pre-installs runner tools through this exact
/// function.
pub fn install_tool_commands(spec: &ToolSpec) -> ToolInstallOutcome {
    if matches!(check_presence(spec), ToolPresence::Present) {
        return ToolInstallOutcome::AlreadyPresent;
    }

    let commands = spec
        .install
        .as_ref()
        .map(|install| install.commands.as_slice())
        .unwrap_or_default();

    let mut attempts = Vec::with_capacity(commands.len());
    for command in commands {
        attempts.push(run_install_command(command));
        if matches!(check_presence(spec), ToolPresence::Present) {
            return ToolInstallOutcome::Installed {
                command: command.clone(),
            };
        }
    }

    ToolInstallOutcome::Failed { attempts }
}

/// Run one install command and record what it reported.
fn run_install_command(command: &str) -> InstallAttempt {
    let detail = match run_shell(command, None, &[]) {
        Ok(output) if output.status.success() => INSTALL_COMMAND_SUCCEEDED.to_string(),
        Ok(output) => command_failure_detail(&output),
        Err(e) => format!("the install command failed to start: {e}"),
    };
    InstallAttempt {
        command: command.to_string(),
        detail,
    }
}

/// Run the whole install lifecycle for one tool: the deterministic commands,
/// then one bounded agent turn when every command failed.
///
/// `rule` is the tool rule's `<set>/<rule>` label, which rides into the agent
/// prompt. `agent` is `None` on the surfaces that must not spend an LLM turn
/// (`sah init`), which stops the lifecycle at its deterministic half.
///
/// The agent cannot assert success: whatever it answers, the doctor check runs
/// again and decides.
pub async fn ensure_tool_installed(
    rule: &str,
    spec: &ToolSpec,
    agent: Option<&dyn ToolInstallAgent>,
) -> ToolInstallOutcome {
    let attempts = match install_tool_commands(spec) {
        ToolInstallOutcome::Failed { attempts } => attempts,
        installed => return installed,
    };

    let (Some(agent), Some(doctor)) = (agent, spec.doctor.as_ref()) else {
        return ToolInstallOutcome::Failed { attempts };
    };

    let request = InstallAgentRequest::new(rule, &doctor.check_command, attempts.clone());
    match agent.install(&request).await {
        Ok(transcript) => tracing::info!(
            rule = %rule,
            transcript = %transcript,
            "the install agent finished its turn; the doctor check decides"
        ),
        Err(e) => tracing::warn!(
            rule = %rule,
            error = %e,
            "the install agent turn could not be run"
        ),
    }

    if matches!(check_presence(spec), ToolPresence::Present) {
        ToolInstallOutcome::InstalledByAgent
    } else {
        ToolInstallOutcome::Failed { attempts }
    }
}

/// One tool rule's install result, for the run's install-stage report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRuleInstall {
    /// The owning validator set.
    set_name: String,
    /// The tool rule's name.
    rule_name: String,
    /// What the lifecycle achieved.
    outcome: ToolInstallOutcome,
}

impl ToolRuleInstall {
    /// The owning validator set.
    pub fn set_name(&self) -> &str {
        &self.set_name
    }

    /// The tool rule's name.
    pub fn rule_name(&self) -> &str {
        &self.rule_name
    }

    /// What the lifecycle achieved.
    pub fn outcome(&self) -> &ToolInstallOutcome {
        &self.outcome
    }
}

/// Run the install lifecycle for every tool rule the work-list matched.
///
/// Matching is [`matched_tool_rules`], the same pass
/// [`plan_tool_rules`](crate::review::tool_rules::plan_tool_rules) uses, so the
/// engine can never install a tool for a rule it will not run. A tool that is
/// already present costs one doctor check and nothing else.
///
/// Returns one row per matched tool rule, in match order.
pub async fn install_missing_tools(
    work: &WorkList,
    loader: &ValidatorLoader,
    project_types: &[&str],
    agent: Option<&dyn ToolInstallAgent>,
) -> Vec<ToolRuleInstall> {
    let matched = matched_tool_rules(work, loader, project_types);
    let mut installs = Vec::with_capacity(matched.len());
    for rule in matched {
        let set_name = rule.ruleset.name().to_string();
        let label = format!("{set_name}/{rule_name}", rule_name = rule.rule.name);
        let outcome = ensure_tool_installed(&label, rule.spec, agent).await;
        if !outcome.tool_present() {
            tracing::warn!(
                rule = %label,
                outcome = ?outcome,
                "the tool is still missing; the superseded prompt rule runs instead"
            );
        }
        installs.push(ToolRuleInstall {
            set_name,
            rule_name: rule.rule.name.clone(),
            outcome,
        });
    }
    installs
}

/// Pre-install the tool of every tool rule the loader declares for
/// `project_types`.
///
/// The install-time counterpart of [`install_missing_tools`]: `sah init` has no
/// work-list, so it covers every tool rule that serves the detected project
/// types — the same set
/// [`check_review_engine_with`](crate::doctor::check_review_engine_with)
/// reports, through the same selection pass. It is the `_with` core the doctor
/// pattern asks for: the caller resolves the loader and the project types (with
/// [`detected_project_type_keys`](crate::review::scope::detected_project_type_keys)),
/// so a test never depends on the host's validator directories.
///
/// Deterministic by construction: it runs [`install_tool_commands`] and never
/// spends an agent turn, so `sah init` stays LLM-free.
///
/// Returns one row per tool rule, in set order.
pub fn install_project_tool_rules(
    loader: &ValidatorLoader,
    project_types: &[&str],
) -> Vec<ToolRuleInstall> {
    crate::review::tool_rules::project_tool_rules(loader, project_types)
        .into_iter()
        .map(|matched| ToolRuleInstall {
            set_name: matched.ruleset.name().to_string(),
            rule_name: matched.rule.name.clone(),
            outcome: install_tool_commands(matched.spec),
        })
        .collect()
}

/// Whether an install command pins the tool's version.
///
/// An unpinned tool can change its rules between runs and break the gate, so
/// every builtin tool rule's install commands must pin. The accepted shapes are
/// [`VERSION_PIN_PATTERN`].
pub fn install_command_pins_version(command: &str) -> bool {
    // The pattern is a compile-time constant, so it always compiles.
    Regex::new(VERSION_PIN_PATTERN)
        .expect("VERSION_PIN_PATTERN is a valid regex")
        .is_match(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::{Path, PathBuf};

    use crate::review::test_support::{with_pool, ScriptedAgent, ScriptedReply};
    use crate::validators::types::{FixHint, ToolDoctor, ToolInstall, ToolScope};
    use crate::validators::PoolConfig;

    /// The prompt line the scripted install agent keys on. It is the goal
    /// sentence [`InstallAgentRequest::render_prompt`] always writes, so the
    /// script matches the real prompt rather than a copy of it.
    const INSTALL_PROMPT_GOAL_LINE: &str = "Goal: make this command exit 0";

    /// A tool spec whose doctor check passes only once `marker` exists, and
    /// whose install commands are `commands`.
    ///
    /// The check is real bash through the engine's own runner, so every test
    /// below proves the lifecycle against the same presence decision doctor
    /// makes in production.
    fn marker_spec(marker: &Path, commands: &[String]) -> ToolSpec {
        ToolSpec {
            scope: ToolScope::Files,
            run: "true".to_string(),
            doctor: Some(ToolDoctor {
                check_command: format!("test -f {}", shell_quote(marker)),
                check_version_command: None,
                fix_hint: None,
            }),
            install: Some(ToolInstall {
                commands: commands.to_vec(),
            }),
        }
    }

    /// A shell command that creates `marker` — a stand-in for a real install.
    fn create_marker(marker: &Path) -> String {
        format!("touch {}", shell_quote(marker))
    }

    /// Single-quote a path for bash so a temp dir with a space cannot break the
    /// scripts these tests build.
    fn shell_quote(path: &Path) -> String {
        format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
    }

    /// A marker path under `dir` that no test has created yet.
    fn marker_in(dir: &Path, name: &str) -> PathBuf {
        dir.join(name)
    }

    /// A command that always fails, so the doctor check stays failing.
    const FAILING_COMMAND: &str = "echo 'no such package' >&2; exit 1";

    #[test]
    fn a_present_tool_runs_no_install_command() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "present");
        std::fs::write(&marker, "").expect("write marker");
        let forbidden = marker_in(temp.path(), "forbidden");
        let spec = marker_spec(&marker, &[create_marker(&forbidden)]);

        let outcome = install_tool_commands(&spec);

        assert_eq!(outcome, ToolInstallOutcome::AlreadyPresent);
        assert!(
            !forbidden.exists(),
            "a present tool must not run any install command"
        );
    }

    #[test]
    fn the_first_command_that_makes_the_check_pass_wins_and_later_ones_never_run() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "tool");
        let never = marker_in(temp.path(), "never");
        let working = create_marker(&marker);
        let spec = marker_spec(
            &marker,
            &[
                FAILING_COMMAND.to_string(),
                working.clone(),
                create_marker(&never),
            ],
        );

        let outcome = install_tool_commands(&spec);

        assert_eq!(outcome, ToolInstallOutcome::Installed { command: working });
        assert!(
            !never.exists(),
            "a command after the one that worked must never run"
        );
    }

    #[test]
    fn every_failing_command_is_reported_in_order() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "tool");
        let spec = marker_spec(
            &marker,
            &[
                "echo first-failed >&2; exit 1".to_string(),
                "echo second-failed >&2; exit 1".to_string(),
            ],
        );

        let outcome = install_tool_commands(&spec);

        let ToolInstallOutcome::Failed { attempts } = outcome else {
            panic!("every command failed, so the outcome must be Failed");
        };
        let details: Vec<&str> = attempts.iter().map(InstallAttempt::detail).collect();
        assert_eq!(details, ["first-failed", "second-failed"]);
        let commands: Vec<&str> = attempts.iter().map(InstallAttempt::command).collect();
        assert_eq!(
            commands,
            [
                "echo first-failed >&2; exit 1",
                "echo second-failed >&2; exit 1"
            ]
        );
    }

    #[test]
    fn a_command_that_exits_zero_without_installing_is_not_a_success() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "tool");
        let spec = marker_spec(&marker, &["true".to_string()]);

        let outcome = install_tool_commands(&spec);

        let ToolInstallOutcome::Failed { attempts } = outcome else {
            panic!("the doctor check is the only proof, so a bare exit 0 must not pass");
        };
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].detail(), INSTALL_COMMAND_SUCCEEDED);
    }

    /// An install agent that runs `script` — the shape of a real agent that
    /// actually installs something — and then answers `answer`.
    struct ScriptedInstallAgent {
        /// The shell script the agent runs, standing in for its tool calls.
        script: String,
        /// What the agent says afterwards. Never trusted.
        answer: String,
    }

    impl ToolInstallAgent for ScriptedInstallAgent {
        fn install<'a>(
            &'a self,
            _request: &'a InstallAgentRequest,
        ) -> BoxFuture<'a, Result<String, AvpError>> {
            Box::pin(async move {
                run_shell(&self.script, None, &[]).map_err(|e| AvpError::Agent(e.to_string()))?;
                Ok(self.answer.clone())
            })
        }
    }

    #[tokio::test]
    async fn the_agent_runs_only_after_every_command_failed_and_doctor_confirms_it() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "tool");
        let spec = marker_spec(&marker, &[FAILING_COMMAND.to_string()]);
        let agent = ScriptedInstallAgent {
            script: create_marker(&marker),
            answer: "installed it".to_string(),
        };

        let outcome = ensure_tool_installed("tool-set/todo-check", &spec, Some(&agent)).await;

        assert_eq!(outcome, ToolInstallOutcome::InstalledByAgent);
        assert!(outcome.tool_present());
    }

    #[tokio::test]
    async fn the_agent_cannot_assert_success() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "tool");
        let spec = marker_spec(&marker, &[FAILING_COMMAND.to_string()]);
        let agent = ScriptedInstallAgent {
            script: "true".to_string(),
            answer: "I successfully installed the tool.".to_string(),
        };

        let outcome = ensure_tool_installed("tool-set/todo-check", &spec, Some(&agent)).await;

        let ToolInstallOutcome::Failed { attempts } = outcome else {
            panic!("the agent claimed success but the doctor check still fails");
        };
        assert_eq!(attempts.len(), 1, "the failed command stays in the report");
    }

    #[tokio::test]
    async fn without_an_agent_the_lifecycle_stops_at_its_deterministic_half() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "tool");
        let spec = marker_spec(&marker, &[FAILING_COMMAND.to_string()]);

        let outcome = ensure_tool_installed("tool-set/todo-check", &spec, None).await;

        assert!(matches!(outcome, ToolInstallOutcome::Failed { .. }));
    }

    #[tokio::test]
    async fn an_already_present_tool_never_reaches_the_agent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "present");
        std::fs::write(&marker, "").expect("write marker");
        let forbidden = marker_in(temp.path(), "forbidden");
        let spec = marker_spec(&marker, &[]);
        let agent = ScriptedInstallAgent {
            script: create_marker(&forbidden),
            answer: "never asked".to_string(),
        };

        let outcome = ensure_tool_installed("tool-set/todo-check", &spec, Some(&agent)).await;

        assert_eq!(outcome, ToolInstallOutcome::AlreadyPresent);
        assert!(
            !forbidden.exists(),
            "a present tool must never reach the install agent"
        );
    }

    /// The production agent submits a real turn to a real [`AgentPool`] over a
    /// real ACP connection, and its answer still decides nothing: the doctor
    /// check does. This covers the wiring an in-test agent cannot.
    #[tokio::test]
    async fn the_pool_install_agent_runs_a_real_turn_whose_claim_still_loses_to_doctor() {
        let script = vec![(
            INSTALL_PROMPT_GOAL_LINE.to_string(),
            ScriptedReply::Text("Done — the tool is installed.".to_string()),
        )];
        let agent = ScriptedAgent::new(script);

        with_pool(agent, PoolConfig::local(), move |pool| async move {
            let temp = tempfile::tempdir().expect("temp dir");
            let marker = marker_in(temp.path(), "tool");
            let spec = marker_spec(&marker, &[FAILING_COMMAND.to_string()]);
            let installer = PoolInstallAgent::new(&pool);

            let outcome =
                ensure_tool_installed("tool-set/todo-check", &spec, Some(&installer)).await;

            assert!(
                matches!(outcome, ToolInstallOutcome::Failed { .. }),
                "the agent answered over a real pool turn, but the doctor check still fails; got {outcome:?}"
            );
        })
        .await;
    }

    #[test]
    fn the_agent_prompt_carries_the_rule_the_platform_the_goal_and_every_error() {
        let attempts = vec![InstallAttempt {
            command: "brew install ruff@0.14.3".to_string(),
            detail: "No available formula".to_string(),
        }];
        let request =
            InstallAgentRequest::new("code-hygiene/missing-docs-rust", "which ruff", attempts);

        let prompt = request.render_prompt();

        assert!(prompt.contains("code-hygiene/missing-docs-rust"));
        assert!(prompt.contains(std::env::consts::OS));
        assert!(prompt.contains("which ruff"));
        assert!(prompt.contains("brew install ruff@0.14.3"));
        assert!(prompt.contains("No available formula"));
        assert!(
            prompt.contains("Do not report success"),
            "the prompt must tell the agent it cannot assert success; got '{prompt}'"
        );
    }

    #[test]
    fn the_agent_prompt_says_so_when_the_rule_declares_no_install_commands() {
        let request = InstallAgentRequest::new("tool-set/todo-check", "which grep", Vec::new());

        let prompt = request.render_prompt();

        assert!(prompt.contains("declares no install commands"));
    }

    #[test]
    fn a_pinned_install_command_is_accepted() {
        for command in [
            "uv tool install ruff==0.14.3",
            "pipx install ruff==0.14.3",
            "brew install ruff@0.14.3",
            "npm install -g typescript@5.9.2",
            "cargo install cargo-deny --version 0.18.5",
            "gem install rubocop -v 1.81.1",
            "docker pull ruff:0.14.3",
        ] {
            assert!(
                install_command_pins_version(command),
                "`{command}` pins a version"
            );
        }
    }

    #[test]
    fn an_unpinned_install_command_is_rejected() {
        for command in [
            "uv tool install ruff",
            "brew install ruff",
            "npm install -g typescript",
            "cargo install cargo-deny",
            "brew install python@3",
        ] {
            assert!(
                !install_command_pins_version(command),
                "`{command}` does not pin a version"
            );
        }
    }

    /// A fix hint is text for a person, never a command the engine runs.
    ///
    /// The lifecycle reads its commands from `install.commands` alone, and a
    /// hint is a [`FixHint`] on the doctor block — not a command string at all.
    /// This drives both halves at once: the deterministic commands and the
    /// bounded agent turn.
    #[tokio::test]
    async fn the_install_lifecycle_never_runs_a_fix_hint() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "tool");
        let hint_ran = marker_in(temp.path(), "hint-ran");
        let mut spec = marker_spec(&marker, &[]);
        spec.doctor
            .as_mut()
            .expect("marker_spec declares a doctor block")
            .fix_hint = Some(FixHint::from(create_marker(&hint_ran)));
        let agent = ScriptedInstallAgent {
            script: "true".to_string(),
            answer: "nothing to do".to_string(),
        };

        let outcome = ensure_tool_installed("tool-set/hinted-check", &spec, Some(&agent)).await;

        assert!(matches!(outcome, ToolInstallOutcome::Failed { .. }));
        assert!(
            !hint_ran.exists(),
            "a fix hint is text for a person; the install lifecycle must never run it"
        );
    }

    /// Every builtin tool rule's [`ToolSpec`], labeled `<set>/<rule>`.
    fn builtin_tool_specs() -> Vec<(String, ToolSpec)> {
        let mut loader = ValidatorLoader::new();
        crate::load_builtins(&mut loader);

        let mut specs = Vec::new();
        for ruleset in loader.list_rulesets() {
            for rule in &ruleset.rules {
                if let Some(spec) = rule.tool.as_ref() {
                    specs.push((format!("{}/{}", ruleset.name(), rule.name), spec.clone()));
                }
            }
        }
        specs
    }

    /// Every builtin tool rule must pin the version in each install command —
    /// an unpinned tool can change its rules between runs and break the gate.
    /// The guard must also have seen a real command, or it proves nothing.
    #[test]
    fn every_builtin_tool_rule_pins_its_install_commands() {
        let mut unpinned = Vec::new();
        let mut guarded = 0usize;
        for (label, spec) in builtin_tool_specs() {
            let Some(install) = spec.install.as_ref() else {
                continue;
            };
            for command in &install.commands {
                guarded += 1;
                if !install_command_pins_version(command) {
                    unpinned.push(format!("{label}: {command}"));
                }
            }
        }

        assert!(
            unpinned.is_empty(),
            "these builtin install commands do not pin a version: {unpinned:?}"
        );
        assert!(
            guarded > 0,
            "the guard saw no builtin install command at all, so it guards nothing"
        );
    }

    /// A tool that ships with the language toolchain has no package version to
    /// pin, so its rule states a fix hint instead of an install command. Every
    /// builtin hint fails the pin guard — which is exactly why it is a hint.
    #[test]
    fn a_builtin_fix_hint_would_not_pass_the_pin_guard() {
        let hints: Vec<(String, String)> = builtin_tool_specs()
            .into_iter()
            .filter_map(|(label, spec)| {
                spec.doctor
                    .and_then(|doctor| doctor.fix_hint)
                    .map(|hint| (label, hint.to_string()))
            })
            .collect();

        assert!(
            !hints.is_empty(),
            "at least one builtin tool rule must state a fix hint"
        );
        for (label, hint) in hints {
            assert!(
                !install_command_pins_version(&hint),
                "{label} states `{hint}` as a fix hint; a string that pins a version \
                 belongs in install.commands, where the lifecycle can run it"
            );
        }
    }
}
