# Integrate Dynamic CLI System into Main Application

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective

Replace the existing static CLI command structure with the new dynamic CLI builder system, eliminating redundant command enums while preserving all functionality.

## Implementation Tasks

### 1. Update Main CLI Module

Replace `swissarmyhammer-cli/src/cli.rs` with dynamic CLI integration:

```rust
use clap::{Parser, ValueEnum};
use is_terminal::IsTerminal;
use std::io;
use crate::cli_builder::CliBuilder;
use swissarmyhammer_tools::mcp::tool_registry::create_tool_registry;

#[derive(ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
}

// Keep existing ValueEnum types for static commands
#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum PromptSourceArg {
    Builtin,
    User,
    Local,
    Dynamic,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ValidateFormat {
    Text,
    Json,
}

// Main CLI structure - now much simpler
#[derive(Parser, Debug)]
#[command(name = "swissarmyhammer")]
#[command(version)]
#[command(about = "An MCP server for managing prompts, workflows, issues, memos, and development tools")]
#[command(long_about = "
swissarmyhammer is an MCP (Model Context Protocol) server that manages
prompts, workflows, issues, memos, and development tools. It supports file watching, 
template substitution, and seamless integration with Claude Code.

Commands are dynamically generated from MCP tools for consistent functionality.
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

// Simplified Commands enum - no more MCP tool duplicates
#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Run as MCP server (default when invoked via stdio)
    Serve,
    
    /// Diagnose configuration and setup issues
    Doctor,
    
    /// Manage and test prompts
    Prompt {
        #[command(subcommand)]
        subcommand: PromptSubcommand,
    },
    
    /// Execute and manage workflows
    Flow {
        #[command(subcommand)]
        subcommand: FlowSubcommand,
    },
    
    /// Generate shell completion scripts
    Completion {
        /// Shell to generate completion for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    
    /// Validate prompt files and workflows
    Validate {
        /// Suppress all output except errors
        #[arg(short, long)]
        quiet: bool,
        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        format: ValidateFormat,
        /// \[DEPRECATED\] This parameter is ignored
        #[arg(long = "workflow-dir", value_name = "DIR", hide = true)]
        workflow_dirs: Vec<String>,
    },
    
    /// Plan a specific specification file
    Plan {
        /// Path to the plan file to process
        plan_filename: String,
    },
    
    /// Execute the implement workflow
    Implement,
}

// Keep existing subcommand enums for static commands
#[derive(clap::Subcommand, Debug)]
pub enum PromptSubcommand {
    /// List available prompts
    List {
        #[arg(short, long, value_enum)]
        source: Option<PromptSourceArg>,
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
    /// Test prompt rendering
    Test {
        /// Prompt name
        name: String,
        /// Template variables as key=value pairs
        #[arg(long = "var", value_name = "KEY=VALUE")]
        variables: Vec<String>,
        #[arg(short, long, value_enum)]
        source: Option<PromptSourceArg>,
    },
    // ... other existing prompt subcommands
}

#[derive(clap::Subcommand, Debug)]
pub enum FlowSubcommand {
    /// Execute workflow
    Run {
        /// Workflow name
        workflow: String,
        /// Initial variables as key=value pairs
        #[arg(long = "var", value_name = "KEY=VALUE")]
        variables: Vec<String>,
        /// Interactive step-by-step execution
        #[arg(short, long)]
        interactive: bool,
    },
    // ... other existing flow subcommands  
}

// New dynamic CLI building function
pub async fn build_dynamic_cli() -> anyhow::Result<clap::Command> {
    let tool_registry = std::sync::Arc::new(create_tool_registry().await?);
    let cli_builder = CliBuilder::new(tool_registry);
    cli_builder.build_cli()
}
```

### 2. Update Main Application Entry Point

Update `swissarmyhammer-cli/src/main.rs`:

