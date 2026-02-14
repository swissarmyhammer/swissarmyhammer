//! Agent Client Protocol implementation for Claude Agent

// Re-export types from extracted modules for backward compatibility
pub use crate::agent_cancellation::{CancellationManager, CancellationState};
pub use crate::agent_file_operations::{
    ReadTextFileParams, ReadTextFileResponse, WriteTextFileParams,
};
pub use crate::agent_notifications::NotificationSender;
pub use crate::agent_permissions::{PermissionRequest, PermissionResponse, ToolCallUpdate};
pub use crate::agent_raw_messages::RawMessageManager;
pub use crate::agent_reasoning::{AgentThought, ReasoningPhase};

use crate::permission_storage;
use crate::{
    base64_processor::Base64Processor,
    claude::ClaudeClient,
    config::AgentConfig,
    constants::sizes,
    content_block_processor::ContentBlockProcessor,
    path_validator::PathValidator,
    permissions::{FilePermissionStorage, PermissionPolicyEngine, PolicyEvaluation},
    protocol_translator::ProtocolTranslator,
    session::SessionManager,
    size_validator::{SizeLimits, SizeValidator},
    tools::ToolCallHandler,
};
use agent_client_protocol::{
    AgentCapabilities, ContentBlock, LoadSessionRequest, LoadSessionResponse, NewSessionRequest,
    NewSessionResponse, PromptRequest, PromptResponse, SessionId, SessionNotification,
    SessionUpdate, SetSessionModeRequest, SetSessionModeResponse, StopReason, TextContent,
};
use agent_client_protocol_extras::AgentWithFixture;
use std::sync::Arc;
use std::time::SystemTime;
use swissarmyhammer_common::Pretty;
use tokio::sync::{broadcast, RwLock};

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
    pub(crate) session_manager: Arc<SessionManager>,
    pub(crate) claude_client: Arc<ClaudeClient>,
    pub(crate) tool_handler: Arc<RwLock<ToolCallHandler>>,
    pub(crate) mcp_manager: Option<Arc<crate::mcp::McpServerManager>>,
    pub(crate) config: AgentConfig,
    pub(crate) capabilities: AgentCapabilities,
    pub(crate) client_capabilities: Arc<RwLock<Option<agent_client_protocol::ClientCapabilities>>>,
    pub(crate) notification_sender: Arc<NotificationSender>,
    pub(crate) cancellation_manager: Arc<CancellationManager>,
    pub(crate) permission_engine: Arc<PermissionPolicyEngine>,
    pub(crate) base64_processor: Arc<Base64Processor>,
    pub(crate) content_block_processor: Arc<ContentBlockProcessor>,
    pub(crate) editor_state_manager: Arc<crate::editor_state::EditorStateManager>,
    pub(crate) raw_message_manager: Option<RawMessageManager>,
    /// Client connection for sending requests back to the client (e.g., request_permission)
    ///
    /// Per ACP protocol, Agent can send requests TO the Client. This is the AgentSideConnection
    /// that implements the Client trait and sends JSON-RPC messages.
    pub(crate) client: Option<Arc<dyn agent_client_protocol::Client + Send + Sync>>,
    /// Storage for user permission preferences
    ///
    /// Stores "always" decisions (allow-always, reject-always) across tool calls
    /// to avoid re-prompting the user for the same tool. Preferences are stored
    /// in-memory and do not persist across agent restarts.
    pub(crate) permission_storage: Arc<permission_storage::PermissionStorage>,
    /// Manager for tracking plan state across sessions
    ///
    /// Tracks plan entries and their status changes as work progresses,
    /// enabling ACP-compliant progress reporting to clients.
    pub(crate) plan_manager: Arc<RwLock<crate::plan::PlanManager>>,
    /// Available agents (modes) from Claude CLI init message
    ///
    /// Stores agent id, name, and description tuples parsed from the Claude CLI
    /// init JSON. Used to provide ACP session modes functionality.
    #[allow(clippy::type_complexity)]
    pub(crate) available_agents: Arc<RwLock<Option<Vec<(String, String, Option<String>)>>>>,
    /// SwissArmyHammer modes with their system prompts
    ///
    /// Maps mode ID to system prompt content. When a SAH mode is set,
    /// the system prompt is passed to Claude via --system-prompt.
    sah_modes: Arc<RwLock<std::collections::HashMap<String, String>>>,
    /// Path validator for secure file operations
    ///
    /// Validates file paths to prevent path traversal attacks, enforce allowed/blocked
    /// path lists, and ensure paths meet security requirements per ACP file-security rule.
    pub(crate) path_validator: Arc<PathValidator>,
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
        config.validate()?;

        let session_manager = Arc::new(SessionManager::new());
        let (notification_sender, notification_receiver) =
            NotificationSender::new(config.notification_buffer_size);

        let permission_engine = Self::create_permission_engine();
        let protocol_translator = Arc::new(ProtocolTranslator::new(permission_engine.clone()));

        let (claude_client, raw_message_manager) = Self::create_claude_client(
            &config,
            protocol_translator,
            &notification_sender,
            raw_message_manager,
        )?;

        let mcp_manager = Arc::new(crate::mcp::McpServerManager::new());
        let tool_handler = Self::create_tool_handler(
            &config,
            &mcp_manager,
            &session_manager,
            &permission_engine,
            &notification_sender,
        )
        .await;

        let capabilities = Self::build_agent_capabilities(&tool_handler).await;
        let (cancellation_manager, _) = CancellationManager::new(config.cancellation_buffer_size);
        let path_validator = Self::create_path_validator(&config);

        let agent = Self::build_agent_instance(
            session_manager,
            claude_client,
            tool_handler,
            mcp_manager,
            config,
            capabilities,
            notification_sender,
            cancellation_manager,
            permission_engine,
            raw_message_manager,
            path_validator,
        );

        Ok((agent, notification_receiver))
    }

    /// Create the permission policy engine with file-based storage.
    fn create_permission_engine() -> Arc<PermissionPolicyEngine> {
        let storage_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(".claude-agent")
            .join("permissions");
        let storage = FilePermissionStorage::new(storage_path);
        Arc::new(PermissionPolicyEngine::new(Box::new(storage)))
    }

    /// Create and configure the Claude client with optional raw message manager.
    fn create_claude_client(
        config: &AgentConfig,
        protocol_translator: Arc<ProtocolTranslator>,
        notification_sender: &NotificationSender,
        raw_message_manager: Option<RawMessageManager>,
    ) -> crate::Result<(Arc<ClaudeClient>, Option<RawMessageManager>)> {
        let mut claude_client = ClaudeClient::new_with_config(&config.claude, protocol_translator)?;
        claude_client.set_notification_sender(Arc::new(notification_sender.clone()));

        let raw_message_manager = Self::resolve_raw_message_manager(raw_message_manager);
        if let Some(ref manager) = raw_message_manager {
            claude_client.set_raw_message_manager(manager.clone());
        }

        Ok((Arc::new(claude_client), raw_message_manager))
    }

    /// Resolve the raw message manager - use provided or create new.
    fn resolve_raw_message_manager(
        raw_message_manager: Option<RawMessageManager>,
    ) -> Option<RawMessageManager> {
        if let Some(manager) = raw_message_manager {
            tracing::debug!("Using shared RawMessageManager from parent agent");
            return Some(manager);
        }

        Self::create_new_raw_message_manager()
    }

    /// Create a new raw message manager for recording JSON-RPC messages.
    fn create_new_raw_message_manager() -> Option<RawMessageManager> {
        let raw_json_path = Self::get_raw_message_path();
        Self::ensure_parent_dir_exists(&raw_json_path);

        RawMessageManager::new(raw_json_path.clone())
            .inspect(|_| {
                tracing::info!(
                    "Raw ACP JSON-RPC messages recording to {}",
                    raw_json_path.display()
                );
            })
            .inspect_err(|e| {
                tracing::warn!("Failed to create raw message manager: {}", e);
            })
            .ok()
    }

    /// Get the path for raw message recording.
    fn get_raw_message_path() -> std::path::PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(".acp")
            .join("transcript_raw.jsonl")
    }

    /// Ensure the parent directory exists.
    fn ensure_parent_dir_exists(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    /// Create the tool handler with MCP support.
    async fn create_tool_handler(
        config: &AgentConfig,
        mcp_manager: &Arc<crate::mcp::McpServerManager>,
        session_manager: &Arc<SessionManager>,
        permission_engine: &Arc<PermissionPolicyEngine>,
        notification_sender: &NotificationSender,
    ) -> Arc<RwLock<ToolCallHandler>> {
        let tool_handler = Arc::new(RwLock::new(ToolCallHandler::new_with_mcp_manager(
            config.security.to_tool_permissions(),
            Arc::clone(mcp_manager),
            Arc::clone(session_manager),
            Arc::clone(permission_engine),
        )));

        {
            let mut handler = tool_handler.write().await;
            handler.set_notification_sender(notification_sender.clone());
        }

        tool_handler
    }

    /// Build agent capabilities based on available tools.
    async fn build_agent_capabilities(
        tool_handler: &Arc<RwLock<ToolCallHandler>>,
    ) -> AgentCapabilities {
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

        let mcp_capabilities = agent_client_protocol::McpCapabilities::new()
            .http(true)
            .sse(false);

        let mut agent_meta_map = serde_json::Map::new();
        agent_meta_map.insert("tools".to_string(), serde_json::json!(available_tools));
        agent_meta_map.insert("streaming".to_string(), serde_json::json!(true));

        AgentCapabilities::new()
            .load_session(true)
            .prompt_capabilities(prompt_capabilities)
            .mcp_capabilities(mcp_capabilities)
            .meta(agent_meta_map)
    }

    /// Create path validator from configuration.
    fn create_path_validator(config: &AgentConfig) -> Arc<PathValidator> {
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
    }

    /// Build the final agent instance with all components.
    #[allow(clippy::too_many_arguments)]
    fn build_agent_instance(
        session_manager: Arc<SessionManager>,
        claude_client: Arc<ClaudeClient>,
        tool_handler: Arc<RwLock<ToolCallHandler>>,
        mcp_manager: Arc<crate::mcp::McpServerManager>,
        config: AgentConfig,
        capabilities: AgentCapabilities,
        notification_sender: NotificationSender,
        cancellation_manager: CancellationManager,
        permission_engine: Arc<PermissionPolicyEngine>,
        raw_message_manager: Option<RawMessageManager>,
        path_validator: Arc<PathValidator>,
    ) -> Self {
        let base64_processor = Arc::new(Base64Processor::default());
        let content_block_processor = Arc::new(ContentBlockProcessor::new(
            (*base64_processor).clone(),
            sizes::content::MAX_RESOURCE_MODERATE,
            true,
        ));
        let editor_state_manager = Arc::new(crate::editor_state::EditorStateManager::new());
        let size_validator = Arc::new(SizeValidator::new(SizeLimits::default()));

        Self {
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
            client: None,
            permission_storage: Arc::new(permission_storage::PermissionStorage::new()),
            plan_manager: Arc::new(RwLock::new(crate::plan::PlanManager::new())),
            available_agents: Arc::new(RwLock::new(None)),
            sah_modes: Arc::new(RwLock::new(std::collections::HashMap::new())),
            path_validator,
            size_validator,
        }
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
    /// Also loads SwissArmyHammer modes from ModeRegistry and merges them.
    /// These are used to provide ACP session modes functionality.
    pub async fn set_available_agents(&self, mut agents: Vec<(String, String, Option<String>)>) {
        let sah_mode_agents = self.load_sah_modes().await;
        agents.extend(sah_mode_agents);

        let mut available_agents = self.available_agents.write().await;
        *available_agents = Some(agents);
    }

    /// Load SwissArmyHammer modes from ModeRegistry.
    ///
    /// Returns a list of agent tuples (id, name, description) for each loaded mode.
    /// Also stores the resolved system prompts in self.sah_modes.
    async fn load_sah_modes(&self) -> Vec<(String, String, Option<String>)> {
        let mut registry = swissarmyhammer_modes::ModeRegistry::new();
        let sah_mode_list = match registry.load_all() {
            Ok(modes) => modes,
            Err(e) => {
                tracing::warn!("Failed to load SwissArmyHammer modes from registry: {}", e);
                return Vec::new();
            }
        };

        let prompt_library = swissarmyhammer_prompts::PromptLibrary::new();
        let template_context = swissarmyhammer_config::TemplateContext::new();
        let mut agents = Vec::new();
        let mut sah_modes = self.sah_modes.write().await;

        for mode in sah_mode_list {
            let system_prompt =
                Self::resolve_mode_system_prompt(&mode, &prompt_library, &template_context);
            sah_modes.insert(mode.id().to_string(), system_prompt);
            agents.push((
                mode.id().to_string(),
                mode.name().to_string(),
                Some(mode.description().to_string()),
            ));
        }

        tracing::info!(
            "Loaded {} SwissArmyHammer modes from ModeRegistry",
            sah_modes.len()
        );
        agents
    }

    /// Resolve the system prompt for a mode.
    ///
    /// If the mode references a prompt file, render it. Otherwise use embedded content.
    fn resolve_mode_system_prompt(
        mode: &swissarmyhammer_modes::Mode,
        prompt_library: &swissarmyhammer_prompts::PromptLibrary,
        template_context: &swissarmyhammer_config::TemplateContext,
    ) -> String {
        let Some(prompt_path) = mode.prompt() else {
            return mode.system_prompt().to_string();
        };

        match prompt_library.render(prompt_path, template_context) {
            Ok(rendered) => {
                tracing::debug!(
                    "Rendered prompt '{}' for mode '{}' ({} chars)",
                    prompt_path,
                    mode.id(),
                    rendered.len()
                );
                rendered
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to render prompt '{}' for mode '{}': {}",
                    prompt_path,
                    mode.id(),
                    e
                );
                mode.system_prompt().to_string()
            }
        }
    }

    /// Get the system prompt for a SwissArmyHammer mode
    ///
    /// Returns None if the mode is not a SwissArmyHammer mode (i.e., it's a Claude CLI mode).
    pub async fn get_sah_mode_system_prompt(&self, mode_id: &str) -> Option<String> {
        let sah_modes = self.sah_modes.read().await;
        sah_modes.get(mode_id).cloned()
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
    pub(crate) fn log_request<T: std::fmt::Debug + serde::Serialize>(
        &self,
        method: &str,
        request: &T,
    ) {
        tracing::debug!("Handling {} request: {}", method, Pretty(request));
    }

    /// Log outgoing response for debugging purposes
    pub(crate) fn log_response<T: std::fmt::Debug + serde::Serialize>(
        &self,
        method: &str,
        response: &T,
    ) {
        tracing::debug!("Returning {} response: {}", method, Pretty(response));
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

    /// Parse and validate a session ID from a SessionId wrapper
    pub(crate) fn parse_session_id(
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
    pub(crate) async fn validate_prompt_request(
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
    // Prompt handling methods (should_stream, handle_streaming_prompt, handle_non_streaming_prompt)
    // are implemented in agent_prompt_handling.rs

    /// Send session update notification
    pub(crate) async fn send_session_update(
        &self,
        notification: SessionNotification,
    ) -> crate::Result<()> {
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
    pub(crate) fn is_response_refusal(&self, response_content: &str) -> bool {
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
    pub(crate) fn create_refusal_response(
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

    // Available commands handling (send_available_commands_update, update_session_available_commands,
    // refresh_commands_for_all_sessions, get_available_commands_for_session)
    // are implemented in agent_commands.rs

    /// Cancel ongoing Claude API requests for a session
    ///
    /// Note: This is a minimal implementation that registers cancellation state.
    /// Individual request cancellation is not yet implemented as the ClaudeClient
    /// doesn't currently track requests by session. The cancellation state is
    /// checked before making new requests to prevent further API calls.
    pub(crate) async fn cancel_claude_requests(&self, session_id: &str) {
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
    pub(crate) async fn cancel_tool_executions(&self, session_id: &str) {
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
    pub(crate) async fn cancel_permission_requests(&self, session_id: &str) {
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
    pub(crate) async fn send_final_cancellation_updates(
        &self,
        session_id: &str,
    ) -> crate::Result<()> {
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

    // =========================================================================
    // new_session helper methods
    // =========================================================================

    /// Validate MCP transport requirements for new session.
    pub(crate) fn validate_new_session_mcp_config(
        &self,
        request: &NewSessionRequest,
    ) -> Result<(), agent_client_protocol::Error> {
        let internal_mcp_servers: Vec<crate::config::McpServerConfig> = request
            .mcp_servers
            .iter()
            .filter_map(|server| self.convert_acp_to_internal_mcp_config(server))
            .collect();

        if let Err(validation_error) =
            crate::capability_validation::CapabilityRequirementChecker::check_new_session_requirements(
                &self.capabilities,
                &internal_mcp_servers,
            )
        {
            tracing::error!(
                "Session creation failed: Transport validation error - {}",
                validation_error
            );
            return Err(self.convert_session_setup_error_to_acp_error(validation_error));
        }
        Ok(())
    }

    /// Create session and register RawMessageManager.
    pub(crate) async fn create_new_session_internal(
        &self,
        request: &NewSessionRequest,
    ) -> Result<crate::session::SessionId, agent_client_protocol::Error> {
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
            self.store_mcp_servers_in_session(&session_id, &request.mcp_servers)?;
        }

        // Register per-session notification channel
        self.notification_sender
            .register_session(&session_id.to_string());

        tracing::info!("Created session: {}", session_id);
        Ok(session_id)
    }

    /// Store MCP server configs in session.
    pub(crate) fn store_mcp_servers_in_session(
        &self,
        session_id: &crate::session::SessionId,
        mcp_servers: &[agent_client_protocol::McpServer],
    ) -> Result<(), agent_client_protocol::Error> {
        self.session_manager
            .update_session(session_id, |session| {
                session.mcp_servers = mcp_servers
                    .iter()
                    .map(|server| {
                        serde_json::to_string(server).unwrap_or_else(|_| format!("{:?}", server))
                    })
                    .collect();
            })
            .map_err(|_e| agent_client_protocol::Error::internal_error())
    }

    /// Spawn Claude process and handle init response for new session.
    pub(crate) async fn spawn_claude_for_new_session(
        &self,
        session_id: &crate::session::SessionId,
        protocol_session_id: &SessionId,
        request: &NewSessionRequest,
    ) {
        use crate::claude_process::SpawnConfig;

        // Extract system_prompt from session metadata if present
        let system_prompt = request
            .meta
            .as_ref()
            .and_then(|meta| meta.get("system_prompt"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        tracing::info!("Spawning Claude process for session: {}", session_id);
        let spawn_config = SpawnConfig::builder()
            .session_id(*session_id)
            .acp_session_id(protocol_session_id.clone())
            .cwd(request.cwd.clone())
            .mcp_servers(self.config.mcp_servers.clone())
            .system_prompt(system_prompt)
            .ephemeral(self.config.claude.ephemeral)
            .build();

        match self
            .claude_client
            .spawn_process_and_consume_init(spawn_config)
            .await
        {
            Ok((Some(agents), current_agent)) => {
                self.handle_claude_init_with_agents(session_id, agents, current_agent)
                    .await;
            }
            Ok((None, _)) => {
                tracing::debug!("No available agents in Claude CLI init message");
                self.set_available_agents(vec![]).await;
            }
            Err(e) => {
                tracing::error!("Failed to spawn Claude process and read init: {}", e);
                self.set_available_agents(vec![]).await;
            }
        }
    }

    /// Handle Claude init response that includes available agents.
    pub(crate) async fn handle_claude_init_with_agents(
        &self,
        session_id: &crate::session::SessionId,
        agents: Vec<(String, String, Option<String>)>,
        current_agent: Option<String>,
    ) {
        tracing::info!(
            "Storing {} available agents from Claude CLI init",
            agents.len()
        );
        self.set_available_agents(agents).await;

        if let Some(mode) = current_agent {
            tracing::info!("Setting initial mode from Claude CLI: {}", mode);
            self.session_manager
                .update_session(session_id, |session| {
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

    /// Send initial available commands after session creation.
    pub(crate) async fn send_initial_session_commands(
        &self,
        session_id: &crate::session::SessionId,
        protocol_session_id: &SessionId,
    ) {
        let initial_commands = self
            .get_available_commands_for_session(protocol_session_id)
            .await;
        if let Err(e) = self
            .update_session_available_commands(protocol_session_id, initial_commands)
            .await
        {
            tracing::warn!(
                "Failed to send initial available commands for session {}: {}",
                session_id,
                e
            );
        }
    }

    /// Build new session response with modes if applicable.
    pub(crate) async fn build_new_session_response(
        &self,
        session_id: &crate::session::SessionId,
        protocol_session_id: &SessionId,
    ) -> NewSessionResponse {
        let mut response = NewSessionResponse::new(protocol_session_id.clone());

        if let Some(available_modes) = self.get_available_modes().await {
            if let Some(current_mode_id) = self.get_session_mode(session_id).await {
                let mode_state = agent_client_protocol::SessionModeState::new(
                    agent_client_protocol::SessionModeId::new(current_mode_id.as_str()),
                    available_modes,
                );
                response = response.modes(mode_state);
                tracing::info!("Session created with mode: {}", current_mode_id);
            } else {
                tracing::debug!(
                    "Session created without mode (available modes: {}, will not use --agent flag)",
                    available_modes.len()
                );
            }
        }

        response
    }

    // =========================================================================
    // load_session helper methods
    // =========================================================================

    /// Validate MCP transport requirements for load session.
    pub(crate) fn validate_load_session_mcp_config(
        &self,
        request: &LoadSessionRequest,
    ) -> Result<(), agent_client_protocol::Error> {
        let internal_mcp_servers: Vec<crate::config::McpServerConfig> = request
            .mcp_servers
            .iter()
            .filter_map(|server| self.convert_acp_to_internal_mcp_config(server))
            .collect();

        if let Err(validation_error) =
            crate::capability_validation::CapabilityRequirementChecker::check_load_session_requirements(
                &self.capabilities,
                &internal_mcp_servers,
            )
        {
            tracing::error!(
                "Session loading failed: Transport/capability validation error - {}",
                validation_error
            );
            return Err(self.convert_session_setup_error_to_acp_error(validation_error));
        }
        Ok(())
    }

    /// Handle found session: replay history and build response.
    pub(crate) async fn handle_session_found(
        &self,
        session: &crate::session::Session,
    ) -> LoadSessionResponse {
        tracing::info!(
            "Loaded session: {} with {} historical messages",
            session.id,
            session.context.len()
        );

        // Replay historical messages
        self.replay_session_history(session).await;

        // Build response metadata
        self.build_load_session_response(session)
    }

    /// Replay session history via notifications.
    pub(crate) async fn replay_session_history(&self, session: &crate::session::Session) {
        if session.context.is_empty() {
            return;
        }

        tracing::info!(
            "Replaying {} historical messages for session {}",
            session.context.len(),
            session.id
        );

        for message in &session.context {
            let notification = self.build_history_notification(session, message);
            if let Err(e) = self.notification_sender.send_update(notification).await {
                tracing::error!("Failed to send historical message notification: {}", e);
            }
        }

        tracing::info!(
            "Completed queueing {} history notifications for session {}",
            session.context.len(),
            session.id
        );
    }

    /// Build notification for historical message replay.
    pub(crate) fn build_history_notification(
        &self,
        session: &crate::session::Session,
        message: &crate::session::Message,
    ) -> SessionNotification {
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

        SessionNotification::new(
            SessionId::new(session.id.to_string()),
            message.update.clone(),
        )
        .meta(meta_map)
    }

    /// Build load session response with metadata.
    pub(crate) fn build_load_session_response(
        &self,
        session: &crate::session::Session,
    ) -> LoadSessionResponse {
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

        LoadSessionResponse::new().meta(meta_map)
    }

    /// Create session not found error.
    pub(crate) fn session_not_found_error(
        &self,
        session_id: &SessionId,
    ) -> agent_client_protocol::Error {
        tracing::warn!("Session not found: {}", session_id);
        agent_client_protocol::Error::new(
            -32602,
            "Session not found: sessionId does not exist or has expired".to_string(),
        )
        .data(serde_json::json!({
            "sessionId": session_id,
            "error": "session_not_found"
        }))
    }

    // =========================================================================
    // set_session_mode helper methods
    // =========================================================================

    /// Parse session ID for mode change request.
    pub(crate) fn parse_mode_session_id(
        &self,
        session_id: &SessionId,
    ) -> Result<crate::session::SessionId, agent_client_protocol::Error> {
        crate::session::SessionId::parse(&session_id.0)
            .map_err(|_| agent_client_protocol::Error::invalid_request())
    }

    /// Validate that the requested mode exists in available modes.
    pub(crate) async fn validate_mode_exists(
        &self,
        mode_id: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        let available_agents = self.available_agents.read().await;
        match available_agents.as_ref() {
            Some(agents) => {
                let mode_exists = agents.iter().any(|(id, _, _)| id == mode_id);
                if !mode_exists {
                    tracing::error!(
                        "Invalid mode '{}' requested. Available modes: {:?}",
                        mode_id,
                        agents
                            .iter()
                            .map(|(id, name, _)| format!("{}:{}", id, name))
                            .collect::<Vec<_>>()
                    );
                    return Err(agent_client_protocol::Error::invalid_params());
                }
                Ok(())
            }
            None => {
                tracing::warn!("set_session_mode called but no available modes configured");
                Err(agent_client_protocol::Error::invalid_params())
            }
        }
    }

    /// Check if mode changed and update session with new mode.
    pub(crate) async fn check_and_update_session_mode(
        &self,
        session_id: &crate::session::SessionId,
        mode_id: &str,
    ) -> Result<bool, agent_client_protocol::Error> {
        let current_mode = self
            .session_manager
            .get_session(session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .map(|session| session.current_mode.clone())
            .unwrap_or(None);

        let mode_changed = current_mode != Some(mode_id.to_string());

        self.session_manager
            .update_session(session_id, |session| {
                session.current_mode = Some(mode_id.to_string());
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        tracing::info!("Session mode set to: {}", mode_id);
        Ok(mode_changed)
    }

    /// Handle process replacement when mode changes.
    pub(crate) async fn handle_mode_change_process(
        &self,
        session_id: &crate::session::SessionId,
        mode_id: &str,
        request: &SetSessionModeRequest,
    ) -> Result<(), agent_client_protocol::Error> {
        let session = self
            .session_manager
            .get_session(session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .ok_or_else(agent_client_protocol::Error::internal_error)?;
        let cwd = session.cwd.clone();

        // Terminate existing process
        self.terminate_existing_process(session_id, mode_id).await;

        // Spawn new process with mode
        let protocol_session_id = SessionId::new(session_id.to_string());
        let spawn_config = self
            .build_mode_spawn_config(session_id, &protocol_session_id, &cwd, mode_id)
            .await;

        if let Err(e) = self
            .claude_client
            .spawn_process_and_consume_init(spawn_config)
            .await
        {
            tracing::error!("Failed to spawn new Claude process for mode change: {}", e);
            return Err(agent_client_protocol::Error::internal_error());
        }

        // Send mode update notification
        self.send_mode_update_notification(session_id, request)
            .await?;
        Ok(())
    }

    /// Terminate existing Claude process for session.
    pub(crate) async fn terminate_existing_process(
        &self,
        session_id: &crate::session::SessionId,
        mode_id: &str,
    ) {
        tracing::info!(
            "Mode changed for session {}, terminating process and spawning new one with mode '{}'",
            session_id,
            mode_id
        );

        if let Err(e) = self.claude_client.terminate_session(session_id).await {
            tracing::warn!(
                "Failed to terminate Claude process for session {}: {}",
                session_id,
                e
            );
        }
    }

    /// Build spawn config for mode change.
    pub(crate) async fn build_mode_spawn_config(
        &self,
        session_id: &crate::session::SessionId,
        protocol_session_id: &SessionId,
        cwd: &std::path::Path,
        mode_id: &str,
    ) -> crate::claude_process::SpawnConfig {
        use crate::claude_process::SpawnConfig;

        let (agent_mode, system_prompt) =
            if let Some(prompt) = self.get_sah_mode_system_prompt(mode_id).await {
                tracing::info!(
                    "Spawning new Claude process with --system-prompt ({} chars) for SAH mode '{}'",
                    prompt.len(),
                    mode_id
                );
                (None, Some(prompt))
            } else {
                tracing::info!(
                    "Spawning new Claude process with --agent '{}' for Claude CLI mode",
                    mode_id
                );
                (Some(mode_id.to_string()), None)
            };

        SpawnConfig::builder()
            .session_id(*session_id)
            .acp_session_id(protocol_session_id.clone())
            .cwd(cwd.to_path_buf())
            .mcp_servers(self.config.mcp_servers.clone())
            .agent_mode(agent_mode)
            .system_prompt(system_prompt)
            .ephemeral(self.config.claude.ephemeral)
            .build()
    }

    /// Send mode update notification and store in session context.
    pub(crate) async fn send_mode_update_notification(
        &self,
        session_id: &crate::session::SessionId,
        request: &SetSessionModeRequest,
    ) -> Result<(), agent_client_protocol::Error> {
        let current_mode_update =
            agent_client_protocol::CurrentModeUpdate::new(request.mode_id.clone());
        let update = SessionUpdate::CurrentModeUpdate(current_mode_update);

        let mode_message = crate::session::Message::from_update(update.clone());
        self.session_manager
            .update_session(session_id, |session| {
                session.add_message(mode_message);
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        if let Err(e) = self
            .send_session_update(SessionNotification::new(request.session_id.clone(), update))
            .await
        {
            tracing::warn!("Failed to send current mode update notification: {}", e);
        }
        Ok(())
    }

    /// Build response for set_session_mode.
    pub(crate) fn build_set_mode_response(&self, mode_changed: bool) -> SetSessionModeResponse {
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
                serde_json::json!("process_replaced"),
            );
        }

        SetSessionModeResponse::new().meta(meta_map)
    }

    // =========================================================================
    // prompt helper methods
    // =========================================================================

    /// Log debug info about prompt request.
    pub(crate) fn log_prompt_debug(&self, request: &PromptRequest) {
        use swissarmyhammer_common::Pretty;

        tracing::debug!("📨 PROMPT REQUEST DEBUG:");
        tracing::debug!("  Session: {}", request.session_id);
        tracing::debug!("  Content blocks: {}", request.prompt.len());
        for (i, block) in request.prompt.iter().enumerate() {
            match block {
                agent_client_protocol::ContentBlock::Text(text) => {
                    tracing::debug!("  Block {}: TEXT ({} chars)", i + 1, text.text.len());
                    tracing::debug!("  Text content: {}", text.text);
                }
                _ => {
                    tracing::debug!("  Block {}: {}", i + 1, Pretty(block));
                }
            }
        }
    }

    /// Send user message chunks for conversation transparency.
    pub(crate) async fn send_user_message_chunks(&self, request: &PromptRequest) {
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
    }

    /// Check if session was cancelled before processing.
    pub(crate) async fn check_cancelled_before_processing(
        &self,
        session_id: &crate::session::SessionId,
    ) -> Option<PromptResponse> {
        if !self
            .cancellation_manager
            .is_cancelled(&session_id.to_string())
            .await
        {
            return None;
        }

        tracing::info!(
            "Session {} is cancelled, returning cancelled response",
            session_id
        );

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
        Some(PromptResponse::new(StopReason::Cancelled).meta(meta_map))
    }

    /// Extract text content from prompt request.
    pub(crate) fn extract_prompt_text(&self, request: &PromptRequest) -> String {
        let mut prompt_text = String::new();
        let mut has_binary_content = false;

        for content_block in &request.prompt {
            match content_block {
                ContentBlock::Text(text_content) => {
                    prompt_text.push_str(&text_content.text);
                }
                ContentBlock::Image(image_content) => {
                    prompt_text.push_str(&format!(
                        "\n[Image content: {} ({})]",
                        image_content.mime_type,
                        image_content.uri.as_deref().unwrap_or("embedded data")
                    ));
                    has_binary_content = true;
                }
                ContentBlock::Audio(audio_content) => {
                    prompt_text.push_str(&format!(
                        "\n[Audio content: {} (embedded data)]",
                        audio_content.mime_type
                    ));
                    has_binary_content = true;
                }
                ContentBlock::Resource(_) => {
                    prompt_text.push_str("\n[Embedded Resource]");
                    has_binary_content = true;
                }
                ContentBlock::ResourceLink(resource_link) => {
                    prompt_text.push_str(&format!("\n[Resource Link: {}]", resource_link.uri));
                }
                _ => {
                    tracing::warn!("Unknown content block type, skipping");
                }
            }
        }

        if has_binary_content {
            tracing::info!("Processing prompt with binary content for plan analysis");
        }

        prompt_text
    }

    /// Get and validate session exists.
    pub(crate) fn get_validated_session(
        &self,
        session_id: &crate::session::SessionId,
    ) -> Result<crate::session::Session, agent_client_protocol::Error> {
        self.session_manager
            .get_session(session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .ok_or_else(agent_client_protocol::Error::invalid_params)
    }

    /// Prepare session for new turn: reset counters and add user message.
    pub(crate) fn prepare_session_for_turn(
        &self,
        session_id: &crate::session::SessionId,
        prompt_text: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        // Reset turn counters
        self.session_manager
            .update_session(session_id, |session| {
                session.reset_turn_counters();
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        // Add user message
        let user_message = crate::session::Message::new(
            crate::session::MessageRole::User,
            prompt_text.to_string(),
        );

        self.session_manager
            .update_session(session_id, |session| {
                session.add_message(user_message);
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())
    }

    /// Check turn limits and return early response if exceeded.
    pub(crate) fn check_turn_limits(
        &self,
        session_id: &crate::session::SessionId,
        prompt_text: &str,
    ) -> Result<Option<PromptResponse>, agent_client_protocol::Error> {
        let mut session = self.get_updated_session(session_id)?;

        // Check turn request limit
        let current_requests = session.increment_turn_requests();
        if current_requests > self.config.max_turn_requests {
            return Ok(Some(
                self.build_max_requests_response(session_id, current_requests),
            ));
        }

        // Check token limit
        let estimated_tokens = (prompt_text.len() as u64) / 4;
        let current_tokens = session.add_turn_tokens(estimated_tokens);
        if current_tokens > self.config.max_tokens_per_turn {
            return Ok(Some(
                self.build_max_tokens_response(session_id, current_tokens),
            ));
        }

        // Update session with counters
        self.session_manager
            .update_session(session_id, |s| {
                s.turn_request_count = session.turn_request_count;
                s.turn_token_count = session.turn_token_count;
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        Ok(None)
    }

    /// Get updated session after modifications.
    pub(crate) fn get_updated_session(
        &self,
        session_id: &crate::session::SessionId,
    ) -> Result<crate::session::Session, agent_client_protocol::Error> {
        self.session_manager
            .get_session(session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .ok_or_else(agent_client_protocol::Error::internal_error)
    }

    /// Build response for max turn requests exceeded.
    pub(crate) fn build_max_requests_response(
        &self,
        session_id: &crate::session::SessionId,
        current_requests: u64,
    ) -> PromptResponse {
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
        PromptResponse::new(StopReason::MaxTurnRequests).meta(meta_map)
    }

    /// Build response for max tokens exceeded.
    pub(crate) fn build_max_tokens_response(
        &self,
        session_id: &crate::session::SessionId,
        current_tokens: u64,
    ) -> PromptResponse {
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
        PromptResponse::new(StopReason::MaxTokens).meta(meta_map)
    }
}

// Agent trait implementation moved to agent_trait_impl.rs

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

        let session_id = self.parse_session_id(&request.session_id)?;

        if self.is_session_cancelled(&session_id).await {
            return Ok(Self::cancelled_response());
        }

        let (tool_name, tool_args) = self
            .extract_tool_info(&request.tool_call.tool_call_id)
            .await;

        let policy_result = match self
            .evaluate_permission_policy(&tool_name, &tool_args)
            .await
        {
            Ok(result) => result,
            Err(_) => return Ok(Self::cancelled_response()),
        };

        let outcome = self
            .resolve_permission_outcome(policy_result, &tool_name, &request)
            .await;

        let response = PermissionResponse { outcome };
        tracing::info!(
            "Permission request completed for session: {} with outcome: {:?}",
            session_id,
            response.outcome
        );
        self.log_response("request_permission", &response);
        Ok(response)
    }

    /// Check if a session has been cancelled.
    async fn is_session_cancelled(&self, session_id: &crate::session::SessionId) -> bool {
        if self
            .cancellation_manager
            .is_cancelled(&session_id.to_string())
            .await
        {
            tracing::info!(
                "Session {} is cancelled, returning cancelled outcome",
                session_id
            );
            true
        } else {
            false
        }
    }

    /// Create a cancelled permission response.
    fn cancelled_response() -> PermissionResponse {
        PermissionResponse {
            outcome: crate::tools::PermissionOutcome::Cancelled,
        }
    }

    /// Extract tool name and arguments from an active tool call.
    async fn extract_tool_info(&self, tool_call_id: &str) -> (String, serde_json::Value) {
        let tool_handler = self.tool_handler.read().await;
        let active_calls = tool_handler.get_active_tool_calls().await;

        match active_calls.get(tool_call_id) {
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
                    tool_call_id
                );
                ("unknown_tool".to_string(), serde_json::json!({}))
            }
        }
    }

    /// Evaluate the permission policy for a tool call.
    async fn evaluate_permission_policy(
        &self,
        tool_name: &str,
        tool_args: &serde_json::Value,
    ) -> Result<PolicyEvaluation, ()> {
        match self
            .permission_engine
            .evaluate_tool_call(tool_name, tool_args)
            .await
        {
            Ok(evaluation) => Ok(evaluation),
            Err(e) => {
                tracing::error!("Permission policy evaluation failed: {}", e);
                Err(())
            }
        }
    }

    /// Resolve the permission outcome based on policy evaluation.
    async fn resolve_permission_outcome(
        &self,
        policy_result: PolicyEvaluation,
        tool_name: &str,
        request: &PermissionRequest,
    ) -> crate::tools::PermissionOutcome {
        match policy_result {
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
                self.handle_user_consent_required(tool_name, request, options)
                    .await
            }
        }
    }

    /// Handle the case where user consent is required for a tool call.
    async fn handle_user_consent_required(
        &self,
        tool_name: &str,
        request: &PermissionRequest,
        policy_options: Vec<crate::tools::PermissionOption>,
    ) -> crate::tools::PermissionOutcome {
        tracing::info!("Tool '{}' requires user consent", tool_name);

        let permission_options = if !request.options.is_empty() {
            request.options.clone()
        } else {
            policy_options
        };

        if let Some(outcome) = self.check_stored_preference(tool_name).await {
            return outcome;
        }

        self.request_client_permission(tool_name, request, &permission_options)
            .await
    }

    /// Check for a stored permission preference for a tool.
    async fn check_stored_preference(
        &self,
        tool_name: &str,
    ) -> Option<crate::tools::PermissionOutcome> {
        let stored_kind = self.permission_storage.get_preference(tool_name).await?;

        let option_id = match stored_kind {
            crate::tools::PermissionOptionKind::AllowAlways => "allow-always",
            crate::tools::PermissionOptionKind::RejectAlways => "reject-always",
            _ => {
                tracing::warn!(
                    "Unexpected stored permission kind: {}",
                    Pretty(&stored_kind)
                );
                "allow-once"
            }
        };

        tracing::info!(
            "Using stored permission preference for '{}': {}",
            tool_name,
            option_id
        );

        Some(crate::tools::PermissionOutcome::Selected {
            option_id: option_id.to_string(),
        })
    }

    /// Request permission from the client via ACP.
    async fn request_client_permission(
        &self,
        tool_name: &str,
        request: &PermissionRequest,
        permission_options: &[crate::tools::PermissionOption],
    ) -> crate::tools::PermissionOutcome {
        let Some(ref client) = self.client else {
            tracing::warn!(
                "Permission required for tool '{}' but no client connection available",
                tool_name
            );
            return crate::tools::PermissionOutcome::Cancelled;
        };

        let acp_options = Self::convert_to_acp_options(permission_options);
        let acp_request = self.build_acp_permission_request(request, acp_options);

        match client.request_permission(acp_request).await {
            Ok(response) => {
                self.process_acp_permission_response(response, tool_name, permission_options)
                    .await
            }
            Err(e) => {
                tracing::error!("Failed to request permission from client: {}", e);
                crate::tools::PermissionOutcome::Cancelled
            }
        }
    }

    /// Convert internal permission options to ACP protocol types.
    fn convert_to_acp_options(
        options: &[crate::tools::PermissionOption],
    ) -> Vec<agent_client_protocol::PermissionOption> {
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
                    opt.option_id.clone(),
                    opt.name.clone(),
                    kind,
                )
            })
            .collect()
    }

    /// Build an ACP permission request from the internal request.
    fn build_acp_permission_request(
        &self,
        request: &PermissionRequest,
        acp_options: Vec<agent_client_protocol::PermissionOption>,
    ) -> agent_client_protocol::RequestPermissionRequest {
        let tool_call_update = agent_client_protocol::ToolCallUpdate::new(
            agent_client_protocol::ToolCallId::new(request.tool_call.tool_call_id.as_str()),
            agent_client_protocol::ToolCallUpdateFields::new(),
        );
        agent_client_protocol::RequestPermissionRequest::new(
            request.session_id.clone(),
            tool_call_update,
            acp_options,
        )
    }

    /// Process the ACP permission response and store preferences if needed.
    async fn process_acp_permission_response(
        &self,
        response: agent_client_protocol::RequestPermissionResponse,
        tool_name: &str,
        permission_options: &[crate::tools::PermissionOption],
    ) -> crate::tools::PermissionOutcome {
        match response.outcome {
            agent_client_protocol::RequestPermissionOutcome::Cancelled => {
                crate::tools::PermissionOutcome::Cancelled
            }
            agent_client_protocol::RequestPermissionOutcome::Selected(selected) => {
                let option_id_str = selected.option_id.to_string();

                if let Some(option) = permission_options
                    .iter()
                    .find(|opt| opt.option_id == option_id_str)
                {
                    self.permission_storage
                        .store_preference(tool_name, option.kind.clone())
                        .await;
                }

                crate::tools::PermissionOutcome::Selected {
                    option_id: option_id_str,
                }
            }
            _ => {
                tracing::warn!("Unknown permission outcome, treating as cancelled");
                crate::tools::PermissionOutcome::Cancelled
            }
        }
    }

    // File operation handlers (handle_read_text_file, handle_write_text_file, etc.)
    // are implemented in agent_file_handlers.rs

    /// Convert ACP MCP server configuration to internal configuration type for validation
    pub(crate) fn convert_acp_to_internal_mcp_config(
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
    pub(crate) fn convert_session_setup_error_to_acp_error(
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
}

impl AgentWithFixture for ClaudeAgent {
    fn agent_type(&self) -> &'static str {
        "claude"
    }
}

// Fixture support
