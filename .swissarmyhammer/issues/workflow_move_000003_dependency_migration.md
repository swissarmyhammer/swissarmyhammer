# Dependency Analysis and Migration

Refer to /Users/wballard/github/swissarmyhammer/ideas/workflow_move.md

## Objective
Extract all workflow-specific dependencies from the main swissarmyhammer crate and add them to the new workflow crate's Cargo.toml.

## Tasks
1. Analyze current workflow dependencies in main crate
2. Identify workflow-specific external dependencies
3. Add dependencies to workflow crate Cargo.toml
4. Remove workflow-specific deps from main crate (if safe)
5. Verify workspace builds correctly

## Current Workflow Dependencies (from analysis)
**External workflow-specific dependencies:**
- `mermaid-parser` - For Mermaid diagram parsing
- `cel-interpreter = "0.8"` - For CEL expression evaluation
- `chumsky = "0.10.1"` - Parser combinator for action parsing  
- `llama-agent` - For agent execution
- `lru = "0.12"` - For caching
- `which = "8.0.0"` - For command validation

**Shared dependencies that workflow uses:**
- Standard workspace deps (tokio, serde, anyhow, etc.)
- Internal crates (swissarmyhammer-common, swissarmyhammer-shell, etc.)

## Implementation Details

### Add to workflow crate Cargo.toml
```toml
[dependencies]
# Core workspace dependencies
tokio = { workspace = true }
serde = { workspace = true }
serde_yaml = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }

# Internal SwissArmyHammer crates
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
swissarmyhammer-shell = { path = "../swissarmyhammer-shell" }
swissarmyhammer-templating = { path = "../swissarmyhammer-templating" }

# Workflow-specific external dependencies
mermaid-parser = { git = "https://github.com/wballard/mermaid_parser" }
cel-interpreter = "0.8"
chumsky = "0.10.1"
llama-agent = { workspace = true }
lru = "0.12"
which = "8.0.0"

# Additional deps that workflow needs
walkdir = { workspace = true }
regex = { workspace = true }
chrono = { workspace = true }
ulid = { workspace = true }
tempfile = { workspace = true }
```

### Update main crate dependencies
- Keep workflow-specific deps in main crate for now (to avoid breaking existing code)
- Will remove in later step after migration complete

## Acceptance Criteria
- [ ] All workflow dependencies identified and documented
- [ ] Workflow crate Cargo.toml has all necessary dependencies
- [ ] `cargo check` passes for workflow crate
- [ ] `cargo check` passes for entire workspace
- [ ] No functionality regressions in main crate

## Next Step
Step 000004: Migrate utility modules (error_utils, test_helpers, transition_key)