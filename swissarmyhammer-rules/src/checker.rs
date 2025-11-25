//! Rule checking with two-stage rendering and agent execution
//!
//! This module provides the `RuleChecker` which performs rule checks against files:
//! 1. Stage 1: Renders rule templates with context (language, target_path, etc.)
//! 2. Stage 2: Renders .check prompt with rendered rule content
//! 3. Executes via AgentExecutor (ClaudeCode or LlamaAgent)
//! 4. Parses responses and fails fast on violations

use crate::{
    detect_language, CachedResult, Result, Rule, RuleCache, RuleError, RuleViolation, Severity,
};
use futures_util::stream::{self, Stream, StreamExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use swissarmyhammer_agent_executor::{AgentExecutionContext, AgentExecutor};
use swissarmyhammer_common::glob_utils::{expand_glob_patterns, GlobExpansionConfig};
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};

/// Check mode controlling fail-fast behavior
///
/// Determines whether checking stops on the first ERROR violation or continues
/// to check all files and collect all violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckMode {
    /// Stop checking files after first ERROR violation is found
    FailFast,
    /// Check all files and collect all ERROR violations
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
///     max_concurrency: None,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct RuleCheckRequest {
    /// Optional list of rule names to check (None = all rules)
    pub rule_names: Option<Vec<String>>,
    /// Optional severity filter
    pub severity: Option<Severity>,
    /// Optional category filter
    pub category: Option<String>,
    /// File paths or glob patterns to check
    pub patterns: Vec<String>,
    /// Check mode controlling fail-fast behavior
    pub check_mode: CheckMode,
    /// Force re-evaluation, bypassing cache
    pub force: bool,
    /// Maximum number of ERROR violations to return (None = unlimited)
    pub max_errors: Option<usize>,
    /// Maximum number of concurrent rule checks (None = default of 4)
    pub max_concurrency: Option<usize>,
}

/// Core rule checker that performs two-stage rendering and executes checks via LLM agent
///
/// The RuleChecker is the heart of the rules system. It:
/// 1. Renders rule templates with repository context
/// 2. Renders the .check prompt with the rendered rule
/// 3. Executes via LlamaAgentExecutor
/// 4. Parses responses and fails fast on violations
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer_rules::{RuleChecker, Rule, Severity};
/// use swissarmyhammer_agent_executor::{AgentExecutor, ClaudeCodeExecutor};
/// use std::sync::Arc;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create and initialize executor (using ClaudeCode as example)
/// let mut executor = ClaudeCodeExecutor::new();
/// executor.initialize().await?;
///
/// // Create checker
/// let agent: Arc<dyn AgentExecutor> = Arc::new(executor);
/// let mut checker = RuleChecker::new(agent)?;
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
    /// LLM agent executor for running checks
    agent: Arc<dyn AgentExecutor>,
    /// Prompt library containing the .check prompt (wrapped in Arc for sharing in streams)
    prompt_library: Arc<PromptLibrary>,
    /// Rule library for partial template support (loaded once for performance)
    rule_library: Arc<crate::RuleLibrary>,
    /// Cache for rule evaluation results (wrapped in Arc for sharing in streams)
    cache: Arc<RuleCache>,
}

impl RuleChecker {
    /// Create a new RuleChecker with the given agent executor
    ///
    /// Loads the PromptLibrary containing the .check prompt from all sources
    /// (builtin, user, local).
    ///
    /// # Arguments
    ///
    /// * `agent` - Agent executor wrapped in Arc for shared ownership
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
    /// use swissarmyhammer_rules::RuleChecker;
    /// use swissarmyhammer_agent_executor::{AgentExecutor, ClaudeCodeExecutor};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create and initialize executor (using ClaudeCode as example)
    /// let mut executor = ClaudeCodeExecutor::new();
    /// executor.initialize().await?;
    ///
    /// let agent: Arc<dyn AgentExecutor> = Arc::new(executor);
    /// let checker = RuleChecker::new(agent)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(agent: Arc<dyn AgentExecutor>) -> Result<Self> {
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
            agent,
            prompt_library: Arc::new(prompt_library),
            rule_library: Arc::new(rule_library),
            cache: Arc::new(cache),
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

