use crate::chat_template::ChatTemplateEngine;
use crate::dependency_analysis::{DependencyAnalyzer, ParallelExecutionDecision};
use crate::generation::GenerationHelper;
use crate::mcp::MCPClient;
use crate::model::ModelManager;
use crate::queue::RequestQueue;
use crate::session::SessionManager;
use crate::session::{CompactionResult, CompactionSummary};
use crate::types::{
    AgentAPI, AgentConfig, AgentError, CompactionConfig, GenerationRequest, GenerationResponse,
    HealthStatus, Message, Session, SessionId, StreamChunk, ToolCall, ToolResult,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use llama_common::retry::{RetryConfig as CommonRetryConfig, RetryManager, RetryableError};

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, trace, warn};

/// Default context window size in tokens used as fallback when model metadata is unavailable.
/// Most modern LLMs support at least 4K tokens. This value is used in compaction decisions
/// when we cannot query the model for its actual context size, ensuring safe operation
/// even in degraded states.
const DEFAULT_CONTEXT_SIZE: usize = 4096;

/// Capability types for ACP operations

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapabilityType {
    FsRead,
    FsWrite,
    Terminal,
}

/// Mapping of tool name patterns to required ACP capabilities
/// This provides a robust, maintainable way to enforce capability checks
/// without relying on fragile string matching in the hot path.

const TOOL_CAPABILITY_MAP: &[(&str, CapabilityType)] = &[
    // Filesystem read operations
    ("fs/read", CapabilityType::FsRead),
    ("read_file", CapabilityType::FsRead),
    ("read_text_file", CapabilityType::FsRead),
    ("fs_read", CapabilityType::FsRead),
    // Filesystem write operations
    ("fs/write", CapabilityType::FsWrite),
    ("write_file", CapabilityType::FsWrite),
    ("write_text_file", CapabilityType::FsWrite),
    ("fs_write", CapabilityType::FsWrite),
    // Terminal operations
    ("terminal", CapabilityType::Terminal),
    ("shell", CapabilityType::Terminal),
];

/// Type alias for the summary generator function used in session compaction.
/// This function takes a vector of messages and returns a future that produces
/// a summary string or an error.
type SummaryGeneratorFn = Box<
    dyn Fn(
            Vec<Message>,
        ) -> Pin<
            Box<
                dyn std::future::Future<Output = Result<String, crate::types::SessionError>> + Send,
            >,
        > + Send
        + Sync,
>;

pub struct AgentServer {
    model_manager: Arc<ModelManager>,
    request_queue: Arc<RequestQueue>,
    session_manager: Arc<SessionManager>,
    mcp_client: Arc<dyn MCPClient>,
    /// Per-session MCP clients for ACP sessions
    /// Maps SessionId to a vector of MCP clients created from session/new mcpServers parameter
    pub(crate) session_mcp_clients: Arc<
        tokio::sync::RwLock<
            std::collections::HashMap<crate::types::SessionId, Vec<Arc<dyn MCPClient>>>,
        >,
    >,
    chat_template: Arc<ChatTemplateEngine>,
    dependency_analyzer: Arc<DependencyAnalyzer>,
    config: AgentConfig,
    start_time: Instant,
    shutdown_token: tokio_util::sync::CancellationToken,
    /// Client capabilities from ACP initialize request (if running as ACP server)
    /// This enables capability enforcement for filesystem, terminal, and other operations
    pub(crate) client_capabilities:
        tokio::sync::RwLock<Option<agent_client_protocol::ClientCapabilities>>,
}

impl std::fmt::Debug for AgentServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentServer")
            .field("config", &self.config)
            .field("start_time", &self.start_time)
            .finish()
    }
}

impl AgentServer {
    pub fn new(
        model_manager: Arc<ModelManager>,
        request_queue: Arc<RequestQueue>,
        session_manager: Arc<SessionManager>,
        mcp_client: Arc<dyn MCPClient>,
        chat_template: Arc<ChatTemplateEngine>,
        dependency_analyzer: Arc<DependencyAnalyzer>,
        config: AgentConfig,
    ) -> Self {
        Self {
            model_manager,
            request_queue,
            session_manager,
            mcp_client,
            session_mcp_clients: Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            chat_template,
            dependency_analyzer,
            config,
            start_time: Instant::now(),
            shutdown_token: tokio_util::sync::CancellationToken::new(),
            client_capabilities: tokio::sync::RwLock::new(None),
        }
    }

    /// Set client capabilities from ACP initialize request
    ///
    /// This method should be called by ACP server implementations after receiving
    /// the initialize request from the client. The stored capabilities are used
    /// to enforce permission checks for filesystem, terminal, and other operations.
    ///
    /// # Arguments
    ///
    /// * `capabilities` - Client capabilities from the ACP initialize request
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // In ACP server implementation:
    /// let client_caps = initialize_request.client_capabilities;
    /// agent_server.set_client_capabilities(client_caps).await;
    /// ```

    pub async fn set_client_capabilities(
        &self,
        capabilities: agent_client_protocol::ClientCapabilities,
    ) {
        let mut caps = self.client_capabilities.write().await;
        *caps = Some(capabilities);
    }

    /// Get current client capabilities
    ///
    /// Returns the client capabilities if they have been set via an ACP initialize
    /// request, or None if running in non-ACP mode or before initialization.

    pub async fn get_client_capabilities(
        &self,
    ) -> Option<agent_client_protocol::ClientCapabilities> {
        self.client_capabilities.read().await.clone()
    }

    /// Check if a tool operation is allowed based on client capabilities
    ///
    /// This method enforces capability checks for ACP operations. It uses a robust
    /// mapping structure (TOOL_CAPABILITY_MAP) to determine which capability is required
    /// for each tool and verifies the client has advertised support.
    ///
    /// # Capability Mapping
    ///
    /// The mapping is defined in TOOL_CAPABILITY_MAP and includes:
    /// - Filesystem read operations → `fs.read_text_file`
    /// - Filesystem write operations → `fs.write_text_file`
    /// - Terminal operations → `terminal`
    ///
    /// If no client capabilities have been set (non-ACP mode), all operations are allowed.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Name of the tool to check
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the operation is allowed
    /// * `Err(AgentError)` - If the client lacks the required capability

    async fn check_tool_capability(&self, tool_name: &str) -> Result<(), AgentError> {
        // Get client capabilities
        let caps = self.client_capabilities.read().await;

        // If no capabilities are set, allow all operations (non-ACP mode)
        let Some(ref capabilities) = *caps else {
            return Ok(());
        };

        // MCP tools from external servers don't require local filesystem capabilities
        // Only built-in agent tools require capability checks
        // MCP tools are identified by being in session.available_tools with server_name != "agent"
        // For now, skip capability checks for all MCP tools (external tools)
        // Built-in tools would have specific patterns we check below

        // Map tool name to required capability using the robust mapping structure
        let tool_lower = tool_name.to_lowercase();

        // Find the required capability by checking if any pattern matches the tool name
        let required_capability = TOOL_CAPABILITY_MAP
            .iter()
            .find(|(pattern, _)| tool_lower.contains(pattern))
            .map(|(_, cap_type)| cap_type);

        // If a capability requirement is found, verify the client has it
        if let Some(&capability_type) = required_capability {
            let (has_capability, capability_name) = match capability_type {
                CapabilityType::FsRead => (capabilities.fs.read_text_file, "filesystem read"),
                CapabilityType::FsWrite => (capabilities.fs.write_text_file, "filesystem write"),
                CapabilityType::Terminal => (capabilities.terminal, "terminal"),
            };

            if !has_capability {
                return Err(AgentError::Session(
                    crate::types::SessionError::InvalidState(format!(
                        "Client does not support {} operations (tool: {})",
                        capability_name, tool_name
                    )),
                ));
            }
        }

        Ok(())
    }

    pub fn mcp_client(&self) -> &dyn MCPClient {
        self.mcp_client.as_ref()
    }

    pub fn session_manager(&self) -> &SessionManager {
        &self.session_manager
    }

    pub fn chat_template(&self) -> &ChatTemplateEngine {
        &self.chat_template
    }

    pub fn request_queue(&self) -> &RequestQueue {
        &self.request_queue
    }

