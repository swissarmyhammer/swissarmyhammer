//! Validator types for the Agent Validator Protocol.
//!
//! Validators are markdown files with YAML frontmatter that specify validation
//! rules to run against hook events.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::types::HookType;

/// Default timeout in seconds for validator execution.
///
/// This value is used when no explicit timeout is specified in the validator
/// frontmatter. 30 seconds provides enough time for LLM-based validators
/// to complete while preventing indefinite hangs.
pub const DEFAULT_VALIDATOR_TIMEOUT_SECONDS: u32 = 30;

/// Severity level for validator findings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational finding, does not affect execution.
    Info,
    /// Warning finding, logged but does not block.
    #[default]
    Warn,
    /// Error finding, blocks the action.
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warn => write!(f, "warn"),
            Severity::Error => write!(f, "error"),
        }
    }
}

/// Match criteria for filtering when a validator should run.
///
/// Validators use this to specify which tool invocations or file operations
/// should trigger validation. Both `tools` and `files` support pattern matching:
/// - `tools`: Regex patterns matched against tool names (case-insensitive)
/// - `files`: Glob patterns matched against file paths (case-insensitive)
///
/// If both are specified, both must match for the validator to run.
/// If neither is specified (empty), the validator matches all events of its trigger type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidatorMatch {
    /// Tool names to match (e.g., ["Write", "Edit"]).
    #[serde(default)]
    pub tools: Vec<String>,

    /// File glob patterns to match (e.g., ["*.ts", "src/**/*.rs"]).
    #[serde(default)]
    pub files: Vec<String>,
}

impl ValidatorMatch {
    /// Check if this match criteria is empty (matches everything).
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty() && self.files.is_empty()
    }
}

/// Context for matching validators against hook events.
///
/// This encapsulates all the information needed to determine if a validator
/// should run for a given hook event.
#[derive(Debug, Clone)]
pub struct MatchContext {
    /// The hook event type.
    pub hook_type: HookType,

    /// The tool name (for tool-related hooks).
    pub tool_name: Option<String>,

    /// The file path being operated on (if applicable).
    pub file_path: Option<String>,

    /// Event context string for triggerMatcher regex matching.
    /// This varies by hook type:
    /// - Notification: notification_type
    /// - SessionStart: source
    /// - SubagentStart/Stop: subagent_type or name
    pub event_context: Option<String>,
}

impl MatchContext {
    /// Create a new match context with just the hook type.
    pub fn new(hook_type: HookType) -> Self {
        Self {
            hook_type,
            tool_name: None,
            file_path: None,
            event_context: None,
        }
    }

    /// Set the tool name.
    pub fn with_tool(mut self, tool_name: impl Into<String>) -> Self {
        self.tool_name = Some(tool_name.into());
        self
    }

    /// Set the file path.
    pub fn with_file(mut self, file_path: impl Into<String>) -> Self {
        self.file_path = Some(file_path.into());
        self
    }

    /// Set the event context for triggerMatcher.
    pub fn with_event_context(mut self, context: impl Into<String>) -> Self {
        self.event_context = Some(context.into());
        self
    }

    /// Create from JSON input, extracting all relevant fields.
    pub fn from_json(hook_type: HookType, input: &serde_json::Value) -> Self {
        let tool_name = input
            .get("tool_name")
            .and_then(|v| v.as_str())
            .map(String::from);

        let file_path = input
            .get("tool_input")
            .and_then(|ti| {
                ti.get("file_path")
                    .or_else(|| ti.get("path"))
                    .or_else(|| ti.get("file"))
            })
            .and_then(|v| v.as_str())
            .map(String::from);

        let event_context = input
            .get("notification_type")
            .or_else(|| input.get("source"))
            .or_else(|| input.get("subagent_type"))
            .or_else(|| input.get("name"))
            .or_else(|| input.get("hook_event_name"))
            .and_then(|v| v.as_str())
            .map(String::from);

        Self {
            hook_type,
            tool_name,
            file_path,
            event_context,
        }
    }
}

/// Default timeout in seconds for validator execution.
fn default_timeout() -> u32 {
    DEFAULT_VALIDATOR_TIMEOUT_SECONDS
}

/// Default trigger hook type.
fn default_trigger() -> HookType {
    HookType::PostToolUse
}

