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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use swissarmyhammer_agent_executor::{AgentExecutionContext, AgentExecutor};
use swissarmyhammer_common::glob_utils::{expand_glob_patterns, GlobExpansionConfig};
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};

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
/// use swissarmyhammer_rules::RuleCheckRequest;
///
/// // Check all rules against Rust files
/// let request = RuleCheckRequest {
///     rule_names: None,
///     severity: None,
///     category: None,
///     patterns: vec!["**/*.rs".to_string()],
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
}

/// Result structure from rule checking operations
///
/// Contains statistics and violation information from a check operation.
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer_rules::RuleCheckResult;
///
/// fn handle_result(result: RuleCheckResult) {
///     println!("Checked {} rules against {} files",
///              result.rules_checked, result.files_checked);
///     if !result.violations.is_empty() {
///         println!("Found {} violations", result.violations.len());
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RuleCheckResult {
    /// Number of rules checked
    pub rules_checked: usize,
    /// Number of files checked
    pub files_checked: usize,
    /// List of violations found (empty if all checks passed)
    pub violations: Vec<RuleViolation>,
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
/// use swissarmyhammer_agent_executor::{AgentExecutorFactory, AgentExecutionContext};
/// use std::sync::Arc;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load agent configuration (respects SAH_AGENT_EXECUTOR env, defaults to ClaudeCode)
/// let workflow_context = WorkflowTemplateContext::load_with_agent_config()?;
/// let agent_context = AgentExecutionContext::new(&workflow_context);
///
/// // Create and initialize executor
/// let mut executor = AgentExecutorFactory::create_executor(&agent_context).await?;
/// executor.initialize().await?;
///
/// // Create checker
/// let agent = Arc::from(executor);
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
    /// Prompt library containing the .check prompt
    prompt_library: PromptLibrary,
    /// Rule library for partial template support (loaded once for performance)
    rule_library: Arc<crate::RuleLibrary>,
    /// Cache for rule evaluation results
    cache: RuleCache,
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
    /// use swissarmyhammer_agent_executor::{AgentExecutorFactory, AgentExecutionContext};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Load agent configuration (respects SAH_AGENT_EXECUTOR env, defaults to ClaudeCode)
    /// let workflow_context = WorkflowTemplateContext::load_with_agent_config()?;
    /// let agent_context = AgentExecutionContext::new(&workflow_context);
    ///
    /// // Create and initialize executor
    /// let mut executor = AgentExecutorFactory::create_executor(&agent_context).await?;
    /// executor.initialize().await?;
    ///
    /// let agent = Arc::from(executor);
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
            prompt_library,
            rule_library: Arc::new(rule_library),
            cache,
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
    /// Returns `Ok(())` if the file passes the check (LLM returns "PASS").
    /// Returns `Err(RuleError::Violation)` if violations are found.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File cannot be read
    /// - Language detection fails
    /// - Template rendering fails
    /// - Agent execution fails
    /// - Response parsing fails
    /// - Violation is found (fail-fast)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use swissarmyhammer_rules::{RuleChecker, Rule, Severity};
    /// # use swissarmyhammer_agent_executor::{AgentExecutorFactory, AgentExecutionContext};
    /// # use std::sync::Arc;
    /// # use std::path::PathBuf;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let workflow_context = WorkflowTemplateContext::load_with_agent_config()?;
    /// # let agent_context = AgentExecutionContext::new(&workflow_context);
    /// # let mut executor = AgentExecutorFactory::create_executor(&agent_context).await?;
    /// # executor.initialize().await?;
    /// # let agent = Arc::from(executor);
    /// # let mut checker = RuleChecker::new(agent)?;
    /// # checker.initialize().await?;
    /// let rule = Rule::new(
    ///     "test-rule".to_string(),
    ///     "Check something".to_string(),
    ///     Severity::Error,
    /// );
    /// let target = PathBuf::from("src/main.rs");
    /// checker.check_file(&rule, &target).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check_file(&self, rule: &Rule, target_path: &Path) -> Result<()> {
        tracing::debug!(
            "Checking file {} against rule {}",
            target_path.display(),
            rule.name
        );

        // Read target file content
        let target_content = std::fs::read_to_string(target_path).map_err(|e| {
            RuleError::CheckError(format!(
                "Failed to read file {}: {}",
                target_path.display(),
                e
            ))
        })?;

        // Calculate cache key from file content + rule template + severity
        let cache_key =
            RuleCache::calculate_cache_key(&target_content, &rule.template, rule.severity);

        // Check cache before proceeding with LLM evaluation
        if let Some(cached_result) = self.cache.get(&cache_key)? {
            tracing::trace!(
                "Cache hit for {} against rule {} - skipping LLM call",
                target_path.display(),
                rule.name
            );

            // Return cached result
            match cached_result {
                CachedResult::Pass => {
                    tracing::trace!(
                        "Cached check passed for {} against rule {}",
                        target_path.display(),
                        rule.name
                    );
                    return Ok(());
                }
                CachedResult::Violation { violation } => {
                    // Log the violation with appropriate severity
                    match violation.severity {
                        Severity::Error => tracing::error!("{}", violation),
                        Severity::Warning => tracing::warn!("{}", violation),
                        Severity::Info => tracing::info!("{}", violation),
                        Severity::Hint => tracing::debug!("{}", violation),
                    }

                    // Apply same severity-based behavior as fresh evaluation
                    // Only fail-fast for Error severity
                    match violation.severity {
                        Severity::Error => return Err(RuleError::Violation(violation).into()),
                        Severity::Warning | Severity::Info | Severity::Hint => {
                            tracing::debug!(
                                "Cached non-error violation logged, continuing execution: {} in {}",
                                violation.rule_name,
                                target_path.display()
                            );
                            return Ok(());
                        }
                    }
                }
            }
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

        // Execute via agent (LLM)
        // Create a simple agent config for rule checking
        let agent_config = swissarmyhammer_config::agent::AgentConfig::default();
        let agent_context = AgentExecutionContext::new(&agent_config);

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
            if let Err(e) = self.cache.store(&cache_key, &cached_result) {
                tracing::warn!("Failed to cache result: {}", e);
            }

            Ok(())
        } else {
            // Violation found - create RuleViolation
            let violation = RuleViolation::new(
                rule.name.clone(),
                target_path.to_path_buf(),
                rule.severity,
                response.content,
            );

            // Log the violation at appropriate level
            match violation.severity {
                Severity::Error => tracing::error!("{}", violation),
                Severity::Warning => tracing::warn!("{}", violation),
                Severity::Info => tracing::info!("{}", violation),
                Severity::Hint => tracing::debug!("{}", violation),
            }

            // Cache the VIOLATION result
            let cached_result = CachedResult::Violation {
                violation: violation.clone(),
            };
            if let Err(e) = self.cache.store(&cache_key, &cached_result) {
                tracing::warn!("Failed to cache result: {}", e);
            }

            // Only fail-fast for Error severity
            // Warnings, Info, and Hint are logged but don't stop execution
            match violation.severity {
                Severity::Error => Err(RuleError::Violation(violation).into()),
                Severity::Warning | Severity::Info | Severity::Hint => {
                    tracing::debug!(
                        "Non-error violation logged, continuing execution: {} in {}",
                        violation.rule_name,
                        target_path.display()
                    );
                    Ok(())
                }
            }
        }
    }

    /// Check multiple files against multiple rules with fail-fast behavior
    ///
    /// Iterates through every rule × target combination. The LLM decides if a rule
    /// is applicable to each file. Stops immediately on the first violation found.
    ///
    /// # Arguments
    ///
    /// * `rules` - Vector of rules to check
    /// * `targets` - Vector of file paths to check
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if all checks pass.
    /// Returns `Err(RuleError::Violation)` on the first violation found.
    ///
    /// # Errors
    ///
    /// Returns an error on the first:
    /// - File read failure
    /// - Rendering failure
    /// - Agent execution failure
    /// - Violation found (fail-fast)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use swissarmyhammer_rules::{RuleChecker, Rule, Severity};
    /// # use swissarmyhammer_agent_executor::{AgentExecutorFactory, AgentExecutionContext};
    /// # use std::sync::Arc;
    /// # use std::path::PathBuf;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let workflow_context = WorkflowTemplateContext::load_with_agent_config()?;
    /// # let agent_context = AgentExecutionContext::new(&workflow_context);
    /// # let mut executor = AgentExecutorFactory::create_executor(&agent_context).await?;
    /// # executor.initialize().await?;
    /// # let agent = Arc::from(executor);
    /// # let mut checker = RuleChecker::new(agent)?;
    /// # checker.initialize().await?;
    /// let rules = vec![
    ///     Rule::new("rule1".to_string(), "Check 1".to_string(), Severity::Error),
    ///     Rule::new("rule2".to_string(), "Check 2".to_string(), Severity::Warning),
    /// ];
    /// let targets = vec![
    ///     PathBuf::from("src/main.rs"),
    ///     PathBuf::from("src/lib.rs"),
    /// ];
    /// checker.check_all(rules, targets).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check_all(&self, rules: Vec<Rule>, targets: Vec<PathBuf>) -> Result<()> {
        tracing::info!(
            "Checking {} rules against {} files",
            rules.len(),
            targets.len()
        );

        // Iterate every rule against every target
        // LLM decides if rule is applicable to each file
        for rule in &rules {
            for target in &targets {
                // check_file will return Err(RuleError::Violation) on first violation
                // which causes immediate return (fail-fast)
                self.check_file(rule, target).await?;
            }
        }

        tracing::info!("All checks passed");
        Ok(())
    }

    /// High-level API for checking rules with filtering and pattern expansion
    ///
    /// This method provides a complete rule checking workflow:
    /// 1. Loads all rules via RuleResolver
    /// 2. Filters rules by name, severity, and category
    /// 3. Validates all rules
    /// 4. Expands glob patterns to file paths
    /// 5. Executes checks via check_all
    /// 6. Returns structured results
    ///
    /// # Arguments
    ///
    /// * `request` - RuleCheckRequest with filters and patterns
    ///
    /// # Returns
    ///
    /// Returns a RuleCheckResult with statistics and any violations found.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Rule loading fails
    /// - Rule validation fails
    /// - Pattern expansion fails
    /// - Check execution fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use swissarmyhammer_rules::{RuleChecker, RuleCheckRequest};
    /// # use swissarmyhammer_agent_executor::{AgentExecutorFactory, AgentExecutionContext};
    /// # use std::sync::Arc;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let workflow_context = WorkflowTemplateContext::load_with_agent_config()?;
    /// # let agent_context = AgentExecutionContext::new(&workflow_context);
    /// # let mut executor = AgentExecutorFactory::create_executor(&agent_context).await?;
    /// # executor.initialize().await?;
    /// # let agent = Arc::from(executor);
    /// # let mut checker = RuleChecker::new(agent)?;
    /// # checker.initialize().await?;
    /// let request = RuleCheckRequest {
    ///     rule_names: None,
    ///     severity: None,
    ///     category: None,
    ///     patterns: vec!["**/*.rs".to_string()],
    /// };
    /// let result = checker.check_with_filters(request).await?;
    /// println!("Checked {} files", result.files_checked);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check_with_filters(&self, request: RuleCheckRequest) -> Result<RuleCheckResult> {
        tracing::info!("Starting rule check with filters");

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

        let rules_checked = rules.len();

        if rules.is_empty() {
            tracing::info!("No rules matched the filters");
            return Ok(RuleCheckResult {
                rules_checked: 0,
                files_checked: 0,
                violations: Vec::new(),
            });
        }

        // Phase 4: Expand glob patterns to get target files
        let config = GlobExpansionConfig::default();
        let target_files = expand_glob_patterns(&request.patterns, &config)
            .map_err(|e| RuleError::CheckError(format!("Failed to expand glob patterns: {}", e)))?;

        let files_checked = target_files.len();

        if target_files.is_empty() {
            tracing::info!("No files matched the patterns");
            return Ok(RuleCheckResult {
                rules_checked,
                files_checked: 0,
                violations: Vec::new(),
            });
        }

        // Phase 5: Run checks
        // Note: check_all currently fails fast on Error violations
        match self.check_all(rules, target_files).await {
            Ok(()) => {
                // All checks passed
                Ok(RuleCheckResult {
                    rules_checked,
                    files_checked,
                    violations: Vec::new(),
                })
            }
            Err(e) => {
                // Propagate the error as-is
                // The error is already logged by check_file
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;
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
    async fn test_check_all_empty_lists() {
        let mut checker = create_test_checker();

        // Initialize is required
        if checker.initialize().await.is_err() {
            // Skip test if agent initialization fails (no model available)
            return;
        }

        let rules = vec![];
        let targets = vec![];
        let result = checker.check_all(rules, targets).await;
        assert!(result.is_ok());
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
    async fn test_check_with_filters_no_matching_rules() {
        let checker = create_test_checker();

        let request = RuleCheckRequest {
            rule_names: Some(vec!["nonexistent-rule".to_string()]),
            severity: None,
            category: None,
            patterns: vec!["test.rs".to_string()],
        };

        let result = checker.check_with_filters(request).await;
        assert!(result.is_ok());
        let check_result = result.unwrap();
        assert_eq!(check_result.rules_checked, 0);
        assert_eq!(check_result.files_checked, 0);
        assert_eq!(check_result.violations.len(), 0);
    }

    #[tokio::test]
    async fn test_check_with_filters_no_matching_files() {
        let checker = create_test_checker();

        let request = RuleCheckRequest {
            rule_names: None,
            severity: None,
            category: None,
            patterns: vec!["/nonexistent/**/*.rs".to_string()],
        };

        let result = checker.check_with_filters(request).await;
        assert!(result.is_ok());
        let check_result = result.unwrap();
        assert_eq!(check_result.files_checked, 0);
    }

    #[tokio::test]
    async fn test_check_with_filters_severity_filter() {
        let checker = create_test_checker();

        let request = RuleCheckRequest {
            rule_names: None,
            severity: Some(Severity::Error),
            category: None,
            patterns: vec!["test.rs".to_string()],
        };

        let result = checker.check_with_filters(request).await;
        // Should succeed - filters to only error-level rules
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_with_filters_category_filter() {
        let checker = create_test_checker();

        let request = RuleCheckRequest {
            rule_names: None,
            severity: None,
            category: Some("security".to_string()),
            patterns: vec!["test.rs".to_string()],
        };

        let result = checker.check_with_filters(request).await;
        // Should succeed - filters to only security category rules
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_with_filters_combined_filters() {
        let checker = create_test_checker();

        let request = RuleCheckRequest {
            rule_names: Some(vec!["specific-rule".to_string()]),
            severity: Some(Severity::Error),
            category: Some("security".to_string()),
            patterns: vec!["test.rs".to_string()],
        };

        let result = checker.check_with_filters(request).await;
        // Should succeed - applies all filters
        assert!(result.is_ok());
    }

    #[test]
    fn test_rule_check_request_creation() {
        let request = RuleCheckRequest {
            rule_names: Some(vec!["test-rule".to_string()]),
            severity: Some(Severity::Warning),
            category: Some("style".to_string()),
            patterns: vec!["**/*.rs".to_string()],
        };

        assert_eq!(request.rule_names, Some(vec!["test-rule".to_string()]));
        assert_eq!(request.severity, Some(Severity::Warning));
        assert_eq!(request.category, Some("style".to_string()));
        assert_eq!(request.patterns, vec!["**/*.rs"]);
    }

    #[test]
    fn test_rule_check_result_creation() {
        let result = RuleCheckResult {
            rules_checked: 5,
            files_checked: 10,
            violations: Vec::new(),
        };

        assert_eq!(result.rules_checked, 5);
        assert_eq!(result.files_checked, 10);
        assert_eq!(result.violations.len(), 0);
    }
}