    /// Execute an MCP tool directly via the MCP client
    ///
    /// This method provides direct access to MCP tool execution without requiring
    /// a full session context. It's useful for:
    /// - ACP server implementations that need to execute tools
    /// - Administrative or system-level tool calls
    /// - Tools that don't require session history
    ///
    /// # Capability Enforcement
    ///
    /// When running as an ACP server (with the `acp` feature enabled), this method
    /// enforces client capability checks before executing operations. Tools will
    /// fail with an error if the client hasn't advertised the required capability.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Name of the MCP tool to execute
    /// * `arguments` - JSON arguments for the tool
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - Tool execution result as a string
    /// * `Err(AgentError)` - If tool execution fails or capabilities are missing
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use serde_json::json;
    ///
    /// let result = agent_server
    ///     .execute_mcp_tool("fs_read", json!({"path": "/tmp/test.txt"}))
    ///     .await?;
    /// ```
    pub async fn execute_mcp_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String, AgentError> {
        debug!(
            "Executing MCP tool: {} with arguments: {}",
            tool_name, arguments
        );

        // Check capabilities if running in ACP mode

        self.check_tool_capability(tool_name).await?;

        self.mcp_client
            .call_tool(tool_name, arguments)
            .await
            .map_err(AgentError::MCP)
    }

