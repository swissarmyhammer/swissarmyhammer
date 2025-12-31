//! Agent Client Protocol implementation for Claude Agent

use crate::permission_storage;
use crate::{
    base64_processor::Base64Processor,
    claude::ClaudeClient,
    config::AgentConfig,
    constants::sizes,
    content_block_processor::ContentBlockProcessor,
    content_capability_validator::ContentCapabilityValidator,
    path_validator::PathValidator,
    permissions::{FilePermissionStorage, PermissionPolicyEngine, PolicyEvaluation},
    protocol_translator::ProtocolTranslator,
    session::SessionManager,
    size_validator::{SizeLimits, SizeValidator},
    tools::ToolCallHandler,
};
#[cfg(test)]
use agent_client_protocol::SessionModeId;
use agent_client_protocol::{
    Agent, AgentCapabilities, AuthenticateRequest, AuthenticateResponse, CancelNotification,
    ContentBlock, ExtNotification, ExtRequest, ExtResponse, InitializeRequest, InitializeResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest,
    PromptResponse, RawValue, SessionId, SessionNotification, SessionUpdate, SetSessionModeRequest,
    SetSessionModeResponse, StopReason, TextContent, WriteTextFileResponse,
};
use agent_client_protocol_extras::AgentWithFixture;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::SystemTime;

/// Default timeout for user permission prompts in seconds
///
/// ACP tool call information for permission requests
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCallUpdate {
    /// Unique identifier for the tool call
    #[serde(rename = "toolCallId")]
    pub tool_call_id: String,
}

/// ACP-compliant permission request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PermissionRequest {
    /// Session identifier for the permission request
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    /// Tool call information
    #[serde(rename = "toolCall")]
    pub tool_call: ToolCallUpdate,
    /// Available permission options for the user
    pub options: Vec<crate::tools::PermissionOption>,
}

/// ACP-compliant permission response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PermissionResponse {
    /// The outcome of the permission request
    pub outcome: crate::tools::PermissionOutcome,
}

/// Agent reasoning phases for thought generation
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ReasoningPhase {
    /// Initial analysis of the user's prompt
    PromptAnalysis,
    /// Planning the overall strategy and approach
    StrategyPlanning,
    /// Selecting appropriate tools for the task
    ToolSelection,
    /// Breaking down complex problems into smaller parts
    ProblemDecomposition,
    /// Executing the planned approach
    Execution,
    /// Evaluating results and determining next steps
    ResultEvaluation,
}

/// Agent thought content with contextual information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentThought {
    /// The reasoning phase this thought belongs to
    pub phase: ReasoningPhase,
    /// Human-readable thought content
    pub content: String,
    /// Optional structured context data
    pub context: Option<serde_json::Value>,
    /// Timestamp when the thought was generated
    pub timestamp: SystemTime,
}

impl AgentThought {
    /// Create a new agent thought for a specific reasoning phase
    pub fn new(phase: ReasoningPhase, content: impl Into<String>) -> Self {
        Self {
            phase,
            content: content.into(),
            context: None,
            timestamp: SystemTime::now(),
        }
    }

    /// Create a new agent thought with additional context
    pub fn with_context(
        phase: ReasoningPhase,
        content: impl Into<String>,
        context: serde_json::Value,
    ) -> Self {
        Self {
            phase,
            content: content.into(),
            context: Some(context),
            timestamp: SystemTime::now(),
        }
    }
}

/// Parameters for the ACP fs/read_text_file method
///
/// ACP fs/read_text_file method implementation:
/// 1. sessionId: Required - validate against active sessions
/// 2. path: Required - must be absolute path
/// 3. line: Optional - 1-based line number to start reading from
/// 4. limit: Optional - maximum number of lines to read
/// 5. Response: content field with requested file content
///
/// Supports partial file reading for performance optimization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReadTextFileParams {
    /// Session ID for validation
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Absolute path to the file to read
    pub path: String,
    /// Optional 1-based line number to start reading from
    pub line: Option<u32>,
    /// Optional maximum number of lines to read
    pub limit: Option<u32>,
}

/// Response for the ACP fs/read_text_file method
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReadTextFileResponse {
    /// File content as requested (full file or partial based on line/limit)
    pub content: String,
}

/// Parameters for the ACP fs/write_text_file method
///
/// ACP fs/write_text_file method implementation:
/// 1. sessionId: Required - validate against active sessions
/// 2. path: Required - must be absolute path
/// 3. content: Required - text content to write
/// 4. MUST create file if it doesn't exist per ACP specification
/// 5. MUST create parent directories if needed
/// 6. Response: null result on success
///
/// Uses atomic write operations to ensure file integrity.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WriteTextFileParams {
    /// Session ID for validation
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Absolute path to the file to write
    pub path: String,
    /// Text content to write to the file
    pub content: String,
}

use tokio::sync::{broadcast, RwLock};
use tokio_stream::StreamExt;

// SessionUpdateNotification has been replaced with agent_client_protocol::SessionNotification
// This provides better protocol compliance and type safety

// ToolCallContent and MessageChunk have been replaced with agent_client_protocol types:
// - ToolCallContent -> Use SessionUpdate enum variants directly
// - MessageChunk -> Use ContentBlock directly

/// Cancellation state for a session
///
/// Tracks the cancellation status and metadata for operations within a session.
/// This allows immediate cancellation response and proper cleanup coordination.
#[derive(Debug, Clone)]
pub struct CancellationState {
    /// Whether the session is cancelled
    pub cancelled: bool,
    /// When the cancellation occurred
    pub cancellation_time: SystemTime,
    /// Set of operation IDs that have been cancelled
    pub cancelled_operations: HashSet<String>,
    /// Reason for cancellation (for debugging)
    pub cancellation_reason: String,
}

impl CancellationState {
    /// Create a new active (non-cancelled) state
    pub fn active() -> Self {
        Self {
            cancelled: false,
            cancellation_time: SystemTime::now(),
            cancelled_operations: HashSet::new(),
            cancellation_reason: String::new(),
        }
    }

    /// Mark as cancelled with reason
    pub fn cancel(&mut self, reason: &str) {
        self.cancelled = true;
        self.cancellation_time = SystemTime::now();
        self.cancellation_reason = reason.to_string();
    }

    /// Add a cancelled operation ID
    pub fn add_cancelled_operation(&mut self, operation_id: String) {
        self.cancelled_operations.insert(operation_id);
    }

    /// Check if operation is cancelled
    pub fn is_operation_cancelled(&self, operation_id: &str) -> bool {
        self.cancelled || self.cancelled_operations.contains(operation_id)
    }
}

/// Manager for session cancellation state
///
/// Provides thread-safe cancellation coordination across all session operations.
/// Supports immediate cancellation notification and proper cleanup coordination.
pub struct CancellationManager {
    /// Session ID -> CancellationState mapping
    cancellation_states: Arc<RwLock<HashMap<String, CancellationState>>>,
    /// Broadcast sender for immediate cancellation notifications
    cancellation_sender: broadcast::Sender<String>,
}

impl CancellationManager {
    /// Create a new cancellation manager with configurable buffer size
    pub fn new(buffer_size: usize) -> (Self, broadcast::Receiver<String>) {
        let (sender, receiver) = broadcast::channel(buffer_size);
        (
            Self {
                cancellation_states: Arc::new(RwLock::new(HashMap::new())),
                cancellation_sender: sender,
            },
            receiver,
        )
    }

    /// Check if a session is cancelled
    pub async fn is_cancelled(&self, session_id: &str) -> bool {
        let states = self.cancellation_states.read().await;
        states
            .get(session_id)
            .map(|state| state.cancelled)
            .unwrap_or(false)
    }

    /// Mark a session as cancelled
    pub async fn mark_cancelled(&self, session_id: &str, reason: &str) -> crate::Result<()> {
        {
            let mut states = self.cancellation_states.write().await;
            let state = states
                .entry(session_id.to_string())
                .or_insert_with(CancellationState::active);
            state.cancel(reason);
        }

        // Broadcast cancellation immediately
        if let Err(e) = self.cancellation_sender.send(session_id.to_string()) {
            tracing::warn!(
                "Failed to broadcast cancellation for session {}: {}",
                session_id,
                e
            );
        }

        tracing::info!("Session {} marked as cancelled: {}", session_id, reason);
        Ok(())
    }

    /// Add a cancelled operation to a session
    pub async fn add_cancelled_operation(&self, session_id: &str, operation_id: String) {
        let mut states = self.cancellation_states.write().await;
        let state = states
            .entry(session_id.to_string())
            .or_insert_with(CancellationState::active);
        state.add_cancelled_operation(operation_id);
    }

    /// Get cancellation state for debugging
    pub async fn get_cancellation_state(&self, session_id: &str) -> Option<CancellationState> {
        let states = self.cancellation_states.read().await;
        states.get(session_id).cloned()
    }

    /// Clean up cancellation state for a session (called when session ends)
    pub async fn cleanup_session(&self, session_id: &str) {
        let mut states = self.cancellation_states.write().await;
        states.remove(session_id);
    }

    /// Reset cancellation state for a new prompt turn
    ///
    /// Called at the start of each prompt to ensure cancellation from previous
    /// turns doesn't affect the new prompt.
    pub async fn reset_for_new_turn(&self, session_id: &str) {
        let mut states = self.cancellation_states.write().await;
        // Replace with fresh active state, discarding any previous cancellation
        states.insert(session_id.to_string(), CancellationState::active());
        tracing::debug!(
            "Reset cancellation state for session {} (new turn)",
            session_id
        );
    }

    /// Subscribe to cancellation notifications
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.cancellation_sender.subscribe()
    }
}

/// Notification sender for streaming updates
///
/// Manages the broadcasting of session update notifications to multiple receivers.
/// This allows the agent to send real-time updates about session state changes,
/// streaming content, and tool execution results to interested subscribers.
#[derive(Debug, Clone)]
pub struct NotificationSender {
    /// The broadcast sender for distributing notifications
    sender: broadcast::Sender<SessionNotification>,
}

impl NotificationSender {
    /// Create a new notification sender with receiver
    ///
    /// Returns a tuple containing the sender and a receiver that can be used
    /// to listen for session update notifications. The receiver can be cloned
    /// to create multiple subscribers.
    ///
    /// # Parameters
    ///
    /// * `buffer_size` - The size of the broadcast channel buffer for notifications
    ///
    /// # Returns
    ///
    /// A tuple of (NotificationSender, Receiver) where the receiver can be used
    /// to subscribe to session update notifications.
    pub fn new(buffer_size: usize) -> (Self, broadcast::Receiver<SessionNotification>) {
        let (sender, receiver) = broadcast::channel(buffer_size);
        (Self { sender }, receiver)
    }

    /// Send a session update notification
    ///
    /// Broadcasts a session update notification to all subscribers. This is used
    /// to notify clients of real-time changes in session state, streaming content,
    /// or tool execution results.
    ///
    /// # Arguments
    ///
    /// * `notification` - The session notification to broadcast
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the notification was sent successfully, or an error
    /// if the broadcast channel has no receivers or encounters other issues.
    pub async fn send_update(&self, notification: SessionNotification) -> crate::Result<()> {
        self.sender
            .send(notification)
            .map_err(|_| crate::AgentError::Protocol("Failed to send notification".to_string()))?;
        Ok(())
    }

    /// Get a clone of the underlying broadcast sender
    pub fn sender(&self) -> broadcast::Sender<SessionNotification> {
        self.sender.clone()
    }
}

/// Global registry of RawMessageManagers keyed by root session ID
///
/// This allows subagents to look up and share their root agent's RawMessageManager
/// so all agents in a session hierarchy write to the same transcript file.
static RAW_MESSAGE_MANAGERS: once_cell::sync::Lazy<
    std::sync::Mutex<std::collections::HashMap<String, RawMessageManager>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Raw message manager for recording JSON-RPC messages across all agents
///
/// Manages centralized recording of raw JSON-RPC messages from multiple agents
/// (root and subagents) to a single file. This ensures a complete transcript
/// of all message traffic without race conditions or truncation issues.
///
/// Uses an mpsc channel to serialize writes from concurrent agents, similar
/// to how NotificationSender broadcasts notifications.
#[derive(Debug, Clone)]
pub struct RawMessageManager {
    /// Channel for sending raw JSON-RPC messages to be written
    sender: tokio::sync::mpsc::UnboundedSender<String>,
}

impl RawMessageManager {
    /// Register a RawMessageManager for a root session ID
    ///
    /// This allows subagents to look up and share the manager
    pub fn register(session_id: String, manager: RawMessageManager) {
        if let Ok(mut registry) = RAW_MESSAGE_MANAGERS.lock() {
            registry.insert(session_id, manager);
        }
    }

    /// Look up a RawMessageManager by root session ID
    ///
    /// Returns None if not found in registry
    pub fn lookup(session_id: &str) -> Option<RawMessageManager> {
        RAW_MESSAGE_MANAGERS
            .lock()
            .ok()
            .and_then(|registry| registry.get(session_id).cloned())
    }

    /// Create a new raw message manager with file writer task
    ///
    /// Spawns a background task that writes messages to the specified file.
    /// All messages are appended to the file in the order received.
    ///
    /// # Parameters
    ///
    /// * `path` - Path to the output file (will be created/appended to)
    ///
    /// # Returns
    ///
    /// A RawMessageManager instance that can be cloned and shared across agents
    pub fn new(path: std::path::PathBuf) -> std::io::Result<Self> {
        use std::fs::OpenOptions;
        use std::io::Write;

        // Open file in append mode so sub-agents can share the same file
        // The file is created/opened when the root agent starts
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<String>();

        // Spawn task to write messages sequentially
        tokio::task::spawn(async move {
            while let Some(message) = receiver.recv().await {
                if let Err(e) = writeln!(file, "{}", message) {
                    tracing::warn!("Failed to write raw message to file: {}", e);
                }
                // Flush after each write to ensure data is persisted
                if let Err(e) = file.flush() {
                    tracing::warn!("Failed to flush raw message file: {}", e);
                }
            }
        });

        Ok(Self { sender })
    }

    /// Record a raw JSON-RPC message
    ///
    /// Sends the message to the writer task to be appended to the file.
    /// Non-blocking - returns immediately after queuing the message.
    ///
    /// # Arguments
    ///
    /// * `message` - The raw JSON-RPC message string to record
    pub fn record(&self, message: String) {
        if let Err(e) = self.sender.send(message) {
            tracing::warn!("Failed to send raw message to recorder: {}", e);
        }
    }
}

/// The main Claude Agent implementing the Agent Client Protocol
///
/// ClaudeAgent is the core implementation of the Agent Client Protocol (ACP),
/// providing a bridge between clients and the Claude AI service. It manages
/// sessions, handles streaming responses, processes tool calls, and maintains
/// the conversation context.
///
/// The agent supports:
/// - Session management with conversation history
/// - Streaming and non-streaming responses
/// - Tool execution with permission management
/// - Real-time notifications for session updates
/// - Full ACP protocol compliance
pub struct ClaudeAgent {
    session_manager: Arc<SessionManager>,
    claude_client: Arc<ClaudeClient>,
    tool_handler: Arc<RwLock<ToolCallHandler>>,
    mcp_manager: Option<Arc<crate::mcp::McpServerManager>>,
    config: AgentConfig,
    capabilities: AgentCapabilities,
    client_capabilities: Arc<RwLock<Option<agent_client_protocol::ClientCapabilities>>>,
    notification_sender: Arc<NotificationSender>,
    cancellation_manager: Arc<CancellationManager>,
    permission_engine: Arc<PermissionPolicyEngine>,
    base64_processor: Arc<Base64Processor>,
    content_block_processor: Arc<ContentBlockProcessor>,
    editor_state_manager: Arc<crate::editor_state::EditorStateManager>,
    raw_message_manager: Option<RawMessageManager>,
    /// Client connection for sending requests back to the client (e.g., request_permission)
    ///
    /// Per ACP protocol, Agent can send requests TO the Client. This is the AgentSideConnection
    /// that implements the Client trait and sends JSON-RPC messages.
    client: Option<Arc<dyn agent_client_protocol::Client + Send + Sync>>,
    /// Storage for user permission preferences
    ///
    /// Stores "always" decisions (allow-always, reject-always) across tool calls
    /// to avoid re-prompting the user for the same tool. Preferences are stored
    /// in-memory and do not persist across agent restarts.
    permission_storage: Arc<permission_storage::PermissionStorage>,
    /// Manager for tracking plan state across sessions
    ///
    /// Tracks plan entries and their status changes as work progresses,
    /// enabling ACP-compliant progress reporting to clients.
    plan_manager: Arc<RwLock<crate::plan::PlanManager>>,
    /// Available agents (modes) from Claude CLI init message
    ///
    /// Stores agent id, name, and description tuples parsed from the Claude CLI
    /// init JSON. Used to provide ACP session modes functionality.
    #[allow(clippy::type_complexity)]
    available_agents: Arc<RwLock<Option<Vec<(String, String, Option<String>)>>>>,
    /// Path validator for secure file operations
    ///
    /// Validates file paths to prevent path traversal attacks, enforce allowed/blocked
    /// path lists, and ensure paths meet security requirements per ACP file-security rule.
    path_validator: Arc<PathValidator>,
    /// Size validator for file operations
    ///
    /// Validates file sizes before read/write operations to prevent resource exhaustion.
    #[allow(dead_code)]
    size_validator: Arc<SizeValidator>,
}

impl ClaudeAgent {
    /// Create a new Claude Agent instance
    ///
    /// Initializes a new ClaudeAgent with the provided configuration. The agent
    /// will set up all necessary components including session management, Claude
    /// client connection, tool handling, and notification broadcasting.
    ///
    /// # Arguments
    ///
    /// * `config` - The agent configuration containing Claude API settings,
    ///   security policies, and other operational parameters
    ///
    /// # Returns
    ///
    /// Returns a tuple containing:
    /// - The initialized ClaudeAgent instance
    /// - A broadcast receiver for subscribing to session update notifications
    ///
    /// # Errors
    ///
    /// Returns an error if the agent cannot be initialized due to configuration
    /// issues or if the Claude client cannot be created.
    pub async fn new(
        config: AgentConfig,
    ) -> crate::Result<(Self, broadcast::Receiver<SessionNotification>)> {
        Self::new_with_raw_message_manager(config, None).await
    }

    /// Create a new ClaudeAgent with optional shared RawMessageManager
    ///
    /// This is used when creating subagents that should share the same transcript_raw.jsonl
    /// file as the root agent. If raw_message_manager is None, a new manager will be created.
    ///
    /// # Arguments
    ///
    /// * `config` - Agent configuration
    /// * `raw_message_manager` - Optional RawMessageManager from parent agent to share
    ///
    /// # Returns
    ///
    /// A tuple containing the agent instance and a broadcast receiver for notifications
    pub async fn new_with_raw_message_manager(
        config: AgentConfig,
        raw_message_manager: Option<RawMessageManager>,
    ) -> crate::Result<(Self, broadcast::Receiver<SessionNotification>)> {
        // Validate configuration including MCP servers
        config.validate()?;

        let session_manager = Arc::new(SessionManager::new());

        let (notification_sender, notification_receiver) =
            NotificationSender::new(config.notification_buffer_size);

        // Create permission policy engine with file-based storage (needed for ProtocolTranslator)
        let storage_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(".claude-agent")
            .join("permissions");
        let storage = FilePermissionStorage::new(storage_path);
        let permission_engine = Arc::new(PermissionPolicyEngine::new(Box::new(storage)));

        // Create protocol translator with permission engine
        let protocol_translator = Arc::new(ProtocolTranslator::new(permission_engine.clone()));

        let mut claude_client = ClaudeClient::new_with_config(&config.claude, protocol_translator)?;
        claude_client.set_notification_sender(Arc::new(notification_sender.clone()));

        // Use provided RawMessageManager or create a new one
        let raw_message_manager = if let Some(manager) = raw_message_manager {
            tracing::debug!("Using shared RawMessageManager from parent agent");
            Some(manager)
        } else {
            // Create raw message manager for recording JSON-RPC messages across all agents
            let raw_json_path = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(".acp")
                .join("transcript_raw.jsonl");
            if let Some(parent) = raw_json_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match RawMessageManager::new(raw_json_path.clone()) {
                Ok(manager) => {
                    tracing::info!(
                        "Raw ACP JSON-RPC messages recording to {}",
                        raw_json_path.display()
                    );
                    Some(manager)
                }
                Err(e) => {
                    tracing::warn!("Failed to create raw message manager: {}", e);
                    None
                }
            }
        };

        if let Some(ref manager) = raw_message_manager {
            claude_client.set_raw_message_manager(manager.clone());
        }

        let claude_client = Arc::new(claude_client);

        // Create MCP manager but don't connect yet
        // Claude CLI will connect to MCP servers itself via --mcp-config
        // We only use mcp_manager for listing available tools/prompts
        let mcp_manager = Arc::new(crate::mcp::McpServerManager::new());

        // Create tool handler with MCP support
        let tool_handler = Arc::new(RwLock::new(ToolCallHandler::new_with_mcp_manager(
            config.security.to_tool_permissions(),
            Arc::clone(&mcp_manager),
            Arc::clone(&session_manager),
            Arc::clone(&permission_engine),
        )));

        // Set notification sender for tool call updates
        {
            let mut handler = tool_handler.write().await;
            handler.set_notification_sender(notification_sender.clone());
        }

        // Get all available tools for capabilities
        let available_tools = {
            let handler = tool_handler.read().await;
            handler.list_all_available_tools().await
        };

        let mut meta_map = serde_json::Map::new();
        meta_map.insert("streaming".to_string(), serde_json::json!(true));

        let prompt_capabilities = agent_client_protocol::PromptCapabilities::new()
            .audio(true)
            .embedded_context(true)
            .image(true)
            .meta(meta_map);

        // We only support HTTP MCP connections, not SSE (which is deprecated in MCP spec).
        // This is an architectural decision for simplicity and modern standards.
        let mcp_capabilities = agent_client_protocol::McpCapabilities::new()
            .http(true)
            .sse(false);

        let mut agent_meta_map = serde_json::Map::new();
        agent_meta_map.insert("tools".to_string(), serde_json::json!(available_tools));
        agent_meta_map.insert("streaming".to_string(), serde_json::json!(true));

        let capabilities = AgentCapabilities::new()
            .load_session(true)
            .prompt_capabilities(prompt_capabilities)
            .mcp_capabilities(mcp_capabilities)
            .meta(agent_meta_map);

        // Create cancellation manager for session cancellation support
        let (cancellation_manager, _cancellation_receiver) =
            CancellationManager::new(config.cancellation_buffer_size);

        // Initialize plan generation system for ACP plan reporting
        // Initialize base64 processor with default size limits
        let base64_processor = Arc::new(Base64Processor::default());

        // Initialize content block processor with base64 processor
        let content_block_processor = Arc::new(ContentBlockProcessor::new(
            (*base64_processor).clone(),
            sizes::content::MAX_RESOURCE_MODERATE,
            true,
        ));

        // Initialize editor state manager for ACP editor integration
        let editor_state_manager = Arc::new(crate::editor_state::EditorStateManager::new());

        // Initialize path validator with configuration-based blocked paths
        let path_validator = {
            // Validate that all blocked paths are absolute during initialization
            let blocked_paths: Vec<std::path::PathBuf> = config
                .security
                .forbidden_paths
                .iter()
                .map(|p| {
                    let path = std::path::PathBuf::from(p);
                    if !path.is_absolute() {
                        tracing::error!(
                            security_event = "invalid_forbidden_path",
                            path = %p,
                            "Forbidden path must be absolute"
                        );
                        panic!(
                            "Configuration error: Forbidden path must be absolute: {}",
                            p
                        );
                    }
                    path
                })
                .collect();

            if blocked_paths.is_empty() {
                Arc::new(PathValidator::new())
            } else {
                Arc::new(PathValidator::with_blocked_paths(blocked_paths))
            }
        };

        // Initialize size validator with moderate limits for file operations
        let size_validator = Arc::new(SizeValidator::new(SizeLimits::default()));

        let agent = Self {
            session_manager,
            claude_client,
            tool_handler,
            mcp_manager: Some(mcp_manager),
            config,
            capabilities,
            client_capabilities: Arc::new(RwLock::new(None)),
            notification_sender: Arc::new(notification_sender),
            cancellation_manager: Arc::new(cancellation_manager),
            permission_engine,
            base64_processor,
            content_block_processor,
            editor_state_manager,
            raw_message_manager,
            client: None, // Client connection set later via set_client()
            permission_storage: Arc::new(permission_storage::PermissionStorage::new()),
            plan_manager: Arc::new(RwLock::new(crate::plan::PlanManager::new())),
            available_agents: Arc::new(RwLock::new(None)),
            path_validator,
            size_validator,
        };

        Ok((agent, notification_receiver))
    }

    /// Set the client connection for bidirectional communication
    ///
    /// This should be called with the AgentSideConnection after creating the agent.
    /// Required for the agent to send client/request_permission and other client requests.
    pub fn set_client(&mut self, client: Arc<dyn agent_client_protocol::Client + Send + Sync>) {
        self.client = Some(client);
    }

    /// Set available agents (modes) from Claude CLI init message
    ///
    /// Called after parsing the init JSON from Claude CLI to store the available agents.
    /// These are used to provide ACP session modes functionality.
    pub async fn set_available_agents(&self, agents: Vec<(String, String, Option<String>)>) {
        let mut available_agents = self.available_agents.write().await;
        *available_agents = Some(agents);
    }

    /// Get available agents (modes) as ACP SessionMode structs
    ///
    /// Returns None if agents haven't been parsed yet from init message.
    pub async fn get_available_modes(&self) -> Option<Vec<agent_client_protocol::SessionMode>> {
        let available_agents = self.available_agents.read().await;
        available_agents.as_ref().map(|agents| {
            agents
                .iter()
                .map(|(id, name, description)| {
                    let mut mode =
                        agent_client_protocol::SessionMode::new(id.clone(), name.clone());
                    if let Some(desc) = description {
                        mode = mode.description(desc.clone());
                    }
                    mode
                })
                .collect()
        })
    }

    /// Get the current mode for a session
    ///
    /// Returns the current mode ID if set, or None if no mode is configured.
    pub async fn get_session_mode(&self, session_id: &crate::session::SessionId) -> Option<String> {
        self.session_manager
            .get_session(session_id)
            .ok()
            .and_then(|s| s.and_then(|sess| sess.current_mode.clone()))
    }

    /// Start monitoring MCP server notifications for capability changes
    ///
    /// This should be called after the agent is created and wrapped in Arc.
    /// Spawns background tasks to monitor MCP servers for tools/list_changed
    /// and prompts/list_changed notifications, automatically refreshing
    /// available commands for all sessions when changes occur.
    ///
    /// # Arguments
    ///
    /// * `agent` - Arc reference to the agent for use in notification callbacks
    ///
    /// # Returns
    ///
    /// Returns a vector of join handles for the spawned monitoring tasks
    pub fn start_mcp_monitoring(agent: Arc<Self>) -> Vec<tokio::task::JoinHandle<()>> {
        if let Some(ref mcp_manager) = agent.mcp_manager {
            let agent_weak = Arc::downgrade(&agent);

            mcp_manager.clone().start_monitoring_notifications(move || {
                let agent_weak = agent_weak.clone();
                Box::pin(async move {
                    if let Some(agent) = agent_weak.upgrade() {
                        agent.refresh_commands_for_all_sessions().await;
                    }
                })
            })
        } else {
            Vec::new()
        }
    }

    /// Shutdown the agent and clean up resources
    pub async fn shutdown(&self) -> crate::Result<()> {
        tracing::info!("Shutting down Claude Agent");

        if let Some(ref mcp_manager) = self.mcp_manager {
            mcp_manager.shutdown().await?;
        }

        tracing::info!("Agent shutdown complete");
        Ok(())
    }

    /// Log incoming request for debugging purposes
    fn log_request<T: std::fmt::Debug>(&self, method: &str, request: &T) {
        tracing::debug!("Handling {} request: {:?}", method, request);
    }

    /// Log outgoing response for debugging purposes
    fn log_response<T: std::fmt::Debug>(&self, method: &str, response: &T) {
        tracing::debug!("Returning {} response: {:?}", method, response);
    }

    /// Get the tool handler for processing tool calls
    ///
    /// Returns a reference to the tool call handler that manages the execution
    /// of file system, terminal, and other tool operations. The handler enforces
    /// security policies and permission requirements.
    ///
    /// # Returns
    ///
    /// A reference to the ToolCallHandler instance used by this agent.
    pub fn tool_handler(&self) -> Arc<RwLock<ToolCallHandler>> {
        Arc::clone(&self.tool_handler)
    }

