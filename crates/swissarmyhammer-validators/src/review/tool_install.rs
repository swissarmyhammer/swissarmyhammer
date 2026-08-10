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
//!
//! Installs are serialized by [`InstallLock`], and the lock covers the whole
//! lifecycle — the declared commands and the agent turn alike. An install
//! command writes to a directory nothing else locks, and only `cargo install`
//! holds a lock of its own, so two installers that ran together could write one
//! destination at the same time. [`InstallLock`] states how far that
//! serialization reaches, which is the temporary directory the installing
//! processes share rather than the whole machine.
//!
//! A process that waits out [`INSTALL_LOCK_WAIT`] installs nothing and reports
//! [`ToolInstallOutcome::Blocked`]. Installing anyway is the very race the lock
//! exists to stop, and a missing tool is a degradation step 4 already handles.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fs2::FileExt;
use futures::future::BoxFuture;
use regex::Regex;

use swissarmyhammer_common::command::command_failure_detail;

use crate::doctor::{check_presence, run_shell, ToolPresence};
use crate::error::AvpError;
use crate::review::scope::WorkList;
use crate::review::tool_rules::matched_tool_rules;
use crate::validators::pool::PROMPT_TURN_CEILING;
use crate::validators::types::ToolSpec;
use crate::validators::{AgentPool, ValidatorLoader};

/// What an install command that exited 0 reported, for the attempt log.
const INSTALL_COMMAND_SUCCEEDED: &str = "the command exited 0";

/// The file whose lock serializes tool installs across processes.
///
/// One file for every destination at once, because a rule's install commands
/// name no destination: `uv` and `pipx` write `~/.local/bin`, `cargo install`
/// writes `~/.cargo/bin`, `npm install -g` writes the prefix of the node on
/// `PATH`, and `go install` writes the directory `GOBIN` names. A lock over all
/// of them is the only one this module can name. [`InstallLock`] states how far
/// the lock on this file reaches.
const INSTALL_LOCK_FILE_NAME: &str = "swissarmyhammer-tool-install.lock";

/// The longest a shipped rule's declared install commands are expected to run.
///
/// `cargo install cargo-machete@0.9.2 --locked` builds the tool from source,
/// and it is the slowest install any shipped rule declares. Nothing here cuts a
/// declared command short — the command is the rule's own shell snippet and it
/// runs to its end — so this is an expectation about shipped rules rather than
/// a bound the code enforces.
const SLOWEST_DECLARED_INSTALL: Duration = Duration::from_secs(300);

/// How long the install agent's one turn may hold the install lock.
///
/// The bound is the pool's own absolute ceiling on a turn, so it is a backstop
/// rather than a second, tighter policy: a turn that reaches it has already
/// outlived the bound the pool promises, and the pool's idle window
/// (`PROMPT_IDLE_TIMEOUT`) stopped a silent turn long before. Abandoning the
/// turn only drops this process's wait for the answer — the pool worker running
/// it is not cancelled — so a bound under the ceiling would release the lock
/// while an agent was still installing, which is the race the lock exists to
/// stop.
const INSTALL_AGENT_TURN_WAIT: Duration = PROMPT_TURN_CEILING;

/// How long a process waits for a contended install lock before it gives up and
/// reports the tool blocked.
///
/// The wait is bounded because `flock(2)` conflicts between two open file
/// descriptions even inside one process, so a process that reaches
/// [`install_tool_commands`] while it already holds the lock — directly, or
/// through a child an install command spawned — would otherwise block for ever
/// with nothing reported.
///
/// The bound covers a whole holder rather than half of one:
/// [`SLOWEST_DECLARED_INSTALL`] for the declared commands, plus
/// [`INSTALL_AGENT_TURN_WAIT`] for the one agent turn that follows them under
/// the same lock. A deadline that covered only the declared half would make the
/// timeout the ordinary outcome for every waiter behind an agent turn.
const INSTALL_LOCK_WAIT: Duration =
    SLOWEST_DECLARED_INSTALL.saturating_add(INSTALL_AGENT_TURN_WAIT);

/// How long the bounded wait sleeps between two tries of a contended install
/// lock.
const INSTALL_LOCK_RETRY: Duration = Duration::from_millis(100);

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

    /// Another installer held the install lock for the whole wait, so nothing
    /// was run at all.
    ///
    /// Not the same answer as [`ToolInstallOutcome::Failed`]: no command was
    /// tried, and the other installer may yet provide the tool. This run does
    /// not know, so it treats the tool as missing and the superseded prompt
    /// rule runs instead.
    Blocked,
}

