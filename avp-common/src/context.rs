//! AVP Context - Manages the AVP directory, logging, and agent access.
//!
//! The AVP directory (configured via `AvpConfig::DIR_NAME`) is created at the
//! git repository root and contains:
//! - `avp.log` - Append-only log of hook events
//! - `validators/` - Project-specific validators
//! - `.gitignore` - Excludes log files from version control
//!
//! User-level validators can be placed in `~/<AVP_DIR>/validators/`.
//!
//! The context also provides access to an ACP Agent for validator execution.
//! In production, this is a ClaudeAgent created lazily. In tests, a PlaybackAgent
//! can be injected via `with_agent()`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use agent_client_protocol::{Agent, SessionNotification};
use chrono::Utc;
use claude_agent::CreateAgentConfig;
use swissarmyhammer_directory::{AvpConfig, DirectoryConfig, ManagedDirectory};
use tokio::sync::{broadcast, Mutex};

use crate::error::AvpError;
use crate::turn::TurnStateManager;
use crate::types::HookType;
use crate::validator::{ExecutedRuleSet, ExecutedValidator, RuleSet, Validator, ValidatorRunner};

/// Capacity for the broadcast channel used for session notifications.
/// Capacity for notification broadcast channels.
///
/// This needs to be large enough to handle multi-turn validators that may
/// generate many streaming notifications. A 43-turn conversation can easily
/// generate thousands of content deltas.
pub const NOTIFICATION_CHANNEL_CAPACITY: usize = 4096;

/// Log file name within the AVP directory.
const LOG_FILE_NAME: &str = "avp.log";

/// Decision outcome for a hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Hook allowed the action to proceed.
    Allow,
    /// Hook blocked the action.
    Block,
    /// Hook encountered an error.
    Error,
}

impl std::fmt::Display for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Decision::Allow => write!(f, "allow"),
            Decision::Block => write!(f, "block"),
            Decision::Error => write!(f, "error"),
        }
    }
}

/// A hook event to log.
#[derive(Debug)]
pub struct HookEvent<'a> {
    /// The hook type (e.g., "PreToolUse", "PostToolUse").
    pub hook_type: &'a str,
    /// The decision outcome.
    pub decision: Decision,
    /// Optional details (tool name, reason, etc.).
    pub details: Option<String>,
}

/// A validator execution event to log.
#[derive(Debug)]
pub struct ValidatorEvent<'a> {
    /// The validator name.
    pub name: &'a str,
    /// Whether the validator passed.
    pub passed: bool,
    /// The validator message.
    pub message: &'a str,
    /// The hook type that triggered this validator.
    pub hook_type: &'a str,
}

/// Holds the agent and notification sender.
struct AgentHandle {
    agent: Arc<dyn Agent + Send + Sync>,
    notifier: Arc<claude_agent::NotificationSender>,
}

/// AVP Context - manages the AVP directory, logging, agent access, turn state, and validator execution.
///
/// All AVP directory logic is centralized here. The directory is created
/// at the git repository root using the shared `swissarmyhammer-directory` crate.
///
/// The context tracks both project-level and user-level directories:
/// - Project: `./<AVP_DIR>/` at git root
/// - User: `~/<AVP_DIR>/` in home directory
///
/// The context also provides:
/// - Access to an ACP Agent for validator execution (lazy or injected)
/// - Turn state management for tracking file changes across tool calls
/// - Cached validator runner for efficient repeated validation
pub struct AvpContext {
    /// Managed directory at git root (<AVP_DIR>)
    project_dir: ManagedDirectory<AvpConfig>,

    /// Managed directory at user home (~/<AVP_DIR>), if available
    home_dir: Option<ManagedDirectory<AvpConfig>>,

    /// Shared log file handle (None if logging failed to initialize).
    log_file: Option<Arc<StdMutex<File>>>,

    /// Agent handle (lazily created or injected)
    agent_handle: Arc<Mutex<Option<AgentHandle>>>,

    /// Turn state manager for tracking file changes during a turn
    turn_state: Arc<TurnStateManager>,

    /// Cached validator runner (lazily initialized from agent)
    runner_cache: Mutex<Option<ValidatorRunner>>,
}

