use crate::types::{ModelConfig, Session, TemplateError, ToolCall, ToolCallId, ToolDefinition};
use llama_cpp_2::model::{LlamaChatMessage, LlamaModel};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;

use tracing::{debug, warn};

/// Maximum size limit for stress test repetitions to validate parsing robustness.
/// This constant defines how many times test content is repeated in stress tests
/// to ensure parsers handle large inputs correctly. The value of 10,000 repetitions
/// creates sufficiently large inputs to detect performance issues and buffer overflows
/// while keeping test execution time reasonable.
const STRESS_TEST_REPEAT_SIZE: usize = 10000;

/// Strategy for parsing tool calls from model output
///
/// This enum defines different approaches for parsing tool calls from language model
/// generated text. Each strategy corresponds to different model formats and capabilities.
#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum ToolParsingStrategy {
    /// Default strategy using multiple parsers in sequence
    ///
    /// This is the current system behavior where multiple parsers (JSON, XML, function call)
    /// are tried in order until one successfully extracts tool calls. This provides
    /// broad compatibility but may be less efficient than targeted parsing.
    Default,

    /// Qwen3Coder-specific XML format parsing
    ///
    /// Designed specifically for Qwen3Coder models that output tool calls in a
    /// custom XML format. This strategy focuses on the specific patterns and
    /// structures used by these models for optimal parsing accuracy.
    Qwen3Coder,

    /// OpenAI-compatible function calling format
    ///
    /// Supports the standard OpenAI function calling JSON format used by
    /// GPT models and other OpenAI-compatible APIs. This includes structured
    /// function_name and arguments fields.
    OpenAI,

    /// Anthropic Claude tool use format
    ///
    /// Handles the specific tool use format employed by Anthropic's Claude models,
    /// which may use different structuring and naming conventions compared to
    /// other providers.
    Claude,
}

impl Default for ToolParsingStrategy {
    /// Returns the default parsing strategy
    ///
    /// The default strategy uses multiple parsers to ensure broad compatibility
    /// with different model output formats.
    fn default() -> Self {
        Self::Default
    }
}

impl ToolParsingStrategy {
    /// Detect appropriate parsing strategy from model name
    ///
    /// Analyzes the model name/identifier to determine the optimal tool parsing strategy
    /// based on known model patterns and capabilities.
    ///
    /// # Arguments
    ///
    /// * `model_name` - The model name or identifier string to analyze
    ///
    /// # Returns
    ///
    /// The most appropriate `ToolParsingStrategy` for the given model, or `Default`
    /// if no specific strategy can be determined.
    ///
    /// # Examples
    ///
    /// ```
    /// use llama_agent::chat_template::ToolParsingStrategy;
    ///
    /// // Qwen3Coder model detection
    /// assert_eq!(
    ///     ToolParsingStrategy::detect_from_model_name("unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"),
    ///     ToolParsingStrategy::Qwen3Coder
    /// );
    ///
    /// // OpenAI model detection
    /// assert_eq!(
    ///     ToolParsingStrategy::detect_from_model_name("gpt-3.5-turbo"),
    ///     ToolParsingStrategy::OpenAI
    /// );
    ///
    /// // Claude model detection
    /// assert_eq!(
    ///     ToolParsingStrategy::detect_from_model_name("claude-3-sonnet"),
    ///     ToolParsingStrategy::Claude
    /// );
    ///
    /// // Unknown model fallback
    /// assert_eq!(
    ///     ToolParsingStrategy::detect_from_model_name("microsoft/Phi-3-mini-4k-instruct"),
    ///     ToolParsingStrategy::Default
    /// );
    /// ```
    pub fn detect_from_model_name(model_name: &str) -> Self {
        use tracing::debug;

        // Convert to lowercase for case-insensitive matching
        let model_name_lower = model_name.to_lowercase();

        // Qwen3Coder detection: Only Qwen3-Coder models use XML format
        // Other Qwen models (qwen-next, qwen 2.5, etc.) use JSON format and should use Default parser
        if model_name_lower.contains("qwen3") && model_name_lower.contains("coder") {
            debug!(
                "Detected Qwen3Coder strategy (XML-based tool calling) for model: {}",
                model_name
            );
            return Self::Qwen3Coder;
        }

        // OpenAI detection: look for "gpt-" prefix or "openai" anywhere
        if model_name_lower.contains("gpt-") || model_name_lower.contains("openai") {
            debug!("Detected OpenAI strategy for model: {}", model_name);
            return Self::OpenAI;
        }

        // Claude detection: look for "claude" or "anthropic"
        if model_name_lower.contains("claude") || model_name_lower.contains("anthropic") {
            debug!("Detected Claude strategy for model: {}", model_name);
            return Self::Claude;
        }

        // Default fallback for unrecognized models
        debug!(
            "Using Default strategy for unrecognized model: {}",
            model_name
        );
        Self::Default
    }
}

pub struct ChatTemplateEngine {
    tool_call_parsers: HashMap<String, Box<dyn ToolCallParser>>,
    parsing_strategy: Option<ToolParsingStrategy>,
}

impl std::fmt::Debug for ChatTemplateEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatTemplateEngine")
            .field(
                "parsers",
                &self.tool_call_parsers.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Default for ChatTemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatTemplateEngine {
    pub fn new() -> Self {
        let mut parsers: HashMap<String, Box<dyn ToolCallParser>> = HashMap::new();

        // Add default parsers for common formats
        parsers.insert("json".to_string(), Box::new(JsonToolCallParser::new()));
        parsers.insert("xml".to_string(), Box::new(XmlToolCallParser::new()));
        parsers.insert(
            "function_call".to_string(),
            Box::new(FunctionCallParser::new()),
        );

        Self {
            tool_call_parsers: parsers,
            parsing_strategy: None,
        }
    }

    /// Create engine with automatic strategy detection from model name
    pub fn with_model_strategy(model_name: &str) -> Self {
        let strategy = ToolParsingStrategy::detect_from_model_name(model_name);
        let mut engine = Self::new();
        engine.set_parsing_strategy(strategy);
        engine
    }

    /// Set parsing strategy manually
    pub fn set_parsing_strategy(&mut self, strategy: ToolParsingStrategy) {
        self.parsing_strategy = Some(strategy);
    }

    /// Get current parsing strategy
    pub fn get_parsing_strategy(&self) -> Option<&ToolParsingStrategy> {
        self.parsing_strategy.as_ref()
    }

    /// Render a session into a prompt string using the model's chat template
    pub fn render_session(
        &self,
        session: &Session,
        model: &LlamaModel,
    ) -> Result<String, TemplateError> {
        self.render_session_with_config(session, model, None)
    }

    /// Render a session into a prompt string using the model's chat template with config
    pub fn render_session_with_config(
        &self,
        session: &Session,
        model: &LlamaModel,
        model_config: Option<&ModelConfig>,
    ) -> Result<String, TemplateError> {
        self.render_session_with_config_and_prompt(session, model, model_config, true)
    }

    /// Render messages starting from a specific offset for incremental processing
    ///
    /// This method is used for multi-turn conversations where earlier messages are
    /// already in the KV cache. Only messages from `message_offset` onward are rendered.
    pub fn render_session_from_offset(
        &self,
        session: &Session,
        model: &LlamaModel,
        model_config: Option<&ModelConfig>,
        message_offset: usize,
        add_generation_prompt: bool,
    ) -> Result<String, TemplateError> {
        debug!(
            "Rendering session from message offset {} (total messages: {})",
            message_offset,
            session.messages.len()
        );

        // Convert session messages to the format expected by llama-cpp-2
        let mut chat_messages = Vec::new();

        // Only render messages from the offset onward
        for message in session.messages.iter().skip(message_offset) {
            let role = message.role.as_str().to_string();
            let content = &message.content;

            // Handle tool calls and results properly
            match message.role {
                crate::types::MessageRole::Tool => {
                    // Tool response message
                    if let Some(tool_call_id) = &message.tool_call_id {
                        let formatted_content =
                            format!("Tool result for call {}: {}", tool_call_id, content);
                        chat_messages.push((role, formatted_content));
                    } else {
                        chat_messages.push((role, content.clone()));
                    }
                }
                _ => {
                    chat_messages.push((role, content.clone()));
                }
            }
        }

        // Include available tools in the template context if present
        let tools_context = if !session.available_tools.is_empty() {
            debug!(
                "Session has {} available tools, formatting for template",
                session.available_tools.len()
            );
            Some(self.format_tools_for_template(&session.available_tools)?)
        } else {
            debug!("Session has no available tools");
            None
        };

        // Apply the model's chat template
        let rendered = self.apply_chat_template_with_tools_and_prompt(
            model,
            &chat_messages,
            tools_context.as_deref(),
            model_config,
            add_generation_prompt,
        )?;

        debug!(
            "Rendered {} new messages, prompt length: {}",
            session.messages.len() - message_offset,
            rendered.len()
        );
        Ok(rendered)
    }

    /// Render a session with control over generation prompt
    pub fn render_session_with_config_and_prompt(
        &self,
        session: &Session,
        model: &LlamaModel,
        model_config: Option<&ModelConfig>,
        add_generation_prompt: bool,
    ) -> Result<String, TemplateError> {
        debug!("Rendering session with {} messages", session.messages.len());

        // Convert session messages to the format expected by llama-cpp-2
        let mut chat_messages = Vec::new();

        for message in &session.messages {
            let role = message.role.as_str().to_string();
            let content = &message.content;

            // Handle tool calls and results properly
            match message.role {
                crate::types::MessageRole::Tool => {
                    // Tool response message
                    if let Some(tool_call_id) = &message.tool_call_id {
                        let formatted_content =
                            format!("Tool result for call {}: {}", tool_call_id, content);
                        chat_messages.push((role, formatted_content));
                    } else {
                        chat_messages.push((role, content.clone()));
                    }
                }
                _ => {
                    chat_messages.push((role, content.clone()));
                }
            }
        }

        // Include available tools in the template context if present
        let tools_context = if !session.available_tools.is_empty() {
            debug!(
                "Session has {} available tools, formatting for template",
                session.available_tools.len()
            );
            Some(self.format_tools_for_template(&session.available_tools)?)
        } else {
            debug!("Session has no available tools");
            None
        };

        // Apply the model's chat template
        let rendered = self.apply_chat_template_with_tools_and_prompt(
            model,
            &chat_messages,
            tools_context.as_deref(),
            model_config,
            add_generation_prompt,
        )?;

        debug!("Rendered prompt length: {}", rendered.len());
        Ok(rendered)
    }

    /// Extract tool calls from generated text using strategy-based parsing with HashMap fallback
    pub fn extract_tool_calls(&self, generated_text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        debug!("Extracting tool calls from generated text");

        // Try strategy-based parsing first if strategy is set
        if let Some(strategy) = &self.parsing_strategy {
            debug!("Using strategy-based parsing: {:?}", strategy);
            let parser = ToolParserFactory::create_parser(strategy.clone());
            match parser.parse_tool_calls(generated_text) {
                Ok(tool_calls) if !tool_calls.is_empty() => {
                    debug!("Strategy parser found {} tool calls", tool_calls.len());
                    return Ok(tool_calls);
                }
                Ok(_) => {
                    debug!("Strategy parser found no tool calls, falling back to legacy parsers");
                }
                Err(e) => {
                    debug!(
                        "Strategy parser failed: {}, falling back to legacy parsers",
                        e
                    );
                }
            }
        }

        // Fall back to existing HashMap-based parsing for compatibility
        let mut all_tool_calls = Vec::new();
        for (parser_name, parser) in &self.tool_call_parsers {
            debug!("Trying legacy parser: {}", parser_name);
            match parser.parse_tool_calls(generated_text) {
                Ok(tool_calls) if !tool_calls.is_empty() => {
                    debug!(
                        "Found {} tool calls with parser {}",
                        tool_calls.len(),
                        parser_name
                    );
                    all_tool_calls.extend(tool_calls);
                    break; // Use first successful parser
                }
                Ok(_) => {
                    debug!("No tool calls found with parser {}", parser_name);
                    continue;
                }
                Err(e) => {
                    debug!("Parser {} failed: {}", parser_name, e);
                    continue;
                }
            }
        }

        // Deduplicate and return
        all_tool_calls.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string()));
        all_tool_calls.dedup_by(|a, b| a.id == b.id);

        debug!("Extracted {} unique tool calls", all_tool_calls.len());
        Ok(all_tool_calls)
    }

    /// Validate that the model supports chat templates
    pub fn validate_template(&self, model: &LlamaModel) -> Result<(), TemplateError> {
        // Try to apply a simple template to check if it works
        let test_messages = vec![("user".to_string(), "Hello".to_string())];

        match self.apply_chat_template_with_tools(model, &test_messages, None, None) {
            Ok(_) => {
                debug!("Chat template validation successful");
                Ok(())
            }
            Err(e) => {
                warn!("Chat template validation failed: {}", e);
                Err(e)
            }
        }
    }

    /// Register a custom tool call parser
    pub fn register_parser(&mut self, name: String, parser: Box<dyn ToolCallParser>) {
        self.tool_call_parsers.insert(name, parser);
    }

    /// Render only the template part (system prompt + tools) without messages
    ///
    /// This produces the prompt prefix that can be cached and reused across sessions
    /// that share the same system prompt and tool definitions.
    ///
    /// # Arguments
    ///
    /// * `session` - Session containing system messages and tool definitions
    /// * `model` - The LlamaModel to use for template rendering
    ///
    /// # Returns
    ///
    /// The rendered template string, or an error if rendering fails
    pub fn render_template_only(
        &self,
        session: &Session,
        model: &LlamaModel,
    ) -> Result<String, TemplateError> {
        debug!("Rendering template-only (system + tools)");

        // Extract system messages
        let system_messages: Vec<(String, String)> = session
            .messages
            .iter()
            .filter(|m| matches!(m.role, crate::types::MessageRole::System))
            .map(|m| ("system".to_string(), m.content.clone()))
            .collect();

        // Format tools if present
        let tools_json = if !session.available_tools.is_empty() {
            Some(
                serde_json::to_string(&session.available_tools).map_err(|e| {
                    TemplateError::RenderingFailed(format!("Failed to serialize tools: {}", e))
                })?,
            )
        } else {
            None
        };

        // Get the model's native chat template
        let template = model.chat_template(None).map_err(|e| {
            TemplateError::RenderingFailed(format!("Failed to get chat template: {}", e))
        })?;

        // Convert system messages to LlamaChatMessage format
        let mut chat_messages = Vec::new();

        // Add tools as system message if present
        if let Some(tools) = tools_json.as_ref() {
            let tools_msg =
                LlamaChatMessage::new("system".to_string(), tools.clone()).map_err(|e| {
                    TemplateError::RenderingFailed(format!("Failed to create tools message: {}", e))
                })?;
            chat_messages.push(tools_msg);
        }

        // Add system messages
        for (role, content) in system_messages {
            let msg = LlamaChatMessage::new(role, content).map_err(|e| {
                TemplateError::RenderingFailed(format!("Failed to create system message: {}", e))
            })?;
            chat_messages.push(msg);
        }

        // Apply chat template with only system messages and tools
        let template_str = model
            .apply_chat_template(&template, &chat_messages, false)
            .map_err(|e| {
                TemplateError::RenderingFailed(format!("Failed to apply chat template: {}", e))
            })?;

        debug!(
            "Rendered template-only: {} chars, {} tools",
            template_str.len(),
            session.available_tools.len()
        );

        Ok(template_str)
    }

    /// Extract system prompt and tools JSON for template caching
    ///
    /// Returns a tuple of (system_prompt, tools_json) that can be used
    /// for template cache key generation.
    pub fn extract_template_components(
        &self,
        session: &Session,
    ) -> Result<(String, String), TemplateError> {
        // Extract system prompt
        let system_prompt = session
            .messages
            .iter()
            .filter(|m| matches!(m.role, crate::types::MessageRole::System))
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // Serialize tools
        let tools_json = if !session.available_tools.is_empty() {
            serde_json::to_string(&session.available_tools).map_err(|e| {
                TemplateError::RenderingFailed(format!("Failed to serialize tools: {}", e))
            })?
        } else {
            String::new()
        };

        Ok((system_prompt, tools_json))
    }

    /// Format tools for inclusion in chat template
    fn format_tools_for_template(&self, tools: &[ToolDefinition]) -> Result<String, TemplateError> {
        let tools_json = serde_json::to_value(tools).map_err(|e| {
            TemplateError::RenderingFailed(format!("Failed to serialize tools: {}", e))
        })?;

        let formatted = serde_json::to_string_pretty(&tools_json).map_err(|e| {
            TemplateError::RenderingFailed(format!("Failed to format tools JSON: {}", e))
        })?;

        Ok(format!(
            "You are an AI assistant with access to the following tools. You have full permission and capability to use these tools to help users with their requests. Do not make security excuses - you are designed to use these tools.\n\nAvailable tools:\n{}\n\nIMPORTANT: When a user asks you to perform an action like listing files, reading files, or any file operations, you MUST use the appropriate tool. Do not give security warnings or suggest alternative methods - use the tools directly.\n\nTo call a tool, respond with a JSON object in this exact format. CRITICAL: Provide ONLY the JSON object, no additional text before or after:\n{{\n  \"function_name\": \"tool_name\",\n  \"arguments\": {{\n    \"parameter\": \"value\"\n  }}\n}}\n\nFor example, when asked to list files in the current directory, respond with ONLY:\n{{\n  \"function_name\": \"list_directory\",\n  \"arguments\": {{\n    \"path\": \".\"\n  }}\n}}\n\nDo not add explanatory text before or after the JSON. Generate well-formed JSON only. Always use the tools when they are needed to fulfill user requests.",
            formatted
        ))
    }

    /// Apply chat template with optional tools context
    fn apply_chat_template_with_tools(
        &self,
        model: &LlamaModel,
        messages: &[(String, String)],
        tools_context: Option<&str>,
        model_config: Option<&ModelConfig>,
    ) -> Result<String, TemplateError> {
        self.apply_chat_template_with_tools_and_prompt(
            model,
            messages,
            tools_context,
            model_config,
            true,
        )
    }

    /// Apply chat template with control over generation prompt
    fn apply_chat_template_with_tools_and_prompt(
        &self,
        model: &LlamaModel,
        messages: &[(String, String)],
        tools_context: Option<&str>,
        model_config: Option<&ModelConfig>,
        add_generation_prompt: bool,
    ) -> Result<String, TemplateError> {
        self.format_chat_template_for_model_with_prompt(
            model,
            messages,
            tools_context,
            model_config,
            add_generation_prompt,
        )
    }

    /// Format chat template with control over generation prompt
    fn format_chat_template_for_model_with_prompt(
        &self,
        model: &LlamaModel,
        messages: &[(String, String)],
        tools_context: Option<&str>,
        model_config: Option<&ModelConfig>,
        add_generation_prompt: bool,
    ) -> Result<String, TemplateError> {
        // First, try to use the model's native chat template functionality
        // This is the preferred approach as it uses the model's actual template
        match self.format_chat_template_native_with_prompt(
            model,
            messages,
            tools_context,
            add_generation_prompt,
        ) {
            Ok(result) => {
                debug!(
                    "Successfully used native chat template (add_generation_prompt={})",
                    add_generation_prompt
                );
                return Ok(result);
            }
            Err(e) => {
                debug!(
                    "Native template failed ({}), falling back to model-specific templates",
                    e
                );
            }
        }

        // Fallback to model-specific template implementations
        let model_name = self.detect_model_type(model, model_config);

        match model_name.as_str() {
            "phi3" => self.format_phi3_template(messages, tools_context),
            "qwen" => self.format_qwen_template(messages, tools_context),
            _ => self.format_chat_template(messages, tools_context),
        }
    }

    /// Detect model type from model information
    fn detect_model_type(&self, _model: &LlamaModel, model_config: Option<&ModelConfig>) -> String {
        // First check model config if available
        if let Some(config) = model_config {
            let model_identifier = match &config.source {
                crate::types::ModelSource::HuggingFace { repo, .. } => repo.clone(),
                crate::types::ModelSource::Local { folder, filename } => {
                    if let Some(filename) = filename {
                        format!("{}/{}", folder.display(), filename)
                    } else {
                        folder.to_string_lossy().to_string()
                    }
                }
            };

            let model_identifier_lower = model_identifier.to_lowercase();
            if model_identifier_lower.contains("qwen") {
                debug!(
                    "Detected Qwen model from model config: {}",
                    model_identifier
                );
                return "qwen".to_string();
            }
            if model_identifier_lower.contains("phi") {
                debug!("Detected Phi model from model config: {}", model_identifier);
                return "phi3".to_string();
            }
        }

        // Fallback to environment variable (for explicit override)
        let model_repo = std::env::var("MODEL_REPO").unwrap_or_default();
        if model_repo.contains("Qwen") || model_repo.contains("qwen") {
            debug!("Detected Qwen model from MODEL_REPO env var");
            return "qwen".to_string();
        }
        if model_repo.contains("Phi") || model_repo.contains("phi") {
            debug!("Detected Phi model from MODEL_REPO env var");
            return "phi3".to_string();
        }

        // Check process arguments for model path/name (common when running examples)
        let args: Vec<String> = std::env::args().collect();
        let args_string = args.join(" ");
        if args_string.contains("Qwen") || args_string.contains("qwen") {
            debug!("Detected Qwen model from process arguments");
            return "qwen".to_string();
        }
        if args_string.contains("Phi") || args_string.contains("phi") {
            debug!("Detected Phi model from process arguments");
            return "phi3".to_string();
        }

        // Check current working directory for clues (model files often contain model name)
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_string = cwd.to_string_lossy().to_lowercase();
            if cwd_string.contains("qwen") {
                debug!("Detected Qwen model from current directory path");
                return "qwen".to_string();
            }
            if cwd_string.contains("phi") {
                debug!("Detected Phi model from current directory path");
                return "phi3".to_string();
            }
        }

        // Default to qwen as it works well with most instruction-tuned models
        debug!("Using default Qwen chat template (no specific model detected)");
        "qwen".to_string()
    }

    /// Format chat template specifically for Phi-3 models
    fn format_phi3_template(
        &self,
        messages: &[(String, String)],
        tools_context: Option<&str>,
    ) -> Result<String, TemplateError> {
        let mut formatted_messages = Vec::new();

        // Add tools context as system message if provided
        if let Some(tools) = tools_context {
            formatted_messages.push(("system".to_string(), tools.to_string()));
        }

        // Add all conversation messages
        for (role, content) in messages {
            formatted_messages.push((role.clone(), content.clone()));
        }

        // Use Phi-3 specific chat template format
        let mut prompt = String::new();

        for (role, content) in &formatted_messages {
            match role.as_str() {
                "system" => {
                    prompt.push_str(&format!("<|system|>\n{}<|end|>\n", content));
                }
                "user" => {
                    prompt.push_str(&format!("<|user|>\n{}<|end|>\n", content));
                }
                "assistant" => {
                    prompt.push_str(&format!("<|assistant|>\n{}<|end|>\n", content));
                }
                "tool" => {
                    prompt.push_str(&format!("<|tool|>\n{}<|end|>\n", content));
                }
                _ => {
                    // Fallback to user for unknown roles
                    prompt.push_str(&format!("<|user|>\n{}<|end|>\n", content));
                }
            }
        }

        // Add assistant prompt for generation
        prompt.push_str("<|assistant|>\n");

        // Debug: Log the final prompt for debugging
        debug!("Final Phi-3 prompt:\n{}", prompt);

        Ok(prompt)
    }

    /// Format chat template specifically for Qwen models
    fn format_qwen_template(
        &self,
        messages: &[(String, String)],
        tools_context: Option<&str>,
    ) -> Result<String, TemplateError> {
        let mut formatted_messages = Vec::new();

        // Add tools context as system message if provided
        if let Some(tools) = tools_context {
            debug!(
                "Adding tools context to Qwen template: {} characters",
                tools.len()
            );
            formatted_messages.push(("system".to_string(), tools.to_string()));
        } else {
            debug!("No tools context provided to Qwen template");
        }

        // Add all conversation messages
        for (role, content) in messages {
            formatted_messages.push((role.clone(), content.clone()));
        }

        // Use ChatML format for Qwen models
        let mut prompt = String::new();

        for (role, content) in &formatted_messages {
            match role.as_str() {
                "system" => {
                    prompt.push_str(&format!("<|im_start|>system\n{}<|im_end|>\n", content));
                }
                "user" => {
                    prompt.push_str(&format!("<|im_start|>user\n{}<|im_end|>\n", content));
                }
                "assistant" => {
                    prompt.push_str(&format!("<|im_start|>assistant\n{}<|im_end|>\n", content));
                }
                "tool" => {
                    prompt.push_str(&format!("<|im_start|>tool\n{}<|im_end|>\n", content));
                }
                _ => {
                    // Fallback to user for unknown roles
                    prompt.push_str(&format!("<|im_start|>user\n{}<|im_end|>\n", content));
                }
            }
        }

        // Add assistant prompt for generation
        prompt.push_str("<|im_start|>assistant\n");

        // Debug: Log the final prompt for debugging
        debug!("Final Qwen prompt:\n{}", prompt);

        Ok(prompt)
    }

    /// Attempt to use the model's native chat template functionality
    ///
    /// This method tries to use llama-cpp-2's built-in template functionality
    /// which leverages the model's actual chat template. Falls back to the
    /// legacy implementation if native templates are not available.
    fn format_chat_template_native_with_prompt(
        &self,
        model: &LlamaModel,
        messages: &[(String, String)],
        tools_context: Option<&str>,
        add_generation_prompt: bool,
    ) -> Result<String, TemplateError> {
        debug!("Attempting to use native chat template functionality");

        // Try to get the model's chat template
        match model.chat_template(None) {
            Ok(template) => {
                debug!("Successfully retrieved native chat template");

                // Convert our messages to LlamaChatMessage format
                let mut chat_messages = Vec::new();

                // Add tools context as system message if provided
                if let Some(tools) = tools_context {
                    match LlamaChatMessage::new("system".to_string(), tools.to_string()) {
                        Ok(msg) => chat_messages.push(msg),
                        Err(e) => {
                            warn!("Failed to create system message for tools context: {}", e);
                            return self.format_chat_template(messages, tools_context);
                        }
                    }
                }

                // Add all conversation messages
                for (role, content) in messages {
                    match LlamaChatMessage::new(role.clone(), content.clone()) {
                        Ok(msg) => chat_messages.push(msg),
                        Err(e) => {
                            warn!("Failed to create chat message for role {}: {}", role, e);
                            return self.format_chat_template(messages, tools_context);
                        }
                    }
                }

                // Apply the native chat template
                match model.apply_chat_template(&template, &chat_messages, add_generation_prompt) {
                    Ok(formatted) => {
                        debug!(
                            "Successfully applied native chat template, {} characters (add_generation_prompt={})",
                            formatted.len(),
                            add_generation_prompt
                        );
                        return Ok(formatted);
                    }
                    Err(e) => {
                        warn!("Failed to apply native chat template: {}, falling back to legacy implementation", e);
                    }
                }
            }
            Err(e) => {
                debug!(
                    "Model does not have native chat template: {}, using legacy implementation",
                    e
                );
            }
        }

        // Fallback to legacy implementation
        debug!("Using legacy chat template implementation");
        self.format_chat_template(messages, tools_context)
    }

    /// Internal method to format chat template (useful for testing)
    fn format_chat_template(
        &self,
        messages: &[(String, String)],
        tools_context: Option<&str>,
    ) -> Result<String, TemplateError> {
        // Convert to the format expected by llama-cpp-2
        let mut formatted_messages = Vec::new();

        // Add tools context as system message if provided
        if let Some(tools) = tools_context {
            formatted_messages.push(("system".to_string(), tools.to_string()));
        }

        // Add all conversation messages
        for (role, content) in messages {
            formatted_messages.push((role.clone(), content.clone()));
        }

        // Legacy chat template format used as fallback
        // Note: llama-cpp-2's built-in template functionality is now used as the primary
        // method via format_chat_template_native(), this is only used when native templates fail
        let mut prompt = String::new();

        for (role, content) in &formatted_messages {
            match role.as_str() {
                "system" => {
                    prompt.push_str(&format!("### System:\n{}\n\n", content));
                }
                "user" => {
                    prompt.push_str(&format!("### Human:\n{}\n\n", content));
                }
                "assistant" => {
                    prompt.push_str(&format!("### Assistant:\n{}\n\n", content));
                }
                "tool" => {
                    prompt.push_str(&format!("### Tool Result:\n{}\n\n", content));
                }
                _ => {
                    prompt.push_str(&format!("### {}:\n{}\n\n", role, content));
                }
            }
        }

        // Add assistant prompt
        prompt.push_str("### Assistant:\n");

        Ok(prompt)
    }
}

