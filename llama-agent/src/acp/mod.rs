//! Agent Client Protocol (ACP) integration for llama-agent
//!
//! This module provides a complete implementation of the Agent Client Protocol,
//! enabling llama-agent to integrate with ACP-compatible code editors like Zed
//! and JetBrains IDEs.
//!
//! # Overview
//!
//! The ACP module bridges the gap between editor clients and the llama-agent inference
//! engine, providing:
//!
//! - **JSON-RPC 2.0 Protocol**: Standard protocol for client-server communication
//! - **Session Management**: Create, load, and manage conversation sessions
//! - **Streaming Responses**: Real-time token streaming with low latency
//! - **Permission System**: Fine-grained control over file and terminal operations
//! - **Filesystem Operations**: Secure file read/write with path validation
//! - **Terminal Execution**: Controlled command execution with output capture
//! - **Agent Plans**: Structured task tracking and progress reporting
//! - **Session Modes**: Switch between Code, Plan, and Test modes
//! - **Slash Commands**: Editor-exposed workflow integration
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │    ACP Client (Editor)                  │
//! │    - Zed, JetBrains, etc.               │
//! └──────────────────┬──────────────────────┘
//!                    │ JSON-RPC 2.0 over stdio
//!                    │
//! ┌──────────────────▼──────────────────────┐
//! │    acp::server::AcpServer               │
//! │    - Protocol handling                  │
//! │    - Session coordination               │
//! │    - Notification routing               │
//! └──────────┬─────────────┬────────────────┘
//!            │             │
//!            │             └──────────────────┐
//!            │                                │
//! ┌──────────▼────────────┐    ┌─────────────▼──────────┐
//! │  acp::translation     │    │  acp::permissions      │
//! │  - Type conversion    │    │  - Policy evaluation   │
//! │  - ACP ↔ llama types  │    │  - Permission storage  │
//! └───────────────────────┘    └────────────────────────┘
//!            │
//! ┌──────────▼────────────────────────────────┐
//! │    Core llama-agent                       │
//! │    - LLaMA inference via llama.cpp        │
//! │    - Session management and persistence   │
//! │    - MCP client for tool calls            │
//! └───────────────────────────────────────────┘
//! ```
//!
//! # Key Components
//!
//! ## Server ([`server::AcpServer`])
//!
//! The main entry point for ACP integration. Handles:
//! - Protocol initialization and capability negotiation
//! - Session lifecycle management
//! - Request routing and response handling
//! - Streaming notifications to clients
//!
//! ## Configuration ([`config::AcpConfig`])
//!
//! Defines server behavior including:
//! - Protocol version advertisement
//! - Capability flags (session loading, modes, terminal, filesystem)
//! - Permission policies (AlwaysAsk, AutoApproveReads, RuleBased)
//! - Filesystem security settings (allowed/blocked paths, size limits)
//! - Terminal resource limits
//!
//! ## Permissions ([`permissions`])
//!
//! Implements a flexible permission system:
//! - **AlwaysAsk**: Request user approval for every operation
//! - **AutoApproveReads**: Automatically approve read operations
//! - **RuleBased**: Define custom rules with pattern matching
//!
//! Supports per-session permission storage and policy evaluation.
//!
//! ## Session State ([`session::AcpSessionState`])
//!
//! Tracks state for each ACP session:
//! - Session identifiers (ACP and llama mappings)
//! - Current mode (Code, Plan, Test, Custom)
//! - Client capabilities for feature gating
//! - Permission storage for the session
//! - Available slash commands
//!
//! ## Translation ([`translation`])
//!
//! Bidirectional type conversion between:
//! - ACP protocol types (ContentBlock, SessionUpdate, etc.)
//! - llama-agent internal types (Message, ToolCall, ToolResult)
//!
//! Ensures seamless communication across the protocol boundary.
//!
//! ## Filesystem Operations ([`filesystem::FilesystemOperations`])
//!
//! Secure file operations with:
//! - Path validation against allowed/blocked lists
//! - File size limit enforcement
//! - Absolute path requirements
//! - Permission checking integration
//!
//! ## Terminal Management ([`terminal::TerminalManager`])
//!
//! Process execution and management:
//! - Command execution with working directory control
//! - Environment variable injection
//! - Output buffering and streaming
//! - Process lifecycle tracking (create, output, wait, kill)
//! - Resource limit enforcement
//!
//! ## Error Handling ([`error`])
//!
//! Comprehensive error types with JSON-RPC 2.0 mapping:
//! - [`ServerError`]: Protocol and server errors
//! - [`SessionError`]: Session lifecycle errors
//! - [`PermissionError`]: Permission denial errors
//! - [`ConfigError`]: Configuration validation errors
//! - [`TerminalError`]: Process execution errors
//!
//! All errors implement [`ToJsonRpcError`] for protocol compliance.
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use llama_agent::acp::{AcpServer, AcpConfig};
//! use llama_agent::AgentServer;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load ACP configuration
//!     let config = AcpConfig::from_file("acp-config.yaml")?;
//!     
//!     // Create underlying llama-agent server
//!     let agent_server = Arc::new(AgentServer::new(/* ... */).await?);
//!     
//!     // Create ACP server
//!     let acp_server = AcpServer::new(agent_server, config);
//!     
//!     // Start JSON-RPC server on stdio
//!     acp_server.start_stdio().await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! # Configuration Example
//!
//! ```yaml
//! protocolVersion: "0.1.0"
//!
//! capabilities:
//!   supportsSessionLoading: true
//!   supportsModes: true
//!   supportsPlans: true
//!   supportsSlashCommands: true
//!   terminal: true
//!   filesystem:
//!     readTextFile: true
//!     writeTextFile: true
//!
//! permissionPolicy: alwaysAsk
//!
//! filesystem:
//!   allowedPaths:
//!     - /home/user/projects
//!   blockedPaths:
//!     - /home/user/.ssh
//!     - /home/user/.aws
//!   maxFileSize: 10485760  # 10MB
//!
//! terminal:
//!   outputBufferBytes: 1048576
//!   gracefulShutdownTimeout: 5  # seconds
//! ```
//!
//! # Session Loading
//!
//! The ACP module supports loading previous sessions with full conversation history.
//! When a client requests session loading:
//!
//! 1. Historical messages are loaded from persistent storage
//! 2. Messages are streamed chronologically via `session/update` notifications
//! 3. All message types are included (user, assistant, tool calls, tool results)
//! 4. The load completes only after the entire history is replayed
//!
//! This enables editors to progressively reconstruct conversation state and provide
//! immediate visual feedback to users.
//!
//! # Security Considerations
//!
//! The ACP module implements multiple security layers:
//!
//! - **Path Validation**: All filesystem operations validate paths against allowed/blocked lists
//! - **Size Limits**: File operations enforce maximum size constraints
//! - **Permission Checks**: Configurable policies control operation approval
//! - **Resource Limits**: Terminal operations are bounded by concurrent process limits
//! - **Absolute Paths**: Relative paths are rejected to prevent traversal attacks
//!
//! For production use, the recommended configuration is:
//! - `permission_policy: AlwaysAsk`
//! - Restrictive `allowed_paths` whitelist
//! - Comprehensive `blocked_paths` for sensitive directories
//! - Conservative `max_file_size_bytes` limit
//!
//! # Protocol Compliance
//!
//! This module implements the Agent Client Protocol specification, ensuring
//! compatibility with any ACP-compliant editor client. The implementation follows
//! JSON-RPC 2.0 standards and handles:
//!
//! - Protocol negotiation and capability advertisement
//! - Request/response pairing with proper ID tracking
//! - Notification streaming for async updates
//! - Error reporting with standard error codes
//! - Graceful shutdown coordination
//!
//! # Feature Flag
//!
//! The ACP module is gated behind the `acp` feature flag to keep dependencies
//! optional for users who only need core llama-agent functionality:
//!
//! ```toml
//! [dependencies]
//! llama-agent = { version = "0.1", features = ["acp"] }
//! ```
//!
//! # Testing
//!
//! Comprehensive test coverage is provided through:
//! - Unit tests for individual components
//! - Integration tests for protocol flows
//! - Mock clients for end-to-end validation
//!
//! Run tests with:
//! ```bash
//! cargo test --features acp --test acp_integration
//! ```
//!
//! # Related Documentation
//!
//! - [Agent Client Protocol Specification](https://agentclientprotocol.com)
//! - [llama-agent README](../../README.md)
//! - [swissarmyhammer Documentation](../../../README.md)

pub mod commands;
pub mod config;
pub mod error;
pub mod filesystem;
pub mod mcp_client_factory;
pub mod permissions;
pub mod plan;
pub mod raw_message_manager;
pub mod server;
pub mod session;
pub mod terminal;
pub mod translation;

// Test utilities are available in both test and non-test builds
// to support integration tests that need to import them
pub mod test_utils;

pub use config::{
    AcpCapabilities, AcpConfig, FilesystemSettings, GracefulShutdownTimeout, TerminalSettings,
};
pub use error::{ConfigError, PermissionError, ServerError, SessionError, TerminalError};
pub use permissions::{
    PermissionAction, PermissionEvaluation, PermissionPolicy, PermissionPolicyEngine,
    PermissionRule, PermissionStorage, ToolPattern,
};
pub use raw_message_manager::RawMessageManager;
pub use server::AcpServer;
pub use session::{AcpSessionState, SessionMode};
