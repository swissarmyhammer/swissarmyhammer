//! AVP Context - Manages the AVP directory and agent access.
//!
//! The AVP directory (configured via `AvpConfig::DIR_NAME`) is created at the
//! git repository root and contains:
//! - `validators/` - Project-specific validators
//! - `.gitignore` - Excludes log files from version control
//!
//! Logging is handled by `tracing` — the CLI sets up a file layer that writes
//! to `.avp/avp.log` at info level so all tracing output from every crate
//! (agents, validators, hooks) flows into the log automatically.
//!
//! User-level validators can be placed in `~/<AVP_DIR>/validators/`.
//!
//! The context also provides access to an ACP Agent for validator execution.
//! In production, this is a ClaudeAgent created lazily. In tests, a PlaybackAgent
//! can be injected via `with_agent()`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_client_protocol::{Agent, SessionNotification};
use swissarmyhammer_directory::{AvpConfig, DirectoryConfig, ManagedDirectory};
use tokio::sync::{broadcast, Mutex};

use swissarmyhammer_config::model::{ModelConfig, ModelManager, ModelPaths};

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

/// Result type for directory initialization
type InitDirectoriesResult = (
    ManagedDirectory<AvpConfig>,
    Option<ManagedDirectory<AvpConfig>>,
);

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

    /// Resolved model configuration (defaults to claude-code)
    model_config: ModelConfig,

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
            .field("model_config", &self.model_config)
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
        let (project_dir, home_dir) = Self::init_directories()?;

        // Resolve model configuration (defaults to claude-code if not configured)
        let model_config = Self::resolve_model_config();

        // Create turn state manager - uses parent of avp_dir (project root)
        let project_root = project_dir.root().parent().unwrap_or(project_dir.root());
        let turn_state = Arc::new(TurnStateManager::new(project_root));

        Ok(Self {
            project_dir,
            home_dir,
            model_config,
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
        let (project_dir, home_dir) = Self::init_directories()?;

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

        // Resolve model configuration
        let model_config = Self::resolve_model_config();

        // Create turn state manager
        let project_root = project_dir.root().parent().unwrap_or(project_dir.root());
        let turn_state = Arc::new(TurnStateManager::new(project_root));

        Ok(Self {
            project_dir,
            home_dir,
            model_config,
            agent_handle: Arc::new(Mutex::new(Some(AgentHandle { agent, notifier }))),
            turn_state,
            runner_cache: Mutex::new(None),
        })
    }

    /// Create an AVP context with an injected agent and explicit model configuration.
    ///
    /// Like `with_agent()`, but allows specifying the model config directly
    /// instead of resolving it from the project config file. This is useful
    /// for testing the full pipeline with a specific model configuration.
    pub fn with_agent_and_model(
        agent: Arc<dyn Agent + Send + Sync>,
        notifications: broadcast::Receiver<SessionNotification>,
        model_config: ModelConfig,
    ) -> Result<Self, AvpError> {
        let (project_dir, home_dir) = Self::init_directories()?;

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
                        tracing::warn!(skipped = n, "notification forwarder lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        let project_root = project_dir.root().parent().unwrap_or(project_dir.root());
        let turn_state = Arc::new(TurnStateManager::new(project_root));

        Ok(Self {
            project_dir,
            home_dir,
            model_config,
            agent_handle: Arc::new(Mutex::new(Some(AgentHandle { agent, notifier }))),
            turn_state,
            runner_cache: Mutex::new(None),
        })
    }

    /// Initialize directories (shared by init and with_agent).
    fn init_directories() -> Result<InitDirectoriesResult, AvpError> {
        let project_dir = ManagedDirectory::<AvpConfig>::from_git_root().map_err(|e| {
            AvpError::Context(format!(
                "failed to create {} directory: {}",
                AvpConfig::DIR_NAME,
                e
            ))
        })?;
        let home_dir = ManagedDirectory::<AvpConfig>::from_user_home().ok();
        Ok((project_dir, home_dir))
    }

    /// Resolve model configuration from project config.
    ///
    /// Uses `ModelManager::resolve_agent_config()` to read the configured model
    /// from the project config file. Falls back to the default claude-code config
    /// if resolution fails (e.g., no config file, invalid model name).
    fn resolve_model_config() -> ModelConfig {
        match ModelManager::resolve_agent_config(&ModelPaths::avp()) {
            Ok(config) => {
                tracing::debug!("Resolved model config: {:?}", config.executor);
                config
            }
            Err(e) => {
                tracing::debug!("Using default model config (claude-code): {}", e);
                ModelConfig::claude_code()
            }
        }
    }

    /// Get the resolved model configuration.
    pub fn model_config(&self) -> &ModelConfig {
        &self.model_config
    }

    /// Get the agent for validator execution.
    ///
    /// Creates an agent on first access based on the resolved model configuration.
    /// For ClaudeCode models, creates an ephemeral ClaudeAgent.
    /// For LlamaAgent models, creates a local LlamaAgent.
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
            tracing::debug!(
                "Creating {:?} agent for validator execution...",
                self.model_config.executor
            );
            let start = std::time::Instant::now();

            let options = swissarmyhammer_agent::CreateAgentOptions { ephemeral: true };
            let handle =
                swissarmyhammer_agent::create_agent_with_options(&self.model_config, None, options)
                    .await
                    .map_err(|e| AvpError::Agent(format!("Failed to create agent: {}", e)))?;

            tracing::debug!("Agent created in {:.2}s", start.elapsed().as_secs_f64());

            // Bridge the broadcast::Receiver into a NotificationSender
            // (NotificationSender provides per-session subscribe semantics)
            let (notifier, _) =
                claude_agent::NotificationSender::new(NOTIFICATION_CHANNEL_CAPACITY);
            let notifier = Arc::new(notifier);
            let notifier_clone = Arc::clone(&notifier);
            tokio::spawn(async move {
                let mut rx = handle.notification_rx;
                loop {
                    match rx.recv().await {
                        Ok(notification) => {
                            let _ = notifier_clone.send_update(notification).await;
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(skipped = n, "agent notification forwarder lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            *guard = Some(AgentHandle {
                agent: handle.agent,
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

    /// Log a hook event via tracing.
    pub fn log_event(&self, event: &HookEvent) {
        tracing::info!(
            hook_type = event.hook_type,
            decision = %event.decision,
            details = ?event.details,
            "hook event"
        );
    }

    /// Log a validator execution event via tracing.
    pub fn log_validator(&self, event: &ValidatorEvent) {
        tracing::info!(
            validator = event.name,
            passed = event.passed,
            hook_type = event.hook_type,
            message = event.message,
            "validator result"
        );
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
    fn test_log_event_does_not_panic() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // log_event emits tracing::info! — should not panic
        let event = HookEvent {
            hook_type: "PreToolUse",
            decision: Decision::Allow,
            details: Some("tool=Bash".to_string()),
        };
        ctx.log_event(&event);

        std::env::set_current_dir(&original_dir).unwrap();
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_log_validator_does_not_panic() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // log_validator emits tracing::info! — should not panic
        let event = ValidatorEvent {
            name: "test-validator",
            hook_type: "PostToolUse",
            passed: true,
            message: "All checks passed",
        };
        ctx.log_validator(&event);

        std::env::set_current_dir(&original_dir).unwrap();
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

    #[test]
    #[serial_test::serial(cwd)]
    fn test_model_config_defaults_to_claude_code() {
        use swissarmyhammer_config::model::ModelExecutorConfig;

        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();
        let config = ctx.model_config();

        // Default should be claude-code
        assert!(
            matches!(config.executor, ModelExecutorConfig::ClaudeCode(_)),
            "Default model config should be ClaudeCode, got {:?}",
            config.executor
        );

        std::env::set_current_dir(&original_dir).unwrap();
    }

    #[test]
    fn test_resolve_model_config_returns_default_without_config() {
        use swissarmyhammer_config::model::ModelExecutorConfig;

        // When no config file exists, resolve should return claude-code default
        let config = AvpContext::resolve_model_config();
        assert!(matches!(
            config.executor,
            ModelExecutorConfig::ClaudeCode(_)
        ));
    }
}
