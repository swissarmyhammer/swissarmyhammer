use clap::{Parser, Subcommand, ValueEnum};
use is_terminal::IsTerminal;
use std::io;

#[derive(ValueEnum, Clone, Debug)]
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
pub enum ValidateFormat {
    Text,
    Json,
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
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run as MCP server (default when invoked via stdio)
    #[command(long_about = "
Runs swissarmyhammer as an MCP server. This is the default mode when
invoked via stdio (e.g., by Claude Code). The server will:

- Load all prompts from builtin, user, and local directories
- Watch for file changes and reload prompts automatically
- Expose prompts via the MCP protocol
- Support template substitution with {{variables}}

Example:
  swissarmyhammer serve
  # Or configure in Claude Code's MCP settings
")]
    Serve,
    /// Diagnose configuration and setup issues
    #[command(long_about = "
Runs comprehensive diagnostics to help troubleshoot setup issues.
The doctor command will check:

- If swissarmyhammer is in your PATH
- Claude Code MCP configuration
- Prompt directories and permissions
- YAML syntax in prompt files
- File watching capabilities

Exit codes:
  0 - All checks passed
  1 - Warnings found
  2 - Errors found

Example:
  swissarmyhammer doctor
  swissarmyhammer doctor --verbose    # Show detailed diagnostics
  swissarmyhammer doctor --migration  # Check migration status and conflicts
")]
    Doctor {
        /// Check migration status and validate directory consolidation readiness
        #[arg(long)]
        migration: bool,
    },
    /// Manage and test prompts
    #[command(long_about = "
Manage prompts with support for listing, validating, testing, and searching.
Prompts are markdown files with YAML front matter that define reusable templates.

Basic usage:
  swissarmyhammer prompt list                    # List all prompts
  swissarmyhammer prompt validate                # Validate prompt files
  swissarmyhammer prompt test <name>             # Test a prompt
  swissarmyhammer prompt search <query>          # Search prompts

Examples:
  swissarmyhammer prompt list --source builtin
  swissarmyhammer prompt validate --quiet
  swissarmyhammer prompt test code-review --var file=main.rs
  swissarmyhammer prompt search \"python code\"
")]
    Prompt {
        #[command(subcommand)]
        subcommand: PromptSubcommand,
    },
    /// Execute and manage workflows
    #[command(long_about = "
Execute and manage workflows with support for starting new runs and resuming existing ones.
Workflows are defined as state machines that can execute actions and tools including Claude commands.

Basic usage:
  swissarmyhammer flow run my-workflow           # Start new workflow
  swissarmyhammer flow resume <run_id>           # Resume paused workflow
  swissarmyhammer flow list                      # List available workflows
  swissarmyhammer flow status <run_id>           # Check run status
  swissarmyhammer flow logs <run_id>             # View execution logs

Workflow execution:
  --vars key=value                               # Pass initial variables
  --interactive                                  # Step-by-step execution
  --dry-run                                      # Show execution plan
  --timeout 60s                                  # Set execution timeout

Examples:
  swissarmyhammer flow run code-review --vars file=main.rs
  swissarmyhammer flow run deploy --dry-run
  swissarmyhammer flow resume a1b2c3d4 --interactive
  swissarmyhammer flow list --format json
  swissarmyhammer flow status a1b2c3d4 --watch
")]
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
    #[command(long_about = "
Validates BOTH prompt files AND workflows for syntax errors and best practices.

This command comprehensively validates:
- All prompt files from builtin, user, and local directories
- All workflow files from standard locations (builtin, user, local)

NOTE: The --workflow-dir parameter is deprecated and will be ignored.
Workflows are now only loaded from standard locations.

Validation checks:
- YAML front matter syntax (skipped for .liquid files with {% partial %} marker)
- Required fields (title, description)
- Template variables match arguments
- Liquid template syntax
- Workflow structure and connectivity
- Best practice recommendations

Examples:
  swissarmyhammer validate                 # Validate all prompts and workflows
  swissarmyhammer validate --quiet         # CI/CD mode - only shows errors, hides warnings
  swissarmyhammer validate --format json   # JSON output for tooling
")]
    Validate {
        /// Suppress all output except errors. In quiet mode, warnings are hidden from both output and summary.
        #[arg(short, long)]
        quiet: bool,

        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        format: ValidateFormat,

        /// \[DEPRECATED\] This parameter is ignored. Workflows are now only loaded from standard locations.
        #[arg(long = "workflow-dir", value_name = "DIR", hide = true)]
        workflow_dirs: Vec<String>,
    },
    /// Issue management commands
    #[command(long_about = "
Manage issues with comprehensive CLI commands for creating, updating, and tracking work items.
Issues are stored as markdown files in the ./issues directory with automatic numbering.

Basic usage:
  swissarmyhammer issue create [name]           # Create new issue
  swissarmyhammer issue list                    # List all issues
  swissarmyhammer issue show <number>           # Show issue details
  swissarmyhammer issue update <number>         # Update issue content
  swissarmyhammer issue complete <number>       # Mark issue as complete
  swissarmyhammer issue work <number>           # Start working on issue (creates git branch)
  swissarmyhammer issue merge <number>          # Merge completed issue to source branch
  swissarmyhammer issue current                 # Show current issue
  swissarmyhammer issue next                    # Show next issue to work on
  swissarmyhammer issue status                  # Show project status

Examples:
  swissarmyhammer issue create \"Bug fix\" --content \"Fix login issue\"
  swissarmyhammer issue create --content \"Quick fix needed\"
  swissarmyhammer issue list --format json --active
  swissarmyhammer issue show 123 --raw
  swissarmyhammer issue update 123 --content \"Updated description\" --append
  swissarmyhammer issue work 123
  swissarmyhammer issue merge 123 --keep-branch
")]
    Issue {
        #[command(subcommand)]
        subcommand: IssueCommands,
    },
    /// Memoranda (memo) management commands
    #[command(long_about = "
Manage memos with comprehensive CLI commands for creating, updating, and tracking structured text notes.
Memos are stored as markdown files with filename-based identifiers and filesystem-based timestamping.

Basic usage:
  swissarmyhammer memo create <title>           # Create new memo
  swissarmyhammer memo list                     # List all memos
  swissarmyhammer memo get <id>                 # Get specific memo
  swissarmyhammer memo update <id>              # Update memo content
  swissarmyhammer memo delete <id>              # Delete memo
  swissarmyhammer memo search <query>           # Search memos
  swissarmyhammer memo context                  # Get all context for AI

Content input:
  --content \"text\"                            # Specify content directly
  --content -                                   # Read content from stdin
  (no --content)                               # Interactive prompt for content

Examples:
  swissarmyhammer memo create \"Meeting Notes\"
  swissarmyhammer memo create \"Task List\" --content \"1. Review code\\n2. Write tests\"
  swissarmyhammer memo list
  swissarmyhammer memo search \"meeting\"
  swissarmyhammer memo get 01GX5Q2D1NPRZ3KXFW2H8V3A1Y
  swissarmyhammer memo update 01GX5Q2D1NPRZ3KXFW2H8V3A1Y --content \"Updated content\"
  swissarmyhammer memo delete 01GX5Q2D1NPRZ3KXFW2H8V3A1Y
  swissarmyhammer memo context
")]
    Memo {
        #[command(subcommand)]
        subcommand: MemoCommands,
    },
    /// File manipulation and search commands
    #[command(long_about = "
Comprehensive file manipulation and search tools for code analysis, editing, and discovery.
Provides direct CLI access to powerful file operations including reading, writing, editing, and searching.

Basic usage:
  swissarmyhammer file read <PATH>                     # Read file contents
  swissarmyhammer file write <PATH> <CONTENT>          # Write content to file
  swissarmyhammer file edit <PATH> <OLD> <NEW>         # Edit file with string replacement
  swissarmyhammer file glob <PATTERN>                  # Find files by pattern
  swissarmyhammer file grep <PATTERN>                  # Search file contents

File operations:
  read                                         # Read file contents with optional offset/limit
  write                                        # Create new files or overwrite existing ones
  edit                                         # Precise string replacement with atomic operations
  glob                                         # Fast file pattern matching with advanced filtering
  grep                                         # Content-based search using ripgrep

Advanced features:
  --offset, --limit                           # Partial file reading for large files
  --replace-all                               # Replace all occurrences in edit operations
  --case-sensitive, --no-git-ignore           # Advanced pattern matching options
  -i, -C, --glob, --type                      # Comprehensive search filtering

Examples:
  swissarmyhammer file read /path/to/config.json --offset 1 --limit 20
  swissarmyhammer file write ./new-file.txt \"Hello World\"
  swissarmyhammer file edit ./src/main.rs \"old_function\" \"new_function\"
  swissarmyhammer file edit ./config.toml \"debug = false\" \"debug = true\" --replace-all
  swissarmyhammer file glob \"**/*.rs\" --case-sensitive
  swissarmyhammer file grep \"TODO\" --path ./src --glob \"*.rs\" -C 2
  swissarmyhammer file grep \"error.*handling\" --type rust --case-insensitive
")]
    File {
        #[command(subcommand)]
        subcommand: FileCommands,
    },
    /// Semantic search commands
    #[command(long_about = "
Manage semantic search functionality for indexing and searching source code files using vector embeddings.
Uses mistral.rs for embeddings, DuckDB for vector storage, and TreeSitter for parsing.

Basic usage:
  swissarmyhammer search index <patterns...>   # Index files for semantic search
  swissarmyhammer search query <query>          # Query indexed files semantically

Indexing:
  <patterns...>                                 # Glob patterns or files to index (supports multiple)
  --force                                       # Force re-indexing of all files

Querying:
  --limit 10                                    # Number of results to return
  --format table                               # Output format (table, json, yaml)

Examples:
  swissarmyhammer search index \"**/*.rs\"       # Index all Rust files (quoted glob)
  swissarmyhammer search index **/*.rs          # Index all Rust files (shell-expanded)
  swissarmyhammer search index \"src/**/*.py\" --force  # Force re-index Python files
  swissarmyhammer search index file1.rs file2.rs file3.rs  # Index specific files
  swissarmyhammer search query \"error handling\"       # Search for error handling code
  swissarmyhammer search query \"async function\" --limit 5 --format json
")]
    Search {
        #[command(subcommand)]
        subcommand: SearchCommands,
    },
    /// Plan a specific specification file
    #[command(long_about = "
Execute planning workflow for a specific specification file.
Takes a path to a markdown specification file and generates step-by-step implementation issues.

USAGE:
  swissarmyhammer plan <PLAN_FILENAME>

The planning workflow will:
• Read and analyze the specified plan file
• Review existing issues to avoid conflicts
• Generate numbered issue files in the ./issues directory  
• Create incremental, focused implementation steps
• Use existing memos and codebase context for better planning

FILE REQUIREMENTS:
The plan file should be:
• A valid markdown file (.md extension recommended)
• Readable and contain meaningful content
• Focused on a specific feature or component
• Well-structured with clear goals and requirements

OUTPUT:
Creates numbered issue files in ./issues/ directory with format:
• PLANNAME_000001_step-description.md
• PLANNAME_000002_step-description.md
• etc.

EXAMPLES:
  # Plan a new feature from specification directory
  swissarmyhammer plan ./specification/user-authentication.md
  