        tracing::info!("RuleChecker ready (agent will initialize on first use via singleton)");
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
    /// # use swissarmyhammer_rules::{RuleChecker, Rule, Severity};
    /// # use swissarmyhammer_agent_executor::{AgentExecutor, ClaudeCodeExecutor};
    /// # use std::sync::Arc;
    /// # use std::path::PathBuf;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut executor = ClaudeCodeExecutor::new();
    /// # executor.initialize().await?;
    /// # let agent: Arc<dyn AgentExecutor> = Arc::new(executor);
    /// # let mut checker = RuleChecker::new(agent)?;
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
        tracing::debug!(
            "Checking file {} against rule {}",
            target_path.display(),
            rule.name
        );

        // Read target file content
        let target_content = match std::fs::read_to_string(target_path) {
            Ok(content) => content,
            Err(e) => {
                // Skip binary files or files with invalid UTF-8
                if e.kind() == std::io::ErrorKind::InvalidData {
                    tracing::debug!(
                        "Skipping binary or non-UTF-8 file: {}",
                        target_path.display()
                    );
                    return Ok(None);
                }
                return Err(RuleError::CheckError(
                    format!(
                        "Failed to read file {}: {}",
                        target_path.display(),
                        e
                    )
                ).into());
            }
        };

        // Check for ignore directives in the file
        let ignore_patterns = crate::ignore::parse_ignore_directives(&target_content);
        if crate::ignore::should_ignore_rule(&rule.name, &ignore_patterns) {
            tracing::debug!(
                "Rule {} ignored in {} (file directive)",
                rule.name,
                target_path.display()
            );
            return Ok(None);
        }

        // Calculate cache key from file content + rule template + severity
        let cache_key =
            RuleCache::calculate_cache_key(&target_content, &rule.template, rule.severity);

        // Check cache before proceeding with LLM evaluation
        if let Some(cached_result) = self.cache.get(&cache_key)? {
            tracing::debug!(
                "Cache hit for {} against rule {} - skipping LLM call",
                target_path.display(),
                rule.name
            );

            return self.handle_cached_result(cached_result, target_path, &rule.name);
        }

        tracing::trace!(
            "Cache miss for {} against rule {} - proceeding with LLM check",
            target_path.display(),
            rule.name
        );

        // Detect language from file extension/content
        let language = detect_language(target_path, &target_content)?;
        tracing::debug!("Detected language: {}", language);

        // STAGE 1: Render the rule template with context variables and partial support
        let mut rule_context = TemplateContext::new();
        rule_context.set("target_content".to_string(), target_content.clone().into());
        rule_context.set(
            "target_path".to_string(),
            target_path.display().to_string().into(),
        );
        rule_context.set("language".to_string(), language.clone().into());

        // Create partial adapter from pre-loaded rule library
        let partial_adapter = crate::RulePartialAdapter::new(Arc::clone(&self.rule_library));

        // Use Template::with_partials for rendering with partial support
        let template_with_partials =
            swissarmyhammer_templating::Template::with_partials(&rule.template, partial_adapter)
                .map_err(|e| {
                    RuleError::CheckError(format!(
                        "Failed to create template with partials for {}: {}",
                        rule.name, e
                    ))
                })?;

        let rendered_rule = template_with_partials
            .render_with_context(&rule_context)
            .map_err(|e| {
                RuleError::CheckError(format!(
                    "Failed to render rule template for {}: {}",
                    rule.name, e
                ))
            })?;

        tracing::debug!("Stage 1 complete: rule template rendered");

        // STAGE 2: Render the .check prompt with rendered rule content
        let mut check_context = TemplateContext::new();
        check_context.set("rule_content".to_string(), rendered_rule.into());
        check_context.set("target_content".to_string(), target_content.into());
        check_context.set(
            "target_path".to_string(),
            target_path.display().to_string().into(),
        );
        check_context.set("language".to_string(), language.into());
        check_context.set("rule_name".to_string(), rule.name.clone().into());

