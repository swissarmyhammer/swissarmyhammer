# Migrate Issue Commands to Dynamic Generation

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Remove the static `IssueCommands` enum and transition issue commands to dynamic generation, building on the success of memo command migration.

## Technical Details

### Remove Static Issue Commands
Delete/replace in `swissarmyhammer-cli/src/cli.rs`:

```rust
// REMOVE this entire enum and related code
#[derive(Subcommand, Debug)]
pub enum IssueCommands {
    Create { name: Option<String>, content: Option<String>, file: Option<PathBuf> },
    List { format: Option<OutputFormat>, show_completed: bool, show_active: bool },
    Show { name: String, raw: bool },
    Update { name: String, content: String, append: bool },
    Complete { name: String },
    Work { name: String },
    Merge { name: String, delete_branch: bool },
    Current,
    Next,
    Status,
}
```

### Update Main Commands Enum
Remove issue from static commands in `Commands` enum:

```rust
pub enum Commands {
    // ... other static commands ...
    
    // REMOVE this line:
    // Issue { #[command(subcommand)] subcommand: IssueCommands },
    
    // Issue commands now handled dynamically
}
```

### Update Command Handlers
Remove `swissarmyhammer-cli/src/issue.rs` or update it for dynamic dispatch:

```rust
// OLD: Remove handle_issue_command function that matches on IssueCommands enum  
// NEW: Issue commands routed through dynamic_execution.rs instead
```

### Special Command Handling
Issue commands have some special cases to handle:

**Argument Mapping:**
- `issue show current` → `issue_show` with `name: "current"`
- `issue show next` → `issue_show` with `name: "next"`  
- `issue status` → `issue_all_complete` (no args)
- `issue complete --name "issue"` → `issue_mark_complete`

**Command Name Aliases:**
- "complete" command → calls `issue_mark_complete` tool
- "status" command → calls `issue_all_complete` tool

### Integration Testing  
Update tests to use dynamic commands:

```rust
#[test]
fn test_issue_create_dynamic() {
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["issue", "create", "--name", "test-issue", "--content", "Test issue"])
        .output()
        .unwrap();
    
    assert!(output.status.success());
}
```

### Argument Mapping Verification
Ensure all issue command arguments map correctly:
- `issue create --name "name" --content "content"`
- `issue list --format json --show-completed --show-active`  
- `issue show --name "issue-name" --raw`
- `issue update --name "issue-name" --content "content" --append`
- `issue complete --name "issue-name"`
- `issue work --name "issue-name"`
- `issue merge --name "issue-name" --delete-branch`
- `issue show current`
- `issue show next`  
- `issue status`

## Acceptance Criteria
- [ ] `IssueCommands` enum completely removed
- [ ] Static issue command handling removed
- [ ] Dynamic issue commands appear in CLI help
- [ ] All issue command arguments work correctly
- [ ] Special commands (current, next, status) work correctly
- [ ] Issue commands execute successfully via MCP tools
- [ ] Integration tests updated and passing
- [ ] Error handling maintains quality
- [ ] No regression in issue command functionality
- [ ] Help text quality matches or exceeds previous version

## Implementation Notes
- Handle special command mappings carefully
- Ensure Git repository requirements still enforced
- Verify branch operations work correctly
- Test all issue workflow scenarios
- Pay attention to boolean flag handling

## Proposed Solution

Based on my analysis of the codebase and the successful memo command migration, here's my approach:

### 1. Understanding the Pattern
The memo command migration shows how static CLI commands are replaced with dynamic generation:
- Issue tools (like `issue_create`, `issue_list`, etc.) are already implemented as MCP tools
- The `ToolRegistry::cli_category()` method already maps issue tools to the "issue" category
- The `CliBuilder` in `dynamic_cli.rs` generates commands from the tool registry automatically
- Dynamic execution happens via `handle_dynamic_command()` in `dynamic_execution.rs`

### 2. Migration Steps

1. **Remove Static Enums**: Delete `IssueCommands` enum from `cli.rs`
2. **Remove Static Command**: Remove `Issue { subcommand: IssueCommands }` from main `Commands` enum 
3. **Remove Handler**: Remove `run_issue()` function and `issue.rs` module from `main.rs`
4. **No Changes Needed**: The tool registry already maps issue tools correctly, and dynamic CLI generation will automatically pick them up

### 3. Special Commands Handling
The existing MCP tools already handle the special commands:
- `issue show current` → `issue_show` with `name: "current"`
- `issue show next` → `issue_show` with `name: "next"` 
- `issue status` → `issue_all_complete` (no args)
- Other commands map directly with argument preservation

### 4. Testing Strategy
- Build and test basic issue commands work: `issue create`, `issue list`, `issue show`
- Test special commands: `issue show current`, `issue show next`, `issue status`
- Verify argument mapping works correctly for all commands
- Ensure Git repository requirements are still enforced

This approach follows the exact pattern used for memo commands and should work seamlessly since the MCP tools already exist.