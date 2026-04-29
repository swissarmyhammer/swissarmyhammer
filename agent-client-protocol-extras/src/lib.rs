//! Shared fixture, hook, recording, and tracing infrastructure for ACP agents.
//!
//! In ACP 0.10 this crate exposed `Arc<dyn Agent>` wrappers built on the
//! now-removed `Agent` trait. ACP 0.11 replaces that trait with a Role +
//! Builder + handler model, and the wrappers in this crate are being ported
//! one-by-one to that model.
//!
//! This file currently exposes the **foundation** layer:
//!
//! - [`tracing_agent`] — the new builder-style [`TracingAgent`] middleware,
//!   plus the `trace_notifications` helper for human-readable session output.
//! - [`hook_config`] — declarative hook configuration types
//!   ([`HookConfig`], [`MatcherGroup`], [`HookHandlerConfig`], …) and the
//!   [`HookEvaluator`] trait.
//!
//! The `hookable_agent`, `recording`, `playback`, and `test_mcp_server`
//! modules are migrated by sibling tasks (A2 / A3 / A4 / D-series) and are
//! not yet wired in here. The fixture-recording entry points
//! (`with_fixture`, `AgentWithFixture`, `start_test_mcp_server_with_capture`)
//! are likewise rebuilt by those tasks.

pub mod hook_config;
pub mod tracing_agent;

pub use hook_config::{
    HookCommandContext, HookConfig, HookConfigError, HookDecision, HookDecisionValue,
    HookEvaluator, HookEvent, HookEventKind, HookEventKindConfig, HookHandler, HookHandlerConfig,
    HookOutput, HookRegistration, HookSpecificOutput, MatcherGroup, PromptHookResponse,
    SessionSource, UnsupportedEventKind,
};
pub use tracing_agent::{trace_notifications, TracingAgent};

// Re-export MCP notification types for convenience
pub use model_context_protocol_extras::{
    start_proxy, McpNotification, McpNotificationSource, McpProxy,
};
