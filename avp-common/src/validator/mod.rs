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
//! 2. User validators (~/.avp/validators)
//! 3. Project validators (./.avp/validators) - highest precedence
//!
//! Later sources override earlier ones with the same name.
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

// Re-export main types for convenience
pub use executor::{
    create_executed_validator, parse_validator_response, render_validator_prompt,
    VALIDATOR_PROMPT_NAME,
};
pub use loader::ValidatorLoader;
pub use parser::parse_validator;
pub use runner::ValidatorRunner;
pub use types::{
    ExecutedValidator, MatchContext, Severity, Validator, ValidatorFrontmatter, ValidatorMatch,
    ValidatorResult, ValidatorSource,
};