    /// Handle ACP extension methods
    ///
    /// This method provides a hook for ACP servers to implement custom extension
    /// methods beyond the standard ACP protocol. Extension methods allow clients
    /// to request custom functionality like file system operations, terminal output,
    /// or other domain-specific features.
    ///
    /// # Capability Enforcement
    ///
    /// **IMPORTANT**: This method enforces client capability checks before executing
    /// extension methods. The implementation verifies that clients have advertised
    /// the required capabilities (fs.read_text_file, fs.write_text_file, terminal)
    /// before proceeding with the operation.
    ///
    /// Capability checks are performed for:
    /// - `fs/read_text_file`: Requires `client_capabilities.fs.read_text_file`
    /// - `fs/write_text_file`: Requires `client_capabilities.fs.write_text_file`
    /// - Terminal operations: Requires `client_capabilities.terminal`
    ///
    /// Failing capability checks will return an error, preventing clients from
    /// executing operations they haven't declared support for.
    ///
    /// # Arguments
    ///
    /// * `method` - The extension method name (e.g., "fs/read_text_file")
    /// * `params` - JSON parameters for the extension method
    ///
    /// # Returns
    ///
    /// * `Ok(serde_json::Value)` - Extension method result
    /// * `Err(AgentError)` - If the extension method is not supported or fails
    ///
    /// # Default Implementation
    ///
    /// By default, this method returns an error indicating the extension method
    /// is not supported. ACP server implementations should override this to provide
    /// custom extension method handling with proper capability checks.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // In an ACP server implementation with capability checking:
    /// pub async fn ext_method(
    ///     &self,
    ///     method: &str,
    ///     params: serde_json::Value,
    ///     client_capabilities: &ClientCapabilities,
    /// ) -> Result<serde_json::Value, AgentError> {
    ///     match method {
    ///         "fs/read_text_file" => {
    ///             // Check capability before execution
    ///             if !client_capabilities.fs.read_text_file {
    ///                 return Err(AgentError::Session(
    ///                     SessionError::InvalidState(
    ///                         "Client does not support fs/read_text_file".to_string()
    ///                     )
    ///                 ));
    ///             }
    ///             // Execute operation...
    ///         }
    ///         _ => Err(AgentError::Session(
    ///             SessionError::InvalidState(
    ///                 format!("Extension method '{}' not supported", method)
    ///             )
    ///         ))
    ///     }
    /// }
    /// ```
    pub async fn ext_method(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, AgentError> {
        // Check capabilities even for unsupported methods to enforce security

        {
            // Map extension method names to required capabilities
            match method {
                m if m.starts_with("fs/read") || m.contains("read_text_file") => {
                    self.check_tool_capability("fs/read_text_file").await?;
                }
                m if m.starts_with("fs/write") || m.contains("write_text_file") => {
                    self.check_tool_capability("fs/write_text_file").await?;
                }
                m if m.contains("terminal") => {
                    self.check_tool_capability("terminal").await?;
                }
                _ => {
                    // Unknown extension methods don't require specific capabilities
                }
            }
        }

        warn!(
            "Extension method '{}' not implemented, params: {}",
            method, params
        );
        Err(AgentError::Session(
            crate::types::SessionError::InvalidState(format!(
                "Extension method '{}' is not supported",
                method
            )),
        ))
    }

    pub async fn shutdown(self) -> Result<(), AgentError> {
        info!("Initiating AgentServer shutdown");
        let shutdown_start = Instant::now();

        // Signal shutdown to all components
        info!("Signaling shutdown to all components...");
        self.shutdown_token.cancel();

        // Shutdown MCP client first - allow graceful completion
        info!("Shutting down MCP client...");
        match self.mcp_client.shutdown_all().await {
            Ok(_) => info!("MCP client shutdown completed successfully"),
            Err(e) => warn!("MCP client shutdown encountered error: {}", e),
        }

        // Shutdown request queue gracefully
        info!("Shutting down request queue...");
        match Arc::try_unwrap(self.request_queue) {
            Ok(queue) => {
                // We have exclusive ownership - perform graceful shutdown
                queue.shutdown().await;
                info!("Request queue shutdown completed successfully");
            }
            Err(arc_queue) => {
                // There are still references to the queue - log stats and let it drop naturally
                let queue_stats = arc_queue.get_stats();
                warn!(
                    "Request queue has {} remaining references - shutdown will occur on drop",
                    Arc::strong_count(&arc_queue)
                );
                warn!(
                    "Queue stats at shutdown: {} pending, {} completed, {} failed",
                    queue_stats.current_queue_size,
                    queue_stats.completed_requests,
                    queue_stats.failed_requests
                );
            }
        }

        // Complete shutdown process
        info!("Finalizing shutdown of remaining components...");

        let shutdown_duration = shutdown_start.elapsed();
        info!(
            "AgentServer shutdown completed gracefully in {:?}",
            shutdown_duration
        );

        Ok(())
    }

    /// Validate tool call arguments against the tool's parameter schema
    fn validate_tool_arguments(
        &self,
        tool_call: &ToolCall,
        tool_def: &crate::types::ToolDefinition,
    ) -> Result<(), String> {
        // If no parameters schema is defined, skip validation
        if tool_def.parameters.is_null() {
            debug!("No parameter schema defined for tool '{}'", tool_call.name);
            return Ok(());
        }

        // Basic validation - could be enhanced with JSON Schema validation
        if tool_call.arguments.is_null() && !tool_def.parameters.is_null() {
            return Err("Tool requires arguments but none provided".to_string());
        }

        // Additional validation could be added here:
        // - JSON Schema validation against tool_def.parameters
        // - Type checking for required fields
        // - Range validation for numeric parameters

        debug!(
            "Tool arguments validation passed for '{}' (basic validation only)",
            tool_call.name
        );
        Ok(())
    }

    /// Determine if tool calls should be executed in parallel using sophisticated dependency analysis
    fn should_execute_in_parallel(&self, tool_calls: &[ToolCall]) -> bool {
        debug!(
            "Analyzing {} tool calls for parallel execution using dependency analysis",
            tool_calls.len()
        );

        match self
            .dependency_analyzer
            .analyze_parallel_execution(tool_calls)
        {
            ParallelExecutionDecision::Parallel => {
                debug!("Dependency analysis result: PARALLEL execution approved");
                true
            }
            ParallelExecutionDecision::Sequential(reason) => {
                debug!(
                    "Dependency analysis result: SEQUENTIAL execution required - {}",
                    reason
                );
                false
            }
        }
    }

    /// Execute multiple tool calls in parallel
    async fn execute_tools_parallel(
        &self,
        tool_calls: Vec<ToolCall>,
        session: &Session,
    ) -> Vec<ToolResult> {
        use futures::future::join_all;

        let futures = tool_calls.into_iter().map(|tool_call| {
            let session = session.clone();
            async move {
                debug!(
                    "Starting parallel execution of tool: {} (id: {})",
                    tool_call.name, tool_call.id
                );
                debug!("Parallel tool call arguments: {}", tool_call.arguments);

                match self.execute_tool(tool_call.clone(), &session).await {
                    Ok(result) => {
                        if let Some(error) = &result.error {
                            debug!(
                                "Parallel tool call '{}' completed with error: {}",
                                tool_call.name, error
                            );
                        } else {
                            debug!(
                                "Parallel tool call '{}' completed successfully",
                                tool_call.name
                            );
                            debug!(
                                "Parallel tool call '{}' result: {}",
                                tool_call.name, result.result
                            );
                        }
                        result
                    }
                    Err(e) => {
                        error!("Parallel tool call '{}' failed: {}", tool_call.name, e);
                        debug!(
                            "Parallel tool call '{}' unexpected error details: {}",
                            tool_call.name, e
                        );
                        ToolResult {
                            call_id: tool_call.id,
                            result: serde_json::Value::Null,
                            error: Some(format!("Parallel execution error: {}", e)),
                        }
                    }
                }
            }
        });

        let results = join_all(futures).await;
        debug!(
            "Parallel tool execution completed with {} results",
            results.len()
        );
        results
    }

    async fn process_tool_calls(
        &self,
        text: &str,
        session: &Session,
    ) -> Result<Vec<ToolResult>, AgentError> {
        debug!("Processing tool calls from generated text");
        debug!("Generated text to analyze: {}", text);

        // Extract tool calls from the generated text
        let tool_calls = match self.chat_template.extract_tool_calls(text) {
            Ok(calls) => {
                debug!(
                    "Successfully extracted {} tool calls from text",
                    calls.len()
                );
                debug!("Tool call extraction result:");
                for (i, call) in calls.iter().enumerate() {
                    debug!(
                        "  Tool call {}: name='{}', id='{}', arguments={}",
                        i + 1,
                        call.name,
                        call.id,
                        call.arguments
                    );
                }
                calls
            }
            Err(e) => {
                error!("Failed to extract tool calls from text: {}", e);
                debug!("Text that failed tool call extraction: {}", text);
                return Ok(Vec::new()); // Return empty results rather than failing
            }
        };

        if tool_calls.is_empty() {
            debug!("No tool calls found in generated text");
            debug!("Text analyzed: {}", text);
            return Ok(Vec::new());
        }

        debug!("Found {} tool calls to process", tool_calls.len());
        for (i, tool_call) in tool_calls.iter().enumerate() {
            debug!(
                "Tool call {}: name='{}', id='{}', arguments={}",
                i + 1,
                tool_call.name,
                tool_call.id,
                tool_call.arguments
            );
        }
        let mut results = Vec::new();
        let mut successful_calls = 0;
        let mut failed_calls = 0;

        // Check if we should execute tools in parallel or sequentially
        let parallel_execution =
            tool_calls.len() > 1 && self.should_execute_in_parallel(&tool_calls);

        if parallel_execution {
            debug!("Executing {} tool calls in parallel", tool_calls.len());
            results = self.execute_tools_parallel(tool_calls, session).await;

            // Count results for logging
            for result in &results {
                if result.error.is_some() {
                    failed_calls += 1;
                } else {
                    successful_calls += 1;
                }
            }
        } else {
            debug!("Executing {} tool calls sequentially", tool_calls.len());

            // Process each tool call sequentially
            for (i, tool_call) in tool_calls.into_iter().enumerate() {
                debug!(
                    "Processing tool call {}/{}: {} (id: {})",
                    i + 1,
                    results.len() + 1,
                    tool_call.name,
                    tool_call.id
                );
                debug!("Tool call arguments: {}", tool_call.arguments);

                // Execute tool call - errors are handled within execute_tool and returned as ToolResult
                debug!(
                    "Executing tool call '{}' with id '{}'...",
                    tool_call.name, tool_call.id
                );
                match self.execute_tool(tool_call.clone(), session).await {
                    Ok(result) => {
                        if let Some(error) = &result.error {
                            failed_calls += 1;
                            warn!(
                                "Tool call '{}' completed with error: {}",
                                tool_call.name, error
                            );
                            debug!(
                                "Tool call '{}' error result: call_id={}, error={}",
                                tool_call.name, result.call_id, error
                            );
                        } else {
                            successful_calls += 1;
                            debug!("Tool call '{}' completed successfully", tool_call.name);
                            debug!(
                                "Tool call '{}' success result: call_id={}, result={}",
                                tool_call.name, result.call_id, result.result
                            );
                        }
                        results.push(result);
                    }
                    Err(e) => {
                        // This should rarely happen since execute_tool now handles errors internally
                        failed_calls += 1;
                        error!(
                            "Unexpected error executing tool call '{}': {}",
                            tool_call.name, e
                        );

                        // Create error result to maintain call order and IDs
                        let error_result = ToolResult {
                            call_id: tool_call.id,
                            result: serde_json::Value::Null,
                            error: Some(format!("Execution error: {}", e)),
                        };
                        results.push(error_result);
                    }
                }
            }
        }

        debug!(
            "Tool call processing completed: {} successful, {} failed, {} total",
            successful_calls,
            failed_calls,
            results.len()
        );

        Ok(results)
    }

    async fn render_session_prompt(&self, session: &Session) -> Result<String, AgentError> {
        self.model_manager
            .with_model(|model| {
                self.chat_template.render_session_with_config(
                    session,
                    model,
                    Some(&self.config.model),
                )
            })
            .await?
            .map_err(AgentError::Template)
    }

    /// Basic validation for generation requests
    fn validate_generation_request_with_session(
        &self,
        _request: &GenerationRequest,
        session: &Session,
    ) -> Result<(), AgentError> {
        use crate::validation::{AgentValidator, Validator};

        // Use structured validation system
        let validator = AgentValidator::new();
        validator
            .validate(session, session)
            .map_err(|validation_error| {
                // Convert ValidationError to AgentError::Session
                AgentError::Session(validation_error.into())
            })?;

        Ok(())
    }

    /// Get metadata about the currently loaded model
    pub async fn get_model_metadata(&self) -> Option<llama_loader::ModelMetadata> {
        self.model_manager.get_metadata().await
    }

    /// Get template cache statistics
    pub fn get_template_cache_stats(&self) -> crate::template_cache::CacheStats {
        self.model_manager.get_template_cache_stats()
    }
}

#[async_trait]
impl AgentAPI for AgentServer {
    async fn initialize(config: AgentConfig) -> Result<Self, AgentError> {
        info!("Initializing AgentServer with config: {:?}", config);

        // Validate configuration
        config.validate()?;

        // Initialize model manager
        let model_manager = ModelManager::new(config.model.clone())?;
        model_manager.load_model().await?;
        info!("Model manager initialized and model loaded");
        let model_manager = Arc::new(model_manager);

        // Initialize request queue
        let request_queue = Arc::new(RequestQueue::new(
            model_manager.clone(),
            config.queue_config.clone(),
            config.session_config.clone(),
        ));
        info!("Request queue initialized");

        // Initialize session manager
        let session_manager = Arc::new(SessionManager::new(config.session_config.clone()));
        info!("Session manager initialized");

        // Initialize MCP client based on the first configured server
        let mcp_client: Arc<dyn crate::mcp::MCPClient> = if config.mcp_servers.is_empty() {
            info!("No MCP servers configured - using no-op client");
            // No MCP servers configured - use a no-op client
            Arc::new(crate::mcp::NoOpMCPClient::new())
        } else {
            info!(
                "MCP servers configured: {} servers",
                config.mcp_servers.len()
            );
            // Use the first MCP server configuration to determine transport type
            match &config.mcp_servers[0] {
                crate::types::MCPServerConfig::InProcess(process_config) => {
                    info!(
                        "Creating MCP client with spawned process: {} command: {}",
                        process_config.name, process_config.command
                    );
                    // Use rmcp's child process support to spawn and connect
                    Arc::new(
                        crate::mcp::UnifiedMCPClient::with_spawned_process(
                            &process_config.command,
                            &process_config.args,
                            process_config.timeout_secs,
                        )
                        .await?,
                    )
                }
                crate::types::MCPServerConfig::Http(http_config) => {
                    info!(
                        "Creating HTTP MCP client for server: {} at {}",
                        http_config.name, http_config.url
                    );
                    // For HTTP servers, use streamable HTTP transport
                    Arc::new(
                        crate::mcp::UnifiedMCPClient::with_streamable_http(
                            &http_config.url,
                            http_config.timeout_secs,
                        )
                        .await?,
                    )
                }
            }
        };
        info!("MCP client initialized");

        // Initialize chat template engine
        let chat_template = Arc::new(ChatTemplateEngine::new());
        info!("Chat template engine initialized");

        // Initialize dependency analyzer with configured settings
        let dependency_analyzer = Arc::new(DependencyAnalyzer::new(
            config.parallel_execution_config.clone(),
        ));
        info!("Dependency analyzer initialized with configuration");

        let agent_server = Self::new(
            model_manager,
            request_queue,
            session_manager,
            mcp_client,
            chat_template,
            dependency_analyzer,
            config,
        );

        info!("AgentServer initialization completed");
        Ok(agent_server)
    }

