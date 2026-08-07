//! Validator types — the rules-as-data model.
//!
//! Validators and RuleSets are markdown files with YAML frontmatter that
//! specify validation rules. This is the hook-free data layer: it describes
//! *what* a validator is (its match criteria, body) and *whether it
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

/// Match criteria for filtering when a validator should run.
///
/// The keys support pattern or value matching:
/// - `tools`: Regex patterns matched against tool names (case-insensitive)
/// - `files`: Glob patterns matched against file paths (case-insensitive)
/// - `project_types`: Detected project type keys matched against the
///   workspace's detected types (case-insensitive)
///
/// The keys combine with an implicit AND: every present key must match. An
/// absent (empty) key matches everything. The values inside one key combine
/// with OR.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatorMatch {
    /// Tool names to match (e.g., ["Write", "Edit"]).
    #[serde(default)]
    pub tools: Vec<String>,

    /// File glob patterns to match (e.g., ["*.ts", "src/**/*.rs"]).
    #[serde(default)]
    pub files: Vec<String>,

    /// Detected project type keys to match (e.g., ["rust", "python"]).
    ///
    /// The keys are the `PROJECT_TYPE_SPECS` spec keys from
    /// `swissarmyhammer-project-detection`, resolved for the workspace under
    /// review. When present, the workspace must have at least one of the named
    /// types detected.
    #[serde(default)]
    pub project_types: Vec<String>,
}

impl ValidatorMatch {
    /// Check if this match criteria is empty (matches everything).
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty() && self.files.is_empty() && self.project_types.is_empty()
    }

    /// Check the `project_types` key alone against a detected key set.
    ///
    /// Empty `project_types` matches everything. Keys compare
    /// case-insensitively against `detected` (the `PROJECT_TYPE_SPECS` spec
    /// keys, e.g. "rust"). This is the project-level applicability question
    /// the doctor surface asks — "does this criteria's project constraint fit
    /// this workspace?" — independent of any file or tool context.
    pub fn project_types_match<S: AsRef<str>>(&self, detected: &[S]) -> bool {
        if self.project_types.is_empty() {
            return true;
        }

        self.project_types.iter().any(|wanted| {
            detected
                .iter()
                .any(|key| key.as_ref().eq_ignore_ascii_case(wanted))
        })
    }

    /// Whether an optional match criteria's project-type constraint fits the
    /// detected project types. Absent criteria applies everywhere.
    ///
    /// The ONE answer to that question: the doctor's set-applicability row and
    /// the review engine's tool-rule selection both ask it here, so a set and
    /// the rules inside it can never be judged applicable by two different
    /// tests.
    pub fn criteria_applies<S: AsRef<str>>(criteria: Option<&Self>, detected: &[S]) -> bool {
        criteria.is_none_or(|c| c.project_types_match(detected))
    }
}

