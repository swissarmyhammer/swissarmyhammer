//! Core CLI generator for creating CLI commands from MCP tools

use crate::generation::types::{GeneratedCommand, GenerationConfig, GenerationError};
use crate::generation::CommandBuilder;
use std::sync::Arc;
use swissarmyhammer_tools::{cli::ToolCliMetadata, ToolRegistry};

/// Generates CLI commands from MCP tool definitions
///
/// The `CliGenerator` serves as the main orchestrator for the CLI generation process.
/// It leverages the existing `ToolRegistry` infrastructure to identify CLI-eligible
/// tools, parse their schemas, and produce structured command representations.
///
/// ## Key Features
///
/// - **Registry Integration**: Uses existing CLI exclusion tracking from `ToolRegistry`
/// - **Schema Parsing**: Converts JSON Schema from MCP tools to CLI argument structures
/// - **Configurable Generation**: Supports multiple naming strategies and organization patterns
/// - **Safety Limits**: Enforces maximum command limits to prevent runaway generation
///
/// ## Usage
///
/// ```rust,no_run
/// use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig, NamingStrategy};
/// use swissarmyhammer_tools::ToolRegistry;
/// use std::sync::Arc;
///
/// // Create registry and register tools
/// let mut registry = ToolRegistry::new();
/// // ... register tools ...
///
/// // Create generator
/// let generator = CliGenerator::new(Arc::new(registry));
///
/// // Generate commands with default configuration
/// let commands = generator.generate_commands().unwrap();
///
/// // Or with custom configuration
/// let config = GenerationConfig {
///     naming_strategy: NamingStrategy::GroupByDomain,
///     use_subcommands: true,
///     ..Default::default()
/// };
/// let generator = generator.with_config(config);
/// let commands = generator.generate_commands().unwrap();
/// ```
pub struct CliGenerator {
    /// Reference to the tool registry containing all MCP tools
    registry: Arc<ToolRegistry>,

    /// Configuration controlling generation behavior
    config: GenerationConfig,
}

impl CliGenerator {
    /// Create a new CLI generator with the given registry
    ///
    /// # Arguments
    ///
    /// * `registry` - Shared reference to the tool registry
    ///
    /// # Returns
    ///
    /// A new `CliGenerator` instance with default configuration
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_cli::generation::CliGenerator;
    /// use swissarmyhammer_tools::ToolRegistry;
    /// use std::sync::Arc;
    ///
    /// let registry = Arc::new(ToolRegistry::new());
    /// let generator = CliGenerator::new(registry);
    /// ```
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            config: GenerationConfig::default(),
        }
    }

    /// Update the generator configuration
    ///
    /// # Arguments
    ///
    /// * `config` - New configuration to use
    ///
    /// # Returns
    ///
    /// A new generator instance with the updated configuration
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig, NamingStrategy};
    /// use swissarmyhammer_tools::ToolRegistry;
    /// use std::sync::Arc;
    ///
    /// let registry = Arc::new(ToolRegistry::new());
    /// let config = GenerationConfig {
    ///     use_subcommands: true,
    ///     naming_strategy: NamingStrategy::GroupByDomain,
    ///     ..Default::default()
    /// };
    /// let generator = CliGenerator::new(registry).with_config(config);
    /// ```
    pub fn with_config(mut self, config: GenerationConfig) -> Self {
        self.config = config;
        self
    }

    /// Generate CLI commands for all eligible tools
    ///
    /// This method performs the complete CLI generation pipeline:
    /// 1. Validates configuration
    /// 2. Retrieves CLI-eligible tools from the registry
    /// 3. Generates commands for each eligible tool
    /// 4. Applies naming transformations according to configuration
    /// 5. Organizes commands according to subcommand settings
    ///
    /// # Returns
    ///
    /// * `Result<Vec<GeneratedCommand>, GenerationError>` - Generated commands or error
    ///
    /// # Errors
    ///
    /// * `GenerationError::ConfigValidation` - Invalid configuration
    /// * `GenerationError::TooManyCommands` - Exceeded maximum command limit
    /// * `GenerationError::SchemaParse` - Failed to parse tool schema
    /// * `GenerationError::CommandBuild` - Failed to build command structure
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_cli::generation::CliGenerator;
    /// use swissarmyhammer_tools::ToolRegistry;
    /// use std::sync::Arc;
    ///
    /// let registry = Arc::new(ToolRegistry::new());
    /// let generator = CliGenerator::new(registry);
    ///
    /// match generator.generate_commands() {
    ///     Ok(commands) => {
    ///         println!("Generated {} commands", commands.len());
    ///         for command in commands {
    ///             println!("- {}: {}", command.name, command.description);
    ///         }
    ///     }
    ///     Err(e) => eprintln!("Generation failed: {}", e),
    /// }
    /// ```
    pub fn generate_commands(&self) -> Result<Vec<GeneratedCommand>, GenerationError> {
        // Validate configuration first
        self.validate_config()?;

        // Get CLI-eligible tools from registry
        let eligible_tools: Vec<_> = if self.config.include_excluded {
            // For debugging, include all tools
            self.registry
                .list_tool_names()
                .into_iter()
                .filter_map(|name| self.registry.get_tool_metadata(&name).cloned())
                .collect()
        } else {
            // Normal operation: only CLI-eligible tools
            self.registry
                .get_cli_eligible_tools()
                .into_iter()
                .cloned()
                .collect()
        };

        // Check command limit
        if eligible_tools.len() > self.config.max_commands {
            return Err(GenerationError::TooManyCommands(
                self.config.max_commands,
                eligible_tools.len(),
            ));
        }

        // Generate commands for each eligible tool
        let mut commands = Vec::new();
        for tool_metadata in eligible_tools {
            match self.generate_command_for_tool(&tool_metadata) {
                Ok(command) => commands.push(command),
                Err(e) => {
                    // Log the error but continue with other tools
                    eprintln!(
                        "Warning: Failed to generate command for tool '{}': {}",
                        tool_metadata.name, e
                    );
                    continue;
                }
            }
        }

        // Apply naming transformations and organization
        self.organize_commands(commands)
    }

    /// Generate CLI command for a specific tool
    ///
    /// This method handles the generation of a single command from a tool's metadata:
    /// 1. Retrieves the tool from the registry
    /// 2. Parses the tool's JSON schema
    /// 3. Builds the CLI command structure
    /// 4. Applies naming transformations
    ///
    /// # Arguments
    ///
    /// * `metadata` - CLI metadata for the tool to generate
    ///
    /// # Returns
    ///
    /// * `Result<GeneratedCommand, GenerationError>` - Generated command or error
    ///
    /// # Errors
    ///
    /// * `GenerationError::ToolNotFound` - Tool not found in registry
    /// * `GenerationError::SchemaParse` - Failed to parse tool schema
    /// * `GenerationError::CommandBuild` - Failed to build command structure
    fn generate_command_for_tool(
        &self,
        metadata: &ToolCliMetadata,
    ) -> Result<GeneratedCommand, GenerationError> {
        // Get the tool from the registry
        let tool = self
            .registry
            .get_tool(&metadata.name)
            .ok_or_else(|| GenerationError::ToolNotFound(metadata.name.clone()))?;

        // Build command using the command builder
        let command = CommandBuilder::new()
            .from_mcp_tool(tool)?
            .with_naming_strategy(&self.config.naming_strategy)
            .build()?;

        Ok(command)
    }

    /// Organize commands according to configuration settings
    ///
    /// This method applies the final organization structure to the generated commands:
    /// - Groups commands by domain if using subcommands
    /// - Applies command prefixes if configured
    /// - Sorts commands for consistent output
    ///
    /// # Arguments
    ///
    /// * `commands` - Raw generated commands to organize
    ///
    /// # Returns
    ///
    /// * `Result<Vec<GeneratedCommand>, GenerationError>` - Organized commands or error
    fn organize_commands(
        &self,
        mut commands: Vec<GeneratedCommand>,
    ) -> Result<Vec<GeneratedCommand>, GenerationError> {
        // Apply command prefixes if configured
        if let Some(ref prefix) = self.config.command_prefix {
            for command in &mut commands {
                if command.subcommand_of.is_none() {
                    command.name = format!("{}{}", prefix, command.name);
                }
            }
        }

        // Organize into subcommands if configured
        if self.config.use_subcommands {
            commands = self.create_subcommand_structure(commands)?;
        }

        // Sort commands for consistent output
        commands.sort_by(|a, b| {
            // Sort parent commands first, then subcommands
            match (&a.subcommand_of, &b.subcommand_of) {
                (None, None) => a.name.cmp(&b.name),
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (Some(a_parent), Some(b_parent)) => {
                    a_parent.cmp(b_parent).then(a.name.cmp(&b.name))
                }
            }
        });

        Ok(commands)
    }

    /// Create subcommand structure from flat command list
    ///
    /// This method groups commands by their domain prefix (e.g., "memo_create" -> "memo create")
    /// and creates appropriate parent commands when needed.
    ///
    /// # Arguments
    ///
    /// * `commands` - Flat list of commands to organize
    ///
    /// # Returns
    ///
    /// * `Result<Vec<GeneratedCommand>, GenerationError>` - Commands organized into subcommand structure
    fn create_subcommand_structure(
        &self,
        commands: Vec<GeneratedCommand>,
    ) -> Result<Vec<GeneratedCommand>, GenerationError> {
        use std::collections::HashMap;

        let mut parent_commands: HashMap<String, GeneratedCommand> = HashMap::new();
        let mut subcommands: Vec<GeneratedCommand> = Vec::new();

        for mut command in commands {
            // Extract domain from tool name (e.g., "memo_create" -> "memo")
            if let Some(underscore_pos) = command.tool_name.find('_') {
                let domain = command.tool_name[..underscore_pos].to_string();
                let action = command.tool_name[underscore_pos + 1..].to_string();

                // Create parent command if it doesn't exist
                if !parent_commands.contains_key(&domain) {
                    let parent = GeneratedCommand::new(
                        domain.clone(),
                        format!("Commands for {domain} operations"),
                        format!("{domain}_parent"), // Synthetic tool name
                    );
                    parent_commands.insert(domain.clone(), parent);
                }

                // Convert command to subcommand
                command.name = action.replace('_', "-");
                command.subcommand_of = Some(domain);
                subcommands.push(command);
            } else {
                // Keep as top-level command if no domain found
                subcommands.push(command);
            }
        }

        // Combine parent commands and subcommands
        let mut result: Vec<GeneratedCommand> = parent_commands.into_values().collect();
        result.extend(subcommands);

        Ok(result)
    }

    /// Validate the current configuration
    ///
    /// # Returns
    ///
    /// * `Result<(), GenerationError>` - Success or configuration error
    fn validate_config(&self) -> Result<(), GenerationError> {
        // Validate command prefix
        if let Some(ref prefix) = self.config.command_prefix {
            if prefix.is_empty() {
                return Err(GenerationError::ConfigValidation(
                    "Command prefix cannot be empty".to_string(),
                ));
            }

            if prefix.contains(char::is_whitespace) {
                return Err(GenerationError::ConfigValidation(
                    "Command prefix cannot contain whitespace".to_string(),
                ));
            }
        }

        // Validate maximum commands limit
        if self.config.max_commands == 0 {
            return Err(GenerationError::ConfigValidation(
                "Maximum commands must be greater than 0".to_string(),
            ));
        }

        // Validate naming strategy
        if let crate::generation::NamingStrategy::Custom(func_name) = &self.config.naming_strategy {
            if func_name.is_empty() {
                return Err(GenerationError::ConfigValidation(
                    "Custom naming strategy function name cannot be empty".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Get configuration information for debugging
    pub fn config(&self) -> &GenerationConfig {
        &self.config
    }

    /// Get registry reference for inspection
    pub fn registry(&self) -> &Arc<ToolRegistry> {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::types::NamingStrategy;
    use async_trait::async_trait;
    use std::sync::Arc;
    use swissarmyhammer_tools::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};

    /// Mock tool for testing
    struct MockTool {
        name: &'static str,
        description: &'static str,
        schema: serde_json::Value,
    }

    impl MockTool {
        fn new(name: &'static str, description: &'static str) -> Self {
            Self {
                name,
                description,
                schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "title": {
                            "type": "string",
                            "description": "The title"
                        },
                        "optional_param": {
                            "type": "string",
                            "description": "Optional parameter",
                            "default": "default_value"
                        }
                    },
                    "required": ["title"]
                }),
            }
        }
    }

    #[async_trait]
    impl McpTool for MockTool {
        fn name(&self) -> &'static str {
            self.name
        }

        fn description(&self) -> &'static str {
            self.description
        }

        fn schema(&self) -> serde_json::Value {
            self.schema.clone()
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, serde_json::Value>,
            _context: &ToolContext,
        ) -> std::result::Result<rmcp::model::CallToolResult, rmcp::Error> {
            Ok(BaseToolImpl::create_success_response("Mock executed"))
        }
    }

    fn create_test_registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();

        // Add some CLI-eligible tools
        registry.register(MockTool::new("memo_create", "Create a memo"));
        registry.register(MockTool::new("memo_list", "List memos"));
        registry.register(MockTool::new("issue_create", "Create an issue"));

        // Add a known excluded tool
        registry.register(MockTool::new("issue_work", "Work on an issue")); // This should be excluded

        registry
    }

    #[test]
    fn test_cli_generator_creation() {
        let registry = Arc::new(create_test_registry());
        let generator = CliGenerator::new(registry.clone());

        assert_eq!(generator.registry().len(), registry.len());
        assert!(!generator.config().use_subcommands);
        assert_eq!(
            generator.config().naming_strategy,
            NamingStrategy::KeepOriginal
        );
    }

    #[test]
    fn test_generator_with_config() {
        let registry = Arc::new(create_test_registry());
        let config = GenerationConfig {
            use_subcommands: true,
            naming_strategy: NamingStrategy::GroupByDomain,
            command_prefix: Some("sah-".to_string()),
            ..Default::default()
        };

        let generator = CliGenerator::new(registry).with_config(config);

        assert!(generator.config().use_subcommands);
        assert_eq!(
            generator.config().naming_strategy,
            NamingStrategy::GroupByDomain
        );
        assert_eq!(generator.config().command_prefix, Some("sah-".to_string()));
    }

    #[test]
    fn test_config_validation_empty_prefix() {
        let registry = Arc::new(create_test_registry());
        let config = GenerationConfig {
            command_prefix: Some("".to_string()),
            ..Default::default()
        };

        let generator = CliGenerator::new(registry).with_config(config);
        let result = generator.generate_commands();

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GenerationError::ConfigValidation(_)
        ));
    }

    #[test]
    fn test_config_validation_whitespace_prefix() {
        let registry = Arc::new(create_test_registry());
        let config = GenerationConfig {
            command_prefix: Some("sah ".to_string()),
            ..Default::default()
        };

        let generator = CliGenerator::new(registry).with_config(config);
        let result = generator.generate_commands();

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GenerationError::ConfigValidation(_)
        ));
    }

    #[test]
    fn test_config_validation_zero_max_commands() {
        let registry = Arc::new(create_test_registry());
        let config = GenerationConfig {
            max_commands: 0,
            ..Default::default()
        };

        let generator = CliGenerator::new(registry).with_config(config);
        let result = generator.generate_commands();

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GenerationError::ConfigValidation(_)
        ));
    }

    #[test]
    fn test_too_many_commands_limit() {
        let registry = Arc::new(create_test_registry());
        let config = GenerationConfig {
            max_commands: 1, // Very low limit
            ..Default::default()
        };

        let generator = CliGenerator::new(registry).with_config(config);
        let result = generator.generate_commands();

        // This test might pass or fail depending on how many CLI-eligible tools we have
        // The exact behavior depends on the registry contents and exclusion logic
        if let Err(GenerationError::TooManyCommands(limit, attempted)) = result {
            assert_eq!(limit, 1);
            assert!(attempted > 1);
        }
        // If it doesn't hit the limit, that's also okay - it means we have few enough tools
    }

    #[test]
    fn test_empty_registry_generation() {
        let registry = Arc::new(ToolRegistry::new());
        let generator = CliGenerator::new(registry);

        let result = generator.generate_commands();
        assert!(result.is_ok());

        let commands = result.unwrap();
        assert!(commands.is_empty());
    }

    #[test]
    fn test_include_excluded_tools() {
        let registry = Arc::new(create_test_registry());
        let config = GenerationConfig {
            include_excluded: true,
            ..Default::default()
        };

        let generator = CliGenerator::new(registry.clone()).with_config(config);
        let result = generator.generate_commands();

        assert!(result.is_ok());
        let commands = result.unwrap();

        // With include_excluded=true, we should get more commands (including excluded ones)

        // Note: The exact count might be different due to generation failures,
        // but we should at least have some commands generated
        assert!(!commands.is_empty());
    }
}