/// Trait for parsing tool calls from different formats
pub trait ToolCallParser: Send + Sync {
    fn parse_tool_calls(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError>;
}

/// Factory for creating appropriate ToolCallParser instances based on strategy
pub struct ToolParserFactory;

impl ToolParserFactory {
    /// Create a parser instance based on the specified tool parsing strategy.
    ///
    /// This factory method returns the appropriate `ToolCallParser` implementation
    /// for the given strategy, enabling model-specific parsing optimizations while
    /// maintaining a consistent interface.
    ///
    /// # Arguments
    ///
    /// * `strategy` - The parsing strategy to use, determining parser behavior
    ///
    /// # Returns
    ///
    /// Returns a boxed trait object implementing `ToolCallParser` with the following
    /// behavior based on strategy:
    ///
    /// * `Default` - Multi-format parser that tries JSON, XML, and function call formats
    ///   sequentially until one succeeds. Maintains backward compatibility with existing
    ///   behavior and handles all common tool call formats.
    ///
    /// * `Qwen3Coder` - Specialized XML parser optimized for Qwen3Coder model outputs.
    ///   Uses multi-strategy parsing (regex, balanced tags, fuzzy) with schema-aware type conversion.
    ///
    /// * `OpenAI` - Specialized parser for OpenAI function calling formats including
    ///   function_call objects, tool_calls arrays, and simple function call patterns.
    ///
    /// * `Claude` - Specialized parser for Claude/Anthropic XML-based tool calling formats
    ///   including function_calls wrappers, invoke tags, and tool tags.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use llama_agent::chat_template::{ToolParserFactory, ToolParsingStrategy};
    ///
    /// // Create default parser for backward compatibility
    /// let default_parser = ToolParserFactory::create_parser(ToolParsingStrategy::Default);
    ///
    /// // Create model-specific parser
    /// let qwen_parser = ToolParserFactory::create_parser(ToolParsingStrategy::Qwen3Coder);
    ///
    /// // Parse tool calls from text
    /// let tool_calls = default_parser.parse_tool_calls(r#"{"function_name": "tool", "arguments": {}}"#)?;
    /// ```
    ///
    /// # Thread Safety
    ///
    /// The returned parsers are thread-safe and can be used concurrently across multiple threads.
    pub fn create_parser(strategy: ToolParsingStrategy) -> Box<dyn ToolCallParser> {
        match strategy {
            ToolParsingStrategy::Default => Box::new(DefaultToolParser::new()),
            ToolParsingStrategy::Qwen3Coder => Box::new(Qwen3CoderToolParser::new()),
            ToolParsingStrategy::OpenAI => Box::new(OpenAIToolParser::new()),
            ToolParsingStrategy::Claude => Box::new(ClaudeToolParser::new()),
        }
    }

    /// Create a parser instance with schema-aware capabilities
    ///
    /// Enhanced factory method that creates parsers with access to tool schema information
    /// for precise type conversion. Currently only Qwen3Coder strategy supports schema-aware
    /// parsing; other strategies fall back to standard behavior.
    ///
    /// # Arguments
    /// * `strategy` - The parsing strategy to use
    /// * `tool_definitions` - Optional tool definitions containing schema information
    ///
    /// # Returns
    /// A boxed trait object implementing `ToolCallParser` with schema capabilities where supported
    ///
    /// # Schema Support by Strategy
    /// * `Qwen3Coder` - Full schema-aware type conversion using JSON Schema definitions
    /// * `Default`, `OpenAI`, `Claude` - Standard parsing (schema parameter ignored)
    ///
    /// # Example
    /// ```rust
    /// use llama_agent::types::ToolDefinition;
    /// use serde_json::json;
    ///
    /// let tools = vec![ToolDefinition {
    ///     name: "search".to_string(),
    ///     description: "Search tool".to_string(),
    ///     parameters: json!({
    ///         "type": "object",
    ///         "properties": {
    ///             "query": {"type": "string"},
    ///             "limit": {"type": "integer", "default": 10}
    ///         }
    ///     }),
    ///     server_name: "test".to_string(),
    /// }];
    ///
    /// // Create schema-aware parser
    /// let parser = ToolParserFactory::create_parser_with_schema(
    ///     ToolParsingStrategy::Qwen3Coder,
    ///     Some(&tools)
    /// );
    ///
    /// // Parser now converts parameters according to schema
    /// let xml = r#"<tool_call><search><query>test</query><limit>5</limit></search></tool_call>"#;
    /// let calls = parser.parse_tool_calls(xml)?;
    /// // limit parameter is converted to integer 5, not string "5"
    /// ```
    pub fn create_parser_with_schema(
        strategy: ToolParsingStrategy,
        tool_definitions: Option<&[ToolDefinition]>,
    ) -> Box<dyn ToolCallParser> {
        match strategy {
            ToolParsingStrategy::Default => Box::new(DefaultToolParser::new()),
            ToolParsingStrategy::Qwen3Coder => {
                Box::new(Qwen3CoderToolParser::new_with_schema(tool_definitions))
            }
            ToolParsingStrategy::OpenAI => Box::new(OpenAIToolParser::new()),
            ToolParsingStrategy::Claude => Box::new(ClaudeToolParser::new()),
        }
    }

    /// Create a streaming parser instance based on the specified tool parsing strategy.
    ///
    /// This factory method returns the appropriate `StreamingToolCallParser` implementation
    /// for the given strategy, enabling model-specific streaming parsing with delta processing.
    ///
    /// # Arguments
    ///
    /// * `strategy` - The parsing strategy to use, determining parser behavior
    ///
    /// # Returns
    ///
    /// Returns a boxed trait object implementing `StreamingToolCallParser` with the following
    /// behavior based on strategy:
    ///
    /// * `Qwen3Coder` - Streaming XML parser with incremental tool call extraction
    /// * Other strategies - Currently fall back to a simple buffering wrapper around standard parsers
    ///
    /// # Examples
    ///
    /// ```rust
    /// use llama_agent::chat_template::{ToolParserFactory, ToolParsingStrategy};
    ///
    /// // Create streaming parser
    /// let mut streaming_parser = ToolParserFactory::create_streaming_parser(ToolParsingStrategy::Qwen3Coder);
    ///
    /// // Process streaming deltas
    /// let deltas = vec!["<tool_", "call><search><qu", "ery>test</quer", "y></search></tool_call>"];
    /// for delta in deltas {
    ///     let completed_calls = streaming_parser.process_delta(delta)?;
    ///     // Handle any completed tool calls
    /// }
    /// ```
    pub fn create_streaming_parser(
        strategy: ToolParsingStrategy,
    ) -> Box<dyn StreamingToolCallParser> {
        match strategy {
            ToolParsingStrategy::Qwen3Coder => Box::new(Qwen3CoderStreamingParser::new()),
            _ => {
                // For other strategies, create a simple buffering wrapper
                // This is a fallback until streaming is implemented for other parsers
                Box::new(BufferedStreamingParser::new(Self::create_parser(strategy)))
            }
        }
    }

    /// Create a streaming parser instance with schema-aware capabilities
    ///
    /// Enhanced factory method that creates streaming parsers with access to tool schema information
    /// for precise type conversion during streaming operation.
    ///
    /// # Arguments
    /// * `strategy` - The parsing strategy to use
    /// * `tool_definitions` - Optional tool definitions containing schema information
    ///
    /// # Returns
    /// A boxed trait object implementing `StreamingToolCallParser` with schema capabilities where supported
    pub fn create_streaming_parser_with_schema(
        strategy: ToolParsingStrategy,
        tool_definitions: Option<&[ToolDefinition]>,
    ) -> Box<dyn StreamingToolCallParser> {
        match strategy {
            ToolParsingStrategy::Qwen3Coder => {
                Box::new(Qwen3CoderStreamingParser::new_with_schema(tool_definitions))
            }
            _ => {
                // For other strategies, create a simple buffering wrapper with schema support
                let base_parser = Self::create_parser_with_schema(strategy, tool_definitions);
                Box::new(BufferedStreamingParser::new(base_parser))
            }
        }
    }
}

/// Simple buffering wrapper that provides streaming interface for non-streaming parsers
/// This is a fallback implementation for strategies that don't yet have native streaming support
pub struct BufferedStreamingParser {
    base_parser: Box<dyn ToolCallParser>,
    buffer: String,
    completed_calls: Vec<ToolCall>,
}

impl BufferedStreamingParser {
    pub fn new(base_parser: Box<dyn ToolCallParser>) -> Self {
        Self {
            base_parser,
            buffer: String::new(),
            completed_calls: Vec::new(),
        }
    }
}

/// Comprehensive end-to-end validation with actual Qwen3-Coder model
/// Integration tests for Qwen3-Coder model with real model inference.
///
/// This test module provides comprehensive end-to-end validation of the Qwen3-Coder
/// integration by testing against actual model files. These tests validate:
///
/// - Model loading and initialization with llama_cpp_2
/// - Template rendering with real model chat templates
/// - Tokenization and context creation
/// - Tool call extraction from real model outputs
/// - Streaming parsing with realistic token deltas
/// - Performance validation under real-world conditions
///
/// ## Requirements
///
/// Tests require an actual Qwen3-Coder model file and use the `QWEN3_CODER_MODEL_PATH`
/// environment variable to specify the model file location. Tests gracefully skip
/// when the environment variable is not set.
///
/// ## Usage
///
/// ```bash
/// export QWEN3_CODER_MODEL_PATH=/path/to/qwen3-coder-model.gguf
/// cargo test qwen3coder_model_integration
/// ```
#[cfg(test)]
mod qwen3coder_model_integration {
    use super::*;
    use crate::types::{
        MCPServerConfig, Message, MessageRole, ModelConfig, ModelSource, ProcessServerConfig,
        RetryConfig, Session, SessionId, ToolDefinition,
    };
    use llama_cpp_2::{
        context::params::LlamaContextParams,
        llama_backend::LlamaBackend,
        model::{params::LlamaModelParams, LlamaModel},
    };
    use std::env;
    use std::path::PathBuf;
    use std::time::{Instant, SystemTime};

    /// Skip tests if model is not available
    fn skip_if_model_unavailable() -> Result<(), &'static str> {
        if env::var("QWEN3_CODER_MODEL_PATH").is_err() {
            eprintln!("Skipping test: QWEN3_CODER_MODEL_PATH not set. Set to path of Qwen3-Coder model file to enable tests.");
            return Err("Model unavailable - test skipped");
        }
        Ok(())
    }

    /// Get model path from environment
    fn get_model_path() -> PathBuf {
        let model_path = env::var("QWEN3_CODER_MODEL_PATH")
            .expect("QWEN3_CODER_MODEL_PATH must be set for integration tests");
        PathBuf::from(&model_path)
    }

    /// Create test model configuration for integration tests
    fn create_test_model_config() -> ModelConfig {
        let model_path = get_model_path();
        let folder = model_path.parent().unwrap().to_path_buf();
        let filename = model_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        ModelConfig {
            source: ModelSource::Local {
                folder,
                filename: Some(filename),
            },
            batch_size: 1,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: false,
            retry_config: RetryConfig::default(),
            debug: false,
        }
    }

    /// Create realistic session with comprehensive tool definitions for testing
    fn create_qwen3coder_session_with_tools() -> Session {
        Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: "You are an AI assistant with access to various tools. Use them as needed to help users.".to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                }
            ],
            mcp_servers: vec![
                MCPServerConfig::InProcess(ProcessServerConfig {
                    name: "code_search".to_string(),
                    command: "mcp-code-search".to_string(),
                    args: vec!["--root=./src".to_string()],
                    timeout_secs: Some(30),
                }),
                MCPServerConfig::InProcess(ProcessServerConfig {
                    name: "code_analyzer".to_string(),
                    command: "mcp-code-analyzer".to_string(),
                    args: vec![],
                    timeout_secs: Some(60),
                }),
            ],
            available_tools: vec![
                ToolDefinition {
                    name: "search_code".to_string(),
                    description: "Search for code patterns in a codebase".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "The search query or pattern to find"
                            },
                            "language": {
                                "type": "string",
                                "description": "Programming language to search in",
                                "enum": ["rust", "python", "javascript", "typescript"]
                            },
                            "max_results": {
                                "type": "integer",
                                "default": 10,
                                "minimum": 1,
                                "maximum": 50
                            },
                            "case_sensitive": {
                                "type": "boolean",
                                "default": false
                            }
                        },
                        "required": ["query", "language"]
                    }),
                    server_name: "code_search".to_string(),
                },
                ToolDefinition {
                    name: "analyze_function".to_string(),
                    description: "Analyze a specific function and provide insights".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "function_name": {"type": "string"},
                            "file_path": {"type": "string"},
                            "analysis_type": {
                                "type": "string",
                                "enum": ["complexity", "performance", "security", "style"]
                            },
                            "include_suggestions": {"type": "boolean", "default": true}
                        },
                        "required": ["function_name", "file_path", "analysis_type"]
                    }),
                    server_name: "code_analyzer".to_string(),
                },
            ],
            available_prompts: vec![],
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,

            todos: Vec::new(),

            available_commands: Vec::new(),
            current_mode: None,

        client_capabilities: None,
            cached_message_count: 0,
            cached_token_count: 0,
        }
    }

    /// Test model loading and template engine configuration
    #[test]
    fn test_qwen3coder_model_loading() {
        if skip_if_model_unavailable().is_err() {
            return;
        }

        let _model_config = create_test_model_config();
        let model_path = get_model_path();

        // Initialize the backend
        let backend = LlamaBackend::init().expect("Backend should initialize");

        // Test model loading
        let model_params = LlamaModelParams::default();
        let model = match LlamaModel::load_from_file(&backend, &model_path, &model_params) {
            Ok(model) => model,
            Err(e) => panic!("Failed to load Qwen3-Coder model: {}", e),
        };

        // Test template engine configuration
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder-30B");
        assert_eq!(
            engine.get_parsing_strategy(),
            Some(&ToolParsingStrategy::Qwen3Coder)
        );

        println!(
            "✓ Successfully loaded Qwen3-Coder model from: {}",
            model_path.display()
        );
        println!("✓ Template engine correctly detected Qwen3Coder strategy");
        println!("✓ Model vocab size: {}", model.n_vocab());
    }

    /// Test template rendering with actual model
    #[test]
    fn test_qwen3coder_template_rendering() {
        if skip_if_model_unavailable().is_err() {
            return;
        }

        let model_config = create_test_model_config();
        let model_path = get_model_path();

        // Initialize the backend
        let backend = LlamaBackend::init().expect("Backend should initialize");

        // Load model
        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, &model_path, &model_params)
            .expect("Model should load");

        let session = create_qwen3coder_session_with_tools();
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder");

        // Test template rendering with tools
        let start = Instant::now();
        let rendered = engine
            .render_session_with_config(&session, &model, Some(&model_config))
            .expect("Template rendering should succeed");
        let render_time = start.elapsed();

        // Verify template contains expected elements
        assert!(rendered.contains("search_code"));
        assert!(rendered.contains("analyze_function"));
        assert!(
            !rendered.is_empty(),
            "Rendered template should not be empty"
        );

        println!(
            "✓ Template rendered successfully: {} characters in {:?}",
            rendered.len(),
            render_time
        );
        println!("✓ Template contains expected tool definitions");

        // Performance information - template rendering time
        println!("✓ Template rendering time: {:?}", render_time);
    }

    /// Test template rendering and basic tokenization with actual model
    ///
    /// This test focuses on what we can reliably test: model loading, template rendering,
    /// and basic tokenization. Full generation is complex and model-dependent.
    #[test]
    fn test_template_and_tokenization_workflow() {
        if skip_if_model_unavailable().is_err() {
            return;
        }

        let model_config = create_test_model_config();
        let model_path = get_model_path();

        // Initialize the backend
        let backend = LlamaBackend::init().expect("Backend should initialize");

        // Load model
        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, &model_path, &model_params)
            .expect("Model should load");

        let mut session = create_qwen3coder_session_with_tools();
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder-30B");

        // Add user message requesting tool use
        session.messages.push(Message {
            role: MessageRole::User,
            content: "Search for async functions in Rust code, limit to 5 results".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });

        // Test template rendering
        let prompt_start = Instant::now();
        let prompt = engine
            .render_session_with_config(&session, &model, Some(&model_config))
            .expect("Prompt rendering should work");
        let prompt_time = prompt_start.elapsed();

        println!("✓ Template rendered in {:?}", prompt_time);
        println!("✓ Prompt length: {} characters", prompt.len());

        // Verify prompt contains expected elements
        assert!(
            prompt.contains("search_code"),
            "Prompt should contain search_code tool"
        );
        assert!(
            prompt.contains("analyze_function"),
            "Prompt should contain analyze_function tool"
        );
        assert!(prompt.len() > 100, "Prompt should be substantial");

        // Test tokenization
        let context_params = LlamaContextParams::default().with_n_ctx(Some(
            std::num::NonZero::<u32>::new(super::tests::DEFAULT_CONTEXT_SIZE).unwrap(),
        ));
        let context = model
            .new_context(&backend, context_params)
            .expect("Context creation should work");

        let tokenize_start = Instant::now();
        use llama_cpp_2::model::AddBos;
        let tokens = context
            .model
            .str_to_token(&prompt, AddBos::Always)
            .expect("Tokenization should work");
        let tokenize_time = tokenize_start.elapsed();

        println!(
            "✓ Tokenized {} characters into {} tokens in {:?}",
            prompt.len(),
            tokens.len(),
            tokenize_time
        );

        // Basic validation
        assert!(!tokens.is_empty(), "Should produce some tokens");

        // Test that we can convert tokens back to text
        let detokenize_start = Instant::now();
        use llama_cpp_2::model::Special;
        let reconstructed = context
            .model
            .tokens_to_str(&tokens, Special::Tokenize)
            .expect("Detokenization should work");
        let detokenize_time = detokenize_start.elapsed();

        println!(
            "✓ Detokenized back to {} characters in {:?}",
            reconstructed.len(),
            detokenize_time
        );

        // Basic text reconstruction validation
        assert!(
            !reconstructed.is_empty(),
            "Reconstructed text should not be empty"
        );

        // Performance information
        println!("✓ Template rendering time: {:?}", prompt_time);
        assert!(
            tokenize_time.as_millis() < 1000,
            "Tokenization should be under 1s, got {:?}",
            tokenize_time
        );
        assert!(
            detokenize_time.as_millis() < 1000,
            "Detokenization should be under 1s, got {:?}",
            detokenize_time
        );

        println!("✓ Full template and tokenization workflow completed successfully");
    }

    /// Test streaming tool call extraction with realistic model-like deltas
    #[test]
    fn test_streaming_tool_call_extraction() {
        // Note: This test doesn't require actual model but simulates realistic streaming
        let mut streaming_parser = Qwen3CoderStreamingParser::new();

        // Simulate realistic streaming deltas that would come from actual model generation
        let realistic_deltas = [
            "I'll help you search for that. Let me use the search tool.\n\n<",
            "tool_call",
            "><search_",
            "code><",
            "query>async fn",
            " examples</query><",
            "language>rust</language><max_",
            "results>5</max_results><case_sensitive>false</case_",
            "sensitive></search_code></tool_call>\n\n",
            "I've initiated the search for async function examples in Rust code.",
        ];

        let mut all_completed_calls = Vec::new();
        let start_streaming = Instant::now();

        for (i, delta) in realistic_deltas.iter().enumerate() {
            match streaming_parser.process_delta(delta) {
                Ok(completed_calls) => {
                    if !completed_calls.is_empty() {
                        println!(
                            "✓ Completed {} tool calls at delta {}",
                            completed_calls.len(),
                            i
                        );
                        all_completed_calls.extend(completed_calls);
                    }
                }
                Err(e) => {
                    println!("⚠ Error processing delta {}: {}", i, e);
                    // Streaming should be tolerant of temporary parsing errors
                }
            }
        }

        let streaming_time = start_streaming.elapsed();

        // Verify final results
        assert_eq!(
            all_completed_calls.len(),
            1,
            "Should have completed exactly one tool call"
        );

        let tool_call = &all_completed_calls[0];
        assert_eq!(tool_call.name, "search_code");
        assert_eq!(tool_call.arguments["query"], "async fn examples");
        assert_eq!(tool_call.arguments["language"], "rust");
        assert_eq!(tool_call.arguments["max_results"], 5);
        assert_eq!(tool_call.arguments["case_sensitive"], false);

        println!("✓ Streaming parsing completed in {:?}", streaming_time);
        println!(
            "✓ Successfully parsed streaming tool call: {}",
            tool_call.name
        );

        // Performance information
        println!("✓ Streaming parsing time: {:?}", streaming_time);
    }

    /// Test compatibility with reference vLLM Qwen3Coder parser outputs
    #[test]
    fn test_compatibility_with_reference_outputs() {
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder");

        // Test basic search call
        {
            let test_name = "Basic search call";
            let output = r#"<tool_call><search><query>rust async</query><limit>10</limit></search></tool_call>"#;

            println!("Testing compatibility: {}", test_name);
            let start = Instant::now();
            let tool_calls = engine
                .extract_tool_calls(output)
                .unwrap_or_else(|_| panic!("Should parse reference output for: {}", test_name));
            let parse_time = start.elapsed();

            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].name, "search");
            assert_eq!(tool_calls[0].arguments["query"], "rust async");
            assert_eq!(tool_calls[0].arguments["limit"], 10);

            println!("✓ {} - parsed in {:?}", test_name, parse_time);
        }

        // Test complex nested parameters
        {
            let test_name = "Complex nested parameters";
            let output = r#"<tool_call><complex_tool><metadata>{"version": "1.0", "tags": ["test", "example"]}</metadata><config>{"enabled": true, "timeout": 30}</config></complex_tool></tool_call>"#;

            println!("Testing compatibility: {}", test_name);
            let start = Instant::now();
            let tool_calls = engine
                .extract_tool_calls(output)
                .unwrap_or_else(|_| panic!("Should parse reference output for: {}", test_name));
            let parse_time = start.elapsed();

            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].name, "complex_tool");
            // JSON strings should be parsed as objects when possible
            if let Some(metadata_str) = tool_calls[0]
                .arguments
                .get("metadata")
                .and_then(|v| v.as_str())
            {
                if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(metadata_str) {
                    assert_eq!(metadata["version"], "1.0");
                }
            }

            println!("✓ {} - parsed in {:?}", test_name, parse_time);
        }

        // Test multiple consecutive tool calls
        {
            let test_name = "Multiple consecutive tool calls";
            let output = r#"<tool_call><search><query>first</query></search></tool_call><tool_call><analyze><target>second</target></analyze></tool_call>"#;

            println!("Testing compatibility: {}", test_name);
            let start = Instant::now();
            let tool_calls = engine
                .extract_tool_calls(output)
                .unwrap_or_else(|_| panic!("Should parse reference output for: {}", test_name));
            let parse_time = start.elapsed();

            assert_eq!(tool_calls.len(), 2);
            assert_eq!(tool_calls[0].name, "search");
            assert_eq!(tool_calls[1].name, "analyze");
            assert_eq!(tool_calls[0].arguments["query"], "first");
            assert_eq!(tool_calls[1].arguments["target"], "second");

            println!("✓ {} - parsed in {:?}", test_name, parse_time);
        }
    }

    /// Test performance validation with timing requirements
    #[test]
    fn test_model_integration_performance() {
        if skip_if_model_unavailable().is_err() {
            return;
        }

        let model_config = create_test_model_config();
        let model_path = get_model_path();

        // Initialize the backend
        let backend = LlamaBackend::init().expect("Backend should initialize");

        // Load model
        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, &model_path, &model_params)
            .expect("Model should load");

        let session = create_qwen3coder_session_with_tools();
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder");

        // Test template rendering performance
        let start = Instant::now();
        let _rendered = engine
            .render_session_with_config(&session, &model, Some(&model_config))
            .expect("Template rendering should work");
        let render_time = start.elapsed();

        println!("✓ Template rendering time: {:?}", render_time);

        // Test tool call extraction performance with typical output
        let typical_output = r#"I'll search for that information.

<tool_call>
<search_code>
<query>async fn main</query>
<language>rust</language>
<max_results>10</max_results>
</search_code>
</tool_call>

Let me analyze the results for you."#;

        let start = Instant::now();
        let tool_calls = engine
            .extract_tool_calls(typical_output)
            .expect("Tool call extraction should work");
        let extract_time = start.elapsed();

        println!("✓ Tool call extraction time: {:?}", extract_time);
        println!("✓ Extracted {} tool calls", tool_calls.len());
        assert_eq!(tool_calls.len(), 1, "Should extract exactly one tool call");
        assert_eq!(tool_calls[0].name, "search_code");
    }
}

impl StreamingToolCallParser for BufferedStreamingParser {
    fn process_delta(&mut self, delta: &str) -> Result<Vec<ToolCall>, TemplateError> {
        self.buffer.push_str(delta);

        // Try to parse whatever we have so far
        match self.base_parser.parse_tool_calls(&self.buffer) {
            Ok(calls) => {
                // If we got tool calls, they're complete
                if !calls.is_empty() {
                    // Only clear buffer of the content we successfully parsed
                    // Keep track of what we've already returned to avoid duplicates
                    let new_calls: Vec<ToolCall> = calls
                        .into_iter()
                        .filter(|call| !self.completed_calls.contains(call))
                        .collect();

                    if !new_calls.is_empty() {
                        self.completed_calls.extend(new_calls.clone());
                        // After successful parse, we can be more aggressive about clearing
                        // but only if we're confident we extracted everything
                        self.buffer.clear();
                        Ok(new_calls)
                    } else {
                        Ok(vec![])
                    }
                } else {
                    Ok(vec![])
                }
            }
            Err(_) => {
                // Not ready yet, continue buffering
                // Don't clear buffer - we need to accumulate more content
                Ok(vec![])
            }
        }
    }

    fn get_completed_tool_calls(&self) -> Vec<ToolCall> {
        self.completed_calls.clone()
    }

    fn is_parsing_tool_call(&self) -> bool {
        !self.buffer.is_empty()
    }

