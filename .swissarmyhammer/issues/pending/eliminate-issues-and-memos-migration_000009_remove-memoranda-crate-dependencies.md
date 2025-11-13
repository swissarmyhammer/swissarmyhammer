# Step 9: Remove swissarmyhammer-memoranda Crate Dependencies

Refer to ideas/eliminate-issues-and-memos-migration.md

## Goal

Remove all dependencies on `swissarmyhammer-memoranda` crate from other crates in the workspace. This must be done before removing the crate itself.

## Context

Before we can remove the `swissarmyhammer-memoranda` crate, we need to ensure no other crates depend on it. Based on the grep results, these crates likely have dependencies:
- `swissarmyhammer-tools` (contains memo MCP tools - already removed in step 6)
- `swissarmyhammer-search` (might use memos)
- `swissarmyhammer-cli` (might have memo CLI commands)

## Implementation Tasks

### 1. Find All Dependencies

Search for references to the memoranda crate:

```bash
rg "swissarmyhammer-memoranda|swissarmyhammer_memoranda" --type toml
rg "use.*swissarmyhammer_memoranda" --type rust
```

### 2. Remove Cargo.toml Dependencies

For each crate that depends on `swissarmyhammer-memoranda`:

**In `swissarmyhammer-tools/Cargo.toml`:**
```toml
# Remove this line
swissarmyhammer-memoranda = { path = "../swissarmyhammer-memoranda" }
```

**In `swissarmyhammer-search/Cargo.toml`:**
```toml
# Remove this line (if exists)
swissarmyhammer-memoranda = { path = "../swissarmyhammer-memoranda" }
```

**In `swissarmyhammer-cli/Cargo.toml`:**
```toml
# Remove this line (if exists)
swissarmyhammer-memoranda = { path = "../swissarmyhammer-memoranda" }
```

### 3. Remove Rust Imports

Search for and remove any remaining Rust imports:

```bash
rg "use swissarmyhammer_memoranda" --type rust
```

Remove lines like:
```rust
use swissarmyhammer_memoranda::*;
```

### 4. Remove Any CLI-Specific Memo Code

Check `swissarmyhammer-cli` for memo-specific code:
- Memo command handlers
- Memo-related display code
- Memo-related test helpers

### 5. Update Tool Context

Check if `ToolContext` in `swissarmyhammer-tools` has any memo-related fields or methods.

## Files to Modify

Based on grep results, check these files:
1. `swissarmyhammer-tools/Cargo.toml`
2. `swissarmyhammer-search/Cargo.toml`
3. `swissarmyhammer-cli/Cargo.toml`
4. Any `.rs` files with `use swissarmyhammer_memoranda`

## Testing Checklist

- ✅ All references to `swissarmyhammer-memoranda` removed from Cargo.toml files
- ✅ All Rust imports removed
- ✅ `cargo check` succeeds for each modified crate
- ✅ No compilation errors about missing types/modules
- ✅ All tests pass in modified crates

## Verification Commands

```bash
# Check for remaining dependencies
rg "swissarmyhammer-memoranda|swissarmyhammer_memoranda" --type toml

# Check for remaining imports
rg "use.*swissarmyhammer_memoranda" --type rust

# Verify each crate builds
cd swissarmyhammer-tools && cargo check
cd swissarmyhammer-search && cargo check
cd swissarmyhammer-cli && cargo check

# Run tests
cargo nextest run --fail-fast
```

## Acceptance Criteria

- No Cargo.toml files reference `swissarmyhammer-memoranda`
- No Rust files import `swissarmyhammer_memoranda`
- All modified crates build successfully
- All tests pass
- No compilation errors about missing memoranda types

## Estimated Changes

~10-30 lines (dependency removals + import cleanup)
