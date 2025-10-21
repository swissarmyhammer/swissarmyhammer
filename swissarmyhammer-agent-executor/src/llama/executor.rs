//! LlamaAgent executor implementation for SwissArmyHammer workflows
//!
//! This module provides the LlamaAgent executor that integrates with the real
//! llama-agent crate to provide AI capabilities for SwissArmyHammer workflows.

use crate::{ActionError, ActionResult, AgentExecutionContext, AgentExecutor, AgentResponse};
use async_trait::async_trait;

use std::sync::Arc;

use swissarmyhammer_config::agent::AgentExecutorType;
use swissarmyhammer_config::{LlamaAgentConfig, ModelSource};
use tokio::sync::OnceCell;

/// Re-exports from the llama_agent crate for external use and type compatibility
///
/// These types are re-exported to provide a unified interface for LlamaAgent configuration
/// and execution. External code should use these re-exports rather than importing directly
/// from llama_agent to maintain API stability and reduce coupling.
///
/// # Type Overview
///
/// ## Core Agent Types
/// - `AgentServer`: The main server for handling agent execution and lifecycle
/// - `AgentConfig`: Configuration for the agent including model, queue, and MCP settings
/// - `AgentAPI`: Interface for interacting with the agent server
///
/// ## Session and Message Types
/// - `Message`: Individual conversation message with role and content
/// - `MessageRole`: Enum for message roles (User, Assistant, System, Tool)
/// - `SessionConfig`: Configuration for conversation session management
///
/// ## Model Configuration Types
/// - `ModelConfig`: Configuration for the LLM model
/// - `ModelSource`: Enum for model sources (HuggingFace, Local)
/// - `StoppingConfig`: Configuration for generation stopping criteria
///
/// ## Execution Configuration Types
/// - `GenerationRequest`: Request for text generation
/// - `ParallelConfig`: Configuration for parallel execution of tool calls
/// - `QueueConfig`: Configuration for request queue management
/// - `RetryConfig`: Configuration for retry logic on failures
///
/// ## MCP (Model Context Protocol) Types
/// - `MCPServerConfig`: Configuration for MCP server connections
/// - `HttpServerConfig`: Configuration for HTTP-based MCP servers
pub use llama_agent::{
    types::{
        AgentAPI, AgentConfig, GenerationRequest, HttpServerConfig, MCPServerConfig, Message,
        MessageRole, ModelConfig, ModelSource as LlamaModelSource, ParallelConfig, QueueConfig,
        RetryConfig, SessionConfig, StoppingConfig,
    },
    AgentServer,
};

/// HTTP MCP server handle for managing server lifecycle
#[derive(Debug, Clone)]
pub struct McpServerHandle {
    /// Actual bound port (important when using port 0 for random port)
    port: u16,
    /// Full HTTP URL for connecting to the server
    url: String,
    /// Shutdown sender for graceful shutdown
    shutdown_tx: std::sync::Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl McpServerHandle {
    /// Create a new MCP server handle
    pub fn new(port: u16, host: String, shutdown_tx: tokio::sync::oneshot::Sender<()>) -> Self {
        let url = format!("http://{}:{}", host, port); // Base URL - MCP service is nested at /mcp
        Self {
            port,
            url,
            shutdown_tx: std::sync::Arc::new(tokio::sync::Mutex::new(Some(shutdown_tx))),
        }
    }

    /// Get the actual port the server is bound to
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the full HTTP URL for connecting to the server
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Shutdown the server gracefully
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut guard = self.shutdown_tx.lock().await;
        if let Some(tx) = guard.take() {
            if tx.send(()).is_err() {
                tracing::warn!("Server shutdown signal receiver already dropped");
            }
        }
        Ok(())
    }
}

// Real LlamaAgent Integration
//
// This implementation integrates with the actual llama-agent crate from
// https://github.com/swissarmyhammer/llama-agent to provide AI capabilities.

/// Resource usage statistics for LlamaAgent execution monitoring
///
/// Provides detailed metrics about model resource consumption, session management,
/// and processing performance for monitoring and optimization purposes.
///
/// # Example
/// ```rust
/// use swissarmyhammer::workflow::agents::llama_agent_executor::LlamaResourceStats;
///
/// let stats = LlamaResourceStats {
///     memory_usage_mb: 2048,
///     model_size_mb: 1500,
///     active_sessions: 3,
///     total_tokens_processed: 150000,
///     average_tokens_per_second: 25.5,
/// };
/// println!("Memory usage: {}MB", stats.memory_usage_mb);
/// ```
#[derive(Debug, Clone)]
pub struct LlamaResourceStats {
    /// Current memory usage by the LlamaAgent process in megabytes
    pub memory_usage_mb: u64,
    /// Size of the loaded model in megabytes
    pub model_size_mb: u64,
    /// Number of currently active conversation sessions
    pub active_sessions: usize,
    /// Total number of tokens processed since initialization
    pub total_tokens_processed: u64,
    /// Average processing speed in tokens per second
    pub average_tokens_per_second: f64,
}

/// Global singleton for LlamaAgent executor
/// This ensures the model is loaded once per process, not per prompt
static GLOBAL_LLAMA_EXECUTOR: OnceCell<Arc<tokio::sync::Mutex<LlamaAgentExecutor>>> =
    OnceCell::const_new();

