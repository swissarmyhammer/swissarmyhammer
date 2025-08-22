# Migrate File Commands to Dynamic Generation

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Remove the static `FileCommands` enum and transition file commands to dynamic generation, continuing the systematic migration of command categories.

## Technical Details

### Remove Static File Commands
Delete/replace in `swissarmyhammer-cli/src/cli.rs`:

```rust
// REMOVE this entire enum and related code
#[derive(Subcommand, Debug)]
pub enum FileCommands {
    Read { path: PathBuf, offset: Option<usize>, limit: Option<usize> },
    Write { path: PathBuf, content: String },
    Edit { path: PathBuf, old_string: String, new_string: String, replace_all: bool },
    Glob { pattern: String, path: Option<PathBuf>, case_sensitive: bool, respect_git_ignore: bool },
    Grep { pattern: String, path: Option<PathBuf>, glob: Option<String>, type_filter: Option<String>, case_insensitive: bool, context_lines: Option<usize>, output_mode: Option<String> },
}
```

### Update Main Commands Enum
Remove file from static commands in `Commands` enum:

```rust
pub enum Commands {
    // ... other static commands ...
    
    // REMOVE this line:
    // File { #[command(subcommand)] subcommand: FileCommands },
    
    // File commands now handled dynamically
}
```

### Update Command Handlers
Remove `swissarmyhammer-cli/src/file.rs` or update it for dynamic dispatch:

```rust
// OLD: Remove handle_file_command function that matches on FileCommands enum
// NEW: File commands routed through dynamic_execution.rs instead
```

### Parameter Mapping Complexity
File commands have complex parameter patterns:

**Path Parameters:**
- `file read <PATH>` → `files_read` with `absolute_path`
- `file write <PATH> <CONTENT>` → `files_write` with `file_path` and `content`
- `file edit <PATH> <OLD> <NEW>` → `files_edit` with `file_path`, `old_string`, `new_string`

**Optional Parameters:**
- `--offset` and `--limit` for read
- `--replace-all` for edit  
- `--case-sensitive`, `--respect-git-ignore` for glob
- `--case-insensitive`, `--context-lines`, `--output-mode` for grep

**Positional vs Named Arguments:**
Some file commands use positional arguments that need to map to named MCP parameters:
- `file read path.txt` → `{"absolute_path": "path.txt"}`
- `file edit file.txt "old" "new"` → `{"file_path": "file.txt", "old_string": "old", "new_string": "new"}`

### Schema Enhancements
May need to enhance schema converter to handle:
- Positional arguments mapping to named parameters
- Complex parameter combinations
- Path validation and conversion

### Integration Testing
Update tests for complex file operations:

```rust
#[test]
fn test_file_edit_dynamic() {
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["file", "edit", "test.txt", "old_text", "new_text", "--replace-all"])
        .output()
        .unwrap();
    
    assert!(output.status.success());
}
```

### Argument Mapping Verification
Ensure all file command arguments map correctly:
- `file read path.txt --offset 10 --limit 20`
- `file write path.txt "content"`  
- `file edit path.txt "old" "new" --replace-all`
- `file glob "**/*.rs" --case-sensitive`
- `file grep "pattern" --path ./src --glob "*.rs" --context-lines 2`

## Acceptance Criteria
- [ ] `FileCommands` enum completely removed
- [ ] Static file command handling removed
- [ ] Dynamic file commands appear in CLI help
- [ ] All file command arguments work correctly
- [ ] Positional argument mapping works correctly
- [ ] Complex parameter combinations handled properly
- [ ] File commands execute successfully via MCP tools
- [ ] Integration tests updated and passing
- [ ] Path validation and security maintained
- [ ] Error handling maintains quality
- [ ] No regression in file command functionality

## Implementation Notes
- File commands have the most complex parameter patterns
- Ensure positional arguments map correctly
- Maintain file security validation
- Test with various path types and edge cases
- Verify glob pattern handling works correctly