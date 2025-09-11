use crate::commands;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum OutputFormat {
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



#[derive(ValueEnum, Clone, Debug)]
pub enum VisualizationFormat {
    Mermaid,
    Html,
    Json,
    Dot,
}

#[derive(Parser, Debug)]
#[command(name = "swissarmyhammer")]
#[command(version)]
#[command(about = "An MCP server for managing prompts as markdown files")]
#[command(long_about = "
swissarmyhammer is an MCP (Model Context Protocol) server that manages
prompts as markdown files. It supports file watching, template substitution,
and seamless integration with Claude Code.

Example usage:
  swissarmyhammer serve     # Run as MCP server
  swissarmyhammer doctor    # Check configuration and setup
  swissarmyhammer completion bash > ~/.bashrc.d/swissarmyhammer  # Generate bash completions
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
    #[command(long_about = commands::prompt::DESCRIPTION)]
    Prompt {
        #[command(subcommand)]
        subcommand: PromptSubcommand,
    },
    /// Execute and manage workflows
    #[command(long_about = commands::flow::DESCRIPTION)]
    Flow {
        #[command(subcommand)]
        subcommand: FlowSubcommand,
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

    /// Plan a specific specification file
    #[command(long_about = commands::plan::DESCRIPTION)]
    Plan {
        /// Path to the plan file to process
        #[arg(help = "Path to the markdown plan file (relative or absolute)")]
        #[arg(long_help = "
Path to the specification file to plan. Can be:
• Relative path: ./specification/feature.md
• Absolute path: /full/path/to/plan.md  
• Simple filename: my-plan.md (in current directory)

The file should be a readable markdown file containing
the specification or requirements to be planned.")]
        plan_filename: String,
    },
    /// Execute the implement workflow for autonomous issue resolution
    #[command(long_about = commands::implement::DESCRIPTION)]
    Implement,
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
pub enum PromptSubcommand {
    /// List all available prompts
    #[command(long_about = "
Lists all available prompts from all sources (built-in, user, local).
Shows prompt names, titles, descriptions, and source information.

Output formats:
  table  - Formatted table (default)
  json   - JSON output for scripting
  yaml   - YAML output for scripting

Examples:
  swissarmyhammer prompt list                        # Show all prompts in table format
  swissarmyhammer prompt list --format json         # Output as JSON
  swissarmyhammer prompt list --verbose             # Show full details including arguments
  swissarmyhammer prompt list --source builtin      # Show only built-in prompts

")]
    List {
        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,

        /// Show verbose output including arguments
        #[arg(short, long)]
        verbose: bool,

        /// Filter by source
        #[arg(long, value_enum)]
        source: Option<PromptSourceArg>,

        /// Filter by category
        #[arg(long)]
        category: Option<String>,
    },
    /// Test prompts interactively with sample arguments
    #[command(long_about = "
Test prompts interactively to see how they render with different arguments.
Helps debug template errors and refine prompt content before using in Claude Code.

Usage modes:
  swissarmyhammer prompt test prompt-name                    # Test by name (interactive)
  swissarmyhammer prompt test -f path/to/prompt.md          # Test from file
  swissarmyhammer prompt test prompt-name --var key=value   # Non-interactive mode

Interactive features:
- Prompts for each argument with descriptions
- Shows default values (press Enter to accept)
- Validates required arguments
- Supports multi-line input

Output options:
  --raw     Show rendered prompt without formatting
  --copy    Copy rendered prompt to clipboard
  --save    Save rendered prompt to file
  --debug   Show template processing details

Examples:
  swissarmyhammer prompt test code-review                           # Interactive test
  swissarmyhammer prompt test -f my-prompt.md                       # Test file
  swissarmyhammer prompt test help --var topic=git                  # Non-interactive
  swissarmyhammer prompt test plan --debug --save output.md         # Debug + save
  swissarmyhammer prompt test code-review --var author=John --var version=1.0  # With template variables
")]
    Test {
        /// Prompt name to test (alternative to --file)
        prompt_name: Option<String>,

        /// Path to prompt file to test
        #[arg(short, long)]
        file: Option<String>,

        /// Non-interactive mode: specify variables as key=value pairs
        #[arg(long = "var", alias = "arg", value_name = "KEY=VALUE")]
        vars: Vec<String>,

        /// Show raw output without formatting
        #[arg(long)]
        raw: bool,

        /// Copy rendered prompt to clipboard
        #[arg(long)]
        copy: bool,

        /// Save rendered prompt to file
        #[arg(long, value_name = "FILE")]
        save: Option<String>,

        /// Show debug information (template, args, processing steps)
        #[arg(long)]
        debug: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum FlowSubcommand {
    /// Run a workflow
    Run {
        /// Workflow name to run
        workflow: String,

        /// Initial variables as key=value pairs
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,

        /// Interactive mode - prompt at each state
        #[arg(short, long)]
        interactive: bool,

        /// Dry run - show execution plan without running
        #[arg(long)]
        dry_run: bool,

        /// Execution timeout (e.g., 30s, 5m, 1h)
        #[arg(long)]
        timeout: Option<String>,

        /// Quiet mode - only show errors
        #[arg(short, long)]
        quiet: bool,
    },
    /// Resume a paused workflow run
    Resume {
        /// Run ID to resume
        run_id: String,

        /// Interactive mode - prompt at each state
        #[arg(short, long)]
        interactive: bool,

        /// Execution timeout (e.g., 30s, 5m, 1h)
        #[arg(long)]
        timeout: Option<String>,

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
    /// Check status of a workflow run
    Status {
        /// Run ID to check
        run_id: String,

        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,

        /// Watch for status changes
        #[arg(short, long)]
        watch: bool,
    },
    /// View logs for a workflow run
    Logs {
        /// Run ID to view logs for
        run_id: String,

        /// Follow log output (like tail -f)
        #[arg(short, long)]
        follow: bool,

        /// Number of log lines to show (from end)
        #[arg(short = 'n', long)]
        tail: Option<usize>,

        /// Filter logs by level (info, warn, error)
        #[arg(long)]
        level: Option<String>,
    },
    /// View metrics for workflow runs
    Metrics {
        /// Run ID to view metrics for (optional - shows all if not specified)
        run_id: Option<String>,

        /// Workflow name to filter by
        #[arg(long)]
        workflow: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,

        /// Show global metrics summary
        #[arg(short, long)]
        global: bool,
    },
    /// Generate execution visualization
    Visualize {
        /// Run ID to visualize
        run_id: String,

        /// Output format
        #[arg(long, value_enum, default_value = "mermaid")]
        format: VisualizationFormat,

        /// Output file path (optional - prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<String>,

        /// Include timing information
        #[arg(long)]
        timing: bool,

        /// Include execution counts
        #[arg(long)]
        counts: bool,

        /// Show only executed path
        #[arg(long)]
        path_only: bool,
    },
    /// Test a workflow without executing actions (simulates dry run)
    #[command(long_about = "
Test workflows in simulation mode without actually executing actions.
This command provides a safe way to validate workflow logic and see what
actions would be executed without actually running them.

Features:
- Simulates all actions instead of executing them
- Claude prompts are echoed instead of sent to the API
- Generates coverage reports showing visited states and transitions
- Useful for testing workflow logic and debugging

Usage:
  swissarmyhammer flow test my-workflow
  swissarmyhammer flow test my-workflow --var key=value
  swissarmyhammer flow test my-workflow --var template_var=value

Examples:
  swissarmyhammer flow test hello-world                               # Test basic workflow
  swissarmyhammer flow test greeting --var name=John --var language=Spanish  # With template variables
  swissarmyhammer flow test code-review --var file=main.rs --timeout 60s     # With vars and timeout
  swissarmyhammer flow test deploy --interactive                      # Step-by-step execution

This is equivalent to 'flow run --test' but provided as a separate command
for better discoverability and clearer intent.
")]
    Test {
        /// Workflow name to test
        workflow: String,

        /// Initial variables as key=value pairs
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,

        /// Interactive mode - prompt at each state
        #[arg(short, long)]
        interactive: bool,

        /// Execution timeout (e.g., 30s, 5m, 1h)
        #[arg(long)]
        timeout: Option<String>,

        /// Quiet mode - only show errors
        #[arg(short, long)]
        quiet: bool,
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

    /// Determine if color output should be used
    pub fn should_use_color() -> bool {
        use is_terminal::IsTerminal;
        std::io::stderr().is_terminal()
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
    fn test_cli_test_subcommand_with_prompt_name() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "prompt", "test", "help"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Prompt { subcommand }) = cli.command {
            if let PromptSubcommand::Test {
                prompt_name,
                file,
                vars,
                raw,
                copy,
                save,
                debug,
            } = subcommand
            {
                assert_eq!(prompt_name, Some("help".to_string()));
                assert_eq!(file, None);
                assert!(vars.is_empty());
                assert!(!raw);
                assert!(!copy);
                assert_eq!(save, None);
                assert!(!debug);
            } else {
                unreachable!("Expected Test subcommand");
            }
        } else {
            unreachable!("Expected Prompt command");
        }
    }

    #[test]
    fn test_cli_test_subcommand_with_file() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "prompt", "test", "-f", "test.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Prompt { subcommand }) = cli.command {
            if let PromptSubcommand::Test {
                prompt_name,
                file,
                vars,
                raw,
                copy,
                save,
                debug,
            } = subcommand
            {
                assert_eq!(prompt_name, None);
                assert_eq!(file, Some("test.md".to_string()));
                assert!(vars.is_empty());
                assert!(!raw);
                assert!(!copy);
                assert_eq!(save, None);
                assert!(!debug);
            } else {
                unreachable!("Expected Test subcommand");
            }
        } else {
            unreachable!("Expected Prompt command");
        }
    }

    #[test]
    fn test_cli_test_subcommand_with_arguments() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "prompt",
            "test",
            "help",
            "--var",
            "topic=git",
            "--var",
            "format=markdown",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Prompt { subcommand }) = cli.command {
            if let PromptSubcommand::Test {
                prompt_name,
                file,
                vars,
                raw,
                copy,
                save,
                debug,
            } = subcommand
            {
                assert_eq!(prompt_name, Some("help".to_string()));
                assert_eq!(file, None);
                assert_eq!(vars, vec!["topic=git", "format=markdown"]);
                assert!(!raw);
                assert!(!copy);
                assert_eq!(save, None);
                assert!(!debug);
            } else {
                unreachable!("Expected Test subcommand");
            }
        } else {
            unreachable!("Expected Prompt command");
        }
    }

    #[test]
    fn test_cli_test_subcommand_with_all_flags() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "prompt",
            "test",
            "help",
            "--raw",
            "--copy",
            "--debug",
            "--save",
            "output.md",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Prompt { subcommand }) = cli.command {
            if let PromptSubcommand::Test {
                prompt_name,
                file,
                vars,
                raw,
                copy,
                save,
                debug,
            } = subcommand
            {
                assert_eq!(prompt_name, Some("help".to_string()));
                assert_eq!(file, None);
                assert!(vars.is_empty());
                assert!(raw);
                assert!(copy);
                assert_eq!(save, Some("output.md".to_string()));
                assert!(debug);
            } else {
                unreachable!("Expected Test subcommand");
            }
        } else {
            unreachable!("Expected Prompt command");
        }
    }

    #[test]
    fn test_cli_test_subcommand_with_var_variables() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "prompt",
            "test",
            "help",
            "--var",
            "topic=git",
            "--var",
            "author=John",
            "--var",
            "version=1.0",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Prompt { subcommand }) = cli.command {
            if let PromptSubcommand::Test {
                prompt_name,
                file,
                vars,
                raw,
                copy,
                save,
                debug,
            } = subcommand
            {
                assert_eq!(prompt_name, Some("help".to_string()));
                assert_eq!(file, None);
                assert_eq!(vars, vec!["topic=git", "author=John", "version=1.0"]);
                assert!(!raw);
                assert!(!copy);
                assert_eq!(save, None);
                assert!(!debug);
            } else {
                unreachable!("Expected Test subcommand");
            }
        } else {
            unreachable!("Expected Prompt command");
        }
    }

    #[test]
    fn test_cli_prompt_list_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "prompt", "list"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Prompt { subcommand }) = cli.command {
            if let PromptSubcommand::List {
                format,
                verbose,
                source,
                category,
            } = subcommand
            {
                assert!(matches!(format, OutputFormat::Table));
                assert!(!verbose);
                assert_eq!(source, None);
                assert_eq!(category, None);
            } else {
                unreachable!("Expected List subcommand");
            }
        } else {
            unreachable!("Expected Prompt command");
        }
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
    fn test_cli_flow_test_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "flow", "test", "my-workflow"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { subcommand }) = cli.command {
            if let FlowSubcommand::Test {
                workflow,
                vars,
                interactive,
                timeout,
                quiet,
            } = subcommand
            {
                assert_eq!(workflow, "my-workflow");
                assert!(vars.is_empty());
                assert!(!interactive);
                assert_eq!(timeout, None);
                assert!(!quiet);
            } else {
                unreachable!("Expected Test subcommand");
            }
        } else {
            unreachable!("Expected Flow command");
        }
    }

    #[test]
    fn test_cli_flow_test_subcommand_with_options() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "flow",
            "test",
            "my-workflow",
            "--var",
            "input=test",
            "--var",
            "author=Jane",
            "--var",
            "version=2.0",
            "--interactive",
            "--timeout",
            "30s",
            "--quiet",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Flow { subcommand }) = cli.command {
            if let FlowSubcommand::Test {
                workflow,
                vars,
                interactive,
                timeout,
                quiet,
            } = subcommand
            {
                assert_eq!(workflow, "my-workflow");
                assert_eq!(vars, vec!["input=test", "author=Jane", "version=2.0"]);
                assert!(interactive);
                assert_eq!(timeout, Some("30s".to_string()));
                assert!(quiet);
            } else {
                unreachable!("Expected Test subcommand");
            }
        } else {
            unreachable!("Expected Flow command");
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
    fn test_plan_command() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "plan", "./specification/new-feature.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, "./specification/new-feature.md");
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_plan_command_with_absolute_path() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "plan", "/path/to/custom-plan.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, "/path/to/custom-plan.md");
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_plan_command_basic() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "plan", "specification/plan.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, "specification/plan.md");
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_plan_command_relative_path() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "plan", "./plans/feature.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, "./plans/feature.md");
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_plan_command_missing_parameter() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "plan"]);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn test_cli_plan_command_help() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "plan", "--help"]);
        assert!(result.is_err()); // Help exits with error but that's expected

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_cli_plan_command_with_verbose_flag() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "--verbose", "plan", "test-plan.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.verbose);
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, "test-plan.md");
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_plan_command_with_debug_flag() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "--debug", "plan", "debug-plan.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.debug);
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, "debug-plan.md");
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_plan_command_with_quiet_flag() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "--quiet", "plan", "quiet-plan.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.quiet);
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, "quiet-plan.md");
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_plan_command_file_with_spaces() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "plan", "plan with spaces.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, "plan with spaces.md");
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_plan_command_complex_path() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "plan",
            "./specifications/features/advanced-feature-plan.md",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(
                plan_filename,
                "./specifications/features/advanced-feature-plan.md"
            );
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_plan_command_multiple_flags() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "--verbose",
            "--debug",
            "plan",
            "multi-flag-plan.md",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.verbose);
        assert!(cli.debug);
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, "multi-flag-plan.md");
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_plan_command_flag_after_subcommand() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "plan",
            "after-flag-plan.md",
            "--verbose",
        ]);
        // This should fail because --verbose is a global flag and must come before the subcommand
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_plan_command_long_path() {
        let long_path = "./very/long/nested/directory/structure/with/many/levels/plan-file.md";
        let result = Cli::try_parse_from_args(["swissarmyhammer", "plan", long_path]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, long_path);
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_plan_command_with_extension_variations() {
        // Test different file extensions
        let result = Cli::try_parse_from_args(["swissarmyhammer", "plan", "plan.markdown"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, "plan.markdown");
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_plan_command_no_extension() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "plan", "plan-file-without-extension"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Plan { plan_filename }) = cli.command {
            assert_eq!(plan_filename, "plan-file-without-extension");
        } else {
            unreachable!("Expected Plan command");
        }
    }

    #[test]
    fn test_cli_implement_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "implement"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.command, Some(Commands::Implement)));
    }

    #[test]
    fn test_cli_implement_with_verbose() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--verbose", "implement"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.verbose);
        assert!(matches!(cli.command, Some(Commands::Implement)));
    }

    #[test]
    fn test_cli_implement_with_quiet() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--quiet", "implement"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.quiet);
        assert!(matches!(cli.command, Some(Commands::Implement)));
    }

    #[test]
    fn test_cli_implement_with_debug() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--debug", "implement"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.debug);
        assert!(matches!(cli.command, Some(Commands::Implement)));
    }

    #[test]
    fn test_cli_implement_help() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "implement", "--help"]);
        assert!(result.is_err()); // Help exits with error but that's expected

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_cli_implement_no_extra_args() {
        // Ensure implement command doesn't accept unexpected arguments
        let result = Cli::try_parse_from_args(["swissarmyhammer", "implement", "extra"]);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn test_cli_implement_combined_flags() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "--verbose", "--debug", "implement"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.verbose);
        assert!(cli.debug);
        assert!(matches!(cli.command, Some(Commands::Implement)));
    }

    #[test]
    fn test_global_format_flag() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--format", "json", "prompt", "list"]);
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
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--format", "table", "prompt", "list"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.format, Some(OutputFormat::Table)));
    }

    #[test]
    fn test_global_format_flag_default() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "prompt", "list"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        // When global format is not specified, it should be None
        assert_eq!(cli.format, None);
    }

    #[test]
    fn test_global_format_flag_with_verbose() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--verbose", "--format", "json", "prompt", "list"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.verbose);
        assert!(matches!(cli.format, Some(OutputFormat::Json)));
    }

    #[test]
    fn test_global_format_flag_invalid() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--format", "invalid", "prompt", "list"]);
        assert!(result.is_err());
    }
}
