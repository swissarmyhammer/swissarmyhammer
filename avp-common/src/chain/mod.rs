//! Chain of Responsibility implementation for hook processing.

mod context;
mod executor;
mod factory;
mod link;
pub mod links;
mod output;
mod starters;

/// Exit code returned when validators block execution.
///
/// This code (2) indicates that a validator has blocked the operation,
/// distinct from success (0) and general failure (1).
pub const VALIDATOR_BLOCK_EXIT_CODE: i32 = 2;
pub use context::ChainContext;
pub use executor::{Chain, ChainBuilder};
pub use factory::ChainFactory;
pub use link::{
    ChainLink, ChainResult, ContextLink, HookInputType, PassThroughLink, ValidationLink,
};
pub use links::{
    load_changed_files, load_changed_files_as_strings, PostToolUseFileTracker,
    PreToolUseFileTracker, SessionEndCleanup, SessionStartCleanup, StopCleanup,
    ValidatorExecutorLink, ValidatorMatchInfo,
};
pub use output::{ChainOutput, ChainOutputAggregator, LinkOutput, ValidatorBlockInfo};
pub use starters::{
    BlockingErrorStarter, ChainStarter, ConditionalStarter, StarterResult, SuccessStarter,
    ValidatorContextStarter,
};