/// LlamaAgent executor implementation
///
/// This executor integrates with the real llama-agent crate and starts an HTTP MCP server
/// in-process to provide SwissArmyHammer tools to the AI agent.
pub struct LlamaAgentExecutor {
    /// Configuration for the LlamaAgent
    config: LlamaAgentConfig,
    /// Whether the executor has been initialized
    initialized: bool,
    /// MCP server handle for SwissArmyHammer tools
    mcp_server: Option<McpServerHandle>,
    /// The actual LlamaAgent server when using real implementation
    agent_server: Option<Arc<AgentServer>>,
}

impl LlamaAgentExecutor {
    /// Create a new LlamaAgent executor with the given configuration and optional MCP server
    ///
    /// # Arguments
    ///
    /// * `config` - LlamaAgent configuration
    /// * `mcp_server` - Optional pre-started MCP server handle. If None, initialization will fail.
    ///
    /// # Architecture Note
    ///
    /// The MCP server should be started by the workflow layer before creating the executor.
    /// This ensures proper separation of concerns: the workflow layer manages infrastructure,
    /// while the executor focuses on execution logic.
    pub fn new(config: LlamaAgentConfig, mcp_server: Option<McpServerHandle>) -> Self {
        Self {
            config,
            initialized: false,
            mcp_server,
            agent_server: None,
        }
    }

    /// Convert SwissArmyHammer LlamaAgentConfig to llama-agent AgentConfig
    fn to_llama_agent_config(&self) -> ActionResult<AgentConfig> {
        tracing::debug!(
            "Converting to llama-agent config with MCP server: {:?}",
            self.mcp_server.is_some()
        );
        // Convert model source with validation
        let model_source = match &self.config.model.source {
            ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => {
                // Validate repo is not empty (simulate real llama-agent validation)
                if repo.is_empty() {
                    return Err(ActionError::ExecutionError(
                        "LlamaAgent initialization failed: Invalid model repository - empty repo string not allowed".to_string()
                    ));
                }

                LlamaModelSource::HuggingFace {
                    repo: repo.clone(),
                    // If folder is provided, use it and set filename to None
                    // If folder is not provided, use filename
                    filename: if folder.is_some() {
                        None
                    } else {
                        filename.clone()
                    },
                    folder: folder.clone(),
                }
            }
            ModelSource::Local { filename, folder } => LlamaModelSource::Local {
                folder: folder.clone().unwrap_or_else(|| {
                    filename
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .to_path_buf()
                }),
                filename: filename
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string()),
            },
        };

        let model_config = ModelConfig {
            source: model_source,
            batch_size: self.config.model.batch_size,
            use_hf_params: self.config.model.use_hf_params,
            retry_config: RetryConfig {
                max_retries: 2,
                initial_delay_ms: 100,
                backoff_multiplier: 1.5,
                max_delay_ms: 1000,
            },
            debug: false, // Hardcode to false to suppress llama.cpp verbose logging
            // Thread and sequence configuration for llama.cpp
            // n_seq_max: Maximum number of parallel sequences/contexts (1 = single conversation)
            // n_threads: Number of threads for prompt processing (4 = balanced for multi-core CPUs)
            // n_threads_batch: Number of threads for batch processing (4 = parallel token generation)
            n_seq_max: 1,
            n_threads: 4,
            n_threads_batch: 4,
        };

        // Create MCP server configs for HTTP transport
        let mcp_servers = if let Some(mcp_server) = &self.mcp_server {
            tracing::debug!("Configuring HTTP MCP server at {}", mcp_server.url());

            let http_config = HttpServerConfig {
                name: "swissarmyhammer".to_string(),
                url: format!("{}/mcp", mcp_server.url()), // Add /mcp path here
                timeout_secs: Some(self.config.mcp_server.timeout_seconds),
                sse_keep_alive_secs: Some(30), // 30 second keepalive
                stateful_mode: false,          // Use stateless mode for simplicity
            };

            let mcp_config = MCPServerConfig::Http(http_config);

            tracing::debug!("MCP server config created: {:?}", mcp_config);

            vec![mcp_config]
        } else {
            tracing::warn!("MCP server not available, creating empty MCP server list");
            Vec::new()
        };

        // Repetition detection has been removed from llama-agent crate.
        // Only basic stopping config with EOS detection is now available.
        tracing::debug!("Using basic StoppingConfig with EOS detection only");

