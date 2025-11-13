# Step 8: Remove swissarmyhammer-issues Crate Dependencies

Refer to ideas/eliminate-issues-and-memos-migration.md

## Goal

Remove all dependencies on `swissarmyhammer-issues` crate from other crates in the workspace. This must be done before removing the crate itself.

## Context

Before we can remove the `swissarmyhammer-issues` crate, we need to ensure no other crates depend on it. Based on the grep results, these crates likely have dependencies:
- `swissarmyhammer-tools` (contains issue MCP tools - already removed in step 5)
- `swissarmyhammer-cli` (might have issue CLI commands)
- `swissarmyhammer-workflow` (might reference issues)
- Main `swissarmyhammer` crate

## Implementation Tasks

### 1. Find All Dependencies

Search for references to the issues crate:

```bash
rg "swissarmyhammer-issues|swissarmyhammer_issues" --type toml
rg "use.*swissarmyhammer_issues" --type rust
```

### 2. Remove Cargo.toml Dependencies

For each crate that depends on `swissarmyhammer-issues`:

**In `swissarmyhammer-tools/Cargo.toml`:**
```toml
# Remove this line
swissarmyhammer-issues = { path = "../swissarmyhammer-issues" }
```

**In `swissarmyhammer-cli/Cargo.toml`:**
```toml
# Remove this line
swissarmyhammer-issues = { path = "../swissarmyhammer-issues" }
```

**In `swissarmyhammer-workflow/Cargo.toml`:**
```toml
# Remove this line (if exists)
swissarmyhammer-issues = { path = "../swissarmyhammer-issues" }
```

**In `swissarmyhammer/Cargo.toml`:**
```toml
# Remove this line (if exists)
swissarmyhammer-issues = { path = "../swissarmyhammer-issues" }
```

### 3. Remove Rust Imports

Search for and remove any remaining Rust imports:

```bash
rg "use swissarmyhammer_issues" --type rust
```

Remove lines like:
```rust
use swissarmyhammer_issues::*;
```

### 4. Remove Any CLI-Specific Issue Code

Check `swissarmyhammer-cli` for issue-specific code:
- Issue command handlers
- Issue-related display code
- Issue-related test helpers

### 5. Update Tool Context

Check if `ToolContext` in `swissarmyhammer-tools` has any issue-related fields or methods.

## Files to Modify

Based on grep results, check these files:
1. `swissarmyhammer-tools/Cargo.toml`
2. `swissarmyhammer-cli/Cargo.toml`
3. `swissarmyhammer-workflow/Cargo.toml`
4. `swissarmyhammer/Cargo.toml`
5. Any `.rs` files with `use swissarmyhammer_issues`

## Testing Checklist

- ✅ All references to `swissarmyhammer-issues` removed from Cargo.toml files
- ✅ All Rust imports removed
- ✅ `cargo check` succeeds for each modified crate
- ✅ No compilation errors about missing types/modules
- ✅ All tests pass in modified crates

## Verification Commands

```bash
# Check for remaining dependencies
rg "swissarmyhammer-issues|swissarmyhammer_issues" --type toml

# Check for remaining imports
rg "use.*swissarmyhammer_issues" --type rust

# Verify each crate builds
cd swissarmyhammer-tools && cargo check
cd swissarmyhammer-cli && cargo check
cd swissarmyhammer-workflow && cargo check

# Run tests
cargo nextest run --fail-fast
```

## Acceptance Criteria

- No Cargo.toml files reference `swissarmyhammer-issues`
- No Rust files import `swissarmyhammer_issues`
- All modified crates build successfully
- All tests pass
- No compilation errors about missing issue types

## Estimated Changes

~10-30 lines (dependency removals + import cleanup)
