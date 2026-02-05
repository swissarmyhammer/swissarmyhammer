//! Session management and compaction types.
//!
//! This module contains types for managing conversation sessions,
//! including compaction configuration and session state.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::types::errors::SessionError;
use crate::types::ids::SessionId;
use crate::types::mcp::MCPServerConfig;
use crate::types::messages::{Message, MessageRole, SimpleTokenCounter, TokenCounter, TokenUsage};
use crate::types::prompts::PromptDefinition;
use crate::types::tools::ToolDefinition;

/// Context state tracking for incremental prompt processing.
///
/// This struct tracks the state of processed tokens to enable efficient incremental
/// processing where only new tokens are sent to the model context, avoiding
/// reprocessing of conversation history that's already in the KV cache.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextState {
    /// Tokens that have been processed into the context
    ///
    /// Stores the actual token IDs that have been sent to the model context.
    /// This allows for precise comparison with new prompts to determine
    /// what tokens can be skipped during incremental processing.
    pub processed_tokens: Vec<i32>, // Using i32 as llama_cpp_2 uses i32 for LlamaToken

    /// Current position in the context (number of tokens processed)
    ///
    /// Tracks the absolute position in the token sequence for proper
    /// batch positioning during incremental updates.
    pub current_position: usize,

    /// Hash of the last processed prompt for quick comparison
    ///
    /// Provides a fast way to detect if the prompt has changed before
    /// doing expensive token-by-token comparison.
    pub last_prompt_hash: u64,

    /// The actual last processed prompt text for detailed diffing
    ///
    /// Stored for debugging purposes and to enable fallback mechanisms
    /// when token-level diffing encounters issues.
    pub last_prompt_text: String,
}

impl ContextState {
    /// Create a new empty context state
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if this context state is empty (no tokens processed)
    pub fn is_empty(&self) -> bool {
        self.processed_tokens.is_empty()
    }

    /// Reset the context state to empty
    pub fn reset(&mut self) {
        self.processed_tokens.clear();
        self.current_position = 0;
        self.last_prompt_hash = 0;
        self.last_prompt_text.clear();
    }

    /// Update context state with new processed data
    pub fn update(&mut self, tokens: Vec<i32>, prompt: &str) {
        self.processed_tokens = tokens;
        self.current_position = self.processed_tokens.len();
        self.last_prompt_hash = Self::hash_string(prompt);
        self.last_prompt_text = prompt.to_string();
    }

    /// Check if a prompt matches the cached prompt (quick hash comparison)
    pub fn matches_prompt(&self, prompt: &str) -> bool {
        let prompt_hash = Self::hash_string(prompt);
        self.last_prompt_hash == prompt_hash && self.last_prompt_text == prompt
    }

    /// Hash a string for quick comparison
    fn hash_string(s: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }
}

/// Configuration for session compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// When to trigger compaction (0.0-1.0)
    ///
    /// Threshold represents the fraction of the model's context window at which compaction
    /// is triggered. For example, 0.8 means compact when token usage exceeds
    /// 80% of the context window.
    pub threshold: f32,

    /// Number of recent messages to preserve during compaction
    ///
    /// Recent messages are kept verbatim to maintain conversation continuity.
    /// Set to 0 to replace all messages with summary only.
    pub preserve_recent: usize,

    /// Optional custom prompt for summarization
    ///
    /// If provided, overrides the default compaction prompt with specialized
    /// instructions for domain-specific summarization.
    pub custom_prompt: Option<CompactionPrompt>,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            threshold: 0.8,
            preserve_recent: 0,
            custom_prompt: None,
        }
    }
}

impl CompactionConfig {
    /// Validate the compaction configuration parameters.
    pub fn validate(&self) -> Result<(), SessionError> {
        if !(0.0..=1.0).contains(&self.threshold) {
            return Err(SessionError::InvalidState(
                "Compaction threshold must be between 0.0 and 1.0".to_string(),
            ));
        }

        if self.preserve_recent > 1000 {
            return Err(SessionError::InvalidState(
                "Cannot preserve more than 1000 recent messages".to_string(),
            ));
        }

        Ok(())
    }
}