impl std::fmt::Debug for AvpContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AvpContext")
            .field("project_dir", &self.project_dir.root())
            .field("home_dir", &self.home_dir.as_ref().map(|d| d.root()))
            .field("has_log_file", &self.log_file.is_some())
            .field("has_agent", &"<async>")
            .field("turn_state", &"<manager>")
            .field("runner_cache", &"<cached>")
            .finish()
    }
}

impl AvpContext {
    /// Initialize AVP context by finding git root and creating the AVP directory.
    ///
    /// This will:
    /// 1. Create AVP directory at git root (via swissarmyhammer-directory)
    /// 2. Create .gitignore in the AVP directory if it doesn't exist
    /// 3. Open log file for appending
    /// 4. Optionally connect to user AVP directory
    ///
    /// The agent is created lazily on first access.
    ///
    /// Returns Err if not in a git repository.
    pub fn init() -> Result<Self, AvpError> {
        let (project_dir, home_dir, log_file) = Self::init_directories()?;

        // Create turn state manager - uses parent of avp_dir (project root)
        let project_root = project_dir.root().parent().unwrap_or(project_dir.root());
        let turn_state = Arc::new(TurnStateManager::new(project_root));

        Ok(Self {
            project_dir,
            home_dir,
            log_file,
            agent_handle: Arc::new(Mutex::new(None)),
            turn_state,
            runner_cache: Mutex::new(None),
        })
    }