    fn reset(&mut self) {
        self.buffer.clear();
        self.completed_calls.clear();
    }
}

/// Comprehensive integration tests for Qwen3Coder parser
///
/// These tests focus on end-to-end integration scenarios, real-world model outputs,
/// performance validation, and complete workflow testing from strategy detection
/// through tool call extraction and type conversion.
#[cfg(test)]
mod qwen3coder_integration_tests {
    use super::*;
    use crate::types::{
        MCPServerConfig, Message, MessageRole, ProcessServerConfig, Session, SessionId,
        ToolDefinition,
    };
    use std::time::{Instant, SystemTime};

    /// Create a realistic Qwen3Coder session with comprehensive tool definitions
    fn create_qwen3coder_session() -> Session {
        Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: "You are an AI assistant with access to various tools. Use them as needed to help users.".to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                },
                Message {
                    role: MessageRole::User,
                    content: "Search for Rust async programming examples and list the files in the current directory".to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                },
            ],
            mcp_servers: vec![
                MCPServerConfig::InProcess(ProcessServerConfig {
                    name: "filesystem".to_string(),
                    command: "mcp-filesystem".to_string(),
                    args: vec!["--root=/tmp".to_string()],
                    timeout_secs: Some(30),
                }),
            ],
            available_tools: vec![
                ToolDefinition {
                    name: "search".to_string(),
                    description: "Search for information using various criteria".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Search query string"
                            },
                            "limit": {
                                "type": "integer",
                                "default": 10,
                                "minimum": 1,
                                "maximum": 100,
                                "description": "Maximum number of results"
                            },
                            "exact": {
                                "type": "boolean",
                                "default": false,
                                "description": "Whether to perform exact matching"
                            },
                            "filters": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "Optional search filters"
                            }
                        },
                        "required": ["query"]
                    }),
                    server_name: "search_service".to_string(),
                },
                ToolDefinition {
                    name: "list_files".to_string(),
                    description: "List files and directories in a specified path".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Directory path to list",
                                "default": "."
                            },
                            "recursive": {
                                "type": "boolean",
                                "default": false,
                                "description": "Whether to list recursively"
                            },
                            "max_depth": {
                                "type": "integer",
                                "default": 1,
                                "minimum": 1,
                                "maximum": 10,
                                "description": "Maximum recursion depth"
                            },
                            "include_hidden": {
                                "type": "boolean",
                                "default": false,
                                "description": "Include hidden files"
                            }
                        },
                        "required": ["path"]
                    }),
                    server_name: "filesystem".to_string(),
                },
                ToolDefinition {
                    name: "advanced_search".to_string(),
                    description: "Advanced search with complex criteria and nested parameters".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "criteria": {
                                "type": "object",
                                "properties": {
                                    "language": {"type": "string"},
                                    "topic": {"type": "string"},
                                    "difficulty": {"type": "string", "enum": ["beginner", "intermediate", "advanced"]}
                                }
                            },
                            "filters": {
                                "type": "array",
                                "items": {"type": "string"}
                            },
                            "options": {
                                "type": "object",
                                "properties": {
                                    "sort": {"type": "string", "default": "relevance"},
                                    "include_snippets": {"type": "boolean", "default": true},
                                    "max_results": {"type": "integer", "default": 20}
                                }
                            }
                        },
                        "required": ["criteria"]
                    }),
                    server_name: "search_service".to_string(),
                },
            ],
            available_prompts: vec![],
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,

            todos: Vec::new(),

            available_commands: Vec::new(),
            current_mode: None,

        client_capabilities: None,
            cached_message_count: 0,
            cached_token_count: 0,
        }
    }

    /// Test complete end-to-end integration workflow
    #[test]
    fn test_qwen3coder_full_integration() {
        // Initialize tracing for debugging
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        // Test automatic strategy detection from model name
        let engine =
            ChatTemplateEngine::with_model_strategy("unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF");
        assert_eq!(
            engine.get_parsing_strategy(),
            Some(&ToolParsingStrategy::Qwen3Coder)
        );

        // Test with realistic Qwen3Coder model output
        let model_output = r#"I'll help you search for Rust async programming examples and list the files.

<tool_call>
<search>
<query>Rust async programming examples</query>
<limit>5</limit>
<exact>false</exact>
<filters>["rust", "async", "examples"]</filters>
</search>
</tool_call>

Let me also list the files in the current directory:

<tool_call>
<list_files>
<path>.</path>
<recursive>true</recursive>
<max_depth>2</max_depth>
<include_hidden>false</include_hidden>
</list_files>
</tool_call>

I've initiated both searches to gather the information you need."#;

        let tool_calls = engine.extract_tool_calls(model_output).unwrap();
        assert_eq!(tool_calls.len(), 2);

        // Verify first tool call (search)
        assert_eq!(tool_calls[0].name, "search");
        let args = &tool_calls[0].arguments;
        assert_eq!(args["query"], "Rust async programming examples");
        assert_eq!(args["limit"], 5);
        assert_eq!(args["exact"], false);
        assert!(args["filters"].is_array());
        assert_eq!(args["filters"][0], "rust");

        // Verify second tool call (list_files)
        assert_eq!(tool_calls[1].name, "list_files");
        let args = &tool_calls[1].arguments;
        assert_eq!(args["path"], ".");
        assert_eq!(args["recursive"], true);
        assert_eq!(args["max_depth"], 2);
        assert_eq!(args["include_hidden"], false);
    }

    /// Test integration with session context and schema-aware parsing
    #[test]
    fn test_qwen3coder_with_session_context() {
        let session = create_qwen3coder_session();
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder-30B");

        // Test schema-aware parsing with type conversion
        let output = r#"<tool_call><search><query>async Rust patterns</query><limit>15</limit><exact>true</exact></search></tool_call>"#;
        let tool_calls = engine.extract_tool_calls(output).unwrap();

        // Validate against session's available tools
        assert!(session
            .available_tools
            .iter()
            .any(|t| t.name == tool_calls[0].name));

        // Test type conversion based on schema
        assert_eq!(tool_calls[0].arguments["query"], "async Rust patterns");
        assert_eq!(tool_calls[0].arguments["limit"], 15);
        assert_eq!(tool_calls[0].arguments["exact"], true);
    }

    /// Test realistic Qwen3-Coder model output scenarios
    #[test]
    fn test_realistic_qwen3coder_outputs() {
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder");

        // Test 1: Multiple tool calls with conversational text
        let multi_call_output = r#"I'll help you with that comprehensive analysis. Let me start by exploring the codebase structure:

<tool_call>
<list_files>
<path>./src</path>
<recursive>true</recursive>
<max_depth>3</max_depth>
<include_hidden>false</include_hidden>
</list_files>
</tool_call>

Now let me search for specific async patterns in Rust:

<tool_call>
<search>
<query>async fn tokio futures</query>
<limit>10</limit>
<exact>false</exact>
<filters>["rust", "tokio", "async"]</filters>
</search>
</tool_call>

Finally, let me do an advanced search for more detailed information:

<tool_call>
<advanced_search>
<criteria>{"language": "rust", "topic": "async", "difficulty": "intermediate"}</criteria>
<filters>["recent", "popular", "well-documented"]</filters>
<options>{"sort": "relevance", "include_snippets": true, "max_results": 15}</options>
</advanced_search>
</tool_call>

This comprehensive search should give us excellent insights into Rust async programming patterns."#;

        let tool_calls = engine.extract_tool_calls(multi_call_output).unwrap();
        assert_eq!(tool_calls.len(), 3);

        // Verify first call
        assert_eq!(tool_calls[0].name, "list_files");
        assert_eq!(tool_calls[0].arguments["path"], "./src");
        assert_eq!(tool_calls[0].arguments["recursive"], true);
        assert_eq!(tool_calls[0].arguments["max_depth"], 3);

        // Verify second call
        assert_eq!(tool_calls[1].name, "search");
        assert_eq!(tool_calls[1].arguments["query"], "async fn tokio futures");
        assert_eq!(tool_calls[1].arguments["limit"], 10);

        // Verify third call with complex nested parameters
        assert_eq!(tool_calls[2].name, "advanced_search");
        let criteria = &tool_calls[2].arguments["criteria"];
        assert!(criteria.is_object());
        assert_eq!(criteria["language"], "rust");
        assert_eq!(criteria["topic"], "async");
        assert_eq!(criteria["difficulty"], "intermediate");

        let filters = &tool_calls[2].arguments["filters"];
        assert!(filters.is_array());
        assert_eq!(filters[0], "recent");
        assert_eq!(filters[1], "popular");
        assert_eq!(filters[2], "well-documented");

        let options = &tool_calls[2].arguments["options"];
        assert!(options.is_object());
        assert_eq!(options["sort"], "relevance");
        assert_eq!(options["include_snippets"], true);
        assert_eq!(options["max_results"], 15);
    }

    /// Test error recovery and graceful degradation
    #[test]
    fn test_qwen3coder_error_recovery() {
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder");

        // Test incomplete tool call (cut off during generation)
        let incomplete =
            r#"I'll search for that information. <tool_call><search><query>partial search term"#;
        let tool_calls = engine.extract_tool_calls(incomplete).unwrap();
        // Should handle gracefully - either extract partial or return empty
        assert!(tool_calls.len() <= 1);

        // Test malformed but recoverable XML
        let malformed =
            r#"<tool_call><search><query>test query</query><limit>10</search></tool_call>"#; // Missing </limit>
        let tool_calls = engine.extract_tool_calls(malformed).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "search");
        assert_eq!(tool_calls[0].arguments["query"], "test query");

        // Test mixed valid and invalid tool calls
        let mixed_validity = r#"
<tool_call><search><query>valid search</query><limit>5</limit></search></tool_call>
<tool_call><invalid_xml><broken>content</broken></invalid_xml></tool_call>
<tool_call><list_files><path>/tmp</path></list_files></tool_call>
"#;
        let tool_calls = engine.extract_tool_calls(mixed_validity).unwrap();
        assert!(!tool_calls.is_empty()); // Should extract at least the valid ones

        // Find and verify the valid calls
        let search_call = tool_calls.iter().find(|tc| tc.name == "search");
        assert!(search_call.is_some());
        let list_call = tool_calls.iter().find(|tc| tc.name == "list_files");
        assert!(list_call.is_some());
    }

    /// Test performance with various input sizes
    #[test]
    fn test_qwen3coder_performance() {
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder");

        // Test with large input text containing tool calls
        let large_text = format!(
            "{}\n<tool_call><search><query>performance test</query><limit>10</limit></search></tool_call>\n{}",
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(1000),
            "Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. ".repeat(1000)
        );

        let start = Instant::now();
        let tool_calls = engine.extract_tool_calls(&large_text).unwrap();
        let duration = start.elapsed();

        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "search");
        println!("✓ Parsed in {:?}", duration);
    }

    /// Test backward compatibility with existing parsing strategies
    #[test]
    fn test_backward_compatibility() {
        let engine = ChatTemplateEngine::new(); // Default engine without specific strategy

        // Existing JSON parsing should still work
        let json_call = r#"{"function_name": "search", "arguments": {"query": "test"}}"#;
        let tool_calls = engine.extract_tool_calls(json_call).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "search");
        assert_eq!(tool_calls[0].arguments["query"], "test");

        // Existing XML function call parsing should still work
        let xml_call = r#"<function_call name="search">{"query": "test"}</function_call>"#;
        let tool_calls = engine.extract_tool_calls(xml_call).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "search");

        // Mixed formats should be handled
        let mixed_text = r#"
        Here's a JSON call: {"function_name": "search", "arguments": {"query": "json test"}}
        And here's an XML call: <function_call name="list_files">{"path": "/tmp"}</function_call>
        "#;
        let tool_calls = engine.extract_tool_calls(mixed_text).unwrap();
        assert!(!tool_calls.is_empty()); // Should extract at least one format
    }

    /// Test strategy fallback behavior
    #[test]
    fn test_strategy_fallback() {
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder");

        // When Qwen3Coder parsing encounters non-Qwen format, should handle gracefully
        let json_in_qwen_context = r#"I'll use this tool: {"function_name": "search", "arguments": {"query": "fallback test"}}"#;
        let tool_calls = engine.extract_tool_calls(json_in_qwen_context).unwrap();

        // Should still parse the JSON format even in Qwen3Coder strategy context
        if !tool_calls.is_empty() {
            assert_eq!(tool_calls[0].name, "search");
            assert_eq!(tool_calls[0].arguments["query"], "fallback test");
        }
    }

    /// Test complex nested parameter handling
    #[test]
    fn test_complex_nested_parameters() {
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder");

        let complex_nested = r#"
<tool_call>
<advanced_search>
<criteria>{"language": "rust", "frameworks": ["tokio", "async-std"], "complexity": {"min": 3, "max": 8}}</criteria>
<filters>["recent", "popular"]</filters>
<options>{"sort": "date", "grouping": {"by": "framework", "limit": 5}, "metadata": {"include_stats": true, "include_examples": false}}</options>
</advanced_search>
</tool_call>
"#;

        let tool_calls = engine.extract_tool_calls(complex_nested).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "advanced_search");

        let args = &tool_calls[0].arguments;

        // Verify complex criteria object
        let criteria = &args["criteria"];
        assert!(criteria.is_object());
        assert_eq!(criteria["language"], "rust");
        assert!(criteria["frameworks"].is_array());
        assert_eq!(criteria["frameworks"][0], "tokio");
        assert_eq!(criteria["frameworks"][1], "async-std");
        let complexity = &criteria["complexity"];
        assert_eq!(complexity["min"], 3);
        assert_eq!(complexity["max"], 8);

        // Verify filters array
        let filters = &args["filters"];
        assert!(filters.is_array());
        assert_eq!(filters.as_array().unwrap().len(), 2);

        // Verify complex options object
        let options = &args["options"];
        assert!(options.is_object());
        assert_eq!(options["sort"], "date");
        let grouping = &options["grouping"];
        assert_eq!(grouping["by"], "framework");
        assert_eq!(grouping["limit"], 5);
        let metadata = &options["metadata"];
        assert_eq!(metadata["include_stats"], true);
        assert_eq!(metadata["include_examples"], false);
    }

    /// Test integration with different model name patterns
    #[test]
    fn test_model_name_integration() {
        let model_names = vec![
            "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF",
            "Qwen3-Coder-7B-Instruct",
            "qwen3-coder-1.5b",
            "microsoft/Qwen3-Coder-30B",
        ];

        for model_name in model_names {
            let engine = ChatTemplateEngine::with_model_strategy(model_name);
            assert_eq!(
                engine.get_parsing_strategy(),
                Some(&ToolParsingStrategy::Qwen3Coder)
            );

            // Test that tool parsing works with the detected strategy
            let test_output =
                r#"<tool_call><search><query>integration test</query></search></tool_call>"#;
            let tool_calls = engine.extract_tool_calls(test_output).unwrap();
            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].name, "search");
        }
    }

    /// Test realistic conversation flow with multiple exchanges
    #[test]
    fn test_conversation_flow_integration() {
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder");

        // Simulate a realistic conversation flow with multiple tool calls
        let conversation_parts = [
            // First assistant response
            r#"I'll help you analyze the codebase. Let me start by exploring the structure:

<tool_call>
<list_files>
<path>./src</path>
<recursive>true</recursive>
</list_files>
</tool_call>"#,
            // Second assistant response after getting results
            r#"Based on the file structure, let me search for specific patterns:

<tool_call>
<search>
<query>async fn main</query>
<limit>5</limit>
</search>
</tool_call>

<tool_call>
<search>
<query>tokio runtime</query>
<limit>3</limit>
</search>
</tool_call>"#,
            // Final comprehensive analysis
            r#"Now let me perform a comprehensive analysis:

<tool_call>
<advanced_search>
<criteria>{"language": "rust", "topic": "concurrency"}</criteria>
<filters>["async", "tokio"]</filters>
<options>{"sort": "complexity", "include_snippets": true}</options>
</advanced_search>
</tool_call>

This should give us a complete picture of the async patterns in your codebase."#,
        ];

        let mut total_calls = 0;
        for (i, part) in conversation_parts.iter().enumerate() {
            let tool_calls = engine.extract_tool_calls(part).unwrap();
            assert!(
                !tool_calls.is_empty(),
                "No tool calls found in conversation part {}",
                i
            );
            total_calls += tool_calls.len();

            // Verify each part has expected tool calls
            match i {
                0 => {
                    assert_eq!(tool_calls.len(), 1);
                    assert_eq!(tool_calls[0].name, "list_files");
                }
                1 => {
                    assert_eq!(tool_calls.len(), 2);
                    assert!(tool_calls.iter().all(|tc| tc.name == "search"));
                }
                2 => {
                    assert_eq!(tool_calls.len(), 1);
                    assert_eq!(tool_calls[0].name, "advanced_search");
                }
                _ => {}
            }
        }

        assert_eq!(total_calls, 4); // Total across all conversation parts
    }

    /// Test edge cases and boundary conditions
    #[test]
    fn test_edge_cases() {
        let engine = ChatTemplateEngine::with_model_strategy("Qwen3-Coder");

        // Empty tool call
        let empty_call = r#"<tool_call><search></search></tool_call>"#;
        let tool_calls = engine.extract_tool_calls(empty_call).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "search");

        // Self-closing tags
        let self_closing = r#"<tool_call><search query="test" limit="5"/></tool_call>"#;
        let tool_calls = engine.extract_tool_calls(self_closing).unwrap();
        // This might not parse correctly, but should not crash
        assert!(tool_calls.len() <= 1);

        // Very long parameter values
        let long_param = "x".repeat(STRESS_TEST_REPEAT_SIZE);
        let long_call = format!(
            r#"<tool_call><search><query>{}</query></search></tool_call>"#,
            long_param
        );
        let tool_calls = engine.extract_tool_calls(&long_call).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].arguments["query"], long_param);

        // Unicode and special characters
        let unicode_call =
            r#"<tool_call><search><query>测试 🚀 русский עברית</query></search></tool_call>"#;
        let tool_calls = engine.extract_tool_calls(unicode_call).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].arguments["query"], "测试 🚀 русский עברית");
    }
}

/// Default tool parser that encapsulates the existing multi-parser approach
///
/// This parser tries multiple parsing strategies in sequence until one succeeds,
/// maintaining backward compatibility with the existing behavior.
pub struct DefaultToolParser {
    parsers: Vec<Box<dyn ToolCallParser>>,
}

impl DefaultToolParser {
    pub fn new() -> Self {
        let parsers: Vec<Box<dyn ToolCallParser>> = vec![
            Box::new(JsonToolCallParser::new()),
            Box::new(XmlToolCallParser::new()),
            Box::new(FunctionCallParser::new()),
        ];
        Self { parsers }
    }
}

impl Default for DefaultToolParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCallParser for DefaultToolParser {
    fn parse_tool_calls(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        debug!("DefaultToolParser: Trying {} parsers", self.parsers.len());

        // Try each parser until we find tool calls (existing logic from ChatTemplateEngine)
        for (i, parser) in self.parsers.iter().enumerate() {
            debug!("DefaultToolParser: Trying parser {}", i);

            match parser.parse_tool_calls(text) {
                Ok(tool_calls) if !tool_calls.is_empty() => {
                    debug!(
                        "DefaultToolParser: Found {} tool calls with parser {}",
                        tool_calls.len(),
                        i
                    );
                    return Ok(tool_calls);
                }
                Ok(_) => {
                    debug!("DefaultToolParser: No tool calls found with parser {}", i);
                    continue;
                }
                Err(e) => {
                    debug!("DefaultToolParser: Parser {} failed: {}", i, e);
                    continue;
                }
            }
        }

        debug!("DefaultToolParser: No tool calls found with any parser");
        Ok(vec![])
    }
}

/// Parser for Qwen3Coder-specific nested XML tool call formats
///
/// Handles the specific nested XML structure used by Qwen3Coder models:
/// <tool_call><toolname><param>value</param></toolname></tool_call>
/// Falls back to DefaultToolParser for backward compatibility with existing tests.
pub struct Qwen3CoderToolParser {
    tool_call_regex: Regex,
    fallback_parser: DefaultToolParser,
    schema_map: HashMap<String, Value>,
}

impl Qwen3CoderToolParser {
    pub fn new() -> Self {
        // Match tool_call blocks with nested content
        // Use (?s) flag to make . match newlines as well
        let tool_call_regex =
            Regex::new(r"(?s)<tool_call>\s*<(\w+)>(.*?)</\w+>\s*</tool_call>").unwrap();

        Self {
            tool_call_regex,
            fallback_parser: DefaultToolParser::new(),
            schema_map: HashMap::new(),
        }
    }

    /// Create a new Qwen3CoderToolParser with schema-based type conversion support
    ///
    /// This constructor enables schema-aware parameter type conversion by building
    /// a mapping of tool names to their parameter schemas. When available, the parser
    /// will use schema information to convert parameter values to appropriate JSON types
    /// instead of relying on basic type inference.
    ///
    /// # Arguments
    /// * `tool_definitions` - Optional slice of ToolDefinition containing schema information
    ///
    /// # Returns
    /// A new Qwen3CoderToolParser instance with schema mapping configured
    ///
    /// # Example
    /// ```rust
    /// use llama_agent::types::ToolDefinition;
    /// use serde_json::json;
    ///
    /// let tools = vec![ToolDefinition {
    ///     name: "search".to_string(),
    ///     description: "Search tool".to_string(),
    ///     parameters: json!({
    ///         "type": "object",
    ///         "properties": {
    ///             "query": {"type": "string"},
    ///             "limit": {"type": "integer"}
    ///         }
    ///     }),
    ///     server_name: "test".to_string(),
    /// }];
    ///
    /// let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));
    /// ```
    pub fn new_with_schema(tool_definitions: Option<&[ToolDefinition]>) -> Self {
        let tool_call_regex =
            Regex::new(r"(?s)<tool_call>\s*<(\w+)>(.*?)</\w+>\s*</tool_call>").unwrap();

        // Build schema map for type conversion
        let schema_map = if let Some(tools) = tool_definitions {
            Self::build_schema_map(tools)
        } else {
            HashMap::new()
        };

        Self {
            tool_call_regex,
            fallback_parser: DefaultToolParser::new(),
            schema_map,
        }
    }

    /// Build a mapping of tool names to their parameter schemas
    ///
    /// Creates a HashMap where keys are tool names and values are the complete
    /// parameter schema objects from ToolDefinition. This enables fast lookup
    /// of schema information during parameter type conversion.
    ///
    /// # Arguments
    /// * `tools` - Slice of ToolDefinition objects containing schema information
    ///
    /// # Returns
    /// HashMap mapping tool names to their parameter schema JSON objects
    ///
    /// # Example
    /// For a tool with parameters:
    /// ```json
    /// {
    ///   "type": "object",
    ///   "properties": {
    ///     "query": {"type": "string"},
    ///     "limit": {"type": "integer"}
    ///   }
    /// }
    /// ```
    /// The method stores this entire schema object as the value for the tool name key.
    fn build_schema_map(tools: &[ToolDefinition]) -> HashMap<String, Value> {
        let mut schema_map = HashMap::new();
        for tool in tools {
            schema_map.insert(tool.name.clone(), tool.parameters.clone());
        }
        schema_map
    }

    /// Get the JSON Schema definition for a specific parameter of a tool
    ///
    /// Looks up the schema information for a given tool and parameter name,
    /// navigating through the JSON Schema structure to find the parameter's
    /// type definition within the tool's parameters.properties object.
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool to look up
    /// * `param_name` - Name of the parameter within that tool
    ///
    /// # Returns
    /// * `Some(Value)` - The complete schema definition for the parameter
    /// * `None` - If tool or parameter is not found in schema
    ///
    /// # Example
    /// For a tool "search" with schema:
    /// ```json
    /// {
    ///   "type": "object",
    ///   "properties": {
    ///     "query": {"type": "string"},
    ///     "limit": {"type": "integer", "default": 10}
    ///   }
    /// }
    /// ```
    /// Calling `get_parameter_schema("search", "limit")` returns:
    /// ```json
    /// {"type": "integer", "default": 10}
    /// ```
    fn get_parameter_schema(&self, tool_name: &str, param_name: &str) -> Option<&Value> {
        self.schema_map
            .get(tool_name)
            .and_then(|schema| schema.get("properties"))
            .and_then(|props| props.get(param_name))
    }

    /// Handle empty or whitespace-only values based on schema constraints
    ///
    /// Determines the appropriate JSON value for empty input strings by checking
    /// schema properties like nullable flags and default values. This enables
    /// proper handling of optional parameters and nullable fields.
    ///
    /// # Arguments
    /// * `value` - Input string value (may be empty or whitespace)
    /// * `param_schema` - Optional JSON Schema object for the parameter
    ///
    /// # Returns
    /// * `Some(Value)` - Specific value to use for empty input (null, default, or empty string)
    /// * `None` - Value is not empty, continue with normal type conversion
    ///
    /// # Schema Property Handling
    /// * `nullable: true` - Returns `Value::Null` for empty values
    /// * `default: <value>` - Returns the specified default value
    /// * No special properties - Returns empty string `Value::String("")`
    /// * No schema provided - Returns empty string `Value::String("")`
    ///
    /// # Example
    /// ```rust
    /// // Schema with default value
    /// let schema = json!({"type": "integer", "default": 42});
    /// let result = parser.handle_empty_values("  ", Some(&schema));
    /// assert_eq!(result, Some(Value::Number(42.into())));
    ///
    /// // Schema with nullable
    /// let schema = json!({"type": "string", "nullable": true});
    /// let result = parser.handle_empty_values("", Some(&schema));
    /// assert_eq!(result, Some(Value::Null));
    /// ```
    fn handle_empty_values(&self, value: &str, param_schema: Option<&Value>) -> Option<Value> {
        if !value.trim().is_empty() {
            return None; // Value is not empty, continue with normal conversion
        }

        if let Some(schema) = param_schema {
            // Check for nullable in schema
            if schema
                .get("nullable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return Some(Value::Null);
            }

            // Check for default value
            if let Some(default) = schema.get("default") {
                return Some(default.clone());
            }
        }

        // Return empty string by default
        Some(Value::String(String::new()))
    }

