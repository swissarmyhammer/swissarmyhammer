# Migrate Memo Commands to Dynamic Generation

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Remove the static `MemoCommands` enum and transition memo commands to dynamic generation as the first category migration, proving the system works end-to-end.

## Technical Details

### Remove Static Memo Commands
Delete/replace in `swissarmyhammer-cli/src/cli.rs`:

```rust
// REMOVE this entire enum and related code
#[derive(Subcommand, Debug)]
pub enum MemoCommands {
    Create { title: String, content: Option<String>, file: Option<PathBuf> },
    List { format: Option<OutputFormat> },
    Get { id: String },
    Update { id: String, content: String },
    Delete { id: String },
    Search { query: String },
    Context,
}
```

### Update Main Commands Enum
Remove memo from static commands in `Commands` enum:

```rust
pub enum Commands {
    // ... other static commands ...
    
    // REMOVE this line:
    // Memo { #[command(subcommand)] subcommand: MemoCommands },
    
    // Memo commands now handled dynamically
}
```

### Update Command Handlers
Remove `swissarmyhammer-cli/src/memo.rs` or update it to handle dynamic dispatch only:

```rust
// OLD: Remove handle_memo_command function that matches on MemoCommands enum
// NEW: Memo commands routed through dynamic_execution.rs instead
```

### Verify Dynamic Generation
Ensure memo commands appear correctly in CLI:

```bash
sah memo --help                    # Should list: create, list, get, update, delete, search, context
sah memo create --help             # Should show title and content parameters
sah memo list --help               # Should show format options
```

### Integration Testing
Update tests to use dynamic commands:

```rust
// Update tests in swissarmyhammer-cli/tests/
#[test]
fn test_memo_create_dynamic() {
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["memo", "create", "--title", "Test", "--content", "Test content"])
        .output()
        .unwrap();
    
    assert!(output.status.success());
}
```

### Argument Mapping Verification
Ensure all memo command arguments map correctly:
- `memo create --title "Title" --content "Content"` 
- `memo list --format json`
- `memo get --id "ULID"`
- `memo update --id "ULID" --content "New content"`
- `memo delete --id "ULID"`
- `memo search --query "search terms"`  
- `memo context` (no arguments)

### Error Handling
Ensure dynamic memo commands provide same error messages as static versions.

## Acceptance Criteria
- [ ] `MemoCommands` enum completely removed
- [ ] Static memo command handling removed  
- [ ] Dynamic memo commands appear in CLI help
- [ ] All memo command arguments work correctly
- [ ] Memo commands execute successfully via MCP tools
- [ ] Integration tests updated and passing
- [ ] Error handling maintains quality
- [ ] No regression in memo command functionality
- [ ] Help text quality matches or exceeds previous version

## Implementation Notes
- Test thoroughly before removing static enum
- Ensure argument mappings are exactly equivalent
- Verify help text generation meets quality standards
- Update any documentation or examples
- This is the proof-of-concept for the entire migration