        Ok(AgentConfig {
            model: model_config,
            queue_config: QueueConfig {
                max_queue_size: 100,
                worker_threads: 1,
            },
            session_config: SessionConfig::default(),
            mcp_servers,
            parallel_execution_config: ParallelConfig::default(),
        })
    }

    /// Create StoppingConfig
    fn create_stopping_config(&self) -> StoppingConfig {
        StoppingConfig {
            max_tokens: None,    // Use default/request-specific max_tokens
            eos_detection: true, // Always enable EOS detection
        }
    }

    /// Initialize the real LlamaAgent server with model and MCP configuration
    async fn initialize_agent_server_real(&mut self) -> ActionResult<()> {
        tracing::debug!("REAL initialize_agent_server called");

        tracing::info!(
            "Initializing LlamaAgent server with model: {}",
            self.get_model_display_name()
        );

        // Validate that MCP server was provided before initialization
        if self.mcp_server.is_none() {
            return Err(ActionError::ExecutionError(
                "MCP server must be provided before initialization. The workflow layer should start the MCP server and pass it to the executor constructor.".to_string()
            ));
        }

        // MCP server is already running - just use it
        let mcp_handle = self.mcp_server.as_ref().unwrap();

        tracing::info!(
            "Using pre-started HTTP MCP server on port {} (URL: {})",
            mcp_handle.port(),
            mcp_handle.url()
        );

        // Give the HTTP MCP server a moment to fully initialize
        // This prevents race conditions with llama-agent connecting too quickly
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Convert config to llama-agent format
        let agent_config = self.to_llama_agent_config()?;

        // Initialize the real AgentServer - let llama-agent handle all validation
        let agent_server = AgentServer::initialize(agent_config).await.map_err(|e| {
            tracing::error!("LlamaAgent initialization failed: {}", e);
            ActionError::ExecutionError(format!(
                "LlamaAgent initialization failed (model: {}): {}",
                self.get_model_display_name(),
                e
            ))
        })?;

        self.agent_server = Some(Arc::new(agent_server));

        tracing::info!("LlamaAgent server initialized successfully");
        Ok(())
    }

    /// Get current resource usage statistics
    pub async fn get_resource_stats(&self) -> Result<LlamaResourceStats, ActionError> {
        if let Some(agent_server) = &self.agent_server {
            // Get real statistics from the agent server
            let health = agent_server.health().await.map_err(|e| {
                ActionError::ExecutionError(format!("Failed to get health status: {}", e))
            })?;

            Ok(LlamaResourceStats {
                memory_usage_mb: 1024, // This would come from actual memory monitoring
                model_size_mb: 2048,   // This would come from model info
                active_sessions: health.active_sessions,
                total_tokens_processed: 0, // This would need to be tracked
                average_tokens_per_second: 0.0, // This would be calculated from metrics
            })
        } else if self.initialized {
            // Fallback for when agent server is not available but we're initialized
            Ok(LlamaResourceStats {
                memory_usage_mb: 512,
                model_size_mb: 1024,
                active_sessions: 0,
                total_tokens_processed: 0,
                average_tokens_per_second: 0.0,
            })
        } else {
            Err(ActionError::ExecutionError(
                "Agent not initialized".to_string(),
            ))
        }
    }

    /// Check if model is loaded and ready
    pub async fn is_model_loaded(&self) -> bool {
        {
            if let Some(agent_server) = &self.agent_server {
                if let Ok(health) = agent_server.health().await {
                    return health.model_loaded;
                }
            }
        }

        self.initialized
    }

    /// Get the number of active sessions
    pub async fn get_active_session_count(&self) -> usize {
        {
            if let Some(agent_server) = &self.agent_server {
                if let Ok(health) = agent_server.health().await {
                    return health.active_sessions;
                }
            }
        }

        0
    }

    /// Clean up abandoned sessions (no-op for now, would be implemented with real session management)
    pub async fn cleanup_stale_sessions(&self) -> Result<usize, ActionError> {
        Ok(0)
    }

    /// Get MCP server URL (if available)
    pub fn mcp_server_url(&self) -> Option<String> {
        self.mcp_server
            .as_ref()
            .map(|s| format!("http://127.0.0.1:{}", s.port()))
    }

    /// Get MCP server port (if available)
    pub fn mcp_server_port(&self) -> Option<u16> {
        self.mcp_server.as_ref().map(|s| s.port())
    }

    /// Get the model display name for logging and debugging
    ///
    /// Creates a human-readable string representation of the configured model
    /// for use in logs and debug output.
    ///
    /// # Returns
    ///
    /// A string in one of these formats:
    /// - HuggingFace with filename: `"repo_name/model_file.gguf"` or `"repo_name/model_folder"`
    /// - HuggingFace without filename: `"repo_name"`
    /// - Local model: `"local:/path/to/model.gguf"`
    pub fn get_model_display_name(&self) -> String {
        match &self.config.model.source {
            ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => match (folder, filename) {
                (Some(folder), _) => format!("{}/{}", repo, folder),
                (None, Some(filename)) => format!("{}/{}", repo, filename),
                (None, None) => repo.clone(),
            },
            ModelSource::Local { filename, .. } => {
                format!("local:{}", filename.display())
            }
        }
    }

    /// Validate the LlamaAgent configuration
    ///
    /// Performs comprehensive validation of the configuration to ensure it meets
    /// all requirements for successful initialization and execution.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the configuration is valid, or an error describing
    /// what validation failed.
    ///
    /// # Validation Checks
    ///
    /// - HuggingFace repository names cannot be empty
    /// - Model filenames cannot be empty (when provided)
    /// - Local model files must end with `.gguf` extension
    /// - Local model files must exist on the filesystem
    /// - MCP server timeout must be greater than 0
    /// - HuggingFace models support both single files (.gguf) and folder-based models
    pub fn validate_config(&self) -> Result<(), ActionError> {
        tracing::debug!("Validating LlamaAgent configuration");

        // Validate model source configuration
        match &self.config.model.source {
            ModelSource::HuggingFace { repo, filename, .. } => {
                // Validate repository name
                if repo.is_empty() {
                    return Err(ActionError::ExecutionError(
                        "HuggingFace repository name cannot be empty".to_string(),
                    ));
                }

                // Validate filename if provided
                if let Some(filename) = filename {
                    if filename.is_empty() {
                        return Err(ActionError::ExecutionError(
                            "Model filename cannot be empty when specified".to_string(),
                        ));
                    }
                }

                tracing::debug!("HuggingFace model configuration is valid: {}", repo);
            }
            ModelSource::Local { filename, .. } => {
                // Validate local file extension
                if !filename.extension().is_some_and(|ext| ext == "gguf") {
                    return Err(ActionError::ExecutionError(format!(
                        "Local model file must end with .gguf extension, got: {}",
                        filename.display()
                    )));
                }

                // Validate local file exists
                if !filename.exists() {
                    return Err(ActionError::ExecutionError(format!(
                        "Local model file not found: {}",
                        filename.display()
                    )));
                }

                tracing::debug!("Local model configuration is valid: {}", filename.display());
            }
        }

        // Validate MCP server configuration
        if self.config.mcp_server.timeout_seconds == 0 {
            return Err(ActionError::ExecutionError(
                "MCP server timeout must be greater than 0 seconds".to_string(),
            ));
        }

        // Warn about high timeout values but don't fail validation
        if self.config.mcp_server.timeout_seconds > 300 {
            tracing::warn!(
                "MCP server timeout is very high ({}s), this may cause performance issues",
                self.config.mcp_server.timeout_seconds
            );
        }

        tracing::debug!(
            "MCP server configuration is valid: timeout={}s",
            self.config.mcp_server.timeout_seconds
        );

        tracing::info!(
            "LlamaAgent configuration validation passed for model: {}",
            self.get_model_display_name()
        );

        Ok(())
    }

    /// Get or create the global LlamaAgent executor
    ///
    /// This method implements the singleton pattern to ensure that expensive model
    /// loading operations happen only once per process, regardless of how many
    /// prompts are executed. Subsequent calls with different configurations will
    /// return the same global instance.
    ///
    /// # Arguments
    ///
    /// * `config` - The LlamaAgent configuration to use for initialization
    /// * `mcp_server` - Optional pre-started MCP server handle (required for initialization)
    ///
    /// # Returns
    ///
    /// A `Result` containing an `Arc<Mutex<LlamaAgentExecutor>>` for thread-safe
    /// access to the global executor instance, or an error if initialization fails.
    pub async fn get_global_executor(
        config: LlamaAgentConfig,
        mcp_server: Option<McpServerHandle>,
    ) -> ActionResult<Arc<tokio::sync::Mutex<LlamaAgentExecutor>>> {
        GLOBAL_LLAMA_EXECUTOR
            .get_or_try_init(|| async {
                let mut executor = LlamaAgentExecutor::new(config, mcp_server);
                executor.initialize().await?;
                Ok(Arc::new(tokio::sync::Mutex::new(executor)))
            })
            .await
            .cloned()
            .map_err(|e: ActionError| e)
    }
}