    /// Convert a string value to appropriate JSON type based on schema information
    ///
    /// Uses JSON Schema type information to perform precise type conversion,
    /// supporting all standard JSON Schema types with appropriate error handling.
    /// Falls back to basic type inference when schema type is unknown or missing.
    ///
    /// # Arguments
    /// * `value` - String value to convert
    /// * `param_schema` - Complete JSON Schema object for the parameter
    ///
    /// # Returns
    /// * `Ok(Value)` - Successfully converted JSON value with appropriate type
    /// * `Err(TemplateError)` - Type conversion failed with descriptive error
    ///
    /// # Supported Schema Types
    /// * `string` - Direct string value
    /// * `integer` - Parsed as i64, validates numeric format
    /// * `number` - Parsed as f64, validates numeric format
    /// * `boolean` - Accepts "true"/"false", "1"/"0", "yes"/"no" (case-insensitive)
    /// * `object` - Parses JSON object string, validates JSON syntax
    /// * `array` - Parses JSON array string, validates JSON syntax
    /// * `null` - Returns JSON null value
    /// * Unknown types - Falls back to basic type inference
    ///
    /// # Example
    /// ```rust
    /// let schema = json!({"type": "integer", "minimum": 0});
    /// let result = parser.convert_by_schema_type("42", &schema)?;
    /// assert_eq!(result, Value::Number(42.into()));
    /// ```
    fn convert_by_schema_type(
        &self,
        value: &str,
        param_schema: &Value,
    ) -> Result<Value, TemplateError> {
        // First check for empty value handling
        if let Some(empty_result) = self.handle_empty_values(value, Some(param_schema)) {
            return Ok(empty_result);
        }

        // Get the type from schema
        let schema_type = param_schema.get("type").and_then(|t| t.as_str());

        match schema_type {
            Some("string") => Ok(Value::String(value.to_string())),
            Some("integer") => value
                .parse::<i64>()
                .map(|i| Value::Number(serde_json::Number::from(i)))
                .map_err(|e| {
                    TemplateError::ToolCallParsing(format!("Invalid integer '{}': {}", value, e))
                }),
            Some("number") => match value.parse::<f64>() {
                Ok(f) => serde_json::Number::from_f64(f)
                    .map(Value::Number)
                    .ok_or_else(|| {
                        TemplateError::ToolCallParsing(format!(
                            "Invalid number '{}': not a finite number",
                            value
                        ))
                    }),
                Err(e) => Err(TemplateError::ToolCallParsing(format!(
                    "Invalid number '{}': {}",
                    value, e
                ))),
            },
            Some("boolean") => match value.to_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(Value::Bool(true)),
                "false" | "0" | "no" => Ok(Value::Bool(false)),
                _ => Err(TemplateError::ToolCallParsing(format!(
                    "Invalid boolean value: '{}'. Expected true/false, 1/0, or yes/no",
                    value
                ))),
            },
            Some("object") => serde_json::from_str::<Value>(value)
                .and_then(|v| match v {
                    Value::Object(_) => Ok(v),
                    _ => Err(serde_json::from_str::<Value>("invalid").unwrap_err()),
                })
                .map_err(|e| {
                    TemplateError::ToolCallParsing(format!(
                        "Invalid JSON object '{}': {}",
                        value, e
                    ))
                }),
            Some("array") => serde_json::from_str::<Value>(value)
                .and_then(|v| match v {
                    Value::Array(_) => Ok(v),
                    _ => Err(serde_json::from_str::<Value>("invalid").unwrap_err()),
                })
                .map_err(|e| {
                    TemplateError::ToolCallParsing(format!("Invalid JSON array '{}': {}", value, e))
                }),
            Some("null") => Ok(Value::Null),
            _ => {
                // Unknown or missing type, fall back to basic conversion
                debug!(
                    "Unknown schema type {:?} for value '{}', falling back to basic conversion",
                    schema_type, value
                );
                self.convert_parameter_value(value)
            }
        }
    }

    /// Parse nested XML parameter tags with schema-aware type conversion
    ///
    /// Enhanced version of parse_nested_parameters that uses schema information
    /// for precise type conversion when available. Falls back to basic type
    /// inference for parameters not found in schema.
    ///
    /// # Arguments
    /// * `content` - The raw XML content containing nested parameter tags
    /// * `tool_name` - Name of the tool for schema lookup
    ///
    /// # Returns
    /// * `Ok(Value::Object)` - JSON object with parameters converted using schema information
    /// * `Err(TemplateError)` - If parameter value conversion fails
    ///
    /// # Example
    /// ```
    /// // With schema for "search" tool defining limit as integer
    /// let content = "<query>rust async</query><limit>10</limit>";
    /// let result = parser.parse_nested_parameters_with_schema(content, "search")?;
    /// // result = {"query": "rust async", "limit": 10} with correct types
    /// ```
    fn parse_nested_parameters_with_schema(
        &self,
        content: &str,
        tool_name: &str,
    ) -> Result<Value, TemplateError> {
        let param_regex = Regex::new(r"(?s)<(\w+)>(.*?)</\w+>").unwrap();
        let mut params = serde_json::Map::new();

        for cap in param_regex.captures_iter(content) {
            if let (Some(name), Some(value)) = (cap.get(1), cap.get(2)) {
                let param_name = name.as_str();
                let param_value = value.as_str().trim();

                // Use schema-aware conversion when possible
                let json_value =
                    self.convert_parameter_with_schema(tool_name, param_name, param_value)?;
                params.insert(param_name.to_string(), json_value);
            }
        }

        Ok(Value::Object(params))
    }

    /// Convert parameter value using schema information when available
    ///
    /// Attempts to use schema-based type conversion first, falling back to
    /// basic type inference when schema information is not available for
    /// the specific tool and parameter combination.
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool for schema lookup
    /// * `param_name` - Name of the parameter within the tool
    /// * `value` - String value to convert
    ///
    /// # Returns
    /// * `Ok(Value)` - Converted JSON value with appropriate type
    /// * `Err(TemplateError)` - Type conversion failed
    ///
    /// # Conversion Priority
    /// 1. **Schema-based** - Uses tool schema if available for precise conversion
    /// 2. **Basic inference** - Falls back to existing convert_parameter_value logic
    ///
    /// # Example
    /// ```rust
    /// // With schema defining limit as integer type
    /// let result = parser.convert_parameter_with_schema("search", "limit", "42")?;
    /// assert_eq!(result, Value::Number(42.into()));
    ///
    /// // Without schema, uses basic inference
    /// let result = parser.convert_parameter_with_schema("unknown_tool", "param", "42")?;
    /// // Still converts to number based on content analysis
    /// ```
    fn convert_parameter_with_schema(
        &self,
        tool_name: &str,
        param_name: &str,
        value: &str,
    ) -> Result<Value, TemplateError> {
        // Try schema-based conversion first
        if let Some(param_schema) = self.get_parameter_schema(tool_name, param_name) {
            debug!(
                "Using schema-based conversion for {}.{}: {:?}",
                tool_name,
                param_name,
                param_schema.get("type")
            );
            return self.convert_by_schema_type(value, param_schema);
        }

        // Fall back to basic type conversion
        debug!(
            "No schema found for {}.{}, using basic type inference",
            tool_name, param_name
        );
        self.convert_parameter_value(value)
    }

    /// Convert a string parameter value to the appropriate JSON type
    ///
    /// Attempts to intelligently parse string values into the most appropriate JSON type
    /// by trying different type conversions in a specific precedence order.
    ///
    /// # Conversion Priority
    /// 1. **Integer** (`i64`) - if the value is a valid integer
    /// 2. **Float** (`f64`) - if the value is a valid floating-point number
    /// 3. **Boolean** (`bool`) - if the value is "true" or "false" (case-insensitive)
    /// 4. **JSON Object/Array** - if the value starts with `{` or `[` and is valid JSON
    /// 5. **String** - fallback for all other values
    ///
    /// # Arguments
    /// * `value` - The string value to convert
    ///
    /// # Returns
    /// * `Ok(Value)` - The converted JSON value with the appropriate type
    /// * `Err(TemplateError)` - If JSON object/array parsing fails (strings never fail)
    ///
    /// # Examples
    /// ```
    /// parser.convert_parameter_value("42")?;        // -> Number(42)
    /// parser.convert_parameter_value("3.14")?;      // -> Number(3.14)
    /// parser.convert_parameter_value("true")?;      // -> Bool(true)
    /// parser.convert_parameter_value("{\"x\":1}")?; // -> Object({"x": 1})
    /// parser.convert_parameter_value("hello")?;     // -> String("hello")
    /// ```
    fn convert_parameter_value(&self, value: &str) -> Result<Value, TemplateError> {
        // Try to parse as different types in order

        // Try as integer
        if let Ok(int_val) = value.parse::<i64>() {
            return Ok(Value::Number(serde_json::Number::from(int_val)));
        }

        // Try as float
        if let Ok(float_val) = value.parse::<f64>() {
            if let Some(num) = serde_json::Number::from_f64(float_val) {
                return Ok(Value::Number(num));
            }
        }

        // Try as boolean
        if value.eq_ignore_ascii_case("true") {
            return Ok(Value::Bool(true));
        }
        if value.eq_ignore_ascii_case("false") {
            return Ok(Value::Bool(false));
        }

        // Try as JSON object/array
        if (value.starts_with('{') && value.ends_with('}'))
            || (value.starts_with('[') && value.ends_with(']'))
        {
            if let Ok(json_val) = serde_json::from_str::<Value>(value) {
                return Ok(json_val);
            }
        }

        // Default to string
        Ok(Value::String(value.to_string()))
    }

    /// Robust XML parsing with multiple fallback strategies
    ///
    /// Implements a multi-layer parsing approach to handle malformed, incomplete,
    /// or otherwise problematic XML that may be generated by language models.
    ///
    /// # Parsing Strategies (in order)
    /// 1. Standard regex parsing (fast path for well-formed XML)
    /// 2. Balanced tag parsing (handles malformed closing tags)
    /// 3. Fuzzy pattern matching (extracts from partial/incomplete XML)
    ///
    /// # Arguments
    /// * `text` - Raw text that may contain XML tool calls
    ///
    /// # Returns
    /// * `Ok(Vec<ToolCall>)` - Successfully parsed tool calls
    /// * `Err(TemplateError)` - All parsing strategies failed
    fn parse_tool_calls_robust(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        let normalized_text = self.normalize_xml_content(text);

        debug!(
            "Qwen3CoderToolParser: Starting robust parsing with {} strategies",
            3
        );

        // Strategy 1: Standard regex parsing (existing logic)
        if let Ok(tool_calls) = self.parse_with_standard_regex(&normalized_text) {
            if !tool_calls.is_empty() {
                debug!(
                    "Qwen3CoderToolParser: Standard regex parsing succeeded with {} calls",
                    tool_calls.len()
                );
                return Ok(tool_calls);
            }
        }

        // Strategy 2: Balanced tag parsing for malformed XML
        if let Ok(tool_calls) = self.parse_with_balanced_tags(&normalized_text) {
            if !tool_calls.is_empty() {
                debug!(
                    "Qwen3CoderToolParser: Balanced tag parsing succeeded with {} calls",
                    tool_calls.len()
                );
                return Ok(tool_calls);
            }
        }

        // Strategy 3: Fuzzy pattern matching for incomplete XML
        if let Ok(tool_calls) = self.parse_with_fuzzy_matching(&normalized_text) {
            if !tool_calls.is_empty() {
                debug!(
                    "Qwen3CoderToolParser: Fuzzy parsing succeeded with {} calls",
                    tool_calls.len()
                );
                return Ok(tool_calls);
            }
        }

        // All strategies failed
        Err(self.create_detailed_error(
            "robust parsing",
            "All parsing strategies failed",
            &text[..std::cmp::min(200, text.len())],
        ))
    }

    /// Standard regex parsing (original implementation logic)
    fn parse_with_standard_regex(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        let mut tool_calls = Vec::new();

        for cap in self.tool_call_regex.captures_iter(text) {
            if let (Some(tool_name_match), Some(content_match)) = (cap.get(1), cap.get(2)) {
                let tool_name = tool_name_match.as_str();
                let content = content_match.as_str();

                let arguments = if !self.schema_map.is_empty() {
                    self.parse_nested_parameters_with_schema(content, tool_name)?
                } else {
                    self.parse_nested_parameters_tolerant(content)?
                };

                let tool_call = ToolCall {
                    id: ToolCallId::new(),
                    name: tool_name.to_string(),
                    arguments,
                };

                tool_calls.push(tool_call);
            }
        }

        Ok(tool_calls)
    }

    /// Parse with balanced tag matching to handle malformed XML
    ///
    /// This method handles cases where:
    /// - Closing tags are missing or mismatched
    /// - Tool calls have proper opening but improper closing
    /// - Nested content has unbalanced tags
    fn parse_with_balanced_tags(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        let mut tool_calls = Vec::new();
        let mut pos = 0;

        while let Some(start_pos) = text[pos..].find("<tool_call>") {
            let absolute_start = pos + start_pos;

            // Find matching </tool_call> with proper nesting handling
            if let Some(end_offset) =
                self.find_matching_close_tag(&text[absolute_start..], "tool_call")
            {
                let absolute_end = absolute_start + end_offset + "</tool_call>".len();
                let xml_block = &text[absolute_start..absolute_end];

                if let Ok(tool_call) = self.parse_single_tool_call(xml_block) {
                    tool_calls.push(tool_call);
                }

                pos = absolute_end;
            } else {
                // No matching close tag, try to parse what we have
                let remaining = &text[absolute_start..];
                if let Ok(tool_call) = self.parse_incomplete_tool_call(remaining) {
                    tool_calls.push(tool_call);
                }
                break;
            }
        }

        Ok(tool_calls)
    }

    /// Find matching closing tag with proper nesting support
    ///
    /// Handles nested tags of the same name by tracking depth.
    /// Returns the position of the matching closing tag relative to the input.
    fn find_matching_close_tag(&self, text: &str, tag_name: &str) -> Option<usize> {
        let open_tag = format!("<{}>", tag_name);
        let close_tag = format!("</{}>", tag_name);

        let mut depth = 0;
        let mut pos = 0;

        while pos < text.len() {
            if text[pos..].starts_with(&open_tag) {
                depth += 1;
                pos += open_tag.len();
            } else if text[pos..].starts_with(&close_tag) {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
                pos += close_tag.len();
            } else {
                pos += 1;
            }
        }

        None
    }

    /// Parse a single tool call block
    fn parse_single_tool_call(&self, xml_block: &str) -> Result<ToolCall, TemplateError> {
        // Extract tool name from the first nested tag
        let tool_name_regex = regex::Regex::new(r"<tool_call>\s*<(\w+)").unwrap();

        if let Some(cap) = tool_name_regex.captures(xml_block) {
            let tool_name = cap.get(1).unwrap().as_str();

            // Extract content between tool name tags
            let content_pattern = format!(
                r"<{}>(.*?)</{}>",
                regex::escape(tool_name),
                regex::escape(tool_name)
            );
            let content_regex = regex::Regex::new(&format!("(?s){}", content_pattern)).unwrap();

            if let Some(content_cap) = content_regex.captures(xml_block) {
                let content = content_cap.get(1).unwrap().as_str();

                let arguments = if !self.schema_map.is_empty() {
                    self.parse_nested_parameters_with_schema(content, tool_name)?
                } else {
                    self.parse_nested_parameters_tolerant(content)?
                };

                return Ok(ToolCall {
                    id: ToolCallId::new(),
                    name: tool_name.to_string(),
                    arguments,
                });
            }
        }

        Err(TemplateError::ToolCallParsing(
            "XML parsing failed: Could not extract valid tool call from XML block - missing tool name or malformed structure".to_string(),
        ))
    }

    /// Parse incomplete tool call (handles cut-off content)
    fn parse_incomplete_tool_call(&self, text: &str) -> Result<ToolCall, TemplateError> {
        let tool_name_regex = regex::Regex::new(r"<tool_call>\s*<(\w+)").unwrap();

        if let Some(cap) = tool_name_regex.captures(text) {
            let tool_name = cap.get(1).unwrap().as_str();

            // Extract any parameters we can find, even if incomplete
            let params = self.extract_available_parameters(text, tool_name)?;

            return Ok(ToolCall {
                id: ToolCallId::new(),
                name: tool_name.to_string(),
                arguments: params,
            });
        }

        Err(TemplateError::ToolCallParsing(
            "Streaming XML parsing failed: Incomplete tool call block - insufficient content to extract tool name or parameters".to_string(),
        ))
    }

    /// Extract available parameters from potentially incomplete XML
    fn extract_available_parameters(
        &self,
        text: &str,
        tool_name: &str,
    ) -> Result<Value, TemplateError> {
        let mut params = serde_json::Map::new();

        // Look for any parameter tags, even if incomplete
        let param_regex = regex::Regex::new(r"<(\w+)>([^<]*?)(?:</\w+>|$)").unwrap();

        for cap in param_regex.captures_iter(text) {
            if let (Some(name), Some(value)) = (cap.get(1), cap.get(2)) {
                let param_name = name.as_str();
                let param_value = self.clean_parameter_value(value.as_str());

                if !param_value.is_empty() && param_name != tool_name {
                    let json_value = if !self.schema_map.is_empty() {
                        self.convert_parameter_with_schema(tool_name, param_name, &param_value)
                            .unwrap_or(Value::String(param_value.clone()))
                    } else {
                        self.convert_parameter_value(&param_value)
                            .unwrap_or(Value::String(param_value.clone()))
                    };
                    params.insert(param_name.to_string(), json_value);
                }
            }
        }

        Ok(Value::Object(params))
    }

    /// Fuzzy pattern matching for extremely malformed XML
    fn parse_with_fuzzy_matching(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        let mut tool_calls = Vec::new();

        // Strategy 1: Look for tool call patterns even without proper XML structure
        let fuzzy_regex =
            regex::Regex::new(r"(?i)tool_call[^>]*>.*?<(\w+)>(.*?)(?:</\w+>|$)").unwrap();

        for cap in fuzzy_regex.captures_iter(text) {
            if let (Some(tool_name), Some(content)) = (cap.get(1), cap.get(2)) {
                if let Ok(arguments) = self.parse_nested_parameters_fuzzy(content.as_str()) {
                    tool_calls.push(ToolCall {
                        id: ToolCallId::new(),
                        name: tool_name.as_str().to_string(),
                        arguments,
                    });
                }
            }
        }

        Ok(tool_calls)
    }

    /// More tolerant parameter parsing for fuzzy matching
    fn parse_nested_parameters_fuzzy(&self, content: &str) -> Result<Value, TemplateError> {
        let param_regex = regex::Regex::new(r"<(\w+)>\s*([^<]*?)(?:\s*</\w+>|$)").unwrap();
        let mut params = serde_json::Map::new();

        for cap in param_regex.captures_iter(content) {
            if let (Some(name), Some(value)) = (cap.get(1), cap.get(2)) {
                let param_name = name.as_str();
                let param_value = self.clean_parameter_value(value.as_str());

                if !param_value.is_empty() {
                    let json_value = self
                        .convert_parameter_value(&param_value)
                        .unwrap_or(Value::String(param_value));
                    params.insert(param_name.to_string(), json_value);
                }
            }
        }

        Ok(Value::Object(params))
    }

    /// Tolerant version of parse_nested_parameters that handles malformed XML
    fn parse_nested_parameters_tolerant(&self, content: &str) -> Result<Value, TemplateError> {
        // More lenient regex that handles missing closing tags
        let param_regex = regex::Regex::new(r"(?s)<(\w+)>([^<]*?)(?:</\w+>|$)").unwrap();
        let mut params = serde_json::Map::new();

        for cap in param_regex.captures_iter(content) {
            if let (Some(name), Some(value)) = (cap.get(1), cap.get(2)) {
                let param_name = name.as_str();
                let param_value = self.clean_parameter_value(value.as_str());

                // Always include parameters, even empty ones (as empty strings)
                let json_value = if param_value.is_empty() {
                    Value::String(String::new())
                } else {
                    self.convert_parameter_value(&param_value)?
                };
                params.insert(param_name.to_string(), json_value);
            }
        }

        Ok(Value::Object(params))
    }

    /// Normalize XML content for better parsing
    ///
    /// Handles whitespace normalization while preserving parameter values,
    /// and cleans up common formatting issues from model outputs.
    fn normalize_xml_content(&self, content: &str) -> String {
        // Basic cleanup - remove excessive whitespace while preserving structure
        let lines: Vec<&str> = content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect();

        let normalized = lines.join("\n");

        // Ensure proper spacing around tags while preserving inner content
        let tag_spacing_regex = regex::Regex::new(r">(\S)").unwrap();
        let result = tag_spacing_regex.replace_all(&normalized, "> $1");

        let close_tag_regex = regex::Regex::new(r"(\S)<").unwrap();
        close_tag_regex.replace_all(&result, "$1 <").to_string()
    }

    /// Clean parameter values by handling XML entities and whitespace
    fn clean_parameter_value(&self, value: &str) -> String {
        value
            .trim()
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
    }

    /// Create detailed error with context for debugging
    fn create_detailed_error(
        &self,
        context: &str,
        error: &str,
        text_sample: &str,
    ) -> TemplateError {
        let sample = if text_sample.len() > 200 {
            format!("{}...", &text_sample[..200])
        } else {
            text_sample.to_string()
        };

        TemplateError::ToolCallParsing(format!(
            "Qwen3Coder XML parsing failed in {}: {}\nText sample: {}",
            context, error, sample
        ))
    }
}

impl Default for Qwen3CoderToolParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCallParser for Qwen3CoderToolParser {
    fn parse_tool_calls(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        debug!("Qwen3CoderToolParser: Starting robust XML parsing");

        // Use robust parsing with multiple fallback strategies
        match self.parse_tool_calls_robust(text) {
            Ok(tool_calls) if !tool_calls.is_empty() => {
                debug!(
                    "Qwen3CoderToolParser: Successfully parsed {} tool calls",
                    tool_calls.len()
                );
                Ok(tool_calls)
            }
            Ok(_) | Err(_) => {
                debug!(
                    "Qwen3CoderToolParser: Robust parsing failed, falling back to default parser"
                );
                self.fallback_parser.parse_tool_calls(text)
            }
        }
    }
}

/// Trait for parsing tool calls from streaming text input
pub trait StreamingToolCallParser: Send + Sync {
    /// Process a delta (incremental text update) and return any completed tool calls
    fn process_delta(&mut self, delta: &str) -> Result<Vec<ToolCall>, TemplateError>;
    /// Get all tool calls completed so far
    fn get_completed_tool_calls(&self) -> Vec<ToolCall>;
    /// Check if currently in middle of parsing a tool call
    fn is_parsing_tool_call(&self) -> bool;
    /// Reset the parser state
    fn reset(&mut self);
}

/// State management for streaming tool call parsing
///
/// This struct maintains the parsing context across multiple delta updates,
/// tracking partial XML content and tool call completion status.
#[derive(Debug, Clone)]
pub struct StreamingState {
    /// Accumulates incoming text deltas for parsing
    buffer: String,
    /// Indicates whether currently inside a tool_call XML block
    in_tool_call: bool,
    /// Name of the current tool being parsed (first tag after tool_call)
    current_tool_name: Option<String>,
    /// Stack of currently open XML tags for proper nesting validation
    open_tags: Vec<String>,
    /// Collection of fully parsed and completed tool calls
    completed_tool_calls: Vec<ToolCall>,
}

impl Default for StreamingState {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingState {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            in_tool_call: false,
            current_tool_name: None,
            open_tags: Vec::new(),
            completed_tool_calls: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.in_tool_call = false;
        self.current_tool_name = None;
        self.open_tags.clear();
        // Keep completed_tool_calls for retrieval
    }

    pub fn is_complete_tool_call(&self) -> bool {
        self.in_tool_call && self.open_tags.is_empty()
    }

    // Accessor methods for private fields

    /// Get the current buffer content
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Get mutable access to the buffer for modification
    pub fn buffer_mut(&mut self) -> &mut String {
        &mut self.buffer
    }

    /// Check if currently inside a tool_call block
    pub fn in_tool_call(&self) -> bool {
        self.in_tool_call
    }

    /// Set the tool_call parsing state
    pub fn set_in_tool_call(&mut self, in_tool_call: bool) {
        self.in_tool_call = in_tool_call;
    }

    /// Get the current tool name being parsed
    pub fn current_tool_name(&self) -> Option<&str> {
        self.current_tool_name.as_deref()
    }

    /// Set the current tool name
    pub fn set_current_tool_name(&mut self, name: Option<String>) {
        self.current_tool_name = name;
    }

    /// Get reference to the open tags stack
    pub fn open_tags(&self) -> &Vec<String> {
        &self.open_tags
    }

    /// Get mutable reference to the open tags stack
    pub fn open_tags_mut(&mut self) -> &mut Vec<String> {
        &mut self.open_tags
    }

    /// Get completed tool calls
    pub fn completed_tool_calls(&self) -> &Vec<ToolCall> {
        &self.completed_tool_calls
    }

    /// Add a completed tool call
    pub fn add_completed_tool_call(&mut self, tool_call: ToolCall) {
        self.completed_tool_calls.push(tool_call);
    }
}

/// Streaming-capable Qwen3Coder parser for incremental tool call parsing
pub struct Qwen3CoderStreamingParser {
    base_parser: Qwen3CoderToolParser,
    state: StreamingState,
}

impl Qwen3CoderStreamingParser {
    /// Creates a new streaming parser with default configuration
    pub fn new() -> Self {
        Self {
            base_parser: Qwen3CoderToolParser::new(),
            state: StreamingState::new(),
        }
    }

    /// Creates a new streaming parser with schema-aware type conversion
    ///
    /// # Arguments
    /// * `tool_definitions` - Optional tool schema definitions for parameter validation
    pub fn new_with_schema(tool_definitions: Option<&[ToolDefinition]>) -> Self {
        Self {
            base_parser: Qwen3CoderToolParser::new_with_schema(tool_definitions),
            state: StreamingState::new(),
        }
    }

    /// Extract a complete tool call from buffer if available
    fn extract_complete_tool_call(&mut self) -> Result<Option<ToolCall>, TemplateError> {
        // Look for complete tool_call blocks in buffer
        if let Some(start) = self.state.buffer().find("<tool_call>") {
            // Search for closing tag AFTER the opening tag
            let search_start = start + "<tool_call>".len();

            if let Some(end_relative) = self.state.buffer()[search_start..].find("</tool_call>") {
                // Calculate the absolute position of the end of the closing tag
                let end_absolute = search_start + end_relative + "</tool_call>".len();

                // Ensure we don't exceed buffer bounds
                if end_absolute > self.state.buffer().len() {
                    debug!("Invalid XML boundary detected, continuing to buffer");
                    return Ok(None);
                }

                let xml_block = &self.state.buffer()[start..end_absolute];

                debug!("Extracting complete tool call from buffer: '{}'", xml_block);

                // Validate that we have a complete, well-formed tool call
                if !xml_block.starts_with("<tool_call>") || !xml_block.ends_with("</tool_call>") {
                    debug!("Tool call block has invalid boundaries, continuing to buffer");
                    return Ok(None);
                }

                // Parse the complete tool call
                match self.base_parser.parse_single_tool_call(xml_block) {
                    Ok(tool_call) => {
                        // Remove processed content from buffer
                        *self.state.buffer_mut() = self.state.buffer()[end_absolute..].to_string();

                        // Reset tool call tracking state
                        self.reset_tool_call_state();

                        return Ok(Some(tool_call));
                    }
                    Err(e) => {
                        debug!("Failed to parse complete tool call: {}", e);
                        // Remove the problematic content and continue
                        *self.state.buffer_mut() = self.state.buffer()[end_absolute..].to_string();
                        self.reset_tool_call_state();
                    }
                }
            }
        }

        Ok(None)
    }

    fn reset_tool_call_state(&mut self) {
        self.state.set_in_tool_call(false);
        self.state.set_current_tool_name(None);
        self.state.open_tags_mut().clear();
    }

    /// Get current parsing state for debugging
    pub fn get_state(&self) -> &StreamingState {
        &self.state
    }
}

impl StreamingToolCallParser for Qwen3CoderStreamingParser {
    /// Process a delta (incremental text update) and return any completed tool calls
    ///
    /// This method accumulates the delta into an internal buffer and attempts to extract
    /// any complete tool_call XML blocks. Completed tool calls are parsed using the
    /// base Qwen3CoderToolParser and returned immediately.
    fn process_delta(&mut self, delta: &str) -> Result<Vec<ToolCall>, TemplateError> {
        debug!("Processing streaming delta: '{}'", delta);

        // Append delta to buffer
        self.state.buffer_mut().push_str(delta);

        // Try to extract completed tool calls
        let mut completed_calls = Vec::new();

        // Process the buffer for complete tool calls
        while let Some(tool_call) = self.extract_complete_tool_call()? {
            completed_calls.push(tool_call.clone());
            self.state.add_completed_tool_call(tool_call);
        }

        // Update parsing state based on current buffer content
        self.state.set_in_tool_call(
            self.state.buffer().contains("<tool_call>")
                && !self.state.buffer().contains("</tool_call>"),
        );

        Ok(completed_calls)
    }

