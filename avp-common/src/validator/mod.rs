//! Validator module for the Agent Validator Protocol.
//!
//! This module provides types, parsing, and loading for AVP validators.
//! Validators are markdown files with YAML frontmatter that specify
//! validation rules to run against hook events.
//!
//! # Validator Format
//!
//! ```markdown
//! ---
//! name: no-secrets
//! description: Detect hardcoded secrets in code
//! severity: error
//! trigger: PostToolUse
//! match:
//!   tools: [Write, Edit]
//!   files: ["*.ts", "*.js"]
//! tags: [secrets, blocking]
//! ---
//!
//! # No Secrets Validator
//!
//! Instructions for the validation agent to detect secrets...
//! ```
//!
//! # Directory Precedence
//!
//! Validators are loaded from multiple directories with precedence:
//! 1. Builtin validators (embedded in the binary) - lowest precedence
//! 2. User validators (~/<AVP_DIR>/validators)
//! 3. Project validators (./<AVP_DIR>/validators) - highest precedence
//!
//! Later sources override earlier ones with the same name.
//!
//! # Partial Support
//!
//! Validators support Liquid template partials via the unified [`ValidatorPartialAdapter`],
//! which follows the same pattern as prompts and rules. Use `{% include 'partial-name' %}`
//! in validator bodies to include shared content from the `_partials/` directory.
//!
//! # Example
//!
//! ```no_run
//! use avp_common::validator::{ValidatorLoader, MatchContext, Severity};
//! use avp_common::types::HookType;
//!
//! // Create a loader and load all validators
//! let mut loader = ValidatorLoader::new();
//! loader.load_all().unwrap();
//!
//! // Find validators matching a PreToolUse event for the Write tool
//! let ctx = MatchContext::new(HookType::PreToolUse).with_tool("Write");
//! let matching = loader.matching(&ctx);
//!
//! for validator in matching {
//!     println!("{}: {} ({})", validator.name(), validator.description(), validator.severity());
//! }
//! ```

pub mod executor;
pub mod loader;
pub mod parser;
pub mod runner;
pub mod types;

use std::sync::Arc;
use swissarmyhammer_templating::partials::LibraryPartialAdapter;

// Re-export main types for convenience
pub use executor::{
    add_partial_with_aliases, create_executed_ruleset, create_executed_validator,
    extract_partials_from_builtins, is_partial, is_rate_limit_error, log_ruleset_result,
    log_validator_result, parse_validator_response, render_validator_body, render_validator_prompt,
    render_validator_prompt_with_partials, render_validator_prompt_with_partials_and_changed_files,
    RulePromptContext, RuleSetSessionContext, ValidatorRenderContext, VALIDATOR_PROMPT_NAME,
};
pub use loader::{DirectoryInfo, ValidatorDiagnostics, ValidatorLoader};
pub use parser::{parse_rule, parse_ruleset_directory, parse_ruleset_manifest, parse_validator};
pub use runner::ValidatorRunner;
pub use types::{
    ExecutedRuleSet, ExecutedValidator, MatchContext, Rule, RuleFrontmatter, RuleResult, RuleSet,
    RuleSetManifest, RuleSetMetadata, Severity, Validator, ValidatorFrontmatter, ValidatorMatch,
    ValidatorResult, ValidatorSource,
};

/// Adapter that allows validators to be used as Liquid template partials.
///
/// This is a type alias for the generic [`LibraryPartialAdapter`] specialized
/// for [`ValidatorLoader`]. This follows the same unified pattern as
/// [`swissarmyhammer_prompts::PromptPartialAdapter`] and
/// [`swissarmyhammer_rules::RulePartialAdapter`].
///
/// The underlying loader implements [`swissarmyhammer_templating::partials::TemplateContentProvider`],
/// enabling validators to participate in the unified partial system.
pub type ValidatorPartialAdapter = LibraryPartialAdapter<ValidatorLoader>;

/// Create a new validator partial adapter from a loader Arc.
///
/// This is a convenience function that creates a `ValidatorPartialAdapter`
/// (which is a `LibraryPartialAdapter<ValidatorLoader>`).
pub fn new_validator_partial_adapter(loader: Arc<ValidatorLoader>) -> ValidatorPartialAdapter {
    LibraryPartialAdapter::new(loader)
}
