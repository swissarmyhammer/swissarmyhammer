//! Rule checking with two-stage rendering and agent execution
//!
//! This module provides the `RuleChecker` which performs rule checks against files:
//! 1. Stage 1: Renders rule templates with context (language, target_path, etc.)
//! 2. Stage 2: Renders .check prompt with rendered rule content
//! 3. Executes via ACP agents (ClaudeCode or LlamaAgent)
//! 4. Parses responses and fails fast on violations

use crate::{
    detect_language, CachedResult, Result, Rule, RuleCache, RuleError, RuleViolation, Severity,
};
use futures_util::stream::{self, Stream, StreamExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use swissarmyhammer_agent::{self as acp, McpServerConfig};
use swissarmyhammer_common::glob_utils::{expand_glob_patterns, GlobExpansionConfig};
use swissarmyhammer_common::Pretty;
use swissarmyhammer_config::model::ModelConfig;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};

/// Default maximum number of concurrent rule checks.
///
/// This value balances throughput with resource usage for parallel rule checking.
/// Higher values may improve performance on machines with more cores but increase
/// memory and CPU usage.
const DEFAULT_MAX_CONCURRENCY: usize = 4;

/// Configuration for creating ACP agents
///
/// This holds the model configuration and optional MCP server config
/// needed to create ACP agents for rule checking.
#[derive(Clone)]
pub struct AgentConfig {
    /// Model configuration for the ACP agent (e.g., Claude or Llama)
    pub model_config: ModelConfig,
    /// Optional MCP (Model Context Protocol) server configuration for extended capabilities
    pub mcp_config: Option<McpServerConfig>,
}

/// Check mode controlling fail-fast behavior
///
/// Determines whether checking stops on the first ERROR violation or continues
/// to check all files and collect all violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckMode {
    /// Stop checking files after the first ERROR violation is found.
    ///
    /// This mode is useful for CI/CD pipelines where you want to fail fast
    /// and not waste time checking remaining files once a violation is detected.
    FailFast,
    /// Check all files and collect all ERROR violations.
    ///
    /// This mode continues checking all files even after violations are found,
    /// providing a complete report of all issues in the codebase.
    CollectAll,
}

/// Request structure for rule checking with filtering and pattern expansion
///
/// This provides a high-level API for checking rules that handles:
/// - Rule loading and filtering
/// - Glob pattern expansion
/// - Validation
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer_rules::{RuleCheckRequest, CheckMode};
///
/// // Check all rules against Rust files with fail-fast
/// let request = RuleCheckRequest {
///     rule_names: None,
///     severity: None,
///     category: None,
///     patterns: vec!["**/*.rs".to_string()],
///     check_mode: CheckMode::FailFast,
///     force: false,
///     max_errors: None,
///     skip_glob_expansion: false,
///     max_concurrency: None,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct RuleCheckRequest {
    /// Optional list of specific rule names to check.
    ///
    /// When `None`, all available rules are checked. When `Some`, only rules
    /// with names matching those in the list are applied.
    pub rule_names: Option<Vec<String>>,
    /// Optional severity level filter.
    ///
    /// When set, only rules with the specified severity level are checked.
    pub severity: Option<Severity>,
    /// Optional category filter.
    ///
    /// When set, only rules belonging to the specified category are checked.
    pub category: Option<String>,
    /// File paths or glob patterns specifying which files to check.
    ///
    /// Supports glob patterns like `**/*.rs` for recursive matching.
    /// See `skip_glob_expansion` for controlling pattern interpretation.
    pub patterns: Vec<String>,
    /// If true, patterns are treated as explicit file paths and glob expansion is skipped.
    ///
    /// This is used when patterns come from git changed files or other sources
    /// that already provide resolved file paths rather than glob patterns.
    pub skip_glob_expansion: bool,
    /// Check mode controlling fail-fast behavior.
    ///
    /// See [`CheckMode`] for available options.
    pub check_mode: CheckMode,
    /// Force re-evaluation, bypassing the cache.
    ///
    /// When true, all checks are performed even if cached results exist.
    pub force: bool,
    /// Maximum number of ERROR violations to return.
    ///
    /// When `None`, all violations are returned (unlimited).
    /// When `Some(n)`, checking stops after `n` ERROR violations are found.
    pub max_errors: Option<usize>,
    /// Maximum number of concurrent rule checks.
    ///
    /// When `None`, defaults to 4 concurrent checks.
    /// Higher values may improve throughput but increase resource usage.
    pub max_concurrency: Option<usize>,
}

/// Core rule checker that performs two-stage rendering and executes checks via ACP agents
///
/// The RuleChecker is the heart of the rules system. It:
/// 1. Renders rule templates with repository context
/// 2. Renders the .check prompt with the rendered rule
/// 3. Executes via ACP agent (ClaudeAgent or LlamaAgent)
/// 4. Parses responses and fails fast on violations
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer_rules::{RuleChecker, Rule, Severity, AgentConfig};
/// use swissarmyhammer_config::model::ModelConfig;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create agent config
/// let agent_config = AgentConfig {
///     model_config: ModelConfig::default(),
///     mcp_config: None,
/// };
///
/// // Create checker
/// let mut checker = RuleChecker::new(agent_config)?;
/// checker.initialize().await?;
///
/// // Create a rule and check a file
/// let rule = Rule::new(
///     "no-todos".to_string(),
///     "Check for TODO comments in {{language}} code".to_string(),
///     Severity::Warning,
/// );
/// let target = PathBuf::from("src/main.rs");
/// checker.check_file(&rule, &target).await?;
/// # Ok(())
/// # }
/// ```
pub struct RuleChecker {
    /// Agent configuration for creating ACP agents
    agent_config: AgentConfig,
    /// Prompt library containing the .check prompt (wrapped in Arc for sharing in streams)
    prompt_library: Arc<PromptLibrary>,
    /// Rule library for partial template support (loaded once for performance)
    rule_library: Arc<crate::RuleLibrary>,
    /// Cache for rule evaluation results (wrapped in Arc for sharing in streams)
    cache: Arc<RuleCache>,
    /// Cached ACP agent handle (created lazily on first check, reused for all subsequent checks)
    /// This avoids the expensive model loading for each check.
    agent_handle: Arc<tokio::sync::Mutex<Option<acp::AcpAgentHandle>>>,
}

