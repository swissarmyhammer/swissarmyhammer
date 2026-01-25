//! # SwissArmyHammer Rules Domain Crate
//!
//! This crate provides rule management functionality for SwissArmyHammer,
//! including loading, filtering, and checking rules with template integration.
//!
//! ## Features
//!
//! - **Rule Management**: Load and organize rules from various sources
//! - **Template Integration**: Uses swissarmyhammer-templating for rendering
//! - **Filtering**: Advanced filtering capabilities for rule selection
//! - **Resolution**: Hierarchical rule loading with precedence rules

#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use swissarmyhammer_common::SwissArmyHammerError;

// Declare modules
mod cache;
mod checker;
mod error;
mod frontmatter;
mod glob;
pub mod ignore;
mod language;
mod rule_filter;
mod rule_loader;
mod rule_partial_adapter;
mod rule_resolver;
mod rules;
mod severity;
mod storage;

// Re-export public types
pub use cache::{CachedResult, RuleCache};
pub use checker::{AgentConfig, CheckMode, RuleCheckRequest, RuleChecker};
pub use error::{RuleError, RuleViolation};
pub use frontmatter::{parse_frontmatter, FrontmatterResult};
pub use glob::{expand_files_for_rules, DEFAULT_PATTERN};
pub use language::detect_language;
pub use rule_filter::RuleFilter;
pub use rule_loader::RuleLoader;
pub use rule_partial_adapter::{new_rule_partial_adapter, RulePartialAdapter};
pub use rule_resolver::RuleResolver;
pub use rules::{Rule, RuleBuilder, RuleLibrary};
pub use severity::Severity;
pub use storage::{FileStorage, MemoryStorage, StorageBackend};

/// Result type for rule operations
pub type Result<T> = std::result::Result<T, SwissArmyHammerError>;

/// Represents a rule source type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleSource {
    /// Built-in rules embedded in the binary
    Builtin,
    /// User rules from ~/.swissarmyhammer/rules
    User,
    /// Local rules from project .swissarmyhammer directories
    Local,
}

impl From<swissarmyhammer_common::FileSource> for RuleSource {
    fn from(source: swissarmyhammer_common::FileSource) -> Self {
        match source {
            swissarmyhammer_common::FileSource::Builtin => RuleSource::Builtin,
            swissarmyhammer_common::FileSource::User => RuleSource::User,
            swissarmyhammer_common::FileSource::Local => RuleSource::Local,
            // Dynamic rules are runtime-generated content that conceptually belongs to the user's
            // session rather than being persistent files. We map Dynamic to User because:
            // 1. Dynamic content has user-level privileges (not builtin/system level)
            // 2. The rules domain doesn't need to distinguish between persistent and ephemeral user rules
            // 3. This matches the pattern used in swissarmyhammer-prompts for consistency
            swissarmyhammer_common::FileSource::Dynamic => RuleSource::User,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_source_from_file_source() {
        assert_eq!(
            RuleSource::from(swissarmyhammer_common::FileSource::Builtin),
            RuleSource::Builtin
        );
        assert_eq!(
            RuleSource::from(swissarmyhammer_common::FileSource::User),
            RuleSource::User
        );
        assert_eq!(
            RuleSource::from(swissarmyhammer_common::FileSource::Local),
            RuleSource::Local
        );
        assert_eq!(
            RuleSource::from(swissarmyhammer_common::FileSource::Dynamic),
            RuleSource::User
        );
    }
}
