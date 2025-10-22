# Add `--changed` Filter to Rule Check

## Problem

Currently, running rule checks requires either checking all files or manually specifying file paths. For work-in-progress development, we want to easily check only changed files without manually listing them.

## Requirements

### 1. MCP Tool Changes

Add optional `changed` boolean parameter to `rules_check` MCP tool:

```rust
pub struct RuleCheckRequest {
    // ... existing fields ...
    
    /// Check only changed files (intersects with file_paths if provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed: Option<bool>,
}
```

**Behavior:**
- When `changed: true`, get changed files from git changes tool
- Intersect changed files with `file_paths` if provided
- If `file_paths` is empty/None, use `**/*.*` wildcard then filter by changed files
- If `changed: false` or not provided, use existing behavior

### 2. CLI Changes

Add `--changed` flag to `sah rule check` command:

```rust
#[derive(Parser)]
pub struct CheckArgs {
    // ... existing fields ...
    
    /// Check only changed files
    #[arg(long)]
    pub changed: bool,
}
```

### 3. Git Changes Tool Modification

**Current behavior:** When on main/trunk branch, returns ALL tracked files

**New behavior:** When on main/trunk branch, return only uncommitted changes (staged + unstaged)

**Rationale:** 
- On feature branches: return all changes since divergence from parent (current behavior)
- On main/trunk: return only uncommitted work-in-progress changes
- This makes `--changed` useful for checking WIP before committing on any branch

### 4. Implementation Details

**File intersection logic:**
```rust
let files_to_check = if request.changed.unwrap_or(false) {
    // Get changed files from git
    let changed_files = git_changes_tool.get_changed_files(current_branch)?;
    
    // Get base file patterns (or default to all)
    let base_patterns = request.file_paths
        .unwrap_or_else(|| vec!["**/*.*".to_string()]);
    
    // Expand glob patterns
    let matched_files = expand_globs(&base_patterns)?;
    
    // Intersect: only files that are both changed AND match patterns
    matched_files.intersection(&changed_files).collect()
} else {
    // Existing behavior
    request.file_paths.unwrap_or_else(|| vec!["**/*.*".to_string()])
};
```

**Git changes tool update:**
```rust
// In swissarmyhammer-git/src/changes.rs or similar
pub fn get_changed_files(branch: &str) -> Result<Vec<String>> {
    if is_main_or_trunk_branch(branch) {
        // Return only uncommitted changes
        get_uncommitted_changes()
    } else {
        // Existing behavior: all changes since branch divergence
        get_branch_changes(branch)
    }
}

fn get_uncommitted_changes() -> Result<Vec<String>> {
    // git diff HEAD --name-only  (uncommitted changes)
    // git diff --cached --name-only  (staged changes)
    // Union of both
}
```

## Use Cases

### Use Case 1: Check all WIP changes
```bash
sah rule check --changed
```
Checks all uncommitted files against all rules.

### Use Case 2: Check WIP Rust files only
```bash
sah rule check --changed --file-paths "**/*.rs"
```
Checks only uncommitted Rust files.

### Use Case 3: Check specific rules on WIP
```bash
sah rule check --changed --rule-names no-helpers no-magic-numbers
```
Checks specific rules on all uncommitted files.

### Use Case 4: MCP usage
```json
{
  "rule_names": ["no-helpers"],
  "file_paths": ["**/*.rs"],
  "changed": true,
  "max_errors": 10
}
```

## Files to Modify

1. `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs`
   - Add `changed` field to `RuleCheckRequest`
   - Add file intersection logic
   - Call git changes tool

2. `swissarmyhammer-cli/src/commands/rule/check.rs`
   - Add `--changed` flag
   - Pass through to MCP tool

3. `swissarmyhammer-git/src/lib.rs` (or wherever git changes lives)
   - Modify behavior for main/trunk branches
   - Add `get_uncommitted_changes()` function
   - Update `get_changed_files()` to branch on main/trunk detection

4. `swissarmyhammer-tools/src/mcp/tools/git/changes.rs`
   - Update MCP tool wrapper if separate from library

## Tests

1. Test rule check with `--changed` on feature branch
2. Test rule check with `--changed` on main branch
3. Test rule check with `--changed` + `--file-paths` intersection
4. Test git changes tool on main returns only uncommitted
5. Test git changes tool on feature branch returns all changes since divergence

## Success Criteria

- `sah rule check --changed` runs quickly on WIP files only
- Works correctly on both feature branches and main/trunk
- File path filters properly intersect with changed files
- Git changes tool no longer returns "all files" when on main/trunk