/// Define a builder method that stores an `impl Into<String>` value into an
/// `Option<String>` field and returns `self`.
///
/// One macro body backs every string-setting builder, so the setter logic
/// cannot drift between methods. The doc comment travels with the invocation.
macro_rules! builder_into {
    ($(#[$doc:meta])* $name:ident, $field:ident) => {
        $(#[$doc])*
        pub fn $name(mut self, value: impl Into<String>) -> Self {
            self.$field = Some(value.into());
            self
        }
    };
}

/// Define a builder method that collects an `IntoIterator<Item = String>` into
/// an `Option<Vec<String>>` field and returns `self`.
///
/// One macro body backs every collection-setting builder, so the setter logic
/// cannot drift between methods. The doc comment travels with the invocation.
macro_rules! builder_iter {
    ($(#[$doc:meta])* $name:ident, $field:ident) => {
        $(#[$doc])*
        pub fn $name<I: IntoIterator<Item = String>>(mut self, value: I) -> Self {
            self.$field = Some(value.into_iter().collect());
            self
        }
    };
}

/// Context for matching validators against a unit of work.
///
/// This encapsulates the information needed to decide whether a validator
/// applies: an optional tool name, a single file path, an accumulated set of
/// changed files, and a free-form event context string for `triggerMatcher`
/// regex matching. It carries no hook-event semantics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

    /// Detected project type keys for the workspace (e.g., ["rust", "python"]),
    /// resolved from the `PROJECT_TYPE_SPECS` detection. `None` when the caller
    /// did not resolve project types; a validator that requires `project_types`
    /// then does not match, mirroring the other required-but-absent keys.
    pub project_types: Option<Vec<String>>,
}

impl MatchContext {
    /// Create a new, empty match context.
    pub fn new() -> Self {
        Self::default()
    }

    builder_into!(
        /// Set the tool name.
        with_tool,
        tool_name
    );

    builder_into!(
        /// Set the file path.
        with_file,
        file_path
    );

    builder_into!(
        /// Set the event context for `triggerMatcher`.
        with_event_context,
        event_context
    );

    builder_iter!(
        /// Set the accumulated changed files.
        with_changed_files,
        changed_files
    );

    builder_iter!(
        /// Set the detected project type keys for the workspace.
        with_project_types,
        project_types
    );

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
            project_types: None,
        }
    }
}

/// Default timeout in seconds for validator execution.
fn default_timeout() -> u32 {
    DEFAULT_VALIDATOR_TIMEOUT_SECONDS
}

/// Default an empty `name` to the file stem of `path` (falling back to
/// "unnamed" when the stem is not valid UTF-8 or is absent).
///
/// The one shared name-defaulting body behind
/// [`ValidatorFrontmatter::apply_defaults`] and
/// [`RuleFrontmatter::apply_defaults`], so the stem extraction cannot drift
/// between them.
fn default_name_from_path(name: &mut String, path: &std::path::Path) {
    if name.is_empty() {
        *name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string();
    }
}

/// Default an empty `description` to `"{kind}: {name}"`.
///
/// The one shared description-defaulting body behind every `apply_defaults`
/// implementation — only the `kind` label ("Validator", "RuleSet", "Rule")
/// differs per caller.
fn default_description(description: &mut String, kind: &str, name: &str) {
    if description.is_empty() {
        *description = format!("{kind}: {name}");
    }
}

/// The one shared name-then-description defaulting sequence behind
/// [`ValidatorFrontmatter::apply_defaults`] and
/// [`RuleFrontmatter::apply_defaults`]: an empty `name` defaults to the file
/// stem, then an empty `description` defaults to `"{kind}: {name}"`. Only the
/// `kind` label differs per caller, so the sequence cannot drift between them.
fn apply_name_and_description(
    name: &mut String,
    description: &mut String,
    path: &std::path::Path,
    kind: &str,
) {
    default_name_from_path(name, path);
    default_description(description, kind, name);
}

/// YAML frontmatter for a validator file.
///
/// # Sensible Defaults
///
/// When frontmatter fields are omitted, the following defaults are applied:
///
/// - `name`: Defaults to the file stem (e.g., `check-types.md` → `check-types`)
/// - `description`: Defaults to "Validator: {name}"
/// - `match.files`: Defaults to source code patterns when `match` is omitted
/// - `timeout`: Defaults to 30 seconds
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatorFrontmatter {
    /// Unique name for the validator.
    /// Defaults to the file stem if not provided.
    #[serde(default)]
    pub name: String,

    /// Human-readable description.
    /// Defaults to "Validator: {name}" if not provided.
    #[serde(default)]
    pub description: String,

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
        source_code_patterns: Option<&[impl AsRef<str>]>,
    ) {
        apply_name_and_description(&mut self.name, &mut self.description, path, "Validator");

        // Default match criteria to source code files (if patterns provided)
        if self.match_criteria.is_none() {
            if let Some(patterns) = source_code_patterns {
                self.match_criteria = Some(ValidatorMatch {
                    tools: vec![],
                    files: patterns.iter().map(|s| s.as_ref().to_string()).collect(),
                    project_types: vec![],
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
    /// User validators from ~/.validators.
    User,
    /// Project validators from `<workspace_root>/.validators`.
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
/// The frontmatter contains configuration (match criteria)
/// while the body contains instructions for the validation agent.
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// Check if this validator matches the given context.
    ///
    /// A validator matches if:
    /// 1. If `triggerMatcher` is specified, the event context matches the regex
    /// 2. If tools are specified in match criteria, the tool name matches
    /// 3. If files are specified in match criteria, a file path matches a glob
    /// 4. If project types are specified in match criteria, a detected
    ///    workspace project type matches
    pub fn matches(&self, ctx: &MatchContext) -> bool {
        matches_trigger_and_criteria(
            self.frontmatter.trigger_matcher.as_deref(),
            self.frontmatter.match_criteria.as_ref(),
            &self.frontmatter.name,
            ctx,
        )
    }
}

/// The one shared match evaluation behind [`Validator::matches`] and
/// [`RuleSet::matches`]: the optional `triggerMatcher` regex must accept the
/// event context, then the optional match criteria must accept the context.
/// An absent matcher or absent criteria accepts everything.
fn matches_trigger_and_criteria(
    trigger_matcher: Option<&str>,
    match_criteria: Option<&ValidatorMatch>,
    owner: &str,
    ctx: &MatchContext,
) -> bool {
    matches_trigger_regex(trigger_matcher, ctx, owner)
        && match_criteria.is_none_or(|criteria| matches_criteria(criteria, ctx))
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

/// Check if the context satisfies every present key of the match criteria.
///
/// The keys combine with an implicit AND: `tools`, `files`, and
/// `project_types` must each match when present. An absent (empty) key
/// matches everything. This is the one shared evaluation both
/// [`Validator::matches`] and [`RuleSet::matches`] run.
fn matches_criteria(match_criteria: &ValidatorMatch, ctx: &MatchContext) -> bool {
    matches_tools(match_criteria, ctx)
        && matches_files(match_criteria, ctx)
        && matches_project_types(match_criteria, ctx)
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

/// Check if a detected workspace project type matches any of the
/// `project_types` keys.
///
/// Empty `project_types` matches everything. The keys compare
/// case-insensitively against the context's detected project type keys (the
/// `PROJECT_TYPE_SPECS` spec keys, e.g. "rust"). If project types are required
/// but the context carries none — unresolved (`None`) or detected empty — the
/// criteria does not match.
fn matches_project_types(match_criteria: &ValidatorMatch, ctx: &MatchContext) -> bool {
    if match_criteria.project_types.is_empty() {
        return true;
    }

    let Some(detected) = &ctx.project_types else {
        return false;
    };

    match_criteria.project_types_match(detected)
}

/// Result of running a validator.
///
/// The LLM returns just passed/failed with a message. The validator name
/// is known by the calling code from the validator's frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum ValidatorResult {
    /// Validation passed.
    #[serde(rename = "passed")]
    Passed {
        /// What the validator confirmed, in the words the LLM returned.
        message: String,
    },
    /// Validation failed.
    #[serde(rename = "failed")]
    Failed {
        /// The defect the validator found, in the words the LLM returned.
        message: String,
    },
}

/// Define a [`ValidatorResult`] factory method that wraps a message in the
/// named enum variant.
///
/// One macro body backs both factories, so the construction logic cannot
/// drift between them. The doc comment travels with the invocation.
macro_rules! result_method {
    ($(#[$doc:meta])* $name:ident, $variant:ident) => {
        $(#[$doc])*
        pub fn $name(message: impl Into<String>) -> Self {
            Self::$variant {
                message: message.into(),
            }
        }
    };
}

impl ValidatorResult {
    result_method!(
        /// Create a passing result.
        pass,
        Passed
    );

    result_method!(
        /// Create a failing result.
        fail,
        Failed
    );

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedValidator {
    /// Name of the validator that was executed.
    pub name: String,
    /// Result returned by the LLM.
    pub result: ValidatorResult,
}

/// Access to the [`ValidatorResult`] carried by an execution record.
///
/// Every container that pairs a result with metadata implements the one
/// accessor; `passed()` and `message()` are shared default delegations, so
/// the delegation logic cannot drift between containers.
pub trait HasValidatorResult {
    /// The result this record carries.
    fn result(&self) -> &ValidatorResult;

    /// Check if the validation passed.
    fn passed(&self) -> bool {
        self.result().passed()
    }

    /// Get the message from the result.
    fn message(&self) -> &str {
        self.result().message()
    }
}

impl HasValidatorResult for ExecutedValidator {
    fn result(&self) -> &ValidatorResult {
        &self.result
    }
}

// ============================================================================
// RuleSet Types
// ============================================================================

/// Metadata for a RuleSet, containing version and other package-level information.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuleSetMetadata {
    /// Semantic version (e.g., "1.0.0").
    #[serde(default)]
    pub version: String,
}

/// Manifest for a RuleSet, parsed from VALIDATOR.md.
///
/// The manifest defines shared configuration for all rules in the RuleSet:
/// common match criteria, default timeout (rules can override),
/// and metadata like name, version, tags.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuleSetManifest {
    /// Unique identifier for this RuleSet.
    pub name: String,

    /// Human-readable description of the RuleSet's purpose.
    pub description: String,

    /// Package metadata (version, etc.).
    #[serde(default)]
    pub metadata: RuleSetMetadata,

    /// Match criteria for filtering which work triggers this RuleSet.
    /// Rules inherit this; a rule-level `match` narrows it (intersection).
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

        default_description(&mut self.description, "RuleSet", &self.name);

        if self.metadata.version.is_empty() {
            self.metadata.version = "1.0.0".to_string();
        }
    }
}

/// Individual rule within a RuleSet.
///
/// Rules contain the actual validation logic and can override certain
/// RuleSet defaults (timeout). Rules inherit the set's match criteria; a
/// rule-level `match` narrows it (the rule applies to the intersection).
///
/// A rule with a [`ToolSpec`] is a tool rule — a language tool reports the
/// findings instead of an LLM. A rule without one is a prompt rule.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rule {
    /// Unique identifier for this rule within the RuleSet.
    pub name: String,

    /// Human-readable description of what this rule validates.
    pub description: String,

    /// Markdown body containing validation instructions.
    pub body: String,

    /// Override timeout (if None, inherits from RuleSet).
    pub timeout: Option<u32>,

    /// Optional rule-level match criteria that narrows the set's match.
    pub match_criteria: Option<ValidatorMatch>,

    /// The prompt rule this tool rule replaces when its tool is healthy.
    pub supersedes: Option<String>,

    /// The tool block. Present on tool rules, absent on prompt rules.
    pub tool: Option<ToolSpec>,
}

impl Rule {
    /// Get the effective timeout for this rule.
    pub fn effective_timeout(&self, ruleset: &RuleSet) -> u32 {
        self.timeout.unwrap_or(ruleset.manifest.timeout)
    }

    /// Whether this rule is a tool rule (its frontmatter carries a `tool` block).
    pub fn is_tool_rule(&self) -> bool {
        self.tool.is_some()
    }

    /// Check if this rule matches the given context, narrow-only.
    ///
    /// A rule matches when its set matches AND its own optional `match`
    /// criteria matches — the intersection. Both sides run through the same
    /// [`matches_criteria`] evaluation the set-level [`RuleSet::matches`] uses,
    /// so a rule never matches a file its set does not match.
    pub fn matches(&self, ruleset: &RuleSet, ctx: &MatchContext) -> bool {
        if !ruleset.matches(ctx) {
            return false;
        }

        match &self.match_criteria {
            Some(criteria) => matches_criteria(criteria, ctx),
            None => true,
        }
    }
}

/// Which inputs a tool rule's `run` script receives.
///
/// - `files`: the script receives the changed files as its arguments (`"$@"`).
/// - `workspace`: the script runs one time at the workspace root with no
///   arguments; the engine keeps only the findings in changed files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolScope {
    /// The script receives the changed files as arguments (`"$@"`).
    Files,
    /// The script runs once at the workspace root with no arguments.
    Workspace,
}

/// The fix a person runs to make a tool rule's tool usable, for a tool with no
/// package to install.
///
/// Clippy is a component of the Rust toolchain, not a package with its own
/// version, so no `install.commands` entry can pin it. Its rule states
/// `rustup component add clippy` here instead, and doctor reports that as the
/// fix for the degraded row.
///
/// A fix hint is text for a person, never a command the engine runs, and that
/// is a type rather than a convention. A hint is not a `String`: it offers no
/// `AsRef<str>`, no `Deref<Target = str>`, and no owned-string conversion, so
/// it cannot slide into any position that expects a command. It also lives on
/// [`ToolDoctor`], the block doctor reports from, while every command the
/// install lifecycle runs comes from [`ToolInstall::commands`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FixHint(String);

impl std::fmt::Display for FixHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for FixHint {
    fn from(text: String) -> Self {
        Self(text)
    }
}

/// The doctor commands that prove a tool rule's tools are usable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDoctor {
    /// The command that shows every tool the pipe needs is installed
    /// (for example `which ruff jq`).
    pub check_command: String,

    /// The command that shows the main tool's version
    /// (for example `ruff --version`).
    #[serde(default)]
    pub check_version_command: Option<String>,

    /// The fix a person runs when the tool has no package to install
    /// (for example `rustup component add clippy`). Doctor reports it; the
    /// install lifecycle never runs it.
    #[serde(default)]
    pub fix_hint: Option<FixHint>,
}

/// The install commands for a tool rule's tool, in order of preference.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolInstall {
    /// The install commands to try, in order. Each command should pin the
    /// tool version — an unpinned tool can change its rules and break the gate.
    #[serde(default)]
    pub commands: Vec<String>,
}

/// The `tool` block of a tool rule's frontmatter.
///
/// A rule with a `tool` block is a tool rule: a language tool examines the
/// code and reports the findings instead of an LLM. There is no output,
/// format, jq, regex, or filter configuration and no `exit.findings` key —
/// the `run` pipe is the mapping, and unknown keys are rejected at parse time.
///
/// The script's contract is its stdout: one finding per line, either
/// `path:line: message` or a `{"file": ..., "line": ..., "message": ...}`
/// JSON object per line (what `jq -c` emits). Empty stdout means clean.
/// Exit 0 means the script judged the code; a nonzero exit means the script
/// broke and no findings are read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolSpec {
    /// Which inputs the `run` script receives.
    pub scope: ToolScope,

    /// The shell script that runs the tool — the pipeline is the mapping.
    pub run: String,

    /// The commands that prove the script's tools are installed.
    #[serde(default)]
    pub doctor: Option<ToolDoctor>,

    /// How to install the tool when it is missing.
    #[serde(default)]
    pub install: Option<ToolInstall>,
}

/// Frontmatter for individual rule files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuleFrontmatter {
    /// Rule identifier within the RuleSet.
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// Optional timeout override.
    #[serde(default)]
    pub timeout: Option<u32>,

    /// Optional rule-level match criteria. Narrows the set's match — the rule
    /// applies to the intersection, and never matches a file its set does not
    /// match.
    #[serde(default, rename = "match")]
    pub match_criteria: Option<ValidatorMatch>,

    /// The prompt rule this tool rule replaces when its tool is healthy.
    #[serde(default)]
    pub supersedes: Option<String>,

    /// The tool block. Present on tool rules, absent on prompt rules.
    #[serde(default)]
    pub tool: Option<ToolSpec>,
}

