//! Shared fixture, hook, recording, and tracing infrastructure for ACP agents.
//!
//! In ACP 0.10 this crate exposed `Arc<dyn Agent>` wrappers built on the
//! now-removed `Agent` trait. ACP 0.11 replaces that trait with a Role +
//! Builder + handler model, and the wrappers in this crate are being ported
//! one-by-one to that model.
//!
//! This file currently exposes the **foundation** layer plus the
//! `RecordingAgent` middleware (task A3) and the `HookableAgent`
//! middleware (task A2):
//!
//! - [`tracing_agent`] — the new builder-style [`TracingAgent`] middleware,
//!   plus the `trace_notifications` helper for human-readable session output.
//! - [`hook_config`] — declarative hook configuration types
//!   ([`HookConfig`], [`MatcherGroup`], [`HookHandlerConfig`], …) and the
//!   [`HookEvaluator`] trait.
//! - [`hookable_agent`] — the [`HookableAgent`] middleware that fires
//!   lifecycle hooks (`SessionStart`, `UserPromptSubmit`, `PreToolUse`,
//!   `PostToolUse`, `Stop`, `Notification`, …) and applies their
//!   `HookDecision` outputs.
//! - [`recording`] — the [`RecordingAgent`] middleware that captures every
//!   request/response/notification flowing through to a stable on-disk
//!   JSON file for later replay.
//!
//! The `playback` and `test_mcp_server` modules are migrated by sibling
//! tasks (A4 / D-series) and are not yet wired in here. The
//! fixture-recording entry points (`with_fixture`, `AgentWithFixture`,
//! `start_test_mcp_server_with_capture`) are likewise rebuilt by those
//! tasks.

pub mod hook_config;
pub mod hookable_agent;
pub mod recording;
pub mod tracing_agent;

pub use hook_config::{
    HookCommandContext, HookConfig, HookConfigError, HookDecision, HookDecisionValue,
    HookEvaluator, HookEvent, HookEventKind, HookEventKindConfig, HookHandler, HookHandlerConfig,
    HookOutput, HookRegistration, HookSpecificOutput, MatcherGroup, PromptHookResponse,
    SessionSource, UnsupportedEventKind,
};
pub use hookable_agent::HookableAgent;
pub use recording::{RecordedCall, RecordedSession, RecordingAgent};
pub use tracing_agent::{trace_notifications, TracingAgent};

// Re-export MCP notification types for convenience
pub use model_context_protocol_extras::{
    start_proxy, McpNotification, McpNotificationSource, McpProxy,
};