/// YAML frontmatter for a validator file.
///
/// # Sensible Defaults
///
/// When frontmatter fields are omitted, the following defaults are applied:
///
/// - `name`: Defaults to the file stem (e.g., `check-types.md` → `check-types`)
/// - `description`: Defaults to "Validator: {name}"
/// - `trigger`: Defaults to `PostToolUse`
/// - `severity`: Defaults to `warn`
/// - `match.files`: Defaults to source code patterns when `match` is omitted
/// - `timeout`: Defaults to 30 seconds
///
/// This allows creating minimal validators with just a body:
///
/// ```markdown
/// ---
/// ---
///
/// Check that the code follows best practices...
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorFrontmatter {
    /// Unique name for the validator.
    /// Defaults to the file stem if not provided.
    #[serde(default)]
    pub name: String,

    /// Human-readable description.
    /// Defaults to "Validator: {name}" if not provided.
    #[serde(default)]
    pub description: String,

    /// Severity level for findings.
    #[serde(default)]
    pub severity: Severity,

    /// Hook event type that triggers this validator.
    /// Defaults to PostToolUse if not provided.
    #[serde(default = "default_trigger")]
    pub trigger: HookType,

    /// Optional match criteria for filtering which events trigger this validator.
    ///
    /// When present, the validator only runs if the event matches the specified
    /// tools and/or file patterns. When absent, defaults are applied:
    /// - `files`: Source code patterns (*.rs, *.ts, *.py, etc.)
    /// - `tools`: All tools (no filtering)
    #[serde(default, rename = "match")]
    pub match_criteria: Option<ValidatorMatch>,

    /// Optional regex pattern for matching lifecycle events.
    #[serde(default, rename = "triggerMatcher")]
    pub trigger_matcher: Option<String>,

    /// Optional tags for filtering and organization.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Run only once per session (default: false).
    #[serde(default)]
    pub once: bool,

    /// Timeout in seconds (default: 30).
    #[serde(default = "default_timeout")]
    pub timeout: u32,
}

impl ValidatorFrontmatter {
    /// Apply defaults based on the file path and optional source code patterns.
    ///
    /// This fills in missing fields with sensible defaults:
    /// - `name`: File stem (e.g., `check-types.md` → `check-types`)
    /// - `description`: "Validator: {name}"
    /// - `match.files`: Source code patterns from `@file_groups/source_code` (if provided and match is None)
    ///
    /// The `source_code_patterns` parameter should be loaded from `builtin/file_groups/source_code.yaml`
    /// via the YAML expander. If None, no default match criteria is applied.
    ///
    /// Note: Stop validators do not get default file patterns because they check all changed
    /// files at session end rather than filtering by specific file patterns.
    pub fn apply_defaults(
        &mut self,
        path: &std::path::Path,
        source_code_patterns: Option<&[String]>,
    ) {
        // Default name to file stem
        if self.name.is_empty() {
            self.name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unnamed")
                .to_string();
        }

        // Default description
        if self.description.is_empty() {
            self.description = format!("Validator: {}", self.name);
        }

        // Default match criteria to source code files (if patterns provided)
        // Skip for Stop validators - they check all changed files, not specific patterns
        if self.match_criteria.is_none() && self.trigger != HookType::Stop {
            if let Some(patterns) = source_code_patterns {
                self.match_criteria = Some(ValidatorMatch {
                    tools: vec![],
                    files: patterns.to_vec(),
                });
            }
        }
    }
}

/// Source of a validator (builtin, user, or project).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ValidatorSource {
    /// Builtin validators embedded in the binary.
    Builtin,
    /// User validators from ~/<AVP_DIR>/validators.
    User,
    /// Project validators from ./<AVP_DIR>/validators.
    Project,
}

impl std::fmt::Display for ValidatorSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidatorSource::Builtin => write!(f, "builtin"),
            ValidatorSource::User => write!(f, "user"),
            ValidatorSource::Project => write!(f, "project"),
        }
    }
}

/// A loaded validator with its metadata and instructions.
///
/// Validators are loaded from markdown files with YAML frontmatter.
/// The frontmatter contains configuration (trigger type, match criteria, severity)
/// while the body contains instructions for the validation agent.
#[derive(Debug, Clone)]
pub struct Validator {
    /// Parsed YAML frontmatter containing validator configuration.
    ///
    /// This includes the validator's name, description, severity level,
    /// trigger hook type, and optional match criteria for filtering.
    pub frontmatter: ValidatorFrontmatter,

    /// Markdown body containing validation instructions.
    pub body: String,

    /// Where this validator came from.
    pub source: ValidatorSource,

    /// Path to the validator file.
    pub path: PathBuf,
}

impl Validator {
    /// Get the validator name.
    pub fn name(&self) -> &str {
        &self.frontmatter.name
    }

    /// Get the validator description.
    pub fn description(&self) -> &str {
        &self.frontmatter.description
    }

    /// Get the severity level.
    pub fn severity(&self) -> Severity {
        self.frontmatter.severity
    }

    /// Get the trigger hook type.
    pub fn trigger(&self) -> HookType {
        self.frontmatter.trigger
    }

    /// Check if this validator matches the given context.
    ///
    /// A validator matches if:
    /// 1. The hook type matches the trigger
    /// 2. If tools are specified in match criteria, the tool name matches
    /// 3. If files are specified in match criteria, the file path matches a glob
    /// 4. If triggerMatcher is specified, the event context matches the regex
    pub fn matches(&self, ctx: &MatchContext) -> bool {
        // Must match hook type
        if self.frontmatter.trigger != ctx.hook_type {
            return false;
        }

        // Check triggerMatcher regex if present
        if !self.matches_trigger_regex(ctx) {
            return false;
        }

        // Check match criteria if present
        if let Some(match_criteria) = &self.frontmatter.match_criteria {
            if !self.matches_tools(match_criteria, ctx) {
                return false;
            }
            if !self.matches_files(match_criteria, ctx) {
                return false;
            }
        }

        true
    }