/// Metadata about a compaction operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionMetadata {
    pub compacted_at: SystemTime,
    pub original_message_count: usize,
    pub original_token_count: usize,
    pub compressed_token_count: usize,
    pub compression_ratio: f32,
    // Additional fields with defaults for compatibility
    pub timestamp: SystemTime,
    pub messages_before: usize,
    pub messages_after: usize,
    pub tokens_before: usize,
    pub tokens_after: usize,
}

impl CompactionMetadata {
    pub fn default_time() -> SystemTime {
        SystemTime::UNIX_EPOCH
    }
}

impl Default for CompactionMetadata {
    fn default() -> Self {
        Self {
            compacted_at: SystemTime::UNIX_EPOCH,
            original_message_count: 0,
            original_token_count: 0,
            compressed_token_count: 0,
            compression_ratio: 1.0,
            timestamp: SystemTime::UNIX_EPOCH,
            messages_before: 0,
            messages_after: 0,
            tokens_before: 0,
            tokens_after: 0,
        }
    }
}

impl CompactionMetadata {
    /// Create metadata from basic compaction info.
    pub fn new(
        original_token_count: usize,
        compressed_token_count: usize,
        original_message_count: usize,
    ) -> Self {
        let now = SystemTime::now();
        let compression_ratio = if original_token_count > 0 {
            compressed_token_count as f32 / original_token_count as f32
        } else {
            1.0
        };

        Self {
            compacted_at: now,
            original_message_count,
            original_token_count,
            compressed_token_count,
            compression_ratio,
            timestamp: now,
            messages_before: original_message_count,
            messages_after: 1, // Typically replaced with summary
            tokens_before: original_token_count,
            tokens_after: compressed_token_count,
        }
    }
}

/// Backup of session state before compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionBackup {
    pub session_id: SessionId,
    pub messages: Vec<Message>,
    pub backup_timestamp: SystemTime,
    pub compaction_reason: String,
    pub updated_at: SystemTime,
    pub compaction_history: Vec<CompactionMetadata>,
}