    /// Create an AVP context with an injected agent.
    ///
    /// This is primarily for testing with PlaybackAgent or other test agents.
    /// The agent is used immediately without lazy creation.
    ///
    /// # Arguments
    ///
    /// * `agent` - The agent to use for validator execution
    /// * `notifications` - Notification receiver from the agent for streaming responses
    ///
    /// # Example
    ///
    /// ```ignore
    /// let playback = PlaybackAgent::new(fixture_path, "test");
    /// let notifications = playback.subscribe_notifications();
    /// let context = AvpContext::with_agent(Arc::new(playback), notifications)?;
    /// ```
    pub fn with_agent(
        agent: Arc<dyn Agent + Send + Sync>,
        notifications: broadcast::Receiver<SessionNotification>,
    ) -> Result<Self, AvpError> {
        let (project_dir, home_dir, log_file) = Self::init_directories()?;

        // Create a NotificationSender and forward from the injected receiver
        // This is for test/playback agents that provide a Receiver
        let (notifier, _) = claude_agent::NotificationSender::new(NOTIFICATION_CHANNEL_CAPACITY);
        let notifier = Arc::new(notifier);
        let notifier_clone = Arc::clone(&notifier);
        tokio::spawn(async move {
            let mut rx = notifications;
            loop {
                match rx.recv().await {
                    Ok(notification) => {
                        let _ = notifier_clone.send_update(notification).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "with_agent notification forwarder lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        // Create turn state manager
        let project_root = project_dir.root().parent().unwrap_or(project_dir.root());
        let turn_state = Arc::new(TurnStateManager::new(project_root));

        Ok(Self {
            project_dir,
            home_dir,
            log_file,
            agent_handle: Arc::new(Mutex::new(Some(AgentHandle { agent, notifier }))),
            turn_state,
            runner_cache: Mutex::new(None),
        })
    }

    /// Initialize directories and log file (shared by init and with_agent).
    fn init_directories() -> Result<
        (
            ManagedDirectory<AvpConfig>,
            Option<ManagedDirectory<AvpConfig>>,
            Option<Arc<StdMutex<File>>>,
        ),
        AvpError,
    > {
        let project_dir = ManagedDirectory::<AvpConfig>::from_git_root().map_err(|e| {
            AvpError::Context(format!(
                "failed to create {} directory: {}",
                AvpConfig::DIR_NAME,
                e
            ))
        })?;
        let home_dir = ManagedDirectory::<AvpConfig>::from_user_home().ok();
        let log_file = open_log_file(project_dir.root());
        Ok((project_dir, home_dir, log_file))
    }

    /// Get the agent for validator execution.
    ///
    /// Creates an ephemeral ClaudeAgent on first access if not already created.
    /// Returns a reference to the agent and the notification sender (for per-session subscribing).
    pub async fn agent(
        &self,
    ) -> Result<
        (
            Arc<dyn Agent + Send + Sync>,
            Arc<claude_agent::NotificationSender>,
        ),
        AvpError,
    > {
        let mut guard = self.agent_handle.lock().await;

        if guard.is_none() {
            tracing::debug!("Creating ephemeral ClaudeAgent for validator execution...");
            let start = std::time::Instant::now();

            let config = CreateAgentConfig::builder().ephemeral(true).build();
            let (agent, notifier) = claude_agent::create_agent(config)
                .await
                .map_err(|e| AvpError::Agent(format!("Failed to create agent: {}", e)))?;

            tracing::debug!(
                "Ephemeral ClaudeAgent created in {:.2}s",
                start.elapsed().as_secs_f64()
            );

            *guard = Some(AgentHandle {
                agent: Arc::new(agent),
                notifier,
            });
        }

        let handle = guard.as_ref().unwrap();
        Ok((Arc::clone(&handle.agent), Arc::clone(&handle.notifier)))
    }

    /// Get the project AVP directory path.
    pub fn avp_dir(&self) -> &Path {
        self.project_dir.root()
    }

    /// Get the turn state manager for tracking file changes.
    pub fn turn_state(&self) -> Arc<TurnStateManager> {
        Arc::clone(&self.turn_state)
    }

    /// Get the project validators directory path (./<AVP_DIR>/validators).
    ///
    /// Returns the path even if it doesn't exist yet.
    pub fn project_validators_dir(&self) -> PathBuf {
        self.project_dir.subdir("validators")
    }

    /// Get the user validators directory path (~/<AVP_DIR>/validators).
    ///
    /// Returns None if user directory is not available.
    pub fn home_validators_dir(&self) -> Option<PathBuf> {
        self.home_dir.as_ref().map(|d| d.subdir("validators"))
    }

    /// Ensure the project validators directory exists.
    ///
    /// Creates the directory if it doesn't exist.
    pub fn ensure_project_validators_dir(&self) -> Result<PathBuf, AvpError> {
        self.project_dir
            .ensure_subdir("validators")
            .map_err(|e| AvpError::Context(format!("failed to create validators directory: {}", e)))
    }

    /// Ensure the user validators directory exists.
    ///
    /// Creates the directory if it doesn't exist.
    /// Returns None if user directory is not available.
    pub fn ensure_home_validators_dir(&self) -> Option<Result<PathBuf, AvpError>> {
        self.home_dir.as_ref().map(|d| {
            d.ensure_subdir("validators").map_err(|e| {
                AvpError::Context(format!("failed to create user validators directory: {}", e))
            })
        })
    }

    /// Get all validator directories that exist.
    ///
    /// Returns directories in precedence order (user first, then project).
    pub fn existing_validator_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // User directory (lower precedence)
        if let Some(home_dir) = self.home_validators_dir() {
            if home_dir.exists() {
                dirs.push(home_dir);
            }
        }

        // Project directory (higher precedence)
        let project_dir = self.project_validators_dir();
        if project_dir.exists() {
            dirs.push(project_dir);
        }

        dirs
    }

    /// Write a line to the log file with timestamp.
    fn write_log_line(&self, content: &str) {
        let Some(log_file) = &self.log_file else {
            return;
        };

        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
        let line = format!("{} {}\n", timestamp, content);

        if let Ok(mut file) = log_file.lock() {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }

    /// Log a hook event.
    ///
    /// Format: `2024-01-23T10:15:32.123Z PreToolUse decision=allow tool=Bash`
    pub fn log_event(&self, event: &HookEvent) {
        let details_str = event
            .details
            .as_ref()
            .map(|d| format!(" {}", d))
            .unwrap_or_default();

        let content = format!(
            "{} decision={}{}",
            event.hook_type, event.decision, details_str
        );

        self.write_log_line(&content);
    }

    /// Log a validator execution event.
    ///
    /// Format: `2024-01-23T10:15:32.123Z VALIDATOR rust-coding passed hook=PostToolUse "No issues found"`
    pub fn log_validator(&self, event: &ValidatorEvent) {
        let status = if event.passed { "passed" } else { "FAILED" };

        let content = format!(
            "VALIDATOR {} {} hook={} \"{}\"",
            event.name, status, event.hook_type, event.message
        );

        self.write_log_line(&content);
    }

    // =========================================================================
    // Validator Execution
    // =========================================================================

    /// Execute validators using the cached runner.
    ///
    /// The runner is created lazily on first access and reused for subsequent calls.
    /// If the agent is unavailable, placeholder pass results are returned.
    pub async fn execute_validators(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> Vec<ExecutedValidator> {
        if validators.is_empty() {
            return Vec::new();
        }

        if self.is_agent_skipped() {
            return self.placeholder_validator_results(validators, hook_type);
        }

        let results = self
            .run_validators_with_fallback(validators, hook_type, input, changed_files)
            .await;

        self.log_validator_results(&results, hook_type);
        results
    }

    /// Check if agent execution is disabled via environment variable.
    fn is_agent_skipped(&self) -> bool {
        if std::env::var("AVP_SKIP_AGENT").is_ok() {
            tracing::debug!("AVP_SKIP_AGENT set - skipping agent execution");
            return true;
        }
        false
    }

    /// Run validators with cached runner, falling back to placeholders on error.
    async fn run_validators_with_fallback(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> Vec<ExecutedValidator> {
        match self
            .execute_with_cached_runner(validators, hook_type, input, changed_files)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                tracing::warn!("Failed to execute validators: {} - using placeholders", e);
                self.placeholder_validator_results(validators, hook_type)
            }
        }
    }

    /// Log results for each executed validator.
    fn log_validator_results(&self, results: &[ExecutedValidator], hook_type: HookType) {
        let hook_type_str = hook_type.to_string();
        for result in results {
            self.log_validator(&ValidatorEvent {
                name: &result.name,
                passed: result.result.passed(),
                message: result.result.message(),
                hook_type: &hook_type_str,
            });
        }
    }

    /// Execute validators with the cached runner.
    async fn execute_with_cached_runner(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> Result<Vec<ExecutedValidator>, AvpError> {
        let mut guard = self.runner_cache.lock().await;

        // Create runner if not cached
        if guard.is_none() {
            tracing::debug!("Creating cached ValidatorRunner...");
            let (agent, notifications) = self.agent().await?;
            let runner = ValidatorRunner::new(agent, notifications)?;
            *guard = Some(runner);
            tracing::debug!("ValidatorRunner cached successfully");
        }

        // Execute with the cached runner
        let runner = guard.as_ref().unwrap();
        tracing::debug!(
            "Executing {} validators via cached ACP runner for hook {}",
            validators.len(),
            hook_type
        );
        Ok(runner
            .execute_validators(validators, hook_type, input, changed_files)
            .await)
    }

    /// Generate placeholder pass results when agent is unavailable.
    fn placeholder_validator_results(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
    ) -> Vec<ExecutedValidator> {
        validators
            .iter()
            .map(|validator| {
                tracing::debug!(
                    "Would execute validator '{}' ({}) for hook {}",
                    validator.name(),
                    validator.source,
                    hook_type
                );

                ExecutedValidator {
                    name: validator.name().to_string(),
                    severity: validator.severity(),
                    result: crate::validator::ValidatorResult::pass(format!(
                        "Validator '{}' matched (runner unavailable)",
                        validator.name()
                    )),
                }
            })
            .collect()
    }

    // ========================================================================
    // RuleSet Execution (New Architecture)
    // ========================================================================

    /// Execute RuleSets using the cached runner.
    ///
    /// Each RuleSet runs in a single agent session with rules evaluated sequentially.
    /// RuleSets execute in parallel with adaptive concurrency control.
    ///
    /// The runner is created lazily on first access and reused for subsequent calls.
    /// If the agent is unavailable, placeholder pass results are returned.
    pub async fn execute_rulesets(
        &self,
        rulesets: &[&RuleSet],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> Vec<ExecutedRuleSet> {
        if rulesets.is_empty() {
            return Vec::new();
        }

        if self.is_agent_skipped() {
            return self.placeholder_ruleset_results(rulesets, hook_type);
        }

        let results = self
            .run_rulesets_with_fallback(rulesets, hook_type, input, changed_files)
            .await;

        self.log_ruleset_results(&results, hook_type);
        results
    }

    /// Run RuleSets with cached runner, falling back to placeholders on error.
    async fn run_rulesets_with_fallback(
        &self,
        rulesets: &[&RuleSet],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> Vec<ExecutedRuleSet> {
        match self
            .execute_rulesets_with_cached_runner(rulesets, hook_type, input, changed_files)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                tracing::warn!("Failed to execute RuleSets: {} - using placeholders", e);
                self.placeholder_ruleset_results(rulesets, hook_type)
            }
        }
    }

    /// Log results for each executed RuleSet.
    fn log_ruleset_results(&self, results: &[ExecutedRuleSet], hook_type: HookType) {
        let hook_type_str = hook_type.to_string();
        for ruleset_result in results {
            for rule_result in &ruleset_result.rule_results {
                self.log_validator(&ValidatorEvent {
                    name: &format!("{}:{}", ruleset_result.ruleset_name, rule_result.rule_name),
                    passed: rule_result.passed(),
                    message: rule_result.message(),
                    hook_type: &hook_type_str,
                });
            }
        }
    }

    /// Execute RuleSets with the cached runner.
    async fn execute_rulesets_with_cached_runner(
        &self,
        rulesets: &[&RuleSet],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> Result<Vec<ExecutedRuleSet>, AvpError> {
        let mut guard = self.runner_cache.lock().await;

        // Create runner if not cached
        if guard.is_none() {
            tracing::debug!("Creating cached ValidatorRunner...");
            let (agent, notifications) = self.agent().await?;
            let runner = ValidatorRunner::new(agent, notifications)?;
            *guard = Some(runner);
            tracing::debug!("ValidatorRunner cached successfully");
        }

        // Execute with the cached runner
        let runner = guard.as_ref().unwrap();
        tracing::debug!(
            "Executing {} RuleSets via cached ACP runner for hook {}",
            rulesets.len(),
            hook_type
        );
        Ok(runner
            .execute_rulesets(rulesets, hook_type, input, changed_files)
            .await)
    }

    /// Generate placeholder pass results when agent is unavailable.
    fn placeholder_ruleset_results(
        &self,
        rulesets: &[&RuleSet],
        hook_type: HookType,
    ) -> Vec<ExecutedRuleSet> {
        rulesets
            .iter()
            .map(|ruleset| {
                tracing::debug!(
                    "Would execute RuleSet '{}' ({}) with {} rules for hook {}",
                    ruleset.name(),
                    ruleset.source,
                    ruleset.rules.len(),
                    hook_type
                );

                let rule_results = ruleset
                    .rules
                    .iter()
                    .map(|rule| crate::validator::RuleResult {
                        rule_name: rule.name.clone(),
                        severity: rule.effective_severity(ruleset),
                        result: crate::validator::ValidatorResult::pass(format!(
                            "Rule '{}' matched (runner unavailable)",
                            rule.name
                        )),
                    })
                    .collect();

                crate::validator::ExecutedRuleSet {
                    ruleset_name: ruleset.name().to_string(),
                    rule_results,
                }
            })
            .collect()
    }
}

/// Open log file for appending.
fn open_log_file(avp_dir: &Path) -> Option<Arc<StdMutex<File>>> {
    let log_path = avp_dir.join(LOG_FILE_NAME);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()
        .map(|f| Arc::new(StdMutex::new(f)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_decision_equality() {
        assert_eq!(Decision::Allow, Decision::Allow);
        assert_eq!(Decision::Block, Decision::Block);
        assert_eq!(Decision::Error, Decision::Error);
        assert_ne!(Decision::Allow, Decision::Block);
        assert_ne!(Decision::Allow, Decision::Error);
        assert_ne!(Decision::Block, Decision::Error);
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_context_with_git_root() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let result = AvpContext::init();

        // Restore original directory
        std::env::set_current_dir(&original_dir).unwrap();

        assert!(result.is_ok());
        let ctx = result.unwrap();
        assert!(ctx.avp_dir().exists());
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_context_validators_dir() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // Validators dir path should be returned even if it doesn't exist
        let validators_path = ctx.project_validators_dir();
        assert!(validators_path.ends_with("validators"));

        // Ensure creates it
        let ensured_path = ctx.ensure_project_validators_dir().unwrap();
        assert!(ensured_path.exists());

        std::env::set_current_dir(&original_dir).unwrap();
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_context_not_in_git_repo() {
        let temp = TempDir::new().unwrap();
        // No .git directory

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let result = AvpContext::init();

        std::env::set_current_dir(&original_dir).unwrap();

        assert!(result.is_err());
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_log_event_writes_to_file() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // Log an event
        let event = HookEvent {
            hook_type: "PreToolUse",
            decision: Decision::Allow,
            details: Some("tool=Bash".to_string()),
        };
        ctx.log_event(&event);

        std::env::set_current_dir(&original_dir).unwrap();

        // Read log file and verify content
        let log_path = ctx.avp_dir().join("avp.log");
        let log_content = fs::read_to_string(&log_path).unwrap();
        assert!(log_content.contains("PreToolUse"));
        assert!(log_content.contains("decision=allow"));
        assert!(log_content.contains("tool=Bash"));
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_log_validator_writes_to_file() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // Log a validator event
        let event = ValidatorEvent {
            name: "test-validator",
            hook_type: "PostToolUse",
            passed: true,
            message: "All checks passed",
        };
        ctx.log_validator(&event);

        std::env::set_current_dir(&original_dir).unwrap();

        // Read log file and verify content
        let log_path = ctx.avp_dir().join("avp.log");
        let log_content = fs::read_to_string(&log_path).unwrap();
        assert!(log_content.contains("VALIDATOR"));
        assert!(log_content.contains("test-validator"));
        assert!(log_content.contains("passed"));
        assert!(log_content.contains("PostToolUse"));
        assert!(log_content.contains("All checks passed"));
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_agent_returns_injected_agent() {
        use agent_client_protocol_extras::PlaybackAgent;

        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        // Create a fixture file for the playback agent
        let fixture_dir = temp.path().join("fixtures");
        fs::create_dir_all(&fixture_dir).unwrap();
        fs::write(fixture_dir.join("test.json"), r#"{"messages": []}"#).unwrap();

        // Create a PlaybackAgent and inject it
        let playback = PlaybackAgent::new(fixture_dir.join("test.json"), "test");
        let notifications = playback.subscribe_notifications();
        let agent: Arc<dyn Agent + Send + Sync> = Arc::new(playback);

        let ctx = AvpContext::with_agent(agent, notifications).unwrap();

        // agent() should return the injected agent
        let result = ctx.agent().await;

        std::env::set_current_dir(&original_dir).unwrap();

        assert!(result.is_ok(), "Should return injected agent");
    }

    #[test]
    fn test_hook_event_construction() {
        // Test HookEvent struct construction and fields
        let event = HookEvent {
            hook_type: "PreToolUse",
            decision: Decision::Allow,
            details: Some("tool=Bash".to_string()),
        };

        assert_eq!(event.hook_type, "PreToolUse");
        assert_eq!(event.decision, Decision::Allow);
        assert_eq!(event.details, Some("tool=Bash".to_string()));

        // Test without details
        let event_no_details = HookEvent {
            hook_type: "Stop",
            decision: Decision::Block,
            details: None,
        };

        assert_eq!(event_no_details.hook_type, "Stop");
        assert_eq!(event_no_details.decision, Decision::Block);
        assert!(event_no_details.details.is_none());
    }

    #[test]
    fn test_validator_event_construction() {
        // Test ValidatorEvent struct construction and fields
        let event = ValidatorEvent {
            name: "no-secrets",
            passed: true,
            message: "No secrets found",
            hook_type: "PostToolUse",
        };

        assert_eq!(event.name, "no-secrets");
        assert!(event.passed);
        assert_eq!(event.message, "No secrets found");
        assert_eq!(event.hook_type, "PostToolUse");

        // Test failed validator
        let failed_event = ValidatorEvent {
            name: "safe-commands",
            passed: false,
            message: "Dangerous command detected",
            hook_type: "PreToolUse",
        };

        assert_eq!(failed_event.name, "safe-commands");
        assert!(!failed_event.passed);
        assert_eq!(failed_event.message, "Dangerous command detected");
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_turn_state() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // turn_state() should return a valid Arc<TurnStateManager>
        let turn_state = ctx.turn_state();
        // Verify we can clone it (Arc functionality)
        let _cloned = Arc::clone(&turn_state);

        std::env::set_current_dir(&original_dir).unwrap();
    }
}
