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
    chat_template: Arc<ChatTemplateEngine>,
    dependency_analyzer: Arc<DependencyAnalyzer>,
    config: AgentConfig,
    start_time: Instant,
    shutdown_token: tokio_util::sync::CancellationToken,
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
            chat_template,
            dependency_analyzer,
            config,
            start_time: Instant::now(),
            shutdown_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    pub fn mcp_client(&self) -> &dyn MCPClient {
        self.mcp_client.as_ref()
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

        // Wait for queue to drain naturally
        info!("Waiting for request queue to drain...");
        let queue_stats = self.request_queue.get_stats();
        if queue_stats.current_queue_size > 0 {
            info!(
                "Draining queue with {} pending requests - allowing natural completion",
                queue_stats.current_queue_size
            );
            // Note: Queue will drain naturally as part of its own shutdown process
            // No artificial timeout imposed - operations complete when ready
        } else {
            info!("Request queue is already empty - no draining required");
        }

        // Complete shutdown process
        info!("Finalizing shutdown of remaining components...");

        let shutdown_duration = shutdown_start.elapsed();
        info!(
            "AgentServer shutdown completed gracefully in {:?}",
            shutdown_duration
        );
        info!(
            "Final statistics: {} requests completed, {} failed, {} total processed",
            queue_stats.completed_requests,
            queue_stats.failed_requests,
            queue_stats.completed_requests + queue_stats.failed_requests
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

        // Execute the tool call through MCP client with error handling
        debug!(
            "Calling MCP server '{}' for tool '{}'",
            tool_def.server_name, tool_call.name
        );
        match self
            .mcp_client
            .call_tool(&tool_call.name, tool_call.arguments.clone())
            .await
        {
            Ok(result_value) => {
                debug!("Tool call '{}' completed successfully", tool_call.name);
                debug!("Tool call result: {}", result_value);
                Ok(ToolResult {
                    call_id: tool_call.id,
                    result: serde_json::Value::String(result_value),
                    error: None,
                })
            }
            Err(mcp_error) => {
                let error_msg = format!("Tool execution failed: {}", mcp_error);
                error!("Tool call '{}' failed: {}", tool_call.name, error_msg);
                debug!("Failed tool call arguments were: {}", tool_call.arguments);

                // Return ToolResult with error instead of propagating the error
                // This allows the workflow to continue with partial failures
                Ok(ToolResult {
                    call_id: tool_call.id,
                    result: serde_json::Value::Null,
                    error: Some(error_msg),
                })
            }
        }
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
}

impl AgentServer {
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
                    mcp_servers: Vec::new(),
                    available_tools: Vec::new(),
                    available_prompts: Vec::new(),
                    created_at: SystemTime::now(),
                    updated_at: SystemTime::now(),
                    compaction_history: Vec::new(),
                    transcript_path: None,
                    context_state: None,
                    template_token_count: None,
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
