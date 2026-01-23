//! Chain of Responsibility implementation for hook processing.

mod aggregator;
mod context;
mod executor;
mod link;
mod starters;

pub use aggregator::ChainAggregator;
pub use context::ChainContext;
pub use executor::{Chain, ChainBuilder};
pub use link::{
    ChainLink, ChainResult, ContextLink, HookInputType, PassThroughLink, ValidationLink,
};
pub use starters::{
    BlockingErrorStarter, ChainStarter, ConditionalStarter, StarterResult, SuccessStarter,
};