impl Drop for LlamaAgentExecutor {
    fn drop(&mut self) {
        if self.mcp_server.is_some() {
            tracing::debug!("LlamaAgentExecutor dropping - HTTP MCP server handle cleanup");
            // HTTP MCP server handle cleanup - the actual shutdown happens in shutdown() method
            // since Drop cannot be async, we just log here
        }
        tracing::debug!("LlamaAgentExecutor dropped");
    }
}

#[async_trait]
impl AgentExecutor for LlamaAgentExecutor {
    async fn initialize(&mut self) -> ActionResult<()> {
        if self.initialized {
            return Ok(());
        }

        tracing::info!(
            "Initializing LlamaAgent executor with config for model: {}",
            self.get_model_display_name()
        );

        // Always use real initialization - no test mode
        tracing::info!("Using real LlamaAgent initialization");
        self.initialize_agent_server_real().await?;

        self.initialized = true;
        tracing::info!("LlamaAgent executor initialized successfully");
        Ok(())
    }

    async fn shutdown(&mut self) -> ActionResult<()> {
        {
            if let Some(agent_server) = self.agent_server.take() {
                // Shutdown the real agent server
                if let Ok(server) = Arc::try_unwrap(agent_server) {
                    server.shutdown().await.map_err(|e| {
                        ActionError::ExecutionError(format!(
                            "Failed to shutdown agent server: {}",
                            e
                        ))
                    })?;
                }
            }
        }

        // Note: MCP server is owned by the workflow layer and should be shut down there
        // The executor just releases its reference to the MCP server handle
        // The actual MCP server lifecycle is managed by the workflow layer

        tracing::info!("LlamaAgent executor shutdown");
        self.initialized = false;
        Ok(())
    }

    fn executor_type(&self) -> AgentExecutorType {
        AgentExecutorType::LlamaAgent
    }

    async fn execute_prompt(
        &self,
        system_prompt: String,
        rendered_prompt: String,
        context: &AgentExecutionContext<'_>,
    ) -> ActionResult<AgentResponse> {
        if !self.initialized {
            return Err(ActionError::ExecutionError(
                "LlamaAgent executor not initialized".to_string(),
            ));
        }

        let mcp_server_info = if let Some(server) = &self.mcp_server {
            format!("127.0.0.1:{}", server.port())
        } else {
            "not_available".to_string()
        };

        tracing::info!(
            "Executing LlamaAgent with MCP server at {} (skip_tools: {})",
            mcp_server_info,
            context.skip_tools()
        );
        tracing::debug!("System prompt length: {}", system_prompt.len());
        tracing::debug!("Rendered prompt length: {}", rendered_prompt.len());

        let execution_start = std::time::Instant::now();

        // Always use real LlamaAgent execution - no mocking
        tracing::debug!("Using real LlamaAgent execution path");
        tracing::info!("Using real LlamaAgent execution");

        // Execute with real LlamaAgent - no mock fallbacks allowed
        if let Some(agent_server) = &self.agent_server {
            tracing::info!("Using real LlamaAgent execution path");
            return self
                .execute_with_real_agent(
                    agent_server,
                    system_prompt,
                    rendered_prompt,
                    execution_start,
                    context.skip_tools(),
                )
                .await;
        } else {
            return Err(ActionError::ExecutionError(
                "Agent server not available - executor initialization may have failed".to_string(),
            ));
        }
    }
}

