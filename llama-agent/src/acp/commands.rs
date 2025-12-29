use crate::mcp::MCPClient;
use agent_client_protocol::{AvailableCommand, AvailableCommandInput, UnstructuredCommandInput};
use std::sync::Arc;

/// Registry for managing available slash commands
///
/// Queries MCP servers for available prompts and converts them to ACP AvailableCommands.
/// This allows the agent to dynamically discover and advertise commands based on
/// connected MCP servers.
pub struct CommandRegistry {
    mcp_client: Arc<dyn MCPClient>,
}

impl CommandRegistry {
    /// Create a new command registry with the given MCP client
    pub fn new(mcp_client: Arc<dyn MCPClient>) -> Self {
        Self { mcp_client }
    }

    /// Get all available commands, including core commands and MCP-based commands
    ///
    /// Queries MCP servers via `prompts/list` and converts each prompt to a command.
    /// Core commands (like /help) are always included.
    pub async fn get_available_commands(&self) -> Result<Vec<AvailableCommand>, String> {
        let mut commands = Vec::new();

        // Add core commands
        commands.push(AvailableCommand::new("/help", "Show available commands"));

        // Query MCP servers for prompts
        match self.mcp_client.list_prompts().await {
            Ok(prompts) => {
                // Convert each MCP prompt to an ACP slash command
                for prompt in prompts {
                    let command_name = format!("/{}", prompt.name);

                    // Use the prompt's description, or fall back to a generic description
                    let description = prompt
                        .description
                        .unwrap_or_else(|| format!("Execute {} prompt", prompt.name));

                    // Create the command
                    let mut command = AvailableCommand::new(command_name, description);

                    // If the prompt has arguments, add input specification and parameter schema
                    if let Some(arguments) = prompt.arguments {
                        if !arguments.is_empty() {
                            // Build a hint string from the argument names
                            let arg_names: Vec<String> = arguments
                                .iter()
                                .map(|arg| {
                                    if arg.required.unwrap_or(false) {
                                        format!("<{}>", arg.name)
                                    } else {
                                        format!("[{}]", arg.name)
                                    }
                                })
                                .collect();

                            let hint = arg_names.join(" ");

                            // Add parameter schema to meta field for structured parameter handling
                            let parameters_schema: Vec<serde_json::Value> = arguments
                                .iter()
                                .map(|arg| {
                                    serde_json::json!({
                                        "name": arg.name,
                                        "description": arg.description,
                                        "required": arg.required.unwrap_or(false),
                                    })
                                })
                                .collect();

                            // Create meta maps for input and command
                            let mut input_meta = serde_json::Map::new();
                            input_meta.insert(
                                "parameters".to_string(),
                                serde_json::Value::Array(parameters_schema.clone()),
                            );

                            let mut command_meta = serde_json::Map::new();
                            command_meta.insert(
                                "parameters".to_string(),
                                serde_json::Value::Array(parameters_schema),
                            );

                            command = command
                                .input(AvailableCommandInput::Unstructured(
                                    UnstructuredCommandInput::new(hint).meta(input_meta),
                                ))
                                .meta(command_meta);
                        }
                    }

                    commands.push(command);
                }
            }
            Err(e) => {
                // Log error but don't fail - just return core commands
                tracing::warn!("Failed to query MCP prompts: {}", e);
            }
        }

        Ok(commands)
    }

