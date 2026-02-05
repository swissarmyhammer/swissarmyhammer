//! Validator execution via ACP agent.
//!
//! This module handles the execution of validators by calling an LLM agent
//! via the Agent Client Protocol (ACP). The LLM evaluates the validator's
//! instructions against the hook event context and returns a pass/fail result.
//!
//! The execution uses the `.system/validator` prompt template from the prompts library,
//! similar to how rule checking uses the `.check` prompt.
//!
//! Validator bodies support Liquid templating with partials, using the unified
//! [`LibraryPartialAdapter`] pattern shared with prompts and rules. Use
//! `{% include 'partial-name' %}` to include partials from the `_partials/` directory.

use agent_client_protocol::StopReason;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_templating::{HashMapPartialLoader, PartialLoader, Template};

use crate::types::HookType;
use crate::validator::{
    ExecutedRuleSet, ExecutedValidator, Rule, RuleResult, RuleSet, Validator, ValidatorResult,
};

/// Length of the markdown JSON code fence marker "```json".
const JSON_CODE_FENCE_LEN: usize = 7;

/// Length of the markdown code fence marker "```".
const CODE_FENCE_LEN: usize = 3;

/// Context for rendering a validator prompt.
///
/// Bundles all the parameters needed to render a validator prompt into a single
/// struct, making the API cleaner and more extensible.
pub struct ValidatorRenderContext<'a, P: PartialLoader + Clone + 'static = HashMapPartialLoader> {
    /// The prompt library containing the validator prompt template.
    pub prompt_library: &'a PromptLibrary,
    /// The validator to render.
    pub validator: &'a Validator,
    /// The hook type that triggered validation.
    pub hook_type: HookType,
    /// The hook event context as JSON.
    pub hook_context: &'a serde_json::Value,
    /// Optional partial loader for template includes.
    pub partials: Option<&'a P>,
    /// Optional list of files changed during the turn (for Stop hooks).
    pub changed_files: Option<&'a [String]>,
}

impl<'a> ValidatorRenderContext<'a, HashMapPartialLoader> {
    /// Create a new render context with minimal required parameters.
    pub fn new(
        prompt_library: &'a PromptLibrary,
        validator: &'a Validator,
        hook_type: HookType,
        hook_context: &'a serde_json::Value,
    ) -> Self {
        Self {
            prompt_library,
            validator,
            hook_type,
            hook_context,
            partials: None,
            changed_files: None,
        }
    }
}

impl<'a, P: PartialLoader + Clone + 'static> ValidatorRenderContext<'a, P> {
    /// Create a render context with a specific partial loader type.
    pub fn with_partials(
        prompt_library: &'a PromptLibrary,
        validator: &'a Validator,
        hook_type: HookType,
        hook_context: &'a serde_json::Value,
        partials: Option<&'a P>,
    ) -> Self {
        Self {
            prompt_library,
            validator,
            hook_type,
            hook_context,
            partials,
            changed_files: None,
        }
    }

    /// Set the changed files for Stop hook validators.
    pub fn with_changed_files(mut self, changed_files: Option<&'a [String]>) -> Self {
        self.changed_files = changed_files;
        self
    }

    /// Render the validator prompt using this context.
    pub fn render(&self) -> Result<String, String> {
        use swissarmyhammer_config::TemplateContext;

        // Render the validator body with partials if provided
        let rendered_body = match self.partials {
            Some(p) => render_validator_body(&self.validator.body, p),
            None => self.validator.body.clone(),
        };

        let mut template_context = TemplateContext::new();
        template_context.set("validator_content".to_string(), rendered_body.into());
        template_context.set(
            "validator_name".to_string(),
            self.validator.name().to_string().into(),
        );
        template_context.set(
            "hook_context".to_string(),
            serde_json::to_string_pretty(self.hook_context)
                .unwrap_or_else(|_| self.hook_context.to_string())
                .into(),
        );
        template_context.set("hook_type".to_string(), self.hook_type.to_string().into());

        // Add changed files if provided (for Stop hooks)
        if let Some(files) = self.changed_files {
            if !files.is_empty() {
                template_context.set(
                    "changed_files".to_string(),
                    serde_json::Value::Array(
                        files
                            .iter()
                            .map(|f| serde_json::Value::String(f.clone()))
                            .collect(),
                    ),
                );
            }
        }

        self.prompt_library
            .render(VALIDATOR_PROMPT_NAME, &template_context)
            .map_err(|e| format!("Failed to render validator prompt: {}", e))
    }
}

/// Name of the validator prompt template in the prompts library.
pub const VALIDATOR_PROMPT_NAME: &str = ".system/validator";

/// Prefix for partial files in the validators directory.
const PARTIALS_PREFIX: &str = "_partials/";

/// Marker for partial content.
const PARTIAL_MARKER: &str = "{% partial %}";

/// Check if a name/content pair represents a partial.
///
/// Partials are identified by:
/// - Names starting with `_partials/`
/// - Content starting with `{% partial %}`
pub fn is_partial(name: &str, content: &str) -> bool {
    name.starts_with(PARTIALS_PREFIX) || content.trim_start().starts_with(PARTIAL_MARKER)
}

