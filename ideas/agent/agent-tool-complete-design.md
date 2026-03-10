# Agent Tool: Complete Design

## Overview

This document consolidates the complete design for llama-agent's subagent system. Agents are tools (not special processes) that can be invoked in parallel, emit smart notifications, and allow result querying via grep/search.

**Core Design Principle:** Agents are like shell processes — spawn them, list them, kill them, query their output. No special fork/join logic or resources.

---

## Operations: Noun × Verb Matrix

| Verb | command | process | history | agent |
|------|---------|---------|---------|-------|
| spawn | | | | X |
| execute | X | | | |
| list | | X | | X |
| kill | | X | | X |
| search | | | X | X |
| grep | | | X | X |
| notify | | | | X |

### Valid Operations

#### Shell Tool (reference model)
- `execute command` — Run a shell command
- `list processes` — List running/completed shell commands with status
- `kill process` — Stop a running command
- `search history` — Semantic search of command output history (embeddings)
- `grep history` — Regex search of command output history

#### Agent Tool (new)
- `spawn agent` — Create and start a new subagent with task description
- `list agents` — List all agents (running, completed, failed) with status
- `kill agent` — Stop a running agent
- `search agent` — Semantic search of agent transcript (embeddings)
- `grep agent` — Regex search of agent transcript
- `notify agent` — Send notification to parent/root agent (with optional prompt injection)

---

## Session Routing: MCP Sessions and Notifications

The key insight: subagent MCP session ≠ parent agent MCP session. When a subagent calls `notify agent`, the notification routes back to the parent's session.

### Spawn Agent Call

Root agent's MCP session (S1):
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "spawn agent",
      "role": "code-reviewer",
      "task": "Review auth.rs for security vulnerabilities"
    }
  }
}
```

### Immediate Response (CreateTaskResult)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "task": {
      "taskId": "agent-review-auth-123",
      "status": "working",
      "statusMessage": "Agent initializing",
      "createdAt": "2026-03-09T14:30:00Z",
      "lastUpdatedAt": "2026-03-09T14:30:00Z",
      "ttl": 3600000,
      "pollInterval": 5000
    }
  }
}
```

Subagent is now running in session S2. Parent gets control back immediately.

### Subagent Calls notify agent

Subagent's MCP session (S2):
```json
{
  "jsonrpc": "2.0",
  "id": 45,
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "notify agent",
      "target": "root",
      "message": "Found critical SQL injection at auth.rs:234",
      "severity": "critical"
    }
  }
}
```

### Notification Routes Back to Parent

The `notify agent` tool handler:
1. Receives the call in S2's context
2. Looks up parent agent (root or specified target)
3. Routes notification to parent's MCP session (S1)

Parent receives in S1:
```json
{
  "jsonrpc": "2.0",
  "method": "notifications/message",
  "params": {
    "level": "warning",
    "logger": "agent:code-reviewer",
    "data": {
      "agent_id": "agent-review-auth-123",
      "type": "finding",
      "message": "Found critical SQL injection at auth.rs:234",
      "severity": "critical"
    }
  }
}
```

### Parent Injects Prompt Back

Parent's MCP session (S1):
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "notify agent",
      "target": "agent-review-auth-123",
      "message": "Can you check if this affects password reset?"
    }
  }
}
```

The tool handler:
1. Receives the call in S1's context
2. Looks up target agent in S2
3. Injects message into S2's message history
4. Subagent receives it and can respond

**Key point:** Sessions are separate (S1 ≠ S2), but notify agent routes messages between them. Like humans interjecting into agent reasoning.

---

## Result Handling

Spawn is **always non-blocking**. It returns a task handle (agent_id) immediately.
The final result is retrieved later via the MCP Tasks protocol (`tasks/result`).

1. **Task handle** — Returned immediately from `spawn agent` (non-blocking)
2. **Progress polling** — Parent calls `tasks/get` to check status
3. **Progress updates** — Arrive via smart notifications during execution
4. **Final result** — Retrieved via `tasks/result` when task status is `completed`
5. **Output querying** — Use `grep agent` or `search agent` to find things in transcript

```
Parent calls: spawn agent → returns task handle immediately (non-blocking)
During execution:
  - Parent polls tasks/get for status updates
  - Agent calls notify agent when it finds important things
  - Parent receives notifications (findings, questions, updates)
  - Parent can respond with injected prompts
After completion:
  - Parent calls tasks/result to retrieve final output
  - Parent can grep/search transcript anytime