    /// Check if the available commands have changed
    ///
    /// Compares two command lists to determine if a notification should be sent
    /// to the client about command availability changes.
    pub fn has_commands_changed(
        &self,
        old_commands: &[AvailableCommand],
        new_commands: &[AvailableCommand],
    ) -> bool {
        old_commands.len() != new_commands.len()
            || old_commands
                .iter()
                .zip(new_commands.iter())
                .any(|(a, b)| a.name != b.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::{MCPClient, NoOpMCPClient};
    use crate::types::errors::MCPError;
    use async_trait::async_trait;
    use rmcp::model::{Prompt, PromptArgument};
    use serde_json::Value;
    use std::collections::HashMap;

    /// Mock MCP client that returns test prompts
    struct MockMCPClient {
        prompts: Vec<Prompt>,
    }

    impl MockMCPClient {
        fn new(prompts: Vec<Prompt>) -> Self {
            Self { prompts }
        }
    }

    #[async_trait]
    impl MCPClient for MockMCPClient {
        async fn list_tools(&self) -> Result<Vec<String>, MCPError> {
            Ok(vec![])
        }

        async fn call_tool(&self, _name: &str, _arguments: Value) -> Result<String, MCPError> {
            Err(MCPError::ServerNotFound("Mock client".to_string()))
        }

        async fn list_prompts(&self) -> Result<Vec<Prompt>, MCPError> {
            Ok(self.prompts.clone())
        }

        async fn get_prompt(
            &self,
            _name: &str,
            _arguments: Option<HashMap<String, Value>>,
        ) -> Result<Vec<String>, MCPError> {
            Err(MCPError::ServerNotFound("Mock client".to_string()))
        }

        async fn health_check(&self) -> Result<(), MCPError> {
            Ok(())
        }

        async fn shutdown_all(&self) -> Result<(), MCPError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_core_commands_always_present() {
        let mcp_client = Arc::new(NoOpMCPClient::new());
        let registry = CommandRegistry::new(mcp_client);

        let commands = registry.get_available_commands().await.unwrap();

        // Should have at least the core /help command
        assert!(!commands.is_empty());
        assert!(commands.iter().any(|c| c.name == "/help"));
    }

    #[tokio::test]
    async fn test_includes_both_core_and_mcp_commands() {
        // Create mock prompts from MCP
        let prompts = vec![
            Prompt {
                name: "test".to_string(),
                title: None,
                description: Some("Run tests".to_string()),
                arguments: None,
                icons: None,
                meta: None,
            },
            Prompt {
                name: "plan".to_string(),
                title: None,
                description: Some("Create a plan".to_string()),
                arguments: Some(vec![PromptArgument {
                    name: "spec".to_string(),
                    title: None,
                    description: Some("Specification file".to_string()),
                    required: Some(true),
                }]),
                icons: None,
                meta: None,
            },
        ];

        let mcp_client = Arc::new(MockMCPClient::new(prompts));
        let registry = CommandRegistry::new(mcp_client);

        let commands = registry.get_available_commands().await.unwrap();

        // Should have at least 3 commands: /help (core) + /test + /plan (MCP)
        assert!(commands.len() >= 3);

        // Verify core command is present
        assert!(commands.iter().any(|c| c.name == "/help"));

        // Verify MCP-based commands are present
        assert!(commands.iter().any(|c| c.name == "/test"));
        assert!(commands.iter().any(|c| c.name == "/plan"));

        // Verify command with arguments has input specification
        let plan_cmd = commands.iter().find(|c| c.name == "/plan").unwrap();
        assert!(plan_cmd.input.is_some());
    }

    #[tokio::test]
    async fn test_commands_changed_detection() {
        let mcp_client = Arc::new(NoOpMCPClient::new());
        let registry = CommandRegistry::new(mcp_client);

        let commands1 = vec![AvailableCommand::new("/help", "Help")];

        let commands2 = vec![
            AvailableCommand::new("/help", "Help"),
            AvailableCommand::new("/test", "Test"),
        ];

        // Different lengths should be detected
        assert!(registry.has_commands_changed(&commands1, &commands2));

        // Same commands should not be detected as changed
        assert!(!registry.has_commands_changed(&commands1, &commands1));
    }

    #[tokio::test]
    async fn test_commands_changed_by_name() {
        let mcp_client = Arc::new(NoOpMCPClient::new());
        let registry = CommandRegistry::new(mcp_client);

        let commands1 = vec![AvailableCommand::new("/help", "Help")];

        let commands2 = vec![AvailableCommand::new("/test", "Test")];

        // Different names should be detected
        assert!(registry.has_commands_changed(&commands1, &commands2));
    }
}
