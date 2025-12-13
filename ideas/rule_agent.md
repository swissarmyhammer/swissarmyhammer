# Rule Agent Configuration

## Problem Statement

Currently, rule checking is hardcoded to use whatever agent configuration is set system-wide. There's no way to specify that rule checking operations should use a particular agent (e.g., a faster/cheaper model, or a specialized agent configuration).

The goal is simple: allow configuration of which agent to use for rule checking operations.

## Current Agent System

SwissArmyHammer already has a comprehensive agent system with:

**Built-in agents:**
- `claude-code` - Shell execution using Claude Code CLI (default)
- `qwen-coder` - Local Qwen model via llama-agent
- `qwen-coder-flash` - Faster Qwen variant
- `deepseek-terminus` - DeepSeek model
- `GLM-4.6` - GLM model

**Agent discovery hierarchy:**
1. Built-in agents (embedded in binary)
2. Project agents (`./agents/*.yaml`)
3. User agents (`~/.swissarmyhammer/agents/*.yaml`)

**Commands:**
- `sah agent list` - List all available agents
- `sah agent use <name>` - Apply a specific agent

**Agent config format:**
```yaml
quiet: false
executor:
  type: claude-code  # or llama-agent
  config: {}
```

## Proposed Approaches

Since we already have an agent system with a registry of available agents, we just need to allow selection of which agent to use for rule checking.

### Approach 1: Config-Only Selection

Add a simple config key to specify which agent to use:

```yaml
# .swissarmyhammer/config.yaml
rule_checker:
  agent: "qwen-coder-flash"  # Use fast local model for rule checking
```

**Pros:**
- Simple - just one config key
- Leverages existing agent system
- Easy to understand

**Cons:**
- Can't override per-invocation without editing config

---

### Approach 2: Config + CLI Override

Config default with CLI flag override:

```yaml
# .swissarmyhammer/config.yaml
rule_checker:
  agent: "qwen-coder-flash"  # Default
```

```bash
# Use default agent from config
sah rules check

# Override with CLI flag
sah rules check --agent claude-code
```

**Pros:**
- Config provides sensible default
- CLI flag allows experimentation
- Good for testing different agents

**Cons:**
- Slightly more implementation work

---

### Approach 3: Use Case-Based Agent Assignment

Instead of just "use an agent", configure different agents for different use cases:

```yaml
# .swissarmyhammer/config.yaml
agents:
  root: "claude-code"           # Default agent for general operations
  rules: "qwen-coder-flash"     # Agent for rule checking
```

**Commands:**
```bash
# Show current agent configuration per use case
sah agent
# Output:
# root: claude-code
# rules: qwen-coder-flash

# Set agent for a specific use case
sah agent use --context rules qwen-coder

# Or simpler syntax?
sah agent use rules qwen-coder
```

**Pros:**
- Clear separation of concerns
- Different operations can use different agents
- Easy to see what agent is used where
- Extensible for future use cases (workflows, planning, etc.)

**Cons:**
- More complex than single agent selection
- Need to define what "use cases" exist

---

## Recommendation

**Approach 3** (Use Case-Based Agent Assignment) because:

### Current vs Proposed Behavior

**Current:**
- `sah agent use <name>` - Sets THE agent (singular, system-wide)
- All operations use the same agent

**Proposed:**
- `sah agent use <name>` - Sets root agent (backward compatible)
- `sah agent use <use-case> <name>` - Sets agent for specific use case
- Different operations can use different agents

### Why This Approach

1. Leverages existing agent registry system
2. Provides clear separation: different operations can use different agents
3. Extensible: easy to add new use cases (workflows, planning, etc.)
4. `sah agent` with no args shows current configuration
5. Backward compatible: `sah agent use <name>` sets root agent

## Implementation Plan

### Phase 1: Basic Use Case Support
1. Add `agents` section to config schema with use case mapping
2. Define initial use cases: `root`, `rules`
3. Implement fallback logic (use case → root → default)
4. Update `sah agent` to show use case assignments
5. Update `sah agent use` to support use case argument

### Phase 2: CLI Override
6. Add `--agent` flag to `rules check` command
7. Pass agent selection through to rule checking code

### Phase 3: Validation & Polish
8. Validate agent names against available agents
9. Show helpful errors for missing agents
10. Add tests for use case resolution

## Config Schema

```yaml
# .swissarmyhammer/config.yaml
agents:
  root: "claude-code"           # Default agent (fallback)
  rules: "qwen-coder-flash"     # Agent for rule checking
  workflows: "claude-code"      # Agent for workflow execution (plan, review, implement)
```

The config keys map directly to the `AgentUseCase` enum variants (lowercase).

## CLI Behavior

```bash
# Show use case assignments
sah agent
# Output:
# Agent Use Case Assignments:
# ┌───────────┬──────────────────┐
# │ Use Case  │ Agent            │
# ├───────────┼──────────────────┤
# │ root      │ claude-code      │
# │ rules     │ qwen-coder-flash │
# │ workflows │ claude-code      │
# └───────────┴──────────────────┘

# List available agents
sah agent list
# → [table of available agents]

# Set agent for specific use case
sah agent use rules qwen-coder

# Set root agent (backward compatible)
sah agent use claude-code
# or explicitly:
sah agent use root claude-code

# Override at runtime (global flag, overrides all use cases)
sah --agent claude-code rules check
```