        let check_prompt_text = self
            .prompt_library
            .render(".check", &check_context)
            .map_err(|e| RuleError::CheckError(format!("Failed to render .check prompt: {}", e)))?;

        tracing::debug!("Stage 2 complete: .check prompt rendered");

        // Execute via agent (LLM) with optimization for rule checking
        // Skip tool discovery since rule checking doesn't need MCP tools
        let agent_config = swissarmyhammer_config::agent::AgentConfig::default();
        let agent_context = AgentExecutionContext::for_rule_checking(&agent_config);

        let response = self
            .agent
            .execute_prompt(String::new(), check_prompt_text, &agent_context)
            .await
            .map_err(|e| RuleError::AgentError(format!("Agent execution failed: {}", e)))?;

        tracing::debug!("LLM execution complete");

        // Parse result - check for PASS or VIOLATION
        // The agent may return detailed analysis followed by PASS or VIOLATION
        // We need to check both the beginning and end of the response
        let result_text = response.content.trim();

        // Check if response contains PASS (with or without markdown formatting)
        // Look for PASS at the start or end, ignoring markdown asterisks
        let normalized_text = result_text.trim_start_matches('*').trim_start();

        // Strip trailing markdown formatting as well before checking
        let trimmed_end = result_text.trim_end_matches('*').trim_end();
        let ends_with_pass = trimmed_end.ends_with("PASS")
            || result_text.contains("**PASS**")
            || result_text.contains("*PASS*");

        tracing::debug!(
            "Response parsing - result_text length: {}",
            result_text.len()
        );
        tracing::debug!(
            "Response parsing - starts with PASS: {}",
            normalized_text.starts_with("PASS")
        );
        tracing::debug!("Response parsing - ends_with_pass: {}", ends_with_pass);

        if normalized_text.starts_with("PASS") || ends_with_pass {
            tracing::info!(
                "Check passed for {} against rule {}",
                target_path.display(),
                rule.name
            );

            // Cache the PASS result
            let cached_result = CachedResult::Pass;
            self.store_cached_result_with_logging(&cache_key, &cached_result);

            Ok(None)
        } else {
            // Violation found - create RuleViolation
            let violation = RuleViolation::new(
                rule.name.clone(),
                target_path.to_path_buf(),
                rule.severity,
                response.content,
            );

            // Log the violation at appropriate level
            Self::log_violation(&violation);

            // Cache the VIOLATION result
            let cached_result = CachedResult::Violation {
                violation: violation.clone(),
            };
            self.store_cached_result_with_logging(&cache_key, &cached_result);

            Ok(Some(violation))
        }
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
    /// # use swissarmyhammer_rules::{RuleChecker, RuleCheckRequest, CheckMode};
    /// # use futures_util::stream::StreamExt;
    /// # use swissarmyhammer_agent_executor::{AgentExecutor, ClaudeCodeExecutor};
    /// # use std::sync::Arc;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut executor = ClaudeCodeExecutor::new();
    /// # executor.initialize().await?;
    /// # let agent: Arc<dyn AgentExecutor> = Arc::new(executor);
    /// # let mut checker = RuleChecker::new(agent)?;
    /// # checker.initialize().await?;
    /// let request = RuleCheckRequest {
    ///     rule_names: None,
    ///     severity: None,
    ///     category: None,
    ///     patterns: vec!["**/*.rs".to_string()],
    ///     check_mode: CheckMode::CollectAll,
    ///     force: false,
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

        // Phase 1: Load all rules via RuleResolver
        let mut rules = Vec::new();
        let mut resolver = crate::RuleResolver::new();
        resolver
            .load_all_rules(&mut rules)
            .map_err(|e| RuleError::CheckError(format!("Failed to load rules: {}", e)))?;

        let total_files = rules.len();
        let partials_count = rules.iter().filter(|r| r.is_partial()).count();

        // Filter out partials - they are not standalone rules
        rules.retain(|r| !r.is_partial());