impl ToolInstallOutcome {
    /// Whether the tool is usable now, however it got there.
    pub fn tool_present(&self) -> bool {
        matches!(
            self,
            ToolInstallOutcome::AlreadyPresent
                | ToolInstallOutcome::Installed { .. }
                | ToolInstallOutcome::InstalledByAgent
        )
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

/// An exclusive lock over every tool install destination at once, held while
/// install commands run and released when the guard drops.
///
/// `fs2` locks the open file description, so the lock holds between processes
/// as well as between threads. That is what `cargo nextest` needs: each test is
/// its own process, and several of them can reach one destination at once.
///
/// # How far the lock reaches
///
/// The lock file sits in [`std::env::temp_dir`], which reads `$TMPDIR` and
/// falls back to `/tmp`. So the lock covers every process that resolves the
/// same temporary directory, and nothing more: on a machine that gives each
/// user a temporary directory of their own — macOS does — it serializes every
/// install this user runs, and it serializes nothing against a second user or
/// against a process launched with a different `$TMPDIR`.
///
/// That reach fits the destinations one user owns: `~/.local/bin`,
/// `~/.cargo/bin`, and the go bin directory the Go rules point at
/// `$HOME/.local/bin`. Two destinations sit outside it, in two different ways.
///
/// Homebrew writes a prefix every user of the machine shares, and it locks that
/// prefix itself: installing a formula takes a `FormulaLock`, an exclusive
/// `flock(2)` under `<prefix>/var/homebrew/locks`. Two users installing the same
/// formula at once are serialized by Homebrew rather than here.
///
/// `npm install -g` is covered by neither lock, and four shipped rules declare
/// it. The global prefix follows the node on `PATH`: a Homebrew node ships an
/// `npmrc` beside itself setting `prefix = /opt/homebrew`, so `npm install -g`
/// under that node writes the shared Homebrew prefix — and takes no Homebrew
/// lock, because installing a node package is not a Homebrew operation. On a
/// machine that gives each user a temporary directory of its own, two users who
/// install at that moment are serialized by nothing.
///
/// That exposure is accepted rather than closed. A lock file both users could
/// take would have to sit in a world-writable directory, where any local user
/// can hold it; and a wait that ends now installs nothing, so one held file
/// would stop every other user's installs on the machine. The narrow race is
/// the smaller hazard.
#[derive(Debug)]
struct InstallLock {
    /// The locked file. `flock(2)` releases on close, and [`Drop`] releases it
    /// before that so a long-lived process frees the lock at once.
    file: File,
}

/// What one try at the install lock produced.
///
/// Three answers rather than two, because the caller has to tell a live race
/// from a machine that cannot lock at all: only one of them means another
/// installer is writing the destinations at this moment.
#[derive(Debug)]
enum InstallLockVerdict {
    /// This process holds the lock, and the guard releases it on drop.
    Held(InstallLock),

    /// Another installer held the lock for the whole wait. It is still inside
    /// its own install, so this install must not run.
    Blocked,

    /// The machine cannot give the lock at all — a temporary directory that
    /// cannot be opened, or an error `flock(2)` reports that no wait can clear.
    /// No holder is known, so an unserialized install races nothing visible,
    /// and an install with no lock is better than no install at all.
    Unlocked,
}

impl InstallLockVerdict {
    /// The guard to hold while installing, or the outcome that says the install
    /// must not run at all.
    fn hold(self) -> Result<Option<InstallLock>, ToolInstallOutcome> {
        match self {
            Self::Held(lock) => Ok(Some(lock)),
            Self::Unlocked => Ok(None),
            Self::Blocked => Err(ToolInstallOutcome::Blocked),
        }
    }
}

impl InstallLock {
    /// Wait up to [`INSTALL_LOCK_WAIT`] for exclusive use of the install
    /// destinations.
    fn acquire() -> InstallLockVerdict {
        Self::acquire_at(&install_lock_path(), INSTALL_LOCK_WAIT)
    }

    /// Open `path` and wait up to `wait` for exclusive use of it.
    ///
    /// The path and the deadline are parameters rather than constants read
    /// inside, so a test can drive both while production reads
    /// [`install_lock_path`] and [`INSTALL_LOCK_WAIT`].
    fn acquire_at(path: &Path, wait: Duration) -> InstallLockVerdict {
        let opened = OpenOptions::new().create(true).append(true).open(path);
        let file = match opened {
            Ok(file) => file,
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %error,
                    "the tool install lock file could not be opened; installing unserialized"
                );
                return InstallLockVerdict::Unlocked;
            }
        };
        Self::take(file, path, wait)
    }

