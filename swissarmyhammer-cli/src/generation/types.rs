//! Type definitions for the CLI generation system

use std::fmt;
use serde::{Deserialize, Serialize};

/// Represents a generated CLI command
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneratedCommand {
    /// The command name (e.g., "memo-create", "issue", "create")
    pub name: String,
    
    /// Human-readable description for help text
    pub description: String,
    
    /// Required and optional command arguments
    pub arguments: Vec<CliArgument>,
    
    /// Command-line options/flags
    pub options: Vec<CliOption>,
    
    /// If this is a subcommand, the parent command name
    pub subcommand_of: Option<String>,
    
    /// The original MCP tool name this command was generated from
    pub tool_name: String,
}

/// CLI argument representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CliArgument {
    /// Argument name (e.g., "title", "content", "file-path")
    pub name: String,
    
    /// The JSON Schema type (e.g., "string", "integer", "boolean")
    pub arg_type: String,
    
    /// Help description for this argument
    pub description: String,
    
    /// Whether this argument is required
    pub required: bool,
    
    /// Default value if optional
    pub default_value: Option<String>,
    
    /// Validation constraints from JSON Schema (e.g., minLength, pattern)
    pub constraints: Vec<ArgumentConstraint>,
}

/// CLI option/flag representation  
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CliOption {
    /// Option name for long form (e.g., "verbose", "output-format")
    pub name: String,
    
    /// Single character short form (e.g., 'v', 'o')
    pub short: Option<char>,
    
    /// Long form with dashes (e.g., "--verbose", "--output-format")
    pub long: String,
    
    /// Help description for this option
    pub description: String,
    
    /// Default value if not specified
    pub default_value: Option<String>,
    
    /// Whether this option takes a value or is just a flag
    pub takes_value: bool,
    
    /// The expected value type if takes_value is true
    pub value_type: Option<String>,
}

/// JSON Schema validation constraints
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ArgumentConstraint {
    /// Minimum string length
    MinLength(usize),
    
    /// Maximum string length
    MaxLength(usize),
    
    /// Regular expression pattern
    Pattern(String),
    
    /// Minimum numeric value
    Minimum(f64),
    
    /// Maximum numeric value
    Maximum(f64),
    
    /// Enumerated valid values
    Enum(Vec<String>),
    
    /// Custom constraint description
    Custom(String),
}

/// Configuration for CLI generation behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Prefix for generated commands (e.g., "sah-")
    pub command_prefix: Option<String>,
    
    /// Whether to generate subcommands or top-level commands
    pub use_subcommands: bool,
    
    /// Strategy for transforming tool names to command names
    pub naming_strategy: NamingStrategy,
    
    /// Whether to include excluded tools (for debugging)
    pub include_excluded: bool,
    
    /// Maximum number of commands to generate (safety limit)
    pub max_commands: usize,
    
    /// Whether to generate shell completion support
    pub generate_completions: bool,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            command_prefix: None,
            use_subcommands: false,
            naming_strategy: NamingStrategy::KeepOriginal,
            include_excluded: false,
            max_commands: 1000,
            generate_completions: true,
        }
    }
}

/// Strategy for transforming MCP tool names into CLI command names
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NamingStrategy {
    /// Keep original tool names with underscores converted to dashes
    /// Example: issue_create -> issue-create
    KeepOriginal,
    
    /// Group commands by domain using subcommands
    /// Example: issue_create -> issue create
    GroupByDomain,
    
    /// Flatten all commands to top-level with action-first naming
    /// Example: issue_create -> create-issue
    Flatten,
    
    /// Custom transformation function (for advanced use cases)
    Custom(String), // Function name or identifier
}

/// Errors that can occur during CLI generation
#[derive(Debug, Clone)]
pub enum GenerationError {
    /// Tool not found in registry
    ToolNotFound(String),
    
    /// Schema parsing failed
    SchemaParse(ParseError),
    
    /// Command building failed
    CommandBuild(String),
    
    /// Configuration validation failed
    ConfigValidation(String),
    
    /// Too many commands would be generated
    TooManyCommands(usize, usize), // (limit, attempted)
    