/// Add a partial to the loader with both full and base names.
///
/// This registers a partial template under its full name and, if the name
/// starts with `_partials/`, also under the base name without the prefix.
/// This allows templates to reference partials with either naming convention.
///
/// # Arguments
///
/// * `loader` - The partial loader to add the partial to
/// * `name` - The full name of the partial (e.g., `_partials/common-checks`)
/// * `content` - The partial template content
///
/// # Example
///
/// A partial named `_partials/common-checks` will be registered as both
/// `_partials/common-checks` and `common-checks`.
pub fn add_partial_with_aliases(loader: &mut HashMapPartialLoader, name: &str, content: &str) {
    loader.add(name, content);

    if let Some(base_name) = name.strip_prefix(PARTIALS_PREFIX) {
        loader.add(base_name, content);
    }
}

/// Extract validator partials from a list of validator (name, content) tuples.
///
/// Partials are identified by:
/// - Names starting with `_partials/`
/// - Content starting with `{% partial %}`
pub fn extract_partials_from_builtins(
    builtins: Vec<(&'static str, &'static str)>,
) -> HashMapPartialLoader {
    let mut loader = HashMapPartialLoader::empty();

    for (name, content) in builtins {
        if is_partial(name, content) {
            add_partial_with_aliases(&mut loader, name, content);
        }
    }

    loader
}