impl RuleFrontmatter {
    /// Apply defaults based on the file path.
    pub fn apply_defaults(&mut self, path: &std::path::Path) {
        apply_name_and_description(&mut self.name, &mut self.description, path, "Rule");
    }
}

/// A rule file inside a RuleSet that failed to parse, retained for reporting.
///
/// A malformed rule (for example a bad `tool` block) never drops the whole
/// set: the set loads with its parseable rules, and each failure is recorded
/// here so the lint surface (`check validators` / `sah doctor`) can name the
/// offending file and its parse problem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleLoadFailure {
    /// The rule file that failed to parse.
    pub path: PathBuf,

    /// The parse problem, formatted for display.
    pub error: String,
}

/// A RuleSet package containing a manifest and multiple rules.
///
/// - VALIDATOR.md contains the manifest with shared configuration
/// - rules/ directory contains individual rule files
/// - Rules inherit the set's match criteria; a rule-level `match` narrows it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSet {
    /// Parsed manifest from VALIDATOR.md.
    pub manifest: RuleSetManifest,

    /// Rules loaded from the rules/ directory.
    pub rules: Vec<Rule>,

    /// Rule files that failed to parse and were skipped, retained so a
    /// malformed rule is reported instead of silently dropped.
    pub rule_failures: Vec<RuleLoadFailure>,

    /// The VALIDATOR.md prose body — everything after the frontmatter's closing
    /// `---`, trimmed. This is authored validator-WIDE guidance (intent, scope,
    /// blanket exclusions that apply across every rule), surfaced into the
    /// per-validator review prompt. Empty when the manifest has no body.
    pub manifest_body: String,

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

    /// Get the VALIDATOR.md prose body (validator-wide guidance), trimmed.
    pub fn manifest_body(&self) -> &str {
        &self.manifest_body
    }

    /// Check if this RuleSet matches the given context.
    ///
    /// A RuleSet matches if:
    /// 1. If `triggerMatcher` is specified, the event context matches the regex
    /// 2. If tools are specified in match criteria, the tool name matches
    /// 3. If files are specified in match criteria, a file path matches a glob
    /// 4. If project types are specified in match criteria, a detected
    ///    workspace project type matches
    pub fn matches(&self, ctx: &MatchContext) -> bool {
        matches_trigger_and_criteria(
            self.manifest.trigger_matcher.as_deref(),
            self.manifest.match_criteria.as_ref(),
            &self.manifest.name,
            ctx,
        )
    }
}