    /// Check if the event context matches the triggerMatcher regex.
    fn matches_trigger_regex(&self, ctx: &MatchContext) -> bool {
        let Some(trigger_matcher) = &self.frontmatter.trigger_matcher else {
            return true; // No trigger matcher means match
        };

        let Some(context) = &ctx.event_context else {
            return false; // triggerMatcher requires context
        };

        match regex::RegexBuilder::new(trigger_matcher)
            .case_insensitive(true)
            .build()
        {
            Ok(re) => re.is_match(context),
            Err(e) => {
                tracing::warn!(
                    "Invalid triggerMatcher regex '{}' in validator '{}': {}",
                    trigger_matcher,
                    self.frontmatter.name,
                    e
                );
                false
            }
        }
    }

    /// Check if the tool name matches any of the tool patterns.
    fn matches_tools(&self, match_criteria: &ValidatorMatch, ctx: &MatchContext) -> bool {
        if match_criteria.tools.is_empty() {
            return true;
        }

        let Some(name) = &ctx.tool_name else {
            return false;
        };

        match_criteria.tools.iter().any(|pattern| {
            let anchored = format!("^(?:{})$", pattern);
            regex::RegexBuilder::new(&anchored)
                .case_insensitive(true)
                .build()
                .map(|re| re.is_match(name))
                .unwrap_or_else(|_| pattern.eq_ignore_ascii_case(name))
        })
    }

    /// Check if the file path matches any of the file glob patterns.
    fn matches_files(&self, match_criteria: &ValidatorMatch, ctx: &MatchContext) -> bool {
        // Skip file matching for Stop hooks - they always run regardless of files
        if match_criteria.files.is_empty() || ctx.hook_type == HookType::Stop {
            return true;
        }

        let Some(path) = &ctx.file_path else {
            return false;
        };

        let match_options = glob::MatchOptions {
            case_sensitive: false,
            ..Default::default()
        };

        match_criteria.files.iter().any(|pattern| {
            glob::Pattern::new(pattern)
                .map(|p| p.matches_with(path, match_options))
                .unwrap_or(false)
        })
    }
}

/// Result of running a validator.
///
/// The LLM returns just passed/failed with a message. The validator name
/// and severity are known by the calling code from the validator's frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum ValidatorResult {
    /// Validation passed.
    #[serde(rename = "passed")]
    Passed { message: String },
    /// Validation failed.
    #[serde(rename = "failed")]
    Failed { message: String },
}

impl ValidatorResult {
    /// Create a passing result.
    pub fn pass(message: impl Into<String>) -> Self {
        Self::Passed {
            message: message.into(),
        }
    }

    /// Create a failing result.
    pub fn fail(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }

    /// Check if the validation passed.
    pub fn passed(&self) -> bool {
        matches!(self, Self::Passed { .. })
    }

    /// Get the message.
    pub fn message(&self) -> &str {
        match self {
            Self::Passed { message } => message,
            Self::Failed { message } => message,
        }
    }
}

/// Result of executing a validator, paired with validator metadata.
///
/// The LLM returns just passed/failed with a message. This struct pairs that
/// result with the validator's name and severity from the frontmatter.
#[derive(Debug, Clone)]
pub struct ExecutedValidator {
    /// Name of the validator that was executed.
    pub name: String,
    /// Severity from the validator's frontmatter.
    pub severity: Severity,
    /// Result returned by the LLM.
    pub result: ValidatorResult,
}

impl ExecutedValidator {
    /// Check if the validation passed.
    pub fn passed(&self) -> bool {
        self.result.passed()
    }

    /// Check if this is a blocking failure (failed + error severity).
    pub fn is_blocking(&self) -> bool {
        !self.result.passed() && self.severity == Severity::Error
    }

    /// Get the message from the result.
    pub fn message(&self) -> &str {
        self.result.message()
    }
}

// ============================================================================
// RuleSet Types (New Architecture)
// ============================================================================