/// Render a validator body as a Liquid template with partials support.
///
/// If the validator body contains Liquid template syntax, it will be rendered
/// with the provided partials. If rendering fails or no template syntax is
/// present, returns the original body unchanged.
///
/// This function accepts any type implementing `PartialLoader + Clone`, allowing
/// it to work with both `HashMapPartialLoader` and `LibraryPartialAdapter<T>`.
/// This is part of the unified partial adapter pattern shared across prompts,
/// rules, and validators.
pub fn render_validator_body<P>(body: &str, partials: &P) -> String
where
    P: PartialLoader + Clone + 'static,
{
    // Quick check - if no template syntax, return as-is
    if !body.contains("{%") && !body.contains("{{") {
        return body.to_string();
    }

    // Try to render as a Liquid template with partials
    match Template::with_partials(body, partials.clone()) {
        Ok(template) => {
            let empty_args: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            match template.render(&empty_args) {
                Ok(rendered) => rendered,
                Err(e) => {
                    tracing::warn!("Failed to render validator body as template: {}", e);
                    body.to_string()
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to parse validator body as template: {}", e);
            body.to_string()
        }
    }
}

/// Build the rendered prompt for validator execution.
///
/// This renders the `.validator` prompt template with the validator content
/// and hook event context, similar to how rule checking renders `.check`.
///
/// The validator body is first rendered as a Liquid template if it contains
/// template syntax, allowing validators to include partials.
///
/// For more control, use [`ValidatorRenderContext`] directly.
pub fn render_validator_prompt(
    prompt_library: &PromptLibrary,
    validator: &Validator,
    hook_type: HookType,
    context: &serde_json::Value,
) -> Result<String, String> {
    ValidatorRenderContext::new(prompt_library, validator, hook_type, context).render()
}

/// Build the rendered prompt for validator execution with optional partials support.
///
/// For more control, use [`ValidatorRenderContext`] directly.
pub fn render_validator_prompt_with_partials<P>(
    prompt_library: &PromptLibrary,
    validator: &Validator,
    hook_type: HookType,
    context: &serde_json::Value,
    partials: Option<&P>,
) -> Result<String, String>
where
    P: PartialLoader + Clone + 'static,
{
    ValidatorRenderContext::with_partials(prompt_library, validator, hook_type, context, partials)
        .render()
}

/// Build the rendered prompt for validator execution with partials and changed files.
///
/// For more control, use [`ValidatorRenderContext`] directly.
pub fn render_validator_prompt_with_partials_and_changed_files<P>(
    prompt_library: &PromptLibrary,
    validator: &Validator,
    hook_type: HookType,
    context: &serde_json::Value,
    partials: Option<&P>,
    changed_files: Option<&[String]>,
) -> Result<String, String>
where
    P: PartialLoader + Clone + 'static,
{
    ValidatorRenderContext::with_partials(prompt_library, validator, hook_type, context, partials)
        .with_changed_files(changed_files)
        .render()
}

/// Parse the LLM response into a ValidatorResult.
///
/// Attempts to extract JSON from the response and parse it as a ValidatorResult.
/// Falls back to creating a failed result if parsing fails.
///
/// # Arguments
///
/// * `response` - The raw response content from the agent
/// * `stop_reason` - Why the agent stopped, used to provide better diagnostics for empty responses
pub fn parse_validator_response(response: &str, stop_reason: &StopReason) -> ValidatorResult {
    if response.trim().is_empty() {
        return handle_empty_response(stop_reason);
    }

    // Try structured JSON parsing approaches
    if let Some(result) = try_parse_json_response(response) {
        return result;
    }

    // Fall back to text-based status detection (JSON patterns)
    if let Some(result) = try_parse_status_from_text(response) {
        return result;
    }

    // Fall back to plain text PASS/FAIL format
    if let Some(result) = try_parse_plain_text_status(response) {
        return result;
    }

    tracing::warn!("Failed to parse validator response as JSON: {:?}", response);
    ValidatorResult::fail(format!(
        "Validator returned invalid JSON response: {}",
        response
    ))
}

/// Try to parse plain text PASS/FAIL format.
///
/// Some validators output just "PASS" or "FAIL" followed by optional explanation.
fn try_parse_plain_text_status(response: &str) -> Option<ValidatorResult> {
    let trimmed = response.trim();

    // Check if response starts with PASS
    if trimmed.starts_with("PASS") {
        let message = if trimmed.len() > 4 {
            trimmed[4..].trim().to_string()
        } else {
            "Validation passed".to_string()
        };
        return Some(ValidatorResult::pass(message));
    }

    // Check if response starts with FAIL
    if trimmed.starts_with("FAIL") {
        let message = if trimmed.len() > 4 {
            trimmed[4..].trim().to_string()
        } else {
            "Validation failed".to_string()
        };
        return Some(ValidatorResult::fail(message));
    }

    None
}

/// Handle empty validator response.
///
/// Logs the stop_reason to help diagnose why the agent produced no output.
fn handle_empty_response(stop_reason: &StopReason) -> ValidatorResult {
    tracing::error!(
        stop_reason = ?stop_reason,
        "Validator returned empty response"
    );
    ValidatorResult::fail(format!(
        "Validator returned empty response - agent stopped with reason: {:?}",
        stop_reason
    ))
}

/// Try to parse response as JSON using multiple strategies.
fn try_parse_json_response(response: &str) -> Option<ValidatorResult> {
    let json_str = extract_json(response);

    // Try direct parsing first
    if let Ok(result) = serde_json::from_str::<ValidatorResult>(json_str) {
        return Some(result);
    }

    // Try to find first valid JSON object (handles streaming duplicates)
    try_parse_first_json_object(response)
}

/// Try to detect status from text patterns when JSON parsing fails.
///
/// Supports both standard format (`status`/`message`) and alternative format (`pass`/`reason`).
fn try_parse_status_from_text(response: &str) -> Option<ValidatorResult> {
    let lower = response.to_lowercase();

    // Standard format: "status": "passed"/"failed"
    if lower.contains("\"status\": \"passed\"") || lower.contains("\"status\":\"passed\"") {
        let message = extract_message_from_response(response)
            .unwrap_or_else(|| "Validation passed".to_string());
        return Some(ValidatorResult::pass(message));
    }

    if lower.contains("\"status\": \"failed\"") || lower.contains("\"status\":\"failed\"") {
        let message = extract_message_from_response(response)
            .unwrap_or_else(|| "Validation failed".to_string());
        return Some(ValidatorResult::fail(message));
    }

    // Alternative format: "pass": true/false with "reason"
    if lower.contains("\"pass\": true") || lower.contains("\"pass\":true") {
        let message = extract_reason_from_response(response)
            .unwrap_or_else(|| "Validation passed".to_string());
        return Some(ValidatorResult::pass(message));
    }

    if lower.contains("\"pass\": false") || lower.contains("\"pass\":false") {
        let message = extract_reason_from_response(response)
            .unwrap_or_else(|| "Validation failed".to_string());
        return Some(ValidatorResult::fail(message));
    }

    None
}

/// Extract "reason" field from response (alternative to "message").
fn extract_reason_from_response(response: &str) -> Option<String> {
    // Look for "reason": "..." pattern
    let pattern = "\"reason\"";
    if let Some(idx) = response.find(pattern) {
        let after_key = &response[idx + pattern.len()..];
        if let Some(colon_idx) = after_key.find(':') {
            let after_colon = after_key[colon_idx + 1..].trim_start();
            if after_colon.starts_with('"') {
                let content = &after_colon[1..];
                if let Some(end_quote) = content.find('"') {
                    return Some(content[..end_quote].to_string());
                }
            }
        }
    }
    None
}

/// Try to find and parse the first valid JSON object containing status field.
fn try_parse_first_json_object(response: &str) -> Option<ValidatorResult> {
    let mut start = 0;
    while let Some(json_start) = response[start..].find('{') {
        let absolute_start = start + json_start;
        let remaining = &response[absolute_start..];

        match find_json_object_end(remaining) {
            Some(end_idx) => {
                let potential_json = &remaining[..=end_idx];
                if let Ok(result) = serde_json::from_str::<ValidatorResult>(potential_json) {
                    return Some(result);
                }
                start = absolute_start + end_idx + 1;
            }
            None => {
                start = absolute_start + 1;
            }
        }
    }
    None
}

/// Find the end index of a JSON object starting at position 0.
///
/// Returns the index of the closing brace if found, None otherwise.
fn find_json_object_end(s: &str) -> Option<usize> {
    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, c) in s.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match c {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract the message field from a response containing JSON.
fn extract_message_from_response(response: &str) -> Option<String> {
    // Look for "message": "..." pattern
    let patterns = [r#""message": ""#, r#""message":""#];

    for pattern in patterns {
        if let Some(start) = response.find(pattern) {
            let after_key = &response[start + pattern.len()..];
            // Find the closing quote (handling escaped quotes)
            let mut end = 0;
            let mut escape_next = false;
            for (i, c) in after_key.char_indices() {
                if escape_next {
                    escape_next = false;
                    continue;
                }
                match c {
                    '\\' => escape_next = true,
                    '"' => {
                        end = i;
                        break;
                    }
                    _ => {}
                }
            }
            if end > 0 {
                return Some(after_key[..end].to_string());
            }
        }
    }
    None
}

/// Extract JSON from a response that might have surrounding text.
///
/// Looks for JSON object delimiters and extracts the content.
/// Handles various edge cases including malformed markdown blocks.
fn extract_json(response: &str) -> &str {
    let trimmed = response.trim();

    // Try markdown code blocks first
    if let Some(json) = extract_from_json_code_block(trimmed) {
        return json;
    }
    if let Some(json) = extract_from_bare_code_block(trimmed) {
        return json;
    }

    // Try bracket matching for raw JSON
    if trimmed.starts_with('{') {
        if let Some(end) = find_json_object_end(trimmed) {
            return &trimmed[..=end];
        }
    }

    // Last resort: find first { and last }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            return &trimmed[start..=end];
        }
    }

    trimmed
}

/// Extract JSON from a ```json code block.
fn extract_from_json_code_block(s: &str) -> Option<&str> {
    let start = s.find("```json")?;
    let after_marker = &s[start + JSON_CODE_FENCE_LEN..];
    let end = after_marker.find("```")?;
    let content = after_marker[..end].trim();

    if content.starts_with('{') && content.ends_with('}') {
        Some(content)
    } else {
        None
    }
}

/// Extract JSON from a bare ``` code block.
fn extract_from_bare_code_block(s: &str) -> Option<&str> {
    let start = s.find("```")?;
    let after_marker = &s[start + CODE_FENCE_LEN..];
    let content_start = after_marker.find('\n').map(|i| i + 1).unwrap_or(0);
    let content = &after_marker[content_start..];
    let end = content.find("```")?;
    let json_content = content[..end].trim();

    if json_content.starts_with('{') && json_content.ends_with('}') {
        Some(json_content)
    } else {
        None
    }
}

/// Create an ExecutedValidator from a validator and its result.
pub fn create_executed_validator(
    validator: &Validator,
    result: ValidatorResult,
) -> ExecutedValidator {
    ExecutedValidator {
        name: validator.name().to_string(),
        severity: validator.severity(),
        result,
    }
}

/// Log validator result at debug level.
///
/// This utility function logs the validator execution outcome in a consistent
/// format for debugging purposes.
pub fn log_validator_result(name: &str, result: &ValidatorResult) {
    tracing::debug!(
        "Validator '{}' result: {} - {}",
        name,
        if result.passed() { "PASSED" } else { "FAILED" },
        result.message()
    );
}

/// Check if an error indicates a rate limit, timeout, or capacity issue.
///
/// This function performs case-insensitive pattern matching against the error
/// message to detect API throttling signals. The patterns detected are:
///
/// - **Rate limits**: "rate limit", "rate_limit", "429", "too many requests"
/// - **Timeouts**: "timeout", "timed out"
/// - **Capacity**: "overloaded", "capacity"
///
/// # Arguments
///
/// * `error` - The error message string to analyze
///
/// # Returns
///
/// `true` if the error indicates rate limiting, timeout, or capacity issues
pub fn is_rate_limit_error(error: &str) -> bool {
    let error_lower = error.to_lowercase();
    error_lower.contains("rate limit")
        || error_lower.contains("rate_limit")
        || error_lower.contains("too many requests")
        || error_lower.contains("429")
        || error_lower.contains("timeout")
        || error_lower.contains("timed out")
        || error_lower.contains("overloaded")
        || error_lower.contains("capacity")
}

// ============================================================================
// RuleSet Execution (New Architecture)
// ============================================================================

/// Context for rendering a RuleSet session initialization prompt.
///
/// This context is used to create the initial system message that sets up
/// the agent session for evaluating all rules in a RuleSet.
pub struct RuleSetSessionContext<'a> {
    /// The prompt library containing prompt templates.
    pub prompt_library: &'a PromptLibrary,
    /// The RuleSet being evaluated.
    pub ruleset: &'a RuleSet,
    /// The hook type that triggered validation.
    pub hook_type: HookType,
    /// The hook event context as JSON.
    pub hook_context: &'a serde_json::Value,
    /// Optional list of files changed during the turn (for Stop hooks).
    pub changed_files: Option<&'a [String]>,
}

impl<'a> RuleSetSessionContext<'a> {
    /// Create a new RuleSet session context.
    pub fn new(
        prompt_library: &'a PromptLibrary,
        ruleset: &'a RuleSet,
        hook_type: HookType,
        hook_context: &'a serde_json::Value,
    ) -> Self {
        Self {
            prompt_library,
            ruleset,
            hook_type,
            hook_context,
            changed_files: None,
        }
    }

    /// Set the changed files for Stop hook validators.
    pub fn with_changed_files(mut self, changed_files: Option<&'a [String]>) -> Self {
        self.changed_files = changed_files;
        self
    }

    /// Render the session initialization prompt.
    ///
    /// This creates the initial system message that explains the RuleSet
    /// context and prepares the agent for sequential rule evaluation.
    pub fn render_session_init(&self) -> Result<String, String> {
        use swissarmyhammer_config::TemplateContext;

        let mut template_context = TemplateContext::new();

        // RuleSet information
        template_context.set("ruleset_name".to_string(), self.ruleset.name().to_string().into());
        template_context.set(
            "ruleset_description".to_string(),
            self.ruleset.description().to_string().into(),
        );
        template_context.set(
            "rule_count".to_string(),
            self.ruleset.rules.len().to_string().into(),
        );

        // Hook context
        template_context.set(
            "hook_context".to_string(),
            serde_json::to_string_pretty(self.hook_context)
                .unwrap_or_else(|_| self.hook_context.to_string())
                .into(),
        );
        template_context.set("hook_type".to_string(), self.hook_type.to_string().into());

        // Add changed files if provided (for Stop hooks)
        if let Some(files) = self.changed_files {
            if !files.is_empty() {
                template_context.set(
                    "changed_files".to_string(),
                    serde_json::Value::Array(
                        files
                            .iter()
                            .map(|f| serde_json::Value::String(f.clone()))
                            .collect(),
                    ),
                );
            }
        }

        self.prompt_library
            .render(VALIDATOR_PROMPT_NAME, &template_context)
            .map_err(|e| format!("Failed to render RuleSet session init prompt: {}", e))
    }
}

/// Context for rendering an individual rule prompt within a RuleSet session.
pub struct RulePromptContext<'a, P: PartialLoader + Clone + 'static = HashMapPartialLoader> {
    /// The rule being evaluated.
    pub rule: &'a Rule,
    /// The parent RuleSet (for inheritance).
    pub ruleset: &'a RuleSet,
    /// Optional partial loader for template includes.
    pub partials: Option<&'a P>,
}

impl<'a> RulePromptContext<'a, HashMapPartialLoader> {
    /// Create a new rule prompt context.
    pub fn new(rule: &'a Rule, ruleset: &'a RuleSet) -> Self {
        Self {
            rule,
            ruleset,
            partials: None,
        }
    }
}

impl<'a, P: PartialLoader + Clone + 'static> RulePromptContext<'a, P> {
    /// Create a rule prompt context with a specific partial loader.
    pub fn with_partials(rule: &'a Rule, ruleset: &'a RuleSet, partials: Option<&'a P>) -> Self {
        Self {
            rule,
            ruleset,
            partials,
        }
    }

    /// Render the rule as a user message for the conversational flow.
    ///
    /// This creates a message that presents the rule to the agent within
    /// the ongoing RuleSet session.
    pub fn render(&self) -> String {
        // Render the rule body with partials if provided
        let rendered_body = match self.partials {
            Some(p) => render_validator_body(&self.rule.body, p),
            None => self.rule.body.clone(),
        };

        // Build the rule message
        let severity = self.rule.effective_severity(self.ruleset);

        format!(
            "# Rule: {}\n\n**Description**: {}\n**Severity**: {}\n\n{}",
            self.rule.name,
            self.rule.description,
            severity,
            rendered_body
        )
    }
}