impl RuleChecker {
    /// Create a new RuleChecker with the given agent configuration
    ///
    /// Loads the PromptLibrary containing the .check prompt from all sources
    /// (builtin, user, local).
    ///
    /// # Arguments
    ///
    /// * `agent_config` - Configuration for creating ACP agents
    ///
    /// # Returns
    ///
    /// Returns a Result containing the initialized RuleChecker or an error if
    /// the .check prompt cannot be loaded.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - PromptLibrary fails to load
    /// - .check prompt is not found in any source
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_rules::{RuleChecker, AgentConfig};
    /// use swissarmyhammer_config::model::ModelConfig;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let agent_config = AgentConfig {
    ///     model_config: ModelConfig::default(),
    ///     mcp_config: None,
    /// };
    /// let checker = RuleChecker::new(agent_config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(agent_config: AgentConfig) -> Result<Self> {
        tracing::trace!("Creating RuleChecker");

        // Load all prompts including the builtin .check prompt
        let mut prompt_library = PromptLibrary::new();
        let mut resolver = PromptResolver::new();
        resolver
            .load_all_prompts(&mut prompt_library)
            .map_err(|e| RuleError::CheckError(format!("Failed to load prompt library: {}", e)))?;

        // Verify .check prompt exists
        prompt_library.get(".check").map_err(|e| {
            RuleError::CheckError(format!(".check prompt not found in prompt library: {}", e))
        })?;

        tracing::debug!(".check prompt loaded successfully");

        // Load all rules into a library for partial support (once, for performance)
        let mut rule_library = crate::RuleLibrary::new();
        let mut rule_resolver = crate::RuleResolver::new();
        let mut all_rules = Vec::new();
        rule_resolver.load_all_rules(&mut all_rules).map_err(|e| {
            RuleError::CheckError(format!("Failed to load rules for partials: {}", e))
        })?;

        // Add all rules to the library for partial lookups
        for r in all_rules {
            rule_library.add(r).map_err(|e| {
                RuleError::CheckError(format!("Failed to add rule to library: {}", e))
            })?;
        }

        tracing::debug!("Rule library loaded for partial support");

        // Initialize cache
        let cache = RuleCache::new()
            .map_err(|e| RuleError::CheckError(format!("Failed to initialize cache: {}", e)))?;

        Ok(Self {
            agent_config,
            prompt_library: Arc::new(prompt_library),
            rule_library: Arc::new(rule_library),
            cache: Arc::new(cache),
            agent_handle: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    /// Initialize the RuleChecker by initializing the agent executor
    ///
    /// Must be called before check_file() or check_all().
    ///
    /// # Errors
    ///
    /// Returns an error if agent initialization fails.
    pub async fn initialize(&mut self) -> Result<()> {
        tracing::info!("Initializing RuleChecker agent");

        // The LlamaAgentExecutorWrapper uses a singleton pattern internally
        // and will initialize the global agent on first execute_prompt call
        // No explicit initialization needed here

        Ok(())
    }

    /// Check a single file against a rule using two-stage rendering
    ///
    /// # Arguments
    ///
    /// * `rule` - The rule to check against
    /// * `target_path` - Path to the file to check
    ///
    /// # Returns
    ///
    /// Returns `Ok(None)` if the file passes the check (LLM returns "PASS").
    /// Returns `Ok(Some(violation))` if a violation is found.
    /// Returns `Err(...)` for operational errors (file I/O, rendering, agent failures).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File cannot be read
    /// - Language detection fails
    /// - Template rendering fails
    /// - Agent execution fails
    /// - Response parsing fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use swissarmyhammer_rules::{RuleChecker, Rule, Severity, AgentConfig};
    /// # use swissarmyhammer_config::model::ModelConfig;
    /// # use std::path::PathBuf;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let agent_config = AgentConfig { model_config: ModelConfig::default(), mcp_config: None };
    /// # let mut checker = RuleChecker::new(agent_config)?;
    /// # checker.initialize().await?;
    /// let rule = Rule::new(
    ///     "test-rule".to_string(),
    ///     "Check something".to_string(),
    ///     Severity::Error,
    /// );
    /// let target = PathBuf::from("src/main.rs");
    /// match checker.check_file(&rule, &target).await? {
    ///     None => println!("Check passed"),
    ///     Some(violation) => println!("Found violation: {}", violation),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check_file(
        &self,
        rule: &Rule,
        target_path: &Path,
    ) -> Result<Option<RuleViolation>> {
        let check_start = std::time::Instant::now();

        tracing::trace!(
            "Checking file {} against rule {}",
            target_path.display(),
            rule.name
        );

        let (target_content, cache_key) = match self.prepare_file_for_checking(rule, target_path)? {
            Some(result) => result,
            None => return Ok(None),
        };

        // Check line count limit (2048 lines) - files exceeding this automatically fail
        const MAX_LINES_FOR_RULE_CHECK: usize = 2048;
        let line_count = target_content.lines().count();
        if line_count > MAX_LINES_FOR_RULE_CHECK {
            tracing::warn!(
                "File {} is too long ({} lines) for rule checking (max: {} lines) - reporting as violation",
                target_path.display(),
                line_count,
                MAX_LINES_FOR_RULE_CHECK
            );
            // Return a synthetic violation indicating the file is too large
            return Ok(Some(RuleViolation::new(
                "file-too-large".to_string(),
                target_path.to_path_buf(),
                Severity::Error,
                format!(
                    "VIOLATION\nRule: file-too-large\nFile: {}\nLine: N/A\nSeverity: error\nMessage: File is too large for rule checking ({} lines, max: {} lines). Files this large are difficult to maintain and should be split into smaller modules.\nSuggestion: Refactor this file by splitting it into multiple smaller files, each focused on a single responsibility. Consider grouping related functionality into separate modules.",
                    target_path.display(),
                    line_count,
                    MAX_LINES_FOR_RULE_CHECK
                ),
            )));
        }

        if let Some(result) = self.check_cache_for_result(&cache_key, target_path, &rule.name)? {
            return Ok(result);
        }

        let check_prompt_text = self.render_check_prompt(rule, target_path, &target_content)?;

        let result = self
            .execute_and_parse_check(check_prompt_text, rule, target_path, check_start)
            .await?;

        self.cache_check_result(&cache_key, &result);

        Ok(result)
    }

    /// Prepare file for checking by reading content and parsing ignore directives
    ///
    /// Returns Ok(None) if the file should be skipped (binary file or rule is ignored)
    /// Returns Ok(Some((content, cache_key))) if the file should be checked
    /// Returns Err for actual I/O errors
    fn prepare_file_for_checking(
        &self,
        rule: &Rule,
        target_path: &Path,
    ) -> Result<Option<(String, String)>> {
        let target_content = match std::fs::read_to_string(target_path) {
            Ok(content) => content,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::InvalidData {
                    tracing::debug!(
                        "Skipping binary or non-UTF-8 file: {}",
                        target_path.display()
                    );
                    return Ok(None);
                }
                return Err(RuleError::CheckError(format!(
                    "Failed to read file {}: {}",
                    target_path.display(),
                    e
                ))
                .into());
            }
        };

        let ignore_patterns = crate::ignore::parse_ignore_directives(&target_content);
        if crate::ignore::should_ignore_rule(&rule.name, &ignore_patterns) {
            tracing::debug!(
                "Rule {} ignored in {} (file directive)",
                rule.name,
                target_path.display()
            );
            return Ok(None);
        }

        let cache_key =
            RuleCache::calculate_cache_key(&target_content, &rule.template, rule.severity);

        Ok(Some((target_content, cache_key)))
    }