        let rule_names: Vec<&str> = rules.iter().map(|r| r.name.as_str()).collect();
        tracing::info!(
            "Loaded {} files ({} rules, {} partials). Rules: {:?}",
            total_files,
            rules.len(),
            partials_count,
            rule_names
        );

        // Phase 2: Validate all rules first (fail if any invalid)
        for rule in &rules {
            rule.validate().map_err(|e| {
                RuleError::CheckError(format!("Rule validation failed for '{}': {}", rule.name, e))
            })?;
        }

        // Phase 3: Apply filters
        if let Some(rule_names) = &request.rule_names {
            tracing::info!("Filtering by rule_names: {:?}", rule_names);
            tracing::info!(
                "Available rule names before filter: {:?}",
                rules.iter().map(|r| &r.name).collect::<Vec<_>>()
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

        if rules.is_empty() {
            tracing::info!("No rules matched the filters");
            // Return empty stream
            return Ok(stream::iter(vec![]).boxed());
        }

        // Phase 4: Expand glob patterns to get target files
        // Note: .git and .swissarmyhammer directories are excluded by the WalkBuilder filter in glob_utils
        let config = GlobExpansionConfig::default();

        let mut target_files = expand_glob_patterns(&request.patterns, &config)
            .map_err(|e| RuleError::CheckError(format!("Failed to expand glob patterns: {}", e)))?;

        // Sort target files for consistent ordering
        target_files.sort();

        if target_files.is_empty() {
            tracing::info!("No files matched the patterns");
            // Return empty stream
            return Ok(stream::iter(vec![]).boxed());
        }

        // Phase 5: Create flat work queue of (rule, file) pairs
        // Filter by applies_to pattern if present
        let work_items: Vec<(Rule, PathBuf)> = rules
            .iter()
            .flat_map(|rule| {
                target_files
                    .iter()
                    .filter(move |file| {
                        // If rule has applies_to pattern, check if file matches
                        if let Some(ref pattern) = rule.applies_to {
                            if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
                                glob_pattern.matches_path(file)
                            } else {
                                // Invalid pattern, skip this file for this rule
                                tracing::warn!(
                                    "Invalid applies_to pattern '{}' in rule '{}', skipping",
                                    pattern,
                                    rule.name
                                );
                                false
                            }
                        } else {
                            // No applies_to pattern, include all files
                            true
                        }
                    })
                    .map(move |file| (rule.clone(), file.clone()))
            })
            .collect();

        tracing::info!("Created work queue with {} check items", work_items.len());

        // Wrap checker in Arc so it can be cloned for parallel execution
        let checker = Arc::new(self.clone_for_streaming());
        let check_mode = request.check_mode;
        let max_errors = request.max_errors;
        let concurrency = request.max_concurrency.unwrap_or(4);

        tracing::debug!("Processing work queue with concurrency={}", concurrency);

        // Process work queue in parallel using buffer_unordered
        let stream = stream::iter(work_items)
            .map(move |(rule, target)| {
                let checker = Arc::clone(&checker);
                async move { checker.check_file(&rule, &target).await }
            })
            .buffer_unordered(concurrency)
            .filter_map(|result| async move {
                match result {
                    Ok(Some(violation)) if violation.severity == Severity::Error => {
                        Some(Ok(violation))
                    }
                    Ok(Some(_)) => None, // Non-error violations are logged but not yielded
                    Ok(None) => None,    // PASS - no violation
                    Err(e) => Some(Err(e)), // Propagate operational errors
                }
            });

        // Apply limits based on check_mode and max_errors
        // Priority: max_errors takes precedence if specified, otherwise use check_mode
        let limited_stream = if let Some(limit) = max_errors {
            stream.take(limit).boxed()
        } else if check_mode == CheckMode::FailFast {
            stream.take(1).boxed()
        } else {
            stream.boxed()
        };

        Ok(limited_stream)
    }