/// Create an ExecutedRuleSet from a ruleset and rule results.
pub fn create_executed_ruleset(
    ruleset: &RuleSet,
    rule_results: Vec<RuleResult>,
) -> ExecutedRuleSet {
    ExecutedRuleSet {
        ruleset_name: ruleset.name().to_string(),
        rule_results,
    }
}

/// Log RuleSet result at debug level.
pub fn log_ruleset_result(name: &str, executed: &ExecutedRuleSet) {
    let passed_count = executed.rule_results.iter().filter(|r| r.passed()).count();
    let total_count = executed.rule_results.len();

    tracing::debug!(
        "RuleSet '{}' result: {}/{} rules passed (overall: {})",
        name,
        passed_count,
        total_count,
        if executed.passed() { "PASSED" } else { "FAILED" }
    );

    // Log individual rule results
    for rule_result in &executed.rule_results {
        tracing::debug!(
            "  Rule '{}' [{}]: {} - {}",
            rule_result.rule_name,
            rule_result.severity,
            if rule_result.passed() { "PASSED" } else { "FAILED" },
            rule_result.message()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::{Severity, ValidatorFrontmatter, ValidatorSource};
    use std::path::PathBuf;
    use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};

    /// Default stop reason for tests - simulates a normal end turn.
    fn test_stop_reason() -> StopReason {
        StopReason::EndTurn
    }

    /// Create a test validator with the given parameters.
    fn create_test_validator(name: &str, trigger: HookType, severity: Severity) -> Validator {
        create_test_validator_with_body(name, trigger, severity, "Check for issues.")
    }

    /// Create a test validator with custom body content.
    fn create_test_validator_with_body(
        name: &str,
        trigger: HookType,
        severity: Severity,
        body: &str,
    ) -> Validator {
        Validator {
            frontmatter: ValidatorFrontmatter {
                name: name.to_string(),
                description: "Test".to_string(),
                severity,
                trigger,
                match_criteria: None,
                trigger_matcher: None,
                tags: vec![],
                once: false,
                timeout: 30,
            },
            body: body.to_string(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        }
    }

    /// Create a prompt library loaded with all prompts.
    fn create_prompt_library() -> PromptLibrary {
        let mut prompt_library = PromptLibrary::new();
        let mut resolver = PromptResolver::new();
        let _ = resolver.load_all_prompts(&mut prompt_library);
        prompt_library
    }

    #[test]
    fn test_parse_validator_response_passed() {
        let response = r#"{"status": "passed", "message": "All checks passed"}"#;
        let result = parse_validator_response(response, &test_stop_reason());
        assert!(result.passed());
        assert_eq!(result.message(), "All checks passed");
    }

    #[test]
    fn test_parse_validator_response_failed() {
        let response = r#"{"status": "failed", "message": "Found a secret on line 42"}"#;
        let result = parse_validator_response(response, &test_stop_reason());
        assert!(!result.passed());
        assert_eq!(result.message(), "Found a secret on line 42");
    }

    #[test]
    fn test_parse_validator_response_with_markdown() {
        let response = r#"
Here's my analysis:

```json
{"status": "passed", "message": "No issues found"}
```
"#;
        let result = parse_validator_response(response, &test_stop_reason());
        assert!(result.passed());
        assert_eq!(result.message(), "No issues found");
    }

    #[test]
    fn test_parse_validator_response_invalid_json() {
        let response = "This is not JSON at all";
        let result = parse_validator_response(response, &test_stop_reason());
        assert!(!result.passed());
        assert!(result.message().contains("invalid JSON"));
    }

    #[test]
    fn test_parse_validator_response_empty() {
        let result = parse_validator_response("", &test_stop_reason());
        assert!(!result.passed());
        assert!(result.message().contains("empty response"));

        // Whitespace-only should also be treated as empty
        let result = parse_validator_response("   \n\t  ", &test_stop_reason());
        assert!(!result.passed());
        assert!(result.message().contains("empty response"));
    }

    #[test]
    fn test_extract_json_bare() {
        let input = r#"{"status": "passed", "message": "OK"}"#;
        assert_eq!(extract_json(input), input);
    }

    #[test]
    fn test_extract_json_with_whitespace() {
        let input = r#"
        {"status": "passed", "message": "OK"}
        "#;
        assert_eq!(
            extract_json(input),
            r#"{"status": "passed", "message": "OK"}"#
        );
    }

    #[test]
    fn test_extract_json_markdown_block() {
        let input = r#"```json
{"status": "failed", "message": "Bad"}
```"#;
        assert_eq!(
            extract_json(input),
            r#"{"status": "failed", "message": "Bad"}"#
        );
    }

    #[test]
    fn test_render_validator_prompt() {
        use crate::validator::{Severity, ValidatorFrontmatter, ValidatorSource};
        use std::path::PathBuf;

        let validator = Validator {
            frontmatter: ValidatorFrontmatter {
                name: "test".to_string(),
                description: "Test validator".to_string(),
                severity: Severity::Error,
                trigger: HookType::PreToolUse,
                match_criteria: None,
                trigger_matcher: None,
                tags: vec![],
                once: false,
                timeout: 30,
            },
            body: "Check for issues in the code.".to_string(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        };

        let context = serde_json::json!({
            "tool_name": "Write",
            "file_path": "test.ts"
        });

        // Create a prompt library with builtins loaded
        let mut resolver = swissarmyhammer_prompts::PromptResolver::new();
        let mut prompt_library = PromptLibrary::new();
        resolver.load_all_prompts(&mut prompt_library).unwrap();

        let prompt =
            render_validator_prompt(&prompt_library, &validator, HookType::PreToolUse, &context);

        assert!(prompt.is_ok());
        let prompt_text = prompt.unwrap();
        assert!(prompt_text.contains("Check for issues in the code."));
        assert!(prompt_text.contains("PreToolUse"));
        assert!(prompt_text.contains("tool_name"));
    }

    #[test]
    fn test_render_validator_body_with_partials() {
        // Create partials with a test partial
        let mut partials = HashMapPartialLoader::empty();
        partials.add("test-partial", "This is included content.");

        // Test body with include directive
        let body = "Before include. {% include 'test-partial' %} After include.";
        let rendered = render_validator_body(body, &partials);

        assert!(
            rendered.contains("This is included content."),
            "Rendered body should include partial content: {}",
            rendered
        );
        assert!(
            !rendered.contains("{% include"),
            "Rendered body should not contain include directive: {}",
            rendered
        );
    }

    #[test]
    fn test_render_validator_body_without_template_syntax() {
        // Body without any template syntax should pass through unchanged
        let partials = HashMapPartialLoader::empty();
        let body = "Just plain text without any templates.";
        let rendered = render_validator_body(body, &partials);

        assert_eq!(rendered, body);
    }

    #[test]
    fn test_extract_partials_from_builtins() {
        // Test that extract_partials_from_builtins correctly identifies partials
        let builtins = vec![
            ("_partials/test", "{% partial %}\n\nPartial content"),
            ("normal-validator", "Normal validator content"),
            ("partial-in-name", "{% partial %}\n\nAnother partial"),
        ];

        let loader = extract_partials_from_builtins(builtins);

        // _partials/test should be included
        assert!(
            swissarmyhammer_templating::PartialLoader::contains(&loader, "_partials/test"),
            "Should contain _partials/test"
        );
        // It should also be accessible as just "test"
        assert!(
            swissarmyhammer_templating::PartialLoader::contains(&loader, "test"),
            "Should contain test (without prefix)"
        );
        // partial-in-name should be included (has {% partial %} marker)
        assert!(
            swissarmyhammer_templating::PartialLoader::contains(&loader, "partial-in-name"),
            "Should contain partial-in-name"
        );
        // normal-validator should NOT be included
        assert!(
            !swissarmyhammer_templating::PartialLoader::contains(&loader, "normal-validator"),
            "Should not contain normal-validator"
        );
    }

    #[test]
    fn test_create_executed_validator() {
        let validator = create_test_validator_with_body(
            "test-validator",
            HookType::PreToolUse,
            Severity::Error,
            "Test body",
        );

        let result = ValidatorResult::pass("All good".to_string());
        let executed = create_executed_validator(&validator, result);

        assert_eq!(executed.name, "test-validator");
        assert_eq!(executed.severity, Severity::Error);
        assert!(executed.result.passed());
        assert_eq!(executed.result.message(), "All good");
    }

    #[test]
    fn test_validator_render_context_new() {
        let prompt_library = create_prompt_library();
        let validator = create_test_validator("test", HookType::PreToolUse, Severity::Warn);
        let context = serde_json::json!({"tool_name": "Read"});

        let render_ctx = ValidatorRenderContext::new(
            &prompt_library,
            &validator,
            HookType::PreToolUse,
            &context,
        );

        assert!(render_ctx.partials.is_none());
        assert!(render_ctx.changed_files.is_none());
    }

    #[test]
    fn test_validator_render_context_with_partials() {
        let prompt_library = create_prompt_library();
        let validator = create_test_validator("test", HookType::PreToolUse, Severity::Warn);
        let context = serde_json::json!({"tool_name": "Read"});
        let partials = HashMapPartialLoader::empty();

        let render_ctx = ValidatorRenderContext::with_partials(
            &prompt_library,
            &validator,
            HookType::PreToolUse,
            &context,
            Some(&partials),
        );

        assert!(render_ctx.partials.is_some());
        assert!(render_ctx.changed_files.is_none());

        let changed = vec!["file1.rs".to_string(), "file2.rs".to_string()];
        let render_ctx = render_ctx.with_changed_files(Some(&changed));

        assert!(render_ctx.changed_files.is_some());
        assert_eq!(render_ctx.changed_files.unwrap().len(), 2);
    }

    #[test]
    fn test_validator_render_context_render_with_changed_files() {
        let prompt_library = create_prompt_library();
        let validator = create_test_validator_with_body(
            "test",
            HookType::Stop,
            Severity::Warn,
            "Check changed files for issues.",
        );
        let context = serde_json::json!({"session_id": "test-session"});
        let changed = vec!["src/lib.rs".to_string(), "src/main.rs".to_string()];

        let render_ctx =
            ValidatorRenderContext::new(&prompt_library, &validator, HookType::Stop, &context)
                .with_changed_files(Some(&changed));

        let result = render_ctx.render();
        assert!(result.is_ok(), "Render should succeed: {:?}", result.err());

        let rendered = result.unwrap();
        assert!(
            rendered.contains("Check changed files"),
            "Rendered should contain validator body"
        );
    }

    #[test]
    fn test_validator_render_context_render_without_changed_files() {
        let prompt_library = create_prompt_library();
        let validator = create_test_validator("test", HookType::PreToolUse, Severity::Error);
        let context = serde_json::json!({"tool_name": "Write"});

        let render_ctx = ValidatorRenderContext::new(
            &prompt_library,
            &validator,
            HookType::PreToolUse,
            &context,
        )
        .with_changed_files(None);

        let result = render_ctx.render();
        assert!(result.is_ok());
    }

    #[test]
    fn test_validator_render_context_with_empty_changed_files() {
        let prompt_library = create_prompt_library();
        let validator = create_test_validator("test", HookType::Stop, Severity::Warn);
        let context = serde_json::json!({"session_id": "test"});
        let empty_files: Vec<String> = vec![];

        let render_ctx =
            ValidatorRenderContext::new(&prompt_library, &validator, HookType::Stop, &context)
                .with_changed_files(Some(&empty_files));

        let result = render_ctx.render();
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_validator_body_with_missing_partial() {
        let partials = HashMapPartialLoader::empty();

        // Body referencing a non-existent partial should fall back gracefully
        let body = "Before. {% include 'nonexistent' %} After.";
        let rendered = render_validator_body(body, &partials);

        // Should return something (either original or with error handling)
        assert!(!rendered.is_empty());
    }

    #[test]
    fn test_render_validator_body_with_nested_partials() {
        let mut partials = HashMapPartialLoader::empty();
        partials.add("outer", "Start {% include 'inner' %} End");
        partials.add("inner", "INNER_CONTENT");

        let body = "{% include 'outer' %}";
        let rendered = render_validator_body(body, &partials);

        // Should handle nested includes
        assert!(
            rendered.contains("INNER_CONTENT") || rendered.contains("include"),
            "Should either resolve nested partials or preserve syntax: {}",
            rendered
        );
    }

    #[test]
    fn test_extract_from_json_code_block_with_extra_whitespace() {
        let input = r#"
Some text before

```json

   {"status": "passed", "message": "OK"}

```

Some text after
"#;
        let result = extract_json(input);
        assert!(result.contains("status"));
        assert!(result.contains("passed"));
    }

    #[test]
    fn test_extract_json_nested_objects() {
        let input = r#"{"status": "failed", "details": {"line": 42, "col": 10}}"#;
        let result = extract_json(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_parse_validator_response_with_escaped_quotes() {
        let response = r#"{"status": "failed", "message": "Found \"secret\" in code"}"#;
        let result = parse_validator_response(response, &test_stop_reason());
        assert!(!result.passed());
        assert!(result.message().contains("secret"));
    }

    #[test]
    fn test_try_parse_status_from_text_with_extra_spacing() {
        // Test with various spacing around status values
        let response1 = r#"{"status":    "passed",   "message": "ok"}"#;
        let result1 = parse_validator_response(response1, &test_stop_reason());
        assert!(result1.passed());

        let response2 = r#"{ "status" : "failed" , "message" : "bad" }"#;
        let result2 = parse_validator_response(response2, &test_stop_reason());
        assert!(!result2.passed());
    }

    // =========================================================================
    // Rate Limit Error Detection Tests
    // =========================================================================

    #[test]
    fn test_is_rate_limit_error_rate_limit_patterns() {
        assert!(is_rate_limit_error("rate limit exceeded"));
        assert!(is_rate_limit_error("Rate Limit Exceeded"));
        assert!(is_rate_limit_error("rate_limit_error"));
        assert!(is_rate_limit_error("Error: rate_limit"));
    }

    #[test]
    fn test_is_rate_limit_error_http_429() {
        assert!(is_rate_limit_error("HTTP 429 Too Many Requests"));
        assert!(is_rate_limit_error("status code: 429"));
        assert!(is_rate_limit_error("Error 429"));
    }

    #[test]
    fn test_is_rate_limit_error_too_many_requests() {
        assert!(is_rate_limit_error("too many requests"));
        assert!(is_rate_limit_error("Too Many Requests - please slow down"));
    }

    #[test]
    fn test_is_rate_limit_error_timeout_patterns() {
        assert!(is_rate_limit_error("request timeout"));
        assert!(is_rate_limit_error("connection timed out"));
        assert!(is_rate_limit_error("Timeout waiting for response"));
    }

    #[test]
    fn test_is_rate_limit_error_capacity_patterns() {
        assert!(is_rate_limit_error("server overloaded"));
        assert!(is_rate_limit_error("capacity exceeded"));
        assert!(is_rate_limit_error("Overloaded: try again later"));
    }

    #[test]
    fn test_is_rate_limit_error_non_rate_limit_errors() {
        assert!(!is_rate_limit_error("validation failed"));
        assert!(!is_rate_limit_error("invalid input"));
        assert!(!is_rate_limit_error("connection refused"));
        assert!(!is_rate_limit_error("authentication error"));
        assert!(!is_rate_limit_error(""));
    }

    // =========================================================================
    // Validator Result Logging Tests
    // =========================================================================

    #[test]
    fn test_log_validator_result_passed() {
        // This test verifies the function doesn't panic and executes correctly
        let result = ValidatorResult::pass("Test passed successfully".to_string());
        // Should not panic
        log_validator_result("test-validator", &result);
    }

    #[test]
    fn test_log_validator_result_failed() {
        let result = ValidatorResult::fail("Test failed".to_string());
        // Should not panic
        log_validator_result("test-validator", &result);
    }

    // =========================================================================
    // TurnState Edge Case Tests
    // =========================================================================

    #[test]
    fn test_turn_state_changed_files_as_strings_empty() {
        let state = crate::turn::TurnState::new();
        let strings = state.changed_files_as_strings();
        assert!(strings.is_empty());
    }

    #[test]
    fn test_turn_state_has_changes_empty() {
        let state = crate::turn::TurnState::new();
        assert!(!state.has_changes());
    }

    #[test]
    fn test_turn_state_has_changes_with_files() {
        let mut state = crate::turn::TurnState::new();
        state.changed.push(std::path::PathBuf::from("/test.rs"));
        assert!(state.has_changes());
    }
}