    /// Generate a response for the given request, executing tool calls as needed.
    ///
    /// This method processes the generation request and may execute multiple tool calls
    /// in sequence until the conversation naturally reaches completion. The execution
    /// continues without artificial iteration limits, relying on natural termination
    /// mechanisms such as model context limits, successful task completion, or user
    /// intervention.
    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResponse, AgentError> {
        debug!(
            "Processing generation request for session: {}",
            request.session_id
        );

        // Try auto-compaction before generation
        self.maybe_auto_compact(&request.session_id).await?;

        // Get session from session manager
        let mut session = self
            .session_manager
            .get_session(&request.session_id)
            .await?
            .ok_or_else(|| {
                AgentError::Session(crate::types::SessionError::NotFound(
                    request.session_id.to_string(),
                ))
            })?;

        // Initialize template cache on first generation for this session
        if session.template_token_count.is_none() {
            debug!("Initializing template cache for session: {}", session.id);

            // Template initialization must happen within with_model to access the model
            // but initialize_session_with_template is async, so we need to handle this carefully
            let template_token_count = self
                .model_manager
                .with_model(|model| {
                    // Create context for template initialization
                    let mut ctx = self
                        .model_manager
                        .create_session_context(model, &session.id)
                        .map_err(|e| {
                            crate::types::ModelError::LoadingFailed(format!(
                                "Failed to create context for template initialization: {}",
                                e
                            ))
                        })?;

                    // Extract template components synchronously
                    let (system_prompt, tools_json) = self
                        .chat_template
                        .extract_template_components(&session)
                        .map_err(|e| {
                            crate::types::ModelError::LoadingFailed(format!(
                                "Failed to extract template: {}",
                                e
                            ))
                        })?;

                    // Hash template for cache lookup
                    let template_hash = crate::template_cache::TemplateCache::hash_template(
                        &system_prompt,
                        &tools_json,
                    );

                    // Check cache synchronously
                    let cache_hit = {
                        let cache = self.model_manager.template_cache();
                        let mut cache_guard = cache.lock().unwrap();
                        cache_guard
                            .get(template_hash)
                            .map(|entry| entry.token_count)
                    };

                    if let Some(token_count) = cache_hit {
                        // Cache HIT - load KV cache from file
                        debug!(
                            "Loading cached template {} ({} tokens)",
                            template_hash, token_count
                        );

                        let n_ctx = ctx.n_ctx() as usize;
                        let _tokens = self.model_manager.load_template_kv_cache(
                            &mut ctx,
                            template_hash,
                            n_ctx,
                        )?;

                        debug!(
                            "Session initialized with cached template: {} tokens",
                            token_count
                        );
                        return Ok(token_count);
                    }

                    // Cache MISS - need to process template
                    // This requires async operations, so we'll need to defer this
                    // For now, return an error indicating async processing is needed
                    Err(crate::types::ModelError::LoadingFailed(
                        "Template cache miss - async processing required".to_string(),
                    ))
                })
                .await;

            match template_token_count {
                Ok(Ok(count)) => {
                    // Cache hit - update session with count
                    debug!(
                        "Template cache hit for session {}: {} tokens",
                        session.id, count
                    );
                    session.template_token_count = Some(count);
                    self.session_manager
                        .update_session(session.clone())
                        .await
                        .map_err(AgentError::Session)?;
                }
                Ok(Err(e)) if e.to_string().contains("async processing required") => {
                    // Cache miss - skip template initialization for now
                    // The template will be processed as part of the normal prompt on first generation
                    debug!(
                        "Template cache miss for session {}, will process with first generation",
                        session.id
                    );
                }
                Ok(Err(e)) => {
                    // Other error
                    return Err(AgentError::Model(e));
                }
                Err(e) => {
                    return Err(AgentError::Model(e));
                }
            }
        }

        // Security: Validate input before processing
        self.validate_generation_request_with_session(&request, &session)?;

        let mut working_session = session;
        let mut accumulated_response = String::new();
        let mut total_tokens = 0u32;

        loop {
            debug!(
                "Processing tool call iteration for session: {}",
                working_session.id
            );
            debug!(
                "Current session has {} messages",
                working_session.messages.len()
            );
            for (i, msg) in working_session.messages.iter().enumerate() {
                debug!(
                    "Message {}: {:?} - {}",
                    i + 1,
                    msg.role,
                    if msg.content.len() > 100 {
                        format!("{}...", &msg.content[..100])
                    } else {
                        msg.content.clone()
                    }
                );
            }

            // Create generation request with current session state
            let current_request = GenerationRequest {
                session_id: working_session.id,
                max_tokens: request.max_tokens,
                temperature: request.temperature,
                top_p: request.top_p,
                stop_tokens: request.stop_tokens.clone(),
                stopping_config: request.stopping_config.clone(),
            };

            // Submit to request queue
            let response = self
                .request_queue
                .submit_request(current_request, &working_session)
                .await?;

            accumulated_response.push_str(&response.generated_text);
            total_tokens += response.tokens_generated;

            debug!(
                "Generation completed: {} tokens, finish_reason: {:?}",
                response.tokens_generated, response.finish_reason
            );
            debug!("Generated text:\n{}\n", response.generated_text);

            // Check if response contains tool calls
            match &response.finish_reason {
                crate::types::FinishReason::Stopped(reason) if reason == "Tool call detected" => {
                    debug!("Tool call detected, processing tool calls...");
                    debug!(
                        "Generated text for tool call processing: {}",
                        response.generated_text
                    );

                    // Process tool calls
                    debug!("Beginning tool call processing workflow...");
                    let tool_results = self
                        .process_tool_calls(&response.generated_text, &working_session)
                        .await?;
                    debug!(
                        "Tool call processing completed with {} results",
                        tool_results.len()
                    );

                    if tool_results.is_empty() {
                        debug!("No tool results returned, ending tool call workflow");
                        break;
                    }

                    // Add the assistant's response (with tool calls) to the session
                    debug!("Adding assistant message with tool calls to session");
                    trace!("Assistant message content: {}", response.generated_text);
                    debug!(
                        "Session message count before adding assistant message: {}",
                        working_session.messages.len()
                    );
                    working_session.messages.push(crate::types::Message {
                        role: crate::types::MessageRole::Assistant,
                        content: response.generated_text.clone(),
                        tool_call_id: None,
                        tool_name: None,
                        timestamp: std::time::SystemTime::now(),
                    });
                    debug!(
                        "Session message count after adding assistant message: {}",
                        working_session.messages.len()
                    );

                    // Add tool results as Tool messages to the session
                    debug!(
                        "Adding {} tool results as messages to session",
                        tool_results.len()
                    );
                    debug!(
                        "Session message count before adding tool results: {}",
                        working_session.messages.len()
                    );

                    for (i, tool_result) in tool_results.iter().enumerate() {
                        let tool_content = if let Some(error) = &tool_result.error {
                            debug!("Tool result {}: ERROR - {}", i + 1, error);
                            format!("Error: {}", error)
                        } else {
                            let content = serde_json::to_string(&tool_result.result)
                                .unwrap_or_else(|_| "Invalid tool result".to_string());
                            debug!("Tool result {}: SUCCESS - {}", i + 1, content);
                            content
                        };

                        debug!(
                            "Adding tool message {}/{} for call_id: {}",
                            i + 1,
                            tool_results.len(),
                            tool_result.call_id
                        );
                        debug!(
                            "Tool message content length: {} characters",
                            tool_content.len()
                        );
                        working_session.messages.push(crate::types::Message {
                            role: crate::types::MessageRole::Tool,
                            content: tool_content,
                            tool_call_id: Some(tool_result.call_id),
                            tool_name: None,
                            timestamp: std::time::SystemTime::now(),
                        });
                        debug!(
                            "Session message count after adding tool result {}: {}",
                            i + 1,
                            working_session.messages.len()
                        );
                    }

                    working_session.updated_at = std::time::SystemTime::now();

                    debug!(
                        "Tool call processing completed with {} results, continuing generation",
                        tool_results.len()
                    );
                    debug!(
                        "Final session message count after tool workflow: {}",
                        working_session.messages.len()
                    );
                    debug!("Continuing to next iteration to generate response incorporating tool results");

                    // Continue the loop to generate response incorporating tool results
                    continue;
                }
                crate::types::FinishReason::Stopped(reason) => {
                    // No more tool calls, we're done
                    debug!(
                        "Generation completed without tool calls (reason: {})",
                        reason
                    );
                    debug!("Final generated text: {}", response.generated_text);
                    debug!(
                        "Final accumulated response length: {} characters",
                        accumulated_response.len()
                    );
                    break;
                }
            }
        }

        let final_response = GenerationResponse {
            generated_text: accumulated_response,
            tokens_generated: total_tokens,
            generation_time: std::time::Duration::from_millis(0), // This would need proper timing
            finish_reason: crate::types::FinishReason::Stopped(
                "End of sequence token detected".to_string(),
            ), // Or original finish reason
            complete_token_sequence: None, // Agent-level generation doesn't track tokens for caching
        };

        debug!(
            "Complete generation workflow finished: {} total tokens",
            total_tokens
        );

        Ok(final_response)
    }

