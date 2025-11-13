# Step 11: Remove swissarmyhammer-memoranda Crate

Refer to ideas/eliminate-issues-and-memos-migration.md

## Goal

Remove the `swissarmyhammer-memoranda` crate entirely from the workspace. This includes the crate directory, workspace membership, and any related files.

## Context

After removing all dependencies (step 9) and MCP tools (step 6), we can now safely delete the entire memoranda crate. This is a permanent removal with no backward compatibility.

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
    "swissarmyhammer-common",
    # Remove this line:
    # "swissarmyhammer-memoranda",
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
rm -rf swissarmyhammer-memoranda/
```

### 3. Update Cargo.lock

```bash
cargo update
```

This will regenerate Cargo.lock without the memoranda crate.

### 4. Verify No References Remain

```bash
# Should return nothing
rg "swissarmyhammer-memoranda|swissarmyhammer_memoranda" --type toml
rg "swissarmyhammer-memoranda|swissarmyhammer_memoranda" --type rust

# Check that directory is gone
ls swissarmyhammer-memoranda/ 2>&1 | grep "No such file"
```

### 5. Check for Documentation References

```bash
# Check README and docs
rg "memoranda crate|swissarmyhammer-memoranda|memos system" --type md
```

Update any documentation that mentions the memoranda crate or memos system.

## Files to Modify

1. `Cargo.toml` (workspace root) - Remove from members
2. `Cargo.lock` (regenerated automatically)
3. Documentation files (if they reference the memoranda crate)

## Files to Delete

1. `swissarmyhammer-memoranda/` (entire directory)

## Testing Checklist

- ✅ Crate removed from workspace members
- ✅ Crate directory deleted
- ✅ Cargo.lock updated
- ✅ No references to memoranda crate remain
- ✅ `cargo check --workspace` succeeds
- ✅ `cargo build --workspace` succeeds
- ✅ All tests pass
- ✅ Documentation updated (if needed)

## Verification Commands

```bash
# Verify directory is gone
ls swissarmyhammer-memoranda/ 2>&1 | grep "No such file"

# Check workspace members
grep -A 20 "^\[workspace\]" Cargo.toml | grep -v "memoranda"

# Build entire workspace
cargo build --workspace

# Run all tests
cargo nextest run --fail-fast --workspace
```

## Acceptance Criteria

- `swissarmyhammer-memoranda` removed from workspace members in root Cargo.toml
- `swissarmyhammer-memoranda/` directory completely deleted
- Cargo.lock regenerated without memoranda crate
- No references to memoranda crate in codebase
- Workspace builds successfully
- All tests pass
- Documentation updated if it referenced memoranda crate or memos

## Estimated Changes

~5 lines in Cargo.toml + deletion of entire crate directory (~800+ lines removed)
