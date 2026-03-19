use crate::commands;
use clap::{Parser, Subcommand, ValueEnum};
use std::str::FromStr;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Default)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Yaml,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(OutputFormat::Json),
            "yaml" => Ok(OutputFormat::Yaml),
            "table" => Ok(OutputFormat::Table),
            _ => Ok(OutputFormat::Table), // Default to Table for unknown formats
        }
    }
}

// Re-export PromptSource from the library
pub use swissarmyhammer::PromptSource;

// Create a wrapper for CLI argument parsing since the library's PromptSource doesn't derive ValueEnum
#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum PromptSourceArg {
    Builtin,
    User,
    Local,
    Dynamic,
}

impl From<PromptSourceArg> for PromptSource {
    fn from(arg: PromptSourceArg) -> Self {
        match arg {
            PromptSourceArg::Builtin => PromptSource::Builtin,
            PromptSourceArg::User => PromptSource::User,
            PromptSourceArg::Local => PromptSource::Local,
            PromptSourceArg::Dynamic => PromptSource::Dynamic,
        }
    }
}

impl From<PromptSource> for PromptSourceArg {
    fn from(source: PromptSource) -> Self {
        match source {
            PromptSource::Builtin => PromptSourceArg::Builtin,
            PromptSource::User => PromptSourceArg::User,
            PromptSource::Local => PromptSourceArg::Local,
            PromptSource::Dynamic => PromptSourceArg::Dynamic,
        }
    }
}

/// Target location for init/deinit operations.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum InstallTarget {
    /// Project-level settings (.claude/settings.json)
    Project,
    /// Local project settings, not committed (.claude/settings.local.json)
    Local,
    /// User-level settings (~/.claude/settings.json)
    User,
}

impl From<InstallTarget> for swissarmyhammer_common::lifecycle::InitScope {
    fn from(target: InstallTarget) -> Self {
        match target {
            InstallTarget::Project => Self::Project,
            InstallTarget::Local => Self::Local,
            InstallTarget::User => Self::User,
        }
    }
}

impl std::fmt::Display for InstallTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallTarget::Project => write!(f, "project"),
            InstallTarget::Local => write!(f, "local"),
            InstallTarget::User => write!(f, "user"),
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "swissarmyhammer")]
#[command(version)]
#[command(about = "An MCP server for managing prompts as markdown files")]
#[command(long_about = "
swissarmyhammer is an MCP (Model Context Protocol) server that manages
prompts as markdown files. It supports file watching, template substitution,
and seamless integration with Claude Code.