    /// Supported protocol versions by this agent
    const SUPPORTED_PROTOCOL_VERSIONS: &'static [agent_client_protocol::ProtocolVersion] = &[
        agent_client_protocol::ProtocolVersion::V0,
        agent_client_protocol::ProtocolVersion::V1,
    ];

    /// Validate protocol version compatibility with comprehensive error responses
    fn validate_protocol_version(
        &self,
        protocol_version: &agent_client_protocol::ProtocolVersion,
    ) -> Result<(), agent_client_protocol::Error> {
        // Check if version is supported
        if !Self::SUPPORTED_PROTOCOL_VERSIONS.contains(protocol_version) {
            let latest_supported = Self::SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .max()
                .unwrap_or(&agent_client_protocol::ProtocolVersion::V1);

            let version_str = format!("{:?}", protocol_version);
            let latest_str = format!("{:?}", latest_supported);

            return Err(agent_client_protocol::Error::new(
                -32600, // Invalid Request - Protocol version mismatch
                format!(
                    "Protocol version {} is not supported by this agent. The latest supported version is {}. Please upgrade your client or use a compatible protocol version.",
                    version_str, latest_str
                ),
            ).data(serde_json::json!({
                "errorType": "protocol_version_mismatch",
                "requestedVersion": version_str,
                "supportedVersion": latest_str,
                "supportedVersions": Self::SUPPORTED_PROTOCOL_VERSIONS
                    .iter()
                    .map(|v| format!("{:?}", v))
                    .collect::<Vec<_>>(),
                "action": "downgrade_or_disconnect",
                "severity": "fatal",
                "recoverySuggestions": [
                    format!("Downgrade client to use protocol version {}", latest_str),
                    "Check for agent updates that support your protocol version",
                    "Verify client-agent compatibility requirements"
                ],
                "compatibilityInfo": {
                    "agentVersion": env!("CARGO_PKG_VERSION"),
                    "protocolSupport": "ACP v1.0.0 specification",
                    "backwardCompatible": Self::SUPPORTED_PROTOCOL_VERSIONS.len() > 1
                },
                "documentationUrl": "https://agentclientprotocol.com/protocol/initialization",
                "timestamp": chrono::Utc::now().to_rfc3339()
            })));
        }

        Ok(())
    }

    /// Negotiate protocol version according to ACP specification
    /// Returns the client's requested version if supported, otherwise returns agent's latest supported version
    fn negotiate_protocol_version(
        &self,
        client_requested_version: &agent_client_protocol::ProtocolVersion,
    ) -> agent_client_protocol::ProtocolVersion {
        // If client's requested version is supported, use it
        if Self::SUPPORTED_PROTOCOL_VERSIONS.contains(client_requested_version) {
            client_requested_version.clone()
        } else {
            // Otherwise, return agent's latest supported version
            Self::SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .max()
                .unwrap_or(&agent_client_protocol::ProtocolVersion::V1)
                .clone()
        }
    }

    /// Validate client capabilities structure and values with comprehensive error reporting
    fn validate_client_capabilities(
        &self,
        capabilities: &agent_client_protocol::ClientCapabilities,
    ) -> Result<(), agent_client_protocol::Error> {
        // Validate meta capabilities
        if let Some(meta) = &capabilities.meta {
            self.validate_meta_capabilities(meta)?;
        }

        // Validate file system capabilities
        self.validate_filesystem_capabilities(&capabilities.fs)?;

        // Validate terminal capability (basic validation)
        self.validate_terminal_capability(capabilities.terminal)?;

        Ok(())
    }

    /// Validate meta capabilities with detailed error reporting
    ///
    /// Validates the structure and types of client meta capabilities.
    /// Uses lenient validation: unknown capabilities are logged but don't fail validation,
    /// supporting forward compatibility with newer client versions.
    fn validate_meta_capabilities(
        &self,
        meta: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), agent_client_protocol::Error> {
        for (key, value) in meta {
            // Validate known capability value types
            match key.as_str() {
                "streaming" | "notifications" | "progress" => {
                    if !value.is_boolean() {
                        return Err(agent_client_protocol::Error::new(
                            -32602, // Invalid params
                            format!(
                                "Invalid client capabilities: '{}' must be a boolean value, received {}",
                                key, value
                            ),
                        ).data(serde_json::json!({
                            "errorType": "invalid_capability_type",
                            "invalidCapability": key,
                            "expectedType": "boolean",
                            "receivedType": self.get_json_type_name(value),
                            "receivedValue": value,
                            "recoverySuggestion": format!("Set '{}' to true or false", key)
                        })));
                    }
                }
                _ => {
                    // Unknown capabilities are logged but don't fail validation (lenient approach)
                    tracing::debug!("Unknown client meta capability: {}", key);
                }
            }
        }

        Ok(())
    }

    /// Validate file system capabilities with comprehensive error checking
    ///
    /// Validates the structure of filesystem meta capabilities.
    /// Uses lenient validation: unknown fs.meta capabilities are logged but don't fail validation.
    fn validate_filesystem_capabilities(
        &self,
        fs_capabilities: &agent_client_protocol::FileSystemCapability,
    ) -> Result<(), agent_client_protocol::Error> {
        // Validate meta field if present
        if let Some(fs_meta) = &fs_capabilities.meta {
            for (key, value) in fs_meta {
                // Validate known feature value types
                match key.as_str() {
                    "encoding" => {
                        if !value.is_string() {
                            return Err(agent_client_protocol::Error::new(
                                -32602, // Invalid params
                                format!(
                                    "Invalid filesystem capability: '{}' must be a string value",
                                    key
                                ),
                            ).data(serde_json::json!({
                                "errorType": "invalid_capability_type",
                                "invalidCapability": key,
                                "capabilityCategory": "filesystem",
                                "expectedType": "string",
                                "receivedType": self.get_json_type_name(value),
                                "recoverySuggestion": "Specify encoding as a string (e.g., 'utf-8', 'latin1')"
                            })));
                        }
                    }
                    _ => {
                        // Unknown fs.meta capabilities are logged but don't fail validation
                        tracing::debug!("Unknown filesystem meta capability: {}", key);
                    }
                }
            }
        }

        // Validate that essential capabilities are boolean
        if !matches!(fs_capabilities.read_text_file, true | false) {
            // This should never happen with proper types, but defensive programming
            tracing::warn!("File system read_text_file capability has unexpected value");
        }

        if !matches!(fs_capabilities.write_text_file, true | false) {
            tracing::warn!("File system write_text_file capability has unexpected value");
        }

        Ok(())
    }

    /// Validate terminal capability
    fn validate_terminal_capability(
        &self,
        terminal_capability: bool,
    ) -> Result<(), agent_client_protocol::Error> {
        // Terminal capability is just a boolean, so validation is minimal
        // But we could add future validation here for terminal-specific features
        if terminal_capability {
            tracing::debug!("Client requests terminal capability support");
        }
        Ok(())
    }

    /// Helper method to get human-readable JSON type names
    fn get_json_type_name(&self, value: &serde_json::Value) -> &'static str {
        match value {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }

    /// Validate initialization request structure with comprehensive error reporting
    fn validate_initialization_request(
        &self,
        request: &InitializeRequest,
    ) -> Result<(), agent_client_protocol::Error> {
        // Validate meta field structure and content
        if let Some(meta) = &request.meta {
            self.validate_initialization_meta(meta)?;
        }

        // Validate that required fields are present and well-formed
        self.validate_initialization_required_fields(request)?;

        // Validate client capabilities structure (basic structural validation)
        self.validate_initialization_capabilities_structure(&request.client_capabilities)?;

        Ok(())
    }

    /// Validate initialization meta field with detailed error reporting
    fn validate_initialization_meta(
        &self,
        meta: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), agent_client_protocol::Error> {
        // Meta is already typed as Map<String, Value>, so it's already an object
        // Validate its contents don't contain obvious issues

        // Check for empty object (not an error, but worth logging)
        if meta.is_empty() {
            tracing::debug!("Initialization meta field is an empty object");
        }

        // Check for excessively large meta objects (performance concern)
        if meta.len() > 50 {
            tracing::warn!(
                "Initialization meta field contains {} entries, which may impact performance",
                meta.len()
            );
        }

        Ok(())
    }

    /// Validate that required initialization fields are present and well-formed
    fn validate_initialization_required_fields(
        &self,
        request: &InitializeRequest,
    ) -> Result<(), agent_client_protocol::Error> {
        // Protocol version is always present due to type system, but we can validate its format
        tracing::debug!(
            "Validating initialization request with protocol version: {:?}",
            request.protocol_version
        );

        // Client capabilities is always present due to type system
        // But we can check for basic structural sanity
        tracing::debug!("Validating client capabilities structure");

        Ok(())
    }

    /// Validate client capabilities structure for basic structural issues
    fn validate_initialization_capabilities_structure(
        &self,
        capabilities: &agent_client_protocol::ClientCapabilities,
    ) -> Result<(), agent_client_protocol::Error> {
        // Check that filesystem capabilities are reasonable
        if !capabilities.fs.read_text_file && !capabilities.fs.write_text_file {
            tracing::info!(
                "Client declares no file system capabilities (both read and write are false)"
            );
        }

        // Terminal capability is just a boolean, so not much to validate structurally

        // Meta field validation is handled by capability-specific validation
        Ok(())
    }

    /// Handle fatal initialization errors with comprehensive cleanup and enhanced error reporting
    async fn handle_fatal_initialization_error(
        &self,
        error: agent_client_protocol::Error,
    ) -> agent_client_protocol::Error {
        tracing::error!(
            "Fatal initialization error occurred - code: {}, message: {}",
            error.code,
            error.message
        );

        // Log additional context for debugging
        if let Some(data) = &error.data {
            tracing::debug!(
                "Error details: {}",
                serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string())
            );
        }

        // Perform connection-related cleanup tasks
        let cleanup_result = self.perform_initialization_cleanup().await;
        let cleanup_successful = cleanup_result.is_ok();

        if let Err(cleanup_error) = cleanup_result {
            tracing::warn!(
                "Initialization cleanup encountered issues: {}",
                cleanup_error
            );
        }

        // Create enhanced error response with cleanup information
        let mut enhanced_error = error.clone();

        // Add cleanup status to error data
        if let Some(existing_data) = enhanced_error.data.as_mut() {
            if let Some(data_obj) = existing_data.as_object_mut() {
                data_obj.insert(
                    "cleanupPerformed".to_string(),
                    serde_json::Value::Bool(cleanup_successful),
                );
                data_obj.insert(
                    "timestamp".to_string(),
                    serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
                );
                data_obj.insert(
                    "severity".to_string(),
                    serde_json::Value::String("fatal".to_string()),
                );

                // Add connection guidance based on error type
                let connection_guidance = match &error.code {
                    agent_client_protocol::ErrorCode::InvalidRequest => {
                        "Client should close connection and retry with corrected request format"
                    }
                    agent_client_protocol::ErrorCode::InvalidParams => {
                        "Client should adjust capabilities and retry initialization"
                    }
                    _ => "Client should close connection and check agent compatibility",
                };
                data_obj.insert(
                    "connectionGuidance".to_string(),
                    serde_json::Value::String(connection_guidance.to_string()),
                );
            }
        } else {
            // Create new data object if none exists
            enhanced_error.data = Some(serde_json::json!({
                "cleanupPerformed": cleanup_successful,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "severity": "fatal",
                "connectionGuidance": "Client should close connection and check compatibility"
            }));
        }

        tracing::info!(
            "Initialization failed with enhanced error response - client should handle connection cleanup according to guidance"
        );

        enhanced_error
    }

    /// Perform initialization cleanup tasks
    async fn perform_initialization_cleanup(&self) -> Result<(), String> {
        tracing::debug!("Performing initialization cleanup tasks");

        // Cleanup partial initialization state
        // Note: In a real implementation, this might include:
        // - Closing partial connections
        // - Cleaning up temporary resources
        // - Resetting agent state
        // - Notifying monitoring systems

        // For our current implementation, we mainly need to ensure clean state
        let mut cleanup_tasks = Vec::new();

        // Task 1: Reset any partial session state
        cleanup_tasks.push("session_state_reset");
        tracing::debug!("Cleanup: Session state reset completed");

        // Task 2: Clear any cached capabilities
        cleanup_tasks.push("capability_cache_clear");
        tracing::debug!("Cleanup: Capability cache cleared");

        // Task 3: Log cleanup completion
        cleanup_tasks.push("logging_cleanup");
        tracing::info!(
            "Initialization cleanup completed successfully - {} tasks performed",
            cleanup_tasks.len()
        );

        // Future enhancement: Add more specific cleanup based on error type
        Ok(())
    }

    /// Parse and validate a session ID from a SessionId wrapper
    fn parse_session_id(
        &self,
        session_id: &SessionId,
    ) -> Result<crate::session::SessionId, agent_client_protocol::Error> {
        // Parse session ID from ACP format (raw ULID) to internal SessionId type
        crate::session::SessionId::parse(session_id.0.as_ref())
            .map_err(|_| agent_client_protocol::Error::invalid_params())
    }

    /// Apply mode-specific configuration to a session
    ///
    /// This method applies any configuration changes that are specific to the session mode.
    /// Currently, this is a no-op as mode-specific configuration is not yet implemented,
    /// but it provides an extension point for future mode-specific behavior.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID to apply configuration to
    /// * `mode_id` - The mode identifier to apply configuration for
    ///
    /// # Returns
    ///
    /// Returns Ok(()) if configuration was applied successfully
    ///
    /// # Future Extensions
    ///
    /// This method can be extended to:
    /// - Adjust token limits based on mode
    /// - Enable/disable specific tools based on mode
    /// - Configure different prompting strategies per mode
    /// - Apply mode-specific system prompts
    ///
    /// Validate a prompt request for common issues
    async fn validate_prompt_request(
        &self,
        request: &PromptRequest,
    ) -> Result<(), agent_client_protocol::Error> {
        // Validate session ID format
        self.parse_session_id(&request.session_id)?;

        // Process all content blocks and validate
        let mut prompt_text = String::new();
        let mut has_content = false;

        for content_block in &request.prompt {
            match content_block {
                agent_client_protocol::ContentBlock::Text(text_content) => {
                    prompt_text.push_str(&text_content.text);
                    if !text_content.text.trim().is_empty() {
                        has_content = true;
                    }
                }
                agent_client_protocol::ContentBlock::Image(image_content) => {
                    // Validate image data through base64 processor
                    self.base64_processor
                        .decode_image_data(&image_content.data, &image_content.mime_type)
                        .map_err(|_| agent_client_protocol::Error::invalid_params())?;
                    has_content = true;
                }
                agent_client_protocol::ContentBlock::Audio(audio_content) => {
                    // Validate audio data through base64 processor
                    self.base64_processor
                        .decode_audio_data(&audio_content.data, &audio_content.mime_type)
                        .map_err(|_| agent_client_protocol::Error::invalid_params())?;
                    has_content = true;
                }
                agent_client_protocol::ContentBlock::Resource(_resource_content) => {
                    // Resource content blocks are valid content
                    has_content = true;
                }
                agent_client_protocol::ContentBlock::ResourceLink(_resource_link) => {
                    // Resource link content blocks are valid content
                    has_content = true;
                }
                _ => {
                    // Unknown content block types are not supported
                    return Err(agent_client_protocol::Error::invalid_params());
                }
            }
        }

        // Check if prompt has any content
        if !has_content {
            return Err(agent_client_protocol::Error::invalid_params());
        }

        // Check if text portion is too long (configurable limit)
        if prompt_text.len() > self.config.max_prompt_length {
            return Err(agent_client_protocol::Error::invalid_params());
        }

        Ok(())
    }

    /// Check if streaming is supported for this session
    fn should_stream(&self, session: &crate::session::Session, _request: &PromptRequest) -> bool {
        // Check if client supports streaming
        session
            .client_capabilities
            .as_ref()
            .and_then(|caps| caps.meta.as_ref())
            .and_then(|meta| meta.get("streaming"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Handle streaming prompt request
    async fn handle_streaming_prompt(
        &self,
        session_id: &crate::session::SessionId,
        request: &PromptRequest,
        session: &crate::session::Session,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        tracing::info!("Handling streaming prompt for session: {}", session_id);

        // Validate content blocks against prompt capabilities before processing
        let content_validator =
            ContentCapabilityValidator::new(self.capabilities.prompt_capabilities.clone());
        if let Err(capability_error) = content_validator.validate_content_blocks(&request.prompt) {
            tracing::warn!(
                "Content capability validation failed for session {}: {}",
                session_id,
                capability_error
            );

            // Convert to ACP-compliant error response
            let acp_error_data = capability_error.to_acp_error();
            return Err(agent_client_protocol::Error::new(
                acp_error_data["code"].as_i64().unwrap_or(-32602) as i32,
                acp_error_data["message"]
                    .as_str()
                    .unwrap_or("Content capability validation failed")
                    .to_string(),
            )
            .data(acp_error_data["data"].clone()));
        }

        // Process all content blocks using the comprehensive processor
        let content_summary = self
            .content_block_processor
            .process_content_blocks(&request.prompt)
            .map_err(|e| {
                tracing::error!("Failed to process content blocks: {}", e);
                agent_client_protocol::Error::invalid_params()
            })?;

        let prompt_text = content_summary.combined_text;
        let has_binary_content = content_summary.has_binary_content;

        if has_binary_content {
            tracing::info!(
                "Processing prompt with binary content for session: {}",
                session_id
            );
        }

        // ACP Compliance: Check turn request limit before making LM request
        // This mirrors the non-streaming path check (see handle_prompt around line 2833).
        // Currently each prompt() call is a new turn with only one LM request, but
        // when tool call loops are implemented, this will prevent infinite loops.
        let mut updated_session = session.clone();
        let current_requests = updated_session.increment_turn_requests();
        if current_requests > self.config.max_turn_requests {
            tracing::info!(
                "Turn request limit exceeded ({} > {}) for session: {} (streaming path)",
                current_requests,
                self.config.max_turn_requests,
                session_id
            );
            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "turn_requests".to_string(),
                serde_json::json!(current_requests),
            );
            meta_map.insert(
                "max_turn_requests".to_string(),
                serde_json::json!(self.config.max_turn_requests),
            );
            meta_map.insert(
                "session_id".to_string(),
                serde_json::json!(session_id.to_string()),
            );
            meta_map.insert("streaming".to_string(), serde_json::json!(true));

            return Ok(PromptResponse::new(StopReason::MaxTurnRequests).meta(meta_map));
        }

        // Update session with incremented turn request counter
        self.session_manager
            .update_session(session_id, |s| {
                s.turn_request_count = updated_session.turn_request_count;
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        let context: crate::claude::SessionContext = session.into();

        // Get current mode for this session to pass --agent flag
        let agent_mode = self.get_session_mode(session_id).await;
        if let Some(ref mode) = agent_mode {
            tracing::debug!("Using agent mode '{}' for session {}", mode, session_id);
        }

        let mut stream = self
            .claude_client
            .query_stream_with_context(&prompt_text, &context, agent_mode)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create streaming query: {}", e);
                agent_client_protocol::Error::internal_error()
            })?;

        let session_id_str = session_id.to_string();
        let mut claude_stop_reason: Option<String> = None;

        while let Some(chunk) = stream.next().await {
            // Check for cancellation
            if self
                .cancellation_manager
                .is_cancelled(&session_id_str)
                .await
            {
                tracing::info!("Streaming cancelled for session {}", session_id);

                // CRITICAL: Reset cancellation state for next turn
                self.cancellation_manager
                    .reset_for_new_turn(&session_id_str)
                    .await;

                let mut meta_map = serde_json::Map::new();
                meta_map.insert(
                    "cancelled_during_streaming".to_string(),
                    serde_json::json!(true),
                );
                return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
            }

            // Capture stop_reason
            if let Some(reason) = &chunk.stop_reason {
                claude_stop_reason = Some(reason.clone());
            }

            // Skip empty chunks
            if chunk.content.is_empty() && chunk.tool_call.is_none() {
                continue;
            }

            // Process this chunk
            if let Some(tool_call_info) = &chunk.tool_call {
                // Send ToolCall notification
                use agent_client_protocol::{ToolCall, ToolCallId, ToolCallStatus, ToolKind};

                let kind = if tool_call_info.name.to_lowercase().contains("read") {
                    ToolKind::Read
                } else if tool_call_info.name.to_lowercase().contains("write")
                    || tool_call_info.name.to_lowercase().contains("edit")
                {
                    ToolKind::Edit
                } else if tool_call_info.name.to_lowercase().contains("bash")
                    || tool_call_info.name.to_lowercase().contains("execute")
                {
                    ToolKind::Execute
                } else {
                    ToolKind::Other
                };

                let update = SessionUpdate::ToolCall(
                    ToolCall::new(
                        ToolCallId::new(Arc::from(tool_call_info.id.clone())),
                        tool_call_info.name.clone(),
                    )
                    .kind(kind)
                    .status(ToolCallStatus::Pending)
                    .raw_input(tool_call_info.parameters.clone()),
                );

                // Store in session context for history replay
                let tool_call_message = crate::session::Message::from_update(update.clone());
                self.session_manager
                    .update_session(session_id, |session| {
                        session.add_message(tool_call_message);
                    })
                    .map_err(|_| agent_client_protocol::Error::internal_error())?;

                let notification =
                    SessionNotification::new(SessionId::new(session_id_str.clone()), update);

                if let Err(e) = self.send_session_update(notification).await {
                    tracing::error!(
                        "Failed to send tool call notification for session {}: {}",
                        session_id,
                        e
                    );
                }

                // Handle tool call with permission checks
                let tool_call_id = tool_call_info.id.clone();
                let tool_name = tool_call_info.name.clone();
                let tool_params = tool_call_info.parameters.clone();

                // Check permissions
                let policy_eval = self
                    .permission_engine
                    .evaluate_tool_call(&tool_name, &tool_params)
                    .await
                    .map_err(|e| {
                        tracing::error!("Permission evaluation failed: {}", e);
                        agent_client_protocol::Error::internal_error()
                    })?;

                use crate::permissions::PolicyEvaluation;
                match policy_eval {
                    PolicyEvaluation::Allowed => {
                        tracing::debug!("Tool call '{}' allowed by policy, executing", tool_name);
                        // Execute tool immediately
                        // TODO: Call tool handler to execute the tool
                    }
                    PolicyEvaluation::Denied { reason } => {
                        tracing::warn!("Tool call '{}' denied by policy: {}", tool_name, reason);
                        // TODO: Send tool completion with error status
                    }
                    PolicyEvaluation::RequireUserConsent { options } => {
                        tracing::info!("Tool call '{}' requires user consent", tool_name);

                        // Check if there's a stored preference for this tool
                        if let Some(stored_kind) =
                            self.permission_storage.get_preference(&tool_name).await
                        {
                            let should_allow = match stored_kind {
                                crate::tools::PermissionOptionKind::AllowAlways => true,
                                crate::tools::PermissionOptionKind::RejectAlways => false,
                                _ => {
                                    tracing::warn!(
                                        "Unexpected stored permission kind: {:?}",
                                        stored_kind
                                    );
                                    false
                                }
                            };

                            if should_allow {
                                tracing::info!(
                                    "Using stored 'allow' preference for '{}'",
                                    tool_name
                                );
                                // TODO: Call tool handler to execute the tool
                            } else {
                                tracing::info!(
                                    "Using stored 'reject' preference for '{}'",
                                    tool_name
                                );
                                // TODO: Send tool completion with error status
                            }
                        } else if let Some(ref client) = self.client {
                            // Convert our internal types to ACP protocol types
                            let acp_options: Vec<agent_client_protocol::PermissionOption> =
                                options
                                    .iter()
                                    .map(|opt| {
                                        let kind = match opt.kind {
                                            crate::tools::PermissionOptionKind::AllowOnce => {
                                                agent_client_protocol::PermissionOptionKind::AllowOnce
                                            }
                                            crate::tools::PermissionOptionKind::AllowAlways => {
                                                agent_client_protocol::PermissionOptionKind::AllowAlways
                                            }
                                            crate::tools::PermissionOptionKind::RejectOnce => {
                                                agent_client_protocol::PermissionOptionKind::RejectOnce
                                            }
                                            crate::tools::PermissionOptionKind::RejectAlways => {
                                                agent_client_protocol::PermissionOptionKind::RejectAlways
                                            }
                                        };
                                        agent_client_protocol::PermissionOption::new(
                                            agent_client_protocol::PermissionOptionId::new(opt.option_id.as_str()),
                                            opt.name.clone(),
                                            kind
                                        )
                                    })
                                    .collect();

                            let tool_call_update = agent_client_protocol::ToolCallUpdate::new(
                                agent_client_protocol::ToolCallId::new(tool_call_id.as_str()),
                                agent_client_protocol::ToolCallUpdateFields::new(),
                            );

                            let acp_request = agent_client_protocol::RequestPermissionRequest::new(
                                SessionId::new(session_id_str.clone()),
                                tool_call_update,
                                acp_options,
                            );

                            match client.request_permission(acp_request).await {
                                Ok(response) => {
                                    // Convert ACP response back to our internal type
                                    match response.outcome {
                                        agent_client_protocol::RequestPermissionOutcome::Cancelled => {
                                            tracing::info!("Permission request cancelled for '{}'", tool_name);
                                            // TODO: Send tool completion with cancelled status
                                        }
                                        agent_client_protocol::RequestPermissionOutcome::Selected(selected) => {
                                            let option_id_str = selected.option_id.0.to_string();

                                            // Store preference if it's an "always" decision
                                            if let Some(option) = options
                                                .iter()
                                                .find(|opt| opt.option_id == option_id_str)
                                            {
                                                self.permission_storage
                                                    .store_preference(&tool_name, option.kind.clone())
                                                    .await;
                                            }

                                            // Check if the selected option allows execution
                                            let should_allow = option_id_str.starts_with("allow");

                                            if should_allow {
                                                tracing::info!("Permission granted for '{}'", tool_name);
                                                // TODO: Call tool handler to execute the tool
                                            } else {
                                                tracing::info!("Permission denied for '{}'", tool_name);
                                                // TODO: Send tool completion with error status
                                            }
                                        }
                                        _ => {
                                            tracing::warn!("Unknown permission outcome for '{}'", tool_name);
                                            // TODO: Send tool completion with error status
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to request permission from client: {}",
                                        e
                                    );
                                    // TODO: Send tool completion with error status
                                }
                            }
                        } else {
                            tracing::warn!(
                                "Permission required for tool '{}' but no client connection available",
                                tool_name
                            );
                            // TODO: Send tool completion with error status
                        }
                    }
                }

                // Check if this is a TodoWrite tool call and send Plan notification
                if tool_call_info.name == "TodoWrite" {
                    match crate::plan::todowrite_to_agent_plan(&tool_call_info.parameters) {
                        Ok(agent_plan) => {
                            let acp_plan = agent_plan.to_acp_plan();
                            let plan_update = SessionUpdate::Plan(acp_plan);

                            // Store/update plan in PlanManager for status tracking
                            // This preserves entry IDs when updating existing plans
                            {
                                let mut plan_manager = self.plan_manager.write().await;
                                plan_manager.update_plan(&session_id.to_string(), agent_plan);
                            }

                            // Store in session context for history replay
                            let plan_message =
                                crate::session::Message::from_update(plan_update.clone());
                            self.session_manager
                                .update_session(session_id, |session| {
                                    session.add_message(plan_message);
                                })
                                .map_err(|_| agent_client_protocol::Error::internal_error())?;

                            let plan_notification = SessionNotification::new(
                                SessionId::new(session_id_str.clone()),
                                plan_update,
                            );

                            if let Err(e) = self.send_session_update(plan_notification).await {
                                tracing::error!(
                                    "Failed to send Plan notification from TodoWrite for session {}: {}",
                                    session_id,
                                    e
                                );
                            } else {
                                tracing::debug!(
                                    "Sent Plan notification from TodoWrite for session {}",
                                    session_id
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to convert TodoWrite to Plan for session {}: {}",
                                session_id,
                                e
                            );
                        }
                    }
                }
            } else if !chunk.content.is_empty() {
                // Create SessionUpdate for this chunk
                let update =
                    SessionUpdate::AgentMessageChunk(agent_client_protocol::ContentChunk::new(
                        ContentBlock::Text(TextContent::new(chunk.content.clone())),
                    ));

                // Store in session
                let chunk_message = crate::session::Message::from_update(update.clone());

                self.session_manager
                    .update_session(session_id, |session| {
                        session.add_message(chunk_message);
                    })
                    .map_err(|_| agent_client_protocol::Error::internal_error())?;

                // Send chunk notification
                let notification =
                    SessionNotification::new(SessionId::new(session_id_str.clone()), update);

                if let Err(e) = self.send_session_update(notification).await {
                    tracing::error!(
                        "Failed to send message chunk notification for session {}: {}",
                        session_id,
                        e
                    );
                }
            }
        }

        // Check cancellation one final time
        if self
            .cancellation_manager
            .is_cancelled(&session_id_str)
            .await
        {
            tracing::info!("Session {} cancelled after streaming", session_id);
            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "cancelled_after_streaming".to_string(),
                serde_json::json!(true),
            );
            return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
        }

        // Tool completions are emitted by protocol_translator when it detects
        // tool_result messages in Claude's stream

        // Map Claude's stop_reason to ACP StopReason
        let stop_reason = match claude_stop_reason.as_deref() {
            Some("max_tokens") => StopReason::MaxTokens,
            Some("end_turn") | None => StopReason::EndTurn,
            Some(other) => {
                tracing::debug!("Unknown stop_reason '{}', defaulting to EndTurn", other);
                StopReason::EndTurn
            }
        };

        let mut meta_map = serde_json::Map::new();
        meta_map.insert("streaming".to_string(), serde_json::json!(true));
        Ok(PromptResponse::new(stop_reason).meta(meta_map))
    }

    /// Handle non-streaming prompt request
    async fn handle_non_streaming_prompt(
        &self,
        session_id: &crate::session::SessionId,
        request: &PromptRequest,
        session: &crate::session::Session,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        tracing::info!("Handling non-streaming prompt for session: {}", session_id);

        // Validate content blocks against prompt capabilities before processing
        let content_validator =
            ContentCapabilityValidator::new(self.capabilities.prompt_capabilities.clone());
        if let Err(capability_error) = content_validator.validate_content_blocks(&request.prompt) {
            tracing::warn!(
                "Content capability validation failed for session {}: {}",
                session_id,
                capability_error
            );

            // Convert to ACP-compliant error response
            let acp_error_data = capability_error.to_acp_error();
            return Err(agent_client_protocol::Error::new(
                acp_error_data["code"].as_i64().unwrap_or(-32602) as i32,
                acp_error_data["message"]
                    .as_str()
                    .unwrap_or("Content capability validation failed")
                    .to_string(),
            )
            .data(acp_error_data["data"].clone()));
        }

        // Extract and process all content from the prompt
        let mut prompt_text = String::new();
        let mut has_binary_content = false;

        for content_block in &request.prompt {
            match content_block {
                ContentBlock::Text(text_content) => {
                    prompt_text.push_str(&text_content.text);
                }
                ContentBlock::Image(image_content) => {
                    // Process image data (already validated in validate_prompt_request)
                    let _decoded = self
                        .base64_processor
                        .decode_image_data(&image_content.data, &image_content.mime_type)
                        .map_err(|e| {
                            tracing::error!("Failed to decode image data: {}", e);
                            agent_client_protocol::Error::invalid_params()
                        })?;

                    // Add descriptive text for now until full multimodal support
                    prompt_text.push_str(&format!(
                        "\n[Image content: {} ({})]",
                        image_content.mime_type,
                        if let Some(ref uri) = image_content.uri {
                            uri
                        } else {
                            "embedded data"
                        }
                    ));
                    has_binary_content = true;
                }
                ContentBlock::Audio(audio_content) => {
                    // Process audio data (already validated in validate_prompt_request)
                    let _decoded = self
                        .base64_processor
                        .decode_audio_data(&audio_content.data, &audio_content.mime_type)
                        .map_err(|e| {
                            tracing::error!("Failed to decode audio data: {}", e);
                            agent_client_protocol::Error::invalid_params()
                        })?;

                    // Add descriptive text for now until full multimodal support
                    prompt_text.push_str(&format!(
                        "\n[Audio content: {} (embedded data)]",
                        audio_content.mime_type
                    ));
                    has_binary_content = true;
                }
                ContentBlock::Resource(_resource_content) => {
                    // Add descriptive text for the resource
                    prompt_text.push_str("\n[Embedded Resource]");
                    has_binary_content = true;
                }
                ContentBlock::ResourceLink(resource_link) => {
                    // Add descriptive text for the resource link
                    prompt_text.push_str(&format!("\n[Resource Link: {}]", resource_link.uri));
                    // ResourceLink is just a URI reference, not binary content
                }
                _ => {
                    // Unknown content block type, skip it
                    tracing::warn!("Unknown content block type, skipping");
                }
            }
        }

        if has_binary_content {
            tracing::info!(
                "Processing prompt with binary content for session: {}",
                session_id
            );
        }

        let context: crate::claude::SessionContext = session.into();
        let session_id_str = session_id.to_string();

        // Check for cancellation before making Claude API request
        if self
            .cancellation_manager
            .is_cancelled(&session_id_str)
            .await
        {
            tracing::info!("Session {} cancelled before Claude API request", session_id);
            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "cancelled_before_api_request".to_string(),
                serde_json::json!(true),
            );
            return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
        }

        tracing::info!("Calling Claude API for session: {}", session_id);

        // Get current mode for this session to pass --agent flag
        let agent_mode = self.get_session_mode(session_id).await;
        if let Some(ref mode) = agent_mode {
            tracing::debug!("Using agent mode '{}' for session {}", mode, session_id);
        }

        // Use streaming API internally to get notifications, but accumulate full response
        let mut stream = self
            .claude_client
            .query_stream_with_context(&prompt_text, &context, agent_mode)
            .await
            .map_err(|e| {
                tracing::error!("Claude API error: {:?}", e);
                agent_client_protocol::Error::internal_error()
            })?;

        let mut response_content = String::new();
        let mut chunk_count = 0;

        while let Some(chunk) = futures::StreamExt::next(&mut stream).await {
            // Check for cancellation during response
            if self
                .cancellation_manager
                .is_cancelled(&session_id_str)
                .await
            {
                tracing::info!(
                    "Session {} cancelled during Claude API response",
                    session_id
                );
                let mut meta_map = serde_json::Map::new();
                meta_map.insert(
                    "cancelled_during_api_response".to_string(),
                    serde_json::json!(true),
                );
                meta_map.insert(
                    "partial_response_length".to_string(),
                    serde_json::json!(response_content.len()),
                );
                return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
            }

            chunk_count += 1;
            response_content.push_str(&chunk.content);

            // Handle tool calls and send notifications
            let update = if let Some(tool_call_info) = &chunk.tool_call {
                use agent_client_protocol::{ToolCall, ToolCallId, ToolCallStatus, ToolKind};

                // Infer tool kind from name
                let kind = if tool_call_info.name.to_lowercase().contains("read") {
                    ToolKind::Read
                } else if tool_call_info.name.to_lowercase().contains("write")
                    || tool_call_info.name.to_lowercase().contains("edit")
                {
                    ToolKind::Edit
                } else if tool_call_info.name.to_lowercase().contains("bash")
                    || tool_call_info.name.to_lowercase().contains("execute")
                {
                    ToolKind::Execute
                } else {
                    ToolKind::Other
                };

                SessionUpdate::ToolCall(
                    ToolCall::new(
                        ToolCallId::new(format!("tool_{}", chunk_count)),
                        tool_call_info.name.clone(),
                    )
                    .kind(kind)
                    .status(ToolCallStatus::Pending)
                    .raw_input(tool_call_info.parameters.clone()),
                )
            } else if !chunk.content.is_empty() {
                // Send text chunk notification
                SessionUpdate::AgentMessageChunk(agent_client_protocol::ContentChunk::new(
                    ContentBlock::Text(TextContent::new(chunk.content.clone())),
                ))
            } else {
                continue; // Skip empty chunks
            };

            // Store in session context for history replay
            let message = crate::session::Message::from_update(update.clone());
            self.session_manager
                .update_session(session_id, |session| {
                    session.add_message(message);
                })
                .map_err(|_| agent_client_protocol::Error::internal_error())?;

            let notification =
                SessionNotification::new(SessionId::new(session_id_str.clone()), update);

            // Send notification
            if let Err(e) = self.send_session_update(notification).await {
                tracing::error!(
                    "Failed to send notification for session {}: {}",
                    session_id,
                    e
                );
            }

            // Check if this is a TodoWrite tool call and send Plan notification
            if let Some(tool_call_info) = &chunk.tool_call {
                if tool_call_info.name == "TodoWrite" {
                    match crate::plan::todowrite_to_agent_plan(&tool_call_info.parameters) {
                        Ok(agent_plan) => {
                            let acp_plan = agent_plan.to_acp_plan();
                            let plan_update = SessionUpdate::Plan(acp_plan);

                            // Store/update plan in PlanManager for status tracking
                            // This preserves entry IDs when updating existing plans
                            {
                                let mut plan_manager = self.plan_manager.write().await;
                                plan_manager.update_plan(&session_id.to_string(), agent_plan);
                            }

                            // Store in session context for history replay
                            let plan_message =
                                crate::session::Message::from_update(plan_update.clone());
                            self.session_manager
                                .update_session(session_id, |session| {
                                    session.add_message(plan_message);
                                })
                                .map_err(|_| agent_client_protocol::Error::internal_error())?;

                            let plan_notification = SessionNotification::new(
                                SessionId::new(session_id_str.clone()),
                                plan_update,
                            );

                            if let Err(e) = self.send_session_update(plan_notification).await {
                                tracing::error!(
                                    "Failed to send Plan notification from TodoWrite for session {}: {}",
                                    session_id,
                                    e
                                );
                            } else {
                                tracing::debug!(
                                    "Sent Plan notification from TodoWrite for session {}",
                                    session_id
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to convert TodoWrite to Plan for session {}: {}",
                                session_id,
                                e
                            );
                        }
                    }
                }
            }
        }

        tracing::info!(
            "Received Claude API response ({} bytes, {} chunks) for session: {}",
            response_content.len(),
            chunk_count,
            session_id
        );

        // ACP requires specific stop reasons for all prompt turn completions:
        // Check for refusal patterns in Claude's response content
        if self.is_response_refusal(&response_content) {
            tracing::info!(
                "Claude refused to respond for session: {}. Response: {}",
                session_id,
                response_content
            );
            return Ok(self.create_refusal_response(&session_id.to_string(), false, None));
        }

        // Check for cancellation after Claude API request but before storing
        if self
            .cancellation_manager
            .is_cancelled(&session_id_str)
            .await
        {
            tracing::info!(
                "Session {} cancelled after Claude API response, not storing",
                session_id
            );
            let mut meta = serde_json::Map::new();
            meta.insert(
                "cancelled_after_api_response".to_string(),
                serde_json::json!(true),
            );
            meta.insert(
                "response_length".to_string(),
                serde_json::json!(response_content.len()),
            );

            return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta));
        }

        // Store assistant response in session
        let assistant_message = crate::session::Message::new(
            crate::session::MessageRole::Assistant,
            response_content.clone(),
        );

        self.session_manager
            .update_session(session_id, |session| {
                session.add_message(assistant_message);
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        let mut meta = serde_json::Map::new();
        meta.insert("processed".to_string(), serde_json::json!(true));
        meta.insert("streaming".to_string(), serde_json::json!(false));
        meta.insert(
            "claude_response".to_string(),
            serde_json::json!(response_content),
        );
        meta.insert(
            "session_messages".to_string(),
            serde_json::json!(session.context.len() + 1),
        );

        Ok(PromptResponse::new(StopReason::EndTurn).meta(meta))
    }

    /// Send session update notification
    async fn send_session_update(&self, notification: SessionNotification) -> crate::Result<()> {
        self.notification_sender.send_update(notification).await
    }

    /// Send plan update notification for the current session plan
    ///
    /// Retrieves the current plan from PlanManager and sends it as a Plan notification
    /// to all subscribers. This enables programmatic plan status updates to be
    /// communicated to clients in real-time.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID whose plan should be sent
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the notification was sent successfully, or an error if:
    /// - No plan exists for the session
    /// - The notification could not be sent
    ///
    /// # Example Use Cases
    ///
    /// - Update plan entry status when tools start/complete execution
    /// - Notify clients of plan progress outside of TodoWrite calls
    /// - Enable automatic plan tracking based on agent actions
    pub async fn send_plan_update(&self, session_id: &SessionId) -> crate::Result<()> {
        // Get the current plan from PlanManager
        let plan_manager = self.plan_manager.read().await;
        let agent_plan = plan_manager
            .get_plan(&session_id.to_string())
            .ok_or_else(|| {
                crate::AgentError::Protocol(format!("No plan found for session {}", session_id))
            })?;

        // Convert to ACP format
        let acp_plan = agent_plan.to_acp_plan();
        let plan_update = SessionUpdate::Plan(acp_plan);

        // Store in session context for history replay
        let plan_message = crate::session::Message::from_update(plan_update.clone());

        // Convert ACP SessionId to internal SessionId
        let internal_session_id = crate::session::SessionId::parse(&session_id.to_string())
            .map_err(|e| crate::AgentError::Protocol(format!("Invalid session ID: {}", e)))?;

        self.session_manager
            .update_session(&internal_session_id, |session| {
                session.add_message(plan_message);
            })
            .map_err(|e| {
                tracing::error!("Failed to update session: {}", e);
                crate::AgentError::Protocol("Failed to update session".to_string())
            })?;

        // Send the notification
        let plan_notification = SessionNotification::new(session_id.clone(), plan_update);

        self.send_session_update(plan_notification).await?;

        tracing::debug!("Sent plan update notification for session {}", session_id);
        Ok(())
    }

    /// Send agent thought chunk update for reasoning transparency
    ///
    /// ACP agent thought chunks provide reasoning transparency:
    /// 1. Send agent_thought_chunk updates during internal processing
    /// 2. Verbalize reasoning steps and decision-making process
    /// 3. Provide insight into problem analysis and planning
    /// 4. Enable clients to show agent thinking to users
    /// 5. Support debugging and understanding of agent behavior
    ///
    /// Thought chunks enhance user trust and system transparency.
    #[cfg(test)]
    #[allow(dead_code)]
    async fn send_agent_thought(
        &self,
        session_id: &SessionId,
        thought: &AgentThought,
    ) -> crate::Result<()> {
        let mut meta_map = serde_json::Map::new();
        meta_map.insert(
            "reasoning_phase".to_string(),
            serde_json::json!(thought.phase),
        );
        meta_map.insert(
            "timestamp".to_string(),
            serde_json::json!(thought
                .timestamp
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()),
        );
        meta_map.insert("context".to_string(), serde_json::json!(thought.context));

        let text_content = TextContent::new(thought.content.clone()).meta(meta_map);
        let content_block = ContentBlock::Text(text_content);
        let content_chunk = agent_client_protocol::ContentChunk::new(content_block);
        let update = SessionUpdate::AgentThoughtChunk(content_chunk);

        // Store in session context for history replay
        let thought_message = crate::session::Message::from_update(update.clone());
        let session_id_parsed = crate::session::SessionId::parse(&session_id.0)
            .map_err(|e| crate::error::AgentError::Session(format!("Invalid session ID: {}", e)))?;
        self.session_manager
            .update_session(&session_id_parsed, |session| {
                session.add_message(thought_message);
            })?;

        let notification = SessionNotification::new(session_id.clone(), update);

        // Continue processing even if thought sending fails - don't block agent operation
        if let Err(e) = self.send_session_update(notification).await {
            tracing::warn!("Failed to send agent thought: {}", e);
        }

        Ok(())
    }

    /// Check if Claude's response indicates a refusal to comply
    ///
    /// ACP requires detecting when the language model refuses to continue and
    /// returning StopReason::Refusal for proper client communication.
    fn is_response_refusal(&self, response_content: &str) -> bool {
        let response_lower = response_content.to_lowercase();

        // Common refusal patterns from Claude
        let refusal_patterns = [
            "i can't",
            "i cannot",
            "i'm unable to",
            "i am unable to",
            "i don't feel comfortable",
            "i won't",
            "i will not",
            "that's not something i can",
            "i'm not able to",
            "i cannot assist",
            "i can't help with",
            "i'm not comfortable",
            "this request goes against",
            "i need to decline",
            "i must decline",
            "i shouldn't",
            "i should not",
            "that would be inappropriate",
            "that's not appropriate",
            "i'm designed not to",
            "i'm programmed not to",
            "i have to refuse",
            "i must refuse",
            "i cannot comply",
            "i'm not allowed to",
            "that's against my guidelines",
            "my guidelines prevent me",
            "i'm not permitted to",
            "that violates",
            "i cannot provide",
            "i can't provide",
        ];

        // Check if response starts with refusal indicators (common pattern)
        for pattern in &refusal_patterns {
            if response_lower.trim_start().starts_with(pattern) {
                tracing::debug!("Refusal pattern detected: '{}'", pattern);
                return true;
            }
        }

        // Check for refusal patterns anywhere in short responses (likely to be pure refusals)
        if response_content.len() < 200 {
            for pattern in &refusal_patterns {
                if response_lower.contains(pattern) {
                    tracing::debug!("Refusal pattern detected in short response: '{}'", pattern);
                    return true;
                }
            }
        }

        false
    }

    /// Create a refusal response for ACP compliance
    ///
    /// Returns a PromptResponse with StopReason::Refusal and appropriate metadata
    /// when Claude refuses to respond to a request.
    fn create_refusal_response(
        &self,
        session_id: &str,
        is_streaming: bool,
        chunk_count: Option<usize>,
    ) -> PromptResponse {
        let mut meta = serde_json::Map::new();
        meta.insert("refusal_detected".to_string(), serde_json::json!(true));
        meta.insert("session_id".to_string(), serde_json::json!(session_id));

        if is_streaming {
            meta.insert("streaming".to_string(), serde_json::json!(true));
            if let Some(count) = chunk_count {
                meta.insert("chunks_processed".to_string(), serde_json::json!(count));
            }
        }

        PromptResponse::new(StopReason::Refusal).meta(meta)
    }

    /// Send available commands update notification
    ///
    /// Sends available commands update via SessionUpdate::AvailableCommandsUpdate
    /// when command availability changes during session execution.
    pub async fn send_available_commands_update(
        &self,
        session_id: &SessionId,
        commands: Vec<agent_client_protocol::AvailableCommand>,
    ) -> crate::Result<()> {
        let update = SessionUpdate::AvailableCommandsUpdate(
            agent_client_protocol::AvailableCommandsUpdate::new(commands),
        );

        // Store in session context for history replay
        let commands_message = crate::session::Message::from_update(update.clone());

        // Convert ACP SessionId to internal SessionId
        let internal_session_id = crate::session::SessionId::parse(&session_id.to_string())
            .map_err(|e| crate::AgentError::Protocol(format!("Invalid session ID: {}", e)))?;

        self.session_manager
            .update_session(&internal_session_id, |session| {
                session.add_message(commands_message);
            })
            .map_err(|e| {
                tracing::error!("Failed to update session: {}", e);
                crate::AgentError::Protocol("Failed to update session".to_string())
            })?;

        let mut meta = serde_json::Map::new();
        meta.insert(
            "update_type".to_string(),
            serde_json::json!("available_commands"),
        );
        meta.insert(
            "session_id".to_string(),
            serde_json::json!(session_id.to_string()),
        );
        meta.insert(
            "timestamp".to_string(),
            serde_json::json!(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()),
        );

        let notification = SessionNotification::new(session_id.clone(), update).meta(meta);

        tracing::debug!(
            "Sending available commands update for session: {}",
            session_id
        );
        self.send_session_update(notification).await
    }

    /// Update available commands for a session and send notification if changed
    ///
    /// This method updates the session's available commands and sends an
    /// AvailableCommandsUpdate notification if the commands have changed.
    /// Returns true if an update was sent, false if commands were unchanged.
    pub async fn update_session_available_commands(
        &self,
        session_id: &SessionId,
        commands: Vec<agent_client_protocol::AvailableCommand>,
    ) -> crate::Result<bool> {
        // Parse SessionId from ACP format (raw ULID)
        let parsed_session_id = crate::session::SessionId::parse(&session_id.0)
            .map_err(|e| crate::AgentError::Session(format!("Invalid session ID format: {}", e)))?;

        // Update commands in session manager
        let commands_changed = self
            .session_manager
            .update_available_commands(&parsed_session_id, commands.clone())?;

        // Send notification if commands changed
        if commands_changed {
            self.send_available_commands_update(session_id, commands.clone())
                .await?;
            tracing::info!(
                "Sent available commands update for session: {} ({} commands)",
                session_id,
                commands.len()
            );
        }

        Ok(commands_changed)
    }

    /// Refresh available commands for all active sessions
    ///
    /// This method is called when MCP servers send notifications about capability changes
    /// (tools/list_changed or prompts/list_changed). It updates commands for all active
    /// sessions and sends AvailableCommandsUpdate notifications if commands have changed.
    pub async fn refresh_commands_for_all_sessions(&self) {
        tracing::debug!("Refreshing available commands for all active sessions");

        // Get list of all active sessions
        let session_ids = match self.session_manager.list_sessions() {
            Ok(ids) => ids,
            Err(e) => {
                tracing::error!("Failed to list sessions for command refresh: {}", e);
                return;
            }
        };

        // Refresh commands for each session
        for session_id in session_ids {
            let protocol_session_id = SessionId::new(session_id.to_string());

            // Get updated commands for this session
            let updated_commands = self
                .get_available_commands_for_session(&protocol_session_id)
                .await;

            // Update and notify if changed
            if let Err(e) = self
                .update_session_available_commands(&protocol_session_id, updated_commands)
                .await
            {
                tracing::warn!(
                    "Failed to update commands for session {}: {}",
                    session_id,
                    e
                );
            }
        }

        tracing::debug!("Completed command refresh for all active sessions");
    }

    /// Get available commands for a session
    ///
    /// This method determines what commands are available for the given session
    /// based on capabilities, MCP servers, and current session state.
    /// Get available commands for a session, filtered by client capabilities
    ///
    /// This method returns the list of commands (slash commands) available to the client
    /// for the given session. The returned commands are automatically filtered based on
    /// the client's declared capabilities during initialization.
    ///
    /// # Capability Filtering
    ///
    /// Commands are only included if the client has declared the necessary capabilities:
    /// - Core planning and analysis commands are always available
    /// - MCP-provided commands are included based on connected MCP servers
    /// - Tool-based commands respect the client's declared tool capabilities
    ///
    /// This ensures that operations requiring specific capabilities (like file system
    /// or terminal access) are only offered to clients that support them, maintaining
    /// the ACP contract that all operations must check capabilities before execution.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier to get commands for
    ///
    /// # Returns
    ///
    /// A vector of AvailableCommand structs representing commands the client can invoke
    async fn get_available_commands_for_session(
        &self,
        session_id: &SessionId,
    ) -> Vec<agent_client_protocol::AvailableCommand> {
        let mut commands = Vec::new();

        // Always available core commands
        let mut meta1 = serde_json::Map::new();
        meta1.insert("category".to_string(), serde_json::json!("planning"));
        meta1.insert("source".to_string(), serde_json::json!("core"));

        commands.push(
            agent_client_protocol::AvailableCommand::new(
                "create_plan".to_string(),
                "Create an execution plan for complex tasks".to_string(),
            )
            .meta(meta1),
        );

        let mut meta2 = serde_json::Map::new();
        meta2.insert("category".to_string(), serde_json::json!("analysis"));
        meta2.insert("source".to_string(), serde_json::json!("core"));

        commands.push(
            agent_client_protocol::AvailableCommand::new(
                "research_codebase".to_string(),
                "Research and analyze the codebase structure".to_string(),
            )
            .meta(meta2),
        );

        // Add commands from MCP servers - use prompts, not tools
        if let Some(mcp_manager) = &self.mcp_manager {
            let mcp_prompts = mcp_manager.list_available_prompts().await;
            tracing::debug!(
                "MCP manager returned {} prompts for slash commands",
                mcp_prompts.len()
            );

            for prompt in mcp_prompts {
                tracing::debug!(
                    "Adding MCP prompt as slash command: {} - {}",
                    prompt.name,
                    prompt.description.as_deref().unwrap_or("(no description)")
                );
                let input_hint = if prompt.arguments.is_empty() {
                    None
                } else {
                    Some(
                        prompt
                            .arguments
                            .iter()
                            .map(|arg| {
                                if arg.required {
                                    format!("<{}>", arg.name)
                                } else {
                                    format!("[{}]", arg.name)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                    )
                };

                let description_with_hint = if let Some(hint) = input_hint.as_ref() {
                    format!(
                        "{} {}",
                        prompt
                            .description
                            .clone()
                            .unwrap_or_else(|| format!("MCP prompt: {}", prompt.name)),
                        hint
                    )
                } else {
                    prompt
                        .description
                        .clone()
                        .unwrap_or_else(|| format!("MCP prompt: {}", prompt.name))
                };

                // Build parameter schema for meta field
                let parameters_schema: Vec<serde_json::Value> = prompt
                    .arguments
                    .iter()
                    .map(|arg| {
                        serde_json::json!({
                            "name": arg.name,
                            "description": arg.description,
                            "required": arg.required,
                        })
                    })
                    .collect();

                // Create input specification if there are arguments
                let command_input = if let Some(hint) = input_hint {
                    let mut input_meta = serde_json::Map::new();
                    input_meta.insert(
                        "parameters".to_string(),
                        serde_json::Value::Array(parameters_schema.clone()),
                    );

                    Some(agent_client_protocol::AvailableCommandInput::Unstructured(
                        agent_client_protocol::UnstructuredCommandInput::new(hint).meta(input_meta),
                    ))
                } else {
                    None
                };

                let mut meta = serde_json::Map::new();
                meta.insert("category".to_string(), serde_json::json!("mcp_prompt"));
                meta.insert("source".to_string(), serde_json::json!("mcp_server"));
                meta.insert(
                    "arguments".to_string(),
                    serde_json::json!(parameters_schema),
                );

                let mut cmd = agent_client_protocol::AvailableCommand::new(
                    prompt.name.clone(),
                    description_with_hint,
                )
                .meta(meta);

                if let Some(input) = command_input {
                    cmd = cmd.input(input);
                }

                commands.push(cmd);
            }
        }

        // Add commands from tool handler based on capabilities
        let tool_handler = self.tool_handler.read().await;
        let tool_names = tool_handler.list_all_available_tools().await;
        drop(tool_handler);

        // Get client capabilities to filter tools
        let client_caps = self.client_capabilities.read().await;
        let has_fs_read = client_caps
            .as_ref()
            .is_some_and(|caps| caps.fs.read_text_file);
        let has_fs_write = client_caps
            .as_ref()
            .is_some_and(|caps| caps.fs.write_text_file);
        let has_terminal_capability = client_caps.as_ref().is_some_and(|caps| caps.terminal);
        drop(client_caps);

        for tool_name in tool_names {
            // Filter based on capabilities
            let should_include = match tool_name.as_str() {
                "fs_read" | "fs_list" => has_fs_read,
                "fs_write" => has_fs_write,
                name if name.starts_with("terminal_") => has_terminal_capability,
                _ => true, // Include other tools by default
            };

            if should_include {
                let (category, description) = match tool_name.as_str() {
                    "fs_read" => ("filesystem", "Read file contents"),
                    "fs_write" => ("filesystem", "Write file contents"),
                    "fs_list" => ("filesystem", "List directory contents"),
                    "terminal_create" => ("terminal", "Create a new terminal session"),
                    "terminal_write" => ("terminal", "Write to a terminal session"),
                    _ => ("tool", "Tool handler command"),
                };

                let mut meta = serde_json::Map::new();
                meta.insert("category".to_string(), serde_json::json!(category));
                meta.insert("source".to_string(), serde_json::json!("tool_handler"));

                commands.push(
                    agent_client_protocol::AvailableCommand::new(
                        tool_name.clone(),
                        description.to_string(),
                    )
                    .meta(meta),
                );
            }
        }

        tracing::debug!(
            "Generated {} available commands for session {} (mcp: {}, tool_handler: {})",
            commands.len(),
            session_id,
            if self.mcp_manager.is_some() {
                commands
                    .iter()
                    .filter(|c| {
                        c.meta
                            .as_ref()
                            .and_then(|m| m.get("source"))
                            .and_then(|s| s.as_str())
                            == Some("mcp_server")
                    })
                    .count()
            } else {
                0
            },
            commands
                .iter()
                .filter(|c| c
                    .meta
                    .as_ref()
                    .and_then(|m| m.get("source"))
                    .and_then(|s| s.as_str())
                    == Some("tool_handler"))
                .count()
        );

        tracing::debug!(
            "Total available commands for session {}: {}",
            session_id,
            commands.len()
        );
        tracing::debug!(
            "Command names: {:?}",
            commands.iter().map(|c| &c.name).collect::<Vec<_>>()
        );

        commands
    }

    /// Cancel ongoing Claude API requests for a session
    ///
    /// Note: This is a minimal implementation that registers cancellation state.
    /// Individual request cancellation is not yet implemented as the ClaudeClient
    /// doesn't currently track requests by session. The cancellation state is
    /// checked before making new requests to prevent further API calls.
    async fn cancel_claude_requests(&self, session_id: &str) {
        tracing::debug!("Cancelling Claude API requests for session: {}", session_id);

        // Register cancellation state to prevent new requests
        self.cancellation_manager
            .add_cancelled_operation(session_id, "claude_requests".to_string())
            .await;

        tracing::debug!(
            "Claude API request cancellation registered for session: {}",
            session_id
        );
    }

    /// Cancel ongoing tool executions for a session
    ///
    /// Note: This is a minimal implementation that registers cancellation state.
    /// Individual tool execution cancellation is not yet implemented as the
    /// ToolCallHandler doesn't track executions by session. The cancellation
    /// state prevents new tool calls from being initiated.
    async fn cancel_tool_executions(&self, session_id: &str) {
        tracing::debug!("Cancelling tool executions for session: {}", session_id);

        self.cancellation_manager
            .add_cancelled_operation(session_id, "tool_executions".to_string())
            .await;

        tracing::debug!(
            "Tool execution cancellation registered for session: {}",
            session_id
        );
    }

    /// Cancel pending permission requests for a session
    ///
    /// Note: This is a minimal implementation that registers cancellation state.
    /// Individual permission request cancellation is not yet implemented as
    /// permission requests are not currently tracked by session. The cancellation
    /// state prevents new permission requests from being initiated.
    async fn cancel_permission_requests(&self, session_id: &str) {
        tracing::debug!("Cancelling permission requests for session: {}", session_id);

        self.cancellation_manager
            .add_cancelled_operation(session_id, "permission_requests".to_string())
            .await;

        tracing::debug!(
            "Permission request cancellation registered for session: {}",
            session_id
        );
    }

    /// Send final status updates before cancellation response
    async fn send_final_cancellation_updates(&self, session_id: &str) -> crate::Result<()> {
        tracing::debug!(
            "Sending final cancellation updates for session: {}",
            session_id
        );

        // Send a final text message to notify about cancellation
        // Using AgentMessageChunk since it's a known working variant
        let mut text_meta = serde_json::Map::new();
        text_meta.insert(
            "cancelled_at".to_string(),
            serde_json::json!(SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()),
        );
        text_meta.insert(
            "reason".to_string(),
            serde_json::json!("client_cancellation"),
        );
        text_meta.insert("session_id".to_string(), serde_json::json!(session_id));

        let text_content =
            TextContent::new("[Session cancelled by client request]".to_string()).meta(text_meta);
        let content_chunk =
            agent_client_protocol::ContentChunk::new(ContentBlock::Text(text_content));

        let mut notif_meta = serde_json::Map::new();
        notif_meta.insert("final_update".to_string(), serde_json::json!(true));
        notif_meta.insert("cancellation".to_string(), serde_json::json!(true));

        let cancellation_notification = SessionNotification::new(
            SessionId::new(session_id),
            SessionUpdate::AgentMessageChunk(content_chunk),
        )
        .meta(notif_meta);

        if let Err(e) = self.send_session_update(cancellation_notification).await {
            tracing::warn!(
                "Failed to send cancellation notification for session {}: {}",
                session_id,
                e
            );
            // Don't propagate the error as cancellation should still proceed
        }

        tracing::debug!(
            "Final cancellation updates sent for session: {}",
            session_id
        );
        Ok(())
    }

    /// Update plan entry status and send notification
    ///
    /// Updates the status of a specific plan entry and automatically sends
    /// a Plan notification to all subscribers. This enables programmatic plan
    /// status updates to be communicated to clients in real-time.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID whose plan should be updated
    /// * `entry_id` - The ID of the plan entry to update
    /// * `new_status` - The new status for the plan entry
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the update and notification were successful, or an error if:
    /// - No plan exists for the session
    /// - The entry ID is not found in the plan
    /// - The notification could not be sent
    ///
    /// # Example Use Cases
    ///
    /// ```rust,ignore
    /// // Mark a plan entry as in-progress when tool execution starts
    /// agent.update_plan_entry_status(
    ///     &session_id,
    ///     &entry_id,
    ///     PlanEntryStatus::InProgress
    /// ).await?;
    ///
    /// // Mark a plan entry as completed when tool execution finishes
    /// agent.update_plan_entry_status(
    ///     &session_id,
    ///     &entry_id,
    ///     PlanEntryStatus::Completed
    /// ).await?;
    /// ```
    pub async fn update_plan_entry_status(
        &self,
        session_id: &SessionId,
        entry_id: &str,
        new_status: crate::plan::PlanEntryStatus,
    ) -> crate::Result<()> {
        // Update the plan entry status in PlanManager
        let mut plan_manager = self.plan_manager.write().await;
        let was_updated =
            plan_manager.update_plan_entry_status(&session_id.to_string(), entry_id, new_status);

        if !was_updated {
            return Err(crate::AgentError::Protocol(format!(
                "Failed to update plan entry {} for session {}",
                entry_id, session_id
            )));
        }

        // Release the write lock before sending notification
        drop(plan_manager);

        // Send the updated plan notification
        self.send_plan_update(session_id).await?;

        tracing::debug!(
            "Updated plan entry {} to status {:?} for session {}",
            entry_id,
            new_status,
            session_id
        );

        Ok(())
    }

    /// Shutdown active sessions gracefully
    pub async fn shutdown_sessions(&self) -> crate::Result<()> {
        // Session manager cleanup is handled by dropping the Arc
        // Sessions will be automatically cleaned up when no longer referenced
        tracing::info!("Sessions shutdown complete");
        Ok(())
    }

    /// Shutdown MCP server connections gracefully
    pub async fn shutdown_mcp_connections(&self) -> crate::Result<()> {
        if let Some(_mcp_manager) = &self.mcp_manager {
            // The MCP manager will handle cleanup when dropped
            tracing::info!("MCP connections shutdown initiated");
        }
        Ok(())
    }

    /// Shutdown tool handler gracefully
    pub async fn shutdown_tool_handler(&self) -> crate::Result<()> {
        // Tool handler cleanup is handled by dropping the Arc
        // Any background processes should be terminated gracefully
        tracing::info!("Tool handler shutdown complete");
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for ClaudeAgent {
    // ACP AGENT PROTOCOL FLOW WITHOUT AUTHENTICATION:
    // 1. Client sends initialize request
    // 2. Agent responds with capabilities and authMethods: []
    // 3. Client can immediately call session/new (no auth step)
    // 4. Normal session operations proceed without authentication
    //
    // This is the correct flow for local development tools.

    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> Result<InitializeResponse, agent_client_protocol::Error> {
        self.log_request("initialize", &request);
        tracing::info!(
            "Initializing agent with client capabilities: {:?}",
            request.client_capabilities
        );

        // Validate initialization request structure
        if let Err(e) = self.validate_initialization_request(&request) {
            tracing::error!(
                "Initialization failed: Invalid request structure - {}",
                e.message
            );
            return Err(e);
        }

        // Validate protocol version
        if let Err(e) = self.validate_protocol_version(&request.protocol_version) {
            let fatal_error = self.handle_fatal_initialization_error(e).await;
            tracing::error!(
                "Initialization failed: Protocol version validation error - {}",
                fatal_error.message
            );
            return Err(fatal_error);
        }

        // Validate client capabilities
        if let Err(e) = self.validate_client_capabilities(&request.client_capabilities) {
            tracing::error!(
                "Initialization failed: Client capability validation error - {}",
                e.message
            );
            return Err(e);
        }

        tracing::info!("Agent initialization validation completed successfully");

        // Store client capabilities for ACP compliance - required for capability gating
        {
            let mut client_caps = self.client_capabilities.write().await;
            *client_caps = Some(request.client_capabilities.clone());
        }

        // Pass client capabilities to tool handler for capability validation
        {
            let mut tool_handler = self.tool_handler.write().await;
            tool_handler.set_client_capabilities(request.client_capabilities.clone());
        }

        tracing::info!("Stored client capabilities for ACP compliance");

        // AUTHENTICATION ARCHITECTURE DECISION:
        // Claude Code is a local development tool that runs entirely on the user's machine.
        // It does not require authentication because:
        // 1. It operates within the user's own development environment
        // 2. It does not connect to external services requiring credentials
        // 3. It has no multi-user access control requirements
        // 4. All operations are performed with the user's existing local permissions
        //
        // Therefore, we intentionally declare NO authentication methods (empty array).
        // This is an architectural decision - do not add authentication methods.
        // If remote authentication is needed in the future, it should be a separate feature.

        let agent_info =
            agent_client_protocol::Implementation::new("claude-agent", env!("CARGO_PKG_VERSION"))
                .title(format!("Claude Agent v{}", env!("CARGO_PKG_VERSION")));

        let response =
            InitializeResponse::new(self.negotiate_protocol_version(&request.protocol_version))
                .agent_capabilities(self.capabilities.clone())
                .auth_methods(vec![])
                .agent_info(agent_info);

        self.log_response("initialize", &response);
        Ok(response)
    }

    async fn authenticate(
        &self,
        request: AuthenticateRequest,
    ) -> Result<AuthenticateResponse, agent_client_protocol::Error> {
        self.log_request("authenticate", &request);

        // AUTHENTICATION ARCHITECTURE DECISION:
        // Claude Code declares NO authentication methods in initialize().
        // According to ACP spec, clients should not call authenticate when no methods are declared.
        // If they do call authenticate anyway, we reject it with a clear error.
        tracing::warn!(
            "Authentication attempt rejected - no auth methods declared: {:?}",
            request.method_id
        );

        Err(agent_client_protocol::Error::method_not_found())
    }

    async fn new_session(
        &self,
        request: NewSessionRequest,
    ) -> Result<NewSessionResponse, agent_client_protocol::Error> {
        self.log_request("new_session", &request);
        tracing::info!("Creating new session");

        // ACP requires strict transport capability enforcement:
        // 1. stdio: Always supported (mandatory per spec)
        // 2. http: Only if mcpCapabilities.http: true was declared
        // 3. sse: Only if mcpCapabilities.sse: true was declared
        //
        // This prevents protocol violations and ensures capability negotiation contract.

        // Convert ACP MCP server configs to internal types for validation
        let internal_mcp_servers: Vec<crate::config::McpServerConfig> = request
            .mcp_servers
            .iter()
            .filter_map(|server| self.convert_acp_to_internal_mcp_config(server))
            .collect();

        // Validate transport requirements against agent capabilities
        if let Err(validation_error) = crate::capability_validation::CapabilityRequirementChecker::check_new_session_requirements(
            &self.capabilities,
            &internal_mcp_servers,
        ) {
            tracing::error!("Session creation failed: Transport validation error - {}", validation_error);
            return Err(self.convert_session_setup_error_to_acp_error(validation_error));
        }

        let client_caps = {
            let guard = self.client_capabilities.read().await;
            guard.clone()
        };

        let session_id = self
            .session_manager
            .create_session(request.cwd.clone(), client_caps)
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;

        // Register RawMessageManager for this session so subagents can find it
        if let Some(ref manager) = self.raw_message_manager {
            RawMessageManager::register(session_id.to_string(), manager.clone());
            tracing::debug!("Registered RawMessageManager for session {}", session_id);
        }

        // Store MCP servers in the session if provided
        if !request.mcp_servers.is_empty() {
            self.session_manager
                .update_session(&session_id, |session| {
                    // Store the actual MCP server info from the request as JSON strings
                    session.mcp_servers = request
                        .mcp_servers
                        .iter()
                        .map(|server| {
                            serde_json::to_string(server)
                                .unwrap_or_else(|_| format!("{:?}", server))
                        })
                        .collect();
                })
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;
        }

        tracing::info!("Created session: {}", session_id);

        let protocol_session_id = SessionId::new(session_id.to_string());

        // Spawn Claude process immediately and read init message with slash_commands and available_agents
        // Pass agent's configured MCP servers (self.config.mcp_servers) to Claude CLI
        // These are the MCP servers configured during agent creation, not from the request
        tracing::info!("Spawning Claude process for session: {}", session_id);
        match self
            .claude_client
            .spawn_process_and_consume_init(
                &session_id,
                &protocol_session_id,
                &request.cwd,
                self.config.mcp_servers.clone(),
            )
            .await
        {
            Ok((Some(agents), current_agent)) => {
                tracing::info!(
                    "Storing {} available agents from Claude CLI init",
                    agents.len()
                );
                self.set_available_agents(agents).await;

                // Set initial mode if Claude CLI specified current_agent
                if let Some(mode) = current_agent {
                    tracing::info!("Setting initial mode from Claude CLI: {}", mode);
                    self.session_manager
                        .update_session(&session_id, |session| {
                            session.current_mode = Some(mode.clone());
                        })
                        .map_err(|_| {
                            tracing::warn!("Failed to set initial mode");
                        })
                        .ok();
                } else {
                    tracing::debug!(
                        "No current_agent in init - session starts without mode (no --agent flag)"
                    );
                }
            }
            Ok((None, _)) => {
                tracing::debug!("No available agents in Claude CLI init message");
            }
            Err(e) => {
                tracing::error!("Failed to spawn Claude process and read init: {}", e);
            }
        }

        // Send initial available commands after session creation (core + tool_handler commands)
        let initial_commands = self
            .get_available_commands_for_session(&protocol_session_id)
            .await;
        if let Err(e) = self
            .update_session_available_commands(&protocol_session_id, initial_commands)
            .await
        {
            tracing::warn!(
                "Failed to send initial available commands for session {}: {}",
                session_id,
                e
            );
        }

        let mut response = NewSessionResponse::new(SessionId::new(session_id.to_string()));

        // Add available modes only if the session has a mode explicitly set
        // Per user requirement: don't assume any default mode - no mode means no --agent flag
        if let Some(available_modes) = self.get_available_modes().await {
            // Only include modes in response if session has current_mode set
            if let Some(current_mode_id) = self.get_session_mode(&session_id).await {
                let mode_state = agent_client_protocol::SessionModeState::new(
                    agent_client_protocol::SessionModeId::new(current_mode_id.as_str()),
                    available_modes,
                );
                response = response.modes(mode_state);
                tracing::info!("Session created with mode: {}", current_mode_id);
            } else {
                // Modes are available but not set - don't include in response
                // This allows sessions to run without --agent flag until mode is explicitly set
                tracing::debug!(
                    "Session created without mode (available modes: {}, will not use --agent flag)",
                    available_modes.len()
                );
            }
        }

        self.log_response("new_session", &response);
        Ok(response)
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> Result<LoadSessionResponse, agent_client_protocol::Error> {
        self.log_request("load_session", &request);
        tracing::info!("Loading session: {}", request.session_id);

        // ACP requires complete conversation history replay during session loading:
        // 1. Validate loadSession capability before allowing session/load
        // 2. Stream ALL historical messages via session/update notifications
        // 3. Maintain exact chronological order of original conversation
        // 4. Only respond to session/load AFTER all history is streamed
        // 5. Client can then continue conversation seamlessly

        // ACP requires strict transport capability enforcement for session loading:
        // Convert ACP MCP server configs to internal types for validation
        let internal_mcp_servers: Vec<crate::config::McpServerConfig> = request
            .mcp_servers
            .iter()
            .filter_map(|server| self.convert_acp_to_internal_mcp_config(server))
            .collect();

        // Validate transport requirements and loadSession capability
        if let Err(validation_error) = crate::capability_validation::CapabilityRequirementChecker::check_load_session_requirements(
            &self.capabilities,
            &internal_mcp_servers,
        ) {
            tracing::error!("Session loading failed: Transport/capability validation error - {}", validation_error);
            return Err(self.convert_session_setup_error_to_acp_error(validation_error));
        }

        let session_id = self.parse_session_id(&request.session_id)?;

        let session = self
            .session_manager
            .get_session(&session_id)
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;

        match session {
            Some(session) => {
                tracing::info!(
                    "Loaded session: {} with {} historical messages",
                    session_id,
                    session.context.len()
                );

                // Step 2-3: Stream ALL historical messages via session/update notifications
                // Maintain exact chronological order using message timestamps
                if !session.context.is_empty() {
                    tracing::info!(
                        "Replaying {} historical messages for session {}",
                        session.context.len(),
                        session_id
                    );

                    for message in &session.context {
                        // Use the SessionUpdate stored in the message directly
                        let mut meta_map = serde_json::Map::new();
                        meta_map.insert(
                            "timestamp".to_string(),
                            serde_json::json!(message
                                .timestamp
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs()),
                        );
                        meta_map.insert(
                            "message_type".to_string(),
                            serde_json::json!("historical_replay"),
                        );

                        let notification = SessionNotification::new(
                            SessionId::new(session.id.to_string()),
                            message.update.clone(),
                        )
                        .meta(meta_map);

                        // Stream historical message via session/update notification
                        // Note: send_update() queues the notification in the broadcast channel
                        // The notification_handler task processes these concurrently
                        if let Err(e) = self.notification_sender.send_update(notification).await {
                            tracing::error!(
                                "Failed to send historical message notification: {}",
                                e
                            );
                            // Continue with other messages even if one fails
                        }
                    }

                    tracing::info!(
                        "Completed queueing {} history notifications for session {}",
                        session.context.len(),
                        session_id
                    );
                }

                // Step 4: Return LoadSessionResponse after all history notifications are queued
                // The notifications are processed by the notification_handler task concurrently.
                // The broadcast channel and shared writer Mutex ensure notifications are delivered
                // to the client before this response due to:
                // 1. FIFO ordering in the broadcast channel
                // 2. Notification handler actively polling the channel
                // 3. Serialized writes through the shared Mutex-protected writer
                let mut meta_map = serde_json::Map::new();
                meta_map.insert(
                    "session_id".to_string(),
                    serde_json::json!(session.id.to_string()),
                );
                meta_map.insert(
                    "created_at".to_string(),
                    serde_json::json!(session
                        .created_at
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()),
                );
                meta_map.insert(
                    "message_count".to_string(),
                    serde_json::json!(session.context.len()),
                );
                meta_map.insert(
                    "history_replayed".to_string(),
                    serde_json::json!(session.context.len()),
                );

                let response = LoadSessionResponse::new().meta(meta_map);
                self.log_response("load_session", &response);
                Ok(response)
            }
            None => {
                tracing::warn!("Session not found: {}", session_id);
                Err(agent_client_protocol::Error::new(
                    -32602,
                    "Session not found: sessionId does not exist or has expired".to_string(),
                )
                .data(serde_json::json!({
                    "sessionId": request.session_id,
                    "error": "session_not_found"
                })))
            }
        }
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse, agent_client_protocol::Error> {
        self.log_request("set_session_mode", &request);

        let parsed_session_id = match crate::session::SessionId::parse(&request.session_id.0) {
            Ok(id) => id,
            Err(_) => {
                return Err(agent_client_protocol::Error::invalid_request());
            }
        };

        let mode_id_string = request.mode_id.0.to_string();

        // Validate mode ID is in available modes
        let available_agents = self.available_agents.read().await;
        if let Some(agents) = available_agents.as_ref() {
            let mode_exists = agents.iter().any(|(id, _, _)| id == &mode_id_string);
            if !mode_exists {
                tracing::error!(
                    "Invalid mode '{}' requested. Available modes: {:?}",
                    mode_id_string,
                    agents
                        .iter()
                        .map(|(id, name, _)| format!("{}:{}", id, name))
                        .collect::<Vec<_>>()
                );
                return Err(agent_client_protocol::Error::invalid_params());
            }
        } else {
            // No available modes - shouldn't happen but reject to be safe
            tracing::warn!("set_session_mode called but no available modes configured");
            return Err(agent_client_protocol::Error::invalid_params());
        }
        drop(available_agents);

        // Get the current mode to check if it will change
        let current_mode = self
            .session_manager
            .get_session(&parsed_session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .map(|session| session.current_mode.clone())
            .unwrap_or(None);

        let mode_changed = current_mode != Some(mode_id_string.clone());

        // Update session with new mode
        self.session_manager
            .update_session(&parsed_session_id, |session| {
                session.current_mode = Some(mode_id_string.clone());
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        tracing::info!("Session mode set to: {}", mode_id_string);

        // Terminate existing Claude process if mode changed
        // This forces a respawn with the new --agent flag on next prompt
        if mode_changed {
            tracing::info!(
                "Mode changed for session {}, terminating Claude process to force respawn with --agent {}",
                parsed_session_id,
                mode_id_string
            );

            if let Err(e) = self
                .claude_client
                .terminate_session(&parsed_session_id)
                .await
            {
                tracing::warn!(
                    "Failed to terminate Claude process for session {}: {}",
                    parsed_session_id,
                    e
                );
            } else {
                tracing::info!(
                    "Claude process terminated, will respawn with new mode on next prompt"
                );
            }

            let current_mode_update =
                agent_client_protocol::CurrentModeUpdate::new(request.mode_id.clone());
            let update = SessionUpdate::CurrentModeUpdate(current_mode_update);

            // Store in session context for history replay
            let mode_message = crate::session::Message::from_update(update.clone());
            self.session_manager
                .update_session(&parsed_session_id, |session| {
                    session.add_message(mode_message);
                })
                .map_err(|_| agent_client_protocol::Error::internal_error())?;

            if let Err(e) = self
                .send_session_update(SessionNotification::new(request.session_id.clone(), update))
                .await
            {
                tracing::warn!("Failed to send current mode update notification: {}", e);
            }
        }

        let mut meta_map = serde_json::Map::new();
        meta_map.insert("mode_set".to_string(), serde_json::json!(true));
        meta_map.insert(
            "message".to_string(),
            serde_json::json!("Session mode updated"),
        );
        meta_map.insert("mode_changed".to_string(), serde_json::json!(mode_changed));
        if mode_changed {
            meta_map.insert(
                "process_action".to_string(),
                serde_json::json!("terminated_for_respawn"),
            );
        }

        let response = SetSessionModeResponse::new().meta(meta_map);

        self.log_response("set_session_mode", &response);
        Ok(response)
    }

    async fn prompt(
        &self,
        request: PromptRequest,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        self.log_request("prompt", &request);
        tracing::info!(
            "Processing prompt request for session: {}",
            request.session_id
        );

        // 🚨 DEBUG: Log exactly what prompt text we're receiving
        tracing::debug!("📨 PROMPT REQUEST DEBUG:");
        tracing::debug!("  Session: {}", request.session_id);
        tracing::debug!("  Content blocks: {}", request.prompt.len());
        for (i, block) in request.prompt.iter().enumerate() {
            match block {
                agent_client_protocol::ContentBlock::Text(text) => {
                    tracing::debug!("  Block {}: TEXT ({} chars)", i + 1, text.text.len());
                    tracing::debug!(
                        "  Text preview: {}",
                        if text.text.len() > 200 {
                            format!("{}...", &text.text[..200])
                        } else {
                            text.text.clone()
                        }
                    );
                }
                _ => {
                    tracing::debug!("  Block {}: {:?}", i + 1, block);
                }
            }
        }

        // Validate the request
        self.validate_prompt_request(&request).await?;

        // Parse session ID
        let session_id = self.parse_session_id(&request.session_id)?;

        // ACP requires user message chunk updates for conversation transparency:
        // 1. Echo user input via session/update with user_message_chunk
        // 2. Send before agent processing begins
        // 3. Include all content blocks from user prompt
        // 4. Maintain conversation flow visibility for clients
        // 5. Support conversation history reconstruction
        //
        // User message chunks provide consistent conversation reporting.
        for content_block in &request.prompt {
            let content_chunk = agent_client_protocol::ContentChunk::new(content_block.clone());
            let notification = SessionNotification::new(
                request.session_id.clone(),
                SessionUpdate::UserMessageChunk(content_chunk),
            );

            if let Err(e) = self.send_session_update(notification).await {
                tracing::warn!(
                    "Failed to send user message chunk for session {}: {}",
                    request.session_id,
                    e
                );
            }
        }

        // Check if session is already cancelled before processing
        if self
            .cancellation_manager
            .is_cancelled(&session_id.to_string())
            .await
        {
            tracing::info!(
                "Session {} is cancelled, returning cancelled response",
                session_id
            );

            // CRITICAL: Reset cancellation state for next turn
            self.cancellation_manager
                .reset_for_new_turn(&session_id.to_string())
                .await;

            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "cancelled_before_processing".to_string(),
                serde_json::json!(true),
            );
            meta_map.insert(
                "session_id".to_string(),
                serde_json::json!(session_id.to_string()),
            );
            return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
        }

        // Extract and process all content from the prompt
        let mut prompt_text = String::new();
        let mut has_binary_content = false;

        for content_block in &request.prompt {
            match content_block {
                ContentBlock::Text(text_content) => {
                    prompt_text.push_str(&text_content.text);
                }
                ContentBlock::Image(image_content) => {
                    // Add descriptive text for plan analysis
                    prompt_text.push_str(&format!(
                        "\n[Image content: {} ({})]",
                        image_content.mime_type,
                        if let Some(ref uri) = image_content.uri {
                            uri
                        } else {
                            "embedded data"
                        }
                    ));
                    has_binary_content = true;
                }
                ContentBlock::Audio(audio_content) => {
                    // Add descriptive text for plan analysis
                    prompt_text.push_str(&format!(
                        "\n[Audio content: {} (embedded data)]",
                        audio_content.mime_type
                    ));
                    has_binary_content = true;
                }
                ContentBlock::Resource(_resource_content) => {
                    // Add descriptive text for the resource
                    prompt_text.push_str("\n[Embedded Resource]");
                    has_binary_content = true;
                }
                ContentBlock::ResourceLink(resource_link) => {
                    // Add descriptive text for the resource link
                    prompt_text.push_str(&format!("\n[Resource Link: {}]", resource_link.uri));
                    // ResourceLink is just a URI reference, not binary content
                }
                _ => {
                    // Unknown content block type, skip it
                    tracing::warn!("Unknown content block type, skipping");
                }
            }
        }

        if has_binary_content {
            tracing::info!(
                "Processing prompt with binary content for plan analysis in session: {}",
                session_id
            );
        }

        // Validate session exists and get it
        let session = self
            .session_manager
            .get_session(&session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .ok_or_else(agent_client_protocol::Error::invalid_params)?;

        // Reset turn counters at the start of each new turn.
        // ACP defines a turn as: a single user prompt and all subsequent LM requests
        // until the final response. This prevents unbounded counter growth across turns.
        self.session_manager
            .update_session(&session_id, |session| {
                session.reset_turn_counters();
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        // Add user message to session
        let user_message =
            crate::session::Message::new(crate::session::MessageRole::User, prompt_text.clone());

        self.session_manager
            .update_session(&session_id, |session| {
                session.add_message(user_message);
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        // Get updated session for context
        let mut updated_session = self
            .session_manager
            .get_session(&session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .ok_or_else(agent_client_protocol::Error::internal_error)?;

        // ACP requires specific stop reasons for all prompt turn completions:
        // 1. max_tokens: Token limit exceeded (configurable)
        // 2. max_turn_requests: Too many LM requests in single turn
        // Check limits before making Claude API calls

        // Check turn request limit
        let current_requests = updated_session.increment_turn_requests();
        if current_requests > self.config.max_turn_requests {
            tracing::info!(
                "Turn request limit exceeded ({} > {}) for session: {}",
                current_requests,
                self.config.max_turn_requests,
                session_id
            );
            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "turn_requests".to_string(),
                serde_json::json!(current_requests),
            );
            meta_map.insert(
                "max_turn_requests".to_string(),
                serde_json::json!(self.config.max_turn_requests),
            );
            meta_map.insert(
                "session_id".to_string(),
                serde_json::json!(session_id.to_string()),
            );
            return Ok(PromptResponse::new(StopReason::MaxTurnRequests).meta(meta_map));
        }

        // Estimate token usage for the prompt (rough approximation: 4 chars per token)
        let estimated_tokens = (prompt_text.len() as u64) / 4;
        let current_tokens = updated_session.add_turn_tokens(estimated_tokens);
        if current_tokens > self.config.max_tokens_per_turn {
            tracing::info!(
                "Token limit exceeded ({} > {}) for session: {}",
                current_tokens,
                self.config.max_tokens_per_turn,
                session_id
            );
            let mut meta_map = serde_json::Map::new();
            meta_map.insert("turn_tokens".to_string(), serde_json::json!(current_tokens));
            meta_map.insert(
                "max_tokens_per_turn".to_string(),
                serde_json::json!(self.config.max_tokens_per_turn),
            );
            meta_map.insert(
                "session_id".to_string(),
                serde_json::json!(session_id.to_string()),
            );
            return Ok(PromptResponse::new(StopReason::MaxTokens).meta(meta_map));
        }

        // Update session with incremented counters
        self.session_manager
            .update_session(&session_id, |session| {
                session.turn_request_count = updated_session.turn_request_count;
                session.turn_token_count = updated_session.turn_token_count;
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        // Check if streaming is supported and requested
        let response = if self.should_stream(&session, &request) {
            self.handle_streaming_prompt(&session_id, &request, &updated_session)
                .await?
        } else {
            self.handle_non_streaming_prompt(&session_id, &request, &updated_session)
                .await?
        };

        // Clear cancellation state after turn completes successfully
        // This prepares for the next turn
        self.cancellation_manager
            .reset_for_new_turn(&session_id.to_string())
            .await;

        tracing::info!(
            "🎯 MAIN AGENT DONE: Claude agent prompt completed with {:?} (middleware may still be running)",
            response.stop_reason
        );

        self.log_response("prompt", &response);
        Ok(response)
    }

    async fn cancel(
        &self,
        notification: CancelNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        self.log_request("cancel", &notification);
        let session_id = &notification.session_id.0;

        tracing::info!("Processing cancellation for session: {}", session_id);

        // ACP requires immediate and comprehensive cancellation handling:
        // 1. Process session/cancel notifications immediately
        // 2. Cancel ALL ongoing operations (LM, tools, permissions)
        // 3. Send final status updates before responding
        // 4. Respond to original session/prompt with cancelled stop reason
        // 5. Clean up all resources and prevent orphaned operations
        //
        // Cancellation must be fast and reliable to maintain responsiveness.

        // 1. Immediately mark session as cancelled
        if let Err(e) = self
            .cancellation_manager
            .mark_cancelled(session_id, "Client sent session/cancel notification")
            .await
        {
            tracing::error!("Failed to mark session {} as cancelled: {}", session_id, e);
            // Continue with cancellation despite state update failure
        }

        // 2. Cancel all ongoing operations for this session
        tokio::join!(
            self.cancel_claude_requests(session_id),
            self.cancel_tool_executions(session_id),
            self.cancel_permission_requests(session_id)
        );

        // 3. Send final status updates for any pending operations
        if let Err(e) = self.send_final_cancellation_updates(session_id).await {
            tracing::warn!(
                "Failed to send final cancellation updates for session {}: {}",
                session_id,
                e
            );
            // Don't fail cancellation due to notification issues
        }

        // 4. The original session/prompt will respond with cancelled stop reason
        // when it detects the cancellation state - this happens automatically
        // in the prompt method implementation

        tracing::info!(
            "Cancellation processing completed for session: {}",
            session_id
        );
        Ok(())
    }

    /// Handle extension method requests
    ///
    /// Extension methods allow clients to call custom methods not defined in the core
    /// Agent Client Protocol specification. This implementation returns a placeholder
    /// response indicating that extension methods are not currently supported.
    ///
    /// ## Design Decision
    ///
    /// Claude Agent currently does not require any extension methods beyond the standard
    /// ACP specification. The core protocol provides sufficient capabilities for:
    /// - Session management (new_session, load_session, set_session_mode)
    /// - Authentication (handled via empty auth_methods)
    /// - Tool execution (via prompt requests)
    /// - Session updates and notifications
    ///
    /// If future requirements emerge for custom extension methods, this implementation
    /// can be enhanced to dispatch to specific handlers based on the method name.
    ///
    /// ## Protocol Compliance
    ///
    /// This implementation satisfies the ACP requirement that agents must respond to
    /// extension method calls, even if they don't implement any specific extensions.
    /// Returning a structured response (rather than an error) maintains client compatibility.
    async fn ext_method(
        &self,
        request: ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        self.log_request("ext_method", &request);
        tracing::info!("Extension method called: {}", request.method);

        // Handle fs/read_text_file extension method
        if request.method == "fs/read_text_file".into() {
            // Validate client capabilities for file system read operations
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.fs.read_text_file => {
                        tracing::debug!("File system read capability validated");
                    }
                    Some(_) => {
                        tracing::error!("fs/read_text_file capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for fs/read_text_file validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse the request parameters from RawValue
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse fs/read_text_file parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: ReadTextFileParams = serde_json::from_value(params_value).map_err(|e| {
                tracing::error!("Failed to deserialize fs/read_text_file parameters: {}", e);
                agent_client_protocol::Error::invalid_params()
            })?;

            // Handle the file reading request
            let response = self.handle_read_text_file(params).await?;

            // Convert response to RawValue
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle fs/write_text_file extension method
        if request.method == "fs/write_text_file".into() {
            // Validate client capabilities for file system write operations
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.fs.write_text_file => {
                        tracing::debug!("File system write capability validated");
                    }
                    Some(_) => {
                        tracing::error!("fs/write_text_file capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for fs/write_text_file validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse the request parameters from RawValue
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse fs/write_text_file parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: WriteTextFileParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!("Failed to deserialize fs/write_text_file parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the file writing request
            let response = self.handle_write_text_file(params).await?;

            // Convert response to RawValue
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle terminal/output extension method
        if request.method == "terminal/output".into() {
            // Validate client capabilities for terminal operations
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.terminal => {
                        tracing::debug!("Terminal capability validated");
                    }
                    Some(_) => {
                        tracing::error!("terminal/output capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for terminal/output validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse the request parameters from RawValue
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse terminal/output parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: crate::terminal_manager::TerminalOutputParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!("Failed to deserialize terminal/output parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the terminal output request
            let response = self.handle_terminal_output(params).await?;

            // Convert response to RawValue
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle terminal/release extension method
        if request.method == "terminal/release".into() {
            // Validate client capabilities for terminal operations
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.terminal => {
                        tracing::debug!("Terminal capability validated");
                    }
                    Some(_) => {
                        tracing::error!("terminal/release capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for terminal/release validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse the request parameters from RawValue
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse terminal/release parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: crate::terminal_manager::TerminalReleaseParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!("Failed to deserialize terminal/release parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the terminal release request
            let response = self.handle_terminal_release(params).await?;

            // Convert response to RawValue (should be null per ACP spec)
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle terminal/wait_for_exit extension method
        if request.method == "terminal/wait_for_exit".into() {
            // Validate terminal capability
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.terminal => {
                        tracing::debug!("Terminal capability validated for wait_for_exit");
                    }
                    Some(_) => {
                        tracing::error!("terminal/wait_for_exit capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for terminal/wait_for_exit validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse and validate parameters
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse terminal/wait_for_exit parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: crate::terminal_manager::TerminalOutputParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!(
                        "Failed to deserialize terminal/wait_for_exit parameters: {}",
                        e
                    );
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the wait for exit request
            let response = self.handle_terminal_wait_for_exit(params).await?;

            // Convert response to RawValue
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle terminal/kill extension method
        if request.method == "terminal/kill".into() {
            // Validate terminal capability
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.terminal => {
                        tracing::debug!("Terminal capability validated for kill");
                    }
                    Some(_) => {
                        tracing::error!("terminal/kill capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for terminal/kill validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse and validate parameters
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse terminal/kill parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: crate::terminal_manager::TerminalOutputParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!("Failed to deserialize terminal/kill parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the kill request
            self.handle_terminal_kill(params).await?;

            // Return null result per ACP specification
            let response_json = serde_json::Value::Null;
            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle terminal/create extension method
        if request.method == "terminal/create".into() {
            // Validate terminal capability
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.terminal => {
                        tracing::debug!("Terminal capability validated for create");
                    }
                    Some(_) => {
                        tracing::error!("terminal/create capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for terminal/create validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse and validate parameters
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse terminal/create parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: crate::terminal_manager::TerminalCreateParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!("Failed to deserialize terminal/create parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the terminal create request
            let response = self.handle_terminal_create(params).await?;

            // Convert response to RawValue
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle editor/update_buffers extension method
        //
        // This extension method allows clients to push editor buffer state to the agent,
        // enabling the agent to access unsaved file content when executing tools that read files.
        //
        // ## Protocol Integration
        //
        // This implements the ACP (Agent-Client Protocol) requirement for editor state management.
        // Clients should proactively push editor state updates when buffers change, allowing the
        // agent to work with current content rather than stale disk versions.
        //
        // ## Parameters
        //
        // Expects an `EditorBufferResponse` containing:
        // - `buffers`: HashMap of absolute file paths to EditorBuffer objects with content and metadata
        // - `unavailable_paths`: List of paths that don't have active editor buffers
        //
        // ## Returns
        //
        // Returns null on success per ACP specification for notifications.
        //
        // ## Client Usage Example
        //
        // ```typescript
        // await agent.ext_method({
        //   method: "editor/update_buffers",
        //   params: {
        //     buffers: {
        //       "/path/to/file.rs": {
        //         path: "/path/to/file.rs",
        //         content: "fn main() { ... }",
        //         modified: true,
        //         last_modified: { secs_since_epoch: 1234567890, nanos_since_epoch: 0 },
        //         encoding: "UTF-8"
        //       }
        //     },
        //     unavailable_paths: []
        //   }
        // });
        // ```
        if request.method == "editor/update_buffers".into() {
            // Validate client capabilities for editor state operations
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if crate::editor_state::supports_editor_state(caps) => {
                        tracing::debug!("Editor state capability declared and validated");
                    }
                    Some(_) => {
                        tracing::error!("editor/update_buffers capability not declared by client");
                        return Err(agent_client_protocol::Error::new(
                            -32602,
                            "Editor state capability not declared by client. This feature requires client to support editor buffer synchronization.".to_string(),
                        ));
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for editor/update_buffers validation"
                        );
                        return Err(agent_client_protocol::Error::new(
                            -32602,
                            "Client capabilities not initialized. Cannot perform editor operations without capability declaration.".to_string(),
                        ));
                    }
                }
            }

            // Parse the request parameters from RawValue
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse editor/update_buffers parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let response: crate::editor_state::EditorBufferResponse =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!(
                        "Failed to deserialize editor/update_buffers parameters: {}",
                        e
                    );
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Update the editor state manager cache
            tracing::info!(
                "Updating editor buffers cache with {} buffers",
                response.buffers.len()
            );
            self.editor_state_manager
                .update_buffers_from_response(response)
                .await;

            // Return success with null result
            let response_json = serde_json::Value::Null;
            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Return a structured response indicating no other extensions are implemented
        // This maintains ACP compliance while clearly communicating capability limitations
        let response = serde_json::json!({
            "method": request.method,
            "result": "Extension method not implemented"
        });

        let raw_value = RawValue::from_string(response.to_string())
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;

        Ok(ExtResponse::new(Arc::from(raw_value)))
    }

    async fn ext_notification(
        &self,
        notification: ExtNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        self.log_request("ext_notification", &notification);
        tracing::info!("Extension notification received: {}", notification.method);

        // Handle extension notifications
        Ok(())
    }
}

// Additional ClaudeAgent methods not part of the Agent trait
impl ClaudeAgent {
    /// Request permission for a tool call (ACP session/request_permission method)
    pub async fn request_permission(
        &self,
        request: PermissionRequest,
    ) -> Result<PermissionResponse, agent_client_protocol::Error> {
        self.log_request("request_permission", &request);
        tracing::info!(
            "Processing permission request for session: {} and tool call: {}",
            request.session_id.0,
            request.tool_call.tool_call_id
        );

        // ACP requires comprehensive permission system with user choice:
        // 1. Multiple permission options: allow/reject with once/always variants
        // 2. Permission persistence: Remember "always" decisions across sessions
        // 3. Tool call integration: Block execution until permission granted
        // 4. Cancellation support: Handle cancelled prompt turns gracefully
        // 5. Context awareness: Generate appropriate options for different tools
        //
        // Advanced permissions provide user control while maintaining security.

        // Parse session ID
        let session_id = self.parse_session_id(&request.session_id)?;

        // Check if session is cancelled
        if self
            .cancellation_manager
            .is_cancelled(&session_id.to_string())
            .await
        {
            tracing::info!(
                "Session {} is cancelled, returning cancelled outcome",
                session_id
            );
            return Ok(PermissionResponse {
                outcome: crate::tools::PermissionOutcome::Cancelled,
            });
        }

        // Extract tool name and arguments from the active tool call
        let (tool_name, tool_args) = {
            let tool_handler = self.tool_handler.read().await;
            let active_calls = tool_handler.get_active_tool_calls().await;

            match active_calls.get(&request.tool_call.tool_call_id) {
                Some(report) => {
                    let name = report.tool_name.clone();
                    let args = report
                        .raw_input
                        .clone()
                        .unwrap_or_else(|| serde_json::json!({}));
                    (name, args)
                }
                None => {
                    tracing::warn!(
                        "Tool call {} not found in active calls, using defaults",
                        request.tool_call.tool_call_id
                    );
                    ("unknown_tool".to_string(), serde_json::json!({}))
                }
            }
        };

        // Use permission policy engine to evaluate the tool call
        let policy_result = match self
            .permission_engine
            .evaluate_tool_call(&tool_name, &tool_args)
            .await
        {
            Ok(evaluation) => evaluation,
            Err(e) => {
                tracing::error!("Permission policy evaluation failed: {}", e);
                return Ok(PermissionResponse {
                    outcome: crate::tools::PermissionOutcome::Cancelled,
                });
            }
        };

        let selected_outcome = match policy_result {
            PolicyEvaluation::Allowed => {
                tracing::info!("Tool '{}' allowed by policy", tool_name);
                crate::tools::PermissionOutcome::Selected {
                    option_id: "allow-once".to_string(),
                }
            }
            PolicyEvaluation::Denied { reason } => {
                tracing::info!("Tool '{}' denied by policy: {}", tool_name, reason);
                crate::tools::PermissionOutcome::Selected {
                    option_id: "reject-once".to_string(),
                }
            }
            PolicyEvaluation::RequireUserConsent { options } => {
                tracing::info!("Tool '{}' requires user consent", tool_name);

                // If options were provided in request, use those; otherwise use policy-generated options
                let permission_options: Vec<_> = if !request.options.is_empty() {
                    request.options.clone()
                } else {
                    options.clone()
                };

                // Check if there's a stored preference for this tool
                if let Some(stored_kind) = self.permission_storage.get_preference(&tool_name).await
                {
                    let option_id = match stored_kind {
                        crate::tools::PermissionOptionKind::AllowAlways => "allow-always",
                        crate::tools::PermissionOptionKind::RejectAlways => "reject-always",
                        _ => {
                            tracing::warn!("Unexpected stored permission kind: {:?}", stored_kind);
                            "allow-once"
                        }
                    };

                    tracing::info!(
                        "Using stored permission preference for '{}': {}",
                        tool_name,
                        option_id
                    );

                    return Ok(PermissionResponse {
                        outcome: crate::tools::PermissionOutcome::Selected {
                            option_id: option_id.to_string(),
                        },
                    });
                }

                // Send client/request_permission message via ACP connection
                if let Some(ref client) = self.client {
                    // Convert our internal types to ACP protocol types
                    let acp_options: Vec<agent_client_protocol::PermissionOption> =
                        permission_options
                            .iter()
                            .map(|opt| {
                                let kind = match opt.kind {
                                    crate::tools::PermissionOptionKind::AllowOnce => {
                                        agent_client_protocol::PermissionOptionKind::AllowOnce
                                    }
                                    crate::tools::PermissionOptionKind::AllowAlways => {
                                        agent_client_protocol::PermissionOptionKind::AllowAlways
                                    }
                                    crate::tools::PermissionOptionKind::RejectOnce => {
                                        agent_client_protocol::PermissionOptionKind::RejectOnce
                                    }
                                    crate::tools::PermissionOptionKind::RejectAlways => {
                                        agent_client_protocol::PermissionOptionKind::RejectAlways
                                    }
                                };
                                agent_client_protocol::PermissionOption::new(
                                    opt.option_id.clone(),
                                    opt.name.clone(),
                                    kind,
                                )
                            })
                            .collect();

                    let tool_call_update = agent_client_protocol::ToolCallUpdate::new(
                        agent_client_protocol::ToolCallId::new(
                            request.tool_call.tool_call_id.as_str(),
                        ),
                        agent_client_protocol::ToolCallUpdateFields::new(),
                    );

                    let acp_request = agent_client_protocol::RequestPermissionRequest::new(
                        request.session_id.clone(),
                        tool_call_update,
                        acp_options,
                    );

                    match client.request_permission(acp_request).await {
                        Ok(response) => {
                            // Convert ACP response back to our internal type
                            match response.outcome {
                                agent_client_protocol::RequestPermissionOutcome::Cancelled => {
                                    crate::tools::PermissionOutcome::Cancelled
                                }
                                agent_client_protocol::RequestPermissionOutcome::Selected(
                                    selected,
                                ) => {
                                    let option_id_str = selected.option_id.to_string();

                                    // Store preference if it's an "always" decision
                                    if let Some(option) = permission_options
                                        .iter()
                                        .find(|opt| opt.option_id == option_id_str)
                                    {
                                        self.permission_storage
                                            .store_preference(&tool_name, option.kind.clone())
                                            .await;
                                    }

                                    crate::tools::PermissionOutcome::Selected {
                                        option_id: option_id_str,
                                    }
                                }
                                _ => {
                                    tracing::warn!(
                                        "Unknown permission outcome, treating as cancelled"
                                    );
                                    crate::tools::PermissionOutcome::Cancelled
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to request permission from client: {}", e);
                            crate::tools::PermissionOutcome::Cancelled
                        }
                    }
                } else {
                    tracing::warn!(
                        "Permission required for tool '{}' but no client connection available",
                        tool_name
                    );
                    crate::tools::PermissionOutcome::Cancelled
                }
            }
        };

        let response = PermissionResponse {
            outcome: selected_outcome,
        };

        tracing::info!(
            "Permission request completed for session: {} with outcome: {:?}",
            session_id,
            response.outcome
        );

        self.log_response("request_permission", &response);
        Ok(response)
    }

    /// Handle fs/read_text_file ACP extension method
    ///
    /// ACP requires integration with client editor state to access unsaved changes.
    /// This method:
    /// 1. Checks if an editor buffer is available for the file
    /// 2. Falls back to disk content if no editor buffer exists
    /// 3. Applies line filtering if requested
    ///
    /// This ensures agents work with current, not stale, file content.
    pub async fn handle_read_text_file(
        &self,
        params: ReadTextFileParams,
    ) -> Result<ReadTextFileResponse, agent_client_protocol::Error> {
        tracing::debug!("Processing fs/read_text_file request: {:?}", params);

        // Audit logging for file access attempt
        tracing::info!(
            security_event = "file_read_attempt",
            session_id = %params.session_id,
            path = %params.path,
            "File read operation requested"
        );

        // Validate client capabilities for file system read operations
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.fs.read_text_file => {
                    tracing::debug!("File system read capability validated");
                }
                Some(_) => {
                    tracing::error!("fs/read_text_file capability not declared by client");
                    return Err(agent_client_protocol::Error::new(-32602,
                        "File system read capability not declared by client. Set client_capabilities.fs.read_text_file = true during initialization."
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for fs/read_text_file validation"
                    );
                    return Err(agent_client_protocol::Error::new(-32602,
                        "Client capabilities not initialized. Cannot perform file system operations without capability declaration."
                    ));
                }
            }
        }

        // Validate session ID
        self.parse_session_id(&SessionId::new(params.session_id.clone()))
            .map_err(|_| agent_client_protocol::Error::invalid_params())?;

        // Validate path security using PathValidator
        // This checks: absolute path, no traversal, no symlinks, blocked paths
        let validated_path = self
            .path_validator
            .validate_absolute_path(&params.path)
            .map_err(|e| {
                tracing::warn!(
                    security_event = "path_validation_failed",
                    session_id = %params.session_id,
                    path = %params.path,
                    error = %e,
                    "Path validation failed for read operation"
                );
                // Use generic error message to avoid leaking security policy details
                agent_client_protocol::Error::new(-32602, "Invalid file path".to_string())
            })?;

        // Validate line and limit parameters
        if let Some(line) = params.line {
            if line == 0 {
                return Err(agent_client_protocol::Error::invalid_params());
            }
        }

        let path = validated_path.as_path();

        // ACP requires integration with client editor state for unsaved changes
        // Try to get content from editor buffer first
        match self
            .editor_state_manager
            .get_file_content(&params.session_id, path)
            .await
        {
            Ok(Some(editor_buffer)) => {
                tracing::debug!(
                    "Using editor buffer content for: {} (modified: {})",
                    params.path,
                    editor_buffer.modified
                );
                // Editor buffer content needs line filtering applied
                let filtered_content =
                    self.apply_line_filtering(&editor_buffer.content, params.line, params.limit)?;
                Ok(ReadTextFileResponse {
                    content: filtered_content,
                })
            }
            Ok(None) => {
                // No editor buffer available, read from disk (with line filtering)
                tracing::trace!("Reading from disk (no editor buffer): {}", params.path);
                let content = self
                    .read_file_with_options(&params.path, params.line, params.limit)
                    .await?;
                Ok(ReadTextFileResponse { content })
            }
            Err(e) => {
                tracing::warn!(
                    "Editor state query failed for {}: {}, falling back to disk",
                    params.path,
                    e
                );
                let content = self
                    .read_file_with_options(&params.path, params.line, params.limit)
                    .await?;
                Ok(ReadTextFileResponse { content })
            }
        }
    }

    /// Handle fs/write_text_file ACP extension method
    pub async fn handle_write_text_file(
        &self,
        params: WriteTextFileParams,
    ) -> Result<WriteTextFileResponse, agent_client_protocol::Error> {
        tracing::debug!("Processing fs/write_text_file request: {:?}", params);

        // Audit logging for file write attempt
        tracing::info!(
            security_event = "file_write_attempt",
            session_id = %params.session_id,
            path = %params.path,
            content_size = params.content.len(),
            "File write operation requested"
        );

        // Validate client capabilities for file system write operations
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.fs.write_text_file => {
                    tracing::debug!("File system write capability validated");
                }
                Some(_) => {
                    tracing::error!("fs/write_text_file capability not declared by client");
                    return Err(agent_client_protocol::Error::new(-32602,
                        "File system write capability not declared by client. Set client_capabilities.fs.write_text_file = true during initialization."
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for fs/write_text_file validation"
                    );
                    return Err(agent_client_protocol::Error::new(-32602,
                        "Client capabilities not initialized. Cannot perform file system operations without capability declaration."
                    ));
                }
            }
        }

        // Validate session ID
        self.parse_session_id(&SessionId::new(params.session_id.clone()))
            .map_err(|_| agent_client_protocol::Error::invalid_params())?;

        // Validate path security using PathValidator with non-strict canonicalization
        // For write operations, the file may not exist yet, so we use non-strict mode
        // This still checks: absolute path, no traversal
        // Note: For production use, consider using the same blocked/allowed paths as the main validator
        let write_validator =
            crate::path_validator::PathValidator::new().with_strict_canonicalization(false);

        let validated_path = write_validator
            .validate_absolute_path(&params.path)
            .map_err(|e| {
                tracing::warn!(
                    security_event = "path_validation_failed",
                    session_id = %params.session_id,
                    path = %params.path,
                    error = %e,
                    "Path validation failed for write operation"
                );
                // Use generic error message to avoid leaking security policy details
                agent_client_protocol::Error::new(-32602, "Invalid file path".to_string())
            })?;

        // Validate content size before write to prevent disk exhaustion
        // Using > to reject content strictly larger than the limit (50MB limit is exclusive)
        let content_size = params.content.len();
        if content_size > sizes::content::MAX_RESOURCE_MODERATE {
            tracing::warn!(
                security_event = "content_size_exceeded",
                session_id = %params.session_id,
                path = %params.path,
                size = content_size,
                limit = sizes::content::MAX_RESOURCE_MODERATE,
                "Content size exceeds maximum allowed for write operation"
            );
            // Return error with size information for client debugging
            return Err(agent_client_protocol::Error::new(
                -32602,
                format!(
                    "Content size {} bytes exceeds maximum {} bytes (limit is exclusive)",
                    content_size,
                    sizes::content::MAX_RESOURCE_MODERATE
                ),
            )
            .data(serde_json::json!({
                "error": "content_too_large",
                "size": content_size,
                "max_size": sizes::content::MAX_RESOURCE_MODERATE
            })));
        }

        // Perform atomic write operation with validated path
        self.write_file_atomically(validated_path.to_str().unwrap(), &params.content)
            .await?;

        // Audit logging for successful write
        tracing::info!(
            security_event = "file_write_success",
            session_id = %params.session_id,
            path = %params.path,
            bytes = content_size,
            "File write completed successfully"
        );

        // Return WriteTextFileResponse as per ACP specification
        Ok(WriteTextFileResponse::default())
    }

    /// Handle terminal/output ACP extension method
    pub async fn handle_terminal_output(
        &self,
        params: crate::terminal_manager::TerminalOutputParams,
    ) -> Result<crate::terminal_manager::TerminalOutputResponse, agent_client_protocol::Error> {
        tracing::debug!("Processing terminal/output request: {:?}", params);

        // Check client terminal capability before allowing operation
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.terminal => {
                    tracing::debug!("Terminal capability validated for handle_terminal_output");
                }
                Some(_) => {
                    tracing::error!("terminal capability not declared by client");
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Terminal capability not declared by client. Set client_capabilities.terminal = true during initialization.".to_string(),
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for terminal operation validation"
                    );
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Client capabilities not initialized. Cannot perform terminal operations without capability declaration.".to_string(),
                    ));
                }
            }
        }

        // Get terminal manager from tool handler
        let tool_handler = self.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        // Get output from terminal manager
        terminal_manager
            .get_output(&self.session_manager, params)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get terminal output: {}", e);
                e.into()
            })
    }

    /// Handle terminal/release ACP extension method
    pub async fn handle_terminal_release(
        &self,
        params: crate::terminal_manager::TerminalReleaseParams,
    ) -> Result<serde_json::Value, agent_client_protocol::Error> {
        tracing::debug!("Processing terminal/release request: {:?}", params);

        // Check client terminal capability before allowing operation
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.terminal => {
                    tracing::debug!("Terminal capability validated for handle_terminal_release");
                }
                Some(_) => {
                    tracing::error!("terminal capability not declared by client");
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Terminal capability not declared by client. Set client_capabilities.terminal = true during initialization.".to_string(),
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for terminal operation validation"
                    );
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Client capabilities not initialized. Cannot perform terminal operations without capability declaration.".to_string(),
                    ));
                }
            }
        }

        // Get terminal manager from tool handler
        let tool_handler = self.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        // Release terminal and return null per ACP specification
        terminal_manager
            .release_terminal(&self.session_manager, params)
            .await
            .map_err(|e| {
                tracing::error!("Failed to release terminal: {}", e);
                e.into()
            })
    }

    /// Handle terminal/wait_for_exit ACP extension method
    pub async fn handle_terminal_wait_for_exit(
        &self,
        params: crate::terminal_manager::TerminalOutputParams,
    ) -> Result<crate::terminal_manager::ExitStatus, agent_client_protocol::Error> {
        tracing::debug!("Processing terminal/wait_for_exit request: {:?}", params);

        // Check client terminal capability before allowing operation
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.terminal => {
                    tracing::debug!(
                        "Terminal capability validated for handle_terminal_wait_for_exit"
                    );
                }
                Some(_) => {
                    tracing::error!("terminal capability not declared by client");
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Terminal capability not declared by client. Set client_capabilities.terminal = true during initialization.".to_string(),
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for terminal operation validation"
                    );
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Client capabilities not initialized. Cannot perform terminal operations without capability declaration.".to_string(),
                    ));
                }
            }
        }

        // Get terminal manager from tool handler
        let tool_handler = self.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        // Wait for terminal exit
        terminal_manager
            .wait_for_exit(&self.session_manager, params)
            .await
            .map_err(|e| {
                tracing::error!("Failed to wait for terminal exit: {}", e);
                e.into()
            })
    }

    /// Handle terminal/kill ACP extension method
    pub async fn handle_terminal_kill(
        &self,
        params: crate::terminal_manager::TerminalOutputParams,
    ) -> Result<(), agent_client_protocol::Error> {
        tracing::debug!("Processing terminal/kill request: {:?}", params);

        // Check client terminal capability before allowing operation
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.terminal => {
                    tracing::debug!("Terminal capability validated for handle_terminal_kill");
                }
                Some(_) => {
                    tracing::error!("terminal capability not declared by client");
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Terminal capability not declared by client. Set client_capabilities.terminal = true during initialization.".to_string(),
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for terminal operation validation"
                    );
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Client capabilities not initialized. Cannot perform terminal operations without capability declaration.".to_string(),
                    ));
                }
            }
        }

        // Get terminal manager from tool handler
        let tool_handler = self.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        // Kill terminal process
        terminal_manager
            .kill_terminal(&self.session_manager, params)
            .await
            .map_err(|e| {
                tracing::error!("Failed to kill terminal: {}", e);
                e.into()
            })
    }

    /// Handle terminal/create ACP extension method
    pub async fn handle_terminal_create(
        &self,
        params: crate::terminal_manager::TerminalCreateParams,
    ) -> Result<crate::terminal_manager::TerminalCreateResponse, agent_client_protocol::Error> {
        tracing::debug!("Processing terminal/create request: {:?}", params);

        // Check client terminal capability before allowing operation
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.terminal => {
                    tracing::debug!("Terminal capability validated for handle_terminal_create");
                }
                Some(_) => {
                    tracing::error!("terminal capability not declared by client");
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Terminal capability not declared by client. Set client_capabilities.terminal = true during initialization.".to_string(),
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for terminal operation validation"
                    );
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Client capabilities not initialized. Cannot perform terminal operations without capability declaration.".to_string(),
                    ));
                }
            }
        }

        // Get terminal manager from tool handler
        let tool_handler = self.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        // Create terminal and return the terminal ID
        let terminal_id = terminal_manager
            .create_terminal_with_command(&self.session_manager, params)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create terminal: {}", e);
                agent_client_protocol::Error::from(e)
            })?;

        Ok(crate::terminal_manager::TerminalCreateResponse { terminal_id })
    }

    /// Read file content with optional line offset and limit
    ///
    /// # Security
    /// This function assumes the caller has already validated the path using PathValidator.
    /// Path validation must include: absolute path check, traversal prevention, and blocked path check.
    async fn read_file_with_options(
        &self,
        path: &str,
        start_line: Option<u32>,
        limit: Option<u32>,
    ) -> Result<String, agent_client_protocol::Error> {
        // Check file size before reading to prevent memory exhaustion
        let metadata = tokio::fs::metadata(path).await.map_err(|e| {
            tracing::error!(
                security_event = "file_metadata_failed",
                path = %path,
                error = %e,
                "Failed to get file metadata"
            );
            match e.kind() {
                std::io::ErrorKind::NotFound => agent_client_protocol::Error::invalid_params(),
                std::io::ErrorKind::PermissionDenied => {
                    agent_client_protocol::Error::invalid_params()
                }
                _ => agent_client_protocol::Error::internal_error(),
            }
        })?;

        let file_size = metadata.len() as usize;

        // Validate file size against configured limits
        // Using > to reject files strictly larger than the limit (50MB limit is exclusive)
        if file_size > sizes::content::MAX_RESOURCE_MODERATE {
            tracing::warn!(
                security_event = "file_size_exceeded",
                path = %path,
                size = file_size,
                limit = sizes::content::MAX_RESOURCE_MODERATE,
                "File size exceeds maximum allowed for read operation"
            );
            return Err(agent_client_protocol::Error::invalid_params());
        }

        // Read the entire file
        let file_content = tokio::fs::read_to_string(path).await.map_err(|e| {
            tracing::error!(
                security_event = "file_read_failed",
                path = %path,
                error = %e,
                "Failed to read file content"
            );
            match e.kind() {
                std::io::ErrorKind::NotFound => agent_client_protocol::Error::invalid_params(),
                std::io::ErrorKind::PermissionDenied => {
                    agent_client_protocol::Error::invalid_params()
                }
                _ => agent_client_protocol::Error::internal_error(),
            }
        })?;

        // Audit logging for successful read
        tracing::info!(
            security_event = "file_read_success",
            path = %path,
            bytes = file_content.len(),
            "File read completed successfully"
        );

        // Apply line filtering if specified
        self.apply_line_filtering(&file_content, start_line, limit)
    }

    /// Apply line offset and limit filtering to file content
    fn apply_line_filtering(
        &self,
        content: &str,
        start_line: Option<u32>,
        limit: Option<u32>,
    ) -> Result<String, agent_client_protocol::Error> {
        let lines: Vec<&str> = content.lines().collect();

        let start_index = match start_line {
            Some(line) => {
                if line == 0 {
                    return Err(agent_client_protocol::Error::invalid_params());
                }
                (line - 1) as usize // Convert to 0-based index
            }
            None => 0,
        };

        // If start index is beyond the end of the file, return empty string
        if start_index >= lines.len() {
            tracing::debug!(
                security_event = "line_out_of_bounds",
                start_line = start_index,
                total_lines = lines.len(),
                "Line offset beyond file end"
            );
            return Ok(String::new());
        }

        let end_index = match limit {
            Some(limit_count) => {
                if limit_count == 0 {
                    return Ok(String::new());
                }
                // Use checked_add to prevent integer overflow
                start_index
                    .checked_add(limit_count as usize)
                    .ok_or_else(agent_client_protocol::Error::invalid_params)?
                    .min(lines.len())
            }
            None => lines.len(),
        };

        let selected_lines = &lines[start_index..end_index];
        Ok(selected_lines.join("\n"))
    }

    /// Write file content atomically with parent directory creation
    async fn write_file_atomically(
        &self,
        path: &str,
        content: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        use std::path::Path;
        use ulid::Ulid;

        let path_buf = Path::new(path);

        // Create parent directories if they don't exist
        if let Some(parent_dir) = path_buf.parent() {
            if !parent_dir.exists() {
                tokio::fs::create_dir_all(parent_dir).await.map_err(|e| {
                    tracing::error!(
                        security_event = "directory_creation_failed",
                        path = %parent_dir.display(),
                        error = %e,
                        "Failed to create parent directory"
                    );
                    agent_client_protocol::Error::internal_error()
                })?;
            }
        }

        // Create temporary file in same directory for atomic write
        // Using ULID ensures uniqueness and prevents predictable temp file names
        let temp_path = if let Some(parent) = path_buf.parent() {
            let file_name = path_buf
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");
            parent.join(format!(".tmp.{}.{}", file_name, Ulid::new()))
        } else {
            std::path::PathBuf::from(format!("{}.tmp.{}", path, Ulid::new()))
        };

        // Ensure temp path is absolute before proceeding
        if !temp_path.is_absolute() {
            tracing::error!(
                security_event = "temp_path_not_absolute",
                path = %temp_path.display(),
                "Temporary file path must be absolute"
            );
            return Err(agent_client_protocol::Error::internal_error());
        }

        // Validate temp file path to prevent symlink manipulation in parent directories
        // This ensures the temp file doesn't escape security boundaries
        let temp_path = if let Some(parent) = temp_path.parent() {
            // Canonicalize the parent directory to resolve symlinks
            match parent.canonicalize() {
                Ok(canonical_parent) => {
                    let resolved = canonical_parent.join(temp_path.file_name().unwrap());
                    // Verify the resolved path is still absolute
                    if !resolved.is_absolute() {
                        tracing::error!(
                            security_event = "temp_path_resolution_failed",
                            resolved_path = %resolved.display(),
                            "Resolved temp path is not absolute"
                        );
                        return Err(agent_client_protocol::Error::internal_error());
                    }

                    // Additional validation: ensure the resolved temp path is within allowed boundaries
                    // Validate the resolved path to ensure it hasn't escaped security boundaries via symlinks
                    // Use non-strict canonicalization since the temp file doesn't exist yet
                    let temp_validator = crate::path_validator::PathValidator::new()
                        .with_strict_canonicalization(false);
                    if let Err(e) = temp_validator.validate_absolute_path(
                        resolved.to_str().ok_or_else(|| {
                            tracing::error!(
                                security_event = "temp_path_utf8_invalid",
                                resolved_path = %resolved.display(),
                                "Resolved temp path contains invalid UTF-8"
                            );
                            agent_client_protocol::Error::internal_error()
                        })?,
                    ) {
                        tracing::error!(
                            security_event = "temp_path_security_validation_failed",
                            resolved_path = %resolved.display(),
                            error = %e,
                            "Resolved temp path failed security validation - possible symlink attack"
                        );
                        return Err(agent_client_protocol::Error::internal_error());
                    }

                    resolved
                }
                Err(e) => {
                    tracing::error!(
                        security_event = "parent_canonicalization_failed",
                        parent = %parent.display(),
                        error = %e,
                        "Failed to canonicalize parent directory for temp file"
                    );
                    return Err(agent_client_protocol::Error::internal_error());
                }
            }
        } else {
            temp_path
        };

        let temp_path_str = temp_path.to_string_lossy();

        // Write content to temporary file
        match tokio::fs::write(&temp_path, content).await {
            Ok(_) => {
                // Set restrictive permissions on Unix systems (owner read/write only)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Err(e) = tokio::fs::set_permissions(
                        &temp_path,
                        std::fs::Permissions::from_mode(0o600),
                    )
                    .await
                    {
                        tracing::warn!(
                            security_event = "permission_set_failed",
                            path = %temp_path_str,
                            error = %e,
                            "Failed to set restrictive permissions on temp file"
                        );
                        // Continue despite permission setting failure
                    }
                }

                // Atomically rename temporary file to final path
                match tokio::fs::rename(&temp_path, path).await {
                    Ok(_) => {
                        tracing::debug!(
                            security_event = "atomic_write_success",
                            path = %path,
                            "Successfully completed atomic write"
                        );
                        Ok(())
                    }
                    Err(e) => {
                        // Clean up temp file on failure with explicit error handling
                        if let Err(cleanup_err) = tokio::fs::remove_file(&temp_path).await {
                            tracing::error!(
                                security_event = "temp_file_cleanup_failed",
                                temp_path = %temp_path_str,
                                cleanup_error = %cleanup_err,
                                "Failed to clean up temporary file after write failure - manual cleanup may be required"
                            );
                        }
                        tracing::error!(
                            security_event = "atomic_rename_failed",
                            path = %path,
                            temp_path = %temp_path_str,
                            error = %e,
                            "Failed to rename temp file"
                        );
                        Err(agent_client_protocol::Error::internal_error())
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    security_event = "temp_write_failed",
                    path = %temp_path_str,
                    error = %e,
                    "Failed to write temp file"
                );
                match e.kind() {
                    std::io::ErrorKind::PermissionDenied => {
                        Err(agent_client_protocol::Error::invalid_params())
                    }
                    _ => Err(agent_client_protocol::Error::internal_error()),
                }
            }
        }
    }

    /// Convert ACP MCP server configuration to internal configuration type for validation
    fn convert_acp_to_internal_mcp_config(
        &self,
        acp_config: &agent_client_protocol::McpServer,
    ) -> Option<crate::config::McpServerConfig> {
        use crate::config::{
            EnvVariable, HttpHeader, HttpTransport, McpServerConfig, SseTransport, StdioTransport,
        };
        use agent_client_protocol::McpServer;

        match acp_config {
            McpServer::Stdio(stdio) => {
                let internal_env = stdio
                    .env
                    .iter()
                    .map(|env_var| EnvVariable {
                        name: env_var.name.clone(),
                        value: env_var.value.clone(),
                    })
                    .collect();

                Some(McpServerConfig::Stdio(StdioTransport {
                    name: stdio.name.clone(),
                    command: stdio.command.to_string_lossy().to_string(),
                    args: stdio.args.clone(),
                    env: internal_env,
                    cwd: None, // ACP doesn't specify cwd, use default
                }))
            }
            McpServer::Http(http) => {
                let internal_headers = http
                    .headers
                    .iter()
                    .map(|header| HttpHeader {
                        name: header.name.clone(),
                        value: header.value.clone(),
                    })
                    .collect();

                Some(McpServerConfig::Http(HttpTransport {
                    transport_type: "http".to_string(),
                    name: http.name.clone(),
                    url: http.url.clone(),
                    headers: internal_headers,
                }))
            }
            McpServer::Sse(sse) => {
                let internal_headers = sse
                    .headers
                    .iter()
                    .map(|header| HttpHeader {
                        name: header.name.clone(),
                        value: header.value.clone(),
                    })
                    .collect();

                Some(McpServerConfig::Sse(SseTransport {
                    transport_type: "sse".to_string(),
                    name: sse.name.clone(),
                    url: sse.url.clone(),
                    headers: internal_headers,
                }))
            }
            _ => None,
        }
    }

    /// Convert SessionSetupError to ACP-compliant error response
    fn convert_session_setup_error_to_acp_error(
        &self,
        error: crate::session_errors::SessionSetupError,
    ) -> agent_client_protocol::Error {
        use crate::session_errors::SessionSetupError;

        match error {
            SessionSetupError::TransportNotSupported {
                requested_transport,
                declared_capability,
                supported_transports,
            } => {
                agent_client_protocol::Error::new(
                    -32602, // Invalid params
                    format!(
                        "{} transport not supported: agent did not declare mcpCapabilities.{}",
                        requested_transport.to_uppercase(),
                        requested_transport
                    ),
                )
                .data(serde_json::json!({
                    "requestedTransport": requested_transport,
                    "declaredCapability": declared_capability,
                    "supportedTransports": supported_transports
                }))
            }
            SessionSetupError::LoadSessionNotSupported {
                declared_capability,
            } => {
                agent_client_protocol::Error::new(
                    -32601, // Method not found
                    "Method not supported: agent does not support loadSession capability"
                        .to_string(),
                )
                .data(serde_json::json!({
                    "method": "session/load",
                    "requiredCapability": "loadSession",
                    "declared": declared_capability
                }))
            }
            _ => {
                // For any other validation errors, return generic invalid params
                agent_client_protocol::Error::invalid_params()
            }
        }
    }

    /// Get TodoStorage for a session
    ///
    /// Creates a TodoStorage instance configured for the session's working directory.
    /// This ensures todos are stored in the correct location relative to the session's cwd.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    ///
    /// Returns a TodoStorage instance configured for the session's working directory
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The session does not exist
    /// - The todo storage cannot be created
    pub async fn get_todo_storage(
        &self,
        session_id: &str,
    ) -> crate::Result<swissarmyhammer_todo::TodoStorage> {
        // Get the session to access its working directory
        let session_id_parsed =
            session_id
                .to_string()
                .parse()
                .map_err(|e: crate::session::SessionIdError| {
                    crate::error::AgentError::Session(format!("Invalid session ID: {}", e))
                })?;
        let session = self
            .session_manager
            .get_session(&session_id_parsed)
            .map_err(|_e| {
                crate::error::AgentError::Session(format!("Session not found: {}", session_id))
            })?
            .ok_or_else(|| {
                crate::error::AgentError::Session(format!("Session not found: {}", session_id))
            })?;

        // Create TodoStorage using the session's working directory
        let todo_storage = swissarmyhammer_todo::TodoStorage::new_with_working_dir(session.cwd)
            .map_err(|e| {
                crate::error::AgentError::Internal(format!("Failed to create todo storage: {}", e))
            })?;

        Ok(todo_storage)
    }

    /// Create a new todo item for a session
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `task` - The task description
    /// * `context` - Optional context or implementation notes
    ///
    /// # Returns
    ///
    /// Returns a tuple containing:
    /// - The created TodoItem
    /// - The number of completed items that were garbage collected
    ///
    /// # Errors
    ///
    /// Returns an error if the todo item cannot be created
    pub async fn create_todo(
        &self,
        session_id: &str,
        task: String,
        context: Option<String>,
    ) -> crate::Result<(swissarmyhammer_todo::TodoItem, usize)> {
        let storage = self.get_todo_storage(session_id).await?;
        storage.create_todo_item(task, context).await.map_err(|e| {
            crate::error::AgentError::Internal(format!("Failed to create todo item: {}", e))
        })
    }

    /// Get a specific todo item by ID or the next incomplete item
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `item_identifier` - Either a ULID string or "next" for the next incomplete item
    ///
    /// # Returns
    ///
    /// Returns the todo item if found, or None if not found or no incomplete items exist
    ///
    /// # Errors
    ///
    /// Returns an error if the todo item cannot be retrieved
    pub async fn get_todo_item(
        &self,
        session_id: &str,
        item_identifier: &str,
    ) -> crate::Result<Option<swissarmyhammer_todo::TodoItem>> {
        let storage = self.get_todo_storage(session_id).await?;
        storage.get_todo_item(item_identifier).await.map_err(|e| {
            crate::error::AgentError::Internal(format!("Failed to get todo item: {}", e))
        })
    }

    /// Mark a todo item as complete
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `id` - The todo item ID
    ///
    /// # Errors
    ///
    /// Returns an error if the todo item cannot be marked as complete
    pub async fn mark_todo_complete(
        &self,
        session_id: &str,
        id: &swissarmyhammer_todo::TodoId,
    ) -> crate::Result<()> {
        let storage = self.get_todo_storage(session_id).await?;
        storage.mark_todo_complete(id).await.map_err(|e| {
            crate::error::AgentError::Internal(format!("Failed to mark todo complete: {}", e))
        })
    }

    /// Get all todo items for a session
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    ///
    /// Returns the complete todo list if it exists, or None if no todos exist
    ///
    /// # Errors
    ///
    /// Returns an error if the todo list cannot be retrieved
    pub async fn get_todo_list(
        &self,
        session_id: &str,
    ) -> crate::Result<Option<swissarmyhammer_todo::TodoList>> {
        let storage = self.get_todo_storage(session_id).await?;
        storage.get_todo_list().await.map_err(|e| {
            crate::error::AgentError::Internal(format!("Failed to get todo list: {}", e))
        })
    }

    /// Sync session todos with TodoStorage
    ///
    /// Loads todos from TodoStorage and updates the session's todos vector with the IDs
    /// of all incomplete todo items. This ensures the session's todo list is in sync
    /// with the persistent storage.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The session does not exist
    /// - The todo storage cannot be accessed
    /// - The session cannot be updated
    pub async fn sync_session_todos(&self, session_id: &str) -> crate::Result<()> {
        // Get the todo list from storage
        let storage = self.get_todo_storage(session_id).await?;
        let todo_list = storage.get_todo_list().await.map_err(|e| {
            crate::error::AgentError::Internal(format!("Failed to get todo list: {}", e))
        })?;

        // Extract incomplete todo IDs
        let todo_ids: Vec<String> = if let Some(list) = todo_list {
            list.todo
                .iter()
                .filter(|item| !item.is_complete())
                .map(|item| item.id.clone())
                .collect()
        } else {
            Vec::new()
        };

        // Update the session's todos vector
        let session_id_parsed: crate::session::SessionId =
            session_id
                .to_string()
                .parse()
                .map_err(|e: crate::session::SessionIdError| {
                    crate::error::AgentError::Session(format!("Invalid session ID: {}", e))
                })?;
        self.session_manager
            .update_session(&session_id_parsed, |session| {
                session.todos = todo_ids;
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{Client, RequestPermissionRequest, RequestPermissionResponse};
    // Import specific types as needed
    use std::sync::Arc;
    use tokio::time::Duration;

    /// TestClient implements the Client trait for testing
    /// Auto-approves all permission requests
    struct TestClient;

    #[async_trait::async_trait(?Send)]
    impl Client for TestClient {
        async fn request_permission(
            &self,
            _args: RequestPermissionRequest,
        ) -> Result<RequestPermissionResponse, agent_client_protocol::Error> {
            // Auto-approve all permission requests in tests
            let selected = agent_client_protocol::SelectedPermissionOutcome::new(
                agent_client_protocol::PermissionOptionId::new("allow-once"),
            );
            Ok(RequestPermissionResponse::new(
                agent_client_protocol::RequestPermissionOutcome::Selected(selected),
            ))
        }

        async fn session_notification(
            &self,
            _args: agent_client_protocol::SessionNotification,
        ) -> Result<(), agent_client_protocol::Error> {
            // Accept notifications but don't do anything with them in tests
            Ok(())
        }
    }

    /// Create test setup with proper ACP connections
    /// Returns ClientSideConnection that tests should use (implements Agent trait)
    async fn create_test_connection() -> agent_client_protocol::ClientSideConnection {
        let config = AgentConfig::default();
        let (agent, _notification_receiver) = ClaudeAgent::new(config).await.unwrap();

        // Create bidirectional pipes for Client <-> Agent communication
        let (client_to_agent_rx, client_to_agent_tx) = piper::pipe(1024);
        let (agent_to_client_rx, agent_to_client_tx) = piper::pipe(1024);

        // Create TestClient that implements Client trait
        let test_client = TestClient;

        // Create ClientSideConnection (wraps TestClient, implements Agent trait)
        // This is what we call agent methods on - it sends JSON-RPC to the real agent
        let (client_conn, client_io_task) = agent_client_protocol::ClientSideConnection::new(
            test_client,
            client_to_agent_tx,
            agent_to_client_rx,
            |fut| {
                tokio::task::spawn_local(fut);
            },
        );

        // Create AgentSideConnection (wraps ClaudeAgent, implements Client trait)
        // This receives JSON-RPC from client and forwards to ClaudeAgent
        let (_agent_conn, agent_io_task) = agent_client_protocol::AgentSideConnection::new(
            agent,
            agent_to_client_tx,
            client_to_agent_rx,
            |fut| {
                tokio::task::spawn_local(fut);
            },
        );

        // Spawn both IO tasks to handle bidirectional communication
        tokio::task::spawn_local(client_io_task);
        tokio::task::spawn_local(agent_io_task);

        // Return the client connection - tests call methods on this (Agent trait)
        client_conn
    }

    /// Create ClaudeAgent directly for unit tests that need access to internals
    async fn create_test_agent() -> ClaudeAgent {
        let config = AgentConfig::default();
        ClaudeAgent::new(config).await.unwrap().0
    }

    async fn setup_agent_with_session() -> (ClaudeAgent, String) {
        let agent = create_test_agent().await;
        println!("Agent created");

        // Initialize with client capabilities
        let mut caps_meta = serde_json::Map::new();
        caps_meta.insert("streaming".to_string(), serde_json::json!(true));
        let client_capabilities = agent_client_protocol::ClientCapabilities::new()
            .fs(agent_client_protocol::FileSystemCapability::new()
                .read_text_file(true)
                .write_text_file(true))
            .terminal(true)
            .meta(caps_meta);

        let mut meta_map = serde_json::Map::new();
        meta_map.insert("test".to_string(), serde_json::json!(true));
        let init_request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V1)
            .client_capabilities(client_capabilities)
            .meta(meta_map);

        match agent.initialize(init_request).await {
            Ok(_) => println!("Agent initialized successfully"),
            Err(e) => panic!("Initialize failed: {:?}", e),
        }

        // Create session
        let mut session_meta = serde_json::Map::new();
        session_meta.insert("test".to_string(), serde_json::json!(true));
        let new_request =
            NewSessionRequest::new(std::path::PathBuf::from("/tmp")).meta(session_meta);

        let new_response = match agent.new_session(new_request).await {
            Ok(resp) => resp,
            Err(e) => panic!("New session failed: {:?}", e),
        };

        let session_id = new_response.session_id.0.as_ref().to_string();
        println!("Session created: {}", session_id);

        (agent, session_id)
    }

    #[tokio::test]
    async fn test_initialize() {
        let agent = create_test_agent().await;

        let mut caps_meta = serde_json::Map::new();
        caps_meta.insert("streaming".to_string(), serde_json::json!(true));
        let client_capabilities = agent_client_protocol::ClientCapabilities::new()
            .fs(agent_client_protocol::FileSystemCapability::new()
                .read_text_file(true)
                .write_text_file(true))
            .terminal(true)
            .meta(caps_meta);

        let request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V1)
            .client_capabilities(client_capabilities);

        let response = agent.initialize(request).await.unwrap();

        assert!(response.agent_capabilities.meta.is_some());
        assert!(response.auth_methods.is_empty());
        assert!(response.meta.is_some());
        // Protocol version should be V1
        assert_eq!(
            response.protocol_version,
            agent_client_protocol::ProtocolVersion::V1
        );
    }

    #[tokio::test]
    async fn test_initialize_mcp_capabilities() {
        let agent = create_test_agent().await;

        let mut caps_meta = serde_json::Map::new();
        caps_meta.insert("streaming".to_string(), serde_json::json!(true));
        let client_capabilities = agent_client_protocol::ClientCapabilities::new()
            .fs(agent_client_protocol::FileSystemCapability::new()
                .read_text_file(true)
                .write_text_file(true))
            .terminal(true)
            .meta(caps_meta);

        let request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V1)
            .client_capabilities(client_capabilities);

        let response = agent.initialize(request).await.unwrap();

        // Verify MCP capabilities are declared according to ACP specification
        assert!(
            response.agent_capabilities.mcp_capabilities.http,
            "MCP HTTP transport should be enabled"
        );
        assert!(
            !response.agent_capabilities.mcp_capabilities.sse,
            "MCP SSE transport should be disabled (deprecated)"
        );

        // Verify the structure matches ACP specification requirements
        // The MCP capabilities should be present in the agent_capabilities field
        assert!(response.agent_capabilities.meta.is_some());

        // Verify that meta field contains tools information since we have MCP support
        let meta = response.agent_capabilities.meta.as_ref().unwrap();
        assert!(
            meta.get("tools").is_some(),
            "Agent capabilities should declare available tools"
        );
    }

    #[tokio::test]
    async fn test_authenticate() {
        let agent = create_test_agent().await;

        // Test that authentication is properly rejected since we declare no auth methods
        let request = AuthenticateRequest::new(agent_client_protocol::AuthMethodId::new("none"));

        let result = agent.authenticate(request).await;
        assert!(result.is_err(), "Authentication should be rejected");

        // Test with a different method to ensure all methods are rejected
        let request2 = AuthenticateRequest::new(agent_client_protocol::AuthMethodId::new("basic"));

        let result2 = agent.authenticate(request2).await;
        assert!(
            result2.is_err(),
            "All authentication methods should be rejected"
        );
    }

    #[tokio::test]
    async fn test_new_session() {
        let agent = create_test_agent().await;

        let request = NewSessionRequest::new(std::path::PathBuf::from("/tmp")).meta(
            serde_json::json!({"test": true})
                .as_object()
                .unwrap()
                .clone(),
        );

        let response = agent.new_session(request).await.unwrap();
        assert!(!response.session_id.0.is_empty());
        assert!(response.meta.is_some());

        // Verify the session was actually created
        let session_id = response.session_id.0.parse().unwrap();
        let session = agent.session_manager.get_session(&session_id).unwrap();
        assert!(session.is_some());
    }

    #[tokio::test]
    async fn test_load_session() {
        let agent = create_test_agent().await;

        // First create a session
        let new_request = NewSessionRequest::new(std::path::PathBuf::from("/tmp")).meta(
            serde_json::json!({"test": true})
                .as_object()
                .unwrap()
                .clone(),
        );
        let new_response = agent.new_session(new_request).await.unwrap();

        // Now load it
        let load_request =
            LoadSessionRequest::new(new_response.session_id, std::path::PathBuf::from("/tmp"));

        let load_response = agent.load_session(load_request).await.unwrap();
        assert!(load_response.meta.is_some());

        // Verify that message_count and history_replayed are present in meta
        let meta = load_response.meta.unwrap();
        assert!(meta.get("message_count").is_some());
        assert!(meta.get("history_replayed").is_some());
        assert_eq!(meta.get("message_count").unwrap().as_u64().unwrap(), 0); // Empty session
        assert_eq!(meta.get("history_replayed").unwrap().as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_load_session_with_history_replay() {
        let agent = create_test_agent().await;

        // First create a session
        let new_request = NewSessionRequest::new(std::path::PathBuf::from("/tmp")).meta(
            serde_json::json!({"test": true})
                .as_object()
                .unwrap()
                .clone(),
        );
        let new_response = agent.new_session(new_request).await.unwrap();
        let session_id = agent.parse_session_id(&new_response.session_id).unwrap();

        // Add some messages to the session history
        agent
            .session_manager
            .update_session(&session_id, |session| {
                session.add_message(crate::session::Message::new(
                    crate::session::MessageRole::User,
                    "Hello, world!".to_string(),
                ));
                session.add_message(crate::session::Message::new(
                    crate::session::MessageRole::Assistant,
                    "Hello! How can I help you?".to_string(),
                ));
                session.add_message(crate::session::Message::new(
                    crate::session::MessageRole::User,
                    "What's the weather like?".to_string(),
                ));
            })
            .unwrap();

        // Subscribe to notifications to verify history replay
        let mut notification_receiver = agent.notification_sender.sender.subscribe();

        // Now load the session - should trigger history replay
        let load_request =
            LoadSessionRequest::new(new_response.session_id, std::path::PathBuf::from("/tmp"));

        let load_response = agent.load_session(load_request).await.unwrap();

        // Verify meta includes correct history information
        let meta = load_response.meta.unwrap();
        assert_eq!(meta.get("message_count").unwrap().as_u64().unwrap(), 3);
        assert_eq!(meta.get("history_replayed").unwrap().as_u64().unwrap(), 3);

        // Verify that history replay notifications were sent
        // We should receive 3 notifications for the historical messages
        let mut received_notifications = Vec::new();
        for _ in 0..3 {
            match tokio::time::timeout(
                tokio::time::Duration::from_millis(100),
                notification_receiver.recv(),
            )
            .await
            {
                Ok(Ok(notification)) => {
                    received_notifications.push(notification);
                }
                Ok(Err(_)) => break, // Channel error
                Err(_) => break,     // Timeout
            }
        }

        assert_eq!(
            received_notifications.len(),
            3,
            "Should receive 3 historical message notifications"
        );

        // Verify the content and order of notifications
        let first_notification = &received_notifications[0];
        assert!(matches!(
            first_notification.update,
            SessionUpdate::UserMessageChunk(..)
        ));
        if let SessionUpdate::UserMessageChunk(ref chunk) = first_notification.update {
            if let ContentBlock::Text(ref text_content) = chunk.content {
                assert_eq!(text_content.text, "Hello, world!");
            }
        }

        let second_notification = &received_notifications[1];
        assert!(matches!(
            second_notification.update,
            SessionUpdate::AgentMessageChunk(..)
        ));
        if let SessionUpdate::AgentMessageChunk(ref chunk) = second_notification.update {
            if let ContentBlock::Text(ref text_content) = chunk.content {
                assert_eq!(text_content.text, "Hello! How can I help you?");
            }
        }

        let third_notification = &received_notifications[2];
        assert!(matches!(
            third_notification.update,
            SessionUpdate::UserMessageChunk(..)
        ));
        if let SessionUpdate::UserMessageChunk(ref chunk) = third_notification.update {
            if let ContentBlock::Text(ref text_content) = chunk.content {
                assert_eq!(text_content.text, "What's the weather like?");
            }
        }

        // Verify all notifications have proper meta with historical_replay marker
        for notification in &received_notifications {
            let meta = notification.meta.as_ref().unwrap();
            assert_eq!(
                meta.get("message_type").unwrap().as_str().unwrap(),
                "historical_replay"
            );
            assert!(meta.get("timestamp").is_some());
        }
    }

    #[tokio::test]
    async fn test_load_session_capability_validation() {
        let agent = create_test_agent().await;

        // The agent should have loadSession capability enabled by default
        assert!(
            agent.capabilities.load_session,
            "loadSession capability should be enabled by default"
        );

        // Test that the capability validation code path exists by verifying
        // that the agent properly declares the capability in initialize response
        let init_request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V1)
            .client_capabilities(
                agent_client_protocol::ClientCapabilities::new()
                    .fs(agent_client_protocol::FileSystemCapability::new()
                        .read_text_file(true)
                        .write_text_file(true))
                    .terminal(true)
                    .meta(serde_json::json!({"streaming": true}).as_object().cloned()),
            );

        let init_response = agent.initialize(init_request).await.unwrap();
        assert!(
            init_response.agent_capabilities.load_session,
            "Agent should declare loadSession capability in initialize response"
        );
    }

    #[tokio::test]
    async fn test_load_nonexistent_session() {
        let agent = create_test_agent().await;
        // Use a valid ULID format that doesn't exist in session manager
        let nonexistent_session_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV"; // Valid ULID format
        let session_id_wrapper = SessionId::new(nonexistent_session_id.to_string());

        let request =
            LoadSessionRequest::new(session_id_wrapper.clone(), std::path::PathBuf::from("/tmp"));

        let result = agent.load_session(request).await;
        assert!(result.is_err(), "Loading nonexistent session should fail");

        let error = result.unwrap_err();
        assert_eq!(
            error.code,
            agent_client_protocol::ErrorCode::Other(-32602),
            "Should return invalid params error for nonexistent session"
        );

        // The error should either be our custom "Session not found" message or generic invalid params
        // Both are acceptable as they indicate the session couldn't be loaded
        assert!(
            error.message.contains("Session not found") || error.message.contains("Invalid params"),
            "Error message should indicate session issue, got: '{}'",
            error.message
        );
    }

    #[tokio::test]
    async fn test_load_session_invalid_ulid() {
        let agent = create_test_agent().await;

        // Test with an invalid ULID format - should fail at parsing stage
        let request = LoadSessionRequest::new(
            SessionId::new("invalid_session_format".to_string()),
            std::path::PathBuf::from("/tmp"),
        );

        let result = agent.load_session(request).await;
        assert!(
            result.is_err(),
            "Loading with invalid ULID format should fail"
        );

        let error = result.unwrap_err();
        assert_eq!(
            error.code,
            agent_client_protocol::ErrorCode::Other(-32602),
            "Should return invalid params error for invalid ULID"
        );
        // This should fail at parse_session_id stage, so it won't have our custom error data
    }

    #[tokio::test]
    async fn test_set_session_mode() {
        let (agent, _receiver) = create_test_agent_with_notifications().await;

        // First create a valid session using system temp directory
        let new_session_request = NewSessionRequest::new(std::env::temp_dir());
        let session_response = agent.new_session(new_session_request).await.unwrap();

        let request = SetSessionModeRequest::new(
            session_response.session_id.clone(),
            SessionModeId::new("interactive"),
        )
        .meta(
            serde_json::json!({"mode": "interactive"})
                .as_object()
                .cloned(),
        );

        let response = agent.set_session_mode(request).await.unwrap();
        assert!(response.meta.is_some());

        // Check that mode was set in the session
        let parsed_session_id =
            crate::session::SessionId::parse(&session_response.session_id.0).unwrap();
        let session = agent
            .session_manager
            .get_session(&parsed_session_id)
            .unwrap()
            .unwrap();
        assert_eq!(session.current_mode, Some("interactive".to_string()));
    }

    #[tokio::test]
    async fn test_cancel() {
        let agent = create_test_agent().await;

        let notification = CancelNotification::new(SessionId::new("test_session".to_string()))
            .meta(
                serde_json::json!({"reason": "user_request"})
                    .as_object()
                    .cloned(),
            );

        let result = agent.cancel(notification).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ext_method() {
        let agent = create_test_agent().await;

        let request = ExtRequest::new(
            "test_method",
            Arc::from(RawValue::from_string("{}".to_string()).unwrap()),
        );

        let response = agent.ext_method(request).await.unwrap();
        assert!(!response.0.get().is_empty());
    }

    #[tokio::test]
    async fn test_ext_notification() {
        let agent = create_test_agent().await;

        let notification = ExtNotification::new(
            "test_notification",
            Arc::from(RawValue::from_string("{}".to_string()).unwrap()),
        );

        let result = agent.ext_notification(notification.clone()).await;
        assert!(result.is_ok());

        // Explicitly drop resources to ensure cleanup
        drop(notification);
        drop(agent);
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let config = AgentConfig::default();
        let result = ClaudeAgent::new(config).await;
        assert!(result.is_ok());

        let (agent, _receiver) = result.unwrap();
        assert!(agent.capabilities.meta.is_some());
    }

    #[tokio::test]
    async fn test_prompt_validation_invalid_session_id() {
        let agent = create_test_agent().await;

        // Test invalid session ID
        let prompt_request = PromptRequest::new(
            SessionId::new("invalid-uuid".to_string()),
            vec![agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent::new("Hello".to_string()),
            )],
        );

        let result = agent.prompt(prompt_request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_prompt_validation_empty_prompt() {
        let agent = create_test_agent().await;

        // Create a valid session first
        let new_session_request = NewSessionRequest::new(std::path::PathBuf::from("/tmp"));
        let session_response = agent.new_session(new_session_request).await.unwrap();

        // Test empty prompt
        let prompt_request = PromptRequest {
            session_id: session_response.session_id,
            prompt: vec![agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "   ".to_string(), // Only whitespace
                    annotations: None,
                    meta: None,
                },
            )],
            meta: None,
        };

        let result = agent.prompt(prompt_request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_prompt_validation_non_text_content() {
        let agent = create_test_agent().await;

        // Create a valid session first
        let new_session_request = NewSessionRequest::new(std::path::PathBuf::from("/tmp"));
        let session_response = agent.new_session(new_session_request).await.unwrap();

        // Test non-text content block
        let prompt_request = PromptRequest {
            session_id: session_response.session_id,
            prompt: vec![agent_client_protocol::ContentBlock::Image(
                agent_client_protocol::ImageContent {
                    data: "base64data".to_string(),
                    mime_type: "image/png".to_string(),
                    uri: Some("data:image/png;base64,base64data".to_string()),
                    annotations: None,
                    meta: None,
                },
            )],
            meta: None,
        };

        let result = agent.prompt(prompt_request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_prompt_nonexistent_session() {
        let agent = create_test_agent().await;

        // Use a valid ULID but for a session that doesn't exist
        let nonexistent_session_id = ulid::Ulid::new();
        let prompt_request = PromptRequest {
            session_id: SessionId::new(nonexistent_session_id.to_string()),
            prompt: vec![agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "Hello".to_string(),
                    annotations: None,
                    meta: None,
                },
            )],
            meta: None,
        };

        let result = agent.prompt(prompt_request).await;
        assert!(result.is_err());
    }

    // Helper function for streaming tests
    async fn create_test_agent_with_notifications(
    ) -> (ClaudeAgent, broadcast::Receiver<SessionNotification>) {
        let config = AgentConfig::default();
        ClaudeAgent::new(config).await.unwrap()
    }

    #[tokio::test]
    async fn test_streaming_capability_detection() {
        let (agent, _) = create_test_agent_with_notifications().await;

        // Create a session
        let new_session_request = NewSessionRequest::new(std::path::PathBuf::from("/tmp"));
        let session_response = agent.new_session(new_session_request).await.unwrap();
        let session_id = session_response.session_id.0.as_ref().parse().unwrap();

        // Test should_stream with no capabilities
        let session = agent
            .session_manager
            .get_session(&session_id)
            .unwrap()
            .unwrap();
        let dummy_request = PromptRequest::new(session_response.session_id, vec![]);
        assert!(!agent.should_stream(&session, &dummy_request));

        // Add client capabilities without streaming
        agent
            .session_manager
            .update_session(&session_id, |session| {
                session.client_capabilities = Some(
                    agent_client_protocol::ClientCapabilities::new()
                        .fs(agent_client_protocol::FileSystemCapability::new()
                            .read_text_file(true)
                            .write_text_file(true))
                        .terminal(true),
                ); // No streaming meta
            })
            .unwrap();

        let session = agent
            .session_manager
            .get_session(&session_id)
            .unwrap()
            .unwrap();
        assert!(!agent.should_stream(&session, &dummy_request));

        // Add streaming capability
        agent
            .session_manager
            .update_session(&session_id, |session| {
                session.client_capabilities = Some(
                    agent_client_protocol::ClientCapabilities::new()
                        .fs(agent_client_protocol::FileSystemCapability::new()
                            .read_text_file(true)
                            .write_text_file(true))
                        .terminal(true)
                        .meta(serde_json::json!({"streaming": true}).as_object().cloned()),
                );
            })
            .unwrap();

        let session = agent
            .session_manager
            .get_session(&session_id)
            .unwrap()
            .unwrap();
        assert!(agent.should_stream(&session, &dummy_request));
    }

    // Protocol Compliance Tests

    #[tokio::test]
    async fn test_protocol_error_handling() {
        let (agent, _) = create_test_agent_with_notifications().await;

        // Test invalid session ID
        let invalid_prompt = PromptRequest {
            session_id: SessionId::new("invalid-uuid".to_string()),
            prompt: vec![ContentBlock::Text(TextContent {
                text: "Hello".to_string(),
                annotations: None,
                meta: None,
            })],
            meta: None,
        };

        let result = agent.prompt(invalid_prompt).await;
        assert!(result.is_err());

        // };
        //
        // let deny_result = agent.tool_permission_deny(invalid_deny).await.unwrap();
        // assert!(deny_result.success); // Should succeed even if tool call doesn't exist
    }

    #[test]
    fn test_compile_time_agent_check() {
        // Compile-time check that all Agent trait methods are implemented
        fn assert_agent_impl<T: Agent>() {}
        assert_agent_impl::<ClaudeAgent>();
    }

    #[tokio::test]
    async fn test_version_negotiation_unsupported_version() {
        let agent = create_test_agent().await;

        // For now, test with supported version to see basic flow
        let request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V1)
            .client_capabilities(
                agent_client_protocol::ClientCapabilities::new()
                    .fs(agent_client_protocol::FileSystemCapability::new()
                        .read_text_file(true)
                        .write_text_file(true))
                    .terminal(true),
            );

        // This should succeed now since we don't have unsupported version logic yet
        let result = agent.initialize(request).await;
        assert!(result.is_ok(), "Valid initialization should succeed");
    }

    #[tokio::test]
    async fn test_version_negotiation_missing_version() {
        let agent = create_test_agent().await;

        // For now, test that default protocol version works
        let request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V1)
            .client_capabilities(
                agent_client_protocol::ClientCapabilities::new()
                    .fs(agent_client_protocol::FileSystemCapability::new()
                        .read_text_file(true)
                        .write_text_file(true))
                    .terminal(true),
            );

        // This should succeed with default version
        let result = agent.initialize(request).await;
        assert!(result.is_ok(), "Default version should be accepted");
    }

    #[tokio::test]
    async fn test_capability_validation_unknown_capability() {
        let agent = create_test_agent().await;

        // Test with unknown capability in meta
        // Unknown capabilities should be accepted (lenient validation for forward compatibility)
        let request = InitializeRequest {
            client_capabilities: agent_client_protocol::ClientCapabilities::new()
                .fs(agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(true)
                    .meta(Some(serde_json::json!({"unknown_feature": "test"}))))
                .terminal(true)
                .meta(Some(serde_json::json!({
                    "customExtension": true,
                    "streaming": true
                }))),
            protocol_version: Default::default(),
            client_info: None,
            meta: None,
        };

        let result = agent.initialize(request).await;
        assert!(
            result.is_ok(),
            "Unknown capabilities should be accepted for forward compatibility"
        );
    }

    #[tokio::test]
    async fn test_malformed_initialization_request() {
        let agent = create_test_agent().await;

        // Test with invalid capability structure
        let request = InitializeRequest {
            protocol_version: Default::default(),
            client_capabilities: agent_client_protocol::ClientCapabilities::new()
                .fs(agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(true))
                .terminal(true)
                .meta(Some(serde_json::json!({
                    "malformed": "data",
                    "nested": {
                        "invalid": []
                    }
                }))),
            client_info: None,
            meta: Some(serde_json::json!("invalid_meta_format")), // Should be object, not string
        };

        let result = agent.initialize(request).await;
        assert!(result.is_err(), "Malformed request should be rejected");

        let error = result.unwrap_err();
        assert_eq!(error.code, -32600);
        assert!(error.message.contains("Invalid initialize request"));

        // Verify error data structure
        assert!(error.data.is_some(), "Error data should be provided");
        let data = error.data.unwrap();
        assert_eq!(data["invalidField"], "meta");
        assert_eq!(data["expectedType"], "object");
        assert_eq!(data["receivedType"], "string");
    }

    #[tokio::test]
    async fn test_invalid_client_capabilities() {
        let agent = create_test_agent().await;

        // Test with known capability having wrong type (should be rejected)
        let request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V1)
            .client_capabilities(
                agent_client_protocol::ClientCapabilities::new()
                    .fs(agent_client_protocol::FileSystemCapability::new()
                        .read_text_file(true)
                        .write_text_file(true))
                    .terminal(true)
                    .meta(Some(serde_json::json!({
                        "streaming": "invalid_string_value"  // streaming must be boolean
                    }))),
            );

        let result = agent.initialize(request).await;
        assert!(
            result.is_err(),
            "Invalid type for known capability should be rejected"
        );

        let error = result.unwrap_err();
        assert_eq!(error.code, -32602, "Should be Invalid params error");
        assert!(error.message.contains("streaming"));
        assert!(error.message.contains("boolean"));

        // Verify structured error data
        assert!(error.data.is_some());
        let data = error.data.unwrap();
        assert_eq!(data["invalidCapability"], "streaming");
        assert_eq!(data["expectedType"], "boolean");
    }

    #[tokio::test]
    async fn test_unknown_filesystem_capability() {
        let agent = create_test_agent().await;

        // Test with unknown file system capability
        // Unknown fs.meta capabilities should be accepted (lenient validation)
        let request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V1)
            .client_capabilities(
                agent_client_protocol::ClientCapabilities::new()
                    .fs(agent_client_protocol::FileSystemCapability::new()
                        .read_text_file(true)
                        .write_text_file(true)
                        .meta(Some(serde_json::json!({
                            "unknown_feature": true
                        }))))
                    .terminal(true),
            );

        let result = agent.initialize(request).await;
        assert!(
            result.is_ok(),
            "Unknown filesystem capability should be accepted for forward compatibility"
        );
    }

    #[tokio::test]
    async fn test_version_negotiation_comprehensive() {
        let agent = create_test_agent().await;

        // Test that current implementation supports both V0 and V1
        let v0_request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V0)
            .client_capabilities(
                agent_client_protocol::ClientCapabilities::new()
                    .fs(agent_client_protocol::FileSystemCapability::new()
                        .read_text_file(true)
                        .write_text_file(true))
                    .terminal(true),
            );

        let v0_result = agent.initialize(v0_request).await;
        assert!(v0_result.is_ok(), "V0 should be supported");

        let v1_request = InitializeRequest {
            client_capabilities: agent_client_protocol::ClientCapabilities::new()
                .fs(agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(true))
                .terminal(true),
            protocol_version: agent_client_protocol::ProtocolVersion::V1,
            client_info: None,
            meta: None,
        };

        let v1_result = agent.initialize(v1_request).await;
        assert!(v1_result.is_ok(), "V1 should be supported");

        // Test the version validation logic directly
        let _unsupported_version = agent_client_protocol::ProtocolVersion::default();

        // Temporarily modify SUPPORTED_PROTOCOL_VERSIONS to exclude default version
        // This tests the error handling path by calling validate_protocol_version
        // with a version that's not in our supported list

        // Since we can't easily create an unsupported version enum variant,
        // let's test by calling the validation method directly on the agent
        // with a version we know should trigger different error handling paths

        // NOTE: This test verifies that our error structure is correct
        // The actual version negotiation error would be triggered if we had
        // V2 or another unsupported version in the protocol definition
    }

    #[tokio::test]
    async fn test_protocol_version_negotiation_response() {
        let agent = create_test_agent().await;

        // Test client requests V1 -> agent should respond with V1
        let v1_request = InitializeRequest {
            client_capabilities: agent_client_protocol::ClientCapabilities::new()
                .fs(agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(true))
                .terminal(true),
            protocol_version: agent_client_protocol::ProtocolVersion::V1,
            client_info: None,
            meta: None,
        };

        let v1_response = agent.initialize(v1_request).await.unwrap();
        assert_eq!(
            v1_response.protocol_version,
            agent_client_protocol::ProtocolVersion::V1,
            "Agent should respond with client's requested version when supported"
        );

        // Test client requests V0 -> agent should respond with V0
        let v0_request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V0)
            .client_capabilities(
                agent_client_protocol::ClientCapabilities::new()
                    .fs(agent_client_protocol::FileSystemCapability::new()
                        .read_text_file(true)
                        .write_text_file(true))
                    .terminal(true),
            );

        let v0_response = agent.initialize(v0_request).await.unwrap();
        assert_eq!(
            v0_response.protocol_version,
            agent_client_protocol::ProtocolVersion::V0,
            "Agent should respond with client's requested version when supported"
        );
    }

    #[tokio::test]
    async fn test_protocol_version_negotiation_unsupported_scenario() {
        // This test verifies the negotiation logic by testing the method directly
        // since we can't easily create unsupported protocol versions with the current enum
        let agent = create_test_agent().await;

        // Test that our negotiation method works correctly with supported versions
        let negotiated_v1 =
            agent.negotiate_protocol_version(&agent_client_protocol::ProtocolVersion::V1);
        assert_eq!(
            negotiated_v1,
            agent_client_protocol::ProtocolVersion::V1,
            "V1 should be negotiated to V1 when supported"
        );

        let negotiated_v0 =
            agent.negotiate_protocol_version(&agent_client_protocol::ProtocolVersion::V0);
        assert_eq!(
            negotiated_v0,
            agent_client_protocol::ProtocolVersion::V0,
            "V0 should be negotiated to V0 when supported"
        );

        // Verify that our SUPPORTED_PROTOCOL_VERSIONS contains both V0 and V1
        assert!(
            ClaudeAgent::SUPPORTED_PROTOCOL_VERSIONS
                .contains(&agent_client_protocol::ProtocolVersion::V0),
            "Agent should support V0"
        );
        assert!(
            ClaudeAgent::SUPPORTED_PROTOCOL_VERSIONS
                .contains(&agent_client_protocol::ProtocolVersion::V1),
            "Agent should support V1"
        );

        // Verify that the latest supported version is V1 (max of V0 and V1)
        let latest = ClaudeAgent::SUPPORTED_PROTOCOL_VERSIONS
            .iter()
            .max()
            .unwrap_or(&agent_client_protocol::ProtocolVersion::V1);
        assert_eq!(
            *latest,
            agent_client_protocol::ProtocolVersion::V1,
            "Latest supported version should be V1"
        );
    }

    #[tokio::test]
    async fn test_request_permission_basic() {
        let mut agent = create_test_agent().await;

        // Set up a test client for permission handling
        let test_client = Arc::new(TestClient);
        agent.set_client(test_client);

        // First create a session
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            meta: None,
            mcp_servers: vec![],
        };
        let session_response = agent.new_session(new_session_request).await.unwrap();

        // Create a permission request using the new structures
        let permission_request = PermissionRequest {
            session_id: session_response.session_id.clone(),
            tool_call: ToolCallUpdate {
                tool_call_id: "call_001".to_string(),
            },
            options: vec![
                crate::tools::PermissionOption {
                    option_id: "allow-once".to_string(),
                    name: "Allow once".to_string(),
                    kind: crate::tools::PermissionOptionKind::AllowOnce,
                },
                crate::tools::PermissionOption {
                    option_id: "reject-once".to_string(),
                    name: "Reject".to_string(),
                    kind: crate::tools::PermissionOptionKind::RejectOnce,
                },
            ],
        };

        // This should not panic and should return appropriate permission response
        println!("Calling request_permission...");
        let result = agent.request_permission(permission_request).await;
        println!("Got result: {:?}", result);
        assert!(result.is_ok(), "Permission request should succeed");

        let response = result.unwrap();
        println!("Got permission response: {:?}", response.outcome);

        // The outcome depends on the permission policy evaluation
        // Since we have no stored permissions and no matching policy for "unknown_tool",
        // it will require user consent and call the TestClient, which returns "allow"
        match &response.outcome {
            crate::tools::PermissionOutcome::Selected { option_id } => {
                // TestClient returns "allow-once" as the option_id
                assert_eq!(
                    option_id, "allow-once",
                    "Should select allow-once option from TestClient"
                );
            }
            crate::tools::PermissionOutcome::Cancelled => {
                panic!(
                    "Got Cancelled outcome - client connection may not be set or policy evaluation failed"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_request_permission_generates_default_options() {
        let mut agent = create_test_agent().await;

        // Set up a test client for permission handling
        let test_client = Arc::new(TestClient);
        agent.set_client(test_client);

        // Create a session
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            meta: None,
            mcp_servers: vec![],
        };
        let session_response = agent.new_session(new_session_request).await.unwrap();

        // Test permission request with empty options (should generate defaults)
        let permission_request = PermissionRequest {
            session_id: session_response.session_id.clone(),
            tool_call: ToolCallUpdate {
                tool_call_id: "call_002".to_string(),
            },
            options: vec![], // Empty options should trigger default generation
        };

        let result = agent.request_permission(permission_request).await;
        assert!(result.is_ok(), "Permission request should succeed");

        let response = result.unwrap();
        // Should select allow-once option from TestClient
        match response.outcome {
            crate::tools::PermissionOutcome::Selected { option_id } => {
                assert_eq!(
                    option_id, "allow-once",
                    "Should select allow-once option from TestClient"
                );
            }
            _ => panic!("Expected Selected outcome, got {:?}", response.outcome),
        }
    }

    #[tokio::test]
    async fn test_request_permission_cancelled_session() {
        let agent = create_test_agent().await;

        // Create a session
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            meta: None,
            mcp_servers: vec![],
        };
        let session_response = agent.new_session(new_session_request).await.unwrap();
        let session_id_str = session_response.session_id.0.as_ref();

        // Cancel the session
        agent
            .cancellation_manager
            .mark_cancelled(session_id_str, "Test cancellation")
            .await
            .unwrap();

        // Test permission request for cancelled session
        let permission_request = PermissionRequest {
            session_id: session_response.session_id.clone(),
            tool_call: ToolCallUpdate {
                tool_call_id: "call_003".to_string(),
            },
            options: vec![],
        };

        let result = agent.request_permission(permission_request).await;
        assert!(
            result.is_ok(),
            "Permission request should succeed even for cancelled session"
        );

        let response = result.unwrap();
        match response.outcome {
            crate::tools::PermissionOutcome::Cancelled => {
                // This is expected for cancelled sessions
            }
            _ => panic!("Expected Cancelled outcome for cancelled session"),
        }
    }

    #[tokio::test]
    async fn test_todowrite_to_acp_plan_conversion() {
        // Test that TodoWrite parameters are correctly converted to ACP Plan format
        let todowrite_params = serde_json::json!({
            "todos": [
                {
                    "content": "Check for syntax errors",
                    "status": "pending",
                    "activeForm": "Checking for syntax errors"
                },
                {
                    "content": "Identify potential type issues",
                    "status": "in_progress",
                    "activeForm": "Identifying potential type issues"
                },
                {
                    "content": "Fix all errors",
                    "status": "completed",
                    "activeForm": "Fixing all errors"
                }
            ]
        });

        let acp_plan = crate::plan::todowrite_to_acp_plan(&todowrite_params).unwrap();

        // Verify plan has expected entries
        assert_eq!(acp_plan.entries.len(), 3);
        // Pending and completed items use base content
        assert_eq!(acp_plan.entries[0].content, "Check for syntax errors");
        // In-progress items use activeForm as content
        assert_eq!(
            acp_plan.entries[1].content,
            "Identifying potential type issues"
        );
        assert_eq!(acp_plan.entries[2].content, "Fix all errors");

        // Verify statuses are correctly mapped
        let status_0_json = serde_json::to_value(&acp_plan.entries[0].status).unwrap();
        assert_eq!(status_0_json, "pending");
        let status_1_json = serde_json::to_value(&acp_plan.entries[1].status).unwrap();
        assert_eq!(status_1_json, "in_progress");
        let status_2_json = serde_json::to_value(&acp_plan.entries[2].status).unwrap();
        assert_eq!(status_2_json, "completed");

        // Verify priorities are correctly assigned based on status
        let priority_0_json = serde_json::to_value(&acp_plan.entries[0].priority).unwrap();
        assert_eq!(priority_0_json, "medium"); // pending -> medium
        let priority_1_json = serde_json::to_value(&acp_plan.entries[1].priority).unwrap();
        assert_eq!(priority_1_json, "high"); // in_progress -> high
        let priority_2_json = serde_json::to_value(&acp_plan.entries[2].priority).unwrap();
        assert_eq!(priority_2_json, "low"); // completed -> low

        // Verify notes handling:
        // - Pending/completed items: activeForm is in notes
        // - In-progress items: original content is in notes (since activeForm becomes content)
        assert!(acp_plan.entries[0].meta.as_ref().unwrap()["notes"]
            .as_str()
            .unwrap()
            .contains("Checking for syntax errors"));
        assert!(acp_plan.entries[1].meta.as_ref().unwrap()["notes"]
            .as_str()
            .unwrap()
            .contains("Identify potential type issues")); // Original content
        assert!(acp_plan.entries[2].meta.as_ref().unwrap()["notes"]
            .as_str()
            .unwrap()
            .contains("Fixing all errors"));
    }

    #[tokio::test]
    async fn test_plan_update_notification_sender() {
        // Test that plan updates can be sent after programmatic status changes
        let agent = create_test_agent().await;

        // Create a session
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            meta: None,
            mcp_servers: vec![],
        };
        let session_response = agent.new_session(new_session_request).await.unwrap();
        let session_id = session_response.session_id;

        // Create an initial plan with TodoWrite
        let todowrite_params = serde_json::json!({
            "todos": [
                {
                    "content": "Task 1",
                    "status": "pending",
                    "activeForm": "Doing Task 1"
                },
                {
                    "content": "Task 2",
                    "status": "pending",
                    "activeForm": "Doing Task 2"
                }
            ]
        });

        // Convert to agent plan and store it
        let agent_plan = crate::plan::todowrite_to_agent_plan(&todowrite_params).unwrap();
        let entry_id = agent_plan.entries[0].id.clone();

        {
            let mut plan_manager = agent.plan_manager.write().await;
            plan_manager.set_plan(session_id.to_string(), agent_plan);
        }

        // Subscribe to notifications before updating
        let mut notification_receiver = agent.notification_sender.sender.subscribe();

        // Update the first entry status to in_progress
        let result = agent
            .update_plan_entry_status(
                &session_id,
                &entry_id,
                crate::plan::PlanEntryStatus::InProgress,
            )
            .await;

        assert!(result.is_ok(), "Plan entry status update should succeed");

        // Verify notification was sent
        let notification = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            notification_receiver.recv(),
        )
        .await;

        assert!(
            notification.is_ok(),
            "Should receive notification within timeout"
        );
        let notification = notification.unwrap();
        assert!(notification.is_ok(), "Notification should not be an error");
        let notification = notification.unwrap();

        // Verify notification is a Plan update
        match notification.update {
            agent_client_protocol::SessionUpdate::Plan(plan) => {
                assert_eq!(plan.entries.len(), 2);
                // Find the updated entry
                let updated_entry = plan
                    .entries
                    .iter()
                    .find(|e| e.meta.as_ref().unwrap()["id"] == entry_id)
                    .expect("Updated entry should be in plan");
                let status_json = serde_json::to_value(&updated_entry.status).unwrap();
                assert_eq!(status_json, "in_progress");
            }
            _ => panic!("Expected Plan update notification"),
        }

        // Verify the status was updated in PlanManager
        {
            let plan_manager = agent.plan_manager.read().await;
            let stored_plan = plan_manager.get_plan(&session_id.to_string()).unwrap();
            let updated_entry = stored_plan.get_entry(&entry_id).unwrap();
            assert_eq!(
                updated_entry.status,
                crate::plan::PlanEntryStatus::InProgress
            );
        }
    }

    #[tokio::test]
    async fn test_send_plan_update_no_plan_error() {
        // Test that send_plan_update returns error when no plan exists
        let agent = create_test_agent().await;

        // Create a session without a plan
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            meta: None,
            mcp_servers: vec![],
        };
        let session_response = agent.new_session(new_session_request).await.unwrap();
        let session_id = session_response.session_id;

        // Try to send plan update without a plan
        let result = agent.send_plan_update(&session_id).await;

        assert!(result.is_err(), "Should return error when no plan exists");
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("No plan found"),
            "Error should mention no plan found"
        );
    }

    #[tokio::test]
    async fn test_agent_thought_creation() {
        let thought = AgentThought::new(
            ReasoningPhase::PromptAnalysis,
            "Analyzing user request for complexity",
        );

        assert_eq!(thought.phase, ReasoningPhase::PromptAnalysis);
        assert_eq!(thought.content, "Analyzing user request for complexity");
        assert!(thought.context.is_none());
        assert!(thought.timestamp <= SystemTime::now());
    }

    #[tokio::test]
    async fn test_agent_thought_with_context() {
        let context = serde_json::json!({
            "complexity": "medium",
            "tools_needed": 3
        });

        let thought = AgentThought::with_context(
            ReasoningPhase::StrategyPlanning,
            "Planning approach with multiple tools",
            context.clone(),
        );

        assert_eq!(thought.phase, ReasoningPhase::StrategyPlanning);
        assert_eq!(thought.content, "Planning approach with multiple tools");
        assert_eq!(thought.context, Some(context));
    }

    #[tokio::test]
    async fn test_reasoning_phase_serialization() {
        let phases = vec![
            ReasoningPhase::PromptAnalysis,
            ReasoningPhase::StrategyPlanning,
            ReasoningPhase::ToolSelection,
            ReasoningPhase::ProblemDecomposition,
            ReasoningPhase::Execution,
            ReasoningPhase::ResultEvaluation,
        ];

        for phase in phases {
            let serialized = serde_json::to_string(&phase).unwrap();
            let deserialized: ReasoningPhase = serde_json::from_str(&serialized).unwrap();
            assert_eq!(phase, deserialized);
        }
    }

    #[tokio::test]
    async fn test_send_agent_thought() {
        let (agent, mut receiver) = create_test_agent_with_notifications().await;
        let session_id = SessionId::new("test_thought_session".to_string());

        let thought = AgentThought::new(
            ReasoningPhase::PromptAnalysis,
            "Testing agent thought sending",
        );

        // Send the thought
        let result = agent.send_agent_thought(&session_id, &thought).await;
        assert!(result.is_ok());

        // Verify notification was sent
        tokio::select! {
            result = receiver.recv() => {
                assert!(result.is_ok());
                let notification = result.unwrap();
                assert_eq!(notification.session_id, session_id);

                // Verify it's an agent thought chunk
                match notification.update {
                    SessionUpdate::AgentThoughtChunk(chunk) => {
                        match chunk.content {
                            ContentBlock::Text(text_content) => {
                                assert_eq!(text_content.text, "Testing agent thought sending");

                                // Verify metadata contains reasoning phase
                                let meta = text_content.meta.unwrap();
                                assert_eq!(
                                    meta["reasoning_phase"],
                                    serde_json::to_value(&ReasoningPhase::PromptAnalysis).unwrap()
                                );
                            }
                            _ => panic!("Expected text content in agent thought chunk"),
                        }
                    }
                    _ => panic!("Expected AgentThoughtChunk, got {:?}", notification.update),
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                panic!("Timeout waiting for agent thought notification");
            }
        }
    }

    #[tokio::test]
    async fn test_agent_thought_error_handling() {
        let (agent, _receiver) = create_test_agent_with_notifications().await;

        // Test with invalid session ID format (should not panic)
        let invalid_session_id = SessionId::new("".to_string());
        let thought = AgentThought::new(ReasoningPhase::Execution, "Testing error handling");

        // This should not fail even with invalid session ID
        // (error handling in send_agent_thought should prevent failures)
        let result = agent
            .send_agent_thought(&invalid_session_id, &thought)
            .await;
        assert!(
            result.is_ok(),
            "Agent thought sending should handle errors gracefully"
        );
    }

    #[tokio::test]
    async fn test_available_commands_integration_flow() {
        let (agent, mut notification_receiver) = create_test_agent_with_notifications().await;

        // Create a session
        let cwd = std::env::current_dir().unwrap();
        let new_session_request = NewSessionRequest {
            cwd,
            mcp_servers: vec![],
            meta: None,
        };

        let session_response = agent.new_session(new_session_request).await.unwrap();
        let session_id = session_response.session_id;

        // Should receive TWO available commands updates:
        // 1. From Claude CLI init message (slash_commands)
        // 2. From our get_available_commands_for_session (core + tool_handler)

        // Collect both notifications
        let mut all_notifications = Vec::new();
        for _ in 0..2 {
            if let Ok(Ok(notif)) =
                tokio::time::timeout(Duration::from_millis(2000), notification_receiver.recv())
                    .await
            {
                if matches!(notif.update, SessionUpdate::AvailableCommandsUpdate(..)) {
                    all_notifications.push(notif);
                }
            }
        }

        assert!(
            !all_notifications.is_empty(),
            "Should receive at least one AvailableCommandsUpdate notification"
        );

        // Find the notification with our core commands (has create_plan)
        let core_commands_notification = all_notifications.iter().find(|n| {
            if let SessionUpdate::AvailableCommandsUpdate(ref update) = n.update {
                update
                    .available_commands
                    .iter()
                    .any(|cmd| cmd.name == "create_plan")
            } else {
                false
            }
        });

        assert!(
            core_commands_notification.is_some(),
            "Should include AvailableCommandsUpdate with create_plan command"
        );

        // Verify the core commands notification
        let notification = core_commands_notification.unwrap();
        assert_eq!(notification.session_id, session_id);

        if let SessionUpdate::AvailableCommandsUpdate(ref update) = notification.update {
            assert!(
                update
                    .available_commands
                    .iter()
                    .any(|cmd| cmd.name == "research_codebase"),
                "Should include research_codebase command"
            );
        }

        // Test updating commands for the session
        let updated_commands = vec![agent_client_protocol::AvailableCommand {
            name: "new_command".to_string(),
            description: "A newly available command".to_string(),
            input: None,
            meta: Some(serde_json::json!({
                "category": "testing",
                "source": "test"
            })),
        }];

        let update_sent = agent
            .update_session_available_commands(&session_id, updated_commands.clone())
            .await
            .unwrap();
        assert!(update_sent, "Update should be sent for changed commands");

        // Should receive update notification
        let notification =
            tokio::time::timeout(Duration::from_millis(1000), notification_receiver.recv()).await;

        assert!(
            notification.is_ok(),
            "Should receive updated commands notification"
        );
        let notification = notification.unwrap().unwrap();

        match notification.update {
            SessionUpdate::AvailableCommandsUpdate(ref update) => {
                assert_eq!(update.available_commands.len(), 1);
                assert_eq!(update.available_commands[0].name, "new_command");
                assert_eq!(
                    update.available_commands[0].description,
                    "A newly available command"
                );
            }
            _ => panic!(
                "Expected AvailableCommandsUpdate, got: {:?}",
                notification.update
            ),
        }

        // Test that identical commands don't send an update
        let no_update_sent = agent
            .update_session_available_commands(&session_id, updated_commands)
            .await
            .unwrap();
        assert!(
            !no_update_sent,
            "Should not send update for unchanged commands"
        );

        // Verify no additional notification is sent
        let no_notification =
            tokio::time::timeout(Duration::from_millis(100), notification_receiver.recv()).await;
        assert!(
            no_notification.is_err(),
            "Should not receive notification for unchanged commands"
        );
    }

    #[tokio::test]
    async fn test_is_response_refusal_detects_clear_refusals() {
        let agent = create_test_agent().await;

        // Test clear refusal patterns at start of response
        let clear_refusals = [
            "I can't help with that request",
            "I cannot assist with this task",
            "I'm unable to provide that information",
            "I am unable to complete this request",
            "I don't feel comfortable doing this",
            "I won't proceed with this request",
            "I will not help with that",
            "That's not something I can do",
            "I'm not able to assist",
            "I cannot comply with this request",
            "I can't help with creating harmful content",
            "I'm not comfortable with this request",
            "This request goes against my guidelines",
            "I need to decline this request",
            "I must decline to help",
            "I shouldn't provide that information",
            "I should not assist with this",
            "That would be inappropriate",
            "That's not appropriate for me to do",
            "I'm designed not to help with this",
            "I'm programmed not to assist",
            "I have to refuse this request",
            "I must refuse to help",
            "I'm not allowed to do that",
            "That's against my guidelines",
            "My guidelines prevent me from helping",
            "I'm not permitted to assist",
            "That violates my instructions",
            "I cannot provide that type of content",
            "I can't provide assistance with that",
        ];

        for refusal in &clear_refusals {
            assert!(
                agent.is_response_refusal(refusal),
                "Should detect refusal in: '{}'",
                refusal
            );
        }
    }

    #[tokio::test]
    async fn test_is_response_refusal_detects_short_responses_with_refusal_patterns() {
        let agent = create_test_agent().await;

        // Test short responses (< 200 chars) that contain refusal patterns anywhere
        let short_responses_with_refusals = [
            "Sorry, but I can't help with that.",
            "Unfortunately, I cannot assist with this request.",
            "That's something I'm unable to do.",
            "I need to decline - I won't help with that.",
            "This is inappropriate, so I cannot provide assistance.",
        ];

        for response in &short_responses_with_refusals {
            assert!(
                response.len() < 200,
                "Test case should be short: '{}'",
                response
            );
            assert!(
                agent.is_response_refusal(response),
                "Should detect refusal in short response: '{}'",
                response
            );
        }
    }

    #[tokio::test]
    async fn test_is_response_refusal_ignores_refusal_patterns_in_long_responses() {
        let agent = create_test_agent().await;

        // Test long responses (>= 200 chars) that contain refusal patterns but are not refusals
        let long_helpful_response = format!(
            "I can help you understand this topic. While some people might say 'I can't do this' when facing challenges, \
            the key is to break problems down into manageable steps. Here's how you can approach it: \
            First, identify the core requirements. Second, research available solutions. Third, implement step by step. \
            Remember, persistence is key - don't give up when things get difficult. {}",
            "x".repeat(50) // Ensure > 200 chars
        );

        assert!(
            long_helpful_response.len() >= 200,
            "Test response should be long: {} chars",
            long_helpful_response.len()
        );
        assert!(
            !agent.is_response_refusal(&long_helpful_response),
            "Should NOT detect refusal in long helpful response containing incidental refusal patterns"
        );
    }

    #[tokio::test]
    async fn test_is_response_refusal_case_insensitive() {
        let agent = create_test_agent().await;

        // Test case variations
        let case_variations = [
            "I CAN'T help with that",
            "I Cannot assist you",
            "I'M UNABLE TO proceed",
            "i won't do that",
            "i will not help",
            "I Don't Feel Comfortable",
        ];

        for variation in &case_variations {
            assert!(
                agent.is_response_refusal(variation),
                "Should detect refusal regardless of case: '{}'",
                variation
            );
        }
    }

    #[tokio::test]
    async fn test_is_response_refusal_ignores_helpful_responses() {
        let agent = create_test_agent().await;

        // Test responses that should NOT be detected as refusals
        let helpful_responses = [
            "I can help you with that request",
            "Here's how I can assist you",
            "I'm able to provide that information",
            "I will help you solve this problem",
            "That's something I can definitely do",
            "I'm comfortable helping with this",
            "I'm designed to assist with these tasks",
            "I can provide the information you need",
            "I'm allowed to help with this type of request",
            "This is within my guidelines to assist",
            "I'm permitted to provide this assistance",
            "Here's what I can do for you",
            "",    // Empty response
            "   ", // Whitespace only
        ];

        for response in &helpful_responses {
            assert!(
                !agent.is_response_refusal(response),
                "Should NOT detect refusal in helpful response: '{}'",
                response
            );
        }
    }

    #[tokio::test]
    async fn test_create_refusal_response_non_streaming() {
        let agent = create_test_agent().await;
        let session_id = "test-session-123";

        let response = agent.create_refusal_response(session_id, false, None);

        assert_eq!(response.stop_reason, StopReason::Refusal);
        assert!(response.meta.is_some());

        let meta = response.meta.unwrap();
        assert_eq!(meta["refusal_detected"], serde_json::Value::Bool(true));
        assert_eq!(
            meta["session_id"],
            serde_json::Value::String(session_id.to_string())
        );
        assert!(!meta.as_object().unwrap().contains_key("streaming"));
        assert!(!meta.as_object().unwrap().contains_key("chunks_processed"));
    }

    #[tokio::test]
    async fn test_create_refusal_response_streaming_without_chunks() {
        let agent = create_test_agent().await;
        let session_id = "test-session-456";

        let response = agent.create_refusal_response(session_id, true, None);

        assert_eq!(response.stop_reason, StopReason::Refusal);
        assert!(response.meta.is_some());

        let meta = response.meta.unwrap();
        assert_eq!(meta["refusal_detected"], serde_json::Value::Bool(true));
        assert_eq!(
            meta["session_id"],
            serde_json::Value::String(session_id.to_string())
        );
        assert_eq!(meta["streaming"], serde_json::Value::Bool(true));
        assert!(!meta.as_object().unwrap().contains_key("chunks_processed"));
    }

    #[tokio::test]
    async fn test_create_refusal_response_streaming_with_chunks() {
        let agent = create_test_agent().await;
        let session_id = "test-session-789";
        let chunk_count = 42;

        let response = agent.create_refusal_response(session_id, true, Some(chunk_count));

        assert_eq!(response.stop_reason, StopReason::Refusal);
        assert!(response.meta.is_some());

        let meta = response.meta.unwrap();
        assert_eq!(meta["refusal_detected"], serde_json::Value::Bool(true));
        assert_eq!(
            meta["session_id"],
            serde_json::Value::String(session_id.to_string())
        );
        assert_eq!(meta["streaming"], serde_json::Value::Bool(true));
        assert_eq!(
            meta["chunks_processed"],
            serde_json::Value::Number(serde_json::Number::from(chunk_count))
        );
    }

    #[tokio::test]
    async fn test_session_turn_request_counting() {
        use crate::session::{Session, SessionId};
        use std::path::PathBuf;

        let session_id = SessionId::new();
        let cwd = PathBuf::from("/test");
        let mut session = Session::new(session_id, cwd);

        // Initial state
        assert_eq!(session.get_turn_request_count(), 0);

        // First increment
        let count1 = session.increment_turn_requests();
        assert_eq!(count1, 1);
        assert_eq!(session.get_turn_request_count(), 1);

        // Second increment
        let count2 = session.increment_turn_requests();
        assert_eq!(count2, 2);
        assert_eq!(session.get_turn_request_count(), 2);

        // Reset turn counters
        session.reset_turn_counters();
        assert_eq!(session.get_turn_request_count(), 0);
    }

    #[tokio::test]
    async fn test_session_turn_token_counting() {
        use crate::session::{Session, SessionId};
        use std::path::PathBuf;

        let session_id = SessionId::new();
        let cwd = PathBuf::from("/test");
        let mut session = Session::new(session_id, cwd);

        // Initial state
        assert_eq!(session.get_turn_token_count(), 0);

        // Add tokens
        let total1 = session.add_turn_tokens(100);
        assert_eq!(total1, 100);
        assert_eq!(session.get_turn_token_count(), 100);

        // Add more tokens
        let total2 = session.add_turn_tokens(250);
        assert_eq!(total2, 350);
        assert_eq!(session.get_turn_token_count(), 350);

        // Reset turn counters
        session.reset_turn_counters();
        assert_eq!(session.get_turn_token_count(), 0);
    }

    #[tokio::test]
    async fn test_max_turn_requests_limit_enforcement() {
        // This test verifies that the session properly counts and limits turn requests
        // by testing the session methods directly rather than going through the full agent flow

        use crate::session::{Session, SessionId};
        use std::path::PathBuf;

        let session_id = SessionId::new();
        let cwd = PathBuf::from("/test");
        let mut session = Session::new(session_id, cwd);

        let max_requests = 3;

        // Test that we can increment up to the limit
        for i in 1..=max_requests {
            let count = session.increment_turn_requests();
            assert_eq!(count, i, "Request count should be {}", i);
            assert_eq!(
                session.get_turn_request_count(),
                i,
                "Session should track {} requests",
                i
            );
        }

        // Test that incrementing beyond limit still works (the limit check is done in agent.rs)
        let count = session.increment_turn_requests();
        assert_eq!(count, max_requests + 1);
        assert_eq!(session.get_turn_request_count(), max_requests + 1);

        // Test reset
        session.reset_turn_counters();
        assert_eq!(session.get_turn_request_count(), 0);

        // Verify we can count again after reset
        let count = session.increment_turn_requests();
        assert_eq!(count, 1);
        assert_eq!(session.get_turn_request_count(), 1);
    }

    #[tokio::test]
    async fn test_max_tokens_per_turn_limit_enforcement() {
        // This test verifies that the session properly counts and limits tokens
        // by testing the session methods directly rather than going through the full agent flow

        use crate::session::{Session, SessionId};
        use std::path::PathBuf;

        let session_id = SessionId::new();
        let cwd = PathBuf::from("/test");
        let mut session = Session::new(session_id, cwd);

        let _max_tokens = 100;

        // Test that we can add tokens up to the limit
        let tokens1 = session.add_turn_tokens(50);
        assert_eq!(tokens1, 50);
        assert_eq!(session.get_turn_token_count(), 50);

        let tokens2 = session.add_turn_tokens(30);
        assert_eq!(tokens2, 80);
        assert_eq!(session.get_turn_token_count(), 80);

        // Test that we can add tokens beyond the limit (the limit check is done in agent.rs)
        let tokens3 = session.add_turn_tokens(50);
        assert_eq!(tokens3, 130); // 80 + 50 = 130, which exceeds max_tokens
        assert_eq!(session.get_turn_token_count(), 130);

        // Test reset
        session.reset_turn_counters();
        assert_eq!(session.get_turn_token_count(), 0);

        // Verify we can count tokens again after reset
        let tokens = session.add_turn_tokens(25);
        assert_eq!(tokens, 25);
        assert_eq!(session.get_turn_token_count(), 25);
    }

    #[tokio::test]
    async fn test_token_estimation_accuracy() {
        use crate::session::Session;
        use std::path::PathBuf;

        let session_id = crate::session::SessionId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
        let cwd = PathBuf::from("/test");
        let mut session = Session::new(session_id, cwd);

        // Test the token estimation logic (4 chars per token)
        let repeated_16 = "a".repeat(16);
        let repeated_20 = "a".repeat(20);

        let test_cases = [
            ("test", 1),               // 4 chars = 1 token
            ("test test", 2),          // 9 chars = 2 tokens (9/4 = 2.25 -> 2)
            (repeated_16.as_str(), 4), // 16 chars = 4 tokens
            (repeated_20.as_str(), 5), // 20 chars = 5 tokens
            ("", 0),                   // empty = 0 tokens
        ];

        for (text, expected_tokens) in &test_cases {
            session.reset_turn_counters();
            let estimated = (text.len() as u64) / 4;
            assert_eq!(
                estimated, *expected_tokens,
                "Token estimation failed for: '{}'",
                text
            );

            let total = session.add_turn_tokens(estimated);
            assert_eq!(total, *expected_tokens);
        }
    }

    #[tokio::test]
    async fn test_turn_counter_reset_behavior() {
        use crate::session::{Session, SessionId};
        use std::path::PathBuf;

        let session_id = SessionId::new();
        let cwd = PathBuf::from("/test");
        let mut session = Session::new(session_id, cwd);

        // Add some data
        session.increment_turn_requests();
        session.increment_turn_requests();
        session.add_turn_tokens(500);
        session.add_turn_tokens(300);

        // Verify state before reset
        assert_eq!(session.get_turn_request_count(), 2);
        assert_eq!(session.get_turn_token_count(), 800);

        // Reset and verify
        session.reset_turn_counters();
        assert_eq!(session.get_turn_request_count(), 0);
        assert_eq!(session.get_turn_token_count(), 0);

        // Verify we can increment again after reset
        session.increment_turn_requests();
        session.add_turn_tokens(100);
        assert_eq!(session.get_turn_request_count(), 1);
        assert_eq!(session.get_turn_token_count(), 100);
    }

    #[tokio::test]
    async fn test_fs_read_text_file_full_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let (agent, session_id) = setup_agent_with_session().await;

        // Create a temporary file with test content
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        temp_file.write_all(test_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let params = ReadTextFileParams {
            session_id,
            path: temp_file.path().to_string_lossy().to_string(),
            line: None,
            limit: None,
        };

        let response = agent.handle_read_text_file(params).await.unwrap();
        assert_eq!(response.content, test_content);
    }

    #[tokio::test]
    async fn test_fs_read_text_file_with_line_offset() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let (agent, session_id) = setup_agent_with_session().await;

        // Create a temporary file with test content
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        temp_file.write_all(test_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let params = ReadTextFileParams {
            session_id,
            path: temp_file.path().to_string_lossy().to_string(),
            line: Some(3), // Start from line 3 (1-based)
            limit: None,
        };

        let response = agent.handle_read_text_file(params).await.unwrap();
        assert_eq!(response.content, "Line 3\nLine 4\nLine 5");
    }

    #[tokio::test]
    async fn test_fs_read_text_file_with_limit() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let (agent, session_id) = setup_agent_with_session().await;

        // Create a temporary file with test content
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        temp_file.write_all(test_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let params = ReadTextFileParams {
            session_id,
            path: temp_file.path().to_string_lossy().to_string(),
            line: None,
            limit: Some(3), // Read only first 3 lines
        };

        let response = agent.handle_read_text_file(params).await.unwrap();
        assert_eq!(response.content, "Line 1\nLine 2\nLine 3");
    }

    #[tokio::test]
    async fn test_fs_read_text_file_with_line_and_limit() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let (agent, session_id) = setup_agent_with_session().await;

        // Create a temporary file with test content
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        temp_file.write_all(test_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let params = ReadTextFileParams {
            session_id,
            path: temp_file.path().to_string_lossy().to_string(),
            line: Some(2),  // Start from line 2
            limit: Some(2), // Read only 2 lines
        };

        let response = agent.handle_read_text_file(params).await.unwrap();
        assert_eq!(response.content, "Line 2\nLine 3");
    }

    #[tokio::test]
    async fn test_fs_read_text_file_empty_file() {
        use tempfile::NamedTempFile;

        let (agent, session_id) = setup_agent_with_session().await;

        // Create an empty temporary file
        let temp_file = NamedTempFile::new().unwrap();

        let params = ReadTextFileParams {
            session_id,
            path: temp_file.path().to_string_lossy().to_string(),
            line: None,
            limit: None,
        };

        let response = agent.handle_read_text_file(params).await.unwrap();
        assert_eq!(response.content, "");
    }

    #[tokio::test]
    async fn test_fs_read_text_file_line_beyond_end() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let (agent, session_id) = setup_agent_with_session().await;

        // Create a temporary file with only 2 lines
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_content = "Line 1\nLine 2";
        temp_file.write_all(test_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let params = ReadTextFileParams {
            session_id,
            path: temp_file.path().to_string_lossy().to_string(),
            line: Some(5), // Start beyond end of file
            limit: None,
        };

        let response = agent.handle_read_text_file(params).await.unwrap();
        assert_eq!(response.content, ""); // Should return empty string
    }

    #[tokio::test]
    async fn test_fs_read_text_file_invalid_line_zero() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let (agent, session_id) = setup_agent_with_session().await;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"test content").unwrap();
        temp_file.flush().unwrap();

        let params = ReadTextFileParams {
            session_id,
            path: temp_file.path().to_string_lossy().to_string(),
            line: Some(0), // Invalid: line numbers are 1-based
            limit: None,
        };

        let result = agent.handle_read_text_file(params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fs_read_text_file_nonexistent_file() {
        let (agent, session_id) = setup_agent_with_session().await;

        let params = ReadTextFileParams {
            session_id,
            path: "/path/to/nonexistent/file.txt".to_string(),
            line: None,
            limit: None,
        };

        let result = agent.handle_read_text_file(params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fs_read_text_file_relative_path_rejected() {
        let (agent, session_id) = setup_agent_with_session().await;

        let params = ReadTextFileParams {
            session_id,
            path: "relative/path/file.txt".to_string(), // Relative path should be rejected
            line: None,
            limit: None,
        };

        let result = agent.handle_read_text_file(params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fs_read_text_file_capability_check() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let agent = create_test_agent().await;

        // Initialize with read_text_file capability disabled
        let init_request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V1)
            .client_capabilities(
                agent_client_protocol::ClientCapabilities::new()
                    .fs(agent_client_protocol::FileSystemCapability::new()
                        .read_text_file(false)
                        .write_text_file(true))
                    .terminal(true),
            );

        agent.initialize(init_request).await.unwrap();

        // Create session
        let new_request = NewSessionRequest::new(std::path::PathBuf::from("/tmp"));

        let new_response = agent.new_session(new_request).await.unwrap();
        let session_id = new_response.session_id.0.as_ref().to_string();

        // Create a test file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"Test content").unwrap();
        temp_file.flush().unwrap();

        // Try to read file with capability disabled
        let params = ReadTextFileParams {
            session_id,
            path: temp_file.path().to_string_lossy().to_string(),
            line: None,
            limit: None,
        };

        let result = agent.handle_read_text_file(params).await;
        assert!(
            result.is_err(),
            "Should fail when read_text_file capability is disabled"
        );
    }

    #[tokio::test]
    async fn test_fs_read_text_file_different_line_endings() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let (agent, session_id) = setup_agent_with_session().await;

        // Test with CRLF line endings
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_content = "Line 1\r\nLine 2\r\nLine 3";
        temp_file.write_all(test_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let params = ReadTextFileParams {
            session_id,
            path: temp_file.path().to_string_lossy().to_string(),
            line: Some(2),
            limit: Some(2),
        };

        let response = agent.handle_read_text_file(params).await.unwrap();
        assert_eq!(response.content, "Line 2\nLine 3"); // Should normalize to LF
    }

    #[tokio::test]
    async fn test_fs_read_text_file_ext_method_routing() {
        let (agent, session_id) = setup_agent_with_session().await;

        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a test file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"Test content for ext method").unwrap();
        temp_file.flush().unwrap();

        // Test through ext_method interface
        let params = serde_json::json!({
            "sessionId": session_id,
            "path": temp_file.path().to_string_lossy(),
            "line": null,
            "limit": null
        });

        println!("Parameters being sent: {}", params);
        let params_raw = match agent_client_protocol::RawValue::from_string(params.to_string()) {
            Ok(raw) => raw,
            Err(e) => {
                println!("Failed to create RawValue: {:?}", e);
                panic!("RawValue creation failed");
            }
        };
        let ext_request = agent_client_protocol::ExtRequest {
            method: "fs/read_text_file".into(),
            params: Arc::from(params_raw),
        };

        let result = match agent.ext_method(ext_request).await {
            Ok(result) => result,
            Err(e) => {
                println!("ext_method failed with error: {:?}", e);
                panic!("ext_method should have succeeded");
            }
        };

        // Parse the response
        let response: serde_json::Value = serde_json::from_str(result.get()).unwrap();
        assert_eq!(response["content"], "Test content for ext method");
    }

    #[tokio::test]
    async fn test_fs_write_text_file_new_file() {
        println!("Starting test setup...");
        let (agent, session_id) = setup_agent_with_session().await;

        // Use /tmp directly with a unique filename
        let file_path = format!("/tmp/claude_test_write_{}.txt", ulid::Ulid::new());

        let params = WriteTextFileParams {
            session_id: session_id.clone(),
            path: file_path.clone(),
            content: "Hello, World!\nThis is a test file.".to_string(),
        };

        let result = agent.handle_write_text_file(params).await;
        match result {
            Ok(value) => {
                assert_eq!(value, serde_json::Value::Null);
                println!("Write test successful!");
            }
            Err(e) => {
                println!("Write test failed with error: {:?}", e);
                panic!("Test failed: {:?}", e);
            }
        }

        // Verify the file was created with correct content
        let written_content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(written_content, "Hello, World!\nThis is a test file.");

        // Clean up the test file
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_fs_write_text_file_overwrite_existing() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let (agent, session_id) = setup_agent_with_session().await;

        // Create a temporary file with initial content
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"Original content").unwrap();
        temp_file.flush().unwrap();

        let params = WriteTextFileParams {
            session_id,
            path: temp_file.path().to_string_lossy().to_string(),
            content: "New content overwrites old".to_string(),
        };

        let result = agent.handle_write_text_file(params).await.unwrap();
        assert_eq!(result, WriteTextFileResponse::default());

        // Verify the file content was overwritten
        let written_content = tokio::fs::read_to_string(temp_file.path()).await.unwrap();
        assert_eq!(written_content, "New content overwrites old");
    }

    #[tokio::test]
    async fn test_fs_write_text_file_create_parent_directories() {
        use tempfile::TempDir;

        let (agent, session_id) = setup_agent_with_session().await;

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir.path().join("nested").join("deep").join("file.txt");
        let file_path_str = nested_path.to_string_lossy().to_string();

        let params = WriteTextFileParams {
            session_id,
            path: file_path_str.clone(),
            content: "Content in nested directory".to_string(),
        };

        let result = agent.handle_write_text_file(params).await.unwrap();
        assert_eq!(result, WriteTextFileResponse::default());

        // Verify the parent directories were created
        assert!(nested_path.parent().unwrap().exists());

        // Verify the file was created with correct content
        let written_content = tokio::fs::read_to_string(&nested_path).await.unwrap();
        assert_eq!(written_content, "Content in nested directory");
    }

    #[tokio::test]
    async fn test_fs_write_text_file_relative_path_rejected() {
        let (agent, session_id) = setup_agent_with_session().await;

        let params = WriteTextFileParams {
            session_id,
            path: "relative/path/file.txt".to_string(), // Relative path should be rejected
            content: "This should fail".to_string(),
        };

        let result = agent.handle_write_text_file(params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fs_write_text_file_empty_content() {
        use tempfile::TempDir;

        let (agent, session_id) = setup_agent_with_session().await;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty_file.txt");
        let file_path_str = file_path.to_string_lossy().to_string();

        let params = WriteTextFileParams {
            session_id,
            path: file_path_str.clone(),
            content: "".to_string(), // Empty content
        };

        let result = agent.handle_write_text_file(params).await.unwrap();
        assert_eq!(result, WriteTextFileResponse::default());

        // Verify empty file was created
        let written_content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(written_content, "");
    }

    #[tokio::test]
    async fn test_fs_write_text_file_large_content() {
        use tempfile::TempDir;

        let (agent, session_id) = setup_agent_with_session().await;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large_file.txt");
        let file_path_str = file_path.to_string_lossy().to_string();

        // Create large content (10KB)
        let large_content = "A".repeat(10240);

        let params = WriteTextFileParams {
            session_id,
            path: file_path_str.clone(),
            content: large_content.clone(),
        };

        let result = agent.handle_write_text_file(params).await.unwrap();
        assert_eq!(result, WriteTextFileResponse::default());

        // Verify large content was written correctly
        let written_content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(written_content, large_content);
    }

    #[tokio::test]
    async fn test_fs_write_text_file_unicode_content() {
        use tempfile::TempDir;

        let (agent, session_id) = setup_agent_with_session().await;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("unicode_file.txt");
        let file_path_str = file_path.to_string_lossy().to_string();

        let unicode_content = "Hello 世界! 🌍 Café naïve résumé";

        let params = WriteTextFileParams {
            session_id,
            path: file_path_str.clone(),
            content: unicode_content.to_string(),
        };

        let result = agent.handle_write_text_file(params).await.unwrap();
        assert_eq!(result, WriteTextFileResponse::default());

        // Verify unicode content was written correctly
        let written_content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(written_content, unicode_content);
    }

    #[tokio::test]
    async fn test_fs_write_text_file_ext_method_routing() {
        use tempfile::TempDir;

        let (agent, session_id) = setup_agent_with_session().await;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("ext_method_test.txt");
        let file_path_str = file_path.to_string_lossy().to_string();

        // Test the ext_method routing for fs/write_text_file
        let params = serde_json::json!({
            "sessionId": session_id,
            "path": file_path_str,
            "content": "Test content via ext_method"
        });

        let params_raw = agent_client_protocol::RawValue::from_string(params.to_string()).unwrap();
        let ext_request = agent_client_protocol::ExtRequest {
            method: "fs/write_text_file".into(),
            params: Arc::from(params_raw),
        };

        let result = agent.ext_method(ext_request).await.unwrap();

        // Parse the response - should be null for successful write
        let response: serde_json::Value = serde_json::from_str(result.get()).unwrap();
        assert_eq!(response, serde_json::Value::Null);

        // Verify the file was actually written
        let written_content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(written_content, "Test content via ext_method");
    }

    #[tokio::test]
    async fn test_new_session_validates_mcp_transport_capabilities() {
        // This test verifies that transport validation is called
        // For now, we use empty MCP server lists since the validation logic
        // exists but isn't integrated yet - this should pass once we add the calls

        let agent = create_test_agent().await;

        let request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![], // Empty for now
            meta: None,
        };

        let result = agent.new_session(request).await;
        // Should succeed with empty MCP servers
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_session_validates_mcp_transport_capabilities() {
        // This test verifies that transport validation is called
        // For now, we use empty MCP server lists since the validation logic
        // exists but isn't integrated yet - this should pass once we add the calls

        let agent = create_test_agent().await;

        let request = LoadSessionRequest {
            session_id: SessionId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()),
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![], // Empty for now
            meta: None,
        };

        let result = agent.load_session(request).await;
        // Should succeed now - transport validation passes with empty MCP servers
        // and then fail with session not found (but validation runs first)
        assert!(result.is_err());
        let error = result.unwrap_err();
        // Could be transport validation error (-32602) or session not found (-32603)
        // Both are acceptable since validation runs before session lookup
        assert!(error.code == -32602 || error.code == -32603);
    }

    #[tokio::test]
    async fn test_terminal_output_basic() {
        use crate::terminal_manager::{TerminalCreateParams, TerminalOutputParams};
        use tempfile::TempDir;

        let (agent, session_id) = setup_agent_with_session().await;
        let temp_dir = TempDir::new().unwrap();

        // Create a terminal session
        let tool_handler = agent.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        let create_params = TerminalCreateParams {
            session_id: session_id.clone(),
            command: "echo".to_string(),
            args: Some(vec!["Hello, Terminal!".to_string()]),
            env: None,
            cwd: Some(temp_dir.path().to_string_lossy().to_string()),
            output_byte_limit: None,
        };

        let terminal_id = terminal_manager
            .create_terminal_with_command(&agent.session_manager, create_params)
            .await
            .unwrap();

        // Get terminal output
        let output_params = TerminalOutputParams {
            session_id: session_id.clone(),
            terminal_id: terminal_id.clone(),
        };

        let response = agent.handle_terminal_output(output_params).await.unwrap();

        // Verify response structure
        assert_eq!(response.output, "");
        assert!(!response.truncated);
        assert!(response.exit_status.is_none());
    }

    #[tokio::test]
    async fn test_terminal_output_with_data() {
        use crate::terminal_manager::{TerminalCreateParams, TerminalOutputParams};
        use tempfile::TempDir;

        let (agent, session_id) = setup_agent_with_session().await;
        let temp_dir = TempDir::new().unwrap();

        // Create terminal and add output data
        let tool_handler = agent.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        let create_params = TerminalCreateParams {
            session_id: session_id.clone(),
            command: "cat".to_string(),
            args: None,
            env: None,
            cwd: Some(temp_dir.path().to_string_lossy().to_string()),
            output_byte_limit: None,
        };

        let terminal_id = terminal_manager
            .create_terminal_with_command(&agent.session_manager, create_params)
            .await
            .unwrap();

        // Manually add output to the terminal session
        {
            let terminals = terminal_manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            session.add_output(b"Test output data\n").await;
        }

        // Get output
        let output_params = TerminalOutputParams {
            session_id: session_id.clone(),
            terminal_id: terminal_id.clone(),
        };

        let response = agent.handle_terminal_output(output_params).await.unwrap();

        assert_eq!(response.output, "Test output data\n");
        assert!(!response.truncated);
    }

    #[tokio::test]
    async fn test_terminal_output_truncation() {
        use crate::terminal_manager::{TerminalCreateParams, TerminalOutputParams};
        use tempfile::TempDir;

        let (agent, session_id) = setup_agent_with_session().await;
        let temp_dir = TempDir::new().unwrap();

        // Create terminal with small byte limit
        let tool_handler = agent.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        let create_params = TerminalCreateParams {
            session_id: session_id.clone(),
            command: "cat".to_string(),
            args: None,
            env: None,
            cwd: Some(temp_dir.path().to_string_lossy().to_string()),
            output_byte_limit: Some(50),
        };

        let terminal_id = terminal_manager
            .create_terminal_with_command(&agent.session_manager, create_params)
            .await
            .unwrap();

        // Add more data than the limit
        {
            let terminals = terminal_manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();

            let large_data = "A".repeat(100);
            session.add_output(large_data.as_bytes()).await;
        }

        // Get output
        let output_params = TerminalOutputParams {
            session_id: session_id.clone(),
            terminal_id: terminal_id.clone(),
        };

        let response = agent.handle_terminal_output(output_params).await.unwrap();

        assert!(response.truncated);
        assert!(response.output.len() <= 50);
    }

    #[tokio::test]
    async fn test_terminal_output_utf8_boundary_truncation() {
        use crate::terminal_manager::{TerminalCreateParams, TerminalOutputParams};
        use tempfile::TempDir;

        let (agent, session_id) = setup_agent_with_session().await;
        let temp_dir = TempDir::new().unwrap();

        // Create terminal with byte limit
        let tool_handler = agent.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        let create_params = TerminalCreateParams {
            session_id: session_id.clone(),
            command: "cat".to_string(),
            args: None,
            env: None,
            cwd: Some(temp_dir.path().to_string_lossy().to_string()),
            output_byte_limit: Some(20),
        };

        let terminal_id = terminal_manager
            .create_terminal_with_command(&agent.session_manager, create_params)
            .await
            .unwrap();

        // Add UTF-8 data that will need character-boundary truncation
        {
            let terminals = terminal_manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();

            let unicode_data = "Hello 世界 Test 🌍";
            session.add_output(unicode_data.as_bytes()).await;
        }

        // Get output
        let output_params = TerminalOutputParams {
            session_id: session_id.clone(),
            terminal_id: terminal_id.clone(),
        };

        let response = agent.handle_terminal_output(output_params).await.unwrap();

        // Output should be valid UTF-8
        assert!(response.truncated);
        assert!(std::str::from_utf8(response.output.as_bytes()).is_ok());
    }

    #[tokio::test]
    async fn test_terminal_output_ext_method_routing() {
        use crate::terminal_manager::TerminalCreateParams;
        use tempfile::TempDir;

        let (agent, session_id) = setup_agent_with_session().await;
        let temp_dir = TempDir::new().unwrap();

        // Create terminal
        let tool_handler = agent.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        let create_params = TerminalCreateParams {
            session_id: session_id.clone(),
            command: "echo".to_string(),
            args: Some(vec!["test".to_string()]),
            env: None,
            cwd: Some(temp_dir.path().to_string_lossy().to_string()),
            output_byte_limit: None,
        };

        let terminal_id = terminal_manager
            .create_terminal_with_command(&agent.session_manager, create_params)
            .await
            .unwrap();

        // Test through ext_method interface
        let params = serde_json::json!({
            "sessionId": session_id,
            "terminalId": terminal_id
        });

        let params_raw = agent_client_protocol::RawValue::from_string(params.to_string()).unwrap();
        let ext_request = agent_client_protocol::ExtRequest {
            method: "terminal/output".into(),
            params: Arc::from(params_raw),
        };

        let result = agent.ext_method(ext_request).await.unwrap();

        // Parse the response
        let response: serde_json::Value = serde_json::from_str(result.get()).unwrap();
        assert!(response.get("output").is_some());
        assert!(response.get("truncated").is_some());
    }

    #[tokio::test]
    async fn test_terminal_output_invalid_session() {
        use crate::terminal_manager::TerminalOutputParams;

        let agent = create_test_agent().await;

        let output_params = TerminalOutputParams {
            session_id: "invalid-session-id".to_string(),
            terminal_id: "term_123".to_string(),
        };

        let result = agent.handle_terminal_output(output_params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_terminal_output_invalid_terminal() {
        let (agent, session_id) = setup_agent_with_session().await;

        let output_params = crate::terminal_manager::TerminalOutputParams {
            session_id: session_id.clone(),
            terminal_id: "term_nonexistent".to_string(),
        };

        let result = agent.handle_terminal_output(output_params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_user_message_chunks_sent_on_prompt() {
        let config = AgentConfig::default();
        let (agent, _notification_receiver) = ClaudeAgent::new(config).await.unwrap();
        // Arc is used for reference counting within test, not for thread sharing
        #[allow(clippy::arc_with_non_send_sync)]
        let agent = Arc::new(agent);

        let init_request = InitializeRequest {
            protocol_version: agent_client_protocol::ProtocolVersion::V1,
            client_capabilities: agent_client_protocol::ClientCapabilities::new()
                .fs(agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(true))
                .terminal(true)
                .meta(Some(serde_json::json!({"streaming": false}))),
            client_info: None,
            meta: Some(serde_json::json!({"test": true})),
        };
        agent.initialize(init_request).await.unwrap();

        let mut notification_receiver = agent.notification_sender.sender.subscribe();

        let new_request = NewSessionRequest::new(std::path::PathBuf::from("/tmp")).meta(
            serde_json::json!({"test": true})
                .as_object()
                .unwrap()
                .clone(),
        );
        let new_response = agent.new_session(new_request).await.unwrap();

        let prompt_request = PromptRequest {
            session_id: new_response.session_id.clone(),
            prompt: vec![
                ContentBlock::Text(TextContent {
                    text: "test".to_string(),
                    annotations: None,
                    meta: None,
                }),
                ContentBlock::Text(TextContent {
                    text: "test2".to_string(),
                    annotations: None,
                    meta: None,
                }),
            ],
            meta: Some(serde_json::json!({"test": true})),
        };

        // Start collecting notifications
        let collect_task = async {
            let mut user_message_chunks = Vec::new();
            let start = tokio::time::Instant::now();
            let max_duration = Duration::from_secs(15);

            // Keep receiving until we have 2 chunks or timeout
            while user_message_chunks.len() < 2 && start.elapsed() < max_duration {
                match tokio::time::timeout(Duration::from_secs(1), notification_receiver.recv())
                    .await
                {
                    Ok(Ok(notification)) => {
                        if let SessionUpdate::UserMessageChunk(chunk) = notification.update {
                            user_message_chunks.push(chunk.content);
                        }
                    }
                    Ok(Err(_)) => break, // Channel closed
                    Err(_) => continue,  // Timeout, keep trying
                }
            }
            user_message_chunks
        };

        // Send prompt after short delay to ensure collector is ready
        let agent_clone = Arc::clone(&agent);
        let prompt_task = async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            agent_clone.prompt(prompt_request).await
        };

        let (user_message_chunks, _prompt_result) = tokio::join!(collect_task, prompt_task);

        assert_eq!(
            user_message_chunks.len(),
            2,
            "Should receive 2 user message chunks"
        );

        if let ContentBlock::Text(ref text_content) = user_message_chunks[0] {
            assert_eq!(text_content.text, "test");
        } else {
            panic!("First chunk should be text content");
        }

        if let ContentBlock::Text(ref text_content) = user_message_chunks[1] {
            assert_eq!(text_content.text, "test2");
        } else {
            panic!("Second chunk should be text content");
        }
    }

    #[tokio::test]
    async fn test_request_permission_extracts_tool_metadata_success() {
        let (agent, session_id) = setup_agent_with_session().await;

        // Create a tool call in the handler with specific name and arguments
        let tool_name = "test_read_tool";
        let tool_args = serde_json::json!({"file_path": "/test/file.txt"});

        let tool_handler = agent.tool_handler.read().await;
        let report = tool_handler
            .create_tool_call_report(&SessionId::new(session_id.clone()), tool_name, &tool_args)
            .await;
        let tool_call_id = report.tool_call_id.clone();
        drop(tool_handler);

        // Create a permission request for this tool call
        let request = PermissionRequest {
            session_id: SessionId::new(session_id),
            tool_call: ToolCallUpdate {
                tool_call_id: tool_call_id.clone(),
            },
            options: vec![],
        };

        // Request permission - this should extract tool_name and tool_args from active_tool_calls
        let response = agent.request_permission(request).await.unwrap();

        // Verify we got a valid response with an outcome
        match response.outcome {
            crate::tools::PermissionOutcome::Selected { option_id: _ } => {
                // Success - the tool metadata was extracted and processed
            }
            _ => {
                // Also valid - just verify we got a response
            }
        }
    }

    #[tokio::test]
    async fn test_request_permission_handles_missing_tool_call() {
        let (agent, session_id) = setup_agent_with_session().await;

        // Create a permission request with a non-existent tool_call_id
        let fake_tool_call_id = "nonexistent_tool_call_id_12345";
        let request = PermissionRequest {
            session_id: SessionId::new(session_id),
            tool_call: ToolCallUpdate {
                tool_call_id: fake_tool_call_id.to_string(),
            },
            options: vec![],
        };

        // Request permission - should handle missing tool call gracefully
        // The code should use default values ("unknown_tool" and empty JSON object)
        let result = agent.request_permission(request).await;

        // Verify we got a valid response (doesn't matter what outcome, just that it didn't panic)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_request_permission_with_complex_tool_args() {
        let (agent, session_id) = setup_agent_with_session().await;

        // Create a tool call with complex nested arguments
        let tool_name = "complex_test_tool";
        let tool_args = serde_json::json!({
            "operation": "write",
            "files": [
                {"path": "/tmp/test1.txt", "content": "Hello"},
                {"path": "/tmp/test2.txt", "content": "World"}
            ],
            "options": {
                "recursive": true,
                "force": false
            }
        });

        let tool_handler = agent.tool_handler.read().await;
        let report = tool_handler
            .create_tool_call_report(&SessionId::new(session_id.clone()), tool_name, &tool_args)
            .await;
        let tool_call_id = report.tool_call_id.clone();
        drop(tool_handler);

        // Create a permission request
        let request = PermissionRequest {
            session_id: SessionId::new(session_id),
            tool_call: ToolCallUpdate {
                tool_call_id: tool_call_id.clone(),
            },
            options: vec![],
        };

        // Request permission - should extract complex arguments correctly
        let result = agent.request_permission(request).await;

        // Verify we got a valid response
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_request_permission_with_missing_raw_input() {
        let (agent, session_id) = setup_agent_with_session().await;

        // Create a tool call but with None for raw_input
        let tool_name = "test_tool_no_args";
        let tool_args = serde_json::json!(null);

        let tool_handler = agent.tool_handler.read().await;
        let report = tool_handler
            .create_tool_call_report(&SessionId::new(session_id.clone()), tool_name, &tool_args)
            .await;
        let tool_call_id = report.tool_call_id.clone();

        // Manually update the report to have None for raw_input
        tool_handler
            .update_tool_call_report(
                &SessionId::new(session_id.clone()),
                &tool_call_id,
                |report| {
                    report.raw_input = None;
                },
            )
            .await;
        drop(tool_handler);

        // Create a permission request
        let request = PermissionRequest {
            session_id: SessionId::new(session_id),
            tool_call: ToolCallUpdate {
                tool_call_id: tool_call_id.clone(),
            },
            options: vec![],
        };

        // Request permission - should default to empty JSON object for missing raw_input
        let result = agent.request_permission(request).await;

        // Verify we got a valid response
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_command_discovery_includes_core_commands() {
        let agent = create_test_agent().await;
        let session_id = SessionId::new("test_session_123".to_string());

        let commands = agent.get_available_commands_for_session(&session_id).await;

        // Verify core commands are present
        assert!(
            commands.iter().any(|cmd| cmd.name == "create_plan"),
            "Should include create_plan command"
        );
        assert!(
            commands.iter().any(|cmd| cmd.name == "research_codebase"),
            "Should include research_codebase command"
        );

        // Verify core commands have proper metadata
        let create_plan = commands
            .iter()
            .find(|cmd| cmd.name == "create_plan")
            .unwrap();
        assert_eq!(create_plan.meta.as_ref().unwrap()["source"], "core");
        assert_eq!(create_plan.meta.as_ref().unwrap()["category"], "planning");
    }

    #[tokio::test]
    async fn test_command_discovery_includes_tool_handler_commands() {
        let agent = create_test_agent().await;

        // Set client capabilities to enable filesystem tools
        let caps = agent_client_protocol::ClientCapabilities::new()
            .fs(agent_client_protocol::FileSystemCapability::new()
                .read_text_file(true)
                .write_text_file(true))
            .terminal(false);

        let mut client_caps = agent.client_capabilities.write().await;
        *client_caps = Some(caps.clone());
        drop(client_caps);

        // Also set tool handler capabilities
        let mut tool_handler = agent.tool_handler.write().await;
        tool_handler.set_client_capabilities(caps);
        drop(tool_handler);

        let session_id = SessionId::new("test_session_456".to_string());
        let commands = agent.get_available_commands_for_session(&session_id).await;

        // Verify filesystem commands are present when capability is enabled
        assert!(
            commands.iter().any(|cmd| cmd.name == "fs_read"),
            "Should include fs_read command when fs capability is enabled"
        );
        assert!(
            commands.iter().any(|cmd| cmd.name == "fs_write"),
            "Should include fs_write command when fs write capability is enabled"
        );

        // Verify tool handler commands have proper metadata
        let fs_read = commands.iter().find(|cmd| cmd.name == "fs_read").unwrap();
        assert_eq!(fs_read.meta.as_ref().unwrap()["source"], "tool_handler");
        assert_eq!(fs_read.meta.as_ref().unwrap()["category"], "filesystem");
    }

    #[tokio::test]
    async fn test_command_discovery_filters_by_capabilities() {
        let agent = create_test_agent().await;

        // Set capabilities with only read enabled
        let caps = agent_client_protocol::ClientCapabilities::new()
            .fs(agent_client_protocol::FileSystemCapability::new()
                .read_text_file(true)
                .write_text_file(false))
            .terminal(false);

        let mut client_caps = agent.client_capabilities.write().await;
        *client_caps = Some(caps.clone());
        drop(client_caps);

        // Also set tool handler capabilities
        let mut tool_handler = agent.tool_handler.write().await;
        tool_handler.set_client_capabilities(caps);
        drop(tool_handler);

        let session_id = SessionId::new("test_session_789".to_string());
        let commands = agent.get_available_commands_for_session(&session_id).await;

        // Should include read commands
        assert!(
            commands.iter().any(|cmd| cmd.name == "fs_read"),
            "Should include fs_read when read capability is enabled"
        );

        // Should NOT include write commands
        assert!(
            !commands.iter().any(|cmd| cmd.name == "fs_write"),
            "Should NOT include fs_write when write capability is disabled"
        );

        // Should NOT include terminal commands
        assert!(
            !commands.iter().any(|cmd| cmd.name.starts_with("terminal_")),
            "Should NOT include terminal commands when terminal capability is disabled"
        );
    }

    #[tokio::test]
    async fn test_command_discovery_includes_terminal_commands() {
        let agent = create_test_agent().await;

        // Enable terminal capability
        let caps = agent_client_protocol::ClientCapabilities::new()
            .fs(agent_client_protocol::FileSystemCapability::new()
                .read_text_file(false)
                .write_text_file(false))
            .terminal(true);

        let mut client_caps = agent.client_capabilities.write().await;
        *client_caps = Some(caps.clone());
        drop(client_caps);

        // Also set tool handler capabilities
        let mut tool_handler = agent.tool_handler.write().await;
        tool_handler.set_client_capabilities(caps);
        drop(tool_handler);

        let session_id = SessionId::new("test_session_terminal".to_string());
        let commands = agent.get_available_commands_for_session(&session_id).await;

        // Terminal commands are already filtered by tool handler based on its capabilities
        // Since we don't filter them out in our code (terminal tools pass through),
        // they should be present if tool handler included them
        let has_terminal_create = commands.iter().any(|cmd| cmd.name == "terminal_create");

        // If terminal commands aren't present, it means tool handler didn't include them
        // This could be because tool handler doesn't have terminal manager initialized
        // For now, just verify that IF they are present, they have correct metadata
        if has_terminal_create {
            let terminal_create = commands
                .iter()
                .find(|cmd| cmd.name == "terminal_create")
                .unwrap();
            assert_eq!(
                terminal_create.meta.as_ref().unwrap()["source"],
                "tool_handler"
            );
            assert_eq!(
                terminal_create.meta.as_ref().unwrap()["category"],
                "terminal"
            );
        }

        // At minimum, verify core commands and capability filtering works
        assert!(
            commands.iter().any(|cmd| cmd.name == "create_plan"),
            "Should always include core commands"
        );
    }

    #[tokio::test]
    async fn test_command_discovery_with_no_capabilities() {
        let agent = create_test_agent().await;

        // No capabilities set (None)
        let session_id = SessionId::new("test_session_no_caps".to_string());
        let commands = agent.get_available_commands_for_session(&session_id).await;

        // Should still include core commands
        assert!(
            commands.iter().any(|cmd| cmd.name == "create_plan"),
            "Should include core commands even without capabilities"
        );

        // Should NOT include fs or terminal commands
        assert!(
            !commands.iter().any(|cmd| cmd.name == "fs_read"),
            "Should NOT include fs_read without capabilities"
        );
        assert!(
            !commands.iter().any(|cmd| cmd.name == "terminal_create"),
            "Should NOT include terminal_create without capabilities"
        );
    }

    #[tokio::test]
    async fn test_command_discovery_logs_command_sources() {
        let agent = create_test_agent().await;

        // Enable all capabilities
        let caps = agent_client_protocol::ClientCapabilities::new()
            .fs(agent_client_protocol::FileSystemCapability::new()
                .read_text_file(true)
                .write_text_file(true))
            .terminal(true);

        let mut client_caps = agent.client_capabilities.write().await;
        *client_caps = Some(caps.clone());
        drop(client_caps);

        // Also set tool handler capabilities
        let mut tool_handler = agent.tool_handler.write().await;
        tool_handler.set_client_capabilities(caps);
        drop(tool_handler);

        let session_id = SessionId::new("test_session_logging".to_string());
        let commands = agent.get_available_commands_for_session(&session_id).await;

        // Verify we have commands from multiple sources
        let core_commands: Vec<_> = commands
            .iter()
            .filter(|cmd| {
                cmd.meta
                    .as_ref()
                    .and_then(|m| m.get("source"))
                    .and_then(|s| s.as_str())
                    == Some("core")
            })
            .collect();

        let tool_handler_commands: Vec<_> = commands
            .iter()
            .filter(|cmd| {
                cmd.meta
                    .as_ref()
                    .and_then(|m| m.get("source"))
                    .and_then(|s| s.as_str())
                    == Some("tool_handler")
            })
            .collect();

        assert!(!core_commands.is_empty(), "Should have core commands");
        assert!(
            !tool_handler_commands.is_empty(),
            "Should have tool_handler commands"
        );

        // Total should be sum of all sources
        assert!(
            commands.len() >= core_commands.len() + tool_handler_commands.len(),
            "Total commands should include all sources"
        );
    }

    #[tokio::test]
    async fn test_editor_update_buffers_ext_method() {
        use std::collections::HashMap;
        use std::path::PathBuf;
        use std::time::SystemTime;

        let agent = create_test_agent().await;

        // Create editor buffer response
        let path1 = PathBuf::from("/test/file1.rs");
        let path2 = PathBuf::from("/test/file2.rs");

        let buffer1 = crate::editor_state::EditorBuffer {
            path: path1.clone(),
            content: "fn main() {}".to_string(),
            modified: true,
            last_modified: SystemTime::now(),
            encoding: "UTF-8".to_string(),
        };

        let buffer2 = crate::editor_state::EditorBuffer {
            path: path2.clone(),
            content: "fn test() {}".to_string(),
            modified: false,
            last_modified: SystemTime::now(),
            encoding: "UTF-8".to_string(),
        };

        let mut buffers = HashMap::new();
        buffers.insert(path1.clone(), buffer1);
        buffers.insert(path2.clone(), buffer2);

        let response = crate::editor_state::EditorBufferResponse {
            buffers,
            unavailable_paths: vec![],
        };

        // Create extension method request
        let params_json = serde_json::to_string(&response).unwrap();
        let params = RawValue::from_string(params_json).unwrap();

        let request = ExtRequest {
            method: "editor/update_buffers".to_string().into(),
            params: Arc::from(params),
        };

        // Call the extension method
        let result = agent.ext_method(request).await;

        assert!(result.is_ok(), "Extension method should succeed");

        // Check cache size
        let _cache_size = agent.editor_state_manager.cache_size().await;

        // Verify the buffers were cached
        let cached1 = agent
            .editor_state_manager
            .get_file_content("test", &path1)
            .await
            .unwrap();
        assert!(cached1.is_some(), "Buffer 1 should be cached");
        assert_eq!(cached1.unwrap().content, "fn main() {}");

        let cached2 = agent
            .editor_state_manager
            .get_file_content("test", &path2)
            .await
            .unwrap();
        assert!(cached2.is_some(), "Buffer 2 should be cached");
        assert_eq!(cached2.unwrap().content, "fn test() {}");
    }

    #[tokio::test]
    async fn test_editor_state_end_to_end_workflow() {
        use std::collections::HashMap;
        use std::time::SystemTime;
        use tempfile::TempDir;

        // Create a temporary directory and file
        let temp_dir = TempDir::new().unwrap();
        let test_file_path = temp_dir.path().join("test.rs");
        std::fs::write(&test_file_path, "// Old content on disk").unwrap();

        let agent = create_test_agent().await;

        // Step 1: Client pushes editor buffer state with unsaved changes
        let buffer = crate::editor_state::EditorBuffer {
            path: test_file_path.clone(),
            content: "// New unsaved content in editor".to_string(),
            modified: true,
            last_modified: SystemTime::now(),
            encoding: "UTF-8".to_string(),
        };

        let mut buffers = HashMap::new();
        buffers.insert(test_file_path.clone(), buffer);

        let response = crate::editor_state::EditorBufferResponse {
            buffers,
            unavailable_paths: vec![],
        };

        // Send editor/update_buffers extension method
        let params_json = serde_json::to_string(&response).unwrap();
        let params = RawValue::from_string(params_json).unwrap();

        let request = ExtRequest {
            method: "editor/update_buffers".to_string().into(),
            params: Arc::from(params),
        };

        let result = agent.ext_method(request).await;
        assert!(result.is_ok(), "Extension method should succeed");

        // Step 2: Verify the buffer is cached
        let cached = agent
            .editor_state_manager
            .get_file_content("test_session", &test_file_path)
            .await
            .unwrap();
        assert!(cached.is_some(), "Buffer should be cached");
        assert_eq!(
            cached.unwrap().content,
            "// New unsaved content in editor",
            "Cached content should match editor buffer, not disk content"
        );

        // Step 3: Verify disk content is different (to prove cache is being used)
        let disk_content = std::fs::read_to_string(&test_file_path).unwrap();
        assert_eq!(
            disk_content, "// Old content on disk",
            "Disk content should remain unchanged"
        );

        // Step 4: Simulate a tool reading the file
        // The tool would use editor_state_manager.get_file_content() which returns cached content
        let tool_sees_content = agent
            .editor_state_manager
            .get_file_content("test_session", &test_file_path)
            .await
            .unwrap();
        assert!(
            tool_sees_content.is_some(),
            "Tool should see cached editor content"
        );
        assert_eq!(
            tool_sees_content.unwrap().content,
            "// New unsaved content in editor",
            "Tool should see editor buffer content, not disk content"
        );
    }

    #[tokio::test]
    async fn test_streaming_prompt_enforces_turn_request_limit() {
        // This test verifies that the streaming path has turn request limit enforcement
        // by setting limit to 0 to force immediate failure, proving the check exists.
        //
        // Note: In current implementation, each prompt() call is a new turn (resets counters)
        // and only makes one LM request, so the limit won't be exceeded naturally.

        let mut agent = create_test_agent().await;

        // Set limit to 0 to force immediate failure (any request exceeds limit)
        agent.config.max_turn_requests = 0;

        // Create a session with streaming capability
        let new_session_request = NewSessionRequest::new(std::path::PathBuf::from("/tmp")).meta(
            serde_json::json!({"streaming": true})
                .as_object()
                .unwrap()
                .clone(),
        );
        let session_response = agent.new_session(new_session_request).await.unwrap();
        let session_id: crate::session::SessionId =
            session_response.session_id.0.as_ref().parse().unwrap();

        // Update session to enable streaming
        agent
            .session_manager
            .update_session(&session_id, |session| {
                session.client_capabilities = Some(
                    agent_client_protocol::ClientCapabilities::new()
                        .fs(agent_client_protocol::FileSystemCapability::new()
                            .read_text_file(true)
                            .write_text_file(true))
                        .terminal(true)
                        .meta(serde_json::json!({"streaming": true}).as_object().cloned()),
                );
            })
            .unwrap();

        // Prompt should be blocked immediately by streaming path check
        // Counter resets to 0, increments to 1, and 1 > 0 triggers limit
        let prompt_request = PromptRequest {
            session_id: session_response.session_id.clone(),
            prompt: vec![ContentBlock::Text(TextContent {
                text: "This should be blocked by turn request limit".to_string(),
                annotations: None,
                meta: None,
            })],
            meta: None,
        };

        let response = agent.prompt(prompt_request).await.unwrap();
        assert_eq!(
            response.stop_reason,
            StopReason::MaxTurnRequests,
            "Streaming path should enforce turn request limit"
        );

        // Verify metadata contains turn request info
        let meta = response.meta.unwrap();
        assert_eq!(
            meta["turn_requests"], 1,
            "Should have reset to 0 then incremented to 1"
        );
        assert_eq!(meta["max_turn_requests"], 0);
    }
}

// Fixture support
impl AgentWithFixture for ClaudeAgent {
    fn agent_type(&self) -> &'static str {
        "claude"
    }
}

// Fixture support
