# Step 10: Remove swissarmyhammer-issues Crate

Refer to ideas/eliminate-issues-and-memos-migration.md

## Goal

Remove the `swissarmyhammer-issues` crate entirely from the workspace. This includes the crate directory, workspace membership, and any related files.

## Context

After removing all dependencies (step 8) and MCP tools (step 5), we can now safely delete the entire issues crate. This is a permanent removal with no backward compatibility.

## Implementation Tasks

### 1. Remove from Workspace

Edit `Cargo.toml` in workspace root:

```toml
[workspace]
members = [
    "swissarmyhammer",
    "swissarmyhammer-cli",
    "swissarmyhammer-tools",
    "swissarmyhammer-config",
    # Remove this line:
    # "swissarmyhammer-issues",
    "swissarmyhammer-common",
    "swissarmyhammer-memoranda",
    "swissarmyhammer-git",
    "swissarmyhammer-search",
    "swissarmyhammer-shell",
    "swissarmyhammer-todo",
    "swissarmyhammer-outline",
    "swissarmyhammer-templating",
    "swissarmyhammer-prompts",
    "swissarmyhammer-agent-executor",
    "swissarmyhammer-rules",
    "swissarmyhammer-workflow",
]
```

### 2. Remove Crate Directory

```bash
rm -rf swissarmyhammer-issues/
```

### 3. Update Cargo.lock

```bash
cargo update
```

This will regenerate Cargo.lock without the issues crate.

### 4. Verify No References Remain

```bash
# Should return nothing
rg "swissarmyhammer-issues|swissarmyhammer_issues" --type toml
rg "swissarmyhammer-issues|swissarmyhammer_issues" --type rust

# Check that directory is gone
ls swissarmyhammer-issues/ 2>&1 | grep "No such file"
```

### 5. Check for Documentation References

```bash
# Check README and docs
rg "issues crate|swissarmyhammer-issues" --type md
```

Update any documentation that mentions the issues crate.

## Files to Modify

1. `Cargo.toml` (workspace root) - Remove from members
2. `Cargo.lock` (regenerated automatically)
3. Documentation files (if they reference the issues crate)

## Files to Delete

1. `swissarmyhammer-issues/` (entire directory)

## Testing Checklist

- ✅ Crate removed from workspace members
- ✅ Crate directory deleted
- ✅ Cargo.lock updated
- ✅ No references to issues crate remain
- ✅ `cargo check --workspace` succeeds
- ✅ `cargo build --workspace` succeeds
- ✅ All tests pass
- ✅ Documentation updated (if needed)

## Verification Commands

```bash
# Verify directory is gone
ls swissarmyhammer-issues/ 2>&1 | grep "No such file"

# Check workspace members
grep -A 20 "^\[workspace\]" Cargo.toml | grep -v "issues"

# Build entire workspace
cargo build --workspace

# Run all tests
cargo nextest run --fail-fast --workspace
```

## Acceptance Criteria

- `swissarmyhammer-issues` removed from workspace members in root Cargo.toml
- `swissarmyhammer-issues/` directory completely deleted
- Cargo.lock regenerated without issues crate
- No references to issues crate in codebase
- Workspace builds successfully
- All tests pass
- Documentation updated if it referenced issues crate

## Estimated Changes

~5 lines in Cargo.toml + deletion of entire crate directory (~1000+ lines removed)