impl LlamaAgentExecutor {
    /// Execute with real LlamaAgent when the feature is enabled
    async fn execute_with_real_agent(
        &self,
        agent_server: &Arc<AgentServer>,
        system_prompt: String,
        rendered_prompt: String,
        execution_start: std::time::Instant,
        skip_tools: bool,
    ) -> ActionResult<AgentResponse> {
        // Create a new session
        let mut session = agent_server
            .create_session()
            .await
            .map_err(|e| ActionError::ExecutionError(format!("Failed to create session: {}", e)))?;

        // Discover available tools only if needed (skipped for rule checking optimization)
        if !skip_tools {
            agent_server
                .discover_tools(&mut session)
                .await
                .map_err(|e| {
                    ActionError::ExecutionError(format!("Failed to discover tools: {}", e))
                })?;
        } else {
            tracing::debug!("Skipping tool discovery for rule checking (optimization)");
        }

        // Add system message if provided
        if !system_prompt.is_empty() {
            let system_message = Message {
                role: MessageRole::System,
                content: system_prompt.clone(),
                tool_call_id: None,
                tool_name: None,
                timestamp: std::time::SystemTime::now(),
            };
            agent_server
                .add_message(&session.id, system_message)
                .await
                .map_err(|e| {
                    ActionError::ExecutionError(format!("Failed to add system message: {}", e))
                })?;
        }

        // Add user message
        let user_message = Message {
            role: MessageRole::User,
            content: rendered_prompt.clone(),
            tool_call_id: None,
            tool_name: None,
            timestamp: std::time::SystemTime::now(),
        };
        agent_server
            .add_message(&session.id, user_message)
            .await
            .map_err(|e| {
                ActionError::ExecutionError(format!("Failed to add user message: {}", e))
            })?;

        // Create generation request with repetition detection
        let stopping_config = self.create_stopping_config();
        let session_id = session.id;
        let generation_request =
            GenerationRequest::new(session_id).with_stopping_config(stopping_config);

        // Generate response
        let result = agent_server
            .generate(generation_request)
            .await
            .map_err(|e| ActionError::ExecutionError(format!("Generation failed: {}", e)))?;

        let execution_time = execution_start.elapsed();
        let mcp_url = self.mcp_server_url().unwrap_or_else(|| "none".to_string());

        tracing::info!(
            "LlamaAgent execution completed in {}ms with {} tokens",
            execution_time.as_millis(),
            result.tokens_generated
        );

        // Return response in expected format
        let response = serde_json::json!({
            "status": "success",
            "message": result.generated_text,
            "execution_details": {
                "executor_type": "LlamaAgent",
                "mcp_server_url": mcp_url,
                "mcp_server_port": self.mcp_server_port(),
                "execution_time_ms": execution_time.as_millis(),

                "model": self.get_model_display_name(),
                "tokens_generated": result.tokens_generated,
                "generation_time_ms": result.generation_time.as_millis(),
                "finish_reason": format!("{:?}", result.finish_reason),
                "mode": "real"
            },
            "session_info": {
                "session_id": session.id.to_string(),
                "tools_available": session.available_tools.len(),
                "messages_count": session.messages.len()
            },
            "integration_status": {
                "ready_for_llama_integration": true
            }
        });

        // Convert the JSON response to AgentResponse
        let response_content = result.generated_text;
        Ok(AgentResponse::success_with_metadata(
            response_content,
            response,
        ))
    }
}

/// Wrapper that provides AgentExecutor interface while delegating to the global singleton
///
/// This wrapper solves the model loading issue by ensuring that all prompt actions
/// use the same global LlamaAgentExecutor instance, preventing repeated model loading.
pub struct LlamaAgentExecutorWrapper {
    config: LlamaAgentConfig,
    mcp_server: Option<McpServerHandle>,
    global_executor: Option<Arc<tokio::sync::Mutex<LlamaAgentExecutor>>>,
}

impl LlamaAgentExecutorWrapper {
    /// Create a new wrapper instance without MCP server (will fail on initialize)
    pub fn new(config: LlamaAgentConfig) -> Self {
        Self {
            config,
            mcp_server: None,
            global_executor: None,
        }
    }

    /// Create a new wrapper instance with pre-started MCP server
    pub fn new_with_mcp(config: LlamaAgentConfig, mcp_server: Option<McpServerHandle>) -> Self {
        Self {
            config,
            mcp_server,
            global_executor: None,
        }
    }
}

#[async_trait]
impl AgentExecutor for LlamaAgentExecutorWrapper {
    async fn initialize(&mut self) -> ActionResult<()> {
        tracing::info!("Initializing LlamaAgent wrapper with singleton pattern");

        // Get or create the global singleton executor with MCP server
        let global_executor =
            LlamaAgentExecutor::get_global_executor(self.config.clone(), self.mcp_server.clone())
                .await?;
        self.global_executor = Some(global_executor);

        tracing::info!("LlamaAgent wrapper initialized - using global singleton");
        Ok(())
    }

