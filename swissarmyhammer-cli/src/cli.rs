use crate::commands;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Default)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Yaml,
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

Main commands:
  serve         Run as MCP server (default when invoked via stdio)
  doctor        Diagnose configuration and setup issues
  prompt        Manage and test prompts with interactive capabilities
  flow          Execute and manage workflows for complex task automation
  agent         Manage and interact with specialized agents for specific use cases
  validate      Validate prompt files and workflows for syntax and best practices
  completion    Generate shell completion scripts

Example usage:
  swissarmyhammer serve                           # Run as MCP server
  swissarmyhammer doctor                          # Check configuration
  swissarmyhammer --verbose prompt list          # List prompts with details
  swissarmyhammer --format=json prompt list      # List prompts as JSON
  swissarmyhammer --debug prompt test help       # Test prompt with debug info
  swissarmyhammer agent list                     # List available agents
  swissarmyhammer agent use claude-code          # Apply Claude Code agent to project
  swissarmyhammer flow run code-review           # Execute code review workflow
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
    /// Execute and manage workflows
    #[command(long_about = commands::flow::DESCRIPTION)]
    #[command(trailing_var_arg = true)]
    Flow {
        /// Workflow name or 'list' command followed by arguments
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
    /// Validate prompt files and workflows for syntax and best practices
    #[command(long_about = commands::validate::DESCRIPTION)]
    Validate {
        /// Suppress all output except errors. In quiet mode, warnings are hidden from both output and summary.
        #[arg(short, long)]
        quiet: bool,

        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,

        /// \[DEPRECATED\] This parameter is ignored. Workflows are now only loaded from standard locations.
        #[arg(long = "workflow-dir", value_name = "DIR", hide = true)]
        workflow_dirs: Vec<String>,

        /// Validate MCP tool schemas for CLI compatibility
        #[arg(long)]
        validate_tools: bool,
    },

    /// Manage and interact with agents
    #[command(long_about = commands::agent::DESCRIPTION)]
    Agent {
        #[command(subcommand)]
        subcommand: AgentSubcommand,
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
pub enum FlowSubcommand {
    /// Execute a workflow directly
    Execute {
        /// Workflow name to execute
        workflow: String,

        /// Required workflow parameters as positional arguments
        positional_args: Vec<String>,

        /// Optional workflow parameters as key=value pairs
        #[arg(long = "param", short = 'p', value_name = "KEY=VALUE")]
        params: Vec<String>,

        /// \[DEPRECATED\] Use --param instead
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,

        /// Interactive mode - prompt at each state
        #[arg(short, long)]
        interactive: bool,

        /// Dry run - show execution plan without running
        #[arg(long)]
        dry_run: bool,

        /// Quiet mode - only show errors
        #[arg(short, long)]
        quiet: bool,
    },
    /// List available workflows
    List {
        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,

        /// Show verbose output including workflow details
        #[arg(short, long)]
        verbose: bool,

        /// Filter by source
        #[arg(long, value_enum)]
        source: Option<PromptSourceArg>,
    },
}

#[derive(Subcommand, Debug)]
pub enum AgentSubcommand {
    /// List available agents
    #[command(long_about = "
List all available agents from built-in, project, and user sources.

Agents are discovered with hierarchical precedence where user agents override
project agents, which override built-in agents. This command shows all available
agents with their sources and descriptions.

Built-in agents are embedded in the binary and provide default configurations
for common workflows. Project agents (./agents/*.yaml) allow customization for
specific projects. User agents (~/.swissarmyhammer/agents/*.yaml) provide
personal configurations that apply across all projects.

Output includes:
• Agent name and source (built-in, project, or user)
• Description when available
• Current agent status (if one is applied to the project)

Examples:
  sah agent list                           # List all agents in table format
  sah agent list --format json            # Output as JSON for processing
  sah --verbose agent list                 # Include detailed descriptions
  sah --quiet agent list                   # Only show agent names
")]
    List {
        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
    /// Use a specific agent
    #[command(long_about = "
Apply a specific agent configuration to the current project.

This command finds the specified agent by name and applies its configuration
to the project by creating or updating .swissarmyhammer/sah.yaml. The agent
configuration determines how SwissArmyHammer executes AI workflows in your
project, including which AI model to use and how to execute tools.

Agent precedence (highest to lowest):
• User agents: ~/.swissarmyhammer/agents/<name>.yaml
• Project agents: ./agents/<name>.yaml  
• Built-in agents: embedded in the binary

The command preserves any existing configuration sections while updating
only the agent configuration. This allows you to maintain project-specific
settings alongside agent configurations.

Common agent types:
• claude-code    - Uses Claude Code CLI for AI execution
• qwen-coder     - Uses local Qwen3-Coder model with in-process execution
• custom agents  - User-defined configurations for specialized workflows

Examples:
  sah agent use claude-code                # Apply Claude Code agent
  sah agent use qwen-coder                 # Switch to local Qwen model
  sah agent use my-custom-agent            # Apply user-defined agent
  sah --debug agent use claude-code        # Apply with debug output
")]
    Use {
        /// Name of the agent to use
        agent_name: String,
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
            workflow_dirs,
            validate_tools: _,
        }) = cli.command
        {
            assert!(!quiet);
            assert!(matches!(format, OutputFormat::Table));
            assert!(workflow_dirs.is_empty());
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
            "--workflow-dir",
            "./workflows",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Validate {
            quiet,
            format,
            workflow_dirs,
            validate_tools: _,
        }) = cli.command
        {
            assert!(quiet);
            assert!(matches!(format, OutputFormat::Json));
            assert_eq!(workflow_dirs, vec!["./workflows"]);
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
    fn test_flow_run_basic_workflow() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "flow", "implement"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            if let FlowSubcommand::Execute {
                workflow,
                positional_args,
                params,
                vars,
                interactive,
                dry_run,
                quiet,
            } = subcommand
            {
                assert_eq!(workflow, "implement");
                assert!(positional_args.is_empty());
                assert!(params.is_empty());
                assert!(vars.is_empty());
                assert!(!interactive);
                assert!(!dry_run);
                assert!(!quiet);
            } else {
                unreachable!("Expected Execute subcommand");
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_run_with_positional_args() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "flow", "plan", "spec.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            if let FlowSubcommand::Execute {
                workflow,
                positional_args,
                params,
                vars,
                ..
            } = subcommand
            {
                assert_eq!(workflow, "plan");
                assert_eq!(positional_args, vec!["spec.md"]);
                assert!(params.is_empty());
                assert!(vars.is_empty());
            } else {
                unreachable!("Expected Execute subcommand");
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_run_with_multiple_positional_args() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "flow",
            "code-review",
            "main",
            "feature-x",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            if let FlowSubcommand::Execute {
                workflow,
                positional_args,
                ..
            } = subcommand
            {
                assert_eq!(workflow, "code-review");
                assert_eq!(positional_args, vec!["main", "feature-x"]);
            } else {
                unreachable!("Expected Execute subcommand");
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_run_with_param_flag() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "flow",
            "plan",
            "spec.md",
            "--param",
            "author=alice",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            if let FlowSubcommand::Execute {
                workflow,
                positional_args,
                params,
                vars,
                ..
            } = subcommand
            {
                assert_eq!(workflow, "plan");
                assert_eq!(positional_args, vec!["spec.md"]);
                assert_eq!(params, vec!["author=alice"]);
                assert!(vars.is_empty());
            } else {
                unreachable!("Expected Execute subcommand");
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_run_with_multiple_params() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "flow",
            "custom",
            "--param",
            "key1=value1",
            "--param",
            "key2=value2",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            if let FlowSubcommand::Execute { params, .. } = subcommand {
                assert_eq!(params, vec!["key1=value1", "key2=value2"]);
            } else {
                unreachable!("Expected Execute subcommand");
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_run_with_deprecated_var_flag() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "flow", "plan", "--var", "input=test"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            if let FlowSubcommand::Execute {
                workflow,
                params,
                vars,
                ..
            } = subcommand
            {
                assert_eq!(workflow, "plan");
                assert!(params.is_empty());
                assert_eq!(vars, vec!["input=test"]);
            } else {
                unreachable!("Expected Execute subcommand");
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_run_with_both_param_and_var() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "flow",
            "workflow",
            "--param",
            "key1=param_value",
            "--var",
            "key2=var_value",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            if let FlowSubcommand::Execute { params, vars, .. } = subcommand {
                assert_eq!(params, vec!["key1=param_value"]);
                assert_eq!(vars, vec!["key2=var_value"]);
            } else {
                unreachable!("Expected Execute subcommand");
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_run_with_all_flags() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "flow",
            "plan",
            "spec.md",
            "--param",
            "author=alice",
            "--interactive",
            "--dry-run",
            "--quiet",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            if let FlowSubcommand::Execute {
                workflow,
                positional_args,
                params,
                interactive,
                dry_run,
                quiet,
                ..
            } = subcommand
            {
                assert_eq!(workflow, "plan");
                assert_eq!(positional_args, vec!["spec.md"]);
                assert_eq!(params, vec!["author=alice"]);
                assert!(interactive);
                assert!(dry_run);
                assert!(quiet);
            } else {
                unreachable!("Expected Execute subcommand");
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_run_short_param_flag() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "flow", "workflow", "-p", "key=value"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            if let FlowSubcommand::Execute { params, .. } = subcommand {
                assert_eq!(params, vec!["key=value"]);
            } else {
                unreachable!("Expected Execute subcommand");
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    // New tests for flattened flow command structure (NO "run" subcommand)
    // These tests represent the desired behavior per ideas/flow_mcp.md spec

    #[test]
    fn test_flow_direct_workflow_basic() {
        // Test: sah flow implement (no "run")
        let result = Cli::try_parse_from_args(["swissarmyhammer", "flow", "implement"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            match subcommand {
                FlowSubcommand::Execute {
                    workflow,
                    positional_args,
                    params,
                    ..
                } => {
                    assert_eq!(workflow, "implement");
                    assert!(positional_args.is_empty());
                    assert!(params.is_empty());
                }
                _ => unreachable!("Expected Execute variant"),
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_direct_workflow_with_positional() {
        // Test: sah flow plan spec.md (no "run")
        let result = Cli::try_parse_from_args(["swissarmyhammer", "flow", "plan", "spec.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            match subcommand {
                FlowSubcommand::Execute {
                    workflow,
                    positional_args,
                    ..
                } => {
                    assert_eq!(workflow, "plan");
                    assert_eq!(positional_args, vec!["spec.md"]);
                }
                _ => unreachable!("Expected Execute variant"),
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_direct_workflow_with_multiple_positional() {
        // Test: sah flow code-review main feature-x (no "run")
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "flow",
            "code-review",
            "main",
            "feature-x",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            match subcommand {
                FlowSubcommand::Execute {
                    workflow,
                    positional_args,
                    ..
                } => {
                    assert_eq!(workflow, "code-review");
                    assert_eq!(positional_args, vec!["main", "feature-x"]);
                }
                _ => unreachable!("Expected Execute variant"),
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_direct_workflow_with_params() {
        // Test: sah flow plan spec.md --param author=alice (no "run")
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "flow",
            "plan",
            "spec.md",
            "--param",
            "author=alice",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            match subcommand {
                FlowSubcommand::Execute {
                    workflow,
                    positional_args,
                    params,
                    ..
                } => {
                    assert_eq!(workflow, "plan");
                    assert_eq!(positional_args, vec!["spec.md"]);
                    assert_eq!(params, vec!["author=alice"]);
                }
                _ => unreachable!("Expected Execute variant"),
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_direct_workflow_with_flags() {
        // Test: sah flow implement --interactive --quiet (no "run")
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "flow",
            "implement",
            "--interactive",
            "--quiet",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            match subcommand {
                FlowSubcommand::Execute {
                    workflow,
                    interactive,
                    quiet,
                    ..
                } => {
                    assert_eq!(workflow, "implement");
                    assert!(interactive);
                    assert!(quiet);
                }
                _ => unreachable!("Expected Execute variant"),
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_flow_list_special_case() {
        // Test: sah flow list --verbose
        // "list" is a special workflow name for discovery
        let result = Cli::try_parse_from_args(["swissarmyhammer", "flow", "list", "--verbose"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { args }) = cli.command {
            let subcommand =
                commands::flow::parse_flow_args(args).expect("Failed to parse flow args");
            match subcommand {
                FlowSubcommand::List { verbose, .. } => {
                    assert!(verbose);
                }
                _ => unreachable!("Expected List variant"),
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }
}
