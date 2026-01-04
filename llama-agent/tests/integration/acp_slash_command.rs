//! Integration test for slash command advertisement in ACP
//!
//! This test verifies that:
//! 1. The ACP server advertises supports_slash_commands capability during initialization
//! 2. Slash commands are derived from MCP prompts via CommandRegistry
//! 3. Commands include proper metadata for parameters when available

mod acp_slash_command_tests {
    use async_trait::async_trait;
    use llama_agent::acp::commands::CommandRegistry;
    use llama_agent::mcp::MCPClient;
    use llama_agent::types::errors::MCPError;
    use rmcp::model::{Prompt, PromptArgument};
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// Mock MCP client that returns test prompts for slash command testing
    struct MockMCPClientWithPrompts {
        prompts: Vec<Prompt>,
    }

    impl MockMCPClientWithPrompts {
        fn new(prompts: Vec<Prompt>) -> Self {
            Self { prompts }
        }
    }

    #[async_trait]
    impl MCPClient for MockMCPClientWithPrompts {
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

    /// Test that the ACP config advertises supports_slash_commands capability
    #[test]
    fn test_acp_config_advertises_slash_commands_capability() {
        // Initialize tracing for test visibility
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        // Verify the default config has supports_slash_commands enabled
        let default_config = llama_agent::acp::AcpConfig::default();
        assert!(
            default_config.capabilities.supports_slash_commands,
            "Default ACP config should support slash commands"
        );

        // Verify the capability is serialized correctly for the protocol
        let json = serde_json::to_string(&default_config.capabilities).unwrap();
        assert!(
            json.contains("\"supports_slash_commands\":true")
                || json.contains("\"supportsSlashCommands\":true"),
            "Slash commands capability should be present in JSON: {}",
            json
        );

        tracing::info!("✓ ACP config advertises slash commands support");
    }