/// Manifest for a RuleSet, parsed from VALIDATOR.md.
///
/// The manifest defines shared configuration for all rules in the RuleSet:
/// - Common trigger and match criteria
/// - Default severity and timeout (rules can override)
/// - Metadata like name, version, tags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSetManifest {
    /// Unique identifier for this RuleSet.
    pub name: String,

    /// Human-readable description of the RuleSet's purpose.
    pub description: String,

    /// Semantic version (e.g., "1.0.0").
    pub version: String,

    /// Hook event type that triggers all rules in this RuleSet.
    /// Rules inherit this and cannot override.
    #[serde(default = "default_trigger")]
    pub trigger: HookType,

    /// Match criteria for filtering which events trigger this RuleSet.
    /// Rules inherit this and cannot override.
    #[serde(default, rename = "match")]
    pub match_criteria: Option<ValidatorMatch>,

    /// Optional regex pattern for matching lifecycle events.
    /// Rules inherit this and cannot override.
    #[serde(default, rename = "triggerMatcher")]
    pub trigger_matcher: Option<String>,

    /// Tags for categorization and organization.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Default severity for rules (rules can override).
    #[serde(default)]
    pub severity: Severity,

    /// Default timeout in seconds (rules can override).
    #[serde(default = "default_timeout")]
    pub timeout: u32,

    /// Run only once per session (applies to entire RuleSet).
    #[serde(default)]
    pub once: bool,
}

impl RuleSetManifest {
    /// Apply defaults based on the directory path.
    ///
    /// This fills in missing fields:
    /// - `name`: Directory name if empty
    /// - `description`: "RuleSet: {name}" if empty
    pub fn apply_defaults(&mut self, dir_path: &std::path::Path) {
        // Default name to directory name
        if self.name.is_empty() {
            self.name = dir_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unnamed")
                .to_string();
        }

        // Default description
        if self.description.is_empty() {
            self.description = format!("RuleSet: {}", self.name);
        }

        // Default version
        if self.version.is_empty() {
            self.version = "1.0.0".to_string();
        }
    }
}

/// Individual rule within a RuleSet.
///
/// Rules contain the actual validation logic and can override certain
/// RuleSet defaults (severity, timeout) while inheriting trigger and match criteria.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Unique identifier for this rule within the RuleSet.
    pub name: String,

    /// Human-readable description of what this rule validates.
    pub description: String,

    /// Markdown body containing validation instructions.
    pub body: String,

    /// Override severity (if None, inherits from RuleSet).
    pub severity: Option<Severity>,

    /// Override timeout (if None, inherits from RuleSet).
    pub timeout: Option<u32>,
}

impl Rule {
    /// Get the effective severity for this rule.
    ///
    /// Returns the rule's override if present, otherwise the RuleSet default.
    pub fn effective_severity(&self, ruleset: &RuleSet) -> Severity {
        self.severity.unwrap_or(ruleset.manifest.severity)
    }

    /// Get the effective timeout for this rule.
    ///
    /// Returns the rule's override if present, otherwise the RuleSet default.
    pub fn effective_timeout(&self, ruleset: &RuleSet) -> u32 {
        self.timeout.unwrap_or(ruleset.manifest.timeout)
    }
}

/// Frontmatter for individual rule files.
///
/// This is used during parsing to extract rule-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFrontmatter {
    /// Rule identifier within the RuleSet.
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// Optional severity override.
    #[serde(default)]
    pub severity: Option<Severity>,

    /// Optional timeout override.
    #[serde(default)]
    pub timeout: Option<u32>,
}

impl RuleFrontmatter {
    /// Apply defaults based on the file path.
    pub fn apply_defaults(&mut self, path: &std::path::Path) {
        // Default name to file stem
        if self.name.is_empty() {
            self.name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unnamed")
                .to_string();
        }

        // Default description
        if self.description.is_empty() {
            self.description = format!("Rule: {}", self.name);
        }
    }
}

/// A RuleSet package containing a manifest and multiple rules.
///
/// RuleSets are the new organizational structure for validators:
/// - VALIDATOR.md contains the manifest with shared configuration
/// - rules/ directory contains individual rule files
/// - All rules in a RuleSet share the same trigger and match criteria
#[derive(Debug, Clone)]
pub struct RuleSet {
    /// Parsed manifest from VALIDATOR.md.
    pub manifest: RuleSetManifest,

    /// Rules loaded from the rules/ directory.
    pub rules: Vec<Rule>,

    /// Source of this RuleSet (builtin, user, or project).
    pub source: ValidatorSource,

    /// Base path to the RuleSet directory.
    pub base_path: PathBuf,
}

impl RuleSet {
    /// Get the RuleSet name.
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    /// Get the RuleSet description.
    pub fn description(&self) -> &str {
        &self.manifest.description
    }

    /// Get the trigger hook type.
    pub fn trigger(&self) -> HookType {
        self.manifest.trigger
    }

    /// Check if this RuleSet matches the given context.
    ///
    /// A RuleSet matches if:
    /// 1. The hook type matches the trigger
    /// 2. If triggerMatcher is specified, the event context matches the regex
    /// 3. If tools are specified in match criteria, the tool name matches
    /// 4. If files are specified in match criteria, the file path matches a glob
    ///
    /// Note: This checks RuleSet-level matching. Individual rules inherit this
    /// matching behavior.
    pub fn matches(&self, ctx: &MatchContext) -> bool {
        // Must match hook type
        if self.manifest.trigger != ctx.hook_type {
            return false;
        }

        // Check triggerMatcher regex if present
        if !self.matches_trigger_regex(ctx) {
            return false;
        }

        // Check match criteria if present
        if let Some(match_criteria) = &self.manifest.match_criteria {
            if !self.matches_tools(match_criteria, ctx) {
                return false;
            }
            if !self.matches_files(match_criteria, ctx) {
                return false;
            }
        }

        true
    }