```rust
use clap::Parser;
use anyhow::Result;
use swissarmyhammer_cli::{Cli, Commands, build_dynamic_cli};
use swissarmyhammer_cli::dynamic_execution::{DynamicCommandExecutor, is_dynamic_command, is_static_command};
use swissarmyhammer_cli::cli_builder::CliBuilder;
use swissarmyhammer_tools::mcp::tool_registry::create_tool_registry;

#[tokio::main]
async fn main() -> Result<()> {
    // Build dynamic CLI
    let cli_app = build_dynamic_cli().await?;
    
    // Parse arguments with dynamic CLI
    let matches = cli_app.try_get_matches()?;
    
    // Initialize global flags and logging
    setup_logging(&matches)?;
    
    // Initialize MCP infrastructure for dynamic commands
    let tool_registry = std::sync::Arc::new(create_tool_registry().await?);
    let tool_context = std::sync::Arc::new(create_tool_context().await?);
    let cli_builder = CliBuilder::new(tool_registry.clone());
    
    // Route based on command type
    if is_static_command(&matches) {
        handle_static_command(&matches).await?;
    } else if is_dynamic_command(&matches, &cli_builder) {
        let command_info = cli_builder.extract_command_info(&matches)
            .ok_or_else(|| anyhow::anyhow!("Failed to extract dynamic command info"))?;
            
        let executor = DynamicCommandExecutor::new(tool_registry, tool_context);
        executor.execute_command(command_info, &matches).await?;
    } else {
        // Show help if no command provided
        cli_app.print_help()?;
        std::process::exit(1);
    }
    
    Ok(())
}

async fn handle_static_command(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("serve", _)) => {
            crate::serve::handle_serve().await
        },
        Some(("doctor", sub_matches)) => {
            crate::doctor::handle_doctor(sub_matches).await
        },
        Some(("prompt", sub_matches)) => {
            crate::prompt::handle_prompt(sub_matches).await
        },
        Some(("flow", sub_matches)) => {
            crate::flow::handle_flow(sub_matches).await
        },
        Some(("completion", sub_matches)) => {
            crate::completions::handle_completion(sub_matches).await
        },
        Some(("validate", sub_matches)) => {
            crate::validate::handle_validate(sub_matches).await
        },
        Some(("plan", sub_matches)) => {
            crate::plan::handle_plan(sub_matches).await
        },
        Some(("implement", _)) => {
            crate::implement::handle_implement().await
        },
        _ => {
            anyhow::bail!("Unknown static command")
        }
    }
}

fn setup_logging(matches: &clap::ArgMatches) -> Result<()> {
    // Extract global flags from matches
    let verbose = matches.get_flag("verbose");
    let debug = matches.get_flag("debug"); 
    let quiet = matches.get_flag("quiet");
    
    // Initialize logging based on flags
    crate::logging::init_logging(verbose, debug, quiet)
}

async fn create_tool_context() -> Result<swissarmyhammer_tools::mcp::tool_registry::ToolContext> {
    // Create tool context with necessary dependencies
    // This would initialize storage backends, git operations, etc.
    todo!("Implement tool context creation")
}
```

### 3. Remove Redundant Command Enums

Delete the redundant command enums from `cli.rs`:

```rust
// DELETE these enums - they're now replaced by dynamic commands
// pub enum IssueCommands { ... }    // ~70 lines
// pub enum MemoCommands { ... }     // ~40 lines  
// pub enum FileCommands { ... }     // ~180 lines
// pub enum SearchCommands { ... }   // ~25 lines
// pub enum WebSearchCommands { ... } // ~40 lines
// pub enum ConfigCommands { ... }   // ~40 lines
// pub enum ShellCommands { ... }    // ~30 lines

// Total removal: ~425+ lines of redundant CLI definitions
```

### 4. Update Command Handler Modules

Remove or update the MCP-based command handlers since they're now handled by DynamicCommandExecutor:

