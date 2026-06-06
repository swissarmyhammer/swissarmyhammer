//! Validator types — the rules-as-data model.
//!
//! Validators and RuleSets are markdown files with YAML frontmatter that
//! specify validation rules. This is the hook-free data layer: it describes
//! *what* a validator is (its match criteria, severity, body) and *whether it
//! matches* a given tool/file context. It does not run anything and is not tied
//! to any hook event.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
/// Both `tools` and `files` support pattern matching:
/// - `tools`: Regex patterns matched against tool names (case-insensitive)
/// - `files`: Glob patterns matched against file paths (case-insensitive)
///
/// If both are specified, both must match for the validator to run.
/// If neither is specified (empty), the validator matches everything.
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

/// Context for matching validators against a unit of work.
///
/// This encapsulates the information needed to decide whether a validator
/// applies: an optional tool name, a single file path, an accumulated set of
/// changed files, and a free-form event context string for `triggerMatcher`
/// regex matching. It carries no hook-event semantics.
#[derive(Debug, Clone, Default)]
pub struct MatchContext {
    /// The tool name (for tool-pattern matching).
    pub tool_name: Option<String>,

    /// A single file path being operated on (if applicable).
    pub file_path: Option<String>,

    /// Event context string for `triggerMatcher` regex matching.
    pub event_context: Option<String>,

    /// Accumulated set of changed files. When present, file glob patterns match
    /// against any of these paths (the review fleet uses this to scope a
    /// validator to the files that changed).
    pub changed_files: Option<Vec<String>>,
}

impl MatchContext {
    /// Create a new, empty match context.
    pub fn new() -> Self {
        Self::default()
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

    /// Set the event context for `triggerMatcher`.
    pub fn with_event_context(mut self, context: impl Into<String>) -> Self {
        self.event_context = Some(context.into());
        self
    }

    /// Set the accumulated changed files.
    pub fn with_changed_files(mut self, files: Vec<String>) -> Self {
        self.changed_files = Some(files);
        self
    }

    /// Create from a JSON value, extracting tool name, file path, and event
    /// context from the conventional field names.
    pub fn from_json(input: &serde_json::Value) -> Self {
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
            .and_then(|v| v.as_str())
            .map(String::from);

        Self {
            tool_name,
            file_path,
            event_context,
            changed_files: None,
        }
    }
}

/// Default timeout in seconds for validator execution.
fn default_timeout() -> u32 {
    DEFAULT_VALIDATOR_TIMEOUT_SECONDS
}

/// YAML frontmatter for a validator file.
///
/// # Sensible Defaults
///
/// When frontmatter fields are omitted, the following defaults are applied:
///
/// - `name`: Defaults to the file stem (e.g., `check-types.md` → `check-types`)
/// - `description`: Defaults to "Validator: {name}"
/// - `severity`: Defaults to `warn`
/// - `match.files`: Defaults to source code patterns when `match` is omitted
/// - `timeout`: Defaults to 30 seconds
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

    /// Optional match criteria for filtering which work triggers this validator.
    ///
    /// When present, the validator only runs if the work matches the specified
    /// tools and/or file patterns. When absent, source-code file defaults may
    /// be applied (see [`ValidatorFrontmatter::apply_defaults`]).
    #[serde(default, rename = "match")]
    pub match_criteria: Option<ValidatorMatch>,