    /// Check if the event context matches the triggerMatcher regex.
    fn matches_trigger_regex(&self, ctx: &MatchContext) -> bool {
        let Some(trigger_matcher) = &self.manifest.trigger_matcher else {
            return true;
        };

        let Some(context) = &ctx.event_context else {
            return false;
        };

        match regex::RegexBuilder::new(trigger_matcher)
            .case_insensitive(true)
            .build()
        {
            Ok(re) => re.is_match(context),
            Err(e) => {
                tracing::warn!(
                    "Invalid triggerMatcher regex '{}' in RuleSet '{}': {}",
                    trigger_matcher,
                    self.manifest.name,
                    e
                );
                false
            }
        }
    }

    /// Check if the tool name matches any of the tool patterns.
    fn matches_tools(&self, match_criteria: &ValidatorMatch, ctx: &MatchContext) -> bool {
        if match_criteria.tools.is_empty() {
            return true;
        }

        let Some(name) = &ctx.tool_name else {
            return false;
        };

        match_criteria.tools.iter().any(|pattern| {
            let anchored = format!("^(?:{})$", pattern);
            regex::RegexBuilder::new(&anchored)
                .case_insensitive(true)
                .build()
                .map(|re| re.is_match(name))
                .unwrap_or_else(|_| pattern.eq_ignore_ascii_case(name))
        })
    }

    /// Check if the file path matches any of the file glob patterns.
    fn matches_files(&self, match_criteria: &ValidatorMatch, ctx: &MatchContext) -> bool {
        // Skip file matching for Stop hooks
        if match_criteria.files.is_empty() || ctx.hook_type == HookType::Stop {
            return true;
        }

        let Some(path) = &ctx.file_path else {
            return false;
        };

        let match_options = glob::MatchOptions {
            case_sensitive: false,
            ..Default::default()
        };

        match_criteria.files.iter().any(|pattern| {
            glob::Pattern::new(pattern)
                .map(|p| p.matches_with(path, match_options))
                .unwrap_or(false)
        })
    }
}

/// Result of executing a single rule within a RuleSet session.
#[derive(Debug, Clone)]
pub struct RuleResult {
    /// Name of the rule that was executed.
    pub rule_name: String,
    /// Severity of this rule.
    pub severity: Severity,
    /// Result returned by the agent for this rule.
    pub result: ValidatorResult,
}

impl RuleResult {
    /// Check if the rule validation passed.
    pub fn passed(&self) -> bool {
        self.result.passed()
    }

    /// Check if this is a blocking failure (failed + error severity).
    pub fn is_blocking(&self) -> bool {
        !self.result.passed() && self.severity == Severity::Error
    }

    /// Get the message from the result.
    pub fn message(&self) -> &str {
        self.result.message()
    }
}

/// Result of executing an entire RuleSet in a single agent session.
///
/// A RuleSet execution involves one agent session where all rules are
/// evaluated sequentially as a conversational flow.
#[derive(Debug, Clone)]
pub struct ExecutedRuleSet {
    /// Name of the RuleSet that was executed.
    pub ruleset_name: String,
    /// Results for each rule in the RuleSet.
    pub rule_results: Vec<RuleResult>,
}

impl ExecutedRuleSet {
    /// Check if all rules in the RuleSet passed.
    pub fn passed(&self) -> bool {
        self.rule_results.iter().all(|r| r.passed())
    }

    /// Check if any rule is a blocking failure.
    pub fn has_blocking_failure(&self) -> bool {
        self.rule_results.iter().any(|r| r.is_blocking())
    }

    /// Get all failed rules.
    pub fn failed_rules(&self) -> Vec<&RuleResult> {
        self.rule_results.iter().filter(|r| !r.passed()).collect()
    }

