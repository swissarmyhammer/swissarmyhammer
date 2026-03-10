# Llama-Agent Feature Gaps vs Claude Code

## Overview
Comprehensive review of llama-agent capabilities compared to Claude Code. Identifies missing features and recommends priority roadmap.

## Critical Gaps (Block Major Workflows)

### 1. Hook System & External Validators
**Current State:** llama-agent has internal validation only
**What's Missing:** PreToolUse/PostToolUse hooks that external validators can intercept

**Why It Matters:**
- Claude Code integrates code-quality, security-rules, test-integrity, command-safety validators
- Validators run on file writes/edits and can block bad changes
- Currently no way to run external validators in llama-agent tool pipeline

**Implementation Notes:**
- Requires ACP protocol extension for hook message types
- Need to wire validators into tool execution flow
- Could start with PostToolUse (easier than PreToolUse)

---

### 2. Subagents (Agent Spawning)
**Current State:** Single agent per connection
**What's Missing:** Ability to spawn child agents for task delegation

**Why It Matters:**
- Claude Code spawns specialized agents (reviewer, tester, implementer, planner, etc.)
- Enables parallelization and work distribution
- Allows agents to delegate to more focused specialists

**Design Status:** ✅ COMPLETE
See: `ideas/agent-tool-complete-design.md` for full specification

**Implementation Strategy:**
- Agents are MCP tools (not special OS processes)
- Follow shell tool pattern: spawn, list, kill, grep, search
- Parallel execution via RequestQueue (shared in-memory model)
- Smart notifications via `notify agent` tool (agents call it directly, routed to parent session)
- Result access via spawn completion + grep/search (no resources)
- Parent correlates calls to agents via agent_id in responses + notifications

---

### 3. Inline Tools (Tools in Skills)
**Current State:** All tools come from external MCP servers
**What's Missing:** Tools defined/bundled within skill definitions

**Why It Matters:**
- Claude Code skills can define tools inline in YAML
- Makes skills self-contained + executable
- Reduces external dependency overhead

**Implementation Notes:**
- Create MCP tool to register tools from skill definitions
- Could parse skill YAML for tool blocks
- Or extend skill library to support tool registration

---

## Medium Priority (Improve DX)

### 4. Planning Workflow Gates
**Current State:** Plan skill exists but no approval mechanism
**What's Missing:** EnterPlanMode/ExitPlanMode structured flow with user approval

**Why It Matters:**
- Prevents accidental execution without user review
- Makes planning explicit and reviewable
- Kanban board becomes checkpoint before implementation

**Implementation Notes:**
- Could fake via prompt flow (not ideal)
- Might work without ACP extension if we're creative
- Better with explicit ACP mode support

---

### 5. Auto Memory
**Current State:** No persistent memory between sessions
**What's Missing:** MEMORY.md per project, persistent across sessions

**Why It Matters:**
- Projects can accumulate knowledge over time
- Patterns and conventions learned and remembered
- Context preserved between disconnects

**Implementation Notes:**
- Add /remember, /forget commands
- Load MEMORY.md into context at session start
- Store learnings back to file at session end

---

## Nice-to-Have (Lower Priority)

- **Keybinding customization** — editor-specific, not server responsibility
- **Test-Driven Development formalization** — strict TDD workflow enforcement
- **Richer builtin prompts** — more workflow templates

---

## What's Already Strong ✅

- MCP support (stdin + HTTP transports)
- ACP protocol compliance (6/6 init tests passing)
- Session modes (Code, Plan, Test)
- Fine-grained permissions (AlwaysAsk, AutoApprove, RuleBased)
- Real-time streaming
- Skill integration
- Slash commands from MCP prompts
- File operations + terminal execution

---

## Implementation Roadmap

### Phase 1: Enable Validation (Post-Tool Hooks)
1. Extend ACP with PostToolUse event message
2. Create hook dispatcher in acp::server
3. Wire tool results through validation pipeline
4. Start with simple validators (code-quality)

### Phase 2: Enable Parallelization (Subagents)
1. Implement agent tool with standard MCP operations (spawn, list, kill, grep, search, notify)
2. Build session routing for `notify agent` (subagent ↔ parent message delivery)
3. Implement agent composition + coordination via MCP notifications
4. Adopt MCP Tasks for long-running agent lifecycle tracking

### Phase 3: Self-Contained Skills (Inline Tools)
1. Create MCP tool for tool registration
2. Parse skill YAML for tool definitions
3. Dynamically register tools at skill load time
4. Update skill library integration

### Phase 4: Developer Experience (Planning + Memory)
1. Implement approval gates for plan workflow
2. Add auto memory loading/saving
3. Formalize TDD skill workflow

---

## Questions for Design Discussion

1. **Hooks architecture:** Should hooks be in ACP protocol or llama-agent internal?
2. **Subagent scope:** Should child agents share parent session or be independent?
3. **Inline tools:** Tool registration via MCP, or extend skill library protocol?
4. **Memory durability:** File-based (MEMORY.md), database, or both?
5. **Protocol compatibility:** Will changes break existing Claude Code integrations?

---

## References

- Detailed comparison: `/memory/llama-agent-vs-claude-code.md`
- ACP Spec: https://agentclientprotocol.com
- Conformance tests: `acp-conformance/` (6/6 passing)