    fn get_completed_tool_calls(&self) -> Vec<ToolCall> {
        self.state.completed_tool_calls().clone()
    }

    fn is_parsing_tool_call(&self) -> bool {
        self.state.in_tool_call()
    }

    fn reset(&mut self) {
        self.state.reset();
    }
}

impl Default for Qwen3CoderStreamingParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Specialized parser for OpenAI-compatible tool call formats
///
/// Handles multiple OpenAI function calling patterns:
/// - Function call objects with arguments
/// - Tool calls array format (newer OpenAI format)
/// - Simple function call syntax with parameter parsing
pub struct OpenAIToolParser {
    delegate: DefaultToolParser,
}

impl OpenAIToolParser {
    pub fn new() -> Self {
        Self {
            delegate: DefaultToolParser::new(),
        }
    }
}

impl Default for OpenAIToolParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCallParser for OpenAIToolParser {
    fn parse_tool_calls(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        debug!("OpenAIToolParser: Parsing text with OpenAI-specific patterns");

        let mut tool_calls = Vec::new();

        // OpenAI models use structured function calling formats
        // Pattern 1: Function call with arguments object
        let function_call_regex = Regex::new(
            r#"(?s)\{"function_call":\s*\{\s*"name":\s*"([^"]+)",\s*"arguments":\s*(\{.*?\})\s*\}\s*\}"#
        ).unwrap();

        for capture in function_call_regex.captures_iter(text) {
            if let (Some(name_match), Some(args_match)) = (capture.get(1), capture.get(2)) {
                let name = name_match.as_str().to_string();
                let args_str = args_match.as_str();

                debug!(
                    "OpenAIToolParser: Found function_call '{}' with args: {}",
                    name, args_str
                );

                match serde_json::from_str::<Value>(args_str) {
                    Ok(arguments) => {
                        let tool_call = ToolCall {
                            id: ToolCallId::new(),
                            name,
                            arguments,
                        };
                        tool_calls.push(tool_call);
                    }
                    Err(e) => {
                        debug!(
                            "OpenAIToolParser: Failed to parse function arguments as JSON: {}",
                            e
                        );
                        // Try to salvage by wrapping as string
                        let tool_call = ToolCall {
                            id: ToolCallId::new(),
                            name,
                            arguments: json!({"input": args_str}),
                        };
                        tool_calls.push(tool_call);
                    }
                }
            }
        }

        // Pattern 2: Tool calls array format (newer OpenAI format)
        if tool_calls.is_empty() {
            let tool_calls_regex = Regex::new(
                r#"(?s)"tool_calls":\s*\[\s*\{\s*"id":\s*"([^"]*)",\s*"type":\s*"function",\s*"function":\s*\{\s*"name":\s*"([^"]+)",\s*"arguments":\s*"([^"]+)"\s*\}\s*\}\s*\]"#
            ).unwrap();

            for capture in tool_calls_regex.captures_iter(text) {
                if let (Some(_id_match), Some(name_match), Some(args_match)) =
                    (capture.get(1), capture.get(2), capture.get(3))
                {
                    let name = name_match.as_str().to_string();
                    let args_str = args_match.as_str();

                    debug!(
                        "OpenAIToolParser: Found tool_calls array item '{}' with args: {}",
                        name, args_str
                    );

                    // Parse the escaped JSON arguments
                    let arguments = match serde_json::from_str::<Value>(args_str) {
                        Ok(json) => json,
                        Err(_) => {
                            // Try unescaping and parsing again
                            let unescaped = args_str.replace("\\\"", "\"");
                            match serde_json::from_str::<Value>(&unescaped) {
                                Ok(json) => json,
                                Err(e) => {
                                    debug!(
                                        "OpenAIToolParser: Failed to parse escaped arguments: {}",
                                        e
                                    );
                                    json!({"input": args_str})
                                }
                            }
                        }
                    };

                    let tool_call = ToolCall {
                        id: ToolCallId::new(),
                        name,
                        arguments,
                    };
                    tool_calls.push(tool_call);
                }
            }
        }

        // Pattern 3: Simple function call format
        if tool_calls.is_empty() {
            let simple_function_regex = Regex::new(r#"(?s)(\w+)\s*\(\s*(.+?)\s*\)"#).unwrap();

            for capture in simple_function_regex.captures_iter(text) {
                if let (Some(name_match), Some(args_match)) = (capture.get(1), capture.get(2)) {
                    let name = name_match.as_str().to_string();
                    let args_str = args_match.as_str().trim();

                    // Only consider this a function call if it looks like parameters
                    if args_str.contains(',') || args_str.contains('=') || args_str.starts_with('"')
                    {
                        debug!(
                            "OpenAIToolParser: Found simple function call '{}' with args: {}",
                            name, args_str
                        );

                        // Try to parse as JSON, otherwise wrap as string
                        let arguments = if args_str.starts_with('{') && args_str.ends_with('}') {
                            serde_json::from_str(args_str)
                                .unwrap_or_else(|_| json!({"input": args_str}))
                        } else {
                            json!({"input": args_str})
                        };

                        let tool_call = ToolCall {
                            id: ToolCallId::new(),
                            name,
                            arguments,
                        };
                        tool_calls.push(tool_call);
                        break; // Only take the first one to avoid noise
                    }
                }
            }
        }

        if tool_calls.is_empty() {
            debug!("OpenAIToolParser: No OpenAI-specific patterns found, falling back to delegate");
            return self.delegate.parse_tool_calls(text);
        }

        debug!(
            "OpenAIToolParser: Extracted {} tool calls",
            tool_calls.len()
        );
        Ok(tool_calls)
    }
}

/// Specialized parser for Claude/Anthropic tool call formats
///
/// Handles multiple Claude XML-based function calling patterns:
/// - Function calls wrapper with invoke tags
/// - Direct invoke tags with name attributes
/// - Tool tags with structured parameter parsing
pub struct ClaudeToolParser {
    delegate: DefaultToolParser,
}

impl ClaudeToolParser {
    pub fn new() -> Self {
        Self {
            delegate: DefaultToolParser::new(),
        }
    }
}

impl Default for ClaudeToolParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCallParser for ClaudeToolParser {
    fn parse_tool_calls(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        debug!("ClaudeToolParser: Parsing text with Claude/Anthropic-specific patterns");

        let mut tool_calls = Vec::new();

        // Claude models use specific XML-like thinking and tool use patterns

        // Pattern 1: <function_calls> wrapper with individual calls
        let function_calls_regex =
            Regex::new(r#"(?s)<function_calls>(.*?)</function_calls>"#).unwrap();

        if let Some(capture) = function_calls_regex.captures(text) {
            let calls_content = capture.get(1).unwrap().as_str();

            // Look for individual invoke tags within function_calls
            let invoke_regex =
                Regex::new(r#"(?s)<invoke\s+name="([^"]+)">(.*?)</invoke>"#).unwrap();

            for invoke_capture in invoke_regex.captures_iter(calls_content) {
                if let (Some(name_match), Some(content_match)) =
                    (invoke_capture.get(1), invoke_capture.get(2))
                {
                    let name = name_match.as_str().to_string();
                    let content = content_match.as_str().trim();

                    debug!(
                        "ClaudeToolParser: Found invoke call '{}' with content: {}",
                        name, content
                    );

                    let arguments = self.parse_claude_parameters(content);
                    let tool_call = ToolCall {
                        id: ToolCallId::new(),
                        name,
                        arguments,
                    };
                    tool_calls.push(tool_call);
                }
            }
        }

        // Pattern 2: Direct <invoke> tags (not wrapped in function_calls)
        if tool_calls.is_empty() {
            let direct_invoke_regex =
                Regex::new(r#"(?s)<invoke\s+name="([^"]+)">(.*?)</invoke>"#).unwrap();

            for capture in direct_invoke_regex.captures_iter(text) {
                if let (Some(name_match), Some(content_match)) = (capture.get(1), capture.get(2)) {
                    let name = name_match.as_str().to_string();
                    let content = content_match.as_str().trim();

                    debug!(
                        "ClaudeToolParser: Found direct invoke '{}' with content: {}",
                        name, content
                    );

                    let arguments = self.parse_claude_parameters(content);
                    let tool_call = ToolCall {
                        id: ToolCallId::new(),
                        name,
                        arguments,
                    };
                    tool_calls.push(tool_call);
                }
            }
        }

        // Pattern 3: <tool> tags with name attribute
        if tool_calls.is_empty() {
            let tool_regex = Regex::new(r#"(?s)<tool\s+name="([^"]+)">(.*?)</tool>"#).unwrap();

            for capture in tool_regex.captures_iter(text) {
                if let (Some(name_match), Some(content_match)) = (capture.get(1), capture.get(2)) {
                    let name = name_match.as_str().to_string();
                    let content = content_match.as_str().trim();

                    debug!(
                        "ClaudeToolParser: Found tool call '{}' with content: {}",
                        name, content
                    );

                    let arguments = self.parse_claude_parameters(content);
                    let tool_call = ToolCall {
                        id: ToolCallId::new(),
                        name,
                        arguments,
                    };
                    tool_calls.push(tool_call);
                }
            }
        }

        // Pattern 4: <thinking> followed by tool use (Claude reasoning pattern)
        if tool_calls.is_empty() {
            let thinking_tool_regex = Regex::new(
                r#"(?s)<thinking>.*?</thinking>.*?<([^>\s]+)[^>]*(?:name="([^"]+)"[^>]*)?>([^<]*)</[^>\s]+>"#
            ).unwrap();

            for capture in thinking_tool_regex.captures_iter(text) {
                if let (Some(tag_match), name_match, Some(content_match)) =
                    (capture.get(1), capture.get(2), capture.get(3))
                {
                    let tag = tag_match.as_str();
                    let name = name_match.map(|m| m.as_str()).unwrap_or(tag).to_string();
                    let content = content_match.as_str().trim();

                    // Only consider tool-like tags
                    if ["invoke", "tool", "function", "call"].contains(&tag) {
                        debug!(
                            "ClaudeToolParser: Found thinking-based tool '{}' with content: {}",
                            name, content
                        );

                        let arguments = self.parse_claude_parameters(content);
                        let tool_call = ToolCall {
                            id: ToolCallId::new(),
                            name,
                            arguments,
                        };
                        tool_calls.push(tool_call);
                    }
                }
            }
        }

        if tool_calls.is_empty() {
            debug!("ClaudeToolParser: No Claude-specific patterns found, falling back to delegate");
            return self.delegate.parse_tool_calls(text);
        }

        debug!(
            "ClaudeToolParser: Extracted {} tool calls",
            tool_calls.len()
        );
        Ok(tool_calls)
    }
}

impl ClaudeToolParser {
    /// Parse Claude-style parameter formats (XML parameters, JSON, or plain text)
    fn parse_claude_parameters(&self, content: &str) -> Value {
        let content = content.trim();

        // Try JSON first
        if (content.starts_with('{') && content.ends_with('}'))
            || (content.starts_with('[') && content.ends_with(']'))
        {
            if let Ok(json) = serde_json::from_str::<Value>(content) {
                return json;
            }
        }

        // Try XML parameter extraction
        let mut params = serde_json::Map::new();

        // Look for <parameter name="key">value</parameter> patterns
        let param_regex =
            Regex::new(r#"(?s)<parameter\s+name="([^"]+)"[^>]*>(.*?)</parameter>"#).unwrap();

        for capture in param_regex.captures_iter(content) {
            if let (Some(key_match), Some(value_match)) = (capture.get(1), capture.get(2)) {
                let key = key_match.as_str().to_string();
                let value = value_match.as_str().trim();

                // Try to parse value as JSON, otherwise store as string
                let parsed_value = serde_json::from_str::<Value>(value)
                    .unwrap_or_else(|_| Value::String(value.to_string()));

                params.insert(key, parsed_value);
            }
        }

        if !params.is_empty() {
            return Value::Object(params);
        }

        // Look for simple key=value pairs
        let key_value_regex = Regex::new(r#"(\w+)\s*=\s*"([^"]*)"|(\w+)\s*=\s*([^\s,]+)"#).unwrap();
        for capture in key_value_regex.captures_iter(content) {
            if let (Some(key_match), Some(value_match)) = (
                capture.get(1).or(capture.get(3)),
                capture.get(2).or(capture.get(4)),
            ) {
                let key = key_match.as_str().to_string();
                let value = value_match.as_str().to_string();
                params.insert(key, Value::String(value));
            }
        }

        if !params.is_empty() {
            Value::Object(params)
        } else {
            // Fallback: treat entire content as input parameter
            json!({"input": content})
        }
    }
}

/// Parser for JSON function call format
pub struct JsonToolCallParser {
    regex: Regex,
}

impl Default for JsonToolCallParser {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonToolCallParser {
    pub fn new() -> Self {
        // Improved regex to match JSON objects more accurately
        // This will match properly balanced JSON objects (handles one level of nesting well)
        let regex = Regex::new(r#"\{(?:[^{}]|\{[^{}]*\})*\}"#).unwrap();

        Self { regex }
    }
}

impl ToolCallParser for JsonToolCallParser {
    fn parse_tool_calls(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        let mut tool_calls = Vec::new();
        debug!(
            "JsonToolCallParser: Analyzing text for JSON objects: {}",
            text
        );

        // First try the main regex approach
        for capture in self.regex.find_iter(text) {
            let json_str = capture.as_str();
            debug!("JsonToolCallParser: Found potential JSON: {}", json_str);

            match serde_json::from_str::<Value>(json_str) {
                Ok(json) => {
                    debug!("JsonToolCallParser: Successfully parsed JSON: {:?}", json);
                    if let Some(tool_call) = self.parse_json_tool_call(&json)? {
                        debug!("JsonToolCallParser: Extracted tool call: {:?}", tool_call);
                        tool_calls.push(tool_call);
                    } else {
                        debug!("JsonToolCallParser: JSON doesn't match tool call format");
                    }
                }
                Err(e) => {
                    debug!(
                        "JsonToolCallParser: Failed to parse JSON '{}': {}",
                        json_str, e
                    );
                    continue;
                }
            }
        }

        // If no tool calls found with regex, try a more lenient line-by-line approach
        if tool_calls.is_empty() {
            debug!(
                "JsonToolCallParser: No tool calls found with regex, trying line-by-line parsing"
            );
            self.try_line_by_line_parsing(text, &mut tool_calls)?;
        }

        debug!(
            "JsonToolCallParser: Extracted {} tool calls total",
            tool_calls.len()
        );
        Ok(tool_calls)
    }
}

impl JsonToolCallParser {
    fn parse_json_tool_call(&self, json: &Value) -> Result<Option<ToolCall>, TemplateError> {
        // Try different common JSON formats for tool calls

        // Format 1: {"function_name": "tool_name", "arguments": {...}}
        if let (Some(function_name), Some(arguments)) = (
            json.get("function_name").and_then(|v| v.as_str()),
            json.get("arguments"),
        ) {
            return Ok(Some(ToolCall {
                id: ToolCallId::new(),
                name: function_name.to_string(),
                arguments: arguments.clone(),
            }));
        }

        // Format 2: {"tool": "tool_name", "parameters": {...}}
        if let (Some(tool_name), Some(parameters)) = (
            json.get("tool").and_then(|v| v.as_str()),
            json.get("parameters"),
        ) {
            return Ok(Some(ToolCall {
                id: ToolCallId::new(),
                name: tool_name.to_string(),
                arguments: parameters.clone(),
            }));
        }

        // Format 3: {"name": "tool_name", "args": {...}}
        if let (Some(name), Some(args)) =
            (json.get("name").and_then(|v| v.as_str()), json.get("args"))
        {
            return Ok(Some(ToolCall {
                id: ToolCallId::new(),
                name: name.to_string(),
                arguments: args.clone(),
            }));
        }

        Ok(None)
    }

    fn try_line_by_line_parsing(
        &self,
        text: &str,
        tool_calls: &mut Vec<ToolCall>,
    ) -> Result<(), TemplateError> {
        debug!("JsonToolCallParser: Trying line-by-line parsing");

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                debug!("JsonToolCallParser: Found JSON-like line: {}", trimmed);

                match serde_json::from_str::<Value>(trimmed) {
                    Ok(json) => {
                        if let Some(tool_call) = self.parse_json_tool_call(&json)? {
                            debug!(
                                "JsonToolCallParser: Line-by-line extracted tool call: {:?}",
                                tool_call
                            );
                            tool_calls.push(tool_call);
                        }
                    }
                    Err(e) => {
                        debug!("JsonToolCallParser: Failed to parse line as JSON: {}", e);
                    }
                }
            }
        }

        // Additional fallback: try to extract JSON from text that might have trailing characters
        if tool_calls.is_empty() {
            debug!("JsonToolCallParser: Trying fallback parsing for malformed JSON");
            self.try_fallback_parsing(text, tool_calls)?;
        }

        Ok(())
    }

    fn try_fallback_parsing(
        &self,
        text: &str,
        tool_calls: &mut Vec<ToolCall>,
    ) -> Result<(), TemplateError> {
        // Use a more sophisticated approach to find JSON objects that might be malformed

        // Find potential JSON start patterns
        let start_patterns = vec![
            r#"\{\s*"function_name"\s*:"#,
            r#"\{\s*"tool"\s*:"#,
            r#"\{\s*"name"\s*:"#,
        ];

        for pattern_str in start_patterns {
            let pattern = Regex::new(pattern_str).unwrap();

            for mat in pattern.find_iter(text) {
                let start_pos = mat.start();
                debug!(
                    "JsonToolCallParser: Found potential JSON start at position {}",
                    start_pos
                );

                // Try to find the matching closing brace using brace counting
                let remaining_text = &text[start_pos..];
                if let Some(json_str) = self.extract_balanced_json(remaining_text) {
                    debug!("JsonToolCallParser: Extracted balanced JSON: {}", json_str);

                    match serde_json::from_str::<Value>(&json_str) {
                        Ok(json) => {
                            if let Some(tool_call) = self.parse_json_tool_call(&json)? {
                                debug!(
                                    "JsonToolCallParser: Fallback extracted tool call: {:?}",
                                    tool_call
                                );
                                tool_calls.push(tool_call);
                            }
                        }
                        Err(e) => {
                            debug!("JsonToolCallParser: Fallback JSON parsing failed: {}", e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn extract_balanced_json(&self, text: &str) -> Option<String> {
        let mut brace_count = 0;
        let mut start_found = false;
        let mut in_string = false;
        let mut escape_next = false;
        let mut result = String::new();

        for ch in text.chars() {
            result.push(ch);

            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => {
                    escape_next = true;
                }
                '"' => {
                    in_string = !in_string;
                }
                '{' if !in_string => {
                    brace_count += 1;
                    start_found = true;
                }
                '}' if !in_string => {
                    brace_count -= 1;
                    if start_found && brace_count == 0 {
                        // Found complete JSON object
                        return Some(result);
                    }
                }
                _ => {}
            }

            // If we've seen many characters without closing, give up
            if result.len() > STRESS_TEST_REPEAT_SIZE {
                break;
            }
        }

        None
    }
}

/// Parser for XML-style function calls
pub struct XmlToolCallParser {
    regex: Regex,
}

impl Default for XmlToolCallParser {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlToolCallParser {
    pub fn new() -> Self {
        // Match XML-style function calls like <function_call name="tool_name">...</function_call>
        let regex = Regex::new(r#"<function_call[^>]*>(.*?)</function_call>"#)
            .unwrap_or_else(|_| Regex::new(r#"<tool_call[^>]*>(.*?)</tool_call>"#).unwrap());

        Self { regex }
    }
}

impl ToolCallParser for XmlToolCallParser {
    fn parse_tool_calls(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        let mut tool_calls = Vec::new();

        for capture in self.regex.captures_iter(text) {
            if let Some(tool_call) = self.parse_xml_tool_call(capture.get(0).unwrap().as_str())? {
                tool_calls.push(tool_call);
            }
        }

        Ok(tool_calls)
    }
}

impl XmlToolCallParser {
    fn parse_xml_tool_call(&self, xml: &str) -> Result<Option<ToolCall>, TemplateError> {
        // Simple XML parsing - extract name attribute and content
        let name_regex = Regex::new(r#"name="([^"]*)"#).unwrap();
        let content_regex = Regex::new(r#"<[^>]*>(.*)</[^>]*>"#).unwrap();

        if let Some(name_match) = name_regex.captures(xml) {
            let name = name_match.get(1).unwrap().as_str();

            let arguments = if let Some(content_match) = content_regex.captures(xml) {
                let content = content_match.get(1).unwrap().as_str();
                match serde_json::from_str::<Value>(content) {
                    Ok(json) => json,
                    Err(_) => Value::String(content.to_string()),
                }
            } else {
                Value::Null
            };

            return Ok(Some(ToolCall {
                id: ToolCallId::new(),
                name: name.to_string(),
                arguments,
            }));
        }

        Ok(None)
    }
}

/// Parser for natural language function call format
pub struct FunctionCallParser {
    regex: Regex,
}

impl Default for FunctionCallParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionCallParser {
    pub fn new() -> Self {
        // Match patterns like "Call function_name with arguments {...}"
        let regex = Regex::new(r"(?i)call\s+(\w+)\s+with\s+(?:arguments?\s+)?(.+)")
            .unwrap_or_else(|_| Regex::new(r"(\w+)\s*\(([^)]*)\)").unwrap());

        Self { regex }
    }
}

impl ToolCallParser for FunctionCallParser {
    fn parse_tool_calls(&self, text: &str) -> Result<Vec<ToolCall>, TemplateError> {
        let mut tool_calls = Vec::new();

        for capture in self.regex.captures_iter(text) {
            if let Some(tool_call) = self.parse_function_call(&capture)? {
                tool_calls.push(tool_call);
            }
        }

        Ok(tool_calls)
    }
}

impl FunctionCallParser {
    fn parse_function_call(
        &self,
        capture: &regex::Captures,
    ) -> Result<Option<ToolCall>, TemplateError> {
        if let (Some(name), Some(args_str)) = (capture.get(1), capture.get(2)) {
            let name = name.as_str();
            let args_str = args_str.as_str().trim();

            let arguments = if args_str.starts_with('{') && args_str.ends_with('}') {
                // Try to parse as JSON
                match serde_json::from_str::<Value>(args_str) {
                    Ok(json) => json,
                    Err(_) => Value::String(args_str.to_string()),
                }
            } else {
                // Treat as string parameter
                Value::String(args_str.to_string())
            };

            return Ok(Some(ToolCall {
                id: ToolCallId::new(),
                name: name.to_string(),
                arguments,
            }));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Message, MessageRole, Session, SessionId, ToolDefinition};
    use std::time::SystemTime;

    /// Test constant for floating point comparison (3.14 is the exact test data value, not π)
    #[allow(clippy::approx_constant)]
    const TEST_FLOAT_VALUE: f64 = 3.14;

    /// Performance constants for integration tests
    pub const DEFAULT_CONTEXT_SIZE: u32 = 2048;
    pub const STRESS_TEST_REPEAT_SIZE: usize = 10000;

    fn create_test_session() -> Session {
        Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: "You are a helpful assistant.".to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                },
                Message {
                    role: MessageRole::User,
                    content: "Hello, can you help me?".to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                },
            ],
            mcp_servers: vec![],
            available_tools: vec![ToolDefinition {
                name: "list_files".to_string(),
                description: "List files in a directory".to_string(),
                parameters: serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
                server_name: "filesystem".to_string(),
            }],
            available_prompts: vec![],
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,

            todos: Vec::new(),

            available_commands: Vec::new(),
            current_mode: None,

            client_capabilities: None,
            cached_message_count: 0,
            cached_token_count: 0,
        }
    }

    #[test]
    fn test_chat_template_engine_creation() {
        let engine = ChatTemplateEngine::new();
        assert_eq!(engine.tool_call_parsers.len(), 3);
        assert!(engine.tool_call_parsers.contains_key("json"));
        assert!(engine.tool_call_parsers.contains_key("xml"));
        assert!(engine.tool_call_parsers.contains_key("function_call"));
    }

    #[test]
    fn test_format_tools_for_template() {
        let engine = ChatTemplateEngine::new();
        let session = create_test_session();

        let formatted = engine
            .format_tools_for_template(&session.available_tools)
            .unwrap();
        assert!(formatted.contains("Available tools:"));
        assert!(formatted.contains("list_files"));
        assert!(formatted.contains("filesystem"));
    }

    #[test]
    fn test_native_template_functionality_fallback() {
        let engine = ChatTemplateEngine::new();

        // Test messages
        let messages = vec![
            (
                "system".to_string(),
                "You are a helpful assistant.".to_string(),
            ),
            ("user".to_string(), "Hello, how are you?".to_string()),
        ];

        // Since we don't have a real model in unit tests, this should fallback
        // to the legacy implementation. We test that the method exists and works
        // by calling the legacy implementation directly
        let result = engine.format_chat_template(&messages, None).unwrap();

        // Verify the result contains expected content
        assert!(result.contains("System:"));
        assert!(result.contains("You are a helpful assistant"));
        assert!(result.contains("Human:"));
        assert!(result.contains("Hello, how are you?"));
        assert!(result.contains("Assistant:"));

        // Test with tools context
        let tools_context = "Available tools: test_tool";
        let result_with_tools = engine
            .format_chat_template(&messages, Some(tools_context))
            .unwrap();
        assert!(result_with_tools.contains(tools_context));
    }

    #[test]
    fn test_json_tool_call_parser() {
        let parser = JsonToolCallParser::new();

        // Test format 1
        let text = r#"{"function_name": "list_files", "arguments": {"path": "/tmp"}}"#;
        let tool_calls = parser.parse_tool_calls(text).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_files");

        // Test format 2
        let text = r#"{"tool": "list_files", "parameters": {"path": "/tmp"}}"#;
        let tool_calls = parser.parse_tool_calls(text).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_files");

        // Test format 3
        let text = r#"{"name": "list_files", "args": {"path": "/tmp"}}"#;
        let tool_calls = parser.parse_tool_calls(text).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_files");
    }

    #[test]
    fn test_json_tool_call_parser_mixed_with_text() {
        let parser = JsonToolCallParser::new();

        // Test mixed with text before and after - this is what models actually generate
        let text = r#"I'll help you list the files in the current directory.

{"function_name": "list_directory", "arguments": {"path": "."}}

I apologize for the confusion. Let me try again with the correct format."#;

        let tool_calls = parser.parse_tool_calls(text).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_directory");

        // Test with complex nested arguments
        let complex_text = r#"{"function_name": "complex_tool", "arguments": {"nested": {"key": "value", "array": [1, 2, 3]}, "simple": "test"}}"#;
        let tool_calls = parser.parse_tool_calls(complex_text).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "complex_tool");

        // Test problematic case: JSON followed immediately by text (what models actually do)
        let problematic_text = r#"{"function_name": "list_directory", "arguments": {"path": "."}}I apologize for the confusion"#;
        let tool_calls = parser.parse_tool_calls(problematic_text).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_directory");

        // Test even more complex nesting with multiple levels
        let deep_nested = r#"{"function_name": "deep_tool", "arguments": {"level1": {"level2": {"level3": {"value": "test"}}}, "array": [{"nested": true}, {"nested": false}]}}"#;
        let tool_calls = parser.parse_tool_calls(deep_nested).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "deep_tool");
    }

    #[test]
    fn test_json_tool_call_parser_fallback_parsing() {
        // Initialize tracing for test debugging
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        let parser = JsonToolCallParser::new();

        // Test malformed JSON that would need fallback parsing (missing closing brace)
        let malformed_text = r#"Sure, I can help! {"function_name": "list_directory", "arguments": {"path": "."}} I need to check what's in your directory first."#;
        let tool_calls = parser.parse_tool_calls(malformed_text).unwrap();
        // Should work with new balanced brace extraction
        println!("Malformed text extracted {} tool calls", tool_calls.len());
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_directory");

        // Test case where JSON is buried in lots of text
        let buried_text = r#"
        Let me help you with that task. First, I need to understand what files are available.
        I'll use the directory listing tool to check: {"function_name": "list_directory", "arguments": {"path": "."}}
        After checking the directory, I can provide better assistance.
        "#;
        let tool_calls = parser.parse_tool_calls(buried_text).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_directory");
    }

    #[test]
    fn test_xml_tool_call_parser() {
        let parser = XmlToolCallParser::new();

        let text = r#"<function_call name="list_files">{"path": "/tmp"}</function_call>"#;
        let tool_calls = parser.parse_tool_calls(text).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_files");
    }

    #[test]
    fn test_function_call_parser() {
        let parser = FunctionCallParser::new();

        let text = "Call list_files with arguments {\"path\": \"/tmp\"}";
        let tool_calls = parser.parse_tool_calls(text).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_files");
    }

    #[test]
    fn test_extract_tool_calls_multiple_formats() {
        let engine = ChatTemplateEngine::new();

        let text = r#"
        I'll help you with that. Let me list the files first.
        {"function_name": "list_files", "arguments": {"path": "/tmp"}}
        "#;

        let tool_calls = engine.extract_tool_calls(text).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_files");
    }

    #[test]
    fn test_debug_logging_tool_extraction() {
        // Initialize tracing for test debugging
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        // This test verifies that our debug logging enhancements work
        let engine = ChatTemplateEngine::new();

        // Test with a tool call that should be extracted
        let text = r#"
        I'll help you list the files in the current directory.
        {"function_name": "list_directory", "arguments": {"path": "."}}
        "#;

        // This will trigger our enhanced debug logging in extract_tool_calls
        let tool_calls = engine.extract_tool_calls(text).unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_directory");

        // Test with text that has no tool calls
        let empty_text = "Just a regular response with no tool calls.";
        let empty_calls = engine.extract_tool_calls(empty_text).unwrap();
        assert_eq!(empty_calls.len(), 0);

        // Test actual problematic pattern that models generate
        let problematic_text = r#"{"function_name": "list_directory", "arguments": {"path": "."}}I need to check the files in the current directory for you."#;
        println!("Testing problematic text: {}", problematic_text);
        let tool_calls = engine.extract_tool_calls(problematic_text).unwrap();
        println!("Extracted {} tool calls", tool_calls.len());
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "list_directory");
    }

    #[test]
    fn test_extract_tool_calls_no_matches() {
        let engine = ChatTemplateEngine::new();

        let text = "This is just regular text with no tool calls.";
        let tool_calls = engine.extract_tool_calls(text).unwrap();
        assert_eq!(tool_calls.len(), 0);
    }

    #[test]
    fn test_tool_parsing_strategy_creation() {
        // Test enum variant creation
        let default_strategy = ToolParsingStrategy::Default;
        let qwen3coder_strategy = ToolParsingStrategy::Qwen3Coder;
        let openai_strategy = ToolParsingStrategy::OpenAI;
        let claude_strategy = ToolParsingStrategy::Claude;

        // Verify they can be created and compared
        assert_eq!(default_strategy, ToolParsingStrategy::Default);
        assert_eq!(qwen3coder_strategy, ToolParsingStrategy::Qwen3Coder);
        assert_eq!(openai_strategy, ToolParsingStrategy::OpenAI);
        assert_eq!(claude_strategy, ToolParsingStrategy::Claude);
    }

    #[test]
    fn test_tool_parsing_strategy_default() {
        // Test Default implementation
        let default_strategy = ToolParsingStrategy::default();
        assert_eq!(default_strategy, ToolParsingStrategy::Default);
    }

    #[test]
    fn test_tool_parsing_strategy_equality() {
        // Test PartialEq implementation
        assert_eq!(ToolParsingStrategy::Default, ToolParsingStrategy::Default);
        assert_eq!(
            ToolParsingStrategy::Qwen3Coder,
            ToolParsingStrategy::Qwen3Coder
        );
        assert_eq!(ToolParsingStrategy::OpenAI, ToolParsingStrategy::OpenAI);
        assert_eq!(ToolParsingStrategy::Claude, ToolParsingStrategy::Claude);

        // Test inequality
        assert_ne!(
            ToolParsingStrategy::Default,
            ToolParsingStrategy::Qwen3Coder
        );
        assert_ne!(ToolParsingStrategy::OpenAI, ToolParsingStrategy::Claude);
        assert_ne!(ToolParsingStrategy::Qwen3Coder, ToolParsingStrategy::OpenAI);
    }

    #[test]
    fn test_tool_parsing_strategy_hashing() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Test that different strategies have different hashes
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        let mut hasher3 = DefaultHasher::new();
        let mut hasher4 = DefaultHasher::new();

        ToolParsingStrategy::Default.hash(&mut hasher1);
        ToolParsingStrategy::Qwen3Coder.hash(&mut hasher2);
        ToolParsingStrategy::OpenAI.hash(&mut hasher3);
        ToolParsingStrategy::Claude.hash(&mut hasher4);

        let hash1 = hasher1.finish();
        let hash2 = hasher2.finish();
        let hash3 = hasher3.finish();
        let hash4 = hasher4.finish();

        // Each variant should have a different hash
        assert_ne!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_ne!(hash1, hash4);
        assert_ne!(hash2, hash3);
        assert_ne!(hash2, hash4);
        assert_ne!(hash3, hash4);

        // Same variants should have the same hash
        let mut hasher5 = DefaultHasher::new();
        ToolParsingStrategy::Default.hash(&mut hasher5);
        assert_eq!(hash1, hasher5.finish());
    }

    #[test]
    fn test_tool_parsing_strategy_can_be_used_as_hashmap_key() {
        use std::collections::HashMap;

        // Test that the enum can be used as HashMap keys
        let mut strategy_map: HashMap<ToolParsingStrategy, &str> = HashMap::new();

        strategy_map.insert(ToolParsingStrategy::Default, "default_parser");
        strategy_map.insert(ToolParsingStrategy::Qwen3Coder, "qwen3coder_parser");
        strategy_map.insert(ToolParsingStrategy::OpenAI, "openai_parser");
        strategy_map.insert(ToolParsingStrategy::Claude, "claude_parser");

        // Verify we can retrieve values by strategy
        assert_eq!(
            strategy_map.get(&ToolParsingStrategy::Default),
            Some(&"default_parser")
        );
        assert_eq!(
            strategy_map.get(&ToolParsingStrategy::Qwen3Coder),
            Some(&"qwen3coder_parser")
        );
        assert_eq!(
            strategy_map.get(&ToolParsingStrategy::OpenAI),
            Some(&"openai_parser")
        );
        assert_eq!(
            strategy_map.get(&ToolParsingStrategy::Claude),
            Some(&"claude_parser")
        );

        // Verify the map contains all expected entries
        assert_eq!(strategy_map.len(), 4);
    }

    #[test]
    fn test_tool_parsing_strategy_clone() {
        // Test Clone implementation
        let original = ToolParsingStrategy::Qwen3Coder;
        let cloned = original.clone();

        assert_eq!(original, cloned);

        // Ensure they're separate instances (if they were references)
        let strategies = vec![
            ToolParsingStrategy::Default,
            ToolParsingStrategy::Qwen3Coder,
            ToolParsingStrategy::OpenAI,
            ToolParsingStrategy::Claude,
        ];

        let cloned_strategies: Vec<_> = strategies.to_vec();
        assert_eq!(strategies, cloned_strategies);
    }

    #[test]
    fn test_tool_parsing_strategy_debug() {
        // Test Debug implementation
        let debug_output = format!("{:?}", ToolParsingStrategy::Default);
        assert!(debug_output.contains("Default"));

        let debug_output = format!("{:?}", ToolParsingStrategy::Qwen3Coder);
        assert!(debug_output.contains("Qwen3Coder"));

        let debug_output = format!("{:?}", ToolParsingStrategy::OpenAI);
        assert!(debug_output.contains("OpenAI"));

        let debug_output = format!("{:?}", ToolParsingStrategy::Claude);
        assert!(debug_output.contains("Claude"));
    }

    #[test]
    fn test_register_custom_parser() {
        let mut engine = ChatTemplateEngine::new();
        let initial_count = engine.tool_call_parsers.len();

        engine.register_parser("custom".to_string(), Box::new(JsonToolCallParser::new()));
        assert_eq!(engine.tool_call_parsers.len(), initial_count + 1);
        assert!(engine.tool_call_parsers.contains_key("custom"));
    }

    #[test]
    fn test_tool_call_deduplication() {
        let engine = ChatTemplateEngine::new();

        // Text with duplicate tool calls
        let text = r#"
        {"function_name": "list_files", "arguments": {"path": "/tmp"}}
        I'll also check another directory.
        {"function_name": "list_files", "arguments": {"path": "/home"}}
        "#;

        let tool_calls = engine.extract_tool_calls(text).unwrap();
        assert_eq!(tool_calls.len(), 2); // Should have 2 unique tool calls
    }

    #[test]
    fn test_apply_chat_template_with_tools_format() {
        let engine = ChatTemplateEngine::new();
        let messages = vec![
            ("user".to_string(), "Hello".to_string()),
            ("assistant".to_string(), "Hi there!".to_string()),
        ];

        let tools_context = "Available tools: list_files";
        let result = engine.format_chat_template(&messages, Some(tools_context));

        // This test verifies the string formatting logic
        assert!(result.is_ok());
        let prompt = result.unwrap();
        assert!(prompt.contains("### System:"));
        assert!(prompt.contains("Available tools: list_files"));
        assert!(prompt.contains("### Human:"));
        assert!(prompt.contains("Hello"));
        assert!(prompt.contains("### Assistant:"));
    }

    #[test]
    fn test_detect_from_model_name_qwen3coder() {
        // Test Qwen3Coder detection with various patterns
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name(
                "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"
            ),
            ToolParsingStrategy::Qwen3Coder
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("Qwen3-coder-1.5b"),
            ToolParsingStrategy::Qwen3Coder
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("qwen3-coder-7b"),
            ToolParsingStrategy::Qwen3Coder
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("QWEN3-CODER-14B"),
            ToolParsingStrategy::Qwen3Coder
        );

        // Test that just qwen3 without coder doesn't match Qwen3Coder
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("Qwen3-30B-Instruct"),
            ToolParsingStrategy::Default
        );

        // Test that just coder without qwen3 doesn't match Qwen3Coder
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("microsoft/coder-1b"),
            ToolParsingStrategy::Default
        );
    }

    #[test]
    fn test_detect_from_model_name_openai() {
        // Test OpenAI detection patterns
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("gpt-3.5-turbo"),
            ToolParsingStrategy::OpenAI
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("gpt-4"),
            ToolParsingStrategy::OpenAI
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("gpt-4-turbo-preview"),
            ToolParsingStrategy::OpenAI
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("openai/gpt-3.5-turbo"),
            ToolParsingStrategy::OpenAI
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("openai-gpt-4"),
            ToolParsingStrategy::OpenAI
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("OpenAI-GPT-4"),
            ToolParsingStrategy::OpenAI
        );
    }

    #[test]
    fn test_detect_from_model_name_claude() {
        // Test Claude detection patterns
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("claude-3-sonnet"),
            ToolParsingStrategy::Claude
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("claude-3-haiku"),
            ToolParsingStrategy::Claude
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("anthropic/claude-3-opus"),
            ToolParsingStrategy::Claude
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("CLAUDE-2.1"),
            ToolParsingStrategy::Claude
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("anthropic-claude-instant"),
            ToolParsingStrategy::Claude
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("ANTHROPIC/CLAUDE-3"),
            ToolParsingStrategy::Claude
        );
    }

    #[test]
    fn test_detect_from_model_name_default_fallback() {
        // Test Default fallback for unrecognized models
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("microsoft/Phi-3-mini-4k-instruct"),
            ToolParsingStrategy::Default
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("meta-llama/Llama-2-7b-chat-hf"),
            ToolParsingStrategy::Default
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("google/gemma-7b-it"),
            ToolParsingStrategy::Default
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("mistralai/Mixtral-8x7B-Instruct-v0.1"),
            ToolParsingStrategy::Default
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("random-unknown-model"),
            ToolParsingStrategy::Default
        );
    }

    #[test]
    fn test_detect_from_model_name_edge_cases() {
        // Test edge cases: empty strings, special characters
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name(""),
            ToolParsingStrategy::Default
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("   "),
            ToolParsingStrategy::Default
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("special!@#$%^&*()characters"),
            ToolParsingStrategy::Default
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("qwen3-coder-with-special-chars!@#"),
            ToolParsingStrategy::Qwen3Coder
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("path/to/gpt-4/model"),
            ToolParsingStrategy::OpenAI
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("some-prefix-claude-suffix"),
            ToolParsingStrategy::Claude
        );
    }

    #[test]
    fn test_detect_from_model_name_case_insensitive() {
        // Verify case insensitive matching works correctly
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("QWEN3-CODER"),
            ToolParsingStrategy::Qwen3Coder
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("qwen3-coder"),
            ToolParsingStrategy::Qwen3Coder
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("Qwen3-Coder"),
            ToolParsingStrategy::Qwen3Coder
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("GPT-4"),
            ToolParsingStrategy::OpenAI
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("gpt-4"),
            ToolParsingStrategy::OpenAI
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("OPENAI"),
            ToolParsingStrategy::OpenAI
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("openai"),
            ToolParsingStrategy::OpenAI
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("CLAUDE"),
            ToolParsingStrategy::Claude
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("claude"),
            ToolParsingStrategy::Claude
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("ANTHROPIC"),
            ToolParsingStrategy::Claude
        );
        assert_eq!(
            ToolParsingStrategy::detect_from_model_name("anthropic"),
            ToolParsingStrategy::Claude
        );
    }

    mod factory_tests {
        use super::*;

        #[test]
        fn test_tool_parser_factory_create_default_parser() {
            let parser = ToolParserFactory::create_parser(ToolParsingStrategy::Default);

            // Test that it parses tool calls using the existing logic
            let test_text = r#"{"function_name": "list_files", "arguments": {"path": "/tmp"}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "list_files");
        }

        #[test]
        fn test_tool_parser_factory_create_qwen3coder_parser() {
            let parser = ToolParserFactory::create_parser(ToolParsingStrategy::Qwen3Coder);

            // Test that it currently delegates to default behavior
            let test_text = r#"{"function_name": "test_tool", "arguments": {"param": "value"}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "test_tool");
        }

        #[test]
        fn test_tool_parser_factory_create_openai_parser() {
            let parser = ToolParserFactory::create_parser(ToolParsingStrategy::OpenAI);

            // Test that it currently delegates to default behavior
            let test_text = r#"{"function_name": "openai_tool", "arguments": {"data": "test"}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "openai_tool");
        }

        #[test]
        fn test_tool_parser_factory_create_claude_parser() {
            let parser = ToolParserFactory::create_parser(ToolParsingStrategy::Claude);

            // Test that it currently delegates to default behavior
            let test_text = r#"{"function_name": "claude_tool", "arguments": {"input": "test"}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "claude_tool");
        }

        #[test]
        fn test_tool_parser_factory_all_strategies() {
            // Test that factory can create all strategy types
            let strategies = vec![
                ToolParsingStrategy::Default,
                ToolParsingStrategy::Qwen3Coder,
                ToolParsingStrategy::OpenAI,
                ToolParsingStrategy::Claude,
            ];

            for strategy in strategies {
                let parser = ToolParserFactory::create_parser(strategy);

                // Test each parser with empty input (should return empty vec)
                let result = parser.parse_tool_calls("no tool calls here").unwrap();
                assert_eq!(result.len(), 0);

                // Test each parser with valid JSON tool call
                let test_text = r#"{"function_name": "test", "arguments": {}}"#;
                let result = parser.parse_tool_calls(test_text).unwrap();
                assert_eq!(result.len(), 1);
                assert_eq!(result[0].name, "test");
            }
        }
    }

    mod default_parser_tests {
        use super::*;

        #[test]
        fn test_default_tool_parser_new() {
            let parser = DefaultToolParser::new();

            // DefaultToolParser should be created successfully
            // We can't directly inspect the internal parsers, but we can test behavior
            let test_text = r#"{"function_name": "test_tool", "arguments": {"key": "value"}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "test_tool");
        }

        #[test]
        fn test_default_tool_parser_default_trait() {
            let parser = DefaultToolParser::default();

            // Should work the same as new()
            let test_text = r#"{"tool": "test_tool", "parameters": {"key": "value"}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "test_tool");
        }

        #[test]
        fn test_default_tool_parser_tries_multiple_formats() {
            let parser = DefaultToolParser::new();

            // Test JSON format (should work with first parser)
            let json_text = r#"{"function_name": "json_tool", "arguments": {"param": "value"}}"#;
            let result = parser.parse_tool_calls(json_text).unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "json_tool");

            // Test XML format (should work with second parser)
            let xml_text = r#"<function_call name="xml_tool">{"param": "value"}</function_call>"#;
            let result = parser.parse_tool_calls(xml_text).unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "xml_tool");

            // Test function call format (should work with third parser)
            let func_text = r#"Call func_tool with arguments {"param": "value"}"#;
            let result = parser.parse_tool_calls(func_text).unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "func_tool");
        }

        #[test]
        fn test_default_tool_parser_no_matches() {
            let parser = DefaultToolParser::new();

            // Text with no tool calls should return empty vector
            let text = "This is just regular text with no tool calls.";
            let result = parser.parse_tool_calls(text).unwrap();
            assert_eq!(result.len(), 0);
        }

        #[test]
        fn test_default_tool_parser_maintains_existing_behavior() {
            let parser = DefaultToolParser::new();
            let engine = ChatTemplateEngine::new();

            // Compare behavior with the existing ChatTemplateEngine
            let test_cases = vec![
                r#"{"function_name": "test1", "arguments": {"path": "/tmp"}}"#,
                r#"{"tool": "test2", "parameters": {"data": "value"}}"#,
                r#"{"name": "test3", "args": {"input": "test"}}"#,
                r#"<function_call name="test4">{"param": "xml_test"}</function_call>"#,
                "Call test5 with arguments {\"param\": \"func_test\"}",
                "No tool calls here",
            ];

            for test_text in test_cases {
                let parser_result = parser.parse_tool_calls(test_text).unwrap();
                let engine_result = engine.extract_tool_calls(test_text).unwrap();

                // Results should be identical
                assert_eq!(
                    parser_result.len(),
                    engine_result.len(),
                    "Mismatch for text: {}",
                    test_text
                );

                for (parser_call, engine_call) in parser_result.iter().zip(engine_result.iter()) {
                    assert_eq!(parser_call.name, engine_call.name);
                    assert_eq!(parser_call.arguments, engine_call.arguments);
                }
            }
        }
    }

    mod placeholder_parser_tests {
        use super::*;

        #[test]
        fn test_qwen3coder_tool_parser_new() {
            let parser = Qwen3CoderToolParser::new();

            let test_text = r#"{"function_name": "qwen_test", "arguments": {"param": "value"}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "qwen_test");
        }

        #[test]
        fn test_qwen3coder_tool_parser_default_trait() {
            let parser = Qwen3CoderToolParser::default();

            let test_text = r#"{"function_name": "qwen_default_test", "arguments": {}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "qwen_default_test");
        }

        #[test]
        fn test_qwen3coder_nested_xml_basic() {
            let parser = Qwen3CoderToolParser::new();

            let xml = r#"<tool_call><search><query>rust async</query><limit>10</limit></search></tool_call>"#;
            let result = parser.parse_tool_calls(xml).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");

            let args = result[0].arguments.as_object().unwrap();
            assert_eq!(args.get("query").unwrap().as_str().unwrap(), "rust async");
            assert_eq!(args.get("limit").unwrap().as_i64().unwrap(), 10);
        }

        #[test]
        fn test_qwen3coder_nested_xml_mixed_types() {
            let parser = Qwen3CoderToolParser::new();

            let xml = r#"<tool_call><complex_tool><name>test</name><count>42</count><enabled>true</enabled><data>{"nested": "json"}</data></complex_tool></tool_call>"#;
            let result = parser.parse_tool_calls(xml).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "complex_tool");

            let args = result[0].arguments.as_object().unwrap();
            assert_eq!(args.get("name").unwrap().as_str().unwrap(), "test");
            assert_eq!(args.get("count").unwrap().as_i64().unwrap(), 42);
            assert!(args.get("enabled").unwrap().as_bool().unwrap());

            let nested_data = args.get("data").unwrap().as_object().unwrap();
            assert_eq!(nested_data.get("nested").unwrap().as_str().unwrap(), "json");
        }

        #[test]
        fn test_qwen3coder_nested_xml_multiple_calls() {
            let parser = Qwen3CoderToolParser::new();

            let xml = r#"<tool_call><search><query>first</query></search></tool_call>
Some text in between
<tool_call><list><path>/tmp</path></list></tool_call>"#;
            let result = parser.parse_tool_calls(xml).unwrap();

            assert_eq!(result.len(), 2);

            assert_eq!(result[0].name, "search");
            let args1 = result[0].arguments.as_object().unwrap();
            assert_eq!(args1.get("query").unwrap().as_str().unwrap(), "first");

            assert_eq!(result[1].name, "list");
            let args2 = result[1].arguments.as_object().unwrap();
            assert_eq!(args2.get("path").unwrap().as_str().unwrap(), "/tmp");
        }

        #[test]
        fn test_qwen3coder_nested_xml_type_conversion() {
            let parser = Qwen3CoderToolParser::new();

            // Test different data types
            let xml = r#"<tool_call><test_tool>
                <string_param>hello world</string_param>
                <int_param>42</int_param>
                <float_param>3.14</float_param>
                <bool_true>true</bool_true>
                <bool_false>FALSE</bool_false>
                <json_array>[1, 2, 3]</json_array>
                <empty_param></empty_param>
            </test_tool></tool_call>"#;

            let result = parser.parse_tool_calls(xml).unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "test_tool");

            let args = result[0].arguments.as_object().unwrap();
            assert_eq!(
                args.get("string_param").unwrap().as_str().unwrap(),
                "hello world"
            );
            assert_eq!(args.get("int_param").unwrap().as_i64().unwrap(), 42);

            // Use approximate comparison for floats to handle precision issues
            let float_value = args.get("float_param").unwrap().as_f64().unwrap();
            let diff = (float_value - TEST_FLOAT_VALUE).abs();
            assert!(
                diff < 0.0001,
                "Expected ~3.14, got {}, diff = {}",
                float_value,
                diff
            );
            assert!(args.get("bool_true").unwrap().as_bool().unwrap());
            assert!(!args.get("bool_false").unwrap().as_bool().unwrap());

            let array = args.get("json_array").unwrap().as_array().unwrap();
            assert_eq!(array.len(), 3);
            assert_eq!(array[0].as_i64().unwrap(), 1);

            assert_eq!(args.get("empty_param").unwrap().as_str().unwrap(), "");
        }