    async fn generate_stream(
        &self,
        request: GenerationRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, AgentError>> + Send>>, AgentError>
    {
        debug!(
            "Processing streaming generation request for session: {}",
            request.session_id
        );

        // Try auto-compaction before generation
        self.maybe_auto_compact(&request.session_id).await?;

        // Get session from session manager
        let session = self
            .session_manager
            .get_session(&request.session_id)
            .await?
            .ok_or_else(|| {
                AgentError::Session(crate::types::SessionError::NotFound(
                    request.session_id.to_string(),
                ))
            })?;

        // Security: Validate input before processing
        self.validate_generation_request_with_session(&request, &session)?;

        // Render session to prompt
        let prompt = self.render_session_prompt(&session).await?;
        debug!("Session rendered to prompt: {} characters", prompt.len());

        // Create streaming request
        let streaming_request = GenerationRequest {
            session_id: request.session_id,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            stop_tokens: request.stop_tokens,
            stopping_config: request.stopping_config,
        };

        // Submit to request queue for streaming
        let receiver = self
            .request_queue
            .submit_streaming_request(streaming_request, &session)
            .await
            .map_err(AgentError::Queue)?;

        // Convert the receiver to a stream and map QueueError to AgentError
        let stream = ReceiverStream::new(receiver).map(|result| result.map_err(AgentError::Queue));

        Ok(Box::pin(stream))
    }

    async fn create_session(&self) -> Result<Session, AgentError> {
        let session = self.session_manager.create_session().await?;
        debug!("Created new session: {}", session.id);
        Ok(session)
    }

    async fn create_session_with_cwd(&self, cwd: PathBuf) -> Result<Session, AgentError> {
        let session = self
            .session_manager
            .create_session_with_cwd_and_transcript(cwd, None)
            .await?;
        debug!("Created new session with cwd: {}", session.id);
        Ok(session)
    }

    async fn create_session_with_transcript(
        &self,
        transcript_path: Option<PathBuf>,
    ) -> Result<Session, AgentError> {
        let session = self
            .session_manager
            .create_session_with_transcript(transcript_path)
            .await?;
        debug!("Created new session with transcript: {}", session.id);
        Ok(session)
    }

    async fn get_session(&self, session_id: &SessionId) -> Result<Option<Session>, AgentError> {
        let session = self.session_manager.get_session(session_id).await?;
        match &session {
            Some(s) => debug!("Retrieved session: {}", s.id),
            None => debug!("Session not found: {}", session_id),
        }
        Ok(session)
    }

    async fn add_message(
        &self,
        session_id: &SessionId,
        message: Message,
    ) -> Result<(), AgentError> {
        self.session_manager
            .add_message(session_id, message)
            .await
            .map_err(AgentError::Session)
    }

    async fn discover_tools(&self, session: &mut Session) -> Result<(), AgentError> {
        debug!("Discovering tools for session: {}", session.id);

        let tool_names = self.mcp_client.list_tools().await?;
        session.available_tools = tool_names
            .into_iter()
            .map(|name| crate::types::ToolDefinition {
                name: name.clone(),
                description: format!("Tool: {}", name),
                parameters: serde_json::Value::Object(serde_json::Map::new()),
                server_name: "discovered".to_string(),
            })
            .collect();
        session.updated_at = SystemTime::now();

        info!(
            "Discovered {} tools for session {}",
            session.available_tools.len(),
            session.id
        );

        // Update the session in the session manager so the tools are persisted
        self.session_manager
            .update_session(session.clone())
            .await
            .map_err(AgentError::Session)?;

        Ok(())
    }

    async fn execute_tool(
        &self,
        tool_call: ToolCall,
        session: &Session,
    ) -> Result<ToolResult, AgentError> {
        debug!(
            "Executing tool call: {} (id: {}) in session: {}",
            tool_call.name, tool_call.id, session.id
        );
        debug!("Tool call arguments: {}", tool_call.arguments);

        // Check client capabilities if running in ACP mode. In non-ACP mode, all tools are allowed.
        // When the acp feature is enabled, this enforces that clients have advertised the required
        // capabilities (filesystem operations, terminal access) before tools are executed.

        if let Err(e) = self.check_tool_capability(&tool_call.name).await {
            let error_msg = format!("Capability check failed: {}", e);
            error!(
                "Tool call '{}' blocked by capability check: {}",
                tool_call.name, error_msg
            );
            return Ok(ToolResult {
                call_id: tool_call.id,
                result: serde_json::Value::Null,
                error: Some(error_msg),
            });
        }

        // Validate tool call name is not empty
        if tool_call.name.trim().is_empty() {
            let error_msg = "Tool name cannot be empty";
            error!("{}", error_msg);
            return Ok(ToolResult {
                call_id: tool_call.id,
                result: serde_json::Value::Null,
                error: Some(error_msg.to_string()),
            });
        }

        // Find the tool definition
        let tool_def = match session
            .available_tools
            .iter()
            .find(|t| t.name == tool_call.name)
        {
            Some(tool) => tool,
            None => {
                let error_msg = format!(
                    "Tool '{}' not found in available tools. Available tools: {}",
                    tool_call.name,
                    session
                        .available_tools
                        .iter()
                        .map(|t| t.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                error!("{}", error_msg);
                return Ok(ToolResult {
                    call_id: tool_call.id,
                    result: serde_json::Value::Null,
                    error: Some(error_msg),
                });
            }
        };

        debug!(
            "Found tool definition for '{}' on server '{}'",
            tool_call.name, tool_def.server_name
        );

        // Validate tool arguments structure if parameters schema is available
        if let Err(validation_error) = self.validate_tool_arguments(&tool_call, tool_def) {
            warn!(
                "Tool call arguments validation failed for '{}': {}",
                tool_call.name, validation_error
            );
            // Continue execution despite validation failure but log the issue
        }

        // Execute the tool call with retry logic for transient failures
        let tool_result = self.execute_tool_with_retry(&tool_call, session).await;

        // Sync session todos if this was a todo-related tool call and it succeeded

        if tool_result.error.is_none()
            && (tool_call.name == "mcp__swissarmyhammer__todo_create"
                || tool_call.name == "mcp__swissarmyhammer__todo_mark_complete")
        {
            debug!(
                "Tool '{}' affects todos, syncing session todos",
                tool_call.name
            );
            if let Err(e) = self.sync_session_todos(&session.id).await {
                warn!(
                    "Failed to sync session todos after '{}': {}",
                    tool_call.name, e
                );
                // Don't fail the tool call if sync fails - the tool itself succeeded
            }
        }

        Ok(tool_result)
    }

    async fn health(&self) -> Result<HealthStatus, AgentError> {
        debug!("Performing health check");

        let model_loaded = self.model_manager.is_loaded().await;
        let queue_stats = self.request_queue.get_stats();
        let sessions_count = self.session_manager.get_session_count().await;
        let mcp_health = self.mcp_client.health_check().await;

        let all_servers_healthy = mcp_health.is_ok();
        let status = if model_loaded && all_servers_healthy {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        };

        let health_status = HealthStatus {
            status,
            model_loaded,
            queue_size: queue_stats.current_queue_size,
            active_sessions: sessions_count,
            uptime: self.start_time.elapsed(),
        };

        debug!("Health check completed: {:?}", health_status);
        Ok(health_status)
    }

    /// Compact a session using AI summarization.
    ///
    /// Replaces conversation history with a concise summary when token usage
    /// approaches context limits, optionally preserving recent messages.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to compact
    /// * `config` - Compaction configuration, or None to use defaults
    ///
    /// # Returns
    ///
    /// `CompactionResult` containing statistics about the compression operation
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use llama_agent::{Agent, CompactionConfig};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let agent = Agent::initialize(Default::default()).await?;
    /// let session = agent.create_session().await?;
    ///
    /// let config = CompactionConfig {
    ///     threshold: 0.8,
    ///     context_limit: 4096,
    ///     preserve_recent: 2,
    ///     custom_prompt: None,
    /// };
    ///
    /// let result = agent.compact_session(&session.id, Some(config)).await?;
    /// println!("Compressed {} tokens to {}", result.original_tokens, result.compressed_tokens);
    /// # Ok(())
    /// # }
    /// ```
    async fn compact_session(
        &self,
        session_id: &SessionId,
        config: Option<CompactionConfig>,
    ) -> Result<CompactionResult, AgentError> {
        let generate_summary =
            Self::create_summary_generator(self.model_manager.clone(), self.chat_template.clone());

        self.session_manager
            .compact_session(session_id, config, generate_summary)
            .await
            .map_err(AgentError::Session)
    }

    /// Check if a session should be compacted based on token usage.
    ///
    /// Evaluates whether the session's current token usage exceeds the
    /// configured threshold relative to the context limit.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to evaluate
    /// * `config` - Configuration containing threshold and context limit
    ///
    /// # Returns
    ///
    /// `true` if the session meets compaction criteria
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use llama_agent::{Agent, CompactionConfig};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let agent = Agent::initialize(Default::default()).await?;
    /// let session = agent.create_session().await?;
    ///
    /// let config = CompactionConfig::default();
    /// if agent.should_compact_session(&session.id, &config).await? {
    ///     agent.compact_session(&session.id, Some(config)).await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn should_compact_session(
        &self,
        session_id: &SessionId,
        config: &CompactionConfig,
    ) -> Result<bool, AgentError> {
        let session = self
            .session_manager
            .get_session(session_id)
            .await
            .map_err(AgentError::Session)?
            .ok_or_else(|| {
                AgentError::Session(crate::types::SessionError::NotFound(session_id.to_string()))
            })?;

        let context_size = self
            .get_model_metadata()
            .await
            .map(|metadata| metadata.context_size)
            .unwrap_or(DEFAULT_CONTEXT_SIZE); // Default fallback if metadata not available

        Ok(session.should_compact(context_size, config.threshold))
    }

    /// Auto-compact sessions based on token usage across all sessions.
    ///
    /// Identifies sessions that meet compaction criteria and compacts them
    /// automatically, providing a summary of operations performed.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for compaction criteria and behavior
    ///
    /// # Returns
    ///
    /// `CompactionSummary` with statistics about the batch operation
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use llama_agent::{Agent, CompactionConfig};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let agent = Agent::initialize(Default::default()).await?;
    ///
    /// let config = CompactionConfig::default();
    /// let summary = agent.auto_compact_sessions(&config).await?;
    ///
    /// println!("Compacted {} sessions, saved {} tokens",
    ///          summary.successful_compactions,
    ///          summary.total_tokens_saved);
    /// # Ok(())
    /// # }
    /// ```
    async fn auto_compact_sessions(
        &self,
        config: &CompactionConfig,
    ) -> Result<CompactionSummary, AgentError> {
        let generate_summary =
            Self::create_summary_generator(self.model_manager.clone(), self.chat_template.clone());

        self.session_manager
            .auto_compact_sessions(config, generate_summary)
            .await
            .map_err(AgentError::Session)
    }

    /// Load an existing session by ID
    ///
    /// Retrieves a session from persistent storage and restores its state,
    /// allowing continuation of a previous conversation.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The ID of the session to load
    ///
    /// # Returns
    ///
    /// The loaded session with full conversation history
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist or cannot be loaded
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use llama_agent::{AgentServer, AgentConfig, AgentAPI, SessionId};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AgentConfig::default();
    /// let agent = AgentServer::initialize(config).await?;
    ///
    /// // Load an existing session
    /// let session_id = SessionId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV")?;
    /// let session = agent.load_session(&session_id).await?;
    ///
    /// println!("Loaded session with {} messages", session.messages.len());
    /// # Ok(())
    /// # }
    /// ```
    async fn load_session(&self, session_id: &SessionId) -> Result<Session, AgentError> {
        debug!("Loading session: {}", session_id);

        let session = self
            .session_manager
            .get_session(session_id)
            .await?
            .ok_or_else(|| {
                AgentError::Session(crate::types::SessionError::NotFound(session_id.to_string()))
            })?;

        info!(
            "Loaded session {} with {} messages",
            session_id,
            session.messages.len()
        );

        Ok(session)
    }
}

impl AgentServer {
    /// Update session mode
    pub async fn set_session_mode(
        &self,
        session_id: &SessionId,
        mode: String,
    ) -> Result<(), AgentError> {
        let mut session = self
            .session_manager
            .get_session(session_id)
            .await?
            .ok_or_else(|| {
                AgentError::Session(crate::types::SessionError::NotFound(session_id.to_string()))
            })?;

        session.current_mode = Some(mode);
        self.session_manager.update_session(session).await?;
        Ok(())
    }

    /// Create a new session and return its ID.
    ///
    /// This is a convenience method that wraps `create_session` and returns
    /// just the session ID, matching the ergonomic API shown in documentation examples.
    ///
    /// # Returns
    ///
    /// The ID of the newly created session
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use llama_agent::{AgentServer, AgentConfig, AgentAPI};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AgentConfig::default();
    /// let agent = AgentServer::initialize(config).await?;
    /// let session_id = agent.new_session().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new_session(&self) -> Result<SessionId, AgentError> {
        let session = self.create_session().await?;
        Ok(session.id)
    }

    /// Send a text prompt to the agent and get a response.
    ///
    /// This is a high-level convenience method that simplifies the common workflow of:
    /// 1. Adding a user message to the session
    /// 2. Generating a response
    /// 3. Extracting the generated text
    ///
    /// The method automatically handles tool calls and continues generation until
    /// the conversation naturally completes. For more control over the generation
    /// process, use `add_message` and `generate` directly.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to send the prompt to
    /// * `prompt` - The text prompt from the user
    ///
    /// # Returns
    ///
    /// The generated response text
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use llama_agent::{AgentServer, AgentConfig, AgentAPI};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AgentConfig::default();
    /// let agent = AgentServer::initialize(config).await?;
    /// let session_id = agent.new_session().await?;
    ///
    /// let response = agent.prompt(&session_id, "Hello, how are you?").await?;
    /// println!("Agent: {}", response);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn prompt(&self, session_id: &SessionId, prompt: &str) -> Result<String, AgentError> {
        debug!("Processing prompt for session: {}", session_id);

        if prompt.trim().is_empty() {
            return Err(AgentError::Session(
                crate::types::SessionError::InvalidState("Prompt cannot be empty".to_string()),
            ));
        }

        // Add user message to session
        let user_message = Message {
            role: crate::types::MessageRole::User,
            content: prompt.to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };

        self.add_message(session_id, user_message).await?;

        // Generate response
        let generation_request = GenerationRequest {
            session_id: *session_id,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        let generation_response = self.generate(generation_request).await?;

        debug!(
            "Prompt completed: {} tokens generated",
            generation_response.tokens_generated
        );

        Ok(generation_response.generated_text)
    }

    /// Submit a streaming generation request directly to the request queue
    ///
    /// This is a lower-level API that bypasses tool calling and session management.
    /// For most use cases, prefer `generate_stream` which includes full tool integration.
    ///
    /// # Arguments
    ///
    /// * `request` - The generation request with parameters
    ///
    /// # Returns
    ///
    /// A receiver that yields streaming chunks as they are generated
    pub async fn submit_streaming_request(
        &self,
        request: GenerationRequest,
    ) -> Result<
        tokio::sync::mpsc::Receiver<Result<StreamChunk, crate::types::QueueError>>,
        AgentError,
    > {
        // Try auto-compaction before generation
        self.maybe_auto_compact(&request.session_id).await?;

        // Get session from session manager
        let session = self
            .session_manager
            .get_session(&request.session_id)
            .await?
            .ok_or_else(|| {
                AgentError::Session(crate::types::SessionError::NotFound(
                    request.session_id.to_string(),
                ))
            })?;

        // Security: Validate input before processing
        self.validate_generation_request_with_session(&request, &session)?;

        // Submit to request queue for streaming
        self.request_queue
            .submit_streaming_request(request, &session)
            .await
            .map_err(AgentError::Queue)
    }

    /// Check and perform auto-compaction if needed before generation
    async fn maybe_auto_compact(&self, session_id: &SessionId) -> Result<(), AgentError> {
        // Check if auto-compaction is configured
        if let Some(config) = &self.config.session_config.auto_compaction {
            if self.should_compact_session(session_id, config).await? {
                info!("Auto-compacting session {} before generation", session_id);

                match self.compact_session(session_id, Some(config.clone())).await {
                    Ok(result) => {
                        info!(
                            "Auto-compaction successful for session {}: {:.1}% reduction, {} -> {} tokens",
                            session_id,
                            (1.0 - result.compression_ratio) * 100.0,
                            result.original_tokens,
                            result.compressed_tokens
                        );
                    }
                    Err(e) => {
                        warn!("Auto-compaction failed for session {}: {}", session_id, e);
                        // Continue with generation anyway - compaction failure shouldn't block generation
                    }
                }
            }
        }
        Ok(())
    }

    /// Creates a summary generation function for session compaction.
    ///
    /// This helper method eliminates code duplication between compact_session
    /// and auto_compact_sessions by providing a shared closure that generates
    /// summaries for message histories.
    ///
    /// # Returns
    ///
    /// A closure that takes messages and returns a future producing a summary string
    fn create_summary_generator(
        model_manager: Arc<ModelManager>,
        chat_template: Arc<ChatTemplateEngine>,
    ) -> SummaryGeneratorFn {
        Box::new(move |messages: Vec<Message>| {
            let model_manager = model_manager.clone();
            let chat_template = chat_template.clone();

            Box::pin(async move {
                use crate::types::{Session, SessionId};
                use std::time::SystemTime;

                let temp_session = Session {
                    id: SessionId::new(),
                    messages,
                    cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
                    mcp_servers: Vec::new(),
                    available_tools: Vec::new(),
                    available_prompts: Vec::new(),
                    created_at: SystemTime::now(),
                    updated_at: SystemTime::now(),
                    compaction_history: Vec::new(),
                    transcript_path: None,
                    context_state: None,
                    template_token_count: None,
                    todos: Vec::new(),
                    available_commands: Vec::new(),
                    current_mode: None,
                    client_capabilities: None,
                };

                model_manager
                    .with_model(|model| {
                        let prompt = match chat_template.render_session_with_config(
                            &temp_session,
                            model,
                            Some(model_manager.get_config()),
                        ) {
                            Ok(prompt) => prompt,
                            Err(e) => {
                                return Err(crate::types::SessionError::InvalidState(format!(
                                    "Failed to render session prompt: {}",
                                    e
                                )))
                            }
                        };

                        let mut ctx =
                            match model_manager.create_session_context(model, &temp_session.id) {
                                Ok(context) => context,
                                Err(e) => {
                                    return Err(crate::types::SessionError::InvalidState(format!(
                                        "Failed to create context: {}",
                                        e
                                    )))
                                }
                            };

                        let request = GenerationRequest {
                            session_id: SessionId::new(),
                            max_tokens: Some(512),
                            temperature: None,
                            top_p: None,
                            stop_tokens: Vec::new(),
                            stopping_config: None,
                        };

                        let batch_size = model_manager.get_batch_size();
                        let generation_result =
                            match GenerationHelper::generate_text_with_borrowed_model(
                                model,
                                &mut ctx,
                                &prompt,
                                &request,
                                &tokio_util::sync::CancellationToken::new(),
                                batch_size,
                            ) {
                                Ok(result) => result,
                                Err(e) => {
                                    return Err(crate::types::SessionError::InvalidState(format!(
                                        "Generation failed during compaction: {}",
                                        e
                                    )))
                                }
                            };

                        Ok(generation_result.generated_text.trim().to_string())
                    })
                    .await
                    .map_err(|e| {
                        crate::types::SessionError::InvalidState(format!(
                            "Model error during summarization: {}",
                            e
                        ))
                    })?
            })
        })
    }

    /// Synchronize session todos with TodoStorage
    ///
    /// Loads all todos from the TodoStorage and updates the session's todos vector.
    /// This ensures the session has the latest todo state from the filesystem.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID to sync todos for
    ///
    /// # Returns
    ///
    /// `Ok(())` if sync succeeds, `Err` if the session doesn't exist or sync fails

    async fn sync_session_todos(&self, session_id: &SessionId) -> Result<(), AgentError> {
        use swissarmyhammer_todo::TodoStorage;

        debug!("Syncing todos for session: {}", session_id);

        // Create TodoStorage instance
        let storage = TodoStorage::new_default().map_err(|e| {
            AgentError::Session(crate::types::SessionError::InvalidState(format!(
                "Failed to create todo storage: {}",
                e
            )))
        })?;

        // Load all todos from storage
        let todo_list = storage.get_todo_list().await.map_err(|e| {
            AgentError::Session(crate::types::SessionError::InvalidState(format!(
                "Failed to load todo list: {}",
                e
            )))
        })?;

        // Get the session, update its todos, and save it back
        let mut session = self
            .session_manager
            .get_session(session_id)
            .await?
            .ok_or_else(|| {
                AgentError::Session(crate::types::SessionError::NotFound(session_id.to_string()))
            })?;

        // Update session todos
        if let Some(list) = todo_list {
            session.todos = list.todo;
            debug!(
                "Updated session {} with {} todos",
                session_id,
                session.todos.len()
            );
        } else {
            session.todos.clear();
            debug!(
                "Cleared todos for session {} (no todo list found)",
                session_id
            );
        }

        // Save the updated session back
        self.session_manager
            .update_session(session)
            .await
            .map_err(AgentError::Session)?;

        Ok(())
    }

    /// Execute a tool call with retry logic for transient failures
    ///
    /// This method implements error recovery strategies including:
    /// - Retry with exponential backoff for network errors
    /// - Retry for server errors (5xx)
    /// - Immediate failure for client errors (4xx)
    /// - Graceful degradation by returning errors in ToolResult
    ///
    /// # Capability Enforcement
    ///
    /// When running in ACP mode, this method checks client capabilities before
    /// executing tools. Tools requiring capabilities the client hasn't advertised
    /// will fail immediately without retrying.
    async fn execute_tool_with_retry(&self, tool_call: &ToolCall, session: &Session) -> ToolResult {
        // Check capabilities before attempting execution (ACP mode)

        if let Err(e) = self.check_tool_capability(&tool_call.name).await {
            let error_msg = format!("Capability check failed: {}", e);
            error!(
                "Tool call '{}' blocked by capability check: {}",
                tool_call.name, error_msg
            );
            return ToolResult {
                call_id: tool_call.id,
                result: serde_json::Value::Null,
                error: Some(error_msg),
            };
        }
        // Create a retry-enabled error type
        #[derive(Debug)]
        struct ToolExecutionError {
            message: String,
            is_retriable: bool,
        }

        impl std::fmt::Display for ToolExecutionError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.message)
            }
        }

