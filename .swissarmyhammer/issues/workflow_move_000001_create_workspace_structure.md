# Create Workspace Structure for swissarmyhammer-workflow

Refer to /Users/wballard/github/swissarmyhammer/ideas/workflow_move.md

## Objective
Create the basic workspace infrastructure for the new `swissarmyhammer-workflow` crate as the first step in migrating workflow functionality from the main crate.

## Tasks
1. Create `swissarmyhammer-workflow/` directory at workspace root
2. Initialize `swissarmyhammer-workflow/Cargo.toml` with basic metadata
3. Add `swissarmyhammer-workflow` to workspace members in root `Cargo.toml`
4. Verify workspace builds correctly with new member

## Implementation Details

### Directory Structure
```
swissarmyhammer-workflow/
├── Cargo.toml
└── src/
    └── lib.rs (minimal placeholder)
```

### Cargo.toml Template
```toml
[package]
name = "swissarmyhammer-workflow"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "Workflow execution engine for SwissArmyHammer"

[dependencies]
# Core dependencies (minimal set to start)
tokio = { workspace = true }
serde = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }

# Internal dependencies 
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
```

### Initial lib.rs
```rust
//! SwissArmyHammer Workflow Engine
//!
//! This crate provides workflow definition, parsing, and execution capabilities
//! for the SwissArmyHammer ecosystem.

// Placeholder until modules are migrated
```

## Acceptance Criteria
- [ ] `swissarmyhammer-workflow` directory created
- [ ] Basic `Cargo.toml` with workspace integration
- [ ] Added to root workspace `Cargo.toml` members
- [ ] `cargo check` passes for entire workspace
- [ ] No existing functionality broken

## Next Step
Step 000002: Set up basic library structure