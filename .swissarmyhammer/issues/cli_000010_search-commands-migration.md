# Migrate Search Commands to Dynamic Generation

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Remove the static `SearchCommands` enum and transition search commands to dynamic generation, continuing the systematic migration process.

## Technical Details

### Remove Static Search Commands
Delete/replace in `swissarmyhammer-cli/src/cli.rs`:

```rust
// REMOVE this entire enum and related code
#[derive(Subcommand, Debug)]
pub enum SearchCommands {
    Index { patterns: Vec<String>, force: bool },
    Query { query: String, limit: Option<usize>, format: Option<OutputFormat> },
}
```

### Update Main Commands Enum
Remove search from static commands in `Commands` enum:

```rust
pub enum Commands {
    // ... other static commands ...
    
    // REMOVE this line:
    // Search { #[command(subcommand)] subcommand: SearchCommands },
    
    // Search commands now handled dynamically
}
```

### Update Command Handlers
Remove `swissarmyhammer-cli/src/search.rs` or update it for dynamic dispatch:

```rust
// OLD: Remove handle_search_command function that matches on SearchCommands enum
// NEW: Search commands routed through dynamic_execution.rs instead
```

### Array Parameter Handling
Search commands use array parameters that need special handling:

**Index Command:**
- `search index "**/*.rs" "**/*.py"` → `{"patterns": ["**/*.rs", "**/*.py"]}`
- `search index file1.rs file2.rs file3.rs` → `{"patterns": ["file1.rs", "file2.rs", "file3.rs"]}`
- `--force` flag → `{"force": true}`

**Query Command:**
- `search query "error handling"` → `{"query": "error handling"}`  
- `--limit 5` → `{"limit": 5}`
- `--format json` → output format handled by CLI, not passed to MCP

### Schema Converter Enhancement
Enhance schema converter to handle:
- Array types with `ArgAction::Append`
- Multiple string arguments collected into array
- Optional parameters with default values

```rust
// In schema_conversion.rs
fn handle_array_parameter(matches: &ArgMatches, param_name: &str) -> Option<Value> {
    if let Some(values) = matches.get_many::<String>(param_name) {
        let array: Vec<Value> = values.map(|v| Value::String(v.clone())).collect();
        Some(Value::Array(array))
    } else {
        None
    }
}
```

### Git Repository Requirements
Search commands require Git repository:
- Ensure error handling for non-Git directories
- Maintain database path resolution (`.swissarmyhammer/semantic.db`)
- Preserve search isolation per repository

### Integration Testing
Update search-specific tests:

```rust
#[test]
fn test_search_index_dynamic() {
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["search", "index", "**/*.rs", "--force"])
        .output()
        .unwrap();
    
    assert!(output.status.success());
}

#[test]
fn test_search_query_dynamic() {
    // First index some files
    let _index = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["search", "index", "test.rs"])
        .output()
        .unwrap();
        
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["search", "query", "error handling", "--limit", "5"])
        .output()
        .unwrap();
    
    assert!(output.status.success());
}
```

### Argument Mapping Verification
Ensure all search command arguments map correctly:
- `search index "**/*.rs" "**/*.py" --force`
- `search index file1.rs file2.rs file3.rs`
- `search query "error handling" --limit 10`
- Format parameter handled at CLI level, not passed to MCP

## Acceptance Criteria
- [ ] `SearchCommands` enum completely removed
- [ ] Static search command handling removed
- [ ] Dynamic search commands appear in CLI help
- [ ] Array parameter handling works correctly
- [ ] Multiple file patterns supported
- [ ] Search commands execute successfully via MCP tools
- [ ] Git repository validation maintained
- [ ] Integration tests updated and passing
- [ ] Semantic search functionality preserved
- [ ] Error handling maintains quality
- [ ] No regression in search command functionality

## Implementation Notes
- Focus on array parameter handling in schema converter
- Test with multiple file patterns and single files
- Ensure Git repository requirements still enforced
- Verify search database operations work correctly
- Test both indexing and querying workflows