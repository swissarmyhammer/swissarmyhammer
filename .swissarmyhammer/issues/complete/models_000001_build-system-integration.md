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

## Proposed Solution

I implemented build-time compilation of built-in agents from the `builtin/agents/` directory into the binary as embedded resources, following the exact same pattern used by prompts and workflows.

### Implementation Steps

1. **Created build.rs**: Added `swissarmyhammer-config/build.rs` with `generate_builtin_agents()` function
   - Scans `../builtin/agents/*.yaml` files at build time
   - Generates `builtin_agents.rs` with embedded content using `include_str!`
   - Uses file stem as agent name (e.g., `qwen-coder.yaml` → `qwen-coder`)

2. **Generated Code Structure**: The build script generates a function:
   ```rust
   pub fn get_builtin_agents() -> Vec<(&'static str, &'static str)> {
       vec![
           ("claude-code", r#"agent: ...#"),
           ("qwen-coder-flash", r#"agent: ...#"),
           ("qwen-coder", r#"agent: ...#"),
       ]
   }
   ```

3. **Library Integration**: Added `include!(concat!(env!("OUT_DIR"), "/builtin_agents.rs"));` to `swissarmyhammer-config/src/lib.rs`

4. **Testing**: Created comprehensive tests in `builtin_agents_generation_test.rs` to verify:
   - All 3 built-in agents are included
   - Agent names are correct (`claude-code`, `qwen-coder`, `qwen-coder-flash`)
   - Content contains valid YAML structure with required keys
   - Specific agent configurations are properly embedded

### Results

- ✅ Build succeeds with new generation
- ✅ Generated code includes all 3 existing built-in agents
- ✅ `get_builtin_agents()` function is available for use
- ✅ No changes to runtime behavior (as required)
- ✅ All tests pass (165 existing + 2 new tests)

The implementation follows the exact pattern from existing prompt/workflow generation with proper build caching via `cargo:rerun-if-changed=../builtin`.

## Implementation Notes

### Found Agent Files
- `builtin/agents/claude-code.yaml` - Claude Code executor
- `builtin/agents/qwen-coder.yaml` - Qwen3 Coder 480B model 
- `builtin/agents/qwen-coder-flash.yaml` - Qwen3 Coder 30B model (faster variant)

### Build Integration
- The build script watches `../builtin` directory for changes
- Only processes `.yaml` files (as specified in requirements)
- Alphabetical ordering maintained in generated code
- Content embedded as raw string literals with proper escaping

### Generated File Location
- Generated at build time: `target/debug/build/swissarmyhammer-config-*/out/builtin_agents.rs`
- Function accessible via public API: `swissarmyhammer_config::get_builtin_agents()`
- Ready for integration with agent loading system in future steps

### Testing Verification  
- All existing tests continue to pass (165 tests)
- New tests verify generation correctness (2 additional tests)
- Content validation ensures proper YAML structure