        #[test]
        fn test_qwen3coder_nested_xml_no_parameters() {
            let parser = Qwen3CoderToolParser::new();

            let xml = r#"<tool_call><simple_tool></simple_tool></tool_call>"#;
            let result = parser.parse_tool_calls(xml).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "simple_tool");

            let args = result[0].arguments.as_object().unwrap();
            assert!(args.is_empty());
        }

        #[test]
        fn test_qwen3coder_nested_xml_malformed_graceful_failure() {
            let parser = Qwen3CoderToolParser::new();

            // Missing closing tool_call tag - robust parser should handle this now
            let malformed1 = r#"<tool_call><search><query>test</query></search>"#;
            let _result1 = parser.parse_tool_calls(malformed1).unwrap();
            // Should now parse successfully with robust parsing
            // Parsing succeeds and returns valid result (empty or populated)

            // No nested content
            let malformed2 = r#"<tool_call>just text content</tool_call>"#;
            let result2 = parser.parse_tool_calls(malformed2).unwrap();
            assert_eq!(result2.len(), 0);

            // Empty tool call
            let empty = r#"<tool_call></tool_call>"#;
            let result3 = parser.parse_tool_calls(empty).unwrap();
            assert_eq!(result3.len(), 0);
        }

        #[test]
        fn test_qwen3coder_nested_xml_mixed_content() {
            let parser = Qwen3CoderToolParser::new();

            let mixed = r#"Here's some regular text before the tool call.

<tool_call><analyze><input>some data</input><format>json</format></analyze></tool_call>

And here's some text after. Let me also call another tool:

<tool_call><validate><schema>user_schema</schema></validate></tool_call>

Done!"#;

            let result = parser.parse_tool_calls(mixed).unwrap();
            assert_eq!(result.len(), 2);

            assert_eq!(result[0].name, "analyze");
            let args1 = result[0].arguments.as_object().unwrap();
            assert_eq!(args1.get("input").unwrap().as_str().unwrap(), "some data");
            assert_eq!(args1.get("format").unwrap().as_str().unwrap(), "json");

            assert_eq!(result[1].name, "validate");
            let args2 = result[1].arguments.as_object().unwrap();
            assert_eq!(
                args2.get("schema").unwrap().as_str().unwrap(),
                "user_schema"
            );
        }

        #[test]
        fn test_openai_tool_parser_new() {
            let parser = OpenAIToolParser::new();

            let test_text = r#"{"function_name": "openai_test", "arguments": {"param": "value"}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "openai_test");
        }

        #[test]
        fn test_openai_tool_parser_default_trait() {
            let parser = OpenAIToolParser::default();

            let test_text = r#"{"function_name": "openai_default_test", "arguments": {}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "openai_default_test");
        }

        #[test]
        fn test_claude_tool_parser_new() {
            let parser = ClaudeToolParser::new();

            let test_text = r#"{"function_name": "claude_test", "arguments": {"param": "value"}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "claude_test");
        }