## How Rule Checking Currently Works

From analyzing the code in `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs`:

1. **MCP Tool Structure:**
   - `RuleCheckTool` is an MCP tool that handles the `rules_check` operation
   - It receives a `ToolContext` which contains an `agent_config: Arc<AgentConfig>`
   - The tool uses this agent config to create an agent executor

2. **Agent Usage Flow:**
   ```rust
   async fn get_checker(&self, context: &ToolContext) -> Result<&RuleChecker, McpError> {
       // Gets agent config from ToolContext
       let agent_config = context.agent_config.clone();

       // Creates agent executor from the config
       let agent = create_agent_from_config(&agent_config).await?;

       // Creates RuleChecker with that agent
       let checker = RuleChecker::new(agent)?;
   }
   ```

3. **ToolContext Structure:**
   - Lives in `swissarmyhammer-tools/src/mcp/tool_registry.rs`
   - Contains `pub agent_config: Arc<AgentConfig>`
   - This is currently THE agent config (singular) used by all tools

## Implementation Approach

### How to Support Use Case-Specific Agents

**Option A: Modify ToolContext (with Enum)**

Define use cases as an enum and add resolution to ToolContext:

```rust
/// Enumeration of agent use cases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentUseCase {
    /// Default/fallback agent for general operations
    Root,
    /// Agent for rule checking operations
    Rules,
    /// Agent for workflow execution (plan, review, implement, etc.)
    Workflows,
}

pub struct ToolContext {
    pub agent_config: Arc<AgentConfig>,  // root/default agent
    pub use_case_agents: Arc<HashMap<AgentUseCase, AgentConfig>>,  // use case -> agent
}

impl ToolContext {
    pub fn get_agent_for_use_case(&self, use_case: AgentUseCase) -> &AgentConfig {
        self.use_case_agents.get(&use_case)
            .unwrap_or(&self.agent_config)  // fallback to root
    }
}
```

Then in rule checking:
```rust
let agent_config = context.get_agent_for_use_case(AgentUseCase::Rules);
```

**Option B: Pass Use Case Through Tool**

Add use case parameter to each tool's execute method, resolve agent in tool registry.

**Option C: Tool-Specific Config Resolution**

Each tool knows its own use case and resolves the appropriate agent:
```rust
// In RuleCheckTool
async fn get_checker(&self, context: &ToolContext) -> Result<&RuleChecker, McpError> {
    // Resolve agent for "rules" use case
    let agent_config = context.resolve_agent("rules");
    let agent = create_agent_from_config(&agent_config).await?;
    ...
}
```

## Testing Results

**Tests Written:**
1. ✅ `agent_override_actually_works_test.rs` - Proves McpServer correctly handles agent override
2. ✅ Server with override → ToolContext has LlamaAgent (qwen-coder-flash) for all use cases
3. ✅ Server without override → ToolContext has ClaudeCode (default)

**Bug Found:**
- `sah --agent qwen-coder-flash rule check` still uses ClaudeCode
- Tests prove: McpServer correctly uses override when passed
- **Root cause: CLI doesn't pass `--agent` flag value to McpServer initialization**
- CliContext struct doesn't have an `agent` field
- The `--agent` flag is defined in dynamic_cli.rs but never used

**Fix Needed:**
1. Add `agent: Option<String>` field to CliContext
2. Parse `--agent` flag value in main.rs
3. Pass agent value to McpServer::new_with_work_dir()

## Design Decisions

1. **Enum placement:** `swissarmyhammer-config` (central, accessible by both config and tools)

2. **Agent validation:** Fail fast when setting (validate at `sah agent use` time)

3. **Fallback behavior:** Silent fallback to Root agent when use case not configured

4. **Backward compatibility:** `sah agent use <agent>` sets only Root agent

5. **Global override:** `--agent` flag is global and overrides all use cases to specified agent

6. **Caching:** Leave caching alone, cache based on source and rule (not agent)

7. **Workflow agent integration:**
   - WorkflowExecutor needs to be expanded to have an agent
   - Flow MCP tool will pass the Workflows use case agent from ToolContext to WorkflowExecutor
   - Need to add agent parameter to WorkflowExecutor initialization

## Next Steps

### Design Phase
- [ ] Define exact enum variants: Root, Rules, Workflows
- [ ] Design config schema for use case -> agent name mapping
- [ ] Design ToolContext API for agent resolution
- [ ] Design WorkflowExecutor agent integration

### Implementation Phase
1. **Add AgentUseCase enum** to swissarmyhammer-config with serde support
2. **Update config schema** to support use case -> agent mappings
3. **Update ToolContext** to hold use_case_agents HashMap and provide resolution method
4. **Update MCP server initialization** to load use case mappings and resolve to AgentConfig
5. **Update rule checking tool** to use `context.get_agent_for_use_case(AgentUseCase::Rules)`
6. **Update WorkflowExecutor** to accept an agent and pass Workflows use case agent from flow tool
7. **Update CLI agent command** to:
   - Show use case assignments when called with no args
   - Support `sah agent use <use-case> <agent>` syntax
   - Parse use case strings to enum variants
8. **Add global --agent flag** that overrides all use cases
9. **Add validation** for agent names when setting use cases
10. **Update tests** for use case resolution and fallback logic