  # Plan using absolute path
  swissarmyhammer plan /home/user/projects/plans/database-migration.md
  
  # Plan a quick enhancement
  swissarmyhammer plan ./docs/bug-fixes.md
  
  # Plan with verbose output for debugging
  swissarmyhammer --verbose plan ./specification/api-redesign.md

TIPS:
• Keep plan files focused - break large features into multiple plans
• Review generated issues before implementation
• Use descriptive filenames that reflect the planned work
• Check existing issues directory to understand numbering
• Plan files work best when they include clear goals and acceptance criteria

TROUBLESHOOTING:
If planning fails:
• Verify file exists and is readable: ls -la <plan_file>
• Check issues directory permissions: ls -ld ./issues
• Ensure adequate disk space for issue file creation
• Try with --debug flag for detailed execution information
• Review file content for proper markdown formatting

For more information, see: swissarmyhammer --help
")]
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
    #[command(long_about = "
Execute the implement workflow to autonomously work through and resolve all pending issues.
This is a convenience command equivalent to 'sah flow run implement'.

The implement workflow will:
• Check for pending issues in the ./issues directory
• Work through each issue systematically  
• Continue until all issues are resolved
• Provide status updates throughout the process

USAGE:
  swissarmyhammer implement

This command provides:
• Consistency with other top-level workflow commands like 'sah plan'
• Convenient shortcut for the common implement workflow
• Autonomous issue resolution without manual intervention
• Integration with existing workflow infrastructure

EXAMPLES:
  # Run the implement workflow
  swissarmyhammer implement
  