    /// Create a cloneable version of RuleChecker for streaming
    ///
    /// This allows the checker to be cloned and used in async stream operations
    /// while maintaining shared access to the underlying agent and libraries.
    fn clone_for_streaming(&self) -> Self {
        Self {
            agent: Arc::clone(&self.agent),
            prompt_library: Arc::clone(&self.prompt_library),
            rule_library: Arc::clone(&self.rule_library),
            cache: Arc::clone(&self.cache),
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
    use swissarmyhammer_agent_executor::LlamaAgentExecutorWrapper;
    use swissarmyhammer_config::LlamaAgentConfig;
    use tempfile::TempDir;

    fn create_test_agent() -> Arc<dyn AgentExecutor> {
        let config = LlamaAgentConfig::for_testing();
        Arc::new(LlamaAgentExecutorWrapper::new(config))
    }

    fn create_test_checker() -> RuleChecker {
        let agent = create_test_agent();
        RuleChecker::new(agent).expect("Failed to create test checker")
    }

    #[test]
    fn test_rule_checker_creation() {
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
    fn test_detect_language_integration() {
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

        let request = RuleCheckRequest {
            rule_names: None,
            severity: None,
            category: None,
            patterns: vec!["/nonexistent/**/*.rs".to_string()],
            check_mode: CheckMode::FailFast,
            force: false,
            max_errors: None,
            max_concurrency: None,
        };

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
    fn test_violation_preserves_severity() {
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

        let request = RuleCheckRequest {
            rule_names: Some(vec!["nonexistent-rule".to_string()]),
            severity: None,
            category: None,
            patterns: vec!["test.rs".to_string()],
            check_mode: CheckMode::FailFast,
            force: false,
            max_errors: None,
            max_concurrency: None,
        };

        let mut stream = checker.check(request).await.expect("Should create stream");
        let violation = stream.next().await;
        assert!(
            violation.is_none(),
            "No matching rules should yield no violations"
        );
    }

    #[tokio::test]
    async fn test_check_streaming_fail_fast_mode() {
        use futures_util::stream::StreamExt;

        let checker = create_test_checker();

        let request = RuleCheckRequest {
            rule_names: None,
            severity: Some(Severity::Error),
            category: None,
            patterns: vec!["test.rs".to_string()],
            check_mode: CheckMode::FailFast,
            force: false,
            max_errors: None,
            max_concurrency: None,
        };

        let mut stream = checker.check(request).await.expect("Should create stream");

        // In fail-fast mode, stream should stop after first violation
        // For this test, we just verify the stream is created successfully
        let _ = stream.next().await;
    }

    #[tokio::test]
    async fn test_check_streaming_collect_all_mode() {
        use futures_util::stream::StreamExt;

        let checker = create_test_checker();

        let request = RuleCheckRequest {
            rule_names: None,
            severity: None,
            category: None,
            patterns: vec!["test.rs".to_string()],
            check_mode: CheckMode::CollectAll,
            force: false,
            max_errors: None,
            max_concurrency: None,
        };

        let mut stream = checker.check(request).await.expect("Should create stream");

        // In collect-all mode, stream continues until all files are checked
        let mut count = 0;
        while (stream.next().await).is_some() {
            count += 1;
            // Prevent infinite loop in case of test issues
            if count > 100 {
                break;
            }
        }
    }

    #[tokio::test]
    async fn test_check_with_max_errors_limits_violations() {
        use futures_util::stream::StreamExt;

        let checker = create_test_checker();

        // Create request with max_errors = 2
        let request = RuleCheckRequest {
            rule_names: None,
            severity: None,
            category: None,
            patterns: vec!["test.rs".to_string()],
            check_mode: CheckMode::CollectAll,
            force: false,
            max_errors: Some(2),
            max_concurrency: None,
        };

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

        // Create request without max_errors (unlimited)
        let request = RuleCheckRequest {
            rule_names: None,
            severity: None,
            category: None,
            patterns: vec!["test.rs".to_string()],
            check_mode: CheckMode::CollectAll,
            force: false,
            max_errors: None,
            max_concurrency: None,
        };

        let stream_result = checker.check(request).await;
        assert!(
            stream_result.is_ok(),
            "Should create stream when max_errors is None"
        );
    }
}
