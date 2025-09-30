# Add All Dependencies to Rules Crate

Refer to ideas/rules.md

## Goal

Add all required dependencies to `swissarmyhammer-rules/Cargo.toml`.

## Context

The rules crate needs dependencies for:
- Prompts (to render .check)
- Workflow (for agent execution)
- Config (for configuration)
- Templating (for liquid templates)
- Standard utilities

## Implementation

Add to `Cargo.toml`:
```toml
[dependencies]
# Core dependencies
serde = { workspace = true }
serde_yaml = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
tokio = { workspace = true }

# SwissArmyHammer dependencies
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
swissarmyhammer-config = { path = "../swissarmyhammer-config" }
swissarmyhammer-templating = { path = "../swissarmyhammer-templating" }
swissarmyhammer-prompts = { path = "../swissarmyhammer-prompts" }
swissarmyhammer-workflow = { path = "../swissarmyhammer-workflow" }

# File system and utilities
walkdir = { workspace = true }
regex = { workspace = true }
chrono = { workspace = true }
ulid = { workspace = true }
glob = { workspace = true }
ignore = { workspace = true }
dirs = "5.0"

# Template engine
liquid = "0.26"

# Include functionality for builtin rules
include_dir = "0.7"

# Language detection - reuse existing tree-sitter
tree-sitter = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
tokio-test = "0.4"
serial_test = "3.1"
```

## Testing

- Verify all dependencies resolve: `cargo build -p swissarmyhammer-rules`

## Success Criteria

- [ ] All dependencies added
- [ ] Crate builds successfully
- [ ] No dependency conflicts
