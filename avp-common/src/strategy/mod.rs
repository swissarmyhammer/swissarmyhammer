//! Strategy pattern implementation for hook processing.

pub mod claude;
mod dispatcher;
mod traits;

pub use claude::strategy::ClaudeCodeHookStrategy;
pub use claude::ClaudeHookOutput;
pub use dispatcher::HookDispatcher;
pub use traits::{AgentHookStrategy, TypedHookStrategy};
