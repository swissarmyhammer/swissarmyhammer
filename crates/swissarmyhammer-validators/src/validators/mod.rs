//! Validator module — the rules-as-data loader plus the shared agent pool.
//!
//! This module provides the data model, parsing, and loading for AVP
//! validators, and the hook-free [`AgentPool`] that controls parallelism for
//! the review pipeline. Validators are markdown files with YAML frontmatter
//! that specify validation rules.
//!
//! # Validator Format
//!
//! ```markdown
//! ---
//! name: no-secrets
//! description: Detect hardcoded secrets in code
//! severity: error
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
//! 2. User validators (~/.validators)
//! 3. Project validators (./.validators) - highest precedence
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
//! use swissarmyhammer_validators::validators::{ValidatorLoader, MatchContext};
//!
//! // Create a loader and load all validators
//! let mut loader = ValidatorLoader::new();
//! loader.load_all().unwrap();
//!
//! // Find validators matching a Write of a TypeScript file
//! let ctx = MatchContext::new().with_tool("Write").with_file("app.ts");
//! let matching = loader.matching(&ctx);
//!
//! for validator in matching {
//!     println!("{}: {} ({})", validator.name(), validator.description(), validator.severity());
//! }
//! ```

pub mod loader;
pub mod parser;
pub mod pool;
pub mod types;

use std::sync::Arc;
use swissarmyhammer_templating::partials::LibraryPartialAdapter;

// Re-export main types for convenience
pub use loader::{DirectoryInfo, ValidatorDiagnostics, ValidatorLoader};
pub use parser::{
    check_manifest_frontmatter, parse_rule, parse_ruleset_directory, parse_ruleset_manifest,
    parse_validator,
};
pub use pool::{AgentPool, PoolConfig, PromptResult, DEFAULT_MAX_TOKENS};
pub use types::{
    compile_glob_patterns, matches_any_pattern, ExecutedRuleSet, ExecutedValidator, MatchContext,
    Rule, RuleFrontmatter, RuleResult, RuleSet, RuleSetManifest, RuleSetMetadata, Severity,
    Validator, ValidatorFrontmatter, ValidatorMatch, ValidatorResult, ValidatorSource,
    GLOB_MATCH_OPTIONS,
};

/// Adapter that allows validators to be used as Liquid template partials.
///
/// This is a type alias for the generic [`LibraryPartialAdapter`] specialized
/// for [`ValidatorLoader`]. This follows the same unified pattern as
/// [`swissarmyhammer_prompts::PromptPartialAdapter`].
///
/// The underlying loader implements [`swissarmyhammer_templating::partials::TemplateContentProvider`],
/// enabling validators to participate in the unified partial system.
pub type ValidatorPartialAdapter = LibraryPartialAdapter<ValidatorLoader>;

/// Create a new validator partial adapter from a loader Arc.
pub fn new_validator_partial_adapter(loader: Arc<ValidatorLoader>) -> ValidatorPartialAdapter {
    LibraryPartialAdapter::new(loader)
}