```

---

## Agent Notifications via notify agent Tool

Agents call the `notify agent` tool when they discover something important. This sends an MCP notification to the parent agent's MCP session. The parent can optionally inject a prompt back via the same tool.

### Subagent Calls notify agent

```json
{
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "notify agent",
      "target": "root",
      "message": "Found critical SQL injection vulnerability at auth.rs:234",
      "severity": "critical",
      "metadata": {
        "type": "security",
        "location": "auth.rs:234",
        "description": "User input not escaped before SQL query"
      }
    }
  }
}
```

### Parent Receives (via MCP notifications)

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/message",
  "params": {
    "level": "warning",
    "logger": "agent:code-reviewer",
    "data": {
      "agent_id": "agent-review-auth-123",
      "type": "finding",
      "message": "Found critical SQL injection vulnerability at auth.rs:234",
      "severity": "critical",
      "location": "auth.rs:234",
      "description": "User input not escaped before SQL query"
    }
  }
}
```

### Parent Injects Prompt Back

Parent can optionally respond via `notify agent` with a prompt to inject into the subagent's session:

```json
{
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "notify agent",
      "target": "agent-review-auth-123",
      "message": "Can you also check if this affects the password reset flow?"
    }
  }
}
```

The subagent receives this as an injected message in its session, like a human interjection.

**Benefits:**
- Agents decide when they need to notify (not parsed from output)
- Notifications are normal MCP messages (not custom format)
- Parent can respond with follow-up prompts
- Like how Claude lets humans interject during reasoning
- Session routing handles the MCP plumbing automatically

---

## List Agents Response

Shows what agents are running and their status.

```json
{
  "agents": [
    {
      "agent_id": "agent-review-auth-123",
      "role": "code_reviewer",
      "status": "running",
      "progress": 67,
      "message": "Analyzing performance bottlenecks",
      "findings_count": 3,
      "created_at": "2026-03-08T14:32:00Z"
    },
    {
      "agent_id": "agent-test-suite-456",
      "role": "tester",
      "status": "completed",
      "findings_count": 2,
      "failures": 1,
      "created_at": "2026-03-08T14:25:00Z",
      "completed_at": "2026-03-08T14:31:00Z"
    }
  ]
}
```

---

## Grep Agent

Query agent transcript by regex or pattern.

```json
{
  "op": "grep agent",
  "agent_id": "agent-review-auth-123",
  "pattern": "SQL|injection|vulnerability"
}
```

Response:

```json
{
  "matches": [
    {
      "line_number": 234,
      "match": "SQL injection vulnerability found in query construction",
      "context_before": "Checking database interactions...",
      "context_after": "User input not properly escaped"
    }
  ],
  "total_matches": 3,
  "agent_id": "agent-review-auth-123"
}
```

---

## Search Agent

Semantic search of agent transcript (embeddings-based).

```json
{
  "op": "search agent",
  "agent_id": "agent-review-auth-123",
  "query": "security vulnerabilities found",
  "top_k": 5
}
```

Response:

```json
{
  "results": [
    {
      "score": 0.92,
      "chunk": "Found SQL injection in auth.rs:234. User input not escaped.",
      "chunk_id": "chunk-123",
      "finding_type": "security"
    }
  ],
  "agent_id": "agent-review-auth-123"
}
```

---

## Parallel Agent Execution

Multiple agents can be spawned in parallel. The RequestQueue handles concurrent model access.

```
Parent Agent:
├─ Call spawn agent (code-reviewer, auth.rs)
│  → agent-1 pending
├─ Call spawn agent (security-auditor, crypto.rs)
│  → agent-2 pending
├─ Call spawn agent (performance-auditor, storage.rs)
│  → agent-3 pending
├─ Subscribe to notifications
│  ← agent-1 finding: critical SQL injection
│  ← agent-3 finding: O(n²) loop
│  ← agent-2 analysis complete
│  ← agent-1 analysis complete
├─ Call list agents
│  ← {agent-1: completed, agent-2: completed, agent-3: completed}
├─ Call grep agent for each to verify findings
└─ Synthesize all findings into report
```

RequestQueue internally queues all generation requests. Each agent runs to completion (or is killed) before next one starts. From parent's perspective, they're parallel.

---

## Agent System Prompt Template

Agents receive a system prompt that teaches them to use the `notify agent` tool for findings and progress updates. The prompt template is defined in `ideas/agent-system-prompt-design.md`.

**Key point:** Agents call `notify agent` directly as a tool rather than emitting text blocks that get parsed. This is cleaner, more structured, and uses the same MCP tool machinery as everything else.

---

## Existing Agent Tool (Upgrade Target)

The agent tool already exists at `swissarmyhammer-tools/src/mcp/tools/agent/mod.rs` with:
- `AgentMcpTool` implementing `McpTool` trait
- Existing operations: `list agent`, `use agent`, `search agent`
- Uses `AgentLibrary`, `PromptLibrary`, `AgentContext`
- Registered in tool registry via `register_agent_tools()`

This is an **upgrade** to the existing tool. New operations (`spawn agent`, `kill agent`, `notify agent`, `grep agent`) extend the existing `AGENT_OPERATIONS` array. The `use agent` operation may be deprecated or repurposed once `spawn agent` is implemented.