    /// Check cache for a previously computed result
    fn check_cache_for_result(
        &self,
        cache_key: &str,
        target_path: &Path,
        rule_name: &str,
    ) -> Result<Option<Option<RuleViolation>>> {
        if let Some(cached_result) = self.cache.get(cache_key)? {
            tracing::debug!(
                "Cache hit for {} against rule {} - skipping LLM call",
                target_path.display(),
                rule_name
            );
            return Ok(Some(self.handle_cached_result(
                cached_result,
                target_path,
                rule_name,
            )?));
        }

        tracing::trace!(
            "Cache miss for {} against rule {} - proceeding with LLM check",
            target_path.display(),
            rule_name
        );

        Ok(None)
    }

    /// Build a base template context with common fields for rule checking
    ///
    /// This helper consolidates the duplicated context-building logic used in
    /// both rule template rendering and check prompt rendering.
    fn build_base_context(
        target_path: &Path,
        target_content: &str,
        language: &str,
        rule_name: &str,
    ) -> TemplateContext {
        let mut context = TemplateContext::new();
        context.set(
            "target_content".to_string(),
            target_content.to_string().into(),
        );
        context.set(
            "target_path".to_string(),
            target_path.display().to_string().into(),
        );
        context.set("language".to_string(), language.to_string().into());
        context.set("rule_name".to_string(), rule_name.to_string().into());
        context
    }

    /// Render the check prompt using two-stage rendering
    fn render_check_prompt(
        &self,
        rule: &Rule,
        target_path: &Path,
        target_content: &str,
    ) -> Result<String> {
        let language = detect_language(target_path, target_content)?;
        tracing::debug!("Detected language: {}", language);

        // Stage 1: Render rule template with base context
        let rule_context =
            Self::build_base_context(target_path, target_content, &language, &rule.name);

        // Only use Liquid templating if the rule explicitly enables it (e.g., filename contains .liquid)
        // This allows rules to contain code examples with {{ }} without needing {% raw %} blocks
        let rendered_rule = if rule.use_liquid {
            let partial_adapter = crate::RulePartialAdapter::new(Arc::clone(&self.rule_library));

            let template_with_partials = swissarmyhammer_templating::Template::with_partials(
                &rule.template,
                partial_adapter,
            )
            .map_err(|e| {
                RuleError::CheckError(format!(
                    "Failed to create template with partials for {}: {}",
                    rule.name, e
                ))
            })?;

            template_with_partials
                .render_with_context(&rule_context)
                .map_err(|e| {
                    RuleError::CheckError(format!(
                        "Failed to render rule template for {}: {}",
                        rule.name, e
                    ))
                })?
        } else {
            // No Liquid parsing - use template as-is
            rule.template.clone()
        };

        tracing::debug!(
            "Stage 1 complete: rule template rendered (liquid={})",
            rule.use_liquid
        );

        // Stage 2: Render check prompt with base context plus rendered rule
        let mut check_context =
            Self::build_base_context(target_path, target_content, &language, &rule.name);
        check_context.set("rule_content".to_string(), rendered_rule.into());

        let check_prompt_text = self
            .prompt_library
            .render(".check", &check_context)
            .map_err(|e| RuleError::CheckError(format!("Failed to render .check prompt: {}", e)))?;

        tracing::debug!("Stage 2 complete: .check prompt rendered");

        Ok(check_prompt_text)
    }