    /// General generation error
    General(String),
}

impl fmt::Display for GenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenerationError::ToolNotFound(name) => {
                write!(f, "Tool not found in registry: {name}")
            }
            GenerationError::SchemaParse(err) => {
                write!(f, "Schema parsing failed: {err}")
            }
            GenerationError::CommandBuild(msg) => {
                write!(f, "Command building failed: {msg}")
            }
            GenerationError::ConfigValidation(msg) => {
                write!(f, "Configuration validation failed: {msg}")
            }
            GenerationError::TooManyCommands(limit, attempted) => {
                write!(f, "Too many commands: attempted {attempted}, limit {limit}")
            }
            GenerationError::General(msg) => {
                write!(f, "Generation error: {msg}")
            }
        }
    }
}

impl std::error::Error for GenerationError {}

/// Errors that can occur during schema parsing
#[derive(Debug, Clone)]
pub enum ParseError {
    /// Invalid JSON Schema structure
    InvalidSchema(String),
    
    /// Unsupported schema feature
    UnsupportedFeature(String),
    
    /// Missing required schema field
    MissingField(String),
    
    /// Type conversion error
    TypeConversion(String),
    
    /// Validation constraint parsing error
    ConstraintParse(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidSchema(msg) => {
                write!(f, "Invalid JSON Schema: {msg}")
            }
            ParseError::UnsupportedFeature(feature) => {
                write!(f, "Unsupported schema feature: {feature}")
            }
            ParseError::MissingField(field) => {
                write!(f, "Missing required schema field: {field}")
            }
            ParseError::TypeConversion(msg) => {
                write!(f, "Type conversion error: {msg}")
            }
            ParseError::ConstraintParse(msg) => {
                write!(f, "Constraint parsing error: {msg}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl From<ParseError> for GenerationError {
    fn from(err: ParseError) -> Self {
        GenerationError::SchemaParse(err)
    }
}

impl GeneratedCommand {
    /// Create a new generated command
    pub fn new(
        name: String,
        description: String,
        tool_name: String,
    ) -> Self {
        Self {
            name,
            description,
            arguments: Vec::new(),
            options: Vec::new(),
            subcommand_of: None,
            tool_name,
        }
    }
    
    /// Add an argument to this command
    pub fn with_argument(mut self, argument: CliArgument) -> Self {
        self.arguments.push(argument);
        self
    }
    
    /// Add an option to this command
    pub fn with_option(mut self, option: CliOption) -> Self {
        self.options.push(option);
        self
    }
    
    /// Set this as a subcommand of another command
    pub fn as_subcommand_of(mut self, parent: String) -> Self {
        self.subcommand_of = Some(parent);
        self
    }
    
    /// Get all required arguments
    pub fn required_arguments(&self) -> Vec<&CliArgument> {
        self.arguments.iter().filter(|arg| arg.required).collect()
    }
    
    /// Get all optional arguments
    pub fn optional_arguments(&self) -> Vec<&CliArgument> {
        self.arguments.iter().filter(|arg| !arg.required).collect()
    }
    
    /// Get options that take values
    pub fn value_options(&self) -> Vec<&CliOption> {
        self.options.iter().filter(|opt| opt.takes_value).collect()
    }
    
    /// Get flag-only options
    pub fn flag_options(&self) -> Vec<&CliOption> {
        self.options.iter().filter(|opt| !opt.takes_value).collect()
    }
}

impl CliArgument {
    /// Create a new CLI argument
    pub fn new(
        name: String,
        arg_type: String,
        description: String,
        required: bool,
    ) -> Self {
        Self {
            name,
            arg_type,
            description,
            required,
            default_value: None,
            constraints: Vec::new(),
        }
    }
    
    /// Add a default value to this argument
    pub fn with_default(mut self, default: String) -> Self {
        self.default_value = Some(default);
        self
    }
    
    /// Add a constraint to this argument
    pub fn with_constraint(mut self, constraint: ArgumentConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }
    
    /// Check if this argument has validation constraints
    pub fn has_constraints(&self) -> bool {
        !self.constraints.is_empty()
    }
}

impl CliOption {
    /// Create a new CLI option
    pub fn new(
        name: String,
        long: String,
        description: String,
        takes_value: bool,
    ) -> Self {
        Self {
            name,
            short: None,
            long,
            description,
            default_value: None,
            takes_value,
            value_type: None,
        }
    }
    
    /// Add a short form to this option
    pub fn with_short(mut self, short: char) -> Self {
        self.short = Some(short);
        self
    }
    
    /// Add a default value to this option
    pub fn with_default(mut self, default: String) -> Self {
        self.default_value = Some(default);
        self
    }
    
    /// Set the value type for this option
    pub fn with_value_type(mut self, value_type: String) -> Self {
        self.value_type = Some(value_type);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generated_command_creation() {
        let command = GeneratedCommand::new(
            "memo-create".to_string(),
            "Create a new memo".to_string(),
            "memo_create".to_string(),
        );

        assert_eq!(command.name, "memo-create");
        assert_eq!(command.description, "Create a new memo");
        assert_eq!(command.tool_name, "memo_create");
        assert!(command.arguments.is_empty());
        assert!(command.options.is_empty());
        assert!(command.subcommand_of.is_none());
    }

    #[test]
    fn test_command_builder_pattern() {
        let arg = CliArgument::new(
            "title".to_string(),
            "string".to_string(),
            "The memo title".to_string(),
            true,
        ).with_constraint(ArgumentConstraint::MinLength(1));

        let option = CliOption::new(
            "verbose".to_string(),
            "--verbose".to_string(),
            "Enable verbose output".to_string(),
            false,
        ).with_short('v');

        let command = GeneratedCommand::new(
            "memo-create".to_string(),
            "Create a new memo".to_string(),
            "memo_create".to_string(),
        )
        .with_argument(arg)
        .with_option(option)
        .as_subcommand_of("memo".to_string());

        assert_eq!(command.arguments.len(), 1);
        assert_eq!(command.options.len(), 1);
        assert_eq!(command.subcommand_of, Some("memo".to_string()));
        
        // Test filtering methods
        assert_eq!(command.required_arguments().len(), 1);
        assert_eq!(command.optional_arguments().len(), 0);
        assert_eq!(command.flag_options().len(), 1);
        assert_eq!(command.value_options().len(), 0);
    }

    #[test]
    fn test_generation_config_defaults() {
        let config = GenerationConfig::default();
        
        assert!(config.command_prefix.is_none());
        assert!(!config.use_subcommands);
        assert_eq!(config.naming_strategy, NamingStrategy::KeepOriginal);
        assert!(!config.include_excluded);
        assert_eq!(config.max_commands, 1000);
        assert!(config.generate_completions);
    }

    #[test]
    fn test_error_display() {
        let error = GenerationError::ToolNotFound("test_tool".to_string());
        assert!(error.to_string().contains("test_tool"));

        let parse_error = ParseError::InvalidSchema("missing type".to_string());
        assert!(parse_error.to_string().contains("missing type"));

        let gen_error: GenerationError = parse_error.into();
        assert!(matches!(gen_error, GenerationError::SchemaParse(_)));
    }

    #[test]
    fn test_argument_constraints() {
        let constraint = ArgumentConstraint::MinLength(5);
        let arg = CliArgument::new(
            "name".to_string(),
            "string".to_string(),
            "Name field".to_string(),
            true,
        ).with_constraint(constraint);

        assert!(arg.has_constraints());
        assert_eq!(arg.constraints.len(), 1);
        assert!(matches!(arg.constraints[0], ArgumentConstraint::MinLength(5)));
    }

    #[test]
    fn test_naming_strategy_equality() {
        assert_eq!(NamingStrategy::KeepOriginal, NamingStrategy::KeepOriginal);
        assert_ne!(NamingStrategy::KeepOriginal, NamingStrategy::GroupByDomain);
        
        let custom1 = NamingStrategy::Custom("func1".to_string());
        let custom2 = NamingStrategy::Custom("func1".to_string());
        let custom3 = NamingStrategy::Custom("func2".to_string());
        
        assert_eq!(custom1, custom2);
        assert_ne!(custom1, custom3);
    }
}