    /// Take the exclusive lock on `file`, waiting at most `wait` for whoever
    /// holds it to let go.
    ///
    /// The deadline is a parameter rather than a constant read inside, so a
    /// test can drive the wait in milliseconds while production waits minutes.
    fn take(file: File, path: &Path, wait: Duration) -> InstallLockVerdict {
        let deadline = Instant::now() + wait;
        let mut waiting = false;
        loop {
            match try_take_install_lock(&file, path) {
                Some(true) => return InstallLockVerdict::Held(Self { file }),
                None => return InstallLockVerdict::Unlocked,
                Some(false) => {}
            }

            if !waiting {
                waiting = true;
                tracing::info!(
                    path = %path.display(),
                    wait_seconds = wait.as_secs(),
                    "another installer holds the tool install lock; waiting for it"
                );
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            std::thread::sleep(INSTALL_LOCK_RETRY.min(remaining));
        }

        tracing::warn!(
            path = %path.display(),
            wait_seconds = wait.as_secs(),
            "the tool install lock stayed held for the whole wait; the tool is not installed"
        );
        InstallLockVerdict::Blocked
    }
}

/// The file whose lock serializes tool installs, in the temporary directory
/// this process resolves.
fn install_lock_path() -> PathBuf {
    std::env::temp_dir().join(INSTALL_LOCK_FILE_NAME)
}

/// Try the exclusive lock on `file` once.
///
/// `Some(true)` took the lock, `Some(false)` means another open file
/// description holds it, and `None` is a failure no wait can clear, which the
/// caller reports as [`InstallLockVerdict::Unlocked`].
fn try_take_install_lock(file: &File, path: &Path) -> Option<bool> {
    match file.try_lock_exclusive() {
        Ok(()) => Some(true),
        Err(error) if error.kind() == fs2::lock_contended_error().kind() => Some(false),
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "the tool install lock could not be taken; installing unserialized"
            );
            None
        }
    }
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        if let Err(error) = FileExt::unlock(&self.file) {
            tracing::warn!(error = %error, "the tool install lock could not be released");
        }
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
/// Returns [`ToolInstallOutcome::Blocked`] without running anything when
/// another installer holds the install lock for the whole of
/// [`INSTALL_LOCK_WAIT`].
///
/// No LLM and no agent: `sah init` pre-installs runner tools through this exact
/// function.
pub fn install_tool_commands(spec: &ToolSpec) -> ToolInstallOutcome {
    if matches!(check_presence(spec), ToolPresence::Present) {
        return ToolInstallOutcome::AlreadyPresent;
    }

    install_under_lock(spec, InstallLock::acquire())
}

/// Run the rule's declared install commands under `verdict`, or report why they
/// must not run at all.
///
/// Split out of [`install_tool_commands`] so a test can drive each answer the
/// lock can give without racing a real one.
fn install_under_lock(spec: &ToolSpec, verdict: InstallLockVerdict) -> ToolInstallOutcome {
    match verdict.hold() {
        Ok(_lock) => run_declared_install_commands(spec),
        Err(blocked) => blocked,
    }
}