/// A conversation session containing messages and state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub messages: Vec<Message>,
    /// Working directory for this session (ACP requirement - must be absolute path)
    pub cwd: PathBuf,
    pub mcp_servers: Vec<MCPServerConfig>,
    pub available_tools: Vec<ToolDefinition>,
    pub available_prompts: Vec<PromptDefinition>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub compaction_history: Vec<CompactionMetadata>,
    pub transcript_path: Option<PathBuf>,
    /// Context state for incremental prompt processing
    ///
    /// Tracks the processed tokens and prompt state to enable efficient
    /// incremental processing where only new tokens are sent to the model.
    /// None indicates no context state is being tracked (full processing mode).
    pub context_state: Option<ContextState>,

    /// Kanban tasks associated with this session
    ///
    /// Tracks tasks and plan entries for this session. Used by the ACP server to send
    /// Plan notifications to clients showing the agent's execution plan.
    ///
    /// # ACP Integration
    ///
    /// When ACP feature is enabled, these tasks are automatically converted to ACP Plan
    /// format and sent to clients via SessionNotification::Plan updates. The conversion
    /// is handled by the `llama_agent::acp::plan::tasks_to_acp_plan` function.
    /// Available commands that can be invoked during this session
    ///
    /// Tracks slash commands that are available to the user in this session.
    /// Commands can be dynamically added/removed based on context, MCP server state,
    /// and session mode. This field is updated when:
    ///
    /// - Session is created (initialized with core commands)
    /// - MCP servers are connected/disconnected (adds/removes server-specific commands)
    /// - Session mode changes (enables/disables mode-specific commands)
    /// - Custom commands are registered via agent configuration
    ///
    /// # ACP Integration
    ///
    /// When ACP feature is enabled, changes to available_commands trigger
    /// AvailableCommandsUpdate notifications to inform clients of the current
    /// command set.
    pub available_commands: Vec<agent_client_protocol::AvailableCommand>,
    /// Current session mode identifier for ACP current mode updates
    ///
    /// Tracks the active mode for this session. When the mode changes,
    /// an ACP CurrentModeUpdate notification is sent to clients to update
    /// their UI and available functionality based on the new mode.
    ///
    /// # Mode Examples
    ///
    /// - `None`: Default/unspecified mode
    /// - `Some("planning")`: Agent is in planning/analysis mode
    /// - `Some("coding")`: Agent is actively writing code
    /// - `Some("debugging")`: Agent is debugging issues
    /// - `Some("research")`: Agent is researching the codebase
    ///
    /// # ACP Integration
    ///
    /// When ACP feature is enabled and this field changes, the agent should
    /// send a CurrentModeUpdate notification to inform clients of the mode transition.
    pub current_mode: Option<String>,
    /// Client capabilities from ACP initialize request
    ///
    /// Stores the client's declared capabilities for file system and terminal operations.
    /// These capabilities must be checked before performing any file or terminal operations
    /// to ensure the client supports the requested functionality.
    ///
    /// # Capability Enforcement
    ///
    /// - `fs.read_text_file`: Must be true before reading files
    /// - `fs.write_text_file`: Must be true before writing, creating, or deleting files
    /// - `terminal`: Must be true before terminal operations
    ///
    /// When capabilities are not available (None), file operations should not be performed
    /// in ACP contexts. For non-ACP contexts (MCP mode), this field is None and operations
    /// proceed without capability checks.
    pub client_capabilities: Option<agent_client_protocol::ClientCapabilities>,

    /// Number of messages that have been processed and saved in the session KV cache
    ///
    /// This tracks how many messages from the conversation history are already
    /// stored in the KV cache. When continuing a multi-turn conversation, only
    /// messages from this count onward need to be rendered and processed.
    ///
    /// # Multi-Turn Workflow
    ///
    /// 1. Turn 1: Process all messages (0 to N), save cache, set to N
    /// 2. Turn 2: New message added (N to N+1), render only message N+1, append to cache
    /// 3. Turn 3: Another message (N+1 to N+2), render only message N+2, append to cache
    ///
    /// This enables efficient multi-turn conversations without reprocessing the
    /// entire conversation history on each turn.
    #[serde(default)]
    pub cached_message_count: usize,

    /// 1. Turn 1: Process all N tokens, save state, set cached_token_count = N
    /// 2. Turn 2: Restore state (KV cache has N tokens), tokenize full prompt → M tokens
    ///    Process only tokens N..M using cached_token_count as offset
    /// 3. Turn 3: Restore state (KV cache has M tokens), tokenize full prompt → P tokens
    ///    Process only tokens M..P using cached_token_count as offset
    ///
    /// This allows efficient incremental processing where the restored KV cache
    /// contains all previous tokens and only new tokens need to be processed.
    #[serde(default)]
    pub cached_token_count: usize,
}

/// Prompt template for session compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionPrompt {
    pub system_instructions: String,
    pub user_template: String,
    pub user_prompt_template: String, // Legacy field for compatibility
}

impl Default for CompactionPrompt {
    fn default() -> Self {
        let user_template =
            "Please create a summary of the following conversation: {conversation_history}"
                .to_string();
        Self {
            system_instructions:
                "You are a helpful AI assistant that creates concise conversation summaries."
                    .to_string(),
            user_template: user_template.clone(),
            user_prompt_template: user_template,
        }
    }
}