    /// Test that CommandRegistry correctly converts MCP prompts to slash commands
    #[tokio::test]
    async fn test_command_registry_converts_prompts_to_slash_commands() {
        // Initialize tracing for test visibility
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        // Create mock prompts with various configurations
        let prompts = vec![
            // Simple prompt without arguments
            Prompt {
                name: "test".to_string(),
                title: None,
                description: Some("Run tests".to_string()),
                arguments: None,
                icons: None,
                meta: None,
            },
            // Prompt with required arguments
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
            // Prompt with optional arguments
            Prompt {
                name: "review".to_string(),
                title: None,
                description: Some("Review code".to_string()),
                arguments: Some(vec![
                    PromptArgument {
                        name: "files".to_string(),
                        title: None,
                        description: Some("Files to review".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "severity".to_string(),
                        title: None,
                        description: Some("Minimum severity level".to_string()),
                        required: Some(false),
                    },
                ]),
                icons: None,
                meta: None,
            },
            // Prompt without description
            Prompt {
                name: "commit".to_string(),
                title: None,
                description: None,
                arguments: None,
                icons: None,
                meta: None,
            },
        ];

        let mcp_client = Arc::new(MockMCPClientWithPrompts::new(prompts));
        let registry = CommandRegistry::new(mcp_client);

        let commands = registry
            .get_available_commands()
            .await
            .expect("Failed to get commands");

        // Verify core command is present
        assert!(
            commands.iter().any(|c| c.name == "/help"),
            "Core /help command should be present"
        );

        // Verify MCP-based commands are present with correct names
        assert!(
            commands.iter().any(|c| c.name == "/test"),
            "MCP prompt 'test' should be converted to /test command"
        );
        assert!(
            commands.iter().any(|c| c.name == "/plan"),
            "MCP prompt 'plan' should be converted to /plan command"
        );
        assert!(
            commands.iter().any(|c| c.name == "/review"),
            "MCP prompt 'review' should be converted to /review command"
        );
        assert!(
            commands.iter().any(|c| c.name == "/commit"),
            "MCP prompt 'commit' should be converted to /commit command"
        );

        // Verify command descriptions
        let test_cmd = commands.iter().find(|c| c.name == "/test").unwrap();
        assert_eq!(test_cmd.description, "Run tests");

        let commit_cmd = commands.iter().find(|c| c.name == "/commit").unwrap();
        assert_eq!(
            commit_cmd.description, "Execute commit prompt",
            "Commands without description should have fallback description"
        );

        // Verify command with required arguments has input specification
        let plan_cmd = commands.iter().find(|c| c.name == "/plan").unwrap();
        assert!(
            plan_cmd.input.is_some(),
            "Commands with arguments should have input specification"
        );

        // Verify parameter metadata is included
        if let Some(input) = &plan_cmd.input {
            match input {
                agent_client_protocol::AvailableCommandInput::Unstructured(unstructured) => {
                    assert!(
                        unstructured.hint.contains("<spec>"),
                        "Required arguments should be shown in angle brackets"
                    );

                    // Verify meta contains parameter schema
                    assert!(
                        unstructured.meta.is_some(),
                        "Input should include parameter metadata"
                    );

                    if let Some(meta) = &unstructured.meta {
                        let parameters = meta
                            .get("parameters")
                            .expect("Meta should contain parameters array");
                        assert!(parameters.is_array(), "Parameters should be an array");

                        let params_array = parameters.as_array().unwrap();
                        assert_eq!(params_array.len(), 1, "Should have one parameter");

                        let param = &params_array[0];
                        assert_eq!(param.get("name").and_then(|v| v.as_str()), Some("spec"));
                        assert_eq!(param.get("required").and_then(|v| v.as_bool()), Some(true));
                    }
                }
                _ => panic!("Expected unstructured input for prompt with arguments"),
            }
        }

        // Verify command with mixed required/optional arguments
        let review_cmd = commands.iter().find(|c| c.name == "/review").unwrap();
        if let Some(agent_client_protocol::AvailableCommandInput::Unstructured(unstructured)) =
            &review_cmd.input
        {
            assert!(
                unstructured.hint.contains("<files>"),
                "Required arguments should be in angle brackets"
            );
            assert!(
                unstructured.hint.contains("[severity]"),
                "Optional arguments should be in square brackets"
            );

            // Verify parameter metadata includes both parameters
            if let Some(meta) = &unstructured.meta {
                let parameters = meta
                    .get("parameters")
                    .expect("Meta should contain parameters");
                let params_array = parameters.as_array().unwrap();
                assert_eq!(params_array.len(), 2, "Should have two parameters");
            }
        }

        // Verify command metadata includes parameters at command level
        assert!(
            plan_cmd.meta.is_some(),
            "Command should have meta field with parameters"
        );
        if let Some(meta) = &plan_cmd.meta {
            assert!(
                meta.contains_key("parameters"),
                "Command meta should contain parameters for structured handling"
            );
        }

        tracing::info!(
            "✓ CommandRegistry correctly converts {} MCP prompts to slash commands",
            commands.len() - 1
        ); // -1 for /help
    }

    /// Test that has_commands_changed correctly detects changes
    #[tokio::test]
    async fn test_command_change_detection() {
        // Initialize tracing for test visibility
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        use agent_client_protocol::AvailableCommand;
        use llama_agent::mcp::NoOpMCPClient;

        let mcp_client = Arc::new(NoOpMCPClient::new());
        let registry = CommandRegistry::new(mcp_client);

        let commands1 = vec![
            AvailableCommand::new("/help", "Help"),
            AvailableCommand::new("/test", "Test"),
        ];

        let commands2 = vec![
            AvailableCommand::new("/help", "Help"),
            AvailableCommand::new("/test", "Test"),
            AvailableCommand::new("/plan", "Plan"),
        ];

        let commands3 = vec![
            AvailableCommand::new("/help", "Help"),
            AvailableCommand::new("/plan", "Plan"), // Different command in same position
        ];

        // Different lengths should be detected
        assert!(
            registry.has_commands_changed(&commands1, &commands2),
            "Adding a command should be detected"
        );

        // Same commands should not be detected as changed
        assert!(
            !registry.has_commands_changed(&commands1, &commands1),
            "Identical command lists should not be detected as changed"
        );

        // Different command names should be detected
        assert!(
            registry.has_commands_changed(&commands1, &commands3),
            "Changed command names should be detected"
        );

        tracing::info!("✓ Command change detection works correctly");
    }

    /// Test that commands with no arguments don't have input specifications
    #[tokio::test]
    async fn test_commands_without_arguments_have_no_input() {
        // Initialize tracing for test visibility
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        let prompts = vec![Prompt {
            name: "simple".to_string(),
            title: None,
            description: Some("Simple command".to_string()),
            arguments: None,
            icons: None,
            meta: None,
        }];

        let mcp_client = Arc::new(MockMCPClientWithPrompts::new(prompts));
        let registry = CommandRegistry::new(mcp_client);

        let commands = registry.get_available_commands().await.unwrap();

        let simple_cmd = commands
            .iter()
            .find(|c| c.name == "/simple")
            .expect("Should find /simple command");

        assert!(
            simple_cmd.input.is_none(),
            "Commands without arguments should not have input specification"
        );
        assert_eq!(simple_cmd.description, "Simple command");

        tracing::info!("✓ Commands without arguments correctly have no input specification");
    }

    /// Test that empty MCP prompt list still returns core commands
    #[tokio::test]
    async fn test_empty_mcp_prompts_returns_core_commands() {
        // Initialize tracing for test visibility
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        let prompts = vec![];

        let mcp_client = Arc::new(MockMCPClientWithPrompts::new(prompts));
        let registry = CommandRegistry::new(mcp_client);

        let commands = registry.get_available_commands().await.unwrap();

        // Should have at least the core /help command
        assert!(!commands.is_empty(), "Should have at least core commands");
        assert!(
            commands.iter().any(|c| c.name == "/help"),
            "Should always have /help command"
        );
        assert_eq!(
            commands.len(),
            1,
            "With no MCP prompts, should only have core commands"
        );

        tracing::info!("✓ Core commands are always present even without MCP prompts");
    }
}