    /// Optional regex pattern matched against the context event string.
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
    /// - `match.files`: Source code patterns from `@file_groups/source_code`
    ///   (if provided and `match` is None)
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
        if self.match_criteria.is_none() {
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
    /// User validators from $XDG_DATA_HOME/validators.
    User,
    /// Project validators from ./.validators.
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
/// The frontmatter contains configuration (match criteria, severity)
/// while the body contains instructions for the validation agent.
#[derive(Debug, Clone)]
pub struct Validator {
    /// Parsed YAML frontmatter containing validator configuration.
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

    /// Check if this validator matches the given context.
    ///
    /// A validator matches if:
    /// 1. If `triggerMatcher` is specified, the event context matches the regex
    /// 2. If tools are specified in match criteria, the tool name matches
    /// 3. If files are specified in match criteria, a file path matches a glob
    pub fn matches(&self, ctx: &MatchContext) -> bool {
        // Check triggerMatcher regex if present
        if !self.matches_trigger_regex(ctx) {
            return false;
        }

        // Check match criteria if present
        if let Some(match_criteria) = &self.frontmatter.match_criteria {
            if !matches_tools(match_criteria, ctx) {
                return false;
            }
            if !matches_files(match_criteria, ctx) {
                return false;
            }
        }

        true
    }

    /// Check if the event context matches the `triggerMatcher` regex.
    fn matches_trigger_regex(&self, ctx: &MatchContext) -> bool {
        matches_trigger_regex(
            self.frontmatter.trigger_matcher.as_deref(),
            ctx,
            &self.frontmatter.name,
        )
    }
}

/// Check if the event context matches the optional `triggerMatcher` regex.
///
/// Shared by [`Validator`] and [`RuleSet`]. Returns `true` when no matcher is
/// set, `false` when a matcher is set but there is no context to match, and the
/// regex result otherwise. Invalid regexes fail closed (no match) with a warning.
fn matches_trigger_regex(trigger_matcher: Option<&str>, ctx: &MatchContext, owner: &str) -> bool {
    let Some(trigger_matcher) = trigger_matcher else {
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
                "Invalid triggerMatcher regex '{}' in '{}': {}",
                trigger_matcher,
                owner,
                e
            );
            false
        }
    }
}

/// Check if the tool name matches any of the tool patterns.
///
/// Empty `tools` matches everything. Patterns are treated as anchored,
/// case-insensitive regexes, falling back to a case-insensitive literal compare
/// when the pattern is not a valid regex.
fn matches_tools(match_criteria: &ValidatorMatch, ctx: &MatchContext) -> bool {
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

/// Check if a file matches any of the file glob patterns.
///
/// Empty `files` matches everything. When `changed_files` is present, the
/// patterns match against any of those paths; otherwise they match against the
/// single `file_path`. If file patterns are specified but there is nothing to
/// match against, the criteria does not match.
fn matches_files(match_criteria: &ValidatorMatch, ctx: &MatchContext) -> bool {
    if match_criteria.files.is_empty() {
        return true;
    }

    let compiled = compile_glob_patterns(&match_criteria.files);

    if let Some(files) = &ctx.changed_files {
        return files.iter().any(|f| matches_any_pattern(f, &compiled));
    }

    let Some(path) = &ctx.file_path else {
        return false;
    };
    matches_any_pattern(path, &compiled)
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
// RuleSet Types
// ============================================================================

/// Metadata for a RuleSet, containing version and other package-level information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleSetMetadata {
    /// Semantic version (e.g., "1.0.0").
    #[serde(default)]
    pub version: String,
}

/// Manifest for a RuleSet, parsed from VALIDATOR.md.
///
/// The manifest defines shared configuration for all rules in the RuleSet:
/// common match criteria, default severity and timeout (rules can override),
/// and metadata like name, version, tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSetManifest {
    /// Unique identifier for this RuleSet.
    pub name: String,

    /// Human-readable description of the RuleSet's purpose.
    pub description: String,

    /// Package metadata (version, etc.).
    #[serde(default)]
    pub metadata: RuleSetMetadata,

    /// Match criteria for filtering which work triggers this RuleSet.
    /// Rules inherit this and cannot override.
    #[serde(default, rename = "match")]
    pub match_criteria: Option<ValidatorMatch>,

    /// Optional regex pattern matched against the context event string.
    /// Rules inherit this and cannot override.
    #[serde(default, rename = "triggerMatcher")]
    pub trigger_matcher: Option<String>,

    /// Tags for categorization and organization.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Probe names this RuleSet requests from the probe catalog.
    ///
    /// Parsed as plain strings here — whether each name is a real catalog entry
    /// is validated downstream by the probe registry's `probe_exists` and the
    /// `check validators` command, not by this loader.
    #[serde(default)]
    pub probes: Vec<String>,

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
    /// - `name`: Directory name if empty
    /// - `description`: "RuleSet: {name}" if empty
    /// - `metadata.version`: "1.0.0" if empty
    pub fn apply_defaults(&mut self, dir_path: &std::path::Path) {
        if self.name.is_empty() {
            self.name = dir_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unnamed")
                .to_string();
        }

        if self.description.is_empty() {
            self.description = format!("RuleSet: {}", self.name);
        }

        if self.metadata.version.is_empty() {
            self.metadata.version = "1.0.0".to_string();
        }
    }
}

