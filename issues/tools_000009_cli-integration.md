# CLI Integration for File Tools

Refer to /Users/wballard/github/sah-filetools/ideas/tools.md

## Objective
Create CLI commands for file editing operations integrated with the MCP tool system.

## Tasks
- [ ] Create `file.rs` module in `swissarmyhammer-cli/src/`
- [ ] Implement `FileCommands` enum with subcommands for each tool
- [ ] Add file command group to main CLI structure
- [ ] Implement command handlers for each file operation
- [ ] Add comprehensive help text and usage examples
- [ ] Implement error handling and output formatting
- [ ] Add CLI integration tests

## CLI Structure
```rust
// In swissarmyhammer-cli/src/file.rs
#[derive(Parser)]
pub enum FileCommands {
    Read {
        #[arg(help = "Absolute path to file")]
        path: String,
        #[arg(long, help = "Starting line number")]
        offset: Option<usize>,
        #[arg(long, help = "Maximum lines to read")]
        limit: Option<usize>,
    },
    Write {
        #[arg(help = "Absolute path for file")]
        path: String,
        #[arg(help = "File content")]
        content: String,
    },
    Edit {
        #[arg(help = "Absolute path to file")]
        path: String,
        #[arg(help = "Text to replace")]
        old_string: String,
        #[arg(help = "Replacement text")]
        new_string: String,
        #[arg(long, help = "Replace all occurrences")]
        replace_all: bool,
    },
    Glob {
        #[arg(help = "Glob pattern")]
        pattern: String,
        #[arg(long, help = "Search directory")]
        path: Option<String>,
        #[arg(long, help = "Case sensitive matching")]
        case_sensitive: bool,
        #[arg(long, help = "Respect .gitignore")]
        respect_git_ignore: bool,
    },
    Grep {
        #[arg(help = "Search pattern")]
        pattern: String,
        #[arg(long, help = "File or directory to search")]
        path: Option<String>,
        #[arg(long, help = "File glob filter")]
        glob: Option<String>,
        #[arg(long, help = "File type filter")]
        file_type: Option<String>,
        #[arg(long, short = 'i', help = "Case insensitive")]
        case_insensitive: bool,
        #[arg(long, short = 'C', help = "Context lines")]
        context_lines: Option<usize>,
        #[arg(long, help = "Output mode")]
        output_mode: Option<String>,
    },
}
```

## Command Integration
- [ ] Add `File { subcommand: FileCommands }` to main CLI Commands enum
- [ ] Implement `handle_file_command` function using MCP integration patterns
- [ ] Add file command to CLI dispatcher in main.rs
- [ ] Follow established patterns from other command modules

## Help Text and Documentation
- [ ] Comprehensive help text for each subcommand
- [ ] Usage examples in help output
- [ ] Clear parameter descriptions
- [ ] Error message guidance for common issues

## Output Formatting
- [ ] Consistent output formatting across all file commands
- [ ] JSON output option for machine-readable results
- [ ] Appropriate error display for file operation failures
- [ ] Progress indicators for long-running operations

## Testing Requirements
- [ ] Unit tests for CLI argument parsing
- [ ] Integration tests with MCP tools
- [ ] Error handling tests for invalid parameters
- [ ] Output formatting tests
- [ ] End-to-end CLI tests with temporary files
- [ ] Help text validation tests

## Acceptance Criteria
- [ ] All file operations accessible via CLI commands
- [ ] Comprehensive help text and usage examples
- [ ] Proper integration with MCP tool system
- [ ] Consistent error handling and output formatting
- [ ] Complete test coverage for CLI functionality
- [ ] Integration with existing CLI patterns and conventions