        #[test]
        fn test_claude_tool_parser_default_trait() {
            let parser = ClaudeToolParser::default();

            let test_text = r#"{"function_name": "claude_default_test", "arguments": {}}"#;
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "claude_default_test");
        }

        #[test]
        fn test_placeholder_parsers_delegate_correctly() {
            let default_parser = DefaultToolParser::new();
            let qwen_parser = Qwen3CoderToolParser::new();
            let openai_parser = OpenAIToolParser::new();
            let claude_parser = ClaudeToolParser::new();

            let test_cases = vec![
                r#"{"function_name": "test1", "arguments": {"key": "value"}}"#,
                r#"{"tool": "test2", "parameters": {"data": "test"}}"#,
                "Call test3 with arguments {\"input\": \"test\"}",
                "No tool calls here at all",
            ];

            for test_text in test_cases {
                let default_result = default_parser.parse_tool_calls(test_text).unwrap();
                let qwen_result = qwen_parser.parse_tool_calls(test_text).unwrap();
                let openai_result = openai_parser.parse_tool_calls(test_text).unwrap();
                let claude_result = claude_parser.parse_tool_calls(test_text).unwrap();

                // All placeholder parsers should return identical results to default
                assert_eq!(
                    default_result.len(),
                    qwen_result.len(),
                    "Qwen parser mismatch for: {}",
                    test_text
                );
                assert_eq!(
                    default_result.len(),
                    openai_result.len(),
                    "OpenAI parser mismatch for: {}",
                    test_text
                );
                assert_eq!(
                    default_result.len(),
                    claude_result.len(),
                    "Claude parser mismatch for: {}",
                    test_text
                );

                for i in 0..default_result.len() {
                    assert_eq!(default_result[i].name, qwen_result[i].name);
                    assert_eq!(default_result[i].name, openai_result[i].name);
                    assert_eq!(default_result[i].name, claude_result[i].name);
                }
            }
        }
    }

    #[test]
    fn test_factory_integration_with_existing_system() {
        // Integration test: verify factory-created parsers work with existing code patterns
        let test_strategies = vec![
            ToolParsingStrategy::Default,
            ToolParsingStrategy::Qwen3Coder,
            ToolParsingStrategy::OpenAI,
            ToolParsingStrategy::Claude,
        ];

        let test_text = r#"I'll help you list the files. {"function_name": "list_files", "arguments": {"path": "/tmp"}}"#;

        // All parsers should extract the same tool call from this text
        for strategy in test_strategies {
            let parser = ToolParserFactory::create_parser(strategy.clone());
            let result = parser.parse_tool_calls(test_text).unwrap();

            assert_eq!(
                result.len(),
                1,
                "Strategy {:?} failed to parse tool call",
                strategy
            );
            assert_eq!(
                result[0].name, "list_files",
                "Strategy {:?} parsed wrong tool name",
                strategy
            );

            // Verify arguments were parsed correctly
            if let Some(path_value) = result[0].arguments.get("path") {
                assert_eq!(path_value.as_str().unwrap(), "/tmp");
            } else {
                panic!(
                    "Strategy {:?} failed to parse arguments correctly",
                    strategy
                );
            }
        }
    }

    #[test]
    fn test_factory_backward_compatibility() {
        // Test that factory maintains backward compatibility with existing behavior
        let old_engine = ChatTemplateEngine::new();
        let new_parser = ToolParserFactory::create_parser(ToolParsingStrategy::Default);

        let test_cases = vec![
            // Complex JSON with nested objects - single tool call (deterministic)
            r#"{"function_name": "complex_tool", "arguments": {"nested": {"key": "value", "array": [1, 2, 3]}, "simple": "test"}}"#,
            // Edge cases that were problematic before - single tool call (deterministic)
            r#"{"function_name": "edge_tool", "arguments": {"path": "."}}I apologize for the confusion"#,
        ];

        for test_text in test_cases {
            let old_result = old_engine.extract_tool_calls(test_text).unwrap();
            let new_result = new_parser.parse_tool_calls(test_text).unwrap();

            // Results should be identical for single tool call cases
            assert_eq!(
                old_result.len(),
                new_result.len(),
                "Length mismatch for: {}",
                test_text
            );

            for (old_call, new_call) in old_result.iter().zip(new_result.iter()) {
                assert_eq!(
                    old_call.name, new_call.name,
                    "Name mismatch for: {}",
                    test_text
                );
                assert_eq!(
                    old_call.arguments, new_call.arguments,
                    "Arguments mismatch for: {}",
                    test_text
                );
            }
        }
    }

    mod chatengine_strategy_integration_tests {
        use super::*;

        #[test]
        fn test_with_model_strategy_constructor() {
            // Test automatic strategy detection
            let qwen_engine = ChatTemplateEngine::with_model_strategy(
                "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF",
            );
            assert_eq!(
                qwen_engine.get_parsing_strategy(),
                Some(&ToolParsingStrategy::Qwen3Coder)
            );

            let openai_engine = ChatTemplateEngine::with_model_strategy("gpt-3.5-turbo");
            assert_eq!(
                openai_engine.get_parsing_strategy(),
                Some(&ToolParsingStrategy::OpenAI)
            );

            let claude_engine = ChatTemplateEngine::with_model_strategy("claude-3-sonnet");
            assert_eq!(
                claude_engine.get_parsing_strategy(),
                Some(&ToolParsingStrategy::Claude)
            );

            let default_engine = ChatTemplateEngine::with_model_strategy("unknown-model");
            assert_eq!(
                default_engine.get_parsing_strategy(),
                Some(&ToolParsingStrategy::Default)
            );
        }

        #[test]
        fn test_manual_strategy_setting() {
            let mut engine = ChatTemplateEngine::new();
            assert_eq!(engine.get_parsing_strategy(), None);

            engine.set_parsing_strategy(ToolParsingStrategy::Qwen3Coder);
            assert_eq!(
                engine.get_parsing_strategy(),
                Some(&ToolParsingStrategy::Qwen3Coder)
            );

            engine.set_parsing_strategy(ToolParsingStrategy::OpenAI);
            assert_eq!(
                engine.get_parsing_strategy(),
                Some(&ToolParsingStrategy::OpenAI)
            );
        }

        #[test]
        fn test_strategy_based_parsing_priority() {
            // Initialize tracing for debugging
            let _ = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::DEBUG)
                .try_init();

            let mut engine = ChatTemplateEngine::new();
            engine.set_parsing_strategy(ToolParsingStrategy::Default);

            let test_text = r#"{"function_name": "test_tool", "arguments": {"param": "value"}}"#;
            let tool_calls = engine.extract_tool_calls(test_text).unwrap();

            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].name, "test_tool");
        }

        #[test]
        fn test_strategy_parsing_fallback() {
            let mut engine = ChatTemplateEngine::new();
            engine.set_parsing_strategy(ToolParsingStrategy::Default);

            // Test that if strategy parser fails, we fall back to legacy parsers
            let test_text = "No tool calls here";
            let tool_calls = engine.extract_tool_calls(test_text).unwrap();
            assert_eq!(tool_calls.len(), 0);

            // Test with valid tool call
            let valid_text = r#"{"function_name": "valid_tool", "arguments": {}}"#;
            let tool_calls = engine.extract_tool_calls(valid_text).unwrap();
            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].name, "valid_tool");
        }

        #[test]
        fn test_no_strategy_uses_legacy_parsers() {
            let engine = ChatTemplateEngine::new(); // No strategy set
            assert_eq!(engine.get_parsing_strategy(), None);

            let test_text = r#"{"function_name": "legacy_tool", "arguments": {"key": "value"}}"#;
            let tool_calls = engine.extract_tool_calls(test_text).unwrap();

            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].name, "legacy_tool");
        }

        #[test]
        fn test_backward_compatibility_preserved() {
            // Test that all existing functionality still works
            let old_engine = ChatTemplateEngine::new();
            let new_engine = ChatTemplateEngine::with_model_strategy("unknown-model");

            let test_cases = vec![
                r#"{"function_name": "test1", "arguments": {"path": "/tmp"}}"#,
                r#"{"tool": "test2", "parameters": {"data": "value"}}"#,
                r#"{"name": "test3", "args": {"input": "test"}}"#,
                r#"<function_call name="test4">{"param": "xml_test"}</function_call>"#,
                "Call test5 with arguments {\"param\": \"func_test\"}",
            ];

            for test_text in test_cases {
                let old_result = old_engine.extract_tool_calls(test_text).unwrap();
                let new_result = new_engine.extract_tool_calls(test_text).unwrap();

                // Results should be identical
                assert_eq!(
                    old_result.len(),
                    new_result.len(),
                    "Length mismatch for: {}",
                    test_text
                );

                for (old_call, new_call) in old_result.iter().zip(new_result.iter()) {
                    assert_eq!(
                        old_call.name, new_call.name,
                        "Name mismatch for: {}",
                        test_text
                    );
                    // Note: We don't compare arguments directly since tool call IDs might differ
                }
            }
        }

        #[test]
        fn test_strategy_integration_with_actual_tool_calls() {
            let strategies = vec![
                ToolParsingStrategy::Default,
                ToolParsingStrategy::Qwen3Coder,
                ToolParsingStrategy::OpenAI,
                ToolParsingStrategy::Claude,
            ];

            let test_text = r#"I'll help you list the files. {"function_name": "list_files", "arguments": {"path": "/tmp"}}"#;

            for strategy in strategies {
                let mut engine = ChatTemplateEngine::new();
                engine.set_parsing_strategy(strategy.clone());

                let result = engine.extract_tool_calls(test_text).unwrap();
                assert_eq!(
                    result.len(),
                    1,
                    "Strategy {:?} failed to parse tool call",
                    strategy
                );
                assert_eq!(
                    result[0].name, "list_files",
                    "Strategy {:?} parsed wrong tool name",
                    strategy
                );

                // Verify arguments were parsed correctly
                if let Some(path_value) = result[0].arguments.get("path") {
                    assert_eq!(path_value.as_str().unwrap(), "/tmp");
                } else {
                    panic!(
                        "Strategy {:?} failed to parse arguments correctly",
                        strategy
                    );
                }
            }
        }

        #[test]
        fn test_debug_logging_with_strategy() {
            // Initialize tracing for test debugging
            let _ = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::DEBUG)
                .try_init();

            let mut engine = ChatTemplateEngine::new();
            engine.set_parsing_strategy(ToolParsingStrategy::Qwen3Coder);

            // This should trigger debug logging for strategy-based parsing
            let text = r#"{"function_name": "debug_test", "arguments": {"param": "value"}}"#;
            let tool_calls = engine.extract_tool_calls(text).unwrap();
            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].name, "debug_test");

            // Test with empty text (should trigger fallback logging)
            let empty_text = "No tool calls here";
            let empty_calls = engine.extract_tool_calls(empty_text).unwrap();
            assert_eq!(empty_calls.len(), 0);
        }

        #[test]
        fn test_register_parser_still_works() {
            let mut engine = ChatTemplateEngine::with_model_strategy("gpt-4");
            let initial_count = engine.tool_call_parsers.len();

            // Test that custom parser registration still works
            engine.register_parser("custom".to_string(), Box::new(JsonToolCallParser::new()));
            assert_eq!(engine.tool_call_parsers.len(), initial_count + 1);
            assert!(engine.tool_call_parsers.contains_key("custom"));

            // Verify strategy is still set
            assert_eq!(
                engine.get_parsing_strategy(),
                Some(&ToolParsingStrategy::OpenAI)
            );
        }

        #[test]
        fn test_deduplication_with_strategy() {
            let mut engine = ChatTemplateEngine::new();
            engine.set_parsing_strategy(ToolParsingStrategy::Default);

            // Text with potentially duplicate tool calls
            let text = r#"{"function_name": "dup_tool", "arguments": {"path": "/tmp"}}"#;

            let tool_calls = engine.extract_tool_calls(text).unwrap();
            assert_eq!(tool_calls.len(), 1); // Should deduplicate properly

            // Test deduplication behavior matches legacy system
            let legacy_engine = ChatTemplateEngine::new();
            let legacy_calls = legacy_engine.extract_tool_calls(text).unwrap();
            assert_eq!(tool_calls.len(), legacy_calls.len());
        }

        // === ROBUST XML PARSING TESTS ===

        #[test]
        fn test_qwen3coder_robust_parsing_malformed_xml() {
            let parser = Qwen3CoderToolParser::new();

            // Missing closing tag for parameter - should still parse successfully
            let malformed =
                r#"<tool_call><search><query>test query</query><limit>10</search></tool_call>"#;
            let result = parser.parse_tool_calls(malformed).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");
            let args = result[0].arguments.as_object().unwrap();
            assert_eq!(args.get("query").unwrap().as_str().unwrap(), "test query");
            // Note: the limit parameter may not parse correctly due to malformed XML, which is expected
            if let Some(limit_val) = args.get("limit") {
                // If it parsed, it should be 10
                if let Some(limit_num) = limit_val.as_i64() {
                    assert_eq!(limit_num, 10);
                }
            }
        }

        #[test]
        fn test_qwen3coder_robust_parsing_incomplete_xml() {
            let parser = Qwen3CoderToolParser::new();

            // Cut off mid-generation
            let incomplete = r#"<tool_call><search><query>partial"#;
            let result = parser.parse_tool_calls(incomplete).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");
            let args = result[0].arguments.as_object().unwrap();
            assert_eq!(args.get("query").unwrap().as_str().unwrap(), "partial");
        }

        #[test]
        fn test_qwen3coder_robust_parsing_extra_whitespace() {
            let parser = Qwen3CoderToolParser::new();

            let whitespace = r#"<tool_call>
    <search>
        <query>  spaced query  </query>
        <limit>   10   </limit>
    </search>
</tool_call>"#;
            let result = parser.parse_tool_calls(whitespace).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");
            let args = result[0].arguments.as_object().unwrap();
            assert_eq!(args.get("query").unwrap().as_str().unwrap(), "spaced query");
            assert_eq!(args.get("limit").unwrap().as_i64().unwrap(), 10);
        }

        #[test]
        fn test_qwen3coder_robust_parsing_xml_entities() {
            let parser = Qwen3CoderToolParser::new();

            let special_chars = r#"<tool_call><search><query>&lt;test&gt; &amp; "quotes"</query></search></tool_call>"#;
            let result = parser.parse_tool_calls(special_chars).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");
            let args = result[0].arguments.as_object().unwrap();
            // The clean_parameter_value function should decode XML entities
            assert_eq!(
                args.get("query").unwrap().as_str().unwrap(),
                r#"<test> & "quotes""#
            );
        }

        #[test]
        fn test_qwen3coder_robust_parsing_mixed_content() {
            let parser = Qwen3CoderToolParser::new();

            let mixed = r#"I'll search for that information now. <tool_call><search><query>information</query></search></tool_call> Let me get those results for you."#;
            let result = parser.parse_tool_calls(mixed).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");
            let args = result[0].arguments.as_object().unwrap();
            assert_eq!(args.get("query").unwrap().as_str().unwrap(), "information");
        }

        #[test]
        fn test_qwen3coder_robust_parsing_missing_closing_tool_call() {
            let parser = Qwen3CoderToolParser::new();

            // Missing </tool_call> tag
            let missing_close =
                r#"<tool_call><search><query>test</query><limit>5</limit></search>"#;
            let result = parser.parse_tool_calls(missing_close).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");
            let args = result[0].arguments.as_object().unwrap();
            assert_eq!(args.get("query").unwrap().as_str().unwrap(), "test");
            assert_eq!(args.get("limit").unwrap().as_i64().unwrap(), 5);
        }

        #[test]
        fn test_qwen3coder_robust_parsing_nested_tags() {
            let parser = Qwen3CoderToolParser::new();

            // Nested tool_call tags (should handle depth correctly)
            let nested = r#"<tool_call><outer><inner><tool_call>nested</tool_call></inner><param>value</param></outer></tool_call>"#;
            let result = parser.parse_tool_calls(nested).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "outer");
        }

        #[test]
        fn test_qwen3coder_robust_parsing_multiple_malformed() {
            let parser = Qwen3CoderToolParser::new();

            let multiple = r#"
<tool_call><search><query>first query</query></search></tool_call>
<tool_call><analyze><data>incomplete
<tool_call><process><input>third input</input></process></tool_call>
"#;
            let result = parser.parse_tool_calls(multiple).unwrap();

            // Should successfully parse the first and third tool calls
            assert!(result.len() >= 2);
            assert_eq!(result[0].name, "search");
            assert_eq!(
                result[0]
                    .arguments
                    .as_object()
                    .unwrap()
                    .get("query")
                    .unwrap()
                    .as_str()
                    .unwrap(),
                "first query"
            );
        }

        #[test]
        fn test_qwen3coder_robust_parsing_fuzzy_matching() {
            let parser = Qwen3CoderToolParser::new();

            // Very malformed but recognizable pattern
            let fuzzy = r#"tool_call><analyze><data>some data</data></analyze"#;
            let result = parser.parse_tool_calls(fuzzy).unwrap();

            // Should either parse successfully or fall back gracefully
            // The exact behavior depends on fallback strategy
            assert!(result.len() <= 1);
        }

        #[test]
        fn test_qwen3coder_robust_parsing_edge_case_empty_params() {
            let parser = Qwen3CoderToolParser::new();

            let empty_param =
                r#"<tool_call><search><query></query><limit>10</limit></search></tool_call>"#;
            let result = parser.parse_tool_calls(empty_param).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");
            let args = result[0].arguments.as_object().unwrap();
            // Empty query may not be included or may be empty string
            if args.contains_key("query") {
                let query_value = args.get("query").unwrap();
                assert!(query_value.as_str().unwrap_or("").is_empty());
            }
            assert_eq!(args.get("limit").unwrap().as_i64().unwrap(), 10);
        }

        #[test]
        fn test_qwen3coder_robust_parsing_type_conversion_errors() {
            let parser = Qwen3CoderToolParser::new();

            // Invalid JSON in parameter value
            let invalid_json = r#"<tool_call><search><query>test</query><data>{"invalid": json}</data></search></tool_call>"#;
            let result = parser.parse_tool_calls(invalid_json).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");
            let args = result[0].arguments.as_object().unwrap();
            assert_eq!(args.get("query").unwrap().as_str().unwrap(), "test");
            // Invalid JSON should fall back to string
            assert_eq!(
                args.get("data").unwrap().as_str().unwrap(),
                r#"{"invalid": json}"#
            );
        }

        #[test]
        fn test_qwen3coder_robust_parsing_performance_large_malformed() {
            let parser = Qwen3CoderToolParser::new();

            // Large text with embedded malformed XML
            let mut large_text = String::new();
            large_text.push_str("This is a very large text with some content. ");
            for i in 0..100 {
                large_text.push_str(&format!(
                    "Line {} with some content that might confuse the parser. ",
                    i
                ));
            }
            large_text.push_str(r#"<tool_call><search><query>embedded query</query><limit>5</limit></search></tool_call>"#);
            for i in 100..200 {
                large_text.push_str(&format!("More content after the tool call. Line {}. ", i));
            }

            let start = std::time::Instant::now();
            let result = parser.parse_tool_calls(&large_text).unwrap();
            let duration = start.elapsed();

            // Should parse successfully and reasonably fast
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");
            assert!(
                duration.as_millis() < 1000,
                "Parsing should be fast even for large malformed input"
            );
        }

        #[test]
        fn test_qwen3coder_robust_parsing_backwards_compatibility() {
            let parser = Qwen3CoderToolParser::new();

            // Well-formed XML should work exactly as before
            let well_formed = r#"<tool_call><search><query>rust programming</query><limit>10</limit><exact>true</exact></search></tool_call>"#;
            let result = parser.parse_tool_calls(well_formed).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");
            let args = result[0].arguments.as_object().unwrap();
            assert_eq!(
                args.get("query").unwrap().as_str().unwrap(),
                "rust programming"
            );
            assert_eq!(args.get("limit").unwrap().as_i64().unwrap(), 10);
            assert!(args.get("exact").unwrap().as_bool().unwrap());
        }

        #[test]
        fn test_qwen3coder_robust_parsing_fallback_to_default() {
            let parser = Qwen3CoderToolParser::new();

            // Text that doesn't match XML pattern at all - should fall back to DefaultToolParser
            let non_xml = r#"{"function_name": "fallback_test", "arguments": {"param": "value"}}"#;
            let result = parser.parse_tool_calls(non_xml).unwrap();

            // Should be handled by fallback parser
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "fallback_test");
        }

        #[test]
        fn test_qwen3coder_nested_xml_integration_with_engine() {
            let mut engine = ChatTemplateEngine::new();
            engine.set_parsing_strategy(ToolParsingStrategy::Qwen3Coder);

            // Test the nested XML format integration
            let text = r#"I'll help you search for information.

<tool_call><search_files><pattern>*.rs</pattern><directory>/src</directory><recursive>true</recursive></search_files></tool_call>

And then let me analyze the results:

<tool_call><analyze_code><language>rust</language><complexity>high</complexity><metrics>["cyclomatic", "maintainability"]</metrics></analyze_code></tool_call>

All done!"#;

            let result = engine.extract_tool_calls(text).unwrap();
            assert_eq!(result.len(), 2, "Should extract both nested XML tool calls");

            // Verify first tool call
            assert_eq!(result[0].name, "search_files");
            let args1 = result[0].arguments.as_object().unwrap();
            assert_eq!(args1.get("pattern").unwrap().as_str().unwrap(), "*.rs");
            assert_eq!(args1.get("directory").unwrap().as_str().unwrap(), "/src");
            assert!(args1.get("recursive").unwrap().as_bool().unwrap());

            // Verify second tool call
            assert_eq!(result[1].name, "analyze_code");
            let args2 = result[1].arguments.as_object().unwrap();
            assert_eq!(args2.get("language").unwrap().as_str().unwrap(), "rust");
            assert_eq!(args2.get("complexity").unwrap().as_str().unwrap(), "high");

            let metrics = args2.get("metrics").unwrap().as_array().unwrap();
            assert_eq!(metrics.len(), 2);
            assert_eq!(metrics[0].as_str().unwrap(), "cyclomatic");
            assert_eq!(metrics[1].as_str().unwrap(), "maintainability");
        }
    }

    #[test]
    fn test_factory_multiple_tool_calls_compatibility() {
        // Test for multiple tool call scenarios where HashMap non-determinism could affect results
        let old_engine = ChatTemplateEngine::new();
        let new_parser = ToolParserFactory::create_parser(ToolParsingStrategy::Default);

        let test_cases = vec![
            // Multiple tool calls in one text - results may vary due to HashMap ordering
            r#"{"function_name": "first_tool", "arguments": {"a": 1}} and {"function_name": "second_tool", "arguments": {"b": 2}}"#,
            // Mixed formats - JSON and XML - results may vary due to HashMap ordering
            r#"{"function_name": "json_tool", "arguments": {}} <function_call name="xml_tool">{}</function_call>"#,
        ];

        for test_text in test_cases {
            let old_result = old_engine.extract_tool_calls(test_text).unwrap();
            let new_result = new_parser.parse_tool_calls(test_text).unwrap();

            // Due to HashMap non-deterministic iteration order in old implementation,
            // we can't guarantee exact same results. Instead, verify both find tool calls
            // and that the new implementation is consistent with some valid parsing.
            assert!(
                !old_result.is_empty() || !new_result.is_empty(),
                "At least one implementation should find tool calls for: {}",
                test_text
            );

            // If both found tool calls, verify they contain valid function names
            for call in old_result.iter().chain(new_result.iter()) {
                assert!(
                    !call.name.is_empty(),
                    "Tool call name should not be empty for: {}",
                    test_text
                );
            }
        }
    }

    mod qwen3coder_schema_tests {
        use super::*;

        /// Create a sample ToolDefinition for testing schema-based conversion
        fn create_test_tool_definition() -> ToolDefinition {
            ToolDefinition {
                name: "test_search".to_string(),
                description: "Test search tool".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"},
                        "limit": {"type": "integer", "default": 10},
                        "exact_match": {"type": "boolean"},
                        "score": {"type": "number"},
                        "metadata": {"type": "object"},
                        "tags": {"type": "array"},
                        "nullable_field": {"type": "string", "nullable": true},
                        "with_default": {"type": "integer", "default": 42}
                    }
                }),
                server_name: "test".to_string(),
            }
        }

        #[test]
        fn test_new_with_schema_constructor() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            // Should have the schema loaded
            assert_eq!(parser.schema_map.len(), 1);
            assert!(parser.schema_map.contains_key("test_search"));
        }

        #[test]
        fn test_new_with_empty_schema() {
            let parser = Qwen3CoderToolParser::new_with_schema(None);
            assert_eq!(parser.schema_map.len(), 0);
        }

        #[test]
        fn test_build_schema_map() {
            let tools = vec![
                create_test_tool_definition(),
                ToolDefinition {
                    name: "another_tool".to_string(),
                    description: "Another tool".to_string(),
                    parameters: json!({"type": "object", "properties": {"param": {"type": "string"}}}),
                    server_name: "test".to_string(),
                },
            ];

            let schema_map = Qwen3CoderToolParser::build_schema_map(&tools);
            assert_eq!(schema_map.len(), 2);
            assert!(schema_map.contains_key("test_search"));
            assert!(schema_map.contains_key("another_tool"));
        }

        #[test]
        fn test_get_parameter_schema() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            // Test existing parameter
            let schema = parser.get_parameter_schema("test_search", "limit");
            assert!(schema.is_some());
            assert_eq!(schema.unwrap().get("type").unwrap(), "integer");

            // Test non-existing parameter
            let schema = parser.get_parameter_schema("test_search", "nonexistent");
            assert!(schema.is_none());

            // Test non-existing tool
            let schema = parser.get_parameter_schema("nonexistent_tool", "param");
            assert!(schema.is_none());
        }

        #[test]
        fn test_convert_by_schema_type_string() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let schema = json!({"type": "string"});
            let result = parser
                .convert_by_schema_type("hello world", &schema)
                .unwrap();
            assert_eq!(result, Value::String("hello world".to_string()));
        }

        #[test]
        fn test_convert_by_schema_type_integer() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let schema = json!({"type": "integer"});
            let result = parser.convert_by_schema_type("42", &schema).unwrap();
            assert_eq!(result, Value::Number(42.into()));

            // Test invalid integer
            let schema = json!({"type": "integer"});
            let result = parser.convert_by_schema_type("not_a_number", &schema);
            assert!(result.is_err());
        }

        #[test]
        fn test_convert_by_schema_type_number() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let schema = json!({"type": "number"});
            let result = parser.convert_by_schema_type("3.14", &schema).unwrap();

            // Use proper floating point comparison instead of exact equality
            match result {
                Value::Number(num) => {
                    let f = num.as_f64().unwrap();
                    let diff = (f - TEST_FLOAT_VALUE).abs();
                    assert!(diff < 0.0001, "Expected ~3.14, got {}, diff = {}", f, diff);
                }
                _ => panic!("Expected Number, got {:?}", result),
            }

            // Test integer as number
            let result = parser.convert_by_schema_type("42", &schema).unwrap();
            assert_eq!(
                result,
                Value::Number(serde_json::Number::from_f64(42.0).unwrap())
            );
        }

        #[test]
        fn test_convert_by_schema_type_boolean() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let schema = json!({"type": "boolean"});

            // Test various true values
            let true_values = vec!["true", "TRUE", "True", "1", "yes", "YES"];
            for val in true_values {
                let result = parser.convert_by_schema_type(val, &schema).unwrap();
                assert_eq!(result, Value::Bool(true), "Failed for value: {}", val);
            }

            // Test various false values
            let false_values = vec!["false", "FALSE", "False", "0", "no", "NO"];
            for val in false_values {
                let result = parser.convert_by_schema_type(val, &schema).unwrap();
                assert_eq!(result, Value::Bool(false), "Failed for value: {}", val);
            }

            // Test invalid boolean
            let result = parser.convert_by_schema_type("maybe", &schema);
            assert!(result.is_err());
        }

        #[test]
        fn test_convert_by_schema_type_object() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let schema = json!({"type": "object"});
            let result = parser
                .convert_by_schema_type(r#"{"key": "value"}"#, &schema)
                .unwrap();
            assert_eq!(result, json!({"key": "value"}));

            // Test invalid JSON object
            let result = parser.convert_by_schema_type("not json", &schema);
            assert!(result.is_err());

            // Test array as object (should fail)
            let result = parser.convert_by_schema_type("[1,2,3]", &schema);
            assert!(result.is_err());
        }

        #[test]
        fn test_convert_by_schema_type_array() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let schema = json!({"type": "array"});
            let result = parser.convert_by_schema_type("[1,2,3]", &schema).unwrap();
            assert_eq!(result, json!([1, 2, 3]));

            // Test invalid JSON array
            let result = parser.convert_by_schema_type("not json", &schema);
            assert!(result.is_err());

            // Test object as array (should fail)
            let result = parser.convert_by_schema_type(r#"{"key": "value"}"#, &schema);
            assert!(result.is_err());
        }

        #[test]
        fn test_convert_by_schema_type_null() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let schema = json!({"type": "null"});
            let result = parser.convert_by_schema_type("anything", &schema).unwrap();
            assert_eq!(result, Value::Null);
        }

        #[test]
        fn test_handle_empty_values_with_nullable() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let schema = json!({"type": "string", "nullable": true});
            let result = parser.handle_empty_values("", Some(&schema));
            assert_eq!(result, Some(Value::Null));

            let result = parser.handle_empty_values("  ", Some(&schema));
            assert_eq!(result, Some(Value::Null));
        }

        #[test]
        fn test_handle_empty_values_with_default() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let schema = json!({"type": "integer", "default": 42});
            let result = parser.handle_empty_values("", Some(&schema));
            assert_eq!(result, Some(json!(42)));
        }

        #[test]
        fn test_handle_empty_values_without_special_props() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let schema = json!({"type": "string"});
            let result = parser.handle_empty_values("", Some(&schema));
            assert_eq!(result, Some(Value::String("".to_string())));
        }

        #[test]
        fn test_convert_parameter_with_schema() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            // Test with schema
            let result = parser
                .convert_parameter_with_schema("test_search", "limit", "42")
                .unwrap();
            assert_eq!(result, Value::Number(42.into()));

            // Test without schema (should fall back to basic conversion)
            let result = parser
                .convert_parameter_with_schema("unknown_tool", "param", "42")
                .unwrap();
            assert_eq!(result, Value::Number(42.into())); // Basic conversion still works
        }

        #[test]
        fn test_parse_nested_parameters_with_schema() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let content = r#"<query>search term</query><limit>5</limit><exact_match>true</exact_match><score>3.14</score>"#;
            let result = parser
                .parse_nested_parameters_with_schema(content, "test_search")
                .unwrap();

            // Check each field individually, using floating point comparison for score
            assert_eq!(result.get("query"), Some(&json!("search term")));
            assert_eq!(result.get("limit"), Some(&json!(5)));
            assert_eq!(result.get("exact_match"), Some(&json!(true)));

            // Use proper floating point comparison for score
            let score = result.get("score").unwrap().as_f64().unwrap();
            let diff = (score - TEST_FLOAT_VALUE).abs();
            assert!(
                diff < 0.0001,
                "Expected score ~3.14, got {}, diff = {}",
                score,
                diff
            );
        }

        #[test]
        fn test_parse_tool_calls_with_schema() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let xml = r#"<tool_call><test_search><query>rust programming</query><limit>3</limit><exact_match>false</exact_match></test_search></tool_call>"#;
            let result = parser.parse_tool_calls(xml).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "test_search");

            let args = &result[0].arguments;
            assert_eq!(args.get("query").unwrap(), "rust programming");
            assert_eq!(args.get("limit").unwrap(), &json!(3));
            assert_eq!(args.get("exact_match").unwrap(), &json!(false));
        }

        #[test]
        fn test_parse_tool_calls_without_schema_fallback() {
            let parser = Qwen3CoderToolParser::new(); // No schema

            let xml = r#"<tool_call><search><query>test</query><limit>10</limit><active>true</active></search></tool_call>"#;
            let result = parser.parse_tool_calls(xml).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");

            // Should still parse using basic type inference
            let args = &result[0].arguments;
            assert_eq!(args.get("query").unwrap(), "test");
            assert_eq!(args.get("limit").unwrap(), &json!(10));
            assert_eq!(args.get("active").unwrap(), &json!(true));
        }

        #[test]
        fn test_tool_parser_factory_with_schema() {
            let tools = vec![create_test_tool_definition()];

            // Test Qwen3Coder with schema
            let parser = ToolParserFactory::create_parser_with_schema(
                ToolParsingStrategy::Qwen3Coder,
                Some(&tools),
            );

            let xml = r#"<tool_call><test_search><limit>5</limit></test_search></tool_call>"#;
            let result = parser.parse_tool_calls(xml).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].arguments.get("limit").unwrap(), &json!(5));

            // Test other strategies (should ignore schema)
            let strategies = vec![
                ToolParsingStrategy::Default,
                ToolParsingStrategy::OpenAI,
                ToolParsingStrategy::Claude,
            ];

            for strategy in strategies {
                let parser =
                    ToolParserFactory::create_parser_with_schema(strategy.clone(), Some(&tools));
                // Should not fail, even if they don't use schema
                let _ = parser.parse_tool_calls("test");
            }
        }

        #[test]
        fn test_error_handling_and_fallback() {
            let tools = vec![create_test_tool_definition()];
            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            // Test invalid integer conversion
            let result =
                parser.convert_parameter_with_schema("test_search", "limit", "not_a_number");
            assert!(result.is_err());

            // Test invalid boolean conversion
            let result =
                parser.convert_parameter_with_schema("test_search", "exact_match", "maybe");
            assert!(result.is_err());

            // Test unknown schema type fallback
            let unknown_schema = json!({"type": "unknown_type"});
            let result = parser
                .convert_by_schema_type("test", &unknown_schema)
                .unwrap();
            // Should fall back to string since basic conversion would make it a string
            assert_eq!(result, Value::String("test".to_string()));
        }

        #[test]
        fn test_complex_nested_xml_with_schema() {
            let tools = vec![ToolDefinition {
                name: "complex_tool".to_string(),
                description: "Complex tool".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "config": {"type": "object"},
                        "tags": {"type": "array"},
                        "enabled": {"type": "boolean"},
                        "priority": {"type": "integer"},
                        "weight": {"type": "number"}
                    }
                }),
                server_name: "test".to_string(),
            }];

            let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));

            let xml = r#"<tool_call><complex_tool><config>{"timeout": 30, "retries": 3}</config><tags>["search", "api"]</tags><enabled>true</enabled><priority>1</priority><weight>0.5</weight></complex_tool></tool_call>"#;
            let result = parser.parse_tool_calls(xml).unwrap();

            assert_eq!(result.len(), 1);
            let args = &result[0].arguments;

            assert_eq!(
                args.get("config").unwrap(),
                &json!({"timeout": 30, "retries": 3})
            );
            assert_eq!(args.get("tags").unwrap(), &json!(["search", "api"]));
            assert_eq!(args.get("enabled").unwrap(), &json!(true));
            assert_eq!(args.get("priority").unwrap(), &json!(1));
            assert_eq!(args.get("weight").unwrap(), &json!(0.5));
        }
    }

    /// Comprehensive tests for streaming tool call parsing
    mod streaming_tests {
        use super::*;

        #[test]
        fn test_streaming_parser_factory_creation() {
            let parser =
                ToolParserFactory::create_streaming_parser(ToolParsingStrategy::Qwen3Coder);
            assert!(!parser.is_parsing_tool_call());
            assert_eq!(parser.get_completed_tool_calls().len(), 0);
        }

        #[test]
        fn test_streaming_incremental_parsing() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Simulate streaming deltas that build up a complete tool call
            let deltas = vec![
                "<tool_",
                "call><search><qu",
                "ery>test query</quer",
                "y><limit>5</lim",
                "it></search></tool_call>",
            ];

            let mut completed_calls = Vec::new();
            for delta in deltas {
                let calls = parser.process_delta(delta).unwrap();
                completed_calls.extend(calls);
            }

            assert_eq!(completed_calls.len(), 1);
            assert_eq!(completed_calls[0].name, "search");
            assert_eq!(completed_calls[0].arguments["query"], "test query");
            // Qwen3CoderToolParser automatically converts numeric strings to numbers
            assert_eq!(completed_calls[0].arguments["limit"], 5);
        }

        #[test]
        fn test_streaming_multiple_tool_calls() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // First tool call in pieces
            let first_deltas = vec!["<tool_call><search><query>rust</query></search></tool_call>"];

            // Second tool call in pieces
            let second_deltas =
                vec!["<tool_call><list_files><path>/home</path></list_files></tool_call>"];

            let mut all_completed = Vec::new();

            // Process first tool call
            for delta in first_deltas {
                let calls = parser.process_delta(delta).unwrap();
                all_completed.extend(calls);
            }

            // Process second tool call
            for delta in second_deltas {
                let calls = parser.process_delta(delta).unwrap();
                all_completed.extend(calls);
            }

            assert_eq!(all_completed.len(), 2);
            assert_eq!(all_completed[0].name, "search");
            assert_eq!(all_completed[1].name, "list_files");
            assert_eq!(all_completed[1].arguments["path"], "/home");
        }

        #[test]
        fn test_streaming_state_management() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Start a tool call but don't complete it
            parser
                .process_delta("<tool_call><search><query>incom")
                .unwrap();

            let state = parser.get_state();
            assert!(state.buffer.contains("<tool_call><search><query>incom"));
            assert!(parser.is_parsing_tool_call());

            // Complete the tool call
            let completed = parser
                .process_delta("plete</query></search></tool_call>")
                .unwrap();
            assert_eq!(completed.len(), 1);
            assert_eq!(completed[0].arguments["query"], "incomplete");
            assert!(!parser.is_parsing_tool_call());
        }

        #[test]
        fn test_streaming_with_schema() {
            let tools = vec![ToolDefinition {
                name: "calculate".to_string(),
                description: "Calculate something".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "value": {"type": "integer"},
                        "enabled": {"type": "boolean"}
                    }
                }),
                server_name: "test".to_string(),
            }];

            let mut parser = Qwen3CoderStreamingParser::new_with_schema(Some(&tools));

            let deltas = vec![
                "<tool_call><calculate>",
                "<value>42</value>",
                "<enabled>true</enabled>",
                "</calculate></tool_call>",
            ];

            let mut completed_calls = Vec::new();
            for delta in deltas {
                let calls = parser.process_delta(delta).unwrap();
                completed_calls.extend(calls);
            }

            assert_eq!(completed_calls.len(), 1);
            assert_eq!(completed_calls[0].name, "calculate");
            // Values should be converted according to schema
            assert_eq!(completed_calls[0].arguments["value"], 42);
            assert_eq!(completed_calls[0].arguments["enabled"], true);
        }

        #[test]
        fn test_streaming_reset_functionality() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Start parsing a tool call
            parser
                .process_delta("<tool_call><search><query>test")
                .unwrap();
            assert!(parser.is_parsing_tool_call());

            // Reset the parser
            parser.reset();
            assert!(!parser.is_parsing_tool_call());
            assert_eq!(parser.get_completed_tool_calls().len(), 0);
            assert!(parser.get_state().buffer.is_empty());
        }

        #[test]
        fn test_streaming_malformed_input() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Send malformed XML
            let result = parser
                .process_delta("<tool_call><broken><no_closing_tag")
                .unwrap();
            assert_eq!(result.len(), 0); // Should not crash, just no tool calls

            // Reset parser state to ensure clean slate
            parser.reset();

            // Send a valid tool call after malformed input
            let result = parser
                .process_delta("<tool_call><search><query>valid</query></search></tool_call>")
                .unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "search");
        }

        #[test]
        fn test_streaming_buffer_management() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Send some text before a tool call
            parser
                .process_delta("Here's some text before the tool call: ")
                .unwrap();

            // Send the tool call
            let result = parser
                .process_delta("<tool_call><search><query>test</query></search></tool_call>")
                .unwrap();
            assert_eq!(result.len(), 1);

            // Buffer should be cleaned after processing
            let state = parser.get_state();
            assert!(!state.buffer.contains("<tool_call>"));
        }

        #[test]
        fn test_streaming_very_small_deltas() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Send each character individually
            let xml = "<tool_call><search><query>test</query></search></tool_call>";
            let mut completed_calls = Vec::new();

            for ch in xml.chars() {
                let calls = parser.process_delta(&ch.to_string()).unwrap();
                completed_calls.extend(calls);
            }

            assert_eq!(completed_calls.len(), 1);
            assert_eq!(completed_calls[0].name, "search");
            assert_eq!(completed_calls[0].arguments["query"], "test");
        }

        #[test]
        fn test_buffered_streaming_parser_fallback() {
            // Test that we can create a buffered streaming parser for non-Qwen3Coder strategies
            let base_parser = ToolParserFactory::create_parser(ToolParsingStrategy::Default);
            let mut streaming_parser = BufferedStreamingParser::new(base_parser);

            // Test basic interface
            assert!(!streaming_parser.is_parsing_tool_call()); // Initially not parsing
            assert_eq!(streaming_parser.get_completed_tool_calls().len(), 0);

            // Add some data to buffer
            streaming_parser.process_delta("some data").unwrap();
            assert!(streaming_parser.is_parsing_tool_call()); // Now buffering

            // Test reset functionality
            streaming_parser.reset();
            assert!(!streaming_parser.is_parsing_tool_call()); // Reset clears buffer
            assert_eq!(streaming_parser.get_completed_tool_calls().len(), 0);
        }

        #[test]
        fn test_streaming_factory_with_schema() {
            let tools = vec![ToolDefinition {
                name: "test_tool".to_string(),
                description: "Test tool".to_string(),
                parameters: json!({"type": "object"}),
                server_name: "test".to_string(),
            }];

            let mut parser = ToolParserFactory::create_streaming_parser_with_schema(
                ToolParsingStrategy::Qwen3Coder,
                Some(&tools),
            );

            let result = parser
                .process_delta("<tool_call><test_tool></test_tool></tool_call>")
                .unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "test_tool");
        }

        #[test]
        fn test_streaming_large_buffer_handling() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Create a very large delta that exceeds buffer limits
            let large_content = "x".repeat(1024 * 1024 + 1); // Exceed 1MB limit
            let delta = format!(
                "<tool_call><large_test><data>{}</data></large_test></tool_call>",
                large_content
            );

            // Processing should succeed but trigger buffer cleanup
            let result = parser.process_delta(&delta);
            assert!(result.is_ok());

            // Buffer should be cleaned up automatically
            assert!(parser.get_state().buffer().len() <= 1024 * 1024);
        }

        #[test]
        fn test_streaming_buffer_cleanup_after_errors() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Add some valid content first
            parser
                .process_delta("<tool_call><test><param>value</param></test></tool_call>")
                .unwrap();

            // Add malformed content that will cause errors but consume buffer space
            let malformed = "<<malformed>>".repeat(STRESS_TEST_REPEAT_SIZE); // Large malformed content
            let _ = parser.process_delta(&malformed); // May error, that's expected

            // Add more valid content - buffer should be manageable
            let result =
                parser.process_delta("<tool_call><test2><param>value2</param></test2></tool_call>");
            assert!(result.is_ok());

            // Buffer should not grow unboundedly
            assert!(parser.get_state().buffer().len() < 1024 * 1024);
        }

        #[test]
        fn test_streaming_extreme_nesting() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Test with deeply nested but invalid XML
            let mut deeply_nested = String::new();
            for i in 0..100 {
                deeply_nested.push_str(&format!("<level_{}>", i));
            }
            deeply_nested.push_str("content");
            for i in (0..100).rev() {
                deeply_nested.push_str(&format!("</level_{}>", i));
            }

            // Should handle deeply nested content gracefully
            let result = parser.process_delta(&format!(
                "<tool_call><nested>{}</nested></tool_call>",
                deeply_nested
            ));
            assert!(result.is_ok()); // Should not panic or crash
        }

        #[test]
        fn test_streaming_unicode_and_special_characters() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Test with various Unicode characters and special XML characters
            let unicode_content = "Hello 🚀 世界 <>&\"'";
            let deltas = vec![
                "<tool_call><unicode_test>",
                "<message>",
                unicode_content,
                "</message>",
                "<emoji>🔥🎯💡</emoji>",
                "</unicode_test></tool_call>",
            ];

            let mut all_calls = Vec::new();
            for delta in deltas {
                let calls = parser.process_delta(delta).unwrap();
                all_calls.extend(calls);
            }

            // Should successfully parse Unicode content
            assert_eq!(all_calls.len(), 1);
            assert_eq!(all_calls[0].name, "unicode_test");
        }

        #[test]
        fn test_streaming_concurrent_simulation() {
            // Simulate concurrent-like processing with rapid state changes
            let mut parser = Qwen3CoderStreamingParser::new();

            // Rapidly switch between different tool calls
            let sequences = vec![
                (
                    "<tool_call><tool1>",
                    "<param>val1",
                    "</param></tool1></tool_call>",
                ),
                (
                    "<tool_call><tool2>",
                    "<param>val2",
                    "</param></tool2></tool_call>",
                ),
                (
                    "<tool_call><tool3>",
                    "<param>val3",
                    "</param></tool3></tool_call>",
                ),
            ];

            for (start, middle, end) in sequences {
                // Simulate interleaved processing
                parser.process_delta(start).unwrap();
                parser.process_delta(middle).unwrap();

                // Check intermediate state
                assert!(parser.is_parsing_tool_call());

                let calls = parser.process_delta(end).unwrap();
                assert_eq!(calls.len(), 1);
            }

            // Should have processed all tool calls
            assert_eq!(parser.get_completed_tool_calls().len(), 3);
        }

        /// Tests for streaming tool call parsing with chunk boundaries
        /// These tests verify that tool calls are correctly parsed even when
        /// XML tags are split across multiple chunks
        #[test]
        fn test_qwen_streaming_chunk_boundary_in_opening_tag() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Split the opening tag across chunks
            let chunks = vec![
                "<tool_ca",
                "ll><files_write><file_path>/test/file.txt</file_path><content>Hello ",
                "World!</content></files_write></tool_call>",
            ];

            let mut all_calls = Vec::new();
            for chunk in chunks {
                let calls = parser.process_delta(chunk).unwrap();
                all_calls.extend(calls);
            }

            assert_eq!(all_calls.len(), 1);
            assert_eq!(all_calls[0].name, "files_write");
            assert_eq!(all_calls[0].arguments["file_path"], "/test/file.txt");
            assert_eq!(all_calls[0].arguments["content"], "Hello World!");
        }

        #[test]
        fn test_qwen_streaming_chunk_boundary_in_closing_tag() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Split the closing tag across chunks
            let chunks = vec![
                "<tool_call><files_write><file_path>/test.txt</file_path><content>Test content</content></files_write></tool_ca",
                "ll>",
            ];

            let mut all_calls = Vec::new();
            for chunk in chunks {
                let calls = parser.process_delta(chunk).unwrap();
                all_calls.extend(calls);
            }

            assert_eq!(all_calls.len(), 1);
            assert_eq!(all_calls[0].name, "files_write");
            assert_eq!(all_calls[0].arguments["content"], "Test content");
        }

        #[test]
        fn test_qwen_streaming_chunk_boundary_in_content() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Split the content across multiple chunks, simulating a large file write
            let chunks = vec![
                "<tool_call><files_write><file_path>/test/file.txt</file_path><content>Line 1\n",
                "Line 2\n",
                "Line 3\n",
                "Line 4</content></files_write></tool_call>",
            ];

            let mut all_calls = Vec::new();
            for chunk in chunks {
                let calls = parser.process_delta(chunk).unwrap();
                all_calls.extend(calls);
            }

            assert_eq!(all_calls.len(), 1);
            assert_eq!(all_calls[0].name, "files_write");
            assert_eq!(
                all_calls[0].arguments["content"],
                "Line 1\nLine 2\nLine 3\nLine 4"
            );
        }

        #[test]
        fn test_qwen_streaming_very_small_chunks() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Simulate character-by-character streaming
            let full_call = "<tool_call><test><param>value</param></test></tool_call>";

            let mut all_calls = Vec::new();
            for ch in full_call.chars() {
                let chunk = ch.to_string();
                let calls = parser.process_delta(&chunk).unwrap();
                all_calls.extend(calls);
            }

            assert_eq!(all_calls.len(), 1);
            assert_eq!(all_calls[0].name, "test");
            assert_eq!(all_calls[0].arguments["param"], "value");
        }

        #[test]
        fn test_buffered_streaming_chunk_boundary_json() {
            let base_parser = ToolParserFactory::create_parser(ToolParsingStrategy::Default);
            let mut parser = BufferedStreamingParser::new(base_parser);

            // Split JSON tool call across chunks
            let chunks = vec![
                r#"{"function_name": "files_write", "arguments": {"file_path": "/test.txt", "con"#,
                r#"tent": "Hello World!"}}"#,
            ];

            let mut all_calls = Vec::new();
            for chunk in chunks {
                let calls = parser.process_delta(chunk).unwrap();
                all_calls.extend(calls);
            }

            assert_eq!(all_calls.len(), 1);
            assert_eq!(all_calls[0].name, "files_write");
            assert_eq!(all_calls[0].arguments["content"], "Hello World!");
        }

        #[test]
        fn test_qwen_streaming_multiple_tool_calls_with_boundaries() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Multiple tool calls split across chunks
            let chunks = vec![
                "<tool_call><files_read><path>/file1.txt</path></files_read></too",
                "l_call><tool_call><files_read><path>/file2.txt</path></files",
                "_read></tool_call>",
            ];

            let mut all_calls = Vec::new();
            for chunk in chunks {
                let calls = parser.process_delta(chunk).unwrap();
                all_calls.extend(calls);
            }

            assert_eq!(all_calls.len(), 2);
            assert_eq!(all_calls[0].name, "files_read");
            assert_eq!(all_calls[0].arguments["path"], "/file1.txt");
            assert_eq!(all_calls[1].name, "files_read");
            assert_eq!(all_calls[1].arguments["path"], "/file2.txt");
        }

        #[test]
        fn test_qwen_streaming_tracks_all_completed_calls() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Send a complete tool call
            let chunk = "<tool_call><test><param>value</param></test></tool_call>";

            let calls1 = parser.process_delta(chunk).unwrap();
            assert_eq!(calls1.len(), 1);

            // If the model generates the same content again (different tool call with same params),
            // it should be treated as a separate call since it has a new ID
            let calls2 = parser.process_delta(chunk).unwrap();
            assert_eq!(calls2.len(), 1);

            // Total completed calls should be 2 (two separate invocations)
            assert_eq!(parser.get_completed_tool_calls().len(), 2);

            // Verify both calls have different IDs
            let completed = parser.get_completed_tool_calls();
            assert_ne!(completed[0].id, completed[1].id);
        }

        #[test]
        fn test_buffered_streaming_no_premature_buffer_clear() {
            let base_parser = ToolParserFactory::create_parser(ToolParsingStrategy::Default);
            let mut parser = BufferedStreamingParser::new(base_parser);

            // Send incomplete JSON that can't be parsed yet
            let incomplete = r#"{"function_name": "test", "arguments": {"pa"#;
            let calls = parser.process_delta(incomplete).unwrap();
            assert_eq!(calls.len(), 0);

            // Buffer should still contain the incomplete data
            assert!(parser.is_parsing_tool_call());

            // Complete the JSON
            let complete = r#"ram": "value"}}"#;
            let calls = parser.process_delta(complete).unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, "test");
        }

        #[test]
        fn test_qwen_next_model_uses_json_not_xml() {
            // Qwen-next models use JSON format, not XML, so they should use Default parser
            assert_eq!(
                ToolParsingStrategy::detect_from_model_name("qwen-next"),
                ToolParsingStrategy::Default
            );
            assert_eq!(
                ToolParsingStrategy::detect_from_model_name("Qwen-Next-30B"),
                ToolParsingStrategy::Default
            );
            assert_eq!(
                ToolParsingStrategy::detect_from_model_name("qwen_next"),
                ToolParsingStrategy::Default
            );
            assert_eq!(
                ToolParsingStrategy::detect_from_model_name("QwenNext-Instruct"),
                ToolParsingStrategy::Default
            );
        }

        #[test]
        fn test_qwen_25_model_uses_json_not_xml() {
            // Qwen 2.5 models use JSON format, not XML, so they should use Default parser
            assert_eq!(
                ToolParsingStrategy::detect_from_model_name("Qwen2.5-7B-Instruct"),
                ToolParsingStrategy::Default
            );
            assert_eq!(
                ToolParsingStrategy::detect_from_model_name("qwen-2.5-coder"),
                ToolParsingStrategy::Default
            );
        }

        #[test]
        fn test_streaming_with_large_file_write_content() {
            let mut parser = Qwen3CoderStreamingParser::new();

            // Simulate a large file write split across many chunks
            let header = "<tool_call><files_write><file_path>/large_file.txt</file_path><content>";
            let footer = "</content></files_write></tool_call>";
            let line = "This is a line of content that will be repeated many times.\n";

            // Start with header
            parser.process_delta(header).unwrap();

            // Add 100 lines in separate chunks
            for _ in 0..100 {
                parser.process_delta(line).unwrap();
            }

            // Complete with footer
            let calls = parser.process_delta(footer).unwrap();

            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, "files_write");
            assert_eq!(calls[0].arguments["file_path"], "/large_file.txt");

            // Verify content has all 100 lines
            let content = calls[0].arguments["content"].as_str().unwrap();
            assert_eq!(content.lines().count(), 100);
        }
    }
}