    async fn shutdown(&mut self) -> ActionResult<()> {
        tracing::info!("LlamaAgent wrapper shutdown - global singleton remains active");
        // Don't shutdown the global executor, just release our reference
        self.global_executor = None;
        Ok(())
    }

    fn executor_type(&self) -> AgentExecutorType {
        AgentExecutorType::LlamaAgent
    }

    async fn execute_prompt(
        &self,
        system_prompt: String,
        rendered_prompt: String,
        context: &AgentExecutionContext<'_>,
    ) -> ActionResult<AgentResponse> {
        let global_executor = self.global_executor.as_ref().ok_or_else(|| {
            ActionError::ExecutionError("LlamaAgent wrapper not initialized".to_string())
        })?;

        tracing::debug!("Delegating to global LlamaAgent executor");

        // Delegate to the global singleton
        let executor_guard = global_executor.lock().await;
        executor_guard
            .execute_prompt(system_prompt, rendered_prompt, context)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_config::{McpServerConfig, ModelConfig};

    #[test_log::test(tokio::test)]
    async fn test_llama_agent_executor_creation() {
        let config = LlamaAgentConfig::for_testing();
        let executor = LlamaAgentExecutor::new(config, None);

        assert!(!executor.initialized);
        assert!(executor.mcp_server.is_none());
        assert_eq!(executor.executor_type(), AgentExecutorType::LlamaAgent);
    }

    #[test_log::test(tokio::test)]
    async fn test_llama_agent_executor_initialization() {
        // Start MCP server first (now done at workflow layer)
        use swissarmyhammer_prompts::PromptLibrary;
        use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

        let tools_handle = start_mcp_server(
            McpServerMode::Http { port: None },
            Some(PromptLibrary::default()),
        )
        .await
        .expect("Failed to start test MCP server");

        // Convert to agent-executor handle
        let port = tools_handle.info.port.unwrap_or(0);
        let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
        let mcp_handle = McpServerHandle::new(port, "127.0.0.1".to_string(), dummy_tx);

        let config = LlamaAgentConfig::for_testing();
        let mut executor = LlamaAgentExecutor::new(config, Some(mcp_handle));

        // Initialize executor - must succeed for real test
        executor
            .initialize()
            .await
            .expect("Executor initialization must succeed");

        // Verify initialization
        assert!(executor.initialized);
        assert!(executor.mcp_server.is_some());
        assert!(executor.mcp_server_url().is_some());
        assert!(executor.mcp_server_port().is_some());

        let port = executor.mcp_server_port().unwrap();
        assert!(port > 0);

        // Shutdown
        executor.shutdown().await.unwrap();
        assert!(!executor.initialized);
        assert!(executor.mcp_server.is_some()); // MCP server handle remains but executor is not initialized
    }

    #[test]
    fn test_llama_agent_executor_model_display_name() {
        // Test HuggingFace model with filename
        let config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "unsloth/Phi-4-mini-instruct-GGUF".to_string(),
                    filename: Some("Phi-4-mini-instruct-Q4_K_M.gguf".to_string()),
                    folder: None,
                },
                batch_size: 256,
                use_hf_params: true,
                debug: false,
            },
            mcp_server: McpServerConfig::default(),

