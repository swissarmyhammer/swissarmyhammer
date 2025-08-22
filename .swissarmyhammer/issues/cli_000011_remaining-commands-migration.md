# Migrate Remaining Command Categories to Dynamic Generation

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Complete the migration by transitioning the remaining command categories (WebSearch, Config, Shell, Migrate) to dynamic generation, finalizing the elimination of static command enums.

## Technical Details

### Remove Remaining Static Command Enums
Delete/replace in `swissarmyhammer-cli/src/cli.rs`:

```rust
// REMOVE these enums and related code:

#[derive(Subcommand, Debug)]
pub enum WebSearchCommands {
    Search { query: String, category: Option<String>, results: Option<usize>, language: Option<String>, fetch_content: Option<bool>, safe_search: Option<u8>, time_range: Option<String>, format: Option<OutputFormat> },
}

#[derive(Subcommand, Debug)]  
pub enum ConfigCommands {
    Show { format: Option<OutputFormat> },
    Variables { format: Option<OutputFormat> },
    Test { template_file: PathBuf },
    Env { missing: bool, format: Option<OutputFormat> },
}

#[derive(Subcommand, Debug)]
pub enum ShellCommands {
    Execute { command: String, directory: Option<PathBuf>, timeout: Option<u64>, env: Vec<String>, format: Option<OutputFormat>, show_metadata: bool, quiet: bool },
}

#[derive(Subcommand, Debug)]
pub enum MigrateCommands {
    Status,
    Check,
    Run { force: bool, backup: bool, dry_run: bool },
    Cleanup,
}
```

### Update Main Commands Enum
Remove all remaining categories from static commands:

```rust
pub enum Commands {
    // Static commands (preserve these)
    Serve,
    Doctor { migration: bool },
    Prompt { subcommand: PromptSubcommand },
    Flow { subcommand: FlowSubcommand },  
    Completion { shell: clap_complete::Shell },
    Validate { quiet: bool, format: ValidateFormat, workflow_dirs: Vec<String> },
    Plan { plan_filename: String },
    Implement,
    
    // REMOVE these lines - now handled dynamically:
    // WebSearch { #[command(subcommand)] subcommand: WebSearchCommands },
    // Config { #[command(subcommand)] subcommand: ConfigCommands },
    // Shell { #[command(subcommand)] subcommand: ShellCommands },
    // Migrate { #[command(subcommand)] subcommand: MigrateCommands },
}
```

### Command Handler Updates
Remove or update corresponding command handlers:
- `swissarmyhammer-cli/src/web_search.rs`
- `swissarmyhammer-cli/src/config.rs`
- `swissarmyhammer-cli/src/shell.rs` 
- `swissarmyhammer-cli/src/migrate.rs`

### Special Cases Handling

**Web Search Commands:**
- `web-search search` → `web_search` tool
- Complex parameter mapping with optional fields
- Format parameter handling at CLI level

**Config Commands:**
Config commands may not have direct MCP tool equivalents - verify mapping:
- `config show` → May need new MCP tool or special handling
- `config variables` → May need new MCP tool
- `config test` → May need new MCP tool  
- `config env` → May need new MCP tool

**Shell Commands:**
- `shell execute` → `shell_execute` tool (rename from "execute" to avoid conflicts)
- Complex parameter mapping with environment variables

**Migrate Commands:**  
Migrate commands may not have MCP tool equivalents:
- May need to create MCP tools for migrate operations
- Or handle as special case in CLI

### MCP Tool Verification
Verify all remaining categories have corresponding MCP tools:

**Need MCP Tools (if missing):**
- Config management tools
- Migration management tools

**Existing MCP Tools:**
- `web_search` → "web-search" category
- `shell_execute` → "shell" category

### Complex Parameter Mapping
Handle complex parameter patterns:

**Environment Variables (shell commands):**
- `--env KEY=VALUE` → array of environment variable strings
- Multiple `--env` flags → collected into array

**Optional Complex Parameters:**
- Web search has many optional parameters
- Shell commands have timeout, environment, directory parameters

### Integration Testing
Update remaining integration tests:

```rust
#[test]
fn test_web_search_dynamic() {
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()  
        .args(["web-search", "search", "rust async", "--results", "5"])
        .output()
        .unwrap();
    
    assert!(output.status.success());
}
```

## Acceptance Criteria
- [ ] All remaining static command enums removed
- [ ] Dynamic generation handles all remaining categories
- [ ] Web search commands work correctly
- [ ] Config commands work correctly (or alternative solution)
- [ ] Shell commands work correctly
- [ ] Migrate commands work correctly (or alternative solution)
- [ ] Complex parameter mapping functions properly
- [ ] All integration tests updated and passing
- [ ] No static command enums remain in codebase
- [ ] CLI help generation includes all migrated commands

## Implementation Notes
- Verify all categories have corresponding MCP tools
- Create missing MCP tools if needed
- Handle special cases for commands without MCP equivalents
- Test complex parameter scenarios thoroughly  
- This completes the core migration objective