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



## Proposed Solution

Based on the workspace dependencies and the requirements in ideas/rules.md, I will add the following dependencies to the rules crate:

**Core Dependencies:**
- `serde_yaml` - for YAML serialization (rules are YAML files)
- `anyhow` - for error handling
- `tracing` - for logging
- `tokio` - for async runtime

**SwissArmyHammer Dependencies:**
- `swissarmyhammer-config` - for configuration
- `swissarmyhammer-templating` - for liquid templates
- `swissarmyhammer-prompts` - to render .check builtin prompt
- `swissarmyhammer-workflow` - for agent execution

**File System and Utilities:**
- `walkdir` - for traversing directories
- `regex` - for pattern matching
- `chrono` - for timestamps
- `ulid` - for unique identifiers
- `glob` - for glob patterns
- `ignore` - for .gitignore support
- `dirs` - for standard directories

**Template Engine:**
- `liquid` - already used in templating crate, needed for direct template rendering

**Include Functionality:**
- `include_dir` - for embedding builtin rules

**Language Detection:**
- `tree-sitter` - reusing existing tree-sitter infrastructure

**Dev Dependencies:**
- Keep existing: `tempfile`, `tokio-test`, `serial_test`

The approach:
1. Add all dependencies to Cargo.toml
2. Verify the crate builds with `cargo build -p swissarmyhammer-rules`
3. Fix any dependency conflicts if they arise



## Implementation Notes

Successfully added all required dependencies to `swissarmyhammer-rules/Cargo.toml`:

**Added Core Dependencies:**
- `serde_yaml` - for YAML serialization
- `anyhow` - for error handling  
- `tracing` - for logging
- `tokio` - for async runtime

**Added SwissArmyHammer Dependencies:**
- `swissarmyhammer-config` - for configuration
- `swissarmyhammer-templating` - for liquid templates
- `swissarmyhammer-prompts` - to render .check builtin prompt
- `swissarmyhammer-workflow` - for agent execution

**Added File System and Utilities:**
- `walkdir` - for traversing directories
- `regex` - for pattern matching
- `chrono` - for timestamps
- `ulid` - for unique identifiers
- `glob` - for glob patterns
- `ignore` - for .gitignore support
- `dirs` - for standard directories

**Added Template Engine:**
- `liquid` - for template rendering

**Added Include Functionality:**
- `include_dir = "0.7"` - for embedding builtin rules

**Added Language Detection:**
- `tree-sitter` - reusing existing tree-sitter infrastructure

**Verification:**
- ✅ Crate builds successfully: `cargo build -p swissarmyhammer-rules` completed in 27.66s
- ✅ All tests pass: 18 tests run, 18 passed, 0 skipped
- ✅ No dependency conflicts detected

All dependencies resolved correctly from workspace definitions, ensuring consistency across the project.