/// Run the rule's declared install commands, under a lock the caller holds.
///
/// Split out of [`install_tool_commands`] so [`ensure_tool_installed`] can hold
/// ONE lock across both halves of the lifecycle. [`InstallLock`] is not
/// reentrant — `flock(2)` conflicts between two open file descriptions even
/// inside one process — so the agent half cannot take a second one.
fn run_declared_install_commands(spec: &ToolSpec) -> ToolInstallOutcome {
    // Another installer may have finished while this one waited for the lock.
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
///
/// ONE [`InstallLock`] covers both halves. The agent writes the same
/// destinations the declared commands write, and it runs commands no rule
/// declared, so it is the half the lock matters most for. Because the lock is
/// held across the turn, the turn is bounded by [`INSTALL_AGENT_TURN_WAIT`] —
/// a turn that reaches that bound is abandoned, and the doctor check still
/// decides.
///
/// Returns [`ToolInstallOutcome::Blocked`] without running anything when
/// another installer holds the install lock for the whole of
/// [`INSTALL_LOCK_WAIT`].
pub async fn ensure_tool_installed(
    rule: &str,
    spec: &ToolSpec,
    agent: Option<&dyn ToolInstallAgent>,
) -> ToolInstallOutcome {
    ensure_tool_installed_within(rule, spec, agent, INSTALL_AGENT_TURN_WAIT).await
}

/// The lifecycle of [`ensure_tool_installed`], with the agent turn bounded by
/// `turn_wait`.
///
/// The bound is a parameter rather than a constant read inside, so a test can
/// drive it in milliseconds while production reads
/// [`INSTALL_AGENT_TURN_WAIT`].
async fn ensure_tool_installed_within(
    rule: &str,
    spec: &ToolSpec,
    agent: Option<&dyn ToolInstallAgent>,
    turn_wait: Duration,
) -> ToolInstallOutcome {
    if matches!(check_presence(spec), ToolPresence::Present) {
        return ToolInstallOutcome::AlreadyPresent;
    }

    let _lock = match InstallLock::acquire().hold() {
        Ok(lock) => lock,
        Err(blocked) => return blocked,
    };

    let attempts = match run_declared_install_commands(spec) {
        ToolInstallOutcome::Failed { attempts } => attempts,
        installed => return installed,
    };

    let (Some(agent), Some(doctor)) = (agent, spec.doctor.as_ref()) else {
        return ToolInstallOutcome::Failed { attempts };
    };

    let request = InstallAgentRequest::new(rule, &doctor.check_command, attempts.clone());
    match tokio::time::timeout(turn_wait, agent.install(&request)).await {
        Ok(Ok(transcript)) => tracing::info!(
            rule = %rule,
            transcript = %transcript,
            "the install agent finished its turn; the doctor check decides"
        ),
        Ok(Err(e)) => tracing::warn!(
            rule = %rule,
            error = %e,
            "the install agent turn could not be run"
        ),
        Err(_) => tracing::warn!(
            rule = %rule,
            wait_seconds = turn_wait.as_secs(),
            "the install agent turn passed its bound and was abandoned; the doctor check decides"
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

    use swissarmyhammer_common::test_utils::shell_escape_path;

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
                check_command: format!("test -f {}", shell_escape_path(marker)),
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
        format!("touch {}", shell_escape_path(marker))
    }

    /// A marker path under `dir` that no test has created yet.
    fn marker_in(dir: &Path, name: &str) -> PathBuf {
        dir.join(name)
    }

    /// A command that always fails, so the doctor check stays failing.
    const FAILING_COMMAND: &str = "echo 'no such package' >&2; exit 1";

    /// How many installers [`installs_never_overlap`] drives at once.
    const INSTALL_RACE_INSTALLERS: usize = 4;

    /// How long each install command in [`installs_never_overlap`] stays inside
    /// its critical section, in seconds. Long enough that two unserialized
    /// installers overlap, short enough to keep the test quick.
    const INSTALL_RACE_HOLD_SECONDS: &str = "0.2";

    /// What an install command in [`installs_never_overlap`] writes when it
    /// enters its critical section.
    const INSTALL_RACE_ENTERED: &str = "entered";

    /// What an install command in [`installs_never_overlap`] writes when it
    /// leaves its critical section.
    const INSTALL_RACE_LEFT: &str = "left";

    /// An install command that records when it enters and leaves its critical
    /// section, waits inside it, and then creates `marker` so the doctor check
    /// passes.
    fn racing_install_command(log: &Path, marker: &Path) -> String {
        format!(
            "printf '{INSTALL_RACE_ENTERED}\\n' >> {log}; sleep {INSTALL_RACE_HOLD_SECONDS}; \
             printf '{INSTALL_RACE_LEFT}\\n' >> {log}; touch {marker}",
            log = shell_escape_path(log),
            marker = shell_escape_path(marker)
        )
    }

    /// Two installers never write their destination at the same time.
    ///
    /// Every install command a shipped rule declares writes to a directory the
    /// whole machine shares — `~/.local/bin` for `uv` and `pipx`, `~/.cargo/bin`
    /// for `cargo install`, the npm and go bin directories for the rest — and
    /// only `cargo` holds a lock of its own. Under `cargo nextest` each test is
    /// its own process, so several installers can reach one destination at once.
    ///
    /// `fs2` locks the open file description, so two threads that each open the
    /// lock file contend through the same `flock(2)` call two processes use.
    /// The log therefore has to read `entered`, `left`, `entered`, `left` — an
    /// unserialized run writes two `entered` lines in a row.
    #[test]
    fn installs_never_overlap() {
        let temp = tempfile::tempdir().expect("temp dir");
        let log = temp.path().join("install-log");
        let specs: Vec<ToolSpec> = (0..INSTALL_RACE_INSTALLERS)
            .map(|installer| {
                let marker = marker_in(temp.path(), &format!("tool-{installer}"));
                let command = racing_install_command(&log, &marker);
                marker_spec(&marker, &[command])
            })
            .collect();

        std::thread::scope(|scope| {
            for spec in &specs {
                scope.spawn(move || {
                    assert!(
                        install_tool_commands(spec).tool_present(),
                        "every racing install command creates its own marker"
                    );
                });
            }
        });

        let entered: Vec<String> = std::fs::read_to_string(&log)
            .expect("the install commands wrote the log")
            .lines()
            .map(str::to_string)
            .collect();
        let serialized: Vec<String> = std::iter::repeat_n(
            [
                INSTALL_RACE_ENTERED.to_string(),
                INSTALL_RACE_LEFT.to_string(),
            ],
            INSTALL_RACE_INSTALLERS,
        )
        .flatten()
        .collect();
        assert_eq!(
            entered, serialized,
            "an install must hold the destination alone; this log shows two installers inside it"
        );
    }

    /// How long the contended-lock tests wait before they give up. Short,
    /// because they prove the wait ends rather than how long it runs.
    const CONTENDED_LOCK_WAIT: Duration = Duration::from_millis(50);

    /// How long the whole bounded wait may take before the test calls it
    /// unbounded.
    const CONTENDED_LOCK_CEILING: Duration = Duration::from_secs(30);

    /// How long the release test holds the lock before it lets the waiter in.
    const RELEASED_LOCK_HOLD: Duration = Duration::from_millis(200);

    /// How long the release test's waiter waits. Longer than
    /// [`RELEASED_LOCK_HOLD`], so the waiter is still waiting when the holder
    /// releases.
    const RELEASED_LOCK_WAIT: Duration = Duration::from_secs(10);

    /// The lock file with the options [`InstallLock::acquire_at`] opens it
    /// with.
    fn lock_file(path: &Path) -> File {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("open the lock file")
    }

    /// The guard a verdict carries, or a panic naming the verdict that carried
    /// none.
    fn expect_held(verdict: InstallLockVerdict) -> InstallLock {
        match verdict {
            InstallLockVerdict::Held(lock) => lock,
            other => panic!("the lock is free, so it must be held; got {other:?}"),
        }
    }

    /// A second lock on a held file gives up instead of waiting for ever.
    ///
    /// `flock(2)` conflicts between two open file descriptions even inside one
    /// process, so a process that reaches the installer while it already holds
    /// the lock — directly, or through a child an install command spawned —
    /// used to block with no deadline and no line in the log. The wait now ends
    /// and the caller reports the tool blocked.
    #[test]
    fn a_contended_install_lock_gives_up_instead_of_waiting_for_ever() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("install.lock");
        let held = expect_held(InstallLock::take(
            lock_file(&path),
            &path,
            CONTENDED_LOCK_WAIT,
        ));

        let started = Instant::now();
        let second = InstallLock::take(lock_file(&path), &path, CONTENDED_LOCK_WAIT);

        assert!(
            matches!(second, InstallLockVerdict::Blocked),
            "a contended lock must give up, never report a lock another holder still owns"
        );
        assert!(
            started.elapsed() < CONTENDED_LOCK_CEILING,
            "the wait must be bounded; it took {:?}",
            started.elapsed()
        );
        drop(held);
    }

    /// A live race and a machine that cannot lock at all are two different
    /// answers, and the caller has to tell them apart.
    ///
    /// A holder that never let go means another installer is writing the
    /// destinations right now. A lock file that cannot be opened means no
    /// holder is known at all. One of them must stop the install and the other
    /// must not, so one answer for both is no answer.
    #[test]
    fn a_contended_lock_is_told_apart_from_a_lock_the_machine_cannot_give() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("install.lock");
        let held = expect_held(InstallLock::take(
            lock_file(&path),
            &path,
            CONTENDED_LOCK_WAIT,
        ));
        let unopenable = temp.path().join("no-such-directory").join("install.lock");

        let contended = InstallLock::acquire_at(&path, CONTENDED_LOCK_WAIT);
        let unusable = InstallLock::acquire_at(&unopenable, CONTENDED_LOCK_WAIT);

        assert!(
            matches!(contended, InstallLockVerdict::Blocked),
            "a holder that never let go is a live race; got {contended:?}"
        );
        assert!(
            matches!(unusable, InstallLockVerdict::Unlocked),
            "a lock file that cannot be opened names no holder; got {unusable:?}"
        );
        drop(held);
    }

    /// A lock another installer holds throughout stops the install; it never
    /// runs the commands unserialized.
    ///
    /// Installing anyway is exactly the race the lock exists to stop, and a
    /// wait that ends means the other installer is still inside its own
    /// critical section.
    #[test]
    fn a_contended_install_lock_runs_no_install_command() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "blocked-tool");
        let spec = marker_spec(&marker, &[create_marker(&marker)]);

        let outcome = install_under_lock(&spec, InstallLockVerdict::Blocked);

        assert_eq!(outcome, ToolInstallOutcome::Blocked);
        assert!(
            !outcome.tool_present(),
            "a blocked install installed nothing, so it cannot report the tool present"
        );
        assert!(
            !marker.exists(),
            "another installer still held the destinations; this install ran anyway"
        );
    }

    /// A lock the machine cannot give names no holder, so the install goes
    /// ahead unserialized rather than reporting the tool blocked.
    ///
    /// An install with no lock is worse than an install with one, and better
    /// than no install at all.
    #[test]
    fn an_install_with_no_lock_available_still_runs() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "unlocked-tool");
        let spec = marker_spec(&marker, &[create_marker(&marker)]);

        let outcome = install_under_lock(&spec, InstallLockVerdict::Unlocked);

        assert!(outcome.tool_present());
        assert!(marker.exists(), "the install command ran");
    }

    /// The bounded wait still serializes: a waiter takes the lock the holder
    /// releases, rather than giving up the moment it finds the lock busy.
    #[test]
    fn the_bounded_wait_takes_the_lock_the_holder_releases() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("install.lock");
        let held = expect_held(InstallLock::take(
            lock_file(&path),
            &path,
            RELEASED_LOCK_WAIT,
        ));

        std::thread::scope(|scope| {
            let waiter =
                scope.spawn(|| InstallLock::take(lock_file(&path), &path, RELEASED_LOCK_WAIT));
            std::thread::sleep(RELEASED_LOCK_HOLD);
            drop(held);
            let taken = waiter.join().expect("the waiting thread");
            assert!(
                matches!(taken, InstallLockVerdict::Held(_)),
                "the waiter must take the lock once the holder releases it; got {taken:?}"
            );
        });
    }

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
    #[derive(Debug)]
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

    /// An install agent whose turn never answers, for the bounded-turn test.
    #[derive(Debug)]
    struct HangingInstallAgent;

    impl ToolInstallAgent for HangingInstallAgent {
        fn install<'a>(
            &'a self,
            _request: &'a InstallAgentRequest,
        ) -> BoxFuture<'a, Result<String, AvpError>> {
            Box::pin(std::future::pending())
        }
    }

    /// The bound the bounded-turn test gives the agent turn.
    const HANGING_TURN_WAIT: Duration = Duration::from_millis(200);

    /// How long that test gives the whole lifecycle before it calls the agent
    /// turn unbounded. Well past [`HANGING_TURN_WAIT`], so the test measures
    /// the bound rather than the machine's load.
    const HANGING_TURN_CEILING: Duration = Duration::from_secs(30);

    /// An agent turn that never answers gives the install lock back.
    ///
    /// The lock covers the agent half, and the pool's own bounds cannot end a
    /// turn that keeps talking before `PROMPT_TURN_CEILING`. A waiter behind an
    /// unbounded turn would spend its whole deadline while the pool still
    /// called that turn healthy, so the lifecycle bounds the turn itself.
    #[tokio::test]
    async fn an_install_agent_turn_that_never_answers_is_bounded() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = marker_in(temp.path(), "tool");
        let spec = marker_spec(&marker, &[FAILING_COMMAND.to_string()]);

        let outcome = tokio::time::timeout(
            HANGING_TURN_CEILING,
            ensure_tool_installed_within(
                "tool-set/todo-check",
                &spec,
                Some(&HangingInstallAgent),
                HANGING_TURN_WAIT,
            ),
        )
        .await
        .expect(
            "the lifecycle must bound the agent turn; it held the install lock past the ceiling",
        );

        assert!(
            !outcome.tool_present(),
            "an abandoned turn installed nothing, so the doctor check still reports the tool missing"
        );
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