/// Individual rule within a RuleSet.
///
/// Rules contain the actual validation logic and can override certain
/// RuleSet defaults (severity, timeout) while inheriting match criteria.
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
    pub fn effective_severity(&self, ruleset: &RuleSet) -> Severity {
        self.severity.unwrap_or(ruleset.manifest.severity)
    }

    /// Get the effective timeout for this rule.
    pub fn effective_timeout(&self, ruleset: &RuleSet) -> u32 {
        self.timeout.unwrap_or(ruleset.manifest.timeout)
    }
}

/// Frontmatter for individual rule files.
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
        if self.name.is_empty() {
            self.name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unnamed")
                .to_string();
        }

        if self.description.is_empty() {
            self.description = format!("Rule: {}", self.name);
        }
    }
}

/// A RuleSet package containing a manifest and multiple rules.
///
/// - VALIDATOR.md contains the manifest with shared configuration
/// - rules/ directory contains individual rule files
/// - All rules in a RuleSet share the same match criteria
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

    /// Check if this RuleSet matches the given context.
    ///
    /// A RuleSet matches if:
    /// 1. If `triggerMatcher` is specified, the event context matches the regex
    /// 2. If tools are specified in match criteria, the tool name matches
    /// 3. If files are specified in match criteria, a file path matches a glob
    pub fn matches(&self, ctx: &MatchContext) -> bool {
        if !matches_trigger_regex(
            self.manifest.trigger_matcher.as_deref(),
            ctx,
            &self.manifest.name,
        ) {
            return false;
        }

        if let Some(match_criteria) = &self.manifest.match_criteria {
            if !matches_tools(match_criteria, ctx) {
                return false;
            }
            if !matches_files(match_criteria, ctx) {
                return false;
            }
        }

        true
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

/// Standard match options for glob pattern matching across all validator contexts.
///
/// Uses case-insensitive matching with default settings for everything else.
pub const GLOB_MATCH_OPTIONS: glob::MatchOptions = glob::MatchOptions {
    case_sensitive: false,
    require_literal_separator: false,
    require_literal_leading_dot: false,
};

/// Pre-compile a slice of glob pattern strings into `glob::Pattern` objects.
///
/// Invalid patterns are silently skipped.
pub fn compile_glob_patterns(patterns: &[String]) -> Vec<glob::Pattern> {
    patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect()
}

/// Check whether a path matches any of the pre-compiled glob patterns.
///
/// Uses case-insensitive matching via [`GLOB_MATCH_OPTIONS`].
pub fn matches_any_pattern(path: &str, compiled: &[glob::Pattern]) -> bool {
    compiled
        .iter()
        .any(|p| p.matches_with(path, GLOB_MATCH_OPTIONS))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_validator(
        match_criteria: Option<ValidatorMatch>,
        trigger_matcher: Option<String>,
    ) -> Validator {
        Validator {
            frontmatter: ValidatorFrontmatter {
                name: "test".to_string(),
                description: "Test validator".to_string(),
                severity: Severity::Error,
                match_criteria,
                trigger_matcher,
                tags: vec![],
                once: false,
                timeout: 30,
            },
            body: String::new(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        }
    }

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
    fn test_validator_no_criteria_matches_everything() {
        let validator = make_validator(None, None);
        assert!(validator.matches(&MatchContext::new()));
        assert!(validator.matches(&MatchContext::new().with_tool("Write")));
    }

    #[test]
    fn test_validator_matches_tool_filter() {
        let validator = make_validator(
            Some(ValidatorMatch {
                tools: vec!["Write".to_string(), "Edit".to_string()],
                files: vec![],
            }),
            None,
        );

        assert!(validator.matches(&MatchContext::new().with_tool("Write")));
        assert!(validator.matches(&MatchContext::new().with_tool("Edit")));
        // Case-insensitive matching
        assert!(validator.matches(&MatchContext::new().with_tool("write")));
        assert!(validator.matches(&MatchContext::new().with_tool("WRITE")));
        assert!(validator.matches(&MatchContext::new().with_tool("eDiT")));
        assert!(!validator.matches(&MatchContext::new().with_tool("Bash")));
        // No tool given but tools required -> no match
        assert!(!validator.matches(&MatchContext::new()));
    }

    #[test]
    fn test_validator_matches_tool_regex() {
        let validator = make_validator(
            Some(ValidatorMatch {
                tools: vec!["Write|Edit".to_string(), "Bash.*".to_string()],
                files: vec![],
            }),
            None,
        );

        assert!(validator.matches(&MatchContext::new().with_tool("Write")));
        assert!(validator.matches(&MatchContext::new().with_tool("Edit")));
        assert!(validator.matches(&MatchContext::new().with_tool("WRITE")));
        assert!(validator.matches(&MatchContext::new().with_tool("Bash")));
        assert!(validator.matches(&MatchContext::new().with_tool("BashCommand")));
        assert!(validator.matches(&MatchContext::new().with_tool("bash")));
        assert!(!validator.matches(&MatchContext::new().with_tool("Read")));
    }

    #[test]
    fn test_validator_matches_file_filter() {
        let validator = make_validator(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec!["*.ts".to_string(), "src/**/*.rs".to_string()],
            }),
            None,
        );

        assert!(validator.matches(&MatchContext::new().with_file("test.ts")));
        assert!(validator.matches(&MatchContext::new().with_file("src/lib/utils.rs")));
        // Case-insensitive file matching
        assert!(validator.matches(&MatchContext::new().with_file("TEST.TS")));
        assert!(validator.matches(&MatchContext::new().with_file("Test.Ts")));
        assert!(!validator.matches(&MatchContext::new().with_file("test.js")));
        // No file given but files required -> no match
        assert!(!validator.matches(&MatchContext::new()));
    }

    #[test]
    fn test_validator_matches_changed_files() {
        let validator = make_validator(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec!["*.rs".to_string()],
            }),
            None,
        );

        // Matching changed file
        assert!(
            validator.matches(&MatchContext::new().with_changed_files(vec!["foo.rs".to_string()]))
        );
        // Non-matching changed file
        assert!(
            !validator.matches(&MatchContext::new().with_changed_files(vec!["foo.py".to_string()]))
        );
        // Empty changed files with file patterns -> no match
        assert!(!validator.matches(&MatchContext::new().with_changed_files(vec![])));
    }

    #[test]
    fn test_validator_empty_files_matches_with_changed_files() {
        // Empty files matches everything regardless of changed files.
        let validator = make_validator(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec![],
            }),
            None,
        );

        assert!(validator
            .matches(&MatchContext::new().with_changed_files(vec!["anything.txt".to_string()])));
        assert!(validator.matches(&MatchContext::new()));
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
        let validator = make_validator(None, Some("agent_.*_complete".to_string()));

        assert!(validator.matches(&MatchContext::new().with_event_context("agent_task_complete")));
        // Case-insensitive
        assert!(validator.matches(&MatchContext::new().with_event_context("AGENT_TASK_COMPLETE")));
        // Non-matching
        assert!(!validator.matches(&MatchContext::new().with_event_context("something_else")));
        // No context with triggerMatcher present -> no match
        assert!(!validator.matches(&MatchContext::new()));
    }

    #[test]
    fn test_validator_trigger_matcher_invalid_regex() {
        let validator = make_validator(None, Some("[invalid(regex".to_string()));
        assert!(!validator.matches(&MatchContext::new().with_event_context("any_context")));
    }

    #[test]
    fn test_validator_matches_combined_criteria() {
        let validator = make_validator(
            Some(ValidatorMatch {
                tools: vec!["Bash".to_string()],
                files: vec![],
            }),
            Some("deploy_.*".to_string()),
        );

        // Must match all criteria: tool and triggerMatcher
        assert!(validator.matches(
            &MatchContext::new()
                .with_tool("Bash")
                .with_event_context("deploy_production")
        ));
        // Fails if triggerMatcher doesn't match
        assert!(!validator.matches(
            &MatchContext::new()
                .with_tool("Bash")
                .with_event_context("run_tests")
        ));
        // Fails if tool doesn't match
        assert!(!validator.matches(
            &MatchContext::new()
                .with_tool("Write")
                .with_event_context("deploy_production")
        ));
    }

    #[test]
    fn test_match_context_from_json() {
        let input = serde_json::json!({"tool_name": "Bash"});
        let ctx = MatchContext::from_json(&input);
        assert_eq!(ctx.tool_name, Some("Bash".to_string()));
        assert_eq!(ctx.file_path, None);

        let input = serde_json::json!({"tool_input": {"file_path": "/path/to/file.ts"}});
        let ctx = MatchContext::from_json(&input);
        assert_eq!(ctx.file_path, Some("/path/to/file.ts".to_string()));

        let input = serde_json::json!({"tool_input": {"path": "/other/path.rs"}});
        let ctx = MatchContext::from_json(&input);
        assert_eq!(ctx.file_path, Some("/other/path.rs".to_string()));

        let input = serde_json::json!({"notification_type": "agent_complete"});
        let ctx = MatchContext::from_json(&input);
        assert_eq!(ctx.event_context, Some("agent_complete".to_string()));

        let input = serde_json::json!({"source": "startup"});
        let ctx = MatchContext::from_json(&input);
        assert_eq!(ctx.event_context, Some("startup".to_string()));

        let input = serde_json::json!({});
        let ctx = MatchContext::from_json(&input);
        assert_eq!(ctx.tool_name, None);
        assert_eq!(ctx.file_path, None);
        assert_eq!(ctx.event_context, None);
    }

    #[test]
    fn test_match_context_from_json_file_field() {
        let input = serde_json::json!({"tool_input": {"file": "/path/to/file.py"}});
        let ctx = MatchContext::from_json(&input);
        assert_eq!(ctx.file_path, Some("/path/to/file.py".to_string()));
    }

    #[test]
    fn test_match_context_from_json_subagent_type() {
        let input = serde_json::json!({"subagent_type": "task_runner"});
        let ctx = MatchContext::from_json(&input);
        assert_eq!(ctx.event_context, Some("task_runner".to_string()));
    }

    #[test]
    fn test_apply_defaults_sets_name_from_file_stem() {
        let mut frontmatter = base_frontmatter();
        frontmatter.name = String::new();
        frontmatter.description = String::new();
        frontmatter.apply_defaults(&PathBuf::from("/path/to/my-validator.md"), None);
        assert_eq!(frontmatter.name, "my-validator");
    }

    #[test]
    fn test_apply_defaults_sets_description_from_name() {
        let mut frontmatter = base_frontmatter();
        frontmatter.name = String::new();
        frontmatter.description = String::new();
        frontmatter.apply_defaults(&PathBuf::from("check-types.md"), None);
        assert_eq!(frontmatter.description, "Validator: check-types");
    }

    #[test]
    fn test_apply_defaults_sets_source_code_match_criteria_when_patterns_provided() {
        let mut frontmatter = base_frontmatter();
        let patterns = vec!["*.rs".to_string(), "*.ts".to_string(), "*.py".to_string()];
        frontmatter.apply_defaults(&PathBuf::from("test.md"), Some(&patterns));

        let match_criteria = frontmatter
            .match_criteria
            .expect("match_criteria should be set");
        assert!(match_criteria.tools.is_empty());
        assert_eq!(match_criteria.files.len(), 3);
        assert!(match_criteria.files.contains(&"*.rs".to_string()));
    }

    #[test]
    fn test_apply_defaults_no_match_criteria_when_no_patterns() {
        let mut frontmatter = base_frontmatter();
        frontmatter.match_criteria = None;
        frontmatter.apply_defaults(&PathBuf::from("test.md"), None);
        assert!(frontmatter.match_criteria.is_none());
    }

    #[test]
    fn test_apply_defaults_preserves_explicit_values() {
        const CUSTOM_TIMEOUT: u32 = DEFAULT_VALIDATOR_TIMEOUT_SECONDS * 2;
        let mut frontmatter = ValidatorFrontmatter {
            name: "explicit-name".to_string(),
            description: "Explicit description".to_string(),
            severity: Severity::Error,
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

        assert_eq!(frontmatter.name, "explicit-name");
        assert_eq!(frontmatter.description, "Explicit description");
        assert_eq!(frontmatter.severity, Severity::Error);
        let match_criteria = frontmatter.match_criteria.unwrap();
        assert_eq!(match_criteria.tools, vec!["Bash"]);
        assert_eq!(match_criteria.files, vec!["*.sh"]);
    }

    fn base_frontmatter() -> ValidatorFrontmatter {
        ValidatorFrontmatter {
            name: "test".to_string(),
            description: "Test".to_string(),
            severity: Severity::default(),
            match_criteria: None,
            trigger_matcher: None,
            tags: vec![],
            once: false,
            timeout: DEFAULT_VALIDATOR_TIMEOUT_SECONDS,
        }
    }

    // =========================================================================
    // RuleSet Matching Tests
    // =========================================================================

    fn make_ruleset(
        match_criteria: Option<ValidatorMatch>,
        trigger_matcher: Option<String>,
    ) -> RuleSet {
        RuleSet {
            manifest: RuleSetManifest {
                name: "test-ruleset".to_string(),
                description: "Test".to_string(),
                metadata: RuleSetMetadata {
                    version: "1.0.0".to_string(),
                },
                match_criteria,
                trigger_matcher,
                tags: vec![],
                probes: vec![],
                severity: Severity::Error,
                timeout: 30,
                once: false,
            },
            rules: vec![],
            source: ValidatorSource::Builtin,
            base_path: PathBuf::from("/test"),
        }
    }

    #[test]
    fn test_ruleset_no_criteria_matches_everything() {
        let rs = make_ruleset(None, None);
        assert!(rs.matches(&MatchContext::new()));
        assert!(rs.matches(&MatchContext::new().with_tool("Write")));
    }

    #[test]
    fn test_ruleset_matches_tool_filter() {
        let rs = make_ruleset(
            Some(ValidatorMatch {
                tools: vec!["Write".to_string(), "Edit".to_string()],
                files: vec![],
            }),
            None,
        );
        assert!(rs.matches(&MatchContext::new().with_tool("Write")));
        assert!(rs.matches(&MatchContext::new().with_tool("write")));
        assert!(!rs.matches(&MatchContext::new().with_tool("Bash")));
        assert!(!rs.matches(&MatchContext::new()));
    }

    #[test]
    fn test_ruleset_matches_file_filter() {
        let rs = make_ruleset(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec!["*.ts".to_string(), "src/**/*.rs".to_string()],
            }),
            None,
        );
        assert!(rs.matches(&MatchContext::new().with_file("test.ts")));
        assert!(rs.matches(&MatchContext::new().with_file("src/lib.rs")));
        assert!(!rs.matches(&MatchContext::new().with_file("test.py")));
        assert!(!rs.matches(&MatchContext::new()));
    }

    #[test]
    fn test_ruleset_matches_trigger_matcher() {
        let rs = make_ruleset(None, Some("agent_.*".to_string()));
        assert!(rs.matches(&MatchContext::new().with_event_context("agent_complete")));
        assert!(!rs.matches(&MatchContext::new().with_event_context("user_input")));
        assert!(!rs.matches(&MatchContext::new()));
    }

    #[test]
    fn test_ruleset_matches_invalid_trigger_regex() {
        let rs = make_ruleset(None, Some("[invalid(".to_string()));
        assert!(!rs.matches(&MatchContext::new().with_event_context("anything")));
    }

    #[test]
    fn test_ruleset_matches_changed_files() {
        let rs = make_ruleset(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec!["*.ts".to_string()],
            }),
            None,
        );
        assert!(rs.matches(&MatchContext::new().with_changed_files(vec!["app.ts".to_string()])));
        assert!(!rs.matches(&MatchContext::new().with_changed_files(vec!["app.py".to_string()])));
        assert!(!rs.matches(&MatchContext::new().with_changed_files(vec![])));
    }

    #[test]
    fn test_ruleset_name_and_description() {
        let rs = make_ruleset(None, None);
        assert_eq!(rs.name(), "test-ruleset");
        assert_eq!(rs.description(), "Test");
    }

    #[test]
    fn test_rule_effective_severity_override() {
        let rs = make_ruleset(None, None);
        let rule = Rule {
            name: "test".to_string(),
            description: "Test".to_string(),
            body: "Body".to_string(),
            severity: Some(Severity::Warn),
            timeout: Some(60),
        };
        assert_eq!(rule.effective_severity(&rs), Severity::Warn);
        assert_eq!(rule.effective_timeout(&rs), 60);
    }

    #[test]
    fn test_rule_effective_severity_inherits() {
        let rs = make_ruleset(None, None);
        let rule = Rule {
            name: "test".to_string(),
            description: "Test".to_string(),
            body: "Body".to_string(),
            severity: None,
            timeout: None,
        };
        assert_eq!(rule.effective_severity(&rs), Severity::Error);
        assert_eq!(rule.effective_timeout(&rs), 30);
    }

    #[test]
    fn test_executed_ruleset_all_passed() {
        let executed = ExecutedRuleSet {
            ruleset_name: "test".to_string(),
            rule_results: vec![
                RuleResult {
                    rule_name: "r1".to_string(),
                    severity: Severity::Error,
                    result: ValidatorResult::pass("ok".to_string()),
                },
                RuleResult {
                    rule_name: "r2".to_string(),
                    severity: Severity::Warn,
                    result: ValidatorResult::pass("ok".to_string()),
                },
            ],
        };
        assert!(executed.passed());
        assert!(!executed.has_blocking_failure());
        assert!(executed.failed_rules().is_empty());
        assert!(executed.blocking_failures().is_empty());
    }

    #[test]
    fn test_executed_ruleset_with_warn_failure() {
        let executed = ExecutedRuleSet {
            ruleset_name: "test".to_string(),
            rule_results: vec![RuleResult {
                rule_name: "r1".to_string(),
                severity: Severity::Warn,
                result: ValidatorResult::fail("issue".to_string()),
            }],
        };
        assert!(!executed.passed());
        assert!(!executed.has_blocking_failure());
        assert_eq!(executed.failed_rules().len(), 1);
        assert!(executed.blocking_failures().is_empty());
    }

    #[test]
    fn test_executed_ruleset_with_error_failure() {
        let executed = ExecutedRuleSet {
            ruleset_name: "test".to_string(),
            rule_results: vec![RuleResult {
                rule_name: "r1".to_string(),
                severity: Severity::Error,
                result: ValidatorResult::fail("bad".to_string()),
            }],
        };
        assert!(!executed.passed());
        assert!(executed.has_blocking_failure());
        assert_eq!(executed.blocking_failures().len(), 1);
    }

    #[test]
    fn test_rule_result_passed() {
        let rr = RuleResult {
            rule_name: "test".to_string(),
            severity: Severity::Error,
            result: ValidatorResult::pass("all good".to_string()),
        };
        assert!(rr.passed());
        assert!(!rr.is_blocking());
        assert_eq!(rr.message(), "all good");
    }

    #[test]
    fn test_rule_result_blocking() {
        let rr = RuleResult {
            rule_name: "test".to_string(),
            severity: Severity::Error,
            result: ValidatorResult::fail("bad".to_string()),
        };
        assert!(!rr.passed());
        assert!(rr.is_blocking());
        assert_eq!(rr.message(), "bad");
    }

    #[test]
    fn test_rule_result_warn_not_blocking() {
        let rr = RuleResult {
            rule_name: "test".to_string(),
            severity: Severity::Warn,
            result: ValidatorResult::fail("warning".to_string()),
        };
        assert!(!rr.passed());
        assert!(!rr.is_blocking());
    }

    #[test]
    fn test_executed_validator_passed() {
        let ev = ExecutedValidator {
            name: "test".to_string(),
            severity: Severity::Error,
            result: ValidatorResult::pass("ok".to_string()),
        };
        assert!(ev.passed());
        assert!(!ev.is_blocking());
        assert_eq!(ev.message(), "ok");
    }

    #[test]
    fn test_executed_validator_blocking() {
        let ev = ExecutedValidator {
            name: "test".to_string(),
            severity: Severity::Error,
            result: ValidatorResult::fail("bad".to_string()),
        };
        assert!(!ev.passed());
        assert!(ev.is_blocking());
    }

    #[test]
    fn test_executed_validator_warn_not_blocking() {
        let ev = ExecutedValidator {
            name: "test".to_string(),
            severity: Severity::Warn,
            result: ValidatorResult::fail("warn".to_string()),
        };
        assert!(!ev.passed());
        assert!(!ev.is_blocking());
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Info.to_string(), "info");
        assert_eq!(Severity::Warn.to_string(), "warn");
        assert_eq!(Severity::Error.to_string(), "error");
    }

    #[test]
    fn test_validator_source_display() {
        assert_eq!(ValidatorSource::Builtin.to_string(), "builtin");
        assert_eq!(ValidatorSource::User.to_string(), "user");
        assert_eq!(ValidatorSource::Project.to_string(), "project");
    }

    #[test]
    fn test_ruleset_manifest_apply_defaults() {
        let mut manifest = RuleSetManifest {
            name: String::new(),
            description: String::new(),
            metadata: RuleSetMetadata {
                version: String::new(),
            },
            match_criteria: None,
            trigger_matcher: None,
            tags: vec![],
            probes: vec![],
            severity: Severity::Warn,
            timeout: 30,
            once: false,
        };
        manifest.apply_defaults(std::path::Path::new("/path/to/my-rules"));
        assert_eq!(manifest.name, "my-rules");
        assert_eq!(manifest.description, "RuleSet: my-rules");
        assert_eq!(manifest.metadata.version, "1.0.0");
    }

    #[test]
    fn test_ruleset_manifest_apply_defaults_preserves_values() {
        let mut manifest = RuleSetManifest {
            name: "explicit".to_string(),
            description: "My description".to_string(),
            metadata: RuleSetMetadata {
                version: "2.0.0".to_string(),
            },
            match_criteria: None,
            trigger_matcher: None,
            tags: vec![],
            probes: vec![],
            severity: Severity::Error,
            timeout: 60,
            once: true,
        };
        manifest.apply_defaults(std::path::Path::new("other-name"));
        assert_eq!(manifest.name, "explicit");
        assert_eq!(manifest.description, "My description");
        assert_eq!(manifest.metadata.version, "2.0.0");
    }

    #[test]
    fn test_compile_glob_patterns_valid() {
        let patterns = vec!["*.rs".to_string(), "src/**/*.ts".to_string()];
        let compiled = compile_glob_patterns(&patterns);
        assert_eq!(compiled.len(), 2);
    }

    #[test]
    fn test_compile_glob_patterns_skips_invalid() {
        let patterns = vec!["*.rs".to_string(), "[invalid".to_string()];
        let compiled = compile_glob_patterns(&patterns);
        assert_eq!(compiled.len(), 1);
    }

    #[test]
    fn test_compile_glob_patterns_empty() {
        let compiled = compile_glob_patterns(&[]);
        assert!(compiled.is_empty());
    }

    #[test]
    fn test_matches_any_pattern_basic() {
        let compiled = compile_glob_patterns(&["*.rs".to_string(), "*.ts".to_string()]);
        assert!(matches_any_pattern("main.rs", &compiled));
        assert!(matches_any_pattern("index.ts", &compiled));
        assert!(!matches_any_pattern("style.css", &compiled));
    }

    #[test]
    fn test_matches_any_pattern_case_insensitive() {
        let compiled = compile_glob_patterns(&["*.RS".to_string()]);
        assert!(matches_any_pattern("main.rs", &compiled));
        assert!(matches_any_pattern("main.RS", &compiled));
    }

    #[test]
    fn test_matches_any_pattern_empty_patterns() {
        assert!(!matches_any_pattern("anything.rs", &[]));
    }
}
