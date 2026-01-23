//! Strategy pattern implementation for hook processing.

mod claude_code;
mod dispatcher;
mod traits;

pub use claude_code::ClaudeCodeHookStrategy;
pub use dispatcher::HookDispatcher;
pub use traits::{AgentHookStrategy, TypedHookStrategy};
