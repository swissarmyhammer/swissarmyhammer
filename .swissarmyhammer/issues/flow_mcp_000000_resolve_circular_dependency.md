# Step 0: Resolve Circular Dependency Between Workflow and Tools

Refer to ideas/flow_mcp.md

## Objective

Resolve the circular dependency between `swissarmyhammer-workflow` and `swissarmyhammer-tools` to enable flow MCP tool implementation.

## Problem

**Current State**:
- `swissarmyhammer-workflow` depends on `swissarmyhammer-tools` (line 30 of workflow/Cargo.toml)
- Flow MCP tool needs to depend on `swissarmyhammer-workflow` for `WorkflowStorage` and `Workflow` types
- This creates a circular dependency that prevents compilation

**What's Blocked**:
- Flow MCP tool implementation (steps 3, 4, 8)
- Workflow discovery via MCP
- Workflow execution via MCP
- All integration with WorkflowStorage

## Analysis

### Why Does Workflow Depend on Tools?

Check `swissarmyhammer-workflow/Cargo.toml` line 30:
```toml
swissarmyhammer-tools = { path = "../swissarmyhammer-tools" }
```

Need to identify what from tools is being used by workflow.

### Potential Solutions

#### Option 1: Move Shared Types to Common

Move `WorkflowStorage` trait and core workflow types to `swissarmyhammer-common`:
- Both workflow and tools can depend on common
- Common has no dependencies on either
- Cleanest separation of concerns

#### Option 2: Create Workflow Storage Crate

Create new crate `swissarmyhammer-workflow-storage`:
- Contains only storage traits and types
- Both workflow and tools depend on it
- More granular, follows single responsibility

#### Option 3: Remove Tools Dependency from Workflow

Identify what workflow uses from tools and either:
- Move it to common
- Duplicate it (if small)
- Refactor workflow to not need it
- Use dependency injection pattern

#### Option 4: Facade Pattern

Create facade in CLI that coordinates both:
- Tools MCP layer doesn't depend on workflow
- CLI depends on both and wires them together
- More complex but avoids circular dependency

## Tasks

### 1. Identify Workflow's Usage of Tools

```bash
# Find all imports of swissarmyhammer-tools in workflow crate
rg "use.*swissarmyhammer_tools" swissarmyhammer-workflow/src
rg "swissarmyhammer_tools::" swissarmyhammer-workflow/src
```

Document what workflow needs from tools.

### 2. Choose Solution Based on Usage

Based on what workflow actually uses:
- If minimal: Option 3 (remove dependency)
- If storage-related: Option 1 (move to common) or Option 2 (new crate)
- If tightly coupled: Option 4 (facade pattern)

### 3. Implement Chosen Solution

#### If Option 1 (Move to Common):

```bash
# Move types to common
mv swissarmyhammer-workflow/src/storage.rs swissarmyhammer-common/src/workflow_storage.rs

# Update imports in both crates
# Update Cargo.toml dependencies
```

#### If Option 2 (New Crate):

```bash
# Create new crate
cargo new --lib swissarmyhammer-workflow-storage

# Move storage traits and types
# Update both Cargo.toml files to depend on new crate
```

#### If Option 3 (Remove Dependency):

```bash
# Remove tools dependency from workflow/Cargo.toml
# Refactor workflow code to not use tools
# May need to duplicate small utilities
```

### 4. Update All Imports

Search and replace imports throughout codebase:
```bash
# Find all affected files
rg "swissarmyhammer_workflow::.*Storage" --files-with-matches
rg "swissarmyhammer_tools::.*Workflow" --files-with-matches
```

Update imports to use new location.

### 5. Update Cargo.toml Files

Remove circular dependency:
- `swissarmyhammer-workflow/Cargo.toml`: Remove or keep tools dependency
- `swissarmyhammer-tools/Cargo.toml`: Add workflow or storage dependency

### 6. Verify Build

```bash
# Clean build to verify no circular dependency
cargo clean
cargo build --all

# Check for warnings
cargo clippy --all

# Run tests
cargo nextest run --all
```

## Files to Investigate

- `swissarmyhammer-workflow/Cargo.toml`
- `swissarmyhammer-workflow/src/**/*.rs` (find tools usage)
- `swissarmyhammer-tools/Cargo.toml`
- Potentially: `swissarmyhammer-common/Cargo.toml`

## Files to Modify (TBD based on solution)

Will be determined after analysis phase.

## Acceptance Criteria

- [ ] Analysis complete: documented what workflow uses from tools
- [ ] Solution chosen based on actual usage
- [ ] Circular dependency removed
- [ ] `cargo build --all` succeeds
- [ ] `cargo clippy --all` shows no warnings
- [ ] All existing tests still pass
- [ ] No circular dependency errors
- [ ] Tools can now depend on workflow types (or common storage types)

## Estimated Changes

~50-200 lines depending on solution chosen

## Priority

**CRITICAL**: This blocks all other flow MCP work (steps 1-12)