/// Result of executing a single rule within a RuleSet session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleResult {
    /// Name of the rule that was executed.
    pub rule_name: String,
    /// Result returned by the agent for this rule.
    pub result: ValidatorResult,
}

impl HasValidatorResult for RuleResult {
    fn result(&self) -> &ValidatorResult {
        &self.result
    }
}

/// Result of executing an entire RuleSet in a single agent session.
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// Get all failed rules.
    pub fn failed_rules(&self) -> Vec<&RuleResult> {
        self.rule_results.iter().filter(|r| !r.passed()).collect()
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
pub fn compile_glob_patterns<S: AsRef<str>>(patterns: &[S]) -> Vec<glob::Pattern> {
    patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p.as_ref()).ok())
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
    fn test_validator_match_is_empty() {
        let empty = ValidatorMatch::default();
        assert!(empty.is_empty());

        let with_tools = ValidatorMatch {
            tools: vec!["Write".to_string()],
            files: vec![],
            project_types: vec![],
        };
        assert!(!with_tools.is_empty());

        let with_project_types = ValidatorMatch {
            tools: vec![],
            files: vec![],
            project_types: vec!["rust".to_string()],
        };
        assert!(!with_project_types.is_empty());
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
                project_types: vec![],
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
                project_types: vec![],
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
                project_types: vec![],
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
                project_types: vec![],
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
                project_types: vec![],
            }),
            None,
        );

        assert!(validator
            .matches(&MatchContext::new().with_changed_files(vec!["anything.txt".to_string()])));
        assert!(validator.matches(&MatchContext::new()));
    }

    #[test]
    fn test_validator_files_only_ignores_workspace_project_types() {
        // Regression: a files-only match behaves exactly as before, whatever
        // project types the context carries.
        let validator = make_validator(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec!["*.rs".to_string()],
                project_types: vec![],
            }),
            None,
        );

        assert!(validator.matches(
            &MatchContext::new()
                .with_file("main.rs")
                .with_project_types(vec!["python".to_string()])
        ));
        assert!(!validator.matches(
            &MatchContext::new()
                .with_file("main.py")
                .with_project_types(vec!["rust".to_string()])
        ));
    }

    #[test]
    fn test_validator_matches_files_and_project_types_requires_both() {
        let validator = make_validator(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec!["**/*.py".to_string()],
                project_types: vec!["python".to_string()],
            }),
            None,
        );

        // Both criteria hold -> match.
        assert!(validator.matches(
            &MatchContext::new()
                .with_file("src/app.py")
                .with_project_types(vec!["python".to_string()])
        ));
        // File matches but the workspace is not python -> no match.
        assert!(!validator.matches(
            &MatchContext::new()
                .with_file("src/app.py")
                .with_project_types(vec!["rust".to_string()])
        ));
        // Workspace is python but the file does not match -> no match.
        assert!(!validator.matches(
            &MatchContext::new()
                .with_file("src/app.rs")
                .with_project_types(vec!["python".to_string()])
        ));
    }

    #[test]
    fn test_validator_matches_project_types_only() {
        let validator = make_validator(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec![],
                project_types: vec!["rust".to_string(), "python".to_string()],
            }),
            None,
        );

        // Any one of the named types (OR) -> every file matches.
        assert!(validator.matches(
            &MatchContext::new()
                .with_file("README.md")
                .with_project_types(vec!["rust".to_string()])
        ));
        // Case-insensitive comparison.
        assert!(validator.matches(
            &MatchContext::new()
                .with_file("README.md")
                .with_project_types(vec!["Rust".to_string()])
        ));
        // Non-matching workspace -> no file matches.
        assert!(!validator.matches(
            &MatchContext::new()
                .with_file("README.md")
                .with_project_types(vec!["go".to_string()])
        ));
        // No detected types at all -> no match.
        assert!(!validator.matches(
            &MatchContext::new()
                .with_file("README.md")
                .with_project_types(vec![])
        ));
        // Unresolved project types (no context value) -> no match.
        assert!(!validator.matches(&MatchContext::new().with_file("README.md")));
    }

    #[test]
    fn test_validator_match_project_types_serde_default() {
        // A manifest without the key parses unchanged.
        let parsed: ValidatorMatch = serde_yaml_ng::from_str("files:\n  - \"*.py\"\n").unwrap();
        assert!(parsed.project_types.is_empty());
        assert_eq!(parsed.files, vec!["*.py"]);

        // A manifest with the key parses into the field.
        let parsed: ValidatorMatch =
            serde_yaml_ng::from_str("files:\n  - \"*.py\"\nproject_types:\n  - python\n").unwrap();
        assert_eq!(parsed.project_types, vec!["python"]);
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
                project_types: vec![],
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

    /// The `apply_defaults` argument that supplies no source-code patterns.
    ///
    /// `apply_defaults` accepts any slice of string-like patterns, so a bare
    /// `None` leaves the element type open. This names the element type once.
    const NO_SOURCE_CODE_PATTERNS: Option<&[&str]> = None;

    #[test]
    fn test_apply_defaults_sets_name_from_file_stem() {
        let mut frontmatter = base_frontmatter();
        frontmatter.name = String::new();
        frontmatter.description = String::new();
        frontmatter.apply_defaults(
            &PathBuf::from("/path/to/my-validator.md"),
            NO_SOURCE_CODE_PATTERNS,
        );
        assert_eq!(frontmatter.name, "my-validator");
    }

    #[test]
    fn test_apply_defaults_sets_description_from_name() {
        let mut frontmatter = base_frontmatter();
        frontmatter.name = String::new();
        frontmatter.description = String::new();
        frontmatter.apply_defaults(&PathBuf::from("check-types.md"), NO_SOURCE_CODE_PATTERNS);
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
        frontmatter.apply_defaults(&PathBuf::from("test.md"), NO_SOURCE_CODE_PATTERNS);
        assert!(frontmatter.match_criteria.is_none());
    }

    #[test]
    fn test_apply_defaults_preserves_explicit_values() {
        const CUSTOM_TIMEOUT: u32 = DEFAULT_VALIDATOR_TIMEOUT_SECONDS * 2;
        let mut frontmatter = ValidatorFrontmatter {
            name: "explicit-name".to_string(),
            description: "Explicit description".to_string(),
            match_criteria: Some(ValidatorMatch {
                tools: vec!["Bash".to_string()],
                files: vec!["*.sh".to_string()],
                project_types: vec![],
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
        let match_criteria = frontmatter.match_criteria.unwrap();
        assert_eq!(match_criteria.tools, vec!["Bash"]);
        assert_eq!(match_criteria.files, vec!["*.sh"]);
    }

    fn base_frontmatter() -> ValidatorFrontmatter {
        ValidatorFrontmatter {
            name: "test".to_string(),
            description: "Test".to_string(),
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
                timeout: 30,
                once: false,
            },
            rules: vec![],
            rule_failures: vec![],
            manifest_body: String::new(),
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
                project_types: vec![],
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
                project_types: vec![],
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
                project_types: vec![],
            }),
            None,
        );
        assert!(rs.matches(&MatchContext::new().with_changed_files(vec!["app.ts".to_string()])));
        assert!(!rs.matches(&MatchContext::new().with_changed_files(vec!["app.py".to_string()])));
        assert!(!rs.matches(&MatchContext::new().with_changed_files(vec![])));
    }

    #[test]
    fn test_ruleset_matches_project_types() {
        let rs = make_ruleset(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec![],
                project_types: vec!["python".to_string()],
            }),
            None,
        );
        assert!(rs.matches(
            &MatchContext::new()
                .with_file("app.py")
                .with_project_types(vec!["python".to_string()])
        ));
        assert!(!rs.matches(
            &MatchContext::new()
                .with_file("app.py")
                .with_project_types(vec!["rust".to_string()])
        ));
        assert!(!rs.matches(&MatchContext::new().with_file("app.py")));
    }

    #[test]
    fn test_ruleset_name_and_description() {
        let rs = make_ruleset(None, None);
        assert_eq!(rs.name(), "test-ruleset");
        assert_eq!(rs.description(), "Test");
    }

    #[test]
    fn test_rule_effective_timeout_override() {
        let rs = make_ruleset(None, None);
        let rule = Rule {
            name: "test".to_string(),
            description: "Test".to_string(),
            body: "Body".to_string(),
            timeout: Some(60),
            ..Rule::default()
        };
        assert_eq!(rule.effective_timeout(&rs), 60);
    }

    #[test]
    fn test_rule_effective_timeout_inherits() {
        let rs = make_ruleset(None, None);
        let rule = Rule {
            name: "test".to_string(),
            description: "Test".to_string(),
            body: "Body".to_string(),
            timeout: None,
            ..Rule::default()
        };
        assert_eq!(rule.effective_timeout(&rs), 30);
    }

    /// A rule-level `match` narrows the set's match: the rule applies to the
    /// intersection, evaluated through the same `matches_criteria` path the
    /// set-level `RuleSet::matches` uses.
    #[test]
    fn test_rule_match_narrows_set_match() {
        let rs = make_ruleset(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec!["**/*.rs".to_string(), "**/*.py".to_string()],
                project_types: vec![],
            }),
            None,
        );
        let rule = Rule {
            name: "python-only".to_string(),
            match_criteria: Some(ValidatorMatch {
                tools: vec![],
                files: vec!["**/*.py".to_string()],
                project_types: vec![],
            }),
            ..Rule::default()
        };

        // The set matches .rs, but the rule narrows to .py only.
        let rust_ctx = MatchContext::new().with_file("src/main.rs");
        assert!(rs.matches(&rust_ctx));
        assert!(!rule.matches(&rs, &rust_ctx));

        // Both the set and the rule match .py.
        let python_ctx = MatchContext::new().with_file("scripts/tool.py");
        assert!(rs.matches(&python_ctx));
        assert!(rule.matches(&rs, &python_ctx));
    }

    /// A rule never matches a file its set does not match, even when the
    /// rule-level `match` matches it — narrow-only, never widen.
    #[test]
    fn test_rule_match_never_widens_set_match() {
        let rs = make_ruleset(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec!["**/*.rs".to_string()],
                project_types: vec![],
            }),
            None,
        );
        let rule = Rule {
            name: "js-widening".to_string(),
            match_criteria: Some(ValidatorMatch {
                tools: vec![],
                files: vec!["**/*.js".to_string()],
                project_types: vec![],
            }),
            ..Rule::default()
        };

        let js_ctx = MatchContext::new().with_file("web/app.js");
        assert!(!rs.matches(&js_ctx));
        assert!(!rule.matches(&rs, &js_ctx));
    }

    /// A rule without its own `match` inherits the set's match unchanged.
    #[test]
    fn test_rule_without_match_inherits_set_match() {
        let rs = make_ruleset(
            Some(ValidatorMatch {
                tools: vec![],
                files: vec!["**/*.rs".to_string()],
                project_types: vec![],
            }),
            None,
        );
        let rule = Rule {
            name: "inheriting".to_string(),
            ..Rule::default()
        };

        assert!(rule.matches(&rs, &MatchContext::new().with_file("src/main.rs")));
        assert!(!rule.matches(&rs, &MatchContext::new().with_file("web/app.js")));
    }

    #[test]
    fn test_rule_is_tool_rule() {
        let prompt_rule = Rule::default();
        assert!(!prompt_rule.is_tool_rule());

        let tool_rule = Rule {
            tool: Some(ToolSpec {
                scope: ToolScope::Files,
                run: "ruff check \"$@\"".to_string(),
                doctor: None,
                install: None,
            }),
            ..Rule::default()
        };
        assert!(tool_rule.is_tool_rule());
    }

    #[test]
    fn test_executed_ruleset_all_passed() {
        let executed = ExecutedRuleSet {
            ruleset_name: "test".to_string(),
            rule_results: vec![
                RuleResult {
                    rule_name: "r1".to_string(),
                    result: ValidatorResult::pass("ok".to_string()),
                },
                RuleResult {
                    rule_name: "r2".to_string(),
                    result: ValidatorResult::pass("ok".to_string()),
                },
            ],
        };
        assert!(executed.passed());
        assert!(executed.failed_rules().is_empty());
    }

    #[test]
    fn test_executed_ruleset_with_failure() {
        let executed = ExecutedRuleSet {
            ruleset_name: "test".to_string(),
            rule_results: vec![RuleResult {
                rule_name: "r1".to_string(),
                result: ValidatorResult::fail("issue".to_string()),
            }],
        };
        assert!(!executed.passed());
        assert_eq!(executed.failed_rules().len(), 1);
    }

    #[test]
    fn test_rule_result_passed() {
        let rr = RuleResult {
            rule_name: "test".to_string(),
            result: ValidatorResult::pass("all good".to_string()),
        };
        assert!(rr.passed());
        assert_eq!(rr.message(), "all good");
    }

    #[test]
    fn test_rule_result_failed() {
        let rr = RuleResult {
            rule_name: "test".to_string(),
            result: ValidatorResult::fail("warning".to_string()),
        };
        assert!(!rr.passed());
        assert_eq!(rr.message(), "warning");
    }

    #[test]
    fn test_executed_validator_passed() {
        let ev = ExecutedValidator {
            name: "test".to_string(),
            result: ValidatorResult::pass("ok".to_string()),
        };
        assert!(ev.passed());
        assert_eq!(ev.message(), "ok");
    }

    #[test]
    fn test_executed_validator_failed() {
        let ev = ExecutedValidator {
            name: "test".to_string(),
            result: ValidatorResult::fail("bad".to_string()),
        };
        assert!(!ev.passed());
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
        let compiled = compile_glob_patterns::<&str>(&[]);
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