        impl std::error::Error for ToolExecutionError {}

        impl RetryableError for ToolExecutionError {
            fn is_retriable(&self) -> bool {
                self.is_retriable
            }
        }

        // Configure retry behavior for tool execution
        let retry_config = CommonRetryConfig {
            max_retries: 3,
            initial_delay: std::time::Duration::from_millis(500),
            backoff_multiplier: 2.0,
            max_delay: std::time::Duration::from_secs(10),
            use_jitter: true,
        };

        let retry_manager = RetryManager::with_config(retry_config);

        // Clone data needed for the retry closure
        let tool_name = tool_call.name.clone();
        let tool_args = tool_call.arguments.clone();
        let call_id = tool_call.id;

        // Get MCP client for this session (if any), otherwise use agent-level client
        let mcp_client = {
            let session_clients = self.session_mcp_clients.read().await;
            if let Some(clients) = session_clients.get(&session.id) {
                // Use first MCP client for this session
                // TODO: Route to specific client based on tool name/server
                clients.first().cloned()
            } else {
                None
            }
        }
        .unwrap_or_else(|| Arc::clone(&self.mcp_client));

        // Execute with retry logic
        let result = retry_manager
            .retry(&format!("tool_{}", tool_name), || async {
                debug!("Calling MCP server for tool '{}' (attempt)", tool_name);

                mcp_client
                    .call_tool(&tool_name, tool_args.clone())
                    .await
                    .map_err(|mcp_error| {
                        let error_msg = mcp_error.to_string();
                        let is_retriable = AgentServer::is_tool_error_retriable(&error_msg);

                        if is_retriable {
                            debug!(
                                "Tool '{}' failed with retriable error: {}",
                                tool_name, error_msg
                            );
                        } else {
                            debug!(
                                "Tool '{}' failed with non-retriable error: {}",
                                tool_name, error_msg
                            );
                        }

                        ToolExecutionError {
                            message: error_msg,
                            is_retriable,
                        }
                    })
            })
            .await;

