//! # Llama Agent
//!
//! A high-performance, async Rust agent framework for LLaMA models with comprehensive
//! MCP (Model Context Protocol) support, session management, and text embedding capabilities.
//!
//! ## Features
//!
//! ### MCP Server Integration
//! - **Dual Server Types**: Support for both in-process and HTTP-based MCP servers
//! - **Automatic Lifecycle Management**: Process spawning, monitoring, and cleanup
//! - **HTTP Transport**: Server-Sent Events (SSE) for streaming communication
//! - **Validation & Error Handling**: Comprehensive configuration validation
//! - **rmcp Integration**: Direct access to rmcp transport types for advanced use cases
//!
//! ### Session Management
//! - **Intelligent Compaction**: AI-powered conversation summarization
//! - **Context Preservation**: Maintain recent message context during compaction
//! - **Custom Prompts**: Domain-specific summarization strategies
//! - **Auto-Compaction**: Automatic session management during generation
//!
//! ### Text Generation & Embedding
//! - **Streaming Generation**: Async streaming with configurable stopping criteria
//! - **Batch Embedding**: High-throughput text embedding with Parquet output
//! - **Shared Model Cache**: Efficient resource sharing between operations
//! - **Flexible Model Loading**: HuggingFace Hub and local model support
//!
//! ## Quick Start
//!
//! ### Basic Agent with MCP Server
//! ```rust
//! use llama_agent::{Agent, AgentConfig, MCPServerConfig, ProcessServerConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure an in-process Python MCP server
//!     let mcp_config = MCPServerConfig::InProcess(ProcessServerConfig {
//!         name: "filesystem".to_string(),
//!         command: "python".to_string(),
//!         args: vec!["-m".to_string(), "mcp_server.filesystem".to_string()],
//!         timeout_secs: Some(30),
//!     });
//!
//!     // Create agent with MCP server
//!     let config = AgentConfig {
//!         mcp_servers: vec![mcp_config],
//!         ..Default::default()
//!     };
//!     
//!     let mut agent = Agent::new(config).await?;
//!     
//!     // Generate response with tool access
//!     let response = agent.generate("List files in the current directory").await?;
//!     println!("{}", response);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### HTTP MCP Server
//! ```rust
//! use llama_agent::{MCPServerConfig, HttpServerConfig};
//!
//! let mcp_config = MCPServerConfig::Http(HttpServerConfig {
//!     name: "web-search".to_string(),
//!     url: "https://mcp-server.example.com/sse".to_string(),
//!     timeout_secs: Some(60),
//!     sse_keep_alive_secs: Some(30),
//!     stateful_mode: true,
//! });
//! ```
//!
//! ### Session Compaction
//! ```rust
//! use llama_agent::{CompactionConfig, CompactionPrompt};
//!
//! let config = CompactionConfig {
//!     threshold: 0.8,
//!     context_limit: 4096,
//!     preserve_recent: 2,
//!     custom_prompt: Some(CompactionPrompt::custom(
//!         "Focus on technical details and code examples",
//!         "Summarize: {conversation_history}"
//!     )?),
//! };
//! ```
//!
//! ## Migration from Previous Versions
//!
//! ### MCP Configuration Changes
//! The MCP configuration has changed from struct-based to enum-based:
//!
//! ```rust
//! // Before (no longer supported)
//! // let config = MCPServerConfig {
//! //     name: "server".to_string(),
//! //     command: "python".to_string(),
//! //     args: vec!["-m".to_string(), "server".to_string()],
//! //     timeout_secs: Some(30),
//! // };
//!
//! // After (current)
//! let config = MCPServerConfig::InProcess(ProcessServerConfig {
//!     name: "server".to_string(),
//!     command: "python".to_string(),
//!     args: vec!["-m".to_string(), "server".to_string()],
//!     timeout_secs: Some(30),
//! });
//! ```
//!
//! ## Architecture
//!
//! ### Core Components
//! - **Agent**: Main orchestration layer for generation and tool integration
//! - **MCP Client**: Manages communication with MCP servers
//! - **Session Manager**: Handles conversation state and compaction
//! - **Model Loader**: Shared model caching and loading infrastructure
//! - **Validation**: Configuration validation and error handling
//!
//! ### MCP Server Types
//!
//! #### In-Process Servers (`ProcessServerConfig`)
//! - **Best for**: Local development, simple deployments, trusted code
//! - **Communication**: stdin/stdout JSON-RPC
//! - **Lifecycle**: Managed by agent (automatic restart on failure)
//! - **Latency**: Very low (no network overhead)
//! - **Isolation**: Process-level only
//!
//! #### HTTP Servers (`HttpServerConfig`)
//! - **Best for**: Production, microservices, distributed systems
//! - **Communication**: Server-Sent Events over HTTP/HTTPS  
//! - **Lifecycle**: Independent (server manages own lifecycle)
//! - **Latency**: Network-dependent
//! - **Isolation**: Full network isolation
//!
//! ## Performance Characteristics
//!
//! - **Generation**: High-performance streaming with configurable stopping
//! - **MCP Operations**: Sub-100ms for in-process, network-dependent for HTTP
//! - **Session Compaction**: 100-500ms for small sessions, scales with size
//! - **Embedding**: 20-50 texts/second (hardware and model dependent)
//!
//! ## Error Handling
//!
//! The crate provides comprehensive error types for different failure scenarios:
//! - **Configuration Validation**: Early detection of invalid configurations
//! - **Network Failures**: HTTP timeout, connection, and transport errors
//! - **Process Management**: Server startup, communication, and lifecycle errors
//! - **Model Operations**: Loading, generation, and embedding errors

