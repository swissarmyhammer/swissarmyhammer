use crate::mcp::MCPClient;
use agent_client_protocol::{AvailableCommand, AvailableCommandInput, UnstructuredCommandInput};
use std::sync::Arc;
use swissarmyhammer_common::is_prompt_visible;
use swissarmyhammer_skills::SkillLibrary;

/// Registry for managing available slash commands
///
/// Queries MCP servers for available prompts and the skill library, converting
/// both to ACP AvailableCommands. This allows the agent to dynamically discover
/// and advertise commands from MCP prompts and Agent Skills.
pub struct CommandRegistry {
    mcp_client: Arc<dyn MCPClient>,
    skill_library: Option<SkillLibrary>,
}

impl CommandRegistry {
    /// Create a new command registry with the given MCP client
    pub fn new(mcp_client: Arc<dyn MCPClient>) -> Self {
        Self {
            mcp_client,
            skill_library: None,
        }
    }

    /// Create a new command registry with both MCP client and skill library
    pub fn with_skills(mcp_client: Arc<dyn MCPClient>, skill_library: SkillLibrary) -> Self {
        Self {
            mcp_client,
            skill_library: Some(skill_library),
        }
    }

    /// Get all available commands, including core commands, skill commands,
    /// and MCP-based commands.
    ///
    /// Sources (in order):
    /// 1. Core commands (like /help) — always included
    /// 2. Skill commands — from the Agent Skills library
    /// 3. MCP prompt commands — from MCP servers via `prompts/list`
    ///
    /// Skill commands take the form `/<skill-name>` with an optional `[input]` argument.
    pub async fn get_available_commands(&self) -> Result<Vec<AvailableCommand>, String> {
        let mut commands = Vec::new();

        // Add core commands
        commands.push(AvailableCommand::new("/help", "Show available commands"));

        // Add skill-based commands
        if let Some(library) = &self.skill_library {
            for skill in library.list() {
                let command_name = format!("/{}", skill.name.as_str());
                let description = skill.description.clone();

                // All skills accept optional input arguments
                let mut input_meta = serde_json::Map::new();
                input_meta.insert(
                    "parameters".to_string(),
                    serde_json::json!([{
                        "name": "input",
                        "description": "Arguments to pass to the skill",
                        "required": false,
                    }])
                    .as_array()
                    .unwrap()
                    .clone()
                    .into(),
                );

                let mut command_meta = serde_json::Map::new();
                command_meta.insert("source".to_string(), "skill".into());

                let command = AvailableCommand::new(command_name, description)
                    .input(AvailableCommandInput::Unstructured(
                        UnstructuredCommandInput::new("[input]").meta(input_meta),
                    ))
                    .meta(command_meta);

                commands.push(command);
            }
        }

        // Query MCP servers for prompts
        match self.mcp_client.list_prompts().await {
            Ok(prompts) => {
                // Collect skill names to avoid duplicates
                let skill_names: std::collections::HashSet<String> = self
                    .skill_library
                    .as_ref()
                    .map(|lib| lib.names().into_iter().map(|n| n.to_string()).collect())
                    .unwrap_or_default();

                // Convert each MCP prompt to an ACP slash command, filtering out partials
                for prompt in prompts {
                    // Skip MCP prompts that duplicate a skill
                    if skill_names.contains(&prompt.name) {
                        continue;
                    }

                    // Filter out partial templates and hidden prompts
                    let description_str = prompt.description.as_deref();
                    let meta_value = prompt
                        .meta
                        .as_ref()
                        .and_then(|m| serde_json::to_value(m).ok());
                    if !is_prompt_visible(&prompt.name, description_str, meta_value.as_ref()) {
                        continue;
                    }

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
                // Log error but don't fail - just return core + skill commands
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

        async fn set_session(&self, _session_id: agent_client_protocol::SessionId) {
            // No-op for mock
        }

        async fn clear_session(&self) {
            // No-op for mock
        }
    }

    /// Helper to create a SkillLibrary loaded with defaults
    fn loaded_skill_library() -> SkillLibrary {
        let mut library = SkillLibrary::new();
        library.load_defaults();
        library
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
                name: "deploy".to_string(),
                title: None,
                description: Some("Deploy to production".to_string()),
                arguments: None,
                icons: None,
                meta: None,
            },
            Prompt {
                name: "migrate".to_string(),
                title: None,
                description: Some("Run migrations".to_string()),
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

        // Should have at least 3 commands: /help (core) + /deploy + /migrate (MCP)
        assert!(commands.len() >= 3);

        // Verify core command is present
        assert!(commands.iter().any(|c| c.name == "/help"));

        // Verify MCP-based commands are present
        assert!(commands.iter().any(|c| c.name == "/deploy"));
        assert!(commands.iter().any(|c| c.name == "/migrate"));

        // Verify command with arguments has input specification
        let migrate_cmd = commands.iter().find(|c| c.name == "/migrate").unwrap();
        assert!(migrate_cmd.input.is_some());
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

    #[tokio::test]
    async fn test_skills_appear_as_commands() {
        let mcp_client = Arc::new(NoOpMCPClient::new());
        let library = loaded_skill_library();
        let registry = CommandRegistry::with_skills(mcp_client, library);

        let commands = registry.get_available_commands().await.unwrap();

        // Builtin skills should appear as commands
        assert!(
            commands.iter().any(|c| c.name == "/plan"),
            "Skill 'plan' should appear as /plan command"
        );
        assert!(
            commands.iter().any(|c| c.name == "/commit"),
            "Skill 'commit' should appear as /commit command"
        );
        assert!(
            commands.iter().any(|c| c.name == "/test"),
            "Skill 'test' should appear as /test command"
        );
        assert!(
            commands.iter().any(|c| c.name == "/kanban"),
            "Skill 'kanban' should appear as /kanban command"
        );
        assert!(
            commands.iter().any(|c| c.name == "/implement"),
            "Skill 'implement' should appear as /implement command"
        );
    }

    #[tokio::test]
    async fn test_skill_commands_have_descriptions() {
        let mcp_client = Arc::new(NoOpMCPClient::new());
        let library = loaded_skill_library();
        let registry = CommandRegistry::with_skills(mcp_client, library);

        let commands = registry.get_available_commands().await.unwrap();

        let plan_cmd = commands.iter().find(|c| c.name == "/plan").unwrap();
        assert!(
            !plan_cmd.description.is_empty(),
            "Skill commands should have non-empty descriptions"
        );
    }

    #[tokio::test]
    async fn test_skill_commands_have_input_spec() {
        let mcp_client = Arc::new(NoOpMCPClient::new());
        let library = loaded_skill_library();
        let registry = CommandRegistry::with_skills(mcp_client, library);

        let commands = registry.get_available_commands().await.unwrap();

        let plan_cmd = commands.iter().find(|c| c.name == "/plan").unwrap();
        assert!(
            plan_cmd.input.is_some(),
            "Skill commands should have input specification"
        );

        // Verify input hint
        if let Some(AvailableCommandInput::Unstructured(input)) = &plan_cmd.input {
            assert_eq!(input.hint, "[input]");
        } else {
            panic!("Expected unstructured input");
        }
    }

    #[tokio::test]
    async fn test_skill_commands_have_source_meta() {
        let mcp_client = Arc::new(NoOpMCPClient::new());
        let library = loaded_skill_library();
        let registry = CommandRegistry::with_skills(mcp_client, library);

        let commands = registry.get_available_commands().await.unwrap();

        let plan_cmd = commands.iter().find(|c| c.name == "/plan").unwrap();
        let meta = plan_cmd.meta.as_ref().expect("Should have meta");
        assert_eq!(
            meta.get("source").and_then(|v| v.as_str()),
            Some("skill"),
            "Skill commands should have source=skill in meta"
        );
    }

    #[tokio::test]
    async fn test_skills_override_duplicate_mcp_prompts() {
        // MCP prompt with same name as a skill should be skipped
        let prompts = vec![Prompt {
            name: "plan".to_string(),
            title: None,
            description: Some("MCP plan prompt".to_string()),
            arguments: None,
            icons: None,
            meta: None,
        }];

        let mcp_client = Arc::new(MockMCPClient::new(prompts));
        let library = loaded_skill_library();
        let registry = CommandRegistry::with_skills(mcp_client, library);

        let commands = registry.get_available_commands().await.unwrap();

        // Should only have one /plan command (from skills, not MCP)
        let plan_commands: Vec<_> = commands.iter().filter(|c| c.name == "/plan").collect();
        assert_eq!(
            plan_commands.len(),
            1,
            "Should have exactly one /plan command"
        );

        // The command should be from skills (has source=skill meta)
        let meta = plan_commands[0].meta.as_ref().expect("Should have meta");
        assert_eq!(
            meta.get("source").and_then(|v| v.as_str()),
            Some("skill"),
            "When skill and MCP prompt have same name, skill should win"
        );
    }

    #[tokio::test]
    async fn test_skills_and_mcp_commands_coexist() {
        // MCP prompt with a unique name should still appear
        let prompts = vec![Prompt {
            name: "deploy".to_string(),
            title: None,
            description: Some("Deploy to production".to_string()),
            arguments: None,
            icons: None,
            meta: None,
        }];

        let mcp_client = Arc::new(MockMCPClient::new(prompts));
        let library = loaded_skill_library();
        let registry = CommandRegistry::with_skills(mcp_client, library);

        let commands = registry.get_available_commands().await.unwrap();

        // Skills should be present
        assert!(commands.iter().any(|c| c.name == "/plan"));
        assert!(commands.iter().any(|c| c.name == "/commit"));

        // MCP-only commands should also be present
        assert!(
            commands.iter().any(|c| c.name == "/deploy"),
            "Non-overlapping MCP commands should coexist with skills"
        );
    }

    #[tokio::test]
    async fn test_without_skills_behaves_as_before() {
        // CommandRegistry::new (no skills) should behave identically to before
        let prompts = vec![Prompt {
            name: "deploy".to_string(),
            title: None,
            description: Some("Deploy".to_string()),
            arguments: None,
            icons: None,
            meta: None,
        }];

        let mcp_client = Arc::new(MockMCPClient::new(prompts));
        let registry = CommandRegistry::new(mcp_client);

        let commands = registry.get_available_commands().await.unwrap();

        // Should have /help + /deploy only (no skills)
        assert_eq!(commands.len(), 2);
        assert!(commands.iter().any(|c| c.name == "/help"));
        assert!(commands.iter().any(|c| c.name == "/deploy"));
    }
}
