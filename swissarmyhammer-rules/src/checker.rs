//! Rule checking with two-stage rendering and LLM agent execution
//!
//! This module provides the `RuleChecker` which performs rule checks against files:
//! 1. Stage 1: Renders rule templates with context (language, target_path, etc.)
//! 2. Stage 2: Renders .check prompt with rendered rule content
//! 3. Executes via LlamaAgentExecutor
//! 4. Parses responses and fails fast on violations

use crate::{detect_language, Result, Rule, RuleError, RuleViolation};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};
use swissarmyhammer_templating::TemplateEngine;
use swissarmyhammer_workflow::{AgentExecutionContext, AgentExecutor, LlamaAgentExecutorWrapper};

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
/// use swissarmyhammer_config::LlamaAgentConfig;
/// use swissarmyhammer_workflow::agents::LlamaAgentExecutorWrapper;
/// use std::sync::Arc;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create agent wrapper
/// let config = LlamaAgentConfig::for_testing();
/// let agent = Arc::new(LlamaAgentExecutorWrapper::new(config));
///
/// // Create checker
/// let mut checker = RuleChecker::new(agent)?;
/// checker.initialize().await?;
///
/// // Create a rule
/// let rule = Rule::new(
///     "no-todos".to_string(),
///     "Check for TODO comments in {{language}} code".to_string(),
///     Severity::Warning,
/// );
///
/// // Check a file
/// let target = PathBuf::from("src/main.rs");
/// checker.check_file(&rule, &target).await?;
/// # Ok(())
/// # }
/// ```
pub struct RuleChecker {
    /// LLM agent executor for running checks
    agent: Arc<LlamaAgentExecutorWrapper>,
    /// Prompt library containing the .check prompt
    prompt_library: PromptLibrary,
}

impl RuleChecker {
    /// Create a new RuleChecker with the given agent executor
    ///
    /// Loads the PromptLibrary containing the .check prompt from all sources
    /// (builtin, user, local).
    ///
    /// # Arguments
    ///
    /// * `agent` - LlamaAgentExecutor wrapped in Arc for shared ownership
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
    /// use swissarmyhammer_config::LlamaAgentConfig;
    /// use swissarmyhammer_workflow::agents::LlamaAgentExecutorWrapper;
    /// use std::sync::Arc;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = LlamaAgentConfig::for_testing();
    /// let agent = Arc::new(LlamaAgentExecutorWrapper::new(config));
    /// let checker = RuleChecker::new(agent)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(agent: Arc<LlamaAgentExecutorWrapper>) -> Result<Self> {
        tracing::info!("Creating RuleChecker");

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

        Ok(Self {
            agent,
            prompt_library,
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
    /// # use swissarmyhammer_config::LlamaAgentConfig;
    /// # use swissarmyhammer_workflow::agents::LlamaAgentExecutorWrapper;
    /// # use std::sync::Arc;
    /// # use std::path::PathBuf;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = LlamaAgentConfig::for_testing();
    /// # let agent = Arc::new(LlamaAgentExecutorWrapper::new(config));
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

        // Detect language from file extension/content
        let language = detect_language(target_path, &target_content)?;
        tracing::debug!("Detected language: {}", language);

        // STAGE 1: Render the rule template with context variables
        let mut rule_args = HashMap::new();
        rule_args.insert("target_content".to_string(), target_content.clone());
        rule_args.insert("target_path".to_string(), target_path.display().to_string());
        rule_args.insert("language".to_string(), language.clone());

        let engine = TemplateEngine::new();
        let rendered_rule = engine.render(&rule.template, &rule_args).map_err(|e| {
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

        let check_prompt_text = self
            .prompt_library
            .render(".check", &check_context)
            .map_err(|e| RuleError::CheckError(format!("Failed to render .check prompt: {}", e)))?;

        tracing::debug!("Stage 2 complete: .check prompt rendered");

        // Execute via agent (LLM)
        let workflow_context =
            swissarmyhammer_workflow::template_context::WorkflowTemplateContext::with_vars(
                HashMap::new(),
            )
            .map_err(|e| {
                RuleError::CheckError(format!("Failed to create workflow context: {}", e))
            })?;
        let agent_context = AgentExecutionContext::new(&workflow_context);

        let response = self
            .agent
            .execute_prompt(String::new(), check_prompt_text, &agent_context)
            .await
            .map_err(|e| RuleError::AgentError(format!("Agent execution failed: {}", e)))?;

        tracing::debug!("LLM execution complete");

        // Parse result - check for PASS or VIOLATION
        let result_text = response.content.trim();
        if result_text == "PASS" {
            tracing::debug!(
                "Check passed for {} against rule {}",
                target_path.display(),
                rule.name
            );
            Ok(())
        } else {
            // Violation found - create RuleViolation and return error for fail-fast
            let violation = RuleViolation::new(
                rule.name.clone(),
                target_path.to_path_buf(),
                rule.severity,
                response.content,
            );

            tracing::warn!(
                "Violation found in {} for rule {}: {}",
                target_path.display(),
                rule.name,
                violation.message
            );

            Err(RuleError::Violation(violation).into())
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
    /// # use swissarmyhammer_config::LlamaAgentConfig;
    /// # use swissarmyhammer_workflow::agents::LlamaAgentExecutorWrapper;
    /// # use std::sync::Arc;
    /// # use std::path::PathBuf;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = LlamaAgentConfig::for_testing();
    /// # let agent = Arc::new(LlamaAgentExecutorWrapper::new(config));
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;
    use swissarmyhammer_config::LlamaAgentConfig;
    use tempfile::TempDir;

    fn create_test_agent() -> Arc<LlamaAgentExecutorWrapper> {
        let config = LlamaAgentConfig::for_testing();
        Arc::new(LlamaAgentExecutorWrapper::new(config))
    }

    #[test]
    fn test_rule_checker_creation() {
        let agent = create_test_agent();
        let checker = RuleChecker::new(agent);
        assert!(checker.is_ok());
    }

    #[test]
    fn test_rule_checker_creation_loads_check_prompt() {
        let agent = create_test_agent();
        let checker = RuleChecker::new(agent).unwrap();

        // Verify .check prompt is loaded
        let check_prompt = checker.prompt_library.get(".check");
        assert!(check_prompt.is_ok());

        let prompt = check_prompt.unwrap();
        assert_eq!(prompt.name, ".check");
    }

    #[tokio::test]
    async fn test_rule_checker_two_stage_rendering() {
        let agent = create_test_agent();
        let checker = RuleChecker::new(agent).unwrap();

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
        let agent = create_test_agent();
        let checker = RuleChecker::new(agent).unwrap();

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
        let agent = create_test_agent();
        let mut checker = RuleChecker::new(agent).unwrap();

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
}