pub mod agent;
pub mod chat_template;
pub mod dependency_analysis;
pub mod echo;
pub mod echo_real_tests;
pub mod generation;
pub mod mcp;
pub mod mcp_unified_test;
pub mod model;
pub mod queue;
pub mod resources;
pub mod session;
pub mod stopper;
pub mod storage;
pub mod template_cache;
pub mod types;
pub mod validation;

#[cfg(feature = "acp")]
pub mod acp;

#[cfg(any(test, feature = "test-utils"))]
pub mod tests;

#[cfg(test)]
mod error_consistency_tests;

// Re-export commonly used types
pub use types::*;

// Re-export main agent functionality
pub use agent::AgentServer;

// Re-export MCP functionality
pub use mcp::{HealthStatus as MCPHealthStatus, MCPClient, RetryConfig};

// Re-export new unified MCP client functionality
pub use mcp::{MCPClientBuilder, MCPClientError, ServerConnectionConfig, UnifiedMCPClient};

// Re-export validation functionality
pub use validation::{ValidationError, Validator};

// Re-export stopper functionality
pub use stopper::{EosStopper, MaxTokensStopper, Stopper};

// Re-export resource functionality
pub use resources::{ResourceError, ResourceLoader};

// Re-export generation functionality
pub use generation::{GenerationConfig, GenerationError, LlamaCppGenerator, TextGenerator};

// Re-export session management and compaction functionality
pub use session::{
    CompactionResult, CompactionStats, CompactionSummary, SessionManager, SessionStats,
};

// Re-export storage functionality for session persistence
pub use storage::{FileSessionStorage, SessionStorage};

// Re-export template cache functionality
pub use template_cache::{CacheStats, TemplateCache, TemplateCacheEntry, TemplateCacheError};

/// Re-export rmcp transport configuration for direct integration with rmcp.
///
/// This exposes rmcp's StreamableHttpServerConfig in our public API, allowing
/// advanced users to work with rmcp types directly and eliminating configuration
/// duplication. Use this when you need fine-grained control over HTTP transport
/// settings or when integrating with existing rmcp-based infrastructure.
pub use rmcp::transport::StreamableHttpServerConfig;

// Re-export ACP functionality when feature enabled
#[cfg(feature = "acp")]
pub use acp::{
    AcpCapabilities, AcpConfig, AcpServer, AcpSessionState, FilesystemSettings, PermissionPolicy,
    PermissionStorage, SessionMode, TerminalSettings,
};