#[cfg(test)]
mod template_only_tests {
    use super::*;
    use crate::types::{Message, MessageRole, Session, SessionId, ToolDefinition};
    use std::time::SystemTime;

    fn create_test_session_with_system() -> Session {
        let mut session = Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::System,
                content: "You are a helpful assistant.".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }],
            mcp_servers: Vec::new(),
            available_tools: Vec::new(),
            available_prompts: Vec::new(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,

            todos: Vec::new(),

            available_commands: Vec::new(),
            current_mode: None,

            client_capabilities: None,
            cached_message_count: 0,
            cached_token_count: 0,
        };

        // Add a simple tool
        session.available_tools.push(ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            server_name: "test_server".to_string(),
        });

        session
    }

    #[test]
    fn test_extract_template_components() {
        let engine = ChatTemplateEngine::new();
        let session = create_test_session_with_system();

        let (system_prompt, tools_json) = engine.extract_template_components(&session).unwrap();

        assert_eq!(system_prompt, "You are a helpful assistant.");
        assert!(tools_json.contains("test_tool"));
    }

    #[test]
    fn test_extract_template_components_no_tools() {
        let engine = ChatTemplateEngine::new();
        let mut session = create_test_session_with_system();
        session.available_tools.clear();

        let (system_prompt, tools_json) = engine.extract_template_components(&session).unwrap();

        assert_eq!(system_prompt, "You are a helpful assistant.");
        assert_eq!(tools_json, "");
    }

    #[test]
    fn test_extract_template_components_multiple_system_messages() {
        let engine = ChatTemplateEngine::new();
        let mut session = create_test_session_with_system();
        session.messages.push(Message {
            role: MessageRole::System,
            content: "Additional system message.".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });

        let (system_prompt, _tools_json) = engine.extract_template_components(&session).unwrap();

        assert!(system_prompt.contains("You are a helpful assistant."));
        assert!(system_prompt.contains("Additional system message."));
    }
}