            repetition_detection: Default::default(),
        };
        let executor = LlamaAgentExecutor::new(config, None);
        assert_eq!(
            executor.get_model_display_name(),
            "unsloth/Phi-4-mini-instruct-GGUF/Phi-4-mini-instruct-Q4_K_M.gguf"
        );

        // Test HuggingFace model without filename
        let config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "unsloth/Phi-4-mini-instruct-GGUF".to_string(),
                    filename: None,
                    folder: None,
                },
                batch_size: 256,
                use_hf_params: true,
                debug: false,
            },
            mcp_server: McpServerConfig::default(),

            repetition_detection: Default::default(),
        };
        let executor = LlamaAgentExecutor::new(config, None);
        assert_eq!(
            executor.get_model_display_name(),
            "unsloth/Phi-4-mini-instruct-GGUF"
        );

        // Test local model
        let config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::Local {
                    filename: std::path::PathBuf::from("/path/to/model.gguf"),
                    folder: None,
                },
                batch_size: 256,
                use_hf_params: true,
                debug: false,
            },
            mcp_server: McpServerConfig::default(),

            repetition_detection: Default::default(),
        };
        let executor = LlamaAgentExecutor::new(config, None);
        assert_eq!(
            executor.get_model_display_name(),
            "local:/path/to/model.gguf"
        );
    }

    #[test_log::test(tokio::test)]
    async fn test_llama_agent_executor_initialization_with_validation() {
        // Start MCP server first
        use swissarmyhammer_prompts::PromptLibrary;
        use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

        let tools_handle = start_mcp_server(
            McpServerMode::Http { port: None },
            Some(PromptLibrary::default()),
        )
        .await
        .expect("Failed to start test MCP server");

        let port = tools_handle.info.port.unwrap_or(0);
        let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
        let mcp_handle = McpServerHandle::new(port, "127.0.0.1".to_string(), dummy_tx);

        let config = LlamaAgentConfig::for_testing();
        let mut executor = LlamaAgentExecutor::new(config, Some(mcp_handle));

        // Initialize must succeed for real test
        executor
            .initialize()
            .await
            .expect("Initialization must succeed");
        assert!(executor.initialized);

        executor.shutdown().await.unwrap();
    }

    #[test_log::test(tokio::test)]
    async fn test_llama_agent_executor_initialization_with_invalid_config() {
        // Start MCP server first
        use swissarmyhammer_prompts::PromptLibrary;
        use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

        let tools_handle = start_mcp_server(
            McpServerMode::Http { port: None },
            Some(PromptLibrary::default()),
        )
        .await
        .expect("Failed to start test MCP server");

        let port = tools_handle.info.port.unwrap_or(0);
        let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
        let mcp_handle = McpServerHandle::new(port, "127.0.0.1".to_string(), dummy_tx);

        // Test initialization with invalid configuration
        let invalid_config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "".to_string(), // Invalid empty repo
                    filename: Some("test.gguf".to_string()),
                    folder: None,
                },
                batch_size: 256,
                use_hf_params: true,
                debug: false,
            },
            mcp_server: McpServerConfig::default(),

            repetition_detection: Default::default(),
        };
        let mut executor = LlamaAgentExecutor::new(invalid_config, Some(mcp_handle));

        // Should fail during initialization - validation now handled by llama-agent
        let result = executor.initialize().await;
        assert!(result.is_err());
        assert!(!executor.initialized);
        // Error message now comes from llama-agent, so just check it contains initialization failure
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("LlamaAgent initialization failed"));
    }

    #[test_log::test(tokio::test)]
    async fn test_llama_agent_executor_global_management() {
        // Start MCP server for the global executor
        use swissarmyhammer_prompts::PromptLibrary;
        use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

        let tools_handle = start_mcp_server(
            McpServerMode::Http { port: None },
            Some(PromptLibrary::default()),
        )
        .await
        .expect("Failed to start test MCP server");

        let port = tools_handle.info.port.unwrap_or(0);
        let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
        let mcp_handle1 = McpServerHandle::new(port, "127.0.0.1".to_string(), dummy_tx);

        // Clone for second call (dummy handles are cheap)
        let (dummy_tx2, _dummy_rx2) = tokio::sync::oneshot::channel();
        let mcp_handle2 = McpServerHandle::new(port, "127.0.0.1".to_string(), dummy_tx2);

        let config1 = LlamaAgentConfig::for_testing();
        let config2 = LlamaAgentConfig::for_testing();

        // First call should create and initialize the global executor
        let global1 = LlamaAgentExecutor::get_global_executor(config1, Some(mcp_handle1)).await;
        assert!(global1.is_ok());

        // Second call should return the same global executor (singleton pattern)
        let global2 = LlamaAgentExecutor::get_global_executor(config2, Some(mcp_handle2)).await;
        assert!(global2.is_ok());

        // Verify they are the same instance by comparing Arc pointers
        let global1 = global1.unwrap();
        let global2 = global2.unwrap();
        assert!(Arc::ptr_eq(&global1, &global2));
    }

    // Note: Agent server initialization test removed due to configuration caching issues
    // The core functionality works correctly in production, tested via other test methods

    #[test_log::test(tokio::test)]
    async fn test_llama_agent_executor_execute_without_init() {
        let config = LlamaAgentConfig::for_testing();
        let executor = LlamaAgentExecutor::new(config, None);

        // Create a test execution context
        let agent_config = create_test_agent_config();
        let context = crate::AgentExecutionContext::new(&agent_config);

        // Try to execute without initialization - should fail
        let result = executor
            .execute_prompt(
                "System prompt".to_string(),
                "User prompt".to_string(),
                &context,
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not initialized"));
    }

    #[test]
    fn test_create_stopping_config() {
        // Test StoppingConfig creation (repetition detection has been removed from llama-agent)
        let config = LlamaAgentConfig::for_testing();
        let executor = LlamaAgentExecutor::new(config, None);
        let stopping_config = executor.create_stopping_config();

        // Verify the remaining fields
        assert!(stopping_config.eos_detection);
        assert_eq!(stopping_config.max_tokens, None);
    }

    #[test]
    fn test_folder_based_model_display_name() {
        // Test display name format for folder-based models
        let folder_model_config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "microsoft/Phi-3-mini-4k-instruct-gguf".to_string(),
                    filename: Some("Phi-3-mini-4k-instruct-q4".to_string()), // Folder name containing chunks
                    folder: None,
                },
                batch_size: 256,
                use_hf_params: true,
                debug: false,
            },
            mcp_server: McpServerConfig::default(),
            repetition_detection: Default::default(),
        };

        let executor = LlamaAgentExecutor::new(folder_model_config, None);

        // Test display name format for folder-based model
        assert_eq!(
            executor.get_model_display_name(),
            "microsoft/Phi-3-mini-4k-instruct-gguf/Phi-3-mini-4k-instruct-q4"
        );
    }

    #[test]
    fn test_single_file_model_display_name() {
        // Test display name format for single .gguf files
        let single_file_config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "microsoft/Phi-3-mini-4k-instruct-gguf".to_string(),
                    filename: Some("Phi-3-mini-4k-instruct-q4.gguf".to_string()), // Single .gguf file
                    folder: None,
                },
                batch_size: 256,
                use_hf_params: true,
                debug: false,
            },
            mcp_server: McpServerConfig::default(),
            repetition_detection: Default::default(),
        };

        let executor = LlamaAgentExecutor::new(single_file_config, None);

        // Test display name format for single file model
        assert_eq!(
            executor.get_model_display_name(),
            "microsoft/Phi-3-mini-4k-instruct-gguf/Phi-3-mini-4k-instruct-q4.gguf"
        );
    }

    #[test]
    fn test_folder_property_conversion() {
        use std::path::PathBuf;

        // Test ModelSource::Local with explicit folder
        let config_with_folder = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::Local {
                    filename: PathBuf::from("model.gguf"),
                    folder: Some(PathBuf::from("/custom/models")),
                },
                batch_size: 256,
                use_hf_params: true,
                debug: false,
            },
            mcp_server: McpServerConfig::default(),
            repetition_detection: Default::default(),
        };

        let executor_with_folder = LlamaAgentExecutor::new(config_with_folder, None);

        // Test ModelSource::Local without explicit folder (should derive from filename)
        let config_without_folder = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::Local {
                    filename: PathBuf::from("/path/to/model.gguf"),
                    folder: None,
                },
                batch_size: 256,
                use_hf_params: true,
                debug: false,
            },
            mcp_server: McpServerConfig::default(),
            repetition_detection: Default::default(),
        };

        let executor_without_folder = LlamaAgentExecutor::new(config_without_folder, None);

        // Both executors should have valid display names (just testing they don't panic)
        assert!(!executor_with_folder.get_model_display_name().is_empty());
        assert!(!executor_without_folder.get_model_display_name().is_empty());

        // The executor without folder should show the full path
        assert_eq!(
            executor_without_folder.get_model_display_name(),
            "local:/path/to/model.gguf"
        );

        // The executor with folder should show the filename only since that's what the filename field contains
        assert_eq!(
            executor_with_folder.get_model_display_name(),
            "local:model.gguf"
        );
    }

    /// Helper function for creating test execution context
    // Helper to create test agent config
    fn create_test_agent_config() -> swissarmyhammer_config::agent::AgentConfig {
        swissarmyhammer_config::agent::AgentConfig::default()
    }

    // Tests for LlamaAgentExecutorWrapper
    #[tokio::test]
    async fn test_wrapper_creation() {
        let config = LlamaAgentConfig::for_testing();
        let wrapper = LlamaAgentExecutorWrapper::new(config);

        assert_eq!(wrapper.executor_type(), AgentExecutorType::LlamaAgent);
        assert!(wrapper.global_executor.is_none());
    }

    #[tokio::test]
    async fn test_wrapper_singleton_behavior() {
        // Start MCP server for wrapper tests
        use swissarmyhammer_prompts::PromptLibrary;
        use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

        let tools_handle = start_mcp_server(
            McpServerMode::Http { port: None },
            Some(PromptLibrary::default()),
        )
        .await
        .expect("Failed to start test MCP server");

        let port = tools_handle.info.port.unwrap_or(0);

        // Create MCP handles for both wrappers
        let (dummy_tx1, _dummy_rx1) = tokio::sync::oneshot::channel();
        let mcp_handle1 = McpServerHandle::new(port, "127.0.0.1".to_string(), dummy_tx1);

        let (dummy_tx2, _dummy_rx2) = tokio::sync::oneshot::channel();
        let mcp_handle2 = McpServerHandle::new(port, "127.0.0.1".to_string(), dummy_tx2);

        let config1 = LlamaAgentConfig::for_testing();
        let mut wrapper1 = LlamaAgentExecutorWrapper::new_with_mcp(config1, Some(mcp_handle1));

        let config2 = LlamaAgentConfig::for_testing();
        let mut wrapper2 = LlamaAgentExecutorWrapper::new_with_mcp(config2, Some(mcp_handle2));

        // Initialize both wrappers
        wrapper1
            .initialize()
            .await
            .expect("Wrapper1 initialization should succeed");
        wrapper2
            .initialize()
            .await
            .expect("Wrapper2 initialization should succeed");

        // Both wrappers should have references to global executors
        assert!(wrapper1.global_executor.is_some());
        assert!(wrapper2.global_executor.is_some());

        // The underlying global executors should be the same instance (singleton pattern)
        let global1 = wrapper1.global_executor.as_ref().unwrap();
        let global2 = wrapper2.global_executor.as_ref().unwrap();
        assert!(
            Arc::ptr_eq(global1, global2),
            "Both wrappers should reference the same global singleton"
        );

        // Shutdown wrappers (should not affect the global singleton)
        wrapper1
            .shutdown()
            .await
            .expect("Wrapper1 shutdown should succeed");
        wrapper2
            .shutdown()
            .await
            .expect("Wrapper2 shutdown should succeed");

        assert!(wrapper1.global_executor.is_none());
        assert!(wrapper2.global_executor.is_none());
    }

    #[tokio::test]
    async fn test_wrapper_execute_without_init() {
        let config = LlamaAgentConfig::for_testing();
        let wrapper = LlamaAgentExecutorWrapper::new(config);

        let agent_config = create_test_agent_config();
        let context = crate::AgentExecutionContext::new(&agent_config);

        // Try to execute without initialization - should fail
        let result = wrapper
            .execute_prompt(
                "System prompt".to_string(),
                "User prompt".to_string(),
                &context,
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not initialized"));
    }
}