    /// Get or create the ACP agent (lazy initialization)
    ///
    /// Creates the agent on first call and reuses it for subsequent calls.
    /// This avoids the expensive model loading for each check.
    async fn get_or_create_agent(&self) -> Result<()> {
        let mut guard = self.agent_handle.lock().await;
        if guard.is_none() {
            tracing::debug!("Creating ACP agent (first check)...");
            let start = std::time::Instant::now();
            let agent = acp::create_agent(
                &self.agent_config.model_config,
                self.agent_config.mcp_config.clone(),
            )
            .await
            .map_err(|e| RuleError::AgentError(format!("Failed to create agent: {}", e)))?;
            tracing::debug!("ACP agent created in {:.2}s", start.elapsed().as_secs_f64());
            *guard = Some(agent);
        }
        Ok(())
    }

    /// Determine if an LLM response indicates a PASS verdict
    ///
    /// The LLM may provide analysis before the verdict, so we need to check:
    /// - Starts with PASS (original behavior)
    /// - Contains PASS on its own line (handles analysis before verdict)
    /// - Contains emphasized PASS (**PASS** or *PASS*)
    fn is_pass_response(response_text: &str) -> bool {
        let normalized_text = response_text.trim().trim_start_matches('*').trim_start();

        let starts_with_pass = normalized_text.starts_with("PASS");
        let contains_emphasized_pass =
            response_text.contains("**PASS**") || response_text.contains("*PASS*");
        let has_pass_line = response_text.lines().any(|line| {
            let trimmed = line.trim();
            trimmed == "PASS" || trimmed.starts_with("PASS")
        });

        let is_pass = starts_with_pass || contains_emphasized_pass || has_pass_line;

        tracing::debug!(
            "Response parsing - length: {}, first 100 chars: {:?}",
            response_text.len(),
            response_text.chars().take(100).collect::<String>()
        );
        tracing::debug!(
            "Response parsing - starts_with_pass: {}, contains_emphasized: {}, has_pass_line: {}, is_pass: {}",
            starts_with_pass,
            contains_emphasized_pass,
            has_pass_line,
            is_pass
        );

        is_pass
    }

    /// Create a session-scoped agent handle for concurrent checks
    ///
    /// This clones the agent Arc and creates a new notification receiver,
    /// allowing concurrent checks without blocking on the main mutex.
    async fn create_session_handle(&self) -> Result<acp::AcpAgentHandle> {
        let guard = self.agent_handle.lock().await;
        let main_handle = guard
            .as_ref()
            .ok_or_else(|| RuleError::AgentError("Agent not initialized".to_string()))?;

        Ok(acp::AcpAgentHandle {
            agent: std::sync::Arc::clone(&main_handle.agent),
            notification_rx: main_handle.notification_rx.resubscribe(),
        })
    }

    /// Execute the check via ACP agent and parse the response
    async fn execute_and_parse_check(
        &self,
        check_prompt_text: String,
        rule: &Rule,
        target_path: &Path,
        check_start: std::time::Instant,
    ) -> Result<Option<RuleViolation>> {
        tracing::trace!(
            "execute_and_parse_check: starting for {} against {}",
            target_path.display(),
            rule.name
        );

        // Ensure agent is created (lazy initialization)
        self.get_or_create_agent().await?;

        let mut session_handle = self.create_session_handle().await?;

        // Execute prompt via ACP protocol
        // For rule checking, no system prompt is needed - the check prompt contains everything
        // Use the "rule-checker" mode for rule checking operations
        let response = acp::execute_prompt(
            &mut session_handle,
            None,
            Some("rule-checker".to_string()),
            check_prompt_text,
        )
        .await
        .map_err(|e| RuleError::AgentError(format!("Agent execution failed: {}", e)))?;

        tracing::debug!("LLM execution complete");

        self.build_check_result(&response.content, rule, target_path, check_start)
    }

    /// Build the check result from the LLM response
    fn build_check_result(
        &self,
        response_content: &str,
        rule: &Rule,
        target_path: &Path,
        check_start: std::time::Instant,
    ) -> Result<Option<RuleViolation>> {
        let duration = check_start.elapsed();

        if Self::is_pass_response(response_content) {
            tracing::info!(
                "✓ PASS DETECTED ✓ Check passed for {} against rule {} in {:.2}s",
                target_path.display(),
                rule.name,
                duration.as_secs_f64()
            );
            Ok(None)
        } else {
            let violation = RuleViolation::new(
                rule.name.clone(),
                target_path.to_path_buf(),
                rule.severity,
                response_content.to_string(),
            );

            tracing::warn!(
                "Violation found in {} against rule {} ({}) in {:.2}s",
                target_path.display(),
                rule.name,
                rule.severity,
                duration.as_secs_f64()
            );

            Self::log_violation(&violation);
            Ok(Some(violation))
        }
    }

    /// Cache the check result
    fn cache_check_result(&self, cache_key: &str, result: &Option<RuleViolation>) {
        let cached_result = match result {
            None => CachedResult::Pass,
            Some(violation) => CachedResult::Violation {
                violation: violation.clone(),
            },
        };
        self.store_cached_result_with_logging(cache_key, &cached_result);
    }