        // Convert result to ToolResult
        match result {
            Ok(result_value) => {
                debug!("Tool call '{}' completed successfully", tool_name);
                debug!("Tool call result: {}", result_value);
                ToolResult {
                    call_id,
                    result: serde_json::Value::String(result_value),
                    error: None,
                }
            }
            Err(execution_error) => {
                let error_msg = format!("Tool execution failed: {}", execution_error);
                error!("Tool call '{}' failed: {}", tool_name, error_msg);
                debug!("Failed tool call arguments were: {}", tool_args);

                // Return ToolResult with error instead of propagating the error
                // This allows the workflow to continue with partial failures
                ToolResult {
                    call_id,
                    result: serde_json::Value::Null,
                    error: Some(error_msg),
                }
            }
        }
    }

    /// Determine if a tool execution error should be retried
    ///
    /// Recovery strategies:
    /// - Network errors (connection, timeout, DNS): Retry with backoff
    /// - Server errors (5xx): Retry with backoff
    /// - Rate limiting (429): Do not retry (requires different strategy)
    /// - Client errors (4xx): Do not retry (user/configuration error)
    /// - Unknown errors: Retry conservatively
    fn is_tool_error_retriable(error_msg: &str) -> bool {
        let error_lower = error_msg.to_lowercase();

        // Server errors (5xx) are retriable
        if error_lower.contains("500")
            || error_lower.contains("internal server error")
            || error_lower.contains("502")
            || error_lower.contains("bad gateway")
            || error_lower.contains("503")
            || error_lower.contains("service unavailable")
            || error_lower.contains("504")
            || error_lower.contains("gateway timeout")
        {
            return true;
        }

        // Network-level errors are retriable
        if error_lower.contains("connection")
            || error_lower.contains("timeout")
            || error_lower.contains("network")
            || error_lower.contains("dns")
            || error_lower.contains("reset")
            || error_lower.contains("refused")
        {
            return true;
        }

        // Rate limiting should not be retried immediately
        if error_lower.contains("429") || error_lower.contains("too many requests") {
            return false;
        }

        // Client errors (4xx) are not retriable
        if error_lower.contains("400")
            || error_lower.contains("bad request")
            || error_lower.contains("401")
            || error_lower.contains("unauthorized")
            || error_lower.contains("403")
            || error_lower.contains("forbidden")
            || error_lower.contains("404")
            || error_lower.contains("not found")
        {
            return false;
        }

        // Validation errors are not retriable
        if error_lower.contains("invalid")
            || error_lower.contains("validation")
            || error_lower.contains("malformed")
        {
            return false;
        }

        // Default to non-retriable for safety - tool errors are often deterministic
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig, SessionConfig,
    };

    fn create_test_config() -> AgentConfig {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();

        AgentConfig {
            model: ModelConfig {
                source: ModelSource::Local {
                    folder: temp_dir.path().to_path_buf(),
                    filename: Some("test.gguf".to_string()),
                },
                batch_size: 512,
                n_seq_max: 1,
                n_threads: 1,
                n_threads_batch: 1,
                use_hf_params: false,
                retry_config: RetryConfig::default(),
                debug: false,
            },
            queue_config: QueueConfig::default(),
            mcp_servers: Vec::new(),
            session_config: SessionConfig::default(),
            parallel_execution_config: ParallelConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_agent_server_creation() {
        let config = create_test_config();

        // The config validation will fail because the test.gguf file doesn't exist,
        // but that's expected for this test. We're testing that we can create the config
        // structure correctly
        match config.validate() {
            Ok(()) => {
                // This would mean all validation passed (unlikely without real model file)
                // Config validation succeeded
            }
            Err(_) => {
                // Expected - the test.gguf file doesn't exist
                // Config validation failed as expected
            }
        }
    }

    #[test]
    fn test_agent_server_debug() {
        let config = create_test_config();
        let debug_str = format!("{:?}", config);

        // Just test that we can debug the config - safer than trying to create a full AgentServer
        assert!(debug_str.contains("AgentConfig"));
        assert!(debug_str.contains("model"));
        assert!(debug_str.contains("queue_config"));
        assert!(debug_str.contains("session_config"));
    }

    #[test]
    fn test_config_validation() {
        let mut config = create_test_config();
        // Note: config.validate() will fail due to missing model file, but that's expected

        // Test invalid batch size
        config.model.batch_size = 0;
        assert!(config.validate().is_err());

        // Reset and test invalid queue config
        config = create_test_config();
        config.queue_config.max_queue_size = 0;
        assert!(config.validate().is_err());

        // Reset and test invalid session config
        config = create_test_config();
        config.session_config.max_sessions = 0;
        assert!(config.validate().is_err());

        // Test valid values for components that don't depend on file existence
        let valid_model_config = ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "test/model".to_string(),
                filename: Some("model.gguf".to_string()),
                folder: None,
            },
            batch_size: 512,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: false,
            retry_config: RetryConfig::default(),
            debug: false,
        };

        let valid_config = AgentConfig {
            model: valid_model_config,
            queue_config: QueueConfig::default(),
            mcp_servers: Vec::new(),
            session_config: SessionConfig::default(),
            parallel_execution_config: ParallelConfig::default(),
        };

        // This should pass all validation except for the model file not existing
        match valid_config.validate() {
            Ok(()) => {} // Validation passed
            Err(e) => {
                // Expected if model file doesn't exist - that's fine
                let error_msg = format!("{}", e);
                // Should be a model-related error
                assert!(error_msg.contains("model") || error_msg.contains("Model"));
            }
        }
    }
}