    /// Get all blocking failures.
    pub fn blocking_failures(&self) -> Vec<&RuleResult> {
        self.rule_results
            .iter()
            .filter(|r| r.is_blocking())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_default() {
        assert_eq!(Severity::default(), Severity::Warn);
    }

    #[test]
    fn test_validator_match_is_empty() {
        let empty = ValidatorMatch::default();
        assert!(empty.is_empty());

        let with_tools = ValidatorMatch {
            tools: vec!["Write".to_string()],
            files: vec![],
        };
        assert!(!with_tools.is_empty());
    }

    #[test]
    fn test_validator_matches_hook_type() {
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
            body: String::new(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        };

        assert!(validator.matches(&MatchContext::new(HookType::PreToolUse)));
        assert!(!validator.matches(&MatchContext::new(HookType::PostToolUse)));
    }

    #[test]
    fn test_validator_matches_tool_filter() {
        let validator = Validator {
            frontmatter: ValidatorFrontmatter {
                name: "test".to_string(),
                description: "Test validator".to_string(),
                severity: Severity::Error,
                trigger: HookType::PreToolUse,
                match_criteria: Some(ValidatorMatch {
                    tools: vec!["Write".to_string(), "Edit".to_string()],
                    files: vec![],
                }),
                trigger_matcher: None,
                tags: vec![],
                once: false,
                timeout: 30,
            },
            body: String::new(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        };

        assert!(validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("Write")));
        assert!(validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("Edit")));
        // Case-insensitive matching
        assert!(validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("write")));
        assert!(validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("WRITE")));
        assert!(validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("eDiT")));
        assert!(!validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("Bash")));
        assert!(!validator.matches(&MatchContext::new(HookType::PreToolUse)));
    }

    #[test]
    fn test_validator_matches_tool_regex() {
        let validator = Validator {
            frontmatter: ValidatorFrontmatter {
                name: "test".to_string(),
                description: "Test validator".to_string(),
                severity: Severity::Error,
                trigger: HookType::PreToolUse,
                match_criteria: Some(ValidatorMatch {
                    tools: vec!["Write|Edit".to_string(), "Bash.*".to_string()],
                    files: vec![],
                }),
                trigger_matcher: None,
                tags: vec![],
                once: false,
                timeout: 30,
            },
            body: String::new(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        };

        // Regex alternation
        assert!(validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("Write")));
        assert!(validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("Edit")));
        // Case-insensitive regex
        assert!(validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("WRITE")));
        // Regex pattern with wildcard
        assert!(validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("Bash")));
        assert!(
            validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("BashCommand"))
        );
        assert!(validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("bash")));
        // Non-matching
        assert!(!validator.matches(&MatchContext::new(HookType::PreToolUse).with_tool("Read")));
    }

    #[test]
    fn test_validator_matches_file_filter() {
        let validator = Validator {
            frontmatter: ValidatorFrontmatter {
                name: "test".to_string(),
                description: "Test validator".to_string(),
                severity: Severity::Error,
                trigger: HookType::PostToolUse,
                match_criteria: Some(ValidatorMatch {
                    tools: vec![],
                    files: vec!["*.ts".to_string(), "src/**/*.rs".to_string()],
                }),
                trigger_matcher: None,
                tags: vec![],
                once: false,
                timeout: 30,
            },
            body: String::new(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        };

        assert!(validator.matches(&MatchContext::new(HookType::PostToolUse).with_file("test.ts")));
        assert!(validator
            .matches(&MatchContext::new(HookType::PostToolUse).with_file("src/lib/utils.rs")));
        // Case-insensitive file matching
        assert!(validator.matches(&MatchContext::new(HookType::PostToolUse).with_file("TEST.TS")));
        assert!(validator.matches(&MatchContext::new(HookType::PostToolUse).with_file("Test.Ts")));
        assert!(!validator.matches(&MatchContext::new(HookType::PostToolUse).with_file("test.js")));
        assert!(!validator.matches(&MatchContext::new(HookType::PostToolUse)));
    }

    #[test]
    fn test_validator_result_pass() {
        let result = ValidatorResult::pass("All checks passed");
        assert!(result.passed());
        assert_eq!(result.message(), "All checks passed");
    }

    #[test]
    fn test_validator_result_fail() {
        let result = ValidatorResult::fail("Secret detected: Found API key on line 42");
        assert!(!result.passed());
        assert_eq!(
            result.message(),
            "Secret detected: Found API key on line 42"
        );
    }

    #[test]
    fn test_validator_result_serialization() {
        // Test that the enum serializes correctly with the "status" tag
        let passed = ValidatorResult::pass("OK");
        let json = serde_json::to_string(&passed).unwrap();
        assert!(json.contains(r#""status":"passed""#));
        assert!(json.contains(r#""message":"OK""#));

        let failed = ValidatorResult::fail("Bad");
        let json = serde_json::to_string(&failed).unwrap();
        assert!(json.contains(r#""status":"failed""#));
        assert!(json.contains(r#""message":"Bad""#));
    }

    #[test]
    fn test_validator_matches_trigger_matcher() {
        let validator = Validator {
            frontmatter: ValidatorFrontmatter {
                name: "test".to_string(),
                description: "Test validator".to_string(),
                severity: Severity::Error,
                trigger: HookType::Notification,
                match_criteria: None,
                trigger_matcher: Some("agent_.*_complete".to_string()),
                tags: vec![],
                once: false,
                timeout: 30,
            },
            body: String::new(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        };

        // Should match with matching context
        assert!(validator.matches(
            &MatchContext::new(HookType::Notification).with_event_context("agent_task_complete")
        ));

        // Case-insensitive triggerMatcher matching
        assert!(validator.matches(
            &MatchContext::new(HookType::Notification).with_event_context("AGENT_TASK_COMPLETE")
        ));
        assert!(validator.matches(
            &MatchContext::new(HookType::Notification).with_event_context("Agent_Task_Complete")
        ));

        // Should not match with non-matching context
        assert!(!validator.matches(
            &MatchContext::new(HookType::Notification).with_event_context("something_else")
        ));

        // Should not match without context when triggerMatcher is present
        assert!(!validator.matches(&MatchContext::new(HookType::Notification)));
    }

    #[test]
    fn test_validator_trigger_matcher_invalid_regex() {
        let validator = Validator {
            frontmatter: ValidatorFrontmatter {
                name: "test".to_string(),
                description: "Test validator".to_string(),
                severity: Severity::Error,
                trigger: HookType::Notification,
                match_criteria: None,
                trigger_matcher: Some("[invalid(regex".to_string()), // Invalid regex
                tags: vec![],
                once: false,
                timeout: 30,
            },
            body: String::new(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        };

        // Should not match with invalid regex (fails gracefully)
        assert!(!validator
            .matches(&MatchContext::new(HookType::Notification).with_event_context("any_context")));
    }

    #[test]
    fn test_validator_matches_combined_criteria() {
        // Test validator with both match criteria and triggerMatcher
        let validator = Validator {
            frontmatter: ValidatorFrontmatter {
                name: "test".to_string(),
                description: "Test validator".to_string(),
                severity: Severity::Error,
                trigger: HookType::PreToolUse,
                match_criteria: Some(ValidatorMatch {
                    tools: vec!["Bash".to_string()],
                    files: vec![],
                }),
                trigger_matcher: Some("deploy_.*".to_string()),
                tags: vec![],
                once: false,
                timeout: 30,
            },
            body: String::new(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        };

        // Must match all criteria: hook type, tool, and triggerMatcher
        assert!(validator.matches(
            &MatchContext::new(HookType::PreToolUse)
                .with_tool("Bash")
                .with_event_context("deploy_production")
        ));

        // Fails if triggerMatcher doesn't match
        assert!(!validator.matches(
            &MatchContext::new(HookType::PreToolUse)
                .with_tool("Bash")
                .with_event_context("run_tests")
        ));

        // Fails if tool doesn't match
        assert!(!validator.matches(
            &MatchContext::new(HookType::PreToolUse)
                .with_tool("Write")
                .with_event_context("deploy_production")
        ));
    }

    #[test]
    fn test_match_context_from_json() {
        // Test extraction of tool_name
        let input = serde_json::json!({"tool_name": "Bash"});
        let ctx = MatchContext::from_json(HookType::PreToolUse, &input);
        assert_eq!(ctx.tool_name, Some("Bash".to_string()));
        assert_eq!(ctx.file_path, None);

        // Test extraction of file_path from tool_input
        let input = serde_json::json!({
            "tool_input": {"file_path": "/path/to/file.ts"}
        });
        let ctx = MatchContext::from_json(HookType::PostToolUse, &input);
        assert_eq!(ctx.file_path, Some("/path/to/file.ts".to_string()));

        // Test extraction of path (alternative field name)
        let input = serde_json::json!({
            "tool_input": {"path": "/other/path.rs"}
        });
        let ctx = MatchContext::from_json(HookType::PostToolUse, &input);
        assert_eq!(ctx.file_path, Some("/other/path.rs".to_string()));

        // Test extraction of event_context from notification_type
        let input = serde_json::json!({"notification_type": "agent_complete"});
        let ctx = MatchContext::from_json(HookType::Notification, &input);
        assert_eq!(ctx.event_context, Some("agent_complete".to_string()));

        // Test extraction of event_context from source
        let input = serde_json::json!({"source": "startup"});
        let ctx = MatchContext::from_json(HookType::SessionStart, &input);
        assert_eq!(ctx.event_context, Some("startup".to_string()));

        // Test empty input
        let input = serde_json::json!({});
        let ctx = MatchContext::from_json(HookType::PreToolUse, &input);
        assert_eq!(ctx.tool_name, None);
        assert_eq!(ctx.file_path, None);
        assert_eq!(ctx.event_context, None);
    }

    #[test]
    fn test_apply_defaults_sets_name_from_file_stem() {
        let mut frontmatter = ValidatorFrontmatter {
            name: String::new(),
            description: String::new(),
            severity: Severity::default(),
            trigger: HookType::PostToolUse,
            match_criteria: None,
            trigger_matcher: None,
            tags: vec![],
            once: false,
            timeout: DEFAULT_VALIDATOR_TIMEOUT_SECONDS,
        };

        frontmatter.apply_defaults(&PathBuf::from("/path/to/my-validator.md"), None);

        assert_eq!(frontmatter.name, "my-validator");
    }

    #[test]
    fn test_apply_defaults_sets_description_from_name() {
        let mut frontmatter = ValidatorFrontmatter {
            name: String::new(),
            description: String::new(),
            severity: Severity::default(),
            trigger: HookType::PostToolUse,
            match_criteria: None,
            trigger_matcher: None,
            tags: vec![],
            once: false,
            timeout: DEFAULT_VALIDATOR_TIMEOUT_SECONDS,
        };

        frontmatter.apply_defaults(&PathBuf::from("check-types.md"), None);

        assert_eq!(frontmatter.description, "Validator: check-types");
    }

    #[test]
    fn test_apply_defaults_sets_source_code_match_criteria_when_patterns_provided() {
        let mut frontmatter = ValidatorFrontmatter {
            name: "test".to_string(),
            description: "Test".to_string(),
            severity: Severity::default(),
            trigger: HookType::PostToolUse,
            match_criteria: None,
            trigger_matcher: None,
            tags: vec![],
            once: false,
            timeout: DEFAULT_VALIDATOR_TIMEOUT_SECONDS,
        };

        let patterns = vec!["*.rs".to_string(), "*.ts".to_string(), "*.py".to_string()];
        frontmatter.apply_defaults(&PathBuf::from("test.md"), Some(&patterns));

        let match_criteria = frontmatter
            .match_criteria
            .expect("match_criteria should be set");
        assert!(match_criteria.tools.is_empty(), "tools should be empty");
        assert_eq!(match_criteria.files.len(), 3);
        assert!(match_criteria.files.contains(&"*.rs".to_string()));
        assert!(match_criteria.files.contains(&"*.ts".to_string()));
        assert!(match_criteria.files.contains(&"*.py".to_string()));
    }

    #[test]
    fn test_apply_defaults_no_match_criteria_when_no_patterns() {
        let mut frontmatter = ValidatorFrontmatter {
            name: "test".to_string(),
            description: "Test".to_string(),
            severity: Severity::default(),
            trigger: HookType::PostToolUse,
            match_criteria: None,
            trigger_matcher: None,
            tags: vec![],
            once: false,
            timeout: DEFAULT_VALIDATOR_TIMEOUT_SECONDS,
        };

        frontmatter.apply_defaults(&PathBuf::from("test.md"), None);

        assert!(
            frontmatter.match_criteria.is_none(),
            "match_criteria should remain None when no patterns provided"
        );
    }

    #[test]
    fn test_apply_defaults_preserves_explicit_values() {
        const CUSTOM_TIMEOUT: u32 = DEFAULT_VALIDATOR_TIMEOUT_SECONDS * 2;
        let mut frontmatter = ValidatorFrontmatter {
            name: "explicit-name".to_string(),
            description: "Explicit description".to_string(),
            severity: Severity::Error,
            trigger: HookType::PreToolUse,
            match_criteria: Some(ValidatorMatch {
                tools: vec!["Bash".to_string()],
                files: vec!["*.sh".to_string()],
            }),
            trigger_matcher: None,
            tags: vec!["custom".to_string()],
            once: true,
            timeout: CUSTOM_TIMEOUT,
        };

        let patterns = vec!["*.rs".to_string()];
        frontmatter.apply_defaults(&PathBuf::from("other-name.md"), Some(&patterns));

        // All explicit values should be preserved
        assert_eq!(frontmatter.name, "explicit-name");
        assert_eq!(frontmatter.description, "Explicit description");
        assert_eq!(frontmatter.severity, Severity::Error);
        assert_eq!(frontmatter.trigger, HookType::PreToolUse);

        let match_criteria = frontmatter.match_criteria.unwrap();
        assert_eq!(match_criteria.tools, vec!["Bash"]);
        assert_eq!(match_criteria.files, vec!["*.sh"]);
    }

    #[test]
    fn test_apply_defaults_handles_path_without_extension() {
        let mut frontmatter = ValidatorFrontmatter {
            name: String::new(),
            description: String::new(),
            severity: Severity::default(),
            trigger: HookType::PostToolUse,
            match_criteria: None,
            trigger_matcher: None,
            tags: vec![],
            once: false,
            timeout: DEFAULT_VALIDATOR_TIMEOUT_SECONDS,
        };

        frontmatter.apply_defaults(&PathBuf::from("validator-name"), None);

        assert_eq!(frontmatter.name, "validator-name");
    }

    #[test]
    fn test_apply_defaults_stop_validators_no_file_patterns() {
        // Stop validators should NOT get default file patterns because they
        // check all changed files at session end, not specific patterns
        let mut frontmatter = ValidatorFrontmatter {
            name: "stop-validator".to_string(),
            description: "A stop validator".to_string(),
            severity: Severity::default(),
            trigger: HookType::Stop,
            match_criteria: None,
            trigger_matcher: None,
            tags: vec![],
            once: false,
            timeout: DEFAULT_VALIDATOR_TIMEOUT_SECONDS,
        };

        let patterns = vec!["*.rs".to_string(), "*.ts".to_string()];
        frontmatter.apply_defaults(&PathBuf::from("test.md"), Some(&patterns));

        // match_criteria should remain None for Stop validators
        assert!(
            frontmatter.match_criteria.is_none(),
            "Stop validators should not get default file patterns"
        );
    }
}