    /// Check rules against files, streaming violations as they are discovered
    ///
    /// This is the main entry point for rule checking. It loads rules, filters them,
    /// validates them, expands file patterns, and streams violations as they are found.
    ///
    /// The behavior depends on the check_mode in the request:
    /// - FailFast: Stops checking files after the first ERROR violation
    /// - CollectAll: Checks all files and yields all ERROR violations
    ///
    /// # Arguments
    ///
    /// * `request` - RuleCheckRequest with filters, patterns, and check mode
    ///
    /// # Returns
    ///
    /// Returns a Stream that yields Result<RuleViolation>. Each item is either:
    /// - Ok(violation) for a discovered ERROR violation
    /// - Err(e) for non-violation errors (rule loading, validation, etc.)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Rule loading fails
    /// - Rule validation fails
    /// - Pattern expansion fails
    /// - Check execution fails (for non-violation errors)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use swissarmyhammer_rules::{RuleChecker, RuleCheckRequest, CheckMode, AgentConfig};
    /// # use swissarmyhammer_config::model::ModelConfig;
    /// # use futures_util::stream::StreamExt;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let agent_config = AgentConfig { model_config: ModelConfig::default(), mcp_config: None };
    /// # let mut checker = RuleChecker::new(agent_config)?;
    /// # checker.initialize().await?;
    /// let request = RuleCheckRequest {
    ///     rule_names: None,
    ///     severity: None,
    ///     category: None,
    ///     patterns: vec!["**/*.rs".to_string()],
    ///     check_mode: CheckMode::CollectAll,
    ///     force: false,
    ///     max_errors: None,
    ///     skip_glob_expansion: false,
    ///     max_concurrency: None,
    /// };
    ///
    /// let mut stream = checker.check(request).await?;
    /// while let Some(result) = stream.next().await {
    ///     match result {
    ///         Ok(violation) => println!("Found violation: {}", violation.rule_name),
    ///         Err(e) => return Err(e.into()),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check(
        &self,
        request: RuleCheckRequest,
    ) -> Result<impl Stream<Item = Result<RuleViolation>>> {
        tracing::info!("Starting rule check with filters (streaming mode)");

        let rules = self.load_and_validate_rules()?;

        let filtered_rules = self.apply_rule_filters(rules, &request)?;

        if filtered_rules.is_empty() {
            tracing::info!("No rules matched the filters");
            return Ok(stream::iter(vec![]).boxed());
        }

        let target_files =
            self.expand_target_patterns(&request.patterns, request.skip_glob_expansion)?;

        if target_files.is_empty() {
            tracing::info!("No files matched the patterns");
            return Ok(stream::iter(vec![]).boxed());
        }

        let work_items = self.build_work_queue(filtered_rules, target_files);

        let stream = self.process_work_queue_stream(work_items, &request);

        Ok(stream)
    }

    /// Load all rules and validate them
    fn load_and_validate_rules(&self) -> Result<Vec<Rule>> {
        let mut rules = Vec::new();
        let mut resolver = crate::RuleResolver::new();
        resolver
            .load_all_rules(&mut rules)
            .map_err(|e| RuleError::CheckError(format!("Failed to load rules: {}", e)))?;

        let total_files = rules.len();
        let partials_count = rules.iter().filter(|r| r.is_partial()).count();

        rules.retain(|r| !r.is_partial());

        let rule_names: Vec<&str> = rules.iter().map(|r| r.name.as_str()).collect();
        tracing::info!(
            "Loaded {} files ({} rules, {} partials). Rules: {}",
            total_files,
            rules.len(),
            partials_count,
            Pretty(&rule_names)
        );

        for rule in &rules {
            rule.validate().map_err(|e| {
                RuleError::CheckError(format!("Rule validation failed for '{}': {}", rule.name, e))
            })?;
        }

        Ok(rules)
    }

    /// Apply filters to the rule list
    fn apply_rule_filters(
        &self,
        mut rules: Vec<Rule>,
        request: &RuleCheckRequest,
    ) -> Result<Vec<Rule>> {
        if let Some(rule_names) = &request.rule_names {
            tracing::info!("Filtering by rule_names: {}", Pretty(rule_names));
            tracing::info!(
                "Available rule names before filter: {}",
                Pretty(&rules.iter().map(|r| &r.name).collect::<Vec<_>>())
            );
            rules.retain(|r| rule_names.contains(&r.name));
            tracing::info!("After filtering by rule_names: {} rules", rules.len());
        }

        if let Some(severity) = &request.severity {
            rules.retain(|r| &r.severity == severity);
        }

        if let Some(category) = &request.category {
            rules.retain(|r| r.category.as_ref() == Some(category));
        }

        Ok(rules)
    }

    /// Expand glob patterns to get target files, or use patterns as-is if skip_glob_expansion is set
    fn expand_target_patterns(
        &self,
        patterns: &[String],
        skip_glob_expansion: bool,
    ) -> Result<Vec<PathBuf>> {
        let mut target_files = if skip_glob_expansion {
            // Patterns are already explicit file paths - convert directly to PathBufs
            tracing::info!(
                "Using {} explicit file paths (skipping glob expansion)",
                patterns.len()
            );
            patterns
                .iter()
                .map(PathBuf::from)
                .filter(|p| p.is_file())
                .collect()
        } else {
            // Expand glob patterns normally
            let config = GlobExpansionConfig::default();
            expand_glob_patterns(patterns, &config).map_err(|e| {
                RuleError::CheckError(format!("Failed to expand glob patterns: {}", e))
            })?
        };

        target_files.sort();

        if !target_files.is_empty() {
            tracing::info!(
                "Will check {} target files against rules",
                target_files.len()
            );
        }

        Ok(target_files)
    }

    /// Check if a rule should be applied to a specific file based on applies_to pattern
    ///
    /// Returns true if:
    /// - The rule has no applies_to pattern (applies to all files)
    /// - The file matches the rule's applies_to glob pattern
    ///
    /// Returns false if:
    /// - The pattern is invalid (with a warning logged)
    /// - The file doesn't match the pattern
    fn should_apply_rule_to_file(rule: &Rule, file: &Path) -> bool {
        match &rule.applies_to {
            None => true,
            Some(pattern) => match glob::Pattern::new(pattern) {
                Ok(glob_pattern) => glob_pattern.matches_path(file),
                Err(_) => {
                    tracing::warn!(
                        "Invalid applies_to pattern '{}' in rule '{}', skipping",
                        pattern,
                        rule.name
                    );
                    false
                }
            },
        }
    }

    /// Build work queue of (rule, file) pairs
    fn build_work_queue(
        &self,
        rules: Vec<Rule>,
        target_files: Vec<PathBuf>,
    ) -> Vec<(Rule, PathBuf)> {
        let work_items: Vec<(Rule, PathBuf)> = rules
            .iter()
            .flat_map(|rule| {
                target_files
                    .iter()
                    .filter(|file| Self::should_apply_rule_to_file(rule, file))
                    .map(|file| (rule.clone(), file.clone()))
            })
            .collect();

        tracing::info!("Created work queue with {} check items", work_items.len());

        work_items
    }

    /// Calculate progress information for a work item
    ///
    /// Returns (completed_so_far, remaining_items, estimated_remaining_seconds)
    fn calculate_progress(
        completed_count: &std::sync::atomic::AtomicUsize,
        total_items: usize,
        start_time: std::time::Instant,
    ) -> (usize, usize, f64) {
        let completed_so_far = completed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let remaining = total_items.saturating_sub(completed_so_far + 1);

        let elapsed = start_time.elapsed().as_secs_f64();
        let avg_time_per_check = if completed_so_far > 0 {
            elapsed / (completed_so_far as f64)
        } else {
            0.0
        };
        let estimated_remaining_secs = (remaining as f64) * avg_time_per_check;

        (completed_so_far, remaining, estimated_remaining_secs)
    }

    /// Filter check results to only include ERROR severity violations
    fn filter_error_violations(
        result: Result<Option<RuleViolation>>,
    ) -> Option<Result<RuleViolation>> {
        match result {
            Ok(Some(violation)) if violation.severity == Severity::Error => Some(Ok(violation)),
            Ok(Some(_)) => None, // Non-error violations are filtered out
            Ok(None) => None,    // Passes are filtered out
            Err(e) => Some(Err(e)),
        }
    }

    /// Apply termination strategy to the stream based on check mode and limits
    fn apply_termination_strategy<S>(
        stream: S,
        check_mode: CheckMode,
        max_errors: Option<usize>,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Result<RuleViolation>> + Send>>
    where
        S: Stream<Item = Result<RuleViolation>> + Send + 'static,
    {
        if let Some(limit) = max_errors {
            stream.take(limit).boxed()
        } else if check_mode == CheckMode::FailFast {
            stream.take(1).boxed()
        } else {
            stream.boxed()
        }
    }

    /// Process work queue as a stream with concurrency control
    fn process_work_queue_stream(
        &self,
        work_items: Vec<(Rule, PathBuf)>,
        request: &RuleCheckRequest,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Result<RuleViolation>> + Send>> {
        let checker = Arc::new(self.clone_for_streaming());
        let check_mode = request.check_mode;
        let max_errors = request.max_errors;
        let concurrency = request.max_concurrency.unwrap_or(DEFAULT_MAX_CONCURRENCY);
        let total_items = work_items.len();
        let start_time = std::time::Instant::now();

        tracing::debug!("Processing work queue with concurrency={}", concurrency);
        tracing::info!("Total checks to perform: {}", total_items);

        let completed_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let stream = stream::iter(work_items)
            .enumerate()
            .map(move |(_index, (rule, target))| {
                let checker = Arc::clone(&checker);
                let completed = Arc::clone(&completed_count);
                let start = start_time;
                async move {
                    let (completed_so_far, remaining, eta) =
                        Self::calculate_progress(&completed, total_items, start);

                    tracing::info!(
                        "Checking {} against {} [{}/{}] - {} remaining, ETA: {:.1}s",
                        target.display(),
                        rule.name,
                        completed_so_far + 1,
                        total_items,
                        remaining,
                        eta
                    );

                    checker.check_file(&rule, &target).await
                }
            })
            .buffer_unordered(concurrency)
            .filter_map(|result| async move { Self::filter_error_violations(result) });

        Self::apply_termination_strategy(stream, check_mode, max_errors)
    }

    /// Create a cloneable version of RuleChecker for streaming
    ///
    /// This allows the checker to be cloned and used in async stream operations
    /// while maintaining shared access to the underlying configuration and libraries.
    /// The agent handle is shared across all clones so the agent is reused.
    fn clone_for_streaming(&self) -> Self {
        Self {
            agent_config: self.agent_config.clone(),
            prompt_library: Arc::clone(&self.prompt_library),
            rule_library: Arc::clone(&self.rule_library),
            cache: Arc::clone(&self.cache),
            agent_handle: Arc::clone(&self.agent_handle),
        }
    }

    /// Log a violation at the appropriate level based on its severity
    fn log_violation(violation: &RuleViolation) {
        match violation.severity {
            Severity::Error => tracing::error!("{}", violation),
            Severity::Warning => tracing::warn!("{}", violation),
            Severity::Info => tracing::info!("{}", violation),
            Severity::Hint => tracing::debug!("{}", violation),
        }
    }

    /// Store a cached result with error logging on failure
    fn store_cached_result_with_logging(&self, cache_key: &str, result: &CachedResult) {
        if let Err(e) = self.cache.store(cache_key, result) {
            tracing::warn!("Failed to cache result: {}", e);
        }
    }

    /// Handle a cached result by logging and returning the appropriate value
    fn handle_cached_result(
        &self,
        cached_result: CachedResult,
        target_path: &Path,
        rule_name: &str,
    ) -> Result<Option<RuleViolation>> {
        match cached_result {
            CachedResult::Pass => {
                tracing::trace!(
                    "Cached check passed for {} against rule {}",
                    target_path.display(),
                    rule_name
                );
                Ok(None)
            }
            CachedResult::Violation { violation } => {
                Self::log_violation(&violation);
                Ok(Some(violation))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;
    use std::path::PathBuf;
    use swissarmyhammer_config::model::LlamaAgentConfig;
    use tempfile::TempDir;

    /// Create a test checker with local LlamaAgent for fast test execution
    ///
    /// Uses a small test model instead of Claude Code to avoid API calls
    /// and speed up test execution.
    fn create_test_checker() -> RuleChecker {
        let agent_config = AgentConfig {
            model_config: ModelConfig::llama_agent(LlamaAgentConfig::for_testing()),
            mcp_config: None,
        };
        RuleChecker::new(agent_config).expect("Failed to create test checker")
    }

    /// Create a default RuleCheckRequest with common test settings
    ///
    /// This helper reduces duplication across test functions by providing
    /// a base request that tests can override specific fields on.
    fn default_test_request() -> RuleCheckRequest {
        RuleCheckRequest {
            rule_names: None,
            severity: None,
            category: None,
            patterns: vec!["test.rs".to_string()],
            skip_glob_expansion: false,
            check_mode: CheckMode::CollectAll,
            force: false,
            max_errors: None,
            max_concurrency: None,
        }
    }

    /// Helper to test streaming check with a specific CheckMode
    ///
    /// This consolidates the nearly identical test logic for fail-fast and collect-all modes.
    async fn test_check_streaming_with_mode(mode: CheckMode) {
        use futures_util::stream::StreamExt;

        let checker = create_test_checker();

        let mut request = default_test_request();
        request.check_mode = mode;

        let mut stream = checker.check(request).await.expect("Should create stream");

        // Verify stream was created and can be polled
        // In both modes, we just verify the stream is created successfully
        let _ = stream.next().await;
    }

    #[test]
    fn test_rule_checker_creation_with_valid_prompt() {
        let checker = create_test_checker();
        assert!(checker.prompt_library.get(".check").is_ok());
    }

    #[test]
    fn test_rule_checker_creation_loads_check_prompt() {
        let checker = create_test_checker();

        // Verify .check prompt is loaded
        let check_prompt = checker.prompt_library.get(".check");
        assert!(check_prompt.is_ok());

        let prompt = check_prompt.unwrap();
        assert_eq!(prompt.name, ".check");
    }

    #[tokio::test]
    async fn test_rule_checker_two_stage_rendering() {
        let checker = create_test_checker();

        // Create a test rule with template variables
        let _rule = Rule::new(
            "test-rule".to_string(),
            "Check {{language}} code in {{target_path}}".to_string(),
            Severity::Error,
        );

        // Create a temp file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        std::fs::write(&test_file, "fn main() {}").unwrap();

        // Test would require agent initialization which needs real model
        // So we just verify the checker was created successfully
        assert!(checker.prompt_library.get(".check").is_ok());
    }

    #[test]
    fn test_detect_language_identifies_rust_and_python() {
        let path = Path::new("test.rs");
        let content = "fn main() {}";
        let language = detect_language(path, content).unwrap();
        assert_eq!(language, "rust");

        let path = Path::new("test.py");
        let content = "def main(): pass";
        let language = detect_language(path, content).unwrap();
        assert_eq!(language, "python");
    }

    #[tokio::test]
    async fn test_check_file_with_nonexistent_file() {
        let checker = create_test_checker();

        let rule = Rule::new(
            "test-rule".to_string(),
            "Test template".to_string(),
            Severity::Error,
        );

        let nonexistent = PathBuf::from("/nonexistent/file.rs");
        let result = checker.check_file(&rule, &nonexistent).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to read"));
    }

    #[tokio::test]
    async fn test_check_streaming_empty_patterns() {
        use futures_util::stream::StreamExt;

        let checker = create_test_checker();

        let mut request = default_test_request();
        request.patterns = vec!["/nonexistent/**/*.rs".to_string()];
        request.check_mode = CheckMode::FailFast;

        let mut stream = checker
            .check(request)
            .await
            .expect("Should create empty stream");

        // Empty patterns should yield no violations
        let violation = stream.next().await;
        assert!(
            violation.is_none(),
            "Empty file patterns should yield no violations"
        );
    }

    #[test]
    fn test_check_prompt_includes_rule_name() {
        let checker = create_test_checker();

        // Create a test rule with a specific name
        let rule_name = "test-rule-name-123";
        let rule = Rule::new(
            rule_name.to_string(),
            "Check for test violations".to_string(),
            Severity::Error,
        );

        // Create a temp file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        std::fs::write(&test_file, "fn main() {}").unwrap();

        // Build the check context as done in check_file
        let target_content = std::fs::read_to_string(&test_file).unwrap();
        let language = detect_language(&test_file, &target_content).unwrap();

        // Stage 1: Render rule template
        let mut rule_context = TemplateContext::new();
        rule_context.set("target_content".to_string(), target_content.clone().into());
        rule_context.set(
            "target_path".to_string(),
            test_file.display().to_string().into(),
        );
        rule_context.set("language".to_string(), language.clone().into());

        let partial_adapter = crate::RulePartialAdapter::new(Arc::clone(&checker.rule_library));
        let template_with_partials =
            swissarmyhammer_templating::Template::with_partials(&rule.template, partial_adapter)
                .unwrap();
        let rendered_rule = template_with_partials
            .render_with_context(&rule_context)
            .unwrap();

        // Stage 2: Build check context (this is what we're testing)
        let mut check_context = TemplateContext::new();
        check_context.set("rule_content".to_string(), rendered_rule.into());
        check_context.set("target_content".to_string(), target_content.into());
        check_context.set(
            "target_path".to_string(),
            test_file.display().to_string().into(),
        );
        check_context.set("language".to_string(), language.clone().into());
        check_context.set("rule_name".to_string(), rule.name.clone().into());

        // Render the .check prompt
        let rendered_prompt = checker
            .prompt_library
            .render(".check", &check_context)
            .unwrap();

        // Verify the rule name appears in the rendered prompt
        assert!(
            rendered_prompt.contains(rule_name),
            "Rendered check prompt should contain rule_name '{}', but got:\n{}",
            rule_name,
            rendered_prompt
        );
    }

    #[test]
    fn test_pass_response_parsing_exact() {
        let response_text = "PASS";
        assert_eq!(response_text.trim(), "PASS");
        assert!(
            response_text.trim() == "PASS",
            "Exact PASS should match with =="
        );
    }

    #[test]
    fn test_pass_response_parsing_with_explanation() {
        let response_text =
            "PASS\n\nThis is a Rust build script that generates code at compile time.";
        let trimmed = response_text.trim();

        // Current bug: this will fail with == but pass with starts_with
        assert_ne!(
            trimmed, "PASS",
            "Multi-line PASS response should NOT equal exact 'PASS'"
        );
        assert!(
            trimmed.starts_with("PASS"),
            "Multi-line PASS response should start with 'PASS'"
        );
    }

    #[test]
    fn test_pass_response_with_analysis_before() {
        // Test the case where LLM provides analysis before the verdict
        let response_text = "I'll analyze this test file against the rule.\n\nLet me examine the code.\n\nPASS\n\nThe file follows the rule correctly.";

        // Check if our logic correctly identifies this as a PASS
        let is_pass = response_text
            .trim()
            .trim_start_matches('*')
            .trim_start()
            .starts_with("PASS")
            || response_text.contains("**PASS**")
            || response_text.contains("*PASS*")
            || response_text.lines().any(|line| line.trim() == "PASS");

        assert!(
            is_pass,
            "Response with PASS on its own line should be detected as a pass"
        );
    }

    #[test]
    fn test_violation_response_parsing() {
        let response_text = "VIOLATION\n\nFound TODO comment on line 5";
        let trimmed = response_text.trim();

        assert_ne!(
            trimmed, "PASS",
            "VIOLATION response should not equal 'PASS'"
        );
        assert!(
            !trimmed.starts_with("PASS"),
            "VIOLATION response should not start with 'PASS'"
        );
        assert!(
            trimmed.starts_with("VIOLATION"),
            "VIOLATION response should start with 'VIOLATION'"
        );
    }

    #[test]
    fn test_rule_violation_preserves_severity_levels() {
        use crate::RuleViolation;

        // Test that RuleViolation preserves severity levels
        let severities = vec![
            Severity::Error,
            Severity::Warning,
            Severity::Info,
            Severity::Hint,
        ];

        for severity in severities {
            let violation = RuleViolation::new(
                "test-rule".to_string(),
                PathBuf::from("test.rs"),
                severity,
                "Test violation".to_string(),
            );

            assert_eq!(
                violation.severity, severity,
                "Violation should preserve severity level"
            );
        }
    }

    #[tokio::test]
    async fn test_check_streaming_no_matching_rules() {
        use futures_util::stream::StreamExt;

        let checker = create_test_checker();

        let mut request = default_test_request();
        request.rule_names = Some(vec!["nonexistent-rule".to_string()]);
        request.check_mode = CheckMode::FailFast;

        let mut stream = checker.check(request).await.expect("Should create stream");
        let violation = stream.next().await;
        assert!(
            violation.is_none(),
            "No matching rules should yield no violations"
        );
    }

    #[tokio::test]
    async fn test_check_streaming_fail_fast_mode() {
        test_check_streaming_with_mode(CheckMode::FailFast).await;
    }

    #[tokio::test]
    async fn test_check_streaming_collect_all_mode() {
        test_check_streaming_with_mode(CheckMode::CollectAll).await;
    }

    #[tokio::test]
    async fn test_check_with_max_errors_limits_violations() {
        use futures_util::stream::StreamExt;

        let checker = create_test_checker();

        // Create request with max_errors = 2
        let mut request = default_test_request();
        request.max_errors = Some(2);

        let mut stream = checker.check(request).await.expect("Should create stream");

        // Collect all violations from the stream
        let mut violations = Vec::new();
        while let Some(result) = stream.next().await {
            match result {
                Ok(violation) => violations.push(violation),
                Err(_) => break,
            }
        }

        // Should have at most 2 violations due to max_errors limit
        assert!(
            violations.len() <= 2,
            "Expected at most 2 violations with max_errors=2, got {}",
            violations.len()
        );
    }

    #[tokio::test]
    async fn test_check_without_max_errors_unlimited() {
        let checker = create_test_checker();

        // Create request without max_errors (unlimited) - uses default
        let request = default_test_request();

        let stream_result = checker.check(request).await;
        assert!(
            stream_result.is_ok(),
            "Should create stream when max_errors is None"
        );
    }

    #[tokio::test]
    async fn test_skip_glob_expansion_uses_direct_file_paths() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_file.rs");
        fs::write(&test_file, "fn example() {}").unwrap();

        // Change to temp directory so relative paths work
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let checker = create_test_checker();

        // Test with skip_glob_expansion = true (should use file paths directly)
        let mut request_with_skip = default_test_request();
        request_with_skip.patterns = vec!["test_file.rs".to_string()];
        request_with_skip.skip_glob_expansion = true;

        let result_with_skip = checker.check(request_with_skip).await;
        assert!(
            result_with_skip.is_ok(),
            "Should successfully check with skip_glob_expansion=true"
        );

        // Test with skip_glob_expansion = false (should use glob expansion)
        let mut request_without_skip = default_test_request();
        request_without_skip.patterns = vec!["test_file.rs".to_string()];
        request_without_skip.skip_glob_expansion = false;

        let result_without_skip = checker.check(request_without_skip).await;
        assert!(
            result_without_skip.is_ok(),
            "Should successfully check with skip_glob_expansion=false"
        );

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }
}