  # Run with verbose output for debugging
  swissarmyhammer --verbose implement
  
  # Run in quiet mode showing only errors
  swissarmyhammer --quiet implement

WORKFLOW DETAILS:
The implement workflow performs the following steps:
1. Checks if all issues are complete
2. If not complete, runs the 'do_issue' workflow on the next issue
3. Repeats until all issues are resolved
4. Provides completion confirmation

For more control over workflow execution, use:
  swissarmyhammer flow run implement --interactive
  swissarmyhammer flow run implement --dry-run

TROUBLESHOOTING:
If implementation fails:
• Check that ./issues directory exists and contains valid issues
• Ensure you have proper permissions to modify issue files
• Review workflow logs for specific error details
• Use --verbose flag for detailed execution information
• Verify the implement workflow exists in builtin workflows
")]
    Implement,
    /// Web search commands
    #[command(long_about = "
Perform web searches using SearXNG metasearch engines with privacy protection and optional content fetching.
Uses the same backend as the MCP web_search tool for consistent functionality.

Basic usage:
  swissarmyhammer web-search search <query>           # Perform web search
  swissarmyhammer web-search search <query> --results 20 --category it --format json

Search options:
  <query>                                              # Search query string (required)
  --category <category>                                # Search category (general, images, videos, news, etc.)
  --results <count>                                    # Number of results to return (1-50, default: 10)
  --language <lang>                                    # Language code (e.g., 'en', 'fr', 'en-US', default: 'en')
  --fetch-content true/false                           # Whether to fetch page content (default: true)
  --safe-search <level>                                # Safe search level: 0=off, 1=moderate, 2=strict (default: 1)
  --time-range <range>                                 # Time filter: day, week, month, year (default: all time)
  --format <format>                                    # Output format: table, json, yaml (default: table)

Categories:
  general, images, videos, news, map, music, it, science, files

Examples:
  swissarmyhammer web-search search \"rust async programming\"
  swissarmyhammer web-search search \"python web scraping\" --results 15 --fetch-content false
  swissarmyhammer web-search search \"machine learning\" --category science --time-range month
  swissarmyhammer web-search search \"docker tutorial\" --category it --language en --format json
  swissarmyhammer web-search search \"latest news\" --category news --time-range day --safe-search 2

Privacy features:
  • Uses SearXNG instances that don't track users
  • Rotates between multiple instances for load distribution
  • No search history storage or logging
  • Encrypted communication with all instances

Performance:
  • Search operations typically complete in 1-3 seconds
  • Content fetching adds 2-5 seconds depending on target sites
  • Automatic fallback to alternative instances
  • Graceful degradation when content fetching fails
")]
    WebSearch {
        #[command(subcommand)]
        subcommand: WebSearchCommands,
    },
    /// Configuration management commands
    #[command(long_about = "
Manage sah.toml configuration files with comprehensive CLI commands for validation, inspection, and debugging.
Configuration files provide project-specific variables for template rendering.

Basic usage:
  swissarmyhammer config show                   # Display current configuration
  swissarmyhammer config variables              # List all available variables
  swissarmyhammer config test                   # Test template rendering with config
  swissarmyhammer config env                    # Show environment variable usage

Validation:
  Validation is automatically included in 'sah validate' command

Output formats:
  --format table                               # Human-readable table format
  --format json                                # JSON output for machine consumption
  --format yaml                                # YAML output for scripting

Examples:
  swissarmyhammer config show --format json    # Output configuration as JSON
  swissarmyhammer config variables             # List all configured variables
  swissarmyhammer config test template.liquid  # Test template with current config
  swissarmyhammer config env --missing         # Show missing environment variables
")]
    Config {
        #[command(subcommand)]
        subcommand: ConfigCommands,
    },
    /// Execute shell commands with timeout and output capture
    #[command(long_about = "
Execute shell commands with comprehensive timeout controls and output capture.
Provides direct command-line access to the shell execution capabilities while 
following established CLI patterns and user experience guidelines.

Basic usage:
  swissarmyhammer shell \"echo 'Hello, World!'\"         # Simple command execution
  swissarmyhammer shell \"ls -la\" -C /tmp               # Execute in specific directory
  swissarmyhammer shell \"cargo build\" -t 600           # Set timeout and environment
  swissarmyhammer shell \"uname -a\" --show-metadata     # Show execution metadata
  swissarmyhammer shell \"git status\" -q --format json  # Quiet mode with JSON output

Command execution:
  <COMMAND>                                    # Shell command to execute (required)
  -C, --directory <DIR>                        # Working directory for execution
  -t, --timeout <SECONDS>                      # Timeout in seconds (default: 300, max: 1800)
  -e, --env <KEY=VALUE>                        # Set environment variables
  --format <FORMAT>                            # Output format: human, json, yaml (default: human)
  --show-metadata                              # Include execution metadata
  -q, --quiet                                  # Suppress command output, show only results

Security:
  Commands are validated for basic safety patterns. Dangerous commands
  like 'rm -rf /' are blocked by default. Directory access may be restricted
  based on configuration.
  
Timeouts:
  Default timeout is 5 minutes (300 seconds). Maximum timeout is 30 minutes
  (1800 seconds). Commands are terminated cleanly on timeout.

Exit codes:
  The shell command's exit code is returned when using human format.
  For JSON/YAML formats, the tool exit code reflects execution success.

Examples:
  # Basic command execution
  swissarmyhammer shell \"echo 'Hello, World!'\"
  
  # Execute in specific directory with timeout
  swissarmyhammer shell \"cargo test\" -C /project -t 600
  
  # Set environment variables
  swissarmyhammer shell \"echo \\$MY_VAR\" -e MY_VAR=value -e DEBUG=true
  
  # Show execution metadata
  swissarmyhammer shell \"uname -a\" --show-metadata
  
  # Quiet mode with JSON output for automation
  swissarmyhammer shell \"git status --porcelain\" -q --format json
  
