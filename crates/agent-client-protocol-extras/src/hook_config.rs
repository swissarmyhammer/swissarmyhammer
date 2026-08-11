//! Hook configuration — Claude-compatible declarative hook registration
//!
//! Matches Claude Code's 3-level config nesting:
//! 1. Event name (PascalCase) → array of matcher groups
//! 2. Matcher group → optional regex matcher + array of handlers
//! 3. Handler → command, prompt, or agent
//!
//! JSON example (Claude Code format):
//! ```json
//! {
//!   "hooks": {
//!     "PreToolUse": [
//!       {
//!         "matcher": "Bash",
//!         "hooks": [
//!           { "type": "command", "command": "./check.sh" }
//!         ]
//!       }
//!     ]
//!   }
//! }
//! ```
//!
//! YAML example:
//! ```yaml
//! hooks:
//!   PreToolUse:
//!     - matcher: "Bash"
//!       hooks:
//!         - type: command
//!           command: "./check.sh"
//! ```
//!
//! # How the code is divided
//!
//! The parts are in one file for each subject. The review engine renders a
//! whole file into one agent prompt, and it does not review a file that is
//! larger than the per-file prompt cap, so a module this size must be several
//! files and not one.
//!
//! - [`event`] — the lifecycle events, their kinds, and the Claude-compatible
//!   JSON each event sends to a command hook.
//! - [`decision`] — what a hook answers ([`HookDecision`]), the two traits that
//!   answer it, the matcher rules, and a registration.
//! - [`config`] — the deserializable config shapes, and the factory that makes
//!   registrations from them.
//! - [`output`] — the Claude-compatible output shapes a hook prints, and the
//!   rules that turn one into a [`HookDecision`].
//! - [`handlers`] — the two built-in handlers: a shell command, and an LLM
//!   evaluator.
//!
//! Every public type of this module is re-exported here, so a user names it as
//! `hook_config::<Type>` and never names the file it is in.

mod config;
mod decision;
mod event;
mod handlers;
mod output;

pub use config::{
    HookConfig, HookConfigError, HookEventKindConfig, HookHandlerConfig, MatcherGroup,
    UnsupportedEventKind,
};
pub use decision::{HookDecision, HookEvaluator, HookHandler, HookRegistration, Matcher};
pub use event::{HookCommandContext, HookEvent, HookEventKind, SessionSource};
pub use output::{
    HookDecisionValue, HookOutput, HookOutputBuilder, HookSpecificOutput, PromptHookResponse,
};

// `hookable_agent_from_config` lives in `crate::hookable_agent` from ACP 0.11
// onward, because the inner-agent argument type changed shape with the new
// SDK (no more `Arc<dyn Agent>`). It is re-exported through `lib.rs`.