Global arguments can be used with any command to control output and behavior:
  --verbose     Show detailed information and debug output
  --format      Set output format (table, json, yaml) for commands that support it  
  --debug       Enable debug mode with comprehensive tracing
  --quiet       Suppress all output except errors
  --model       Override model for all use cases (runtime only, doesn't modify config)

Main commands:
  serve         Run as MCP server (default when invoked via stdio)
  doctor        Diagnose configuration and setup issues
  prompt        Manage and test prompts with interactive capabilities
  agent         Manage and interact with specialized agents for specific use cases
  validate      Validate prompt files for syntax and best practices
  completion    Generate shell completion scripts

Example usage:
  swissarmyhammer serve                           # Run as MCP server
  swissarmyhammer doctor                          # Check configuration
  swissarmyhammer --verbose prompt list          # List prompts with details
  swissarmyhammer --format=json prompt list      # List prompts as JSON
  swissarmyhammer --debug prompt test help       # Test prompt with debug info
  swissarmyhammer agent list                     # List available agents
  swissarmyhammer agent use claude-code          # Apply Claude Code agent to project
")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Enable debug logging
    #[arg(short, long)]
    pub debug: bool,

    /// Suppress all output except errors
    #[arg(short, long)]
    pub quiet: bool,

    /// Global output format
    #[arg(long, value_enum)]
    pub format: Option<OutputFormat>,

    /// Override model for all use cases (runtime only, doesn't modify config)
    #[arg(long, global = true)]
    pub model: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run as MCP server (default when invoked via stdio)
    #[command(long_about = "
Run as MCP server. This is the default mode when
invoked via stdio (e.g., by Claude Code). The server will:

- Load all prompts from builtin, user, and local directories
- Watch for file changes and reload prompts automatically  
- Expose prompts via the MCP protocol
- Support template substitution with {{variables}}

Example:
  swissarmyhammer serve        # Stdio mode (default)
  swissarmyhammer serve http   # HTTP mode
  # Or configure in Claude Code's MCP settings
")]
    Serve {
        #[command(subcommand)]
        subcommand: Option<ServeSubcommand>,
    },
    /// Set up sah for all detected AI coding agents (skills + MCP)
    #[command(long_about = "
Set up SwissArmyHammer for all detected AI coding agents.

This command:
1. Registers sah as an MCP server for all detected agents (Claude Code, Cursor, Windsurf, etc.)
2. Creates the .sah/ project directory and .prompts/
3. Installs builtin skills to the central .skills/ store with symlinks to each agent

The command is idempotent - safe to run multiple times.

Targets:
  project   Write to project-level config files (default, shared with team via git)
  local     Write to ~/.claude.json per-project config (personal, not committed)
  user      Write to global config files (all projects)

Examples:
  sah init              # Project-level setup (default)
  sah init user         # Global setup for all projects
  sah init local        # Personal setup, not committed to git
")]
    Init {
        /// Where to install the MCP server configuration
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Remove sah from all detected AI coding agents (skills + MCP)
    #[command(long_about = "
Remove SwissArmyHammer from all detected AI coding agents.

By default, only the MCP server entries are removed from agent config files.
Use --remove-directory to also delete .sah/, .prompts/, and installed skills.

Examples:
  sah deinit                     # Remove from project settings
  sah deinit user                # Remove from user settings
  sah deinit --remove-directory  # Also remove .sah/ and skills
")]
    Deinit {
        /// Where to remove the MCP server configuration from
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
        /// Also remove .sah/ project directory
        #[arg(long)]
        remove_directory: bool,
    },
    /// Diagnose configuration and setup issues
    #[command(long_about = commands::doctor::DESCRIPTION)]
    Doctor {},
    /// Manage and test prompts
    #[command(long_about = "
Manage and test prompts with a clean, simplified interface.

The prompt system provides two main commands:
• list - Display all available prompts from all sources  
• test - Test prompts interactively with sample data

Use global arguments to control output:
  --verbose         Show detailed information
  --format FORMAT   Output format: table, json, yaml
  --debug           Enable debug mode
  --quiet           Suppress output except errors

Examples:
  sah prompt list                           # List all prompts
  sah --verbose prompt list                 # Show detailed information
  sah --format=json prompt list             # Output as JSON
  sah prompt test code-review               # Interactive testing
  sah prompt test help --var topic=git      # Test with parameters  
  sah --debug prompt test plan              # Test with debug output
")]
    #[command(trailing_var_arg = true)]
    Prompt {
        /// Subcommand and arguments for prompt (handled dynamically)
        args: Vec<String>,
    },
    /// Generate shell completion scripts
    #[command(long_about = "
Generates shell completion scripts for various shells. Supports:
- bash
- zsh
- fish
- powershell

Examples:
  # Bash (add to ~/.bashrc or ~/.bash_profile)
  swissarmyhammer completion bash > ~/.local/share/bash-completion/completions/swissarmyhammer
  
  # Zsh (add to ~/.zshrc or a file in fpath)
  swissarmyhammer completion zsh > ~/.zfunc/_swissarmyhammer
  
  # Fish
  swissarmyhammer completion fish > ~/.config/fish/completions/swissarmyhammer.fish
  
  # PowerShell
  swissarmyhammer completion powershell >> $PROFILE
")]
    Completion {
        /// Shell to generate completion for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Validate prompt files for syntax and best practices
    #[command(long_about = commands::validate::DESCRIPTION)]
    Validate {
        /// Suppress all output except errors. In quiet mode, warnings are hidden from both output and summary.
        #[arg(short, long)]
        quiet: bool,

        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,

        /// Validate MCP tool schemas for CLI compatibility
        #[arg(long)]
        validate_tools: bool,
    },

    /// Manage and interact with models
    #[command(long_about = commands::model::DESCRIPTION)]
    Model {
        #[command(subcommand)]
        subcommand: Option<ModelSubcommand>,
    },

    /// Manage and interact with Agent Client Protocol server
    #[command(long_about = commands::agent::DESCRIPTION)]
    Agent {
        #[command(subcommand)]
        subcommand: Option<AgentSubcommand>,
    },

    /// Manage tool enable/disable state
    #[command(long_about = "
Manage which MCP tools are enabled or disabled.

Tools are enabled by default. Disable tools you don't need to reduce
the tool surface visible to AI agents.

Examples:
  sah tools                          # List all tools with status
  sah tools disable                  # Disable all tools
  sah tools enable shell git         # Enable specific tools
  sah tools disable kanban web       # Disable specific tools
  sah tools enable                   # Enable all tools
  sah tools --global disable web     # Disable web globally
")]
    Tools {
        /// Write to global config (~/.sah/tools.yaml) instead of project
        #[arg(long)]
        global: bool,

        #[command(subcommand)]
        subcommand: Option<ToolsSubcommand>,
    },

    /// Render statusline from Claude Code JSON (stdin) or dump config
    #[command(long_about = "
Render a styled statusline for Claude Code integration.

In normal mode, reads JSON from stdin and outputs styled ANSI text.
Use 'sah statusline config' to dump the full annotated builtin config.

The statusline is configured via YAML with 3-layer stacking:
  1. Builtin defaults (embedded in binary)
  2. User config (~/.sah/statusline/config.yaml)
  3. Project config (.sah/statusline/config.yaml)

Examples:
  echo '{\"model\":{\"display_name\":\"Opus\"}}' | sah statusline
  sah statusline config > .sah/statusline/config.yaml
")]
    Statusline {
        #[command(subcommand)]
        subcommand: Option<StatuslineSubcommand>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ServeSubcommand {
    /// Start HTTP MCP server
    #[command(long_about = "
Start HTTP MCP server for web clients, debugging, and LlamaAgent integration.
The server exposes MCP tools through HTTP endpoints and provides:

- RESTful MCP protocol implementation
- Health check endpoint at /health
- Support for random port allocation (use port 0)
- Graceful shutdown with Ctrl+C

Example:
  swissarmyhammer serve http --port 8080 --host 127.0.0.1
  swissarmyhammer serve http --port 0  # Random port
")]
    Http {
        /// Port to bind to (use 0 for random port)
        #[arg(long, short = 'p', default_value = "8000", value_parser = clap::value_parser!(u16))]
        port: u16,

        /// Host to bind to
        #[arg(long, short = 'H', default_value = "127.0.0.1")]
        host: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ModelSubcommand {
    /// List available models
    #[command(long_about = "
List all available models from built-in, project, and user sources.

Models are discovered with hierarchical precedence where user models override
project models, which override built-in models. This command shows all available
models with their sources and descriptions.

Built-in models are embedded in the binary and provide default configurations
for common workflows. Project models (./models/*.yaml) allow customization for
specific projects. User models (~/.models/*.yaml) provide
personal configurations that apply across all projects.

Output includes:
• Model name and source (built-in, project, or user)
• Description when available
• Current model status (if one is applied to the project)

Examples:
  sah model list                           # List all models in table format
  sah model list --format json            # Output as JSON for processing
  sah --verbose model list                 # Include detailed descriptions
  sah --quiet model list                   # Only show model names
")]
    List {
        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
    /// Show current model configuration
    #[command(long_about = "
Display the current model configured for this project.

Shows the model name, source, and description. If no model is explicitly
configured, the default (claude-code) is used.

Examples:
  sah model show                           # Show current model
  sah model                               # Same as 'show' (default)
")]
    Show {
        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
    /// Use a specific model
    #[command(long_about = "
Apply a specific model configuration to the current project.

This command finds the specified model by name and applies its configuration
to the project by creating or updating .sah/sah.yaml. The model
configuration determines how SwissArmyHammer executes AI workflows in your
project, including which AI model to use and how to execute tools.

Model precedence (highest to lowest):
• User models: ~/.models/<name>.yaml
• Project models: ./models/<name>.yaml
• Built-in models: embedded in the binary

The command preserves any existing configuration sections while updating
only the model configuration. This allows you to maintain project-specific
settings alongside model configurations.

Common model types:
• claude-code    - Uses Claude Code CLI for AI execution
• qwen-coder     - Uses local Qwen3-Coder model with in-process execution
• custom models  - User-defined configurations for specialized workflows

Examples:
  sah model use claude-code                # Apply Claude Code model
  sah model use qwen-coder                # Apply Qwen Coder model
  sah --debug model use claude-code        # Apply with debug output
")]
    Use {
        /// Model name to apply to the project
        #[arg(id = "name")]
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum AgentSubcommand {
    /// Start ACP server over stdio
    #[command(long_about = "
Start Agent Client Protocol (ACP) server for code editor integration.

The ACP server enables SwissArmyHammer to work with ACP-compatible code editors
like Zed and JetBrains IDEs. The server communicates over stdin/stdout using
JSON-RPC 2.0 protocol.

Features:
• Local LLaMA model execution for coding assistance
• Session management with conversation history
• File system operations (read/write)
• Terminal execution
• Tool integration via MCP servers
• Permission-based security model

Examples:
  sah agent acp                        # Start with default config
  sah agent acp --config acp.yaml      # Start with custom config
  sah agent acp --permission-policy auto-approve-reads
  sah agent acp --allow-path /home/user/projects --block-path /home/user/.ssh
  sah agent acp --max-file-size 5242880 --terminal-buffer-size 2097152

Configuration:
Options can be specified via:
1. Command-line flags (highest priority)
2. Configuration file (--config)
3. Default values (lowest priority)

Command-line flags override configuration file settings.

For editor configuration:
• Zed: Add to agents section in settings
• JetBrains: Install ACP plugin and configure
")]
    Acp {
        /// Path to ACP configuration file (optional)
        #[arg(short, long)]
        config: Option<std::path::PathBuf>,

        /// Permission policy: always-ask, auto-approve-reads
        #[arg(long, value_name = "POLICY")]
        permission_policy: Option<String>,

        /// Allowed filesystem paths (can be specified multiple times)
        #[arg(long, value_name = "PATH")]
        allow_path: Vec<std::path::PathBuf>,

        /// Blocked filesystem paths (can be specified multiple times)
        #[arg(long, value_name = "PATH")]
        block_path: Vec<std::path::PathBuf>,

        /// Maximum file size for read operations in bytes
        #[arg(long, value_name = "BYTES")]
        max_file_size: Option<u64>,

        /// Terminal output buffer size in bytes
        #[arg(long, value_name = "BYTES")]
        terminal_buffer_size: Option<usize>,

        /// Graceful shutdown timeout in seconds
        #[arg(long, value_name = "SECONDS")]
        graceful_shutdown_timeout: Option<u64>,
    },
}

#[derive(Subcommand, Debug)]
pub enum StatuslineSubcommand {
    /// Dump the full annotated builtin config to stdout
    Config,
}

#[derive(Subcommand, Debug)]
pub enum ToolsSubcommand {
    /// Enable tools (all if no names given)
    Enable {
        /// Tool names to enable (omit for all)
        names: Vec<String>,
    },
    /// Disable tools (all if no names given)
    Disable {
        /// Tool names to disable (omit for all)
        names: Vec<String>,
    },
}

impl Cli {
    #[allow(dead_code)]
    pub fn try_parse_from_args<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        <Self as Parser>::try_parse_from(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_help_works() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--help"]);
        assert!(result.is_err()); // Help exits with error code but that's expected

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_cli_version_works() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--version"]);
        assert!(result.is_err()); // Version exits with error code but that's expected

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_cli_no_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.command.is_none());
        assert!(!cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn test_cli_serve_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "serve"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Serve { subcommand: _ })
        ));
    }

    #[test]
    fn test_cli_init_default() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "init"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                target: InstallTarget::Project
            })
        ));
    }

    #[test]
    fn test_cli_init_user() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "init", "user"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                target: InstallTarget::User
            })
        ));
    }

    #[test]
    fn test_cli_init_local() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "init", "local"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                target: InstallTarget::Local
            })
        ));
    }

    #[test]
    fn test_cli_deinit_default() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "deinit"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Deinit {
            target,
            remove_directory,
        }) = cli.command
        {
            assert_eq!(target, InstallTarget::Project);
            assert!(!remove_directory);
        } else {
            unreachable!("Expected Deinit command");
        }
    }

    #[test]
    fn test_cli_deinit_with_remove_directory() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "deinit", "--remove-directory"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Deinit {
            target,
            remove_directory,
        }) = cli.command
        {
            assert_eq!(target, InstallTarget::Project);
            assert!(remove_directory);
        } else {
            unreachable!("Expected Deinit command");
        }
    }

    #[test]
    fn test_cli_deinit_user_with_remove_directory() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "deinit", "user", "--remove-directory"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Deinit {
            target,
            remove_directory,
        }) = cli.command
        {
            assert_eq!(target, InstallTarget::User);
            assert!(remove_directory);
        } else {
            unreachable!("Expected Deinit command");
        }
    }

    #[test]
    fn test_cli_doctor_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "doctor"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.command, Some(Commands::Doctor {})));
    }

    #[test]
    fn test_cli_verbose_flag() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--verbose"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn test_cli_quiet_flag() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--quiet"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.quiet);
        assert!(!cli.verbose);
    }

    #[test]
    fn test_cli_serve_with_verbose() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--verbose", "serve"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.verbose);
        assert!(matches!(
            cli.command,
            Some(Commands::Serve { subcommand: _ })
        ));
    }

    #[test]
    fn test_cli_invalid_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "invalid"]);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn test_cli_validate_command() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "validate"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Validate {
            quiet,
            format,
            validate_tools: _,
        }) = cli.command
        {
            assert!(!quiet);
            assert!(matches!(format, OutputFormat::Table));
        } else {
            unreachable!("Expected Validate command");
        }
    }

    #[test]
    fn test_cli_validate_command_with_options() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "validate",
            "--quiet",
            "--format",
            "json",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Validate {
            quiet,
            format,
            validate_tools: _,
        }) = cli.command
        {
            assert!(quiet);
            assert!(matches!(format, OutputFormat::Json));
        } else {
            unreachable!("Expected Validate command");
        }
    }

    #[test]
    fn test_parse_args_panics_on_error() {
        // This test verifies that parse_args would panic on invalid input
        // We can't easily test the panic itself in unit tests, but we can verify
        // that the underlying try_parse_from_args returns an error
        let result = Cli::try_parse_from_args(["swissarmyhammer", "invalid-command"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_source_arg_conversions() {
        // Test From<PromptSourceArg> for PromptSource
        assert!(matches!(
            PromptSource::from(PromptSourceArg::Builtin),
            PromptSource::Builtin
        ));
        assert!(matches!(
            PromptSource::from(PromptSourceArg::User),
            PromptSource::User
        ));
        assert!(matches!(
            PromptSource::from(PromptSourceArg::Local),
            PromptSource::Local
        ));
        assert!(matches!(
            PromptSource::from(PromptSourceArg::Dynamic),
            PromptSource::Dynamic
        ));

        // Test From<PromptSource> for PromptSourceArg
        assert!(matches!(
            PromptSourceArg::from(PromptSource::Builtin),
            PromptSourceArg::Builtin
        ));
        assert!(matches!(
            PromptSourceArg::from(PromptSource::User),
            PromptSourceArg::User
        ));
        assert!(matches!(
            PromptSourceArg::from(PromptSource::Local),
            PromptSourceArg::Local
        ));
        assert!(matches!(
            PromptSourceArg::from(PromptSource::Dynamic),
            PromptSourceArg::Dynamic
        ));
    }

    #[test]
    fn test_prompt_source_arg_equality() {
        assert_eq!(PromptSourceArg::Builtin, PromptSourceArg::Builtin);
        assert_ne!(PromptSourceArg::Builtin, PromptSourceArg::User);
        assert_ne!(PromptSourceArg::User, PromptSourceArg::Local);
        assert_ne!(PromptSourceArg::Local, PromptSourceArg::Dynamic);
    }

    #[test]
    fn test_debug_flag() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--debug"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.debug);
        assert!(!cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn test_combined_flags() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--debug", "--verbose"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.debug);
        assert!(cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn test_global_format_flag() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--format", "json", "doctor"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.format, Some(OutputFormat::Json)));
    }

    #[test]
    fn test_global_format_flag_yaml() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--format", "yaml", "doctor"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.format, Some(OutputFormat::Yaml)));
    }

    #[test]
    fn test_global_format_flag_table() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--format", "table", "doctor"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.format, Some(OutputFormat::Table)));
    }

    #[test]
    fn test_global_format_flag_default() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "doctor"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        // When global format is not specified, it should be None
        assert_eq!(cli.format, None);
    }

    #[test]
    fn test_global_format_flag_with_verbose() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "--verbose",
            "--format",
            "json",
            "doctor",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.verbose);
        assert!(matches!(cli.format, Some(OutputFormat::Json)));
    }

    #[test]
    fn test_global_format_flag_invalid() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--format", "invalid", "doctor"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_tools_no_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "tools"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Tools { global, subcommand }) = cli.command {
            assert!(!global);
            assert!(subcommand.is_none());
        } else {
            unreachable!("Expected Tools command");
        }
    }

    #[test]
    fn test_tools_enable_multiple_names() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "tools", "enable", "shell", "git"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Tools {
            global,
            subcommand: Some(ToolsSubcommand::Enable { names }),
        }) = cli.command
        {
            assert!(!global);
            assert_eq!(names, vec!["shell", "git"]);
        } else {
            unreachable!("Expected Tools Enable command with names");
        }
    }

    #[test]
    fn test_tools_disable_single_name() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "tools", "disable", "kanban"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Tools {
            global,
            subcommand: Some(ToolsSubcommand::Disable { names }),
        }) = cli.command
        {
            assert!(!global);
            assert_eq!(names, vec!["kanban"]);
        } else {
            unreachable!("Expected Tools Disable command with name");
        }
    }

    #[test]
    fn test_tools_global_flag_with_enable() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "tools", "--global", "enable", "shell"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Tools {
            global,
            subcommand: Some(ToolsSubcommand::Enable { names }),
        }) = cli.command
        {
            assert!(global);
            assert_eq!(names, vec!["shell"]);
        } else {
            unreachable!("Expected Tools Enable command with global flag");
        }
    }

    #[test]
    fn test_tools_enable_no_names() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "tools", "enable"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Tools {
            global,
            subcommand: Some(ToolsSubcommand::Enable { names }),
        }) = cli.command
        {
            assert!(!global);
            assert!(names.is_empty());
        } else {
            unreachable!("Expected Tools Enable command with empty names");
        }
    }
}