  # Build with custom environment
  swissarmyhammer shell \"./build.sh\" -e RUST_LOG=debug -e BUILD_ENV=production -t 900
")]
    Shell {
        #[command(subcommand)]
        subcommand: ShellCommands,
    },
    /// Manage issue directory migration
    #[command(long_about = "
Manage issues directory migration from ./issues to .swissarmyhammer/issues.
Provides manual control over the migration process with safety features and reporting.

Basic usage:
  swissarmyhammer migrate status             # Show migration status
  swissarmyhammer migrate check              # Check migration prerequisites
  swissarmyhammer migrate run                # Perform migration with confirmation
  swissarmyhammer migrate cleanup            # Clean up migration artifacts

Migration options:
  --force                                    # Skip confirmation prompt
  --backup                                   # Create backup before migration
  --dry-run                                  # Preview migration without changes

Safety features:
- Confirmation prompts by default
- Optional backup creation
- Comprehensive pre-migration checks
- Detailed error messages and recovery guidance
- Cleanup command for managing backups

Examples:
  swissarmyhammer migrate status             # Check current status
  swissarmyhammer migrate check              # Validate prerequisites
  swissarmyhammer migrate run --backup       # Migrate with backup
  swissarmyhammer migrate run --dry-run      # Preview migration
  swissarmyhammer migrate run --force        # Non-interactive migration
  swissarmyhammer migrate cleanup            # Remove old backups
")]
    Migrate {
        #[command(subcommand)]
        subcommand: MigrateCommands,
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
  swissarmyhammer prompt list --search debug        # Search for prompts containing 'debug'
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

        /// Search prompts by name or description
        #[arg(long)]
        search: Option<String>,
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
    /// Search for prompts with advanced filtering and ranking
    #[command(long_about = "
Search for prompts using powerful full-text search with fuzzy matching.
Searches prompt names, titles, descriptions, content, and arguments.

Basic usage:
  swissarmyhammer prompt search \"code review\"        # Basic search
  swissarmyhammer prompt search \"debug.*error\" -r   # Regex search
  swissarmyhammer prompt search help --fuzzy          # Fuzzy matching

Search scope:
  --in name,description,content               # Search specific fields
  --source builtin                           # Search only builtin prompts
  --has-arg language                         # Find prompts with 'language' argument

Output options:
  --full                                     # Show complete prompt details
  --json                                     # JSON output for tooling
  --limit 10                                 # Limit number of results
  --highlight                                # Highlight matching terms

Examples:
  swissarmyhammer prompt search \"python code\"        # Find Python-related prompts
  swissarmyhammer prompt search \"review\" --full       # Detailed results for review prompts
  swissarmyhammer prompt search \".*test.*\" --regex     # Regex pattern matching
  swissarmyhammer prompt search help --fuzzy --limit 5  # Fuzzy search, max 5 results
")]
    Search {
        /// Search query
        query: String,

        /// Search in specific fields (name, title, description, content, arguments)
        #[arg(long, value_delimiter = ',')]
        r#in: Option<Vec<String>>,

        /// Use regular expressions
        #[arg(short, long)]
        regex: bool,

        /// Enable fuzzy matching for typo tolerance
        #[arg(short, long)]
        fuzzy: bool,

        /// Case-sensitive search
        #[arg(long)]
        case_sensitive: bool,

        /// Filter by source
        #[arg(long, value_enum)]
        source: Option<PromptSourceArg>,

        /// Find prompts with specific argument name
        #[arg(long)]
        has_arg: Option<String>,

        /// Find prompts without any arguments
        #[arg(long)]
        no_args: bool,

        /// Show complete prompt details
        #[arg(long)]
        full: bool,

        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,

        /// Highlight matching terms in output
        #[arg(long)]
        highlight: bool,

        /// Maximum number of results to show
        #[arg(short, long)]
        limit: Option<usize>,
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

        /// Test mode - execute with mocked actions and generate coverage report
        #[arg(long)]
        test: bool,

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

#[derive(Subcommand, Debug)]
pub enum IssueCommands {
    /// Create a new issue
    Create {
        /// Issue name (optional)
        #[arg()]
        name: Option<String>,
        /// Issue content (use - for stdin)
        #[arg(short, long)]
        content: Option<String>,
        /// Read content from file
        #[arg(short, long)]
        file: Option<std::path::PathBuf>,
    },
    /// List all issues
    List {
        /// Show completed issues
        #[arg(short, long)]
        completed: bool,
        /// Show active issues only
        #[arg(short, long)]
        active: bool,
        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
    /// Show issue details
    Show {
        /// Issue name
        name: String,
        /// Show raw content
        #[arg(short, long)]
        raw: bool,
    },
    /// Update an issue
    Update {
        /// Issue name
        name: String,
        /// New content (use - for stdin)
        #[arg(short, long)]
        content: Option<String>,
        /// Read content from file
        #[arg(short, long)]
        file: Option<std::path::PathBuf>,
        /// Append to existing content
        #[arg(short, long)]
        append: bool,
    },
    /// Mark issue as complete
    Complete {
        /// Issue name
        name: String,
    },
    /// Start working on an issue
    Work {
        /// Issue name
        name: String,
    },
    /// Merge completed issue
    Merge {
        /// Issue name
        name: String,
        /// Keep branch after merge
        #[arg(short, long)]
        keep_branch: bool,
    },
    /// Show current issue
    Current,
    /// Show project status
    Status,
    /// Show the next issue to work on
    Next,
}

#[derive(Subcommand, Debug)]
pub enum MemoCommands {
    /// Create a new memo
    Create {
        /// Memo title
        title: String,
        /// Memo content (use - for stdin)
        #[arg(short, long)]
        content: Option<String>,
    },
    /// List all memos
    List,
    /// Get a specific memo by ID
    Get {
        /// Memo ID (ULID)
        id: String,
    },
    /// Update a memo's content
    Update {
        /// Memo ID (ULID)
        id: String,
        /// New content (use - for stdin)
        #[arg(short, long)]
        content: Option<String>,
    },
    /// Delete a memo
    Delete {
        /// Memo ID (ULID)
        id: String,
    },
    /// Search memos by content and title
    Search {
        /// Search query
        query: String,
    },
    /// Get all memos as context for AI
    Context,
}

#[derive(Subcommand, Debug)]
pub enum FileCommands {
    /// Read file contents with optional offset and limit
    #[command(long_about = "
Read and return file contents from the local filesystem with support for various file types.

USAGE:
  swissarmyhammer file read <PATH>                     # Read entire file
  swissarmyhammer file read <PATH> --offset 10         # Start from line 10
  swissarmyhammer file read <PATH> --limit 50          # Read only first 50 lines
  swissarmyhammer file read <PATH> --offset 10 --limit 50  # Read lines 10-60

PARAMETERS:
  <PATH>        Absolute path to the file to read
  --offset      Starting line number for partial reading (1-based)
  --limit       Maximum number of lines to read

FUNCTIONALITY:
- Validates file path (must be absolute and within workspace)
- Supports text files, images, PDFs, and other file types
- Enables partial file reading via offset/limit for large files
- Provides error handling for missing or inaccessible files
- Respects workspace boundaries and ignore patterns

EXAMPLES:
  swissarmyhammer file read /path/to/file.txt
  swissarmyhammer file read ./config.json --offset 1 --limit 20
  swissarmyhammer file read /src/main.rs --limit 100
")]
    Read {
        /// Absolute path to file
        #[arg(help = "Absolute path to the file to read")]
        path: String,
        /// Starting line number
        #[arg(long, help = "Starting line number for partial reading")]
        offset: Option<usize>,
        /// Maximum lines to read
        #[arg(long, help = "Maximum number of lines to read")]
        limit: Option<usize>,
    },
    /// Write content to file (creates or overwrites)
    #[command(long_about = "
Create new files or completely overwrite existing files with specified content.

USAGE:
  swissarmyhammer file write <PATH> <CONTENT>          # Write content to file
  swissarmyhammer file write <PATH> -                  # Read content from stdin

PARAMETERS:
  <PATH>        Absolute path for the new or existing file
  <CONTENT>     Complete file content to write (use '-' for stdin)

FUNCTIONALITY:
- Creates new files with specified content
- Overwrites existing files completely
- Creates parent directories if they don't exist
- Sets appropriate file permissions
- Validates file path and content

EXAMPLES:
  swissarmyhammer file write /path/to/new-file.txt \"Hello World\"
  echo \"File content\" | swissarmyhammer file write /path/to/file.txt -
  swissarmyhammer file write ./config.json '{\"key\": \"value\"}'
")]
    Write {
        /// Absolute path for file
        #[arg(help = "Absolute path for the new or existing file")]
        path: String,
        /// File content (use - for stdin)
        #[arg(help = "Complete file content to write (use '-' for stdin)")]
        content: String,
    },
    /// Edit file with precise string replacement
    #[command(long_about = "
Perform precise string replacements in existing files with atomic operations.

USAGE:
  swissarmyhammer file edit <PATH> <OLD> <NEW>                 # Single replacement
  swissarmyhammer file edit <PATH> <OLD> <NEW> --replace-all   # Replace all occurrences

PARAMETERS:
  <PATH>        Absolute path to the file to modify
  <OLD>         Exact text to replace
  <NEW>         Replacement text
  --replace-all Replace all occurrences (default: false)

FUNCTIONALITY:
- Performs exact string matching and replacement
- Maintains file encoding and line endings
- Validates that old_string exists in the file
- Provides atomic operations (all or nothing replacement)
- Preserves file permissions and metadata

EXAMPLES:
  swissarmyhammer file edit /path/to/file.rs \"old_function\" \"new_function\"
  swissarmyhammer file edit ./config.toml \"debug = false\" \"debug = true\"
  swissarmyhammer file edit /src/lib.rs \"TODO\" \"DONE\" --replace-all
")]
    Edit {
        /// Absolute path to file
        #[arg(help = "Absolute path to the file to modify")]
        path: String,
        /// Text to replace
        #[arg(help = "Exact text to replace")]
        old_string: String,
        /// Replacement text
        #[arg(help = "Replacement text")]
        new_string: String,
        /// Replace all occurrences
        #[arg(long, help = "Replace all occurrences (default: single replacement)")]
        replace_all: bool,
    },
    /// Find files using glob patterns
    #[command(long_about = "
Fast file pattern matching with advanced filtering and sorting.

USAGE:
  swissarmyhammer file glob <PATTERN>                          # Basic glob matching
  swissarmyhammer file glob <PATTERN> --path /search/dir       # Search in specific directory
  swissarmyhammer file glob <PATTERN> --case-sensitive         # Case-sensitive matching
  swissarmyhammer file glob <PATTERN> --no-git-ignore          # Ignore .gitignore patterns

PARAMETERS:
  <PATTERN>           Glob pattern to match files (e.g., **/*.js, src/**/*.ts)
  --path              Directory to search within (optional)
  --case-sensitive    Case-sensitive matching (default: false)
  --respect-git-ignore Honor .gitignore patterns (default: true)

FUNCTIONALITY:
- Supports standard glob patterns with wildcards
- Returns file paths sorted by modification time (recent first)
- Searches across multiple workspace directories
- Respects git ignore patterns and workspace boundaries
- Provides fast pattern matching for large codebases

EXAMPLES:
  swissarmyhammer file glob \"**/*.rs\"
  swissarmyhammer file glob \"src/**/*.ts\" --path /project
  swissarmyhammer file glob \"*.json\" --case-sensitive
  swissarmyhammer file glob \"test_*.py\" --no-git-ignore
")]
    Glob {
        /// Glob pattern
        #[arg(help = "Glob pattern to match files (e.g., **/*.js, src/**/*.ts)")]
        pattern: String,
        /// Search directory
        #[arg(long, help = "Directory to search within (optional)")]
        path: Option<String>,
        /// Case sensitive matching
        #[arg(long, help = "Case-sensitive matching (default: false)")]
        case_sensitive: bool,
        /// Respect .gitignore
        #[arg(long = "no-git-ignore", help = "Ignore .gitignore patterns")]
        no_git_ignore: bool,
    },
    /// Search file contents using ripgrep
    #[command(long_about = "
Content-based search using ripgrep for fast and flexible text searching.

USAGE:
  swissarmyhammer file grep <PATTERN>                          # Basic text search
  swissarmyhammer file grep <PATTERN> --path /search/dir       # Search in specific location
  swissarmyhammer file grep <PATTERN> --glob \"*.rs\"           # Filter by file pattern
  swissarmyhammer file grep <PATTERN> --type rust              # Filter by file type
  swissarmyhammer file grep <PATTERN> -i                       # Case-insensitive search
  swissarmyhammer file grep <PATTERN> -C 3                     # Show 3 context lines

PARAMETERS:
  <PATTERN>         Regular expression pattern to search
  --path            File or directory to search in (optional)
  --glob            Glob pattern to filter files (e.g., *.js)
  --type            File type filter (e.g., js, py, rust)
  -i, --case-insensitive  Case-insensitive search
  -C, --context     Number of context lines around matches
  --output-mode     Output format (content, files_with_matches, count)

FUNCTIONALITY:
- Leverages ripgrep for high-performance text search
- Supports full regular expression syntax
- Provides file type and glob filtering
- Returns contextual information around matches
- Handles large codebases efficiently

EXAMPLES:
  swissarmyhammer file grep \"function.*async\"
  swissarmyhammer file grep \"TODO\" --path ./src --glob \"*.rs\"
  swissarmyhammer file grep \"error\" --type rust -C 2
  swissarmyhammer file grep \"import.*React\" --case-insensitive
  swissarmyhammer file grep \"test\" --output-mode files_with_matches
")]
    Grep {
        /// Search pattern
        #[arg(help = "Regular expression pattern to search for in file contents")]
        pattern: String,
        /// File or directory to search
        #[arg(long, help = "File or directory to search in (optional)")]
        path: Option<String>,
        /// File glob filter
        #[arg(long, help = "Glob pattern to filter files (e.g., *.js)")]
        glob: Option<String>,
        /// File type filter
        #[arg(long = "type", help = "File type filter (e.g., js, py, rust)")]
        file_type: Option<String>,
        /// Case insensitive search
        #[arg(long, short = 'i', help = "Case-insensitive search")]
        case_insensitive: bool,
        /// Context lines
        #[arg(long, short = 'C', help = "Number of context lines around matches")]
        context_lines: Option<usize>,
        /// Output mode
        #[arg(long, help = "Output format (content, files_with_matches, count)")]
        output_mode: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum SearchCommands {
    /// Index files for semantic search
    Index {
        /// Glob patterns or files to index (supports both "**/*.rs" and expanded file lists)
        patterns: Vec<String>,
        /// Force re-indexing of all files
        #[arg(short, long)]
        force: bool,
    },
    /// Query indexed files semantically
    Query {
        /// Search query
        query: String,
        /// Number of results to return
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
}

#[derive(Subcommand, Debug)]
pub enum WebSearchCommands {
    /// Search the web using SearXNG
    Search {
        /// The search query string
        query: String,

        /// Search category
        #[arg(long, default_value = "general")]
        category: String,

        /// Number of results to return
        #[arg(long, default_value = "10")]
        results: usize,

        /// Search language code (e.g., "en", "fr", "en-US")
        #[arg(long, default_value = "en")]
        language: String,

        /// Whether to fetch content from result URLs
        #[arg(long, default_value = "true")]
        fetch_content: bool,

        /// Safe search level (0=off, 1=moderate, 2=strict)
        #[arg(long, default_value = "1")]
        safe_search: u8,

        /// Time range filter ("", "day", "week", "month", "year")
        #[arg(long, default_value = "")]
        time_range: String,

        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Display current configuration
    Show {
        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
    /// List all available variables
    Variables {
        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
        /// Show variable types and sources
        #[arg(short, long)]
        verbose: bool,
    },
    /// Test template rendering with configuration
    Test {
        /// Template file to test (optional - uses stdin if not provided)
        template: Option<String>,
        /// Template variables as key=value pairs (overrides config)
        #[arg(long = "var", value_name = "KEY=VALUE")]
        variables: Vec<String>,
        /// Show debug information
        #[arg(short, long)]
        debug: bool,
    },
    /// Show environment variable usage
    Env {
        /// Show only missing environment variables
        #[arg(short, long)]
        missing: bool,
        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
}

#[derive(Subcommand, Debug)]
pub enum ShellCommands {
    /// Execute a shell command
    Execute {
        /// Shell command to execute
        #[arg(value_name = "COMMAND")]
        command: String,

        /// Working directory for command execution
        #[arg(short = 'C', long = "directory", value_name = "DIR")]
        working_directory: Option<std::path::PathBuf>,

        /// Command timeout in seconds (default: 300, max: 1800)
        #[arg(
            short = 't',
            long = "timeout",
            value_name = "SECONDS",
            default_value = "300"
        )]
        timeout: u64,

        /// Set environment variables (KEY=VALUE format)
        #[arg(short = 'e', long = "env", value_name = "KEY=VALUE")]
        environment: Vec<String>,

        /// Output format
        #[arg(long, value_enum, default_value = "human")]
        format: ShellOutputFormat,

        /// Show execution metadata
        #[arg(long)]
        show_metadata: bool,

        /// Quiet mode - suppress command output, show only results
        #[arg(short = 'q', long)]
        quiet: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum MigrateCommands {
    /// Show migration status and preview
    #[command(long_about = "
Shows current migration status including:
- Whether migration is needed
- Source directory information (file count, size)
- Destination directory status
- Migration recommendations

Examples:
  swissarmyhammer migrate status           # Show current status
")]
    Status,

    /// Perform migration with confirmation
    #[command(long_about = "
Performs the issue directory migration from ./issues to .swissarmyhammer/issues.
Includes comprehensive safety features:

- User confirmation by default (use --force to skip)
- Optional backup creation with --backup
- Dry-run capability with --dry-run
- Detailed progress reporting
- Automatic rollback on failure

Examples:
  swissarmyhammer migrate run              # Interactive migration
  swissarmyhammer migrate run --backup     # Create backup first
  swissarmyhammer migrate run --dry-run    # Preview without changes
  swissarmyhammer migrate run --force      # Skip confirmation
")]
    Run {
        /// Skip confirmation prompt
        #[arg(long, help = "Skip confirmation prompt")]
        force: bool,

        /// Create backup before migration
        #[arg(long, help = "Create backup before migration")]
        backup: bool,

        /// Dry run - show what would be migrated
        #[arg(long, help = "Dry run - show what would be migrated")]
        dry_run: bool,
    },

    /// Check if migration is possible
    #[command(long_about = "
Validates migration prerequisites and reports potential issues:

- Source directory existence and permissions
- Destination directory availability
- Parent directory write permissions
- File system space and access
- Overall migration feasibility

Examples:
  swissarmyhammer migrate check            # Check prerequisites
")]
    Check,

    /// Clean up migration artifacts
    #[command(long_about = "
Manages migration backup artifacts:

- Lists existing backup directories
- Provides size and age information
- Interactive cleanup with confirmation
- Safe removal of old backups

Examples:
  swissarmyhammer migrate cleanup          # Interactive cleanup
")]
    Cleanup,

    /// Verify completed migration integrity
    #[command(long_about = "
Performs comprehensive validation of completed migration:

- File integrity verification (count, names, sizes)
- Content verification using checksums
- Directory structure preservation
- Metadata and permissions validation
- Detailed reporting of any issues found

This command requires both the migrated directory (.swissarmyhammer/issues) 
and a backup of the original directory for comparison.

Examples:
  swissarmyhammer migrate verify          # Full verification
")]
    Verify,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ShellOutputFormat {
    Human,
    Json,
    Yaml,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    #[allow(dead_code)]
    pub fn try_parse_from_args<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        <Self as Parser>::try_parse_from(args)
    }

    pub fn is_tty() -> bool {
        io::stdout().is_terminal()
    }

    pub fn should_use_color() -> bool {
        Self::is_tty() && std::env::var("NO_COLOR").is_err()
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
        assert!(matches!(cli.command, Some(Commands::Serve)));
    }

    #[test]
    fn test_cli_doctor_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "doctor"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.command, Some(Commands::Doctor { migration: _ })));
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
        assert!(matches!(cli.command, Some(Commands::Serve)));
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
    fn test_cli_search_subcommand_basic() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "prompt", "search", "code review"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Prompt { subcommand }) = cli.command {
            if let PromptSubcommand::Search {
                query,
                r#in,
                regex,
                fuzzy,
                case_sensitive,
                source,
                has_arg,
                no_args,
                full,
                format,
                highlight,
                limit,
            } = subcommand
            {
                assert_eq!(query, "code review");
                assert_eq!(r#in, None);
                assert!(!regex);
                assert!(!fuzzy);
                assert!(!case_sensitive);
                assert_eq!(source, None);
                assert_eq!(has_arg, None);
                assert!(!no_args);
                assert!(!full);
                assert!(matches!(format, OutputFormat::Table));
                assert!(!highlight);
                assert_eq!(limit, None);
            } else {
                unreachable!("Expected Search subcommand");
            }
        } else {
            unreachable!("Expected Prompt command");
        }
    }

    #[test]
    fn test_cli_search_subcommand_with_flags() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "prompt",
            "search",
            "debug.*error",
            "--regex",
            "--fuzzy",
            "--case-sensitive",
            "--source",
            "builtin",
            "--has-arg",
            "language",
            "--full",
            "--format",
            "json",
            "--highlight",
            "--limit",
            "5",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Prompt { subcommand }) = cli.command {
            if let PromptSubcommand::Search {
                query,
                r#in,
                regex,
                fuzzy,
                case_sensitive,
                source,
                has_arg,
                no_args,
                full,
                format,
                highlight,
                limit,
            } = subcommand
            {
                assert_eq!(query, "debug.*error");
                assert_eq!(r#in, None);
                assert!(regex);
                assert!(fuzzy);
                assert!(case_sensitive);
                assert!(matches!(source, Some(PromptSourceArg::Builtin)));
                assert_eq!(has_arg, Some("language".to_string()));
                assert!(!no_args);
                assert!(full);
                assert!(matches!(format, OutputFormat::Json));
                assert!(highlight);
                assert_eq!(limit, Some(5));
            } else {
                unreachable!("Expected Search subcommand");
            }
        } else {
            unreachable!("Expected Prompt command");
        }
    }

    #[test]
    fn test_cli_search_subcommand_with_fields() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "prompt",
            "search",
            "python",
            "--in",
            "name,description,content",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Prompt { subcommand }) = cli.command {
            if let PromptSubcommand::Search { query, r#in, .. } = subcommand {
                assert_eq!(query, "python");
                assert_eq!(
                    r#in,
                    Some(vec![
                        "name".to_string(),
                        "description".to_string(),
                        "content".to_string()
                    ])
                );
            } else {
                unreachable!("Expected Search subcommand");
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
                search,
            } = subcommand
            {
                assert!(matches!(format, OutputFormat::Table));
                assert!(!verbose);
                assert_eq!(source, None);
                assert_eq!(category, None);
                assert_eq!(search, None);
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
        }) = cli.command
        {
            assert!(!quiet);
            assert!(matches!(format, ValidateFormat::Text));
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
        }) = cli.command
        {
            assert!(quiet);
            assert!(matches!(format, ValidateFormat::Json));
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
    fn test_issue_create_with_name() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "issue",
            "create",
            "bug_fix",
            "--content",
            "Fix login bug",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Issue { subcommand }) = cli.command {
            if let IssueCommands::Create {
                name,
                content,
                file,
            } = subcommand
            {
                assert_eq!(name, Some("bug_fix".to_string()));
                assert_eq!(content, Some("Fix login bug".to_string()));
                assert_eq!(file, None);
            } else {
                unreachable!("Expected Create subcommand");
            }
        } else {
            unreachable!("Expected Issue command");
        }
    }

    #[test]
    fn test_issue_create_without_name() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "issue",
            "create",
            "--content",
            "Quick fix needed",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Issue { subcommand }) = cli.command {
            if let IssueCommands::Create {
                name,
                content,
                file,
            } = subcommand
            {
                assert_eq!(name, None);
                assert_eq!(content, Some("Quick fix needed".to_string()));
                assert_eq!(file, None);
            } else {
                unreachable!("Expected Create subcommand");
            }
        } else {
            unreachable!("Expected Issue command");
        }
    }

    #[test]
    fn test_issue_create_with_file() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "issue", "create", "--file", "issue.md"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Issue { subcommand }) = cli.command {
            if let IssueCommands::Create {
                name,
                content,
                file,
            } = subcommand
            {
                assert_eq!(name, None);
                assert_eq!(content, None);
                assert_eq!(file, Some(std::path::PathBuf::from("issue.md")));
            } else {
                unreachable!("Expected Create subcommand");
            }
        } else {
            unreachable!("Expected Issue command");
        }
    }

    #[test]
    fn test_issue_create_named_with_file() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "issue",
            "create",
            "feature_name",
            "--file",
            "feature.md",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Issue { subcommand }) = cli.command {
            if let IssueCommands::Create {
                name,
                content,
                file,
            } = subcommand
            {
                assert_eq!(name, Some("feature_name".to_string()));
                assert_eq!(content, None);
                assert_eq!(file, Some(std::path::PathBuf::from("feature.md")));
            } else {
                unreachable!("Expected Create subcommand");
            }
        } else {
            unreachable!("Expected Issue command");
        }
    }

    #[test]
    fn test_memo_create_basic() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "memo", "create", "Meeting Notes"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Memo { subcommand }) = cli.command {
            if let MemoCommands::Create { title, content } = subcommand {
                assert_eq!(title, "Meeting Notes");
                assert_eq!(content, None);
            } else {
                unreachable!("Expected Create subcommand");
            }
        } else {
            unreachable!("Expected Memo command");
        }
    }

    #[test]
    fn test_memo_create_with_content() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "memo",
            "create",
            "Task List",
            "--content",
            "1. Review code\n2. Write tests",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Memo { subcommand }) = cli.command {
            if let MemoCommands::Create { title, content } = subcommand {
                assert_eq!(title, "Task List");
                assert_eq!(content, Some("1. Review code\n2. Write tests".to_string()));
            } else {
                unreachable!("Expected Create subcommand");
            }
        } else {
            unreachable!("Expected Memo command");
        }
    }

    #[test]
    fn test_memo_list() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "memo", "list"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Memo { subcommand }) = cli.command {
            if let MemoCommands::List = subcommand {
                // Test passes
            } else {
                unreachable!("Expected List subcommand");
            }
        } else {
            unreachable!("Expected Memo command");
        }
    }

    #[test]
    fn test_memo_get() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "memo",
            "get",
            "01GX5Q2D1NPRZ3KXFW2H8V3A1Y",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Memo { subcommand }) = cli.command {
            if let MemoCommands::Get { id } = subcommand {
                assert_eq!(id, "01GX5Q2D1NPRZ3KXFW2H8V3A1Y");
            } else {
                unreachable!("Expected Get subcommand");
            }
        } else {
            unreachable!("Expected Memo command");
        }
    }

    #[test]
    fn test_memo_update() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "memo",
            "update",
            "01GX5Q2D1NPRZ3KXFW2H8V3A1Y",
            "--content",
            "Updated content",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Memo { subcommand }) = cli.command {
            if let MemoCommands::Update { id, content } = subcommand {
                assert_eq!(id, "01GX5Q2D1NPRZ3KXFW2H8V3A1Y");
                assert_eq!(content, Some("Updated content".to_string()));
            } else {
                unreachable!("Expected Update subcommand");
            }
        } else {
            unreachable!("Expected Memo command");
        }
    }

    #[test]
    fn test_memo_delete() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "memo",
            "delete",
            "01GX5Q2D1NPRZ3KXFW2H8V3A1Y",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Memo { subcommand }) = cli.command {
            if let MemoCommands::Delete { id } = subcommand {
                assert_eq!(id, "01GX5Q2D1NPRZ3KXFW2H8V3A1Y");
            } else {
                unreachable!("Expected Delete subcommand");
            }
        } else {
            unreachable!("Expected Memo command");
        }
    }

    #[test]
    fn test_memo_search() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "memo", "search", "meeting notes"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Memo { subcommand }) = cli.command {
            if let MemoCommands::Search { query } = subcommand {
                assert_eq!(query, "meeting notes");
            } else {
                unreachable!("Expected Search subcommand");
            }
        } else {
            unreachable!("Expected Memo command");
        }
    }

    #[test]
    fn test_memo_context() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "memo", "context"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Memo { subcommand }) = cli.command {
            if let MemoCommands::Context = subcommand {
                // Test passes
            } else {
                unreachable!("Expected Context subcommand");
            }
        } else {
            unreachable!("Expected Memo command");
        }
    }

    #[test]
    fn test_search_index_single_pattern() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "search", "index", "**/*.rs"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Search { subcommand }) = cli.command {
            if let SearchCommands::Index { patterns, force } = subcommand {
                assert_eq!(patterns, vec!["**/*.rs".to_string()]);
                assert!(!force);
            } else {
                unreachable!("Expected Index subcommand");
            }
        } else {
            unreachable!("Expected Search command");
        }
    }

    #[test]
    fn test_search_index_multiple_patterns() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "search",
            "index",
            "src/**/*.rs",
            "tests/**/*.rs",
            "benches/**/*.rs",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Search { subcommand }) = cli.command {
            if let SearchCommands::Index { patterns, force } = subcommand {
                assert_eq!(
                    patterns,
                    vec![
                        "src/**/*.rs".to_string(),
                        "tests/**/*.rs".to_string(),
                        "benches/**/*.rs".to_string()
                    ]
                );
                assert!(!force);
            } else {
                unreachable!("Expected Index subcommand");
            }
        } else {
            unreachable!("Expected Search command");
        }
    }

    #[test]
    fn test_search_index_with_force_flag() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "search", "index", "**/*.rs", "--force"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Search { subcommand }) = cli.command {
            if let SearchCommands::Index { patterns, force } = subcommand {
                assert_eq!(patterns, vec!["**/*.rs".to_string()]);
                assert!(force);
            } else {
                unreachable!("Expected Index subcommand");
            }
        } else {
            unreachable!("Expected Search command");
        }
    }

    #[test]
    fn test_search_query_command() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "search",
            "query",
            "error handling",
            "--limit",
            "5",
            "--format",
            "json",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Search { subcommand }) = cli.command {
            if let SearchCommands::Query {
                query,
                limit,
                format,
            } = subcommand
            {
                assert_eq!(query, "error handling");
                assert_eq!(limit, 5);
                assert!(matches!(format, OutputFormat::Json));
            } else {
                unreachable!("Expected Query subcommand");
            }
        } else {
            unreachable!("Expected Search command");
        }
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
}