```rust
// Update swissarmyhammer-cli/src/lib.rs to remove redundant modules
// Keep only:
pub mod cli;
pub mod cli_builder;
pub mod schema_conversion;
pub mod dynamic_execution;
pub mod response_formatting;

// Static command handlers (keep these):
pub mod serve;
pub mod doctor;
pub mod prompt;
pub mod flow;
pub mod completions;
pub mod validate;

// MCP command handlers (can be removed):
// pub mod issue;     // DELETE - now handled dynamically
// pub mod memo;      // DELETE - now handled dynamically  
// pub mod file;      // DELETE - now handled dynamically
// pub mod search;    // DELETE - now handled dynamically
// pub mod web_search; // DELETE - now handled dynamically
// pub mod config;    // DELETE - now handled dynamically
// pub mod shell;     // DELETE - now handled dynamically
```

### 5. Update Help Generation Test

Create `swissarmyhammer-cli/tests/dynamic_cli_integration_test.rs`:

```rust
#[tokio::test]
async fn test_dynamic_cli_generation() {
    let cli = swissarmyhammer_cli::build_dynamic_cli().await.unwrap();
    
    // Verify static commands preserved
    assert!(cli.find_subcommand("serve").is_some());
    assert!(cli.find_subcommand("doctor").is_some());
    assert!(cli.find_subcommand("prompt").is_some());
    
    // Verify dynamic commands generated
    assert!(cli.find_subcommand("issue").is_some());
    assert!(cli.find_subcommand("memo").is_some());
    assert!(cli.find_subcommand("file").is_some());
    
    // Verify subcommands within categories
    let issue_cmd = cli.find_subcommand("issue").unwrap();
    assert!(issue_cmd.find_subcommand("create").is_some());
    assert!(issue_cmd.find_subcommand("list").is_some());
    assert!(issue_cmd.find_subcommand("show").is_some());
}

#[tokio::test]
async fn test_help_generation_quality() {
    let cli = swissarmyhammer_cli::build_dynamic_cli().await.unwrap();
    
    // Test that help text is generated
    let issue_cmd = cli.find_subcommand("issue").unwrap();
    assert!(issue_cmd.get_about().is_some());
    
    let create_cmd = issue_cmd.find_subcommand("create").unwrap();
    assert!(create_cmd.get_about().is_some());
}

#[tokio::test] 
async fn test_argument_generation() {
    let cli = swissarmyhammer_cli::build_dynamic_cli().await.unwrap();
    
    // Test that arguments are generated from schemas
    let issue_create = cli.find_subcommand("issue")
        .unwrap()
        .find_subcommand("create")
        .unwrap();
        
    // Should have arguments based on issue_create MCP tool schema
    assert!(issue_create.get_arguments().count() > 0);
}
```

### 6. Update Integration Tests

Update existing CLI integration tests to use dynamic system:

```rust
// In swissarmyhammer-cli/tests/cli_integration_test.rs
#[tokio::test]
async fn test_issue_create_command() {
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
        .args(["issue", "create", "--name", "test-issue", "--content", "Test content"])
        .output()
        .await
        .unwrap();
        
    assert!(output.status.success());
}

#[tokio::test]
async fn test_memo_list_command() {
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
        .args(["memo", "list"])
        .output()
        .await
        .unwrap();
        
    assert!(output.status.success());
}
```

## Success Criteria

- [ ] Dynamic CLI builder integrated into main application
- [ ] Static commands (serve, doctor, prompt, flow, etc.) work unchanged
- [ ] Dynamic commands generated from MCP tools (issue, memo, file, etc.)
- [ ] Redundant command enums removed (~425+ lines deleted)
- [ ] Help text generated correctly for all commands
- [ ] Command execution works for both static and dynamic commands
- [ ] Integration tests pass with new dynamic system
- [ ] Shell completion works with dynamic commands
- [ ] All existing CLI functionality preserved

## Architecture Notes

- Complete replacement of static MCP command definitions
- Major code reduction while maintaining functionality
- Foundation for automatic CLI updates when MCP tools change
- Preserves backward compatibility with existing CLI usage patterns
- Enables future tools to appear automatically in CLI