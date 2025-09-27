# Step 1: Build System Integration for Built-in Agents

Refer to ideas/models.md

## Objective

Add build-time compilation of built-in agents from `builtin/agents/` directory into the binary as embedded resources, following the same pattern as prompts and workflows.

## Tasks

### 1. Add `generate_builtin_agents()` Function
- Add function to `swissarmyhammer-config/build.rs`
- Scan `../builtin/agents/*.yaml` files at build time
- Generate `builtin_agents.rs` with embedded content using `include_str!`
- Use file stem as agent name (e.g., `qwen-coder.yaml` → `qwen-coder`)

### 2. Call Generation Function
- Add `generate_builtin_agents(&out_dir)` call to `main()` in `build.rs`
- Place after existing generation functions

### 3. Include Generated Code
- Add `include!(concat!(env!("OUT_DIR"), "/builtin_agents.rs"));` to appropriate location in `swissarmyhammer-config/src/lib.rs`

## Implementation Notes

- Follow exact pattern from prompt/workflow generation
- The generated function should return `Vec<(&'static str, &'static str)>`
- Function name: `get_builtin_agents()`
- Generated file: `builtin_agents.rs` in `OUT_DIR`
- Support only `.yaml` extension for consistency

## Acceptance Criteria

- Build succeeds with new generation
- Generated code includes all 3 existing built-in agents
- `get_builtin_agents()` function is available for use
- No changes to runtime behavior yet

## Files to Modify

- `swissarmyhammer-config/build.rs`
- `swissarmyhammer-config/src/lib.rs`