---

## Implementation Phases

### Phase 1: Extend Agent Tool with New Operations
- [ ] Add `SpawnAgent`, `KillAgent`, `NotifyAgent`, `GrepAgent` operation structs
- [ ] Register in `AGENT_OPERATIONS` alongside existing `list`, `use`, `search`
- [ ] Extend `AgentOperation` enum with new variants
- [ ] Update schema generation

### Phase 2: Agent Session & State Management
- [ ] Create agent session infrastructure (parent-child relationships)
- [ ] Agent status tracking (Pending, Running, Completed, Failed, Killed)
- [ ] MCP Tasks integration for non-blocking spawn

### Phase 3: Notify Agent & Session Routing
- [ ] Implement `notify agent` tool handler (subagent → parent direction)
- [ ] Implement session-to-session routing (S2 notification → S1 delivery)
- [ ] Emit MCP `notifications/message` to parent session
- [ ] Support parent → subagent prompt injection via `notify agent`

### Phase 4: Result Querying
- [ ] Implement `grep agent` operation (regex on transcript)
- [ ] Enhance existing `search agent` for transcript semantic search
- [ ] Transcript storage and indexing

### Phase 5: Agent Lifecycle & Cleanup
- [ ] Implement `kill agent` operation (wraps tasks/cancel)
- [ ] Proper cleanup on kill (session, transcript, resources)
- [ ] System prompt integration for subagent roles

---

## Llama-Agent vs Claude Code: Agent Dialog

This is where llama-agent has a **genuine architectural advantage** over Claude Code.

### Claude Code
- Parent spawns agents
- Agents run to completion independently
- Parent gets final result
- One-way communication (no back-and-forth)
- Agent can't ask for clarification or redirect

### Llama-Agent
- Parent spawns agents
- Agents can call `notify agent` to notify parent **during execution**
- Parent receives notification in real-time
- Parent can inject prompt back into agent's session
- **Agents and parents can dialog and collaborate**
- Parent can redirect agent mid-course, ask follow-ups, or provide additional context

**Example Claude Code workflow:**
```
Parent: Spawn code reviewer
Code reviewer: Analyzes entire file, returns findings
Parent: Gets results (no interaction during)
```

**Example Llama-Agent workflow:**
```
Parent: Spawn code reviewer
Code reviewer: Finds SQL injection, calls notify agent
Parent: Receives notification, reviews context
Parent: Injects prompt "Check if password reset is affected"
Code reviewer: Receives prompt, pivots analysis
Code reviewer: Calls notify agent with updated findings
Parent: Gets enriched results from dialog
```

This makes agent coordination more **natural, interactive, and controllable**. Agents aren't black boxes — they're collaborators.

---

## Key Differences from Initial Design

1. **No MCP Resources** — Results retrieved via `tasks/result`, no separate resource get call
2. **Immediate agent_id** — Returned from spawn call as a task handle (non-blocking)
3. **No FINDING/PHASE blocks** — Agents use `notify agent` tool instead of parsing output
4. **Agent dialog** — Parent can inject prompts back, agents can ask follow-ups
5. **No get agent operation** — Parents poll via native `tasks/get` directly
6. **Query-based access** — grep/search instead of pulling whole output
7. **No separate result storage** — Output is transcript in agent session

---

## Integration Points

- **With hooks** — Agents can emit hook events (PreToolUse, PostToolUse)
- **With RequestQueue** — Agents queue their generation requests for serialized model access
- **With ACP** — Agents are MCP tools that follow operation/schema pattern
- **With notifications** — Smart findings extracted and broadcast to parent
- **With skills** — Skills can spawn agents for delegation

---

## Examples: Common Patterns

### Code Review Delegation
```
Parent: "Review this PR"
→ spawn agent (code-reviewer, "Review PR #42 for security")
← agent-review-123 pending
← notification: "Code review started"
[agent analyzes, emits findings]
← notification: "Found 3 security issues"
← notification: "Review complete"
Parent: grep agent (agent-review-123, "security|critical")
← 3 matches from transcript
```

### Parallel Test Runs
```
Parent: "Run all test suites"
→ spawn agent (test-suite-1, "Run unit tests")
→ spawn agent (test-suite-2, "Run integration tests")
→ spawn agent (test-suite-3, "Run e2e tests")
← agent-1, agent-2, agent-3 pending
[All 3 run in parallel, RequestQueue serializes model calls]
← notifications from all 3 as tests complete
Parent: list agents → {all 3: completed, pass/fail status}
```

### Persistent Researcher Agent
```
Parent: "Research Rust security best practices"
→ spawn agent (researcher, task="Research Rust security best practices")
← agent-research-123 pending
← notifications as research progresses

Later:
Parent: "Based on your research, what about async safety?"
→ notify agent (target=agent-research-123, message="What about async safety?")
← agent receives injected prompt, continues with context from previous research
```
