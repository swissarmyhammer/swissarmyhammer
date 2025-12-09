//! Prompt definition and template types.
//!
//! This module contains types for working with MCP prompt templates,
//! including prompt definitions, arguments, messages, and resources.

use serde::{Deserialize, Serialize};

/// Definition of a prompt template available through MCP servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDefinition {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub arguments: Option<Vec<PromptArgument>>,
    pub server_name: String,
}

/// Argument definition for a prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: Option<bool>,
}

/// A message within a prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: PromptRole,
    pub content: PromptContent,
}

/// Role for prompt messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PromptRole {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

/// Content types for prompt messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PromptContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: PromptResource },
}

/// Resource reference within prompt content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptResource {
    pub uri: String,
    pub name: String,
    pub title: Option<String>,
    pub mime_type: String,
    pub text: Option<String>,
}

/// Result of retrieving a prompt from an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPromptResult {
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
}
