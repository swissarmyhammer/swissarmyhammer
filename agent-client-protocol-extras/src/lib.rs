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
//! - [`playback`] — the [`PlaybackAgent`] leaf agent that replays a recorded
//!   JSON session over an ACP 0.11 connection (inverse of [`recording`]).
//! - [`fixture`] — the [`AgentWithFixture`] dyn-compatible facade plus the
//!   [`PlaybackAgentWithFixture`] / [`RecordingAgentWithFixture`] concrete
//!   wrappers used by the conformance suite.
//! - [`test_mcp_server`] — an in-process MCP test server with a notification
//!   capture proxy ([`start_test_mcp_server_with_capture`]) for use in the
//!   conformance fixture-recording flow.

pub mod fixture;
pub mod hook_config;
pub mod hookable_agent;
pub mod playback;
pub mod recording;
pub mod test_mcp_server;
pub mod tracing_agent;

pub use fixture::{
    get_fixture_path_for, get_test_name_from_thread, AgentWithFixture, PlaybackAgentWithFixture,
    RecordingAgentWithFixture,
};
pub use hook_config::{
    HookCommandContext, HookConfig, HookConfigError, HookDecision, HookDecisionValue,
    HookEvaluator, HookEvent, HookEventKind, HookEventKindConfig, HookHandler, HookHandlerConfig,
    HookOutput, HookRegistration, HookSpecificOutput, MatcherGroup, PromptHookResponse,
    SessionSource, UnsupportedEventKind,
};
pub use hookable_agent::{hookable_agent_from_config, HookableAgent};
pub use playback::PlaybackAgent;
pub use test_mcp_server::{start_test_mcp_server_with_capture, TestMcpServer};
// NOTE: The A3 task acceptance criteria mentioned `RecordedEvent` alongside
// `RecordedCall`/`RecordedSession`, but no `RecordedEvent` type ever existed
// — neither in 0.10 nor here. The on-disk schema has only ever been
// `RecordedSession { calls: Vec<RecordedCall> }`. The criterion was
// inaccurate; the public surface re-exported below is the complete public
// surface of the recording module, so future reviewers shouldn't go hunting
// for a phantom `RecordedEvent`.
pub use recording::{RecordedCall, RecordedSession, RecordingAgent};
pub use tracing_agent::{trace_notifications, TracingAgent};

// Re-export MCP notification types for convenience
pub use model_context_protocol_extras::{
    start_proxy, McpNotification, McpNotificationSource, McpProxy,
};