impl CompactionPrompt {
    /// Create a CompactionPrompt from resource content.
    pub fn from_resource(resource_content: &str) -> Result<Self, crate::resources::ResourceError> {
        // Simple parsing - look for system and user sections
        let lines: Vec<&str> = resource_content.lines().collect();
        let mut system_instructions = String::new();
        let mut user_template = String::new();
        let mut current_section = "";

        for line in lines {
            if line.starts_with("# System Instructions") {
                current_section = "system";
                continue;
            } else if line.starts_with("# User Template")
                || line.starts_with("# User Prompt Template")
            {
                current_section = "user";
                continue;
            }

            match current_section {
                "system" => {
                    if !line.trim().is_empty() && !line.starts_with('#') {
                        system_instructions.push_str(line);
                        system_instructions.push('\n');
                    }
                }
                "user" => {
                    if !line.trim().is_empty() && !line.starts_with('#') {
                        user_template.push_str(line);
                        user_template.push('\n');
                    }
                }
                _ => {}
            }
        }

        if system_instructions.is_empty() {
            return Err(crate::resources::ResourceError::ParseError(
                "Missing system instructions section".to_string(),
            ));
        }

        if user_template.is_empty() {
            return Err(crate::resources::ResourceError::ParseError(
                "Missing user template section".to_string(),
            ));
        }

        Ok(Self {
            system_instructions: system_instructions.trim().to_string(),
            user_template: user_template.trim().to_string(),
            user_prompt_template: user_template.trim().to_string(), // Copy for compatibility
        })
    }

    /// Render the user prompt with conversation history.
    pub fn render_user_prompt(&self, conversation_history: &str) -> String {
        self.user_template
            .replace("{conversation_history}", conversation_history)
    }

    /// Load the default compaction prompt.
    pub fn load_default() -> Result<Self, crate::resources::ResourceError> {
        // Default system instructions for compaction
        let system_instructions = "You are an AI assistant specialized in creating conversation summaries. Your task is to create a concise, accurate summary that preserves important context while reducing token usage. Focus on key decisions, outcomes, and ongoing context that future messages might reference.".to_string();

        // Default user template
        let user_template = "Please summarize the following conversation history, preserving important context and decisions:\n\n{conversation_history}".to_string();

        Ok(Self {
            system_instructions,
            user_template: user_template.clone(),
            user_prompt_template: user_template, // Copy for compatibility
        })
    }

    /// Create messages for compaction based on conversation history.
    pub fn create_messages(&self, conversation_history: &str) -> Vec<Message> {
        vec![
            Message {
                role: MessageRole::System,
                content: self.system_instructions.clone(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::User,
                content: self.render_user_prompt(conversation_history),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
        ]
    }

    /// Estimate prompt tokens for the given conversation length.
    pub fn estimated_prompt_tokens(&self, conversation_length: usize) -> usize {
        // Simple estimation: base prompt tokens + conversation scaling
        let base_tokens = 50; // System instructions and template overhead
        let conversation_tokens = conversation_length / 6; // More conservative estimate
        base_tokens + conversation_tokens
    }

    /// Validate the compaction prompt configuration.
    pub fn validate(&self) -> Result<(), crate::resources::ResourceError> {
        if self.system_instructions.trim().is_empty() {
            return Err(crate::resources::ResourceError::ParseError(
                "System instructions cannot be empty".to_string(),
            ));
        }

        if self.system_instructions.trim().len() < 10 {
            return Err(crate::resources::ResourceError::ParseError(
                "System instructions must be at least 10 characters long".to_string(),
            ));
        }

        if self.user_template.trim().is_empty() {
            return Err(crate::resources::ResourceError::ParseError(
                "User template cannot be empty".to_string(),
            ));
        }

        if !self.user_template.contains("{conversation_history}") {
            return Err(crate::resources::ResourceError::ParseError(
                "User template must contain {conversation_history} placeholder".to_string(),
            ));
        }

        Ok(())
    }

    /// Create a custom compaction prompt.
    pub fn custom(
        system_instructions: impl Into<String>,
        user_template: impl Into<String>,
    ) -> Result<Self, crate::resources::ResourceError> {
        let system = system_instructions.into();
        let template = user_template.into();

        let prompt = Self {
            system_instructions: system,
            user_template: template.clone(),
            user_prompt_template: template,
        };

        prompt.validate()?;
        Ok(prompt)
    }
}

impl Session {
    /// Calculate token usage for all messages in the session.
    pub fn token_usage(&self) -> TokenUsage {
        let counter = SimpleTokenCounter::new();
        counter.count_session_tokens(self)
    }

    /// Check if this session should be compacted based on configuration.
    pub fn should_compact(&self, model_context_size: usize, threshold: f32) -> bool {
        let usage = self.token_usage();
        let threshold_tokens = (model_context_size as f32 * threshold) as usize;
        usage.total > threshold_tokens
    }

    /// Create a backup of the current session state.
    pub fn create_backup(&self) -> SessionBackup {
        SessionBackup {
            session_id: self.id,
            messages: self.messages.clone(),
            backup_timestamp: SystemTime::now(),
            compaction_reason: "Pre-compaction backup".to_string(),
            updated_at: self.updated_at,
            compaction_history: self.compaction_history.clone(),
        }
    }

    /// Restore session state from a backup.
    pub fn restore_from_backup(&mut self, backup: SessionBackup) {
        self.id = backup.session_id;
        self.messages = backup.messages;
        self.updated_at = backup.updated_at;
        self.compaction_history = backup.compaction_history;
    }

    /// Check if this session was recently compacted within the specified time window.
    pub fn was_recently_compacted(&self, within_minutes: u64) -> bool {
        if let Some(last_compaction) = self.compaction_history.last() {
            if let Ok(elapsed) = SystemTime::now().duration_since(last_compaction.compacted_at) {
                return elapsed.as_secs() < within_minutes * 60;
            }
        }
        false
    }

    /// Record completion of a compaction operation.
    pub fn record_compaction(&mut self, original_count: usize, original_tokens: usize) {
        let current_usage = self.token_usage();
        let compression_ratio = if original_tokens > 0 {
            current_usage.total as f32 / original_tokens as f32
        } else {
            1.0
        };

        let now = SystemTime::now();
        self.compaction_history.push(CompactionMetadata {
            compacted_at: now,
            original_message_count: original_count,
            original_token_count: original_tokens,
            compressed_token_count: current_usage.total,
            compression_ratio,
            timestamp: now,
            messages_before: original_count,
            messages_after: self.messages.len(),
            tokens_before: original_tokens,
            tokens_after: current_usage.total,
        });

        self.updated_at = now;
    }

    /// Compact the session using the provided configuration and summary generator.
    pub async fn compact<F, Fut>(
        &mut self,
        config: Option<CompactionConfig>,
        generate_summary: F,
    ) -> Result<(), SessionError>
    where
        F: FnOnce(Vec<Message>) -> Fut,
        Fut: std::future::Future<Output = Result<String, SessionError>>,
    {
        let config = config.unwrap_or_default();

        // Validate compaction readiness
        if self.messages.len() < 3 {
            return Err(SessionError::InvalidState(
                "Session must have at least 3 messages to compact".to_string(),
            ));
        }

        // Generate summary
        let summary = generate_summary(self.messages.clone()).await?;

        // Replace messages with summary, preserving recent ones
        let preserve_count = config.preserve_recent.min(self.messages.len());
        let preserved_messages = if preserve_count > 0 {
            self.messages
                .split_off(self.messages.len() - preserve_count)
        } else {
            Vec::new()
        };

        // Clear old messages and add summary
        self.messages.clear();
        self.messages.push(Message {
            role: MessageRole::System,
            content: format!("Previous conversation summary: {}", summary),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });

        // Add back preserved messages
        self.messages.extend(preserved_messages);

        // Reset context state since the conversation structure has changed
        if let Some(ref mut context_state) = self.context_state {
            context_state.reset();
        }

        self.updated_at = SystemTime::now();
        Ok(())
    }

    /// Format conversation history for summarization.
    pub fn format_conversation_history(&self) -> String {
        self.messages
            .iter()
            .map(|msg| format!("{}: {}", msg.role.as_str(), msg.content))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// Session token counting is implemented directly in Session methods
// The TokenCounter trait implementation is in messages.rs
