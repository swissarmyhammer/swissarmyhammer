//! Message and conversation types for chat interactions.
//!
//! This module contains the core message types used in conversations,
//! including message roles, token counting, and related utilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

use crate::types::ids::ToolCallId;

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub tool_call_id: Option<ToolCallId>,
    pub tool_name: Option<String>,
    pub timestamp: SystemTime,
}

/// The role of a message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl MessageRole {
    /// Get the string representation of this role.
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        }
    }
}

/// Token usage tracking for messages and sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub total: usize,
    pub by_role: HashMap<MessageRole, usize>,
    pub by_message: Vec<usize>,
}

impl TokenUsage {
    pub fn new() -> Self {
        Self {
            total: 0,
            by_role: HashMap::new(),
            by_message: Vec::new(),
        }
    }
}

impl Default for TokenUsage {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for counting tokens in text and messages.
pub trait TokenCounter {
    fn count_tokens(&self, text: &str) -> usize;
    fn count_message_tokens(&self, message: &Message) -> usize;
    fn count_session_tokens(&self, session: &crate::types::sessions::Session) -> TokenUsage;
}

/// Simple token counter implementation using word-based estimation.
#[derive(Debug, Clone)]
pub struct SimpleTokenCounter;

impl SimpleTokenCounter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SimpleTokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCounter for SimpleTokenCounter {
    fn count_tokens(&self, text: &str) -> usize {
        // Simple word-based estimation (words * 1.3 for subwords)
        (text.split_whitespace().count() as f64 * 1.3) as usize
    }

    fn count_message_tokens(&self, message: &Message) -> usize {
        // Account for role prefix and formatting
        let role_overhead = match message.role {
            MessageRole::System => 10,
            MessageRole::User => 8,
            MessageRole::Assistant => 12,
            MessageRole::Tool => 15,
        };
        self.count_tokens(&message.content) + role_overhead
    }

    fn count_session_tokens(&self, session: &crate::types::sessions::Session) -> TokenUsage {
        let mut usage = TokenUsage::new();

        for message in &session.messages {
            let message_tokens = self.count_message_tokens(message);
            usage.total += message_tokens;
            usage.by_message.push(message_tokens);

            // Update role-based counts
            *usage.by_role.entry(message.role.clone()).or_insert(0) += message_tokens;
        }

        usage
    }
}

// Session-related token counting will be implemented in a separate impl block
// after the Session type is moved to the sessions module.
