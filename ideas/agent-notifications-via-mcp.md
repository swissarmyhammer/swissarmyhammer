# Agent Notifications via MCP: Architecture Recommendation

## Executive Summary

Use **MCP Tasks (call-now-fetch-later) + Resource Subscriptions + Logging Notifications** for agent coordination.

This pattern leverages MCP's native capabilities without custom parallelization logic.

---

## Recommended Architecture

### 1. Agents as MCP Tasks

When a parent agent calls `spawn_agent(role, task)`:

```json
// Parent calls
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "name": "spawn_agent",
  "arguments": {
    "role": "code-reviewer",
    "task": "Review auth.rs for security vulnerabilities",
    "sampling_strategy": {
      "temperature": 0.7,
      "top_p": 0.95
    }
  },
  "_meta": {
    "task": true  // Indicates this is a long-running task
  }
}

// Immediate response with task ID
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "task": {
      "id": "agent-review-auth-123",
      "status": "pending"
    }
  }
}
```

**Key advantage:** Parent gets immediate response, doesn't block. Multiple spawns happen naturally in parallel.

### 2. Agent Progress via Logging Notifications

As the agent works, it emits structured logging notifications:

```json
// Agent discovers SQL injection risk
{
  "jsonrpc": "2.0",
  "method": "notifications/message",
  "params": {
    "level": "warning",
    "logger": "agent:code-reviewer",
    "message": "SQL injection vulnerability in query construction",
    "_meta": {
      "agent_task_id": "agent-review-auth-123",
      "finding_type": "security",
      "severity": "critical",
      "location": "auth.rs:234",
      "description": "User input not properly escaped in SQL query"
    }
  }
}

// Agent switches analysis phase
{
  "jsonrpc": "2.0",
  "method": "notifications/message",
  "params": {
    "level": "info",
    "logger": "agent:code-reviewer",
    "message": "Completed security review, starting performance analysis",
    "_meta": {
      "agent_task_id": "agent-review-auth-123",
      "phase_transition": "security → performance",
      "findings_so_far": 3
    }
  }
}
```

**Key advantage:** Parent agent receives smart updates, not raw tokens. Can display progress in real-time without implementing custom streaming.

### 3. Query Results via Resources

Agent results are exposed as queryable resources:

```json
// Parent polls task status
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tasks/get",
  "params": {
    "id": "agent-review-auth-123"
  }
}

// Response shows progress
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "id": "agent-review-auth-123",
    "status": "working",
    "progress": 67,
    "total": 100,
    "message": "Analyzing performance bottlenecks"
  }
}

// When complete, parent can query full results
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "resources/read",
  "params": {
    "uri": "agent://results/agent-review-auth-123"
  }
}

// Returns complete analysis
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "contents": [
      {
        "uri": "agent://results/agent-review-auth-123",
        "mimeType": "application/json",
        "text": {
          "summary": "Found 3 security issues, 1 performance issue",
          "findings": [
            {
              "type": "security",
              "severity": "critical",
              "description": "SQL injection in query builder",
              "location": "auth.rs:234"
            },
            // ... more findings ...
          ],
          "duration_ms": 4521,
          "tokens_used": 2847
        }
      }
    ]
  }
}
```

**Key advantage:** Parent can query results on-demand, no streaming overhead. Results persist for later inspection.

### 4. Parent Agent Orchestration

Parent agent naturally coordinates multiple agents:

```text
Parent Agent (executing)
│
├─ Call spawn_agent(code-reviewer, ...)
│  → task-review-1
│
├─ Call spawn_agent(security-auditor, ...)
│  → task-review-2
│
├─ Call spawn_agent(performance-auditor, ...)
│  → task-review-3
│
├─ Subscribe to notifications from all agents
│  ← Receives smart updates as each discovers findings
│
├─ Call list_tasks() to check progress
│  ← {task-review-1: 70%, task-review-2: 45%, task-review-3: 90%}
│
├─ Wait for all to complete (poll or wait on notifications)
│
├─ Call get_agent_summary(task-review-1)
├─ Call get_agent_summary(task-review-2)
├─ Call get_agent_summary(task-review-3)
│
└─ Synthesize into final report
```

---

## System Prompt Integration

Agent system prompts instruct the model on how to emit smart notifications:

```yaml
# .agents/code-reviewer/AGENT.md
---
name: code-reviewer
role: CodeReviewer
---

You are a code reviewer. As you work, emit findings via structured thinking:

## Finding Format
When you discover something important, state it clearly:

FINDING: [severity: critical|warning|info]
TYPE: [security|performance|style|best-practice]
LOCATION: [file:line]
DESCRIPTION: [what you found]
CONTEXT: [why it matters]

Examples:
- FINDING: [severity: critical]
  TYPE: security
  LOCATION: auth.rs:234
  DESCRIPTION: SQL injection in query construction
  CONTEXT: User input not escaped before SQL query

- FINDING: [severity: warning]
  TYPE: performance
  LOCATION: parser.rs:89
  DESCRIPTION: O(n²) loop processing tokens
  CONTEXT: Should use HashMap lookup instead

## Progress Updates
Periodically state what phase you're in:

PHASE: [analyzing|testing|documenting|complete]
PROGRESS: [X% or description]

Examples:
- PHASE: analyzing
  PROGRESS: Completed security review, now checking performance

- PHASE: complete
  PROGRESS: Review finished. Found 3 critical, 2 warning issues.

---

Analyze the provided code systematically...
```

This teaches the model to emit parseable findings that llama-agent can:
1. Extract and convert to logging notifications
2. Aggregate into a structured summary
3. Expose to the parent agent for querying

---

## Implementation in Llama-Agent

### 1. Add MCP Tasks Support

```rust
// In acp/tasks.rs (new)
pub struct ManagedAgentTask {
    pub task_id: String,
    pub agent_role: AgentRole,
    pub session_id: SessionId,
    pub status: TaskStatus,  // pending, working, completed, failed, cancelled
    pub progress: Option<Progress>,
    pub findings: Vec<Finding>,
    pub created_at: Instant,
}

pub async fn spawn_agent_as_task(
    request: SpawnAgentRequest,
) -> Result<ManagedAgentTask> {
    // Create session
    // Return immediately with task_id
    // Queue generation in background
}
```

### 2. Extract Findings from Agent Output

```rust
// In acp/notifications.rs (new)
pub async fn monitor_agent_stream(
    task_id: String,
    session_id: SessionId,
    agent_server: &AgentServer,
    notification_tx: &broadcast::Sender<AgentNotification>,
) {
    // Subscribe to session messages
    let mut receiver = agent_server.session_manager.watch(session_id);

    while let Some(message) = receiver.recv().await {
        // Parse FINDING blocks from agent output
        for finding in parse_findings(&message.content) {
            notification_tx.send(AgentNotification::Finding {
                task_id: task_id.clone(),
                severity: finding.severity,
                description: finding.description,
                location: finding.location,
            }).ok();
        }

        // Parse PHASE transitions
        for phase in parse_phases(&message.content) {
            notification_tx.send(AgentNotification::PhaseChange {
                task_id: task_id.clone(),
                phase: phase,
            }).ok();
        }
    }
}
```

### 3. Expose Results as Resources

```rust
// In acp/resources.rs (extend existing)
pub async fn get_agent_result(task_id: &str) -> Result<ResourceValue> {
    let task = TASK_MANAGER.get_task(task_id)?;
    let session = AGENT_SERVER.session_manager.load(task.session_id)?;

    // Summarize findings
    let summary = create_summary(&task, &session);

    Ok(ResourceValue {
        uri: format!("agent://results/{}", task_id),
        mimeType: "application/json",
        text: serde_json::to_string(&summary)?,
    })
}
```

### 4. Support Task Queries

```rust
// MCP protocol handlers
pub async fn handle_tasks_get(id: String) -> TaskStatus {
    TASK_MANAGER.get_status(&id)
}

pub async fn handle_tasks_list() -> Vec<TaskStatus> {
    TASK_MANAGER.list_active()
}

pub async fn handle_tasks_cancel(id: String) -> Result<()> {
    TASK_MANAGER.cancel(&id)
}

pub async fn handle_resources_read(uri: &str) -> ResourceValue {
    // Routes to agent results
    if uri.starts_with("agent://results/") {
        get_agent_result(&uri[16..]).await?
    }
}
```

---

## Calling Agent Interface

Parent agents use standard MCP calls:

```python
# Spawn 3 agents in parallel (natural parallelization)
task1 = client.call_tool("spawn_agent", {
    "role": "code-reviewer",
    "task": "Review auth.rs"
})

task2 = client.call_tool("spawn_agent", {
    "role": "security-auditor",
    "task": "Check for vulnerabilities"
})

task3 = client.call_tool("spawn_agent", {
    "role": "performance-auditor",
    "task": "Find performance issues"
})

# Subscribe to notifications
unsub = client.subscribe_notifications(
    filter=lambda n: n.get("_meta", {}).get("agent_task_id")
                     in [task1, task2, task3]
)

# Poll for completion (or wait on notifications)
while True:
    statuses = client.call_tool("list_tasks", {})
    if all(s["status"] == "completed" for s in statuses):
        break
    await asyncio.sleep(1)

# Query results
result1 = client.call_tool("resources/read", {
    "uri": f"agent://results/{task1['task']['id']}"
})

# Synthesize findings
all_findings = [json.loads(r)["findings"] for r in [result1, result2, result3]]
summary = synthesize(all_findings)
```

---

## Advantages of This Approach

| Aspect | Benefit |
|--------|---------|
| **Parallelization** | Natural via multiple tool calls, no fork/join logic |
| **Notifications** | Structured via MCP logging, not token streaming |
| **Querying** | Async results via resources and tasks APIs |
| **Resilience** | Task IDs survive connection loss |
| **Scalability** | No custom state machines, uses MCP standards |
| **Integration** | Parent agents use familiar tool call interface |
| **Monitoring** | Standard MCP progress notifications |

---

## Differences from Claude Code

| Aspect | Claude Code | Llama-Agent (Proposed) |
|--------|------------|----------------------|
| **Parallelization** | Built into Claude backend | Natural MCP parallelization |
| **Notifications** | Internal to Claude | Standard MCP notifications |
| **Result Querying** | Implicit in conversation | Explicit via resources/tasks APIs |
| **Agent Communication** | Black box | Transparent via MCP |
| **Extensibility** | Not user-extensible | Full MCP control |

---

## Implementation Checklist

- [ ] Define `ManagedAgentTask` struct and task lifecycle
- [ ] Implement `spawn_agent()` tool returning task ID
- [ ] Add task monitoring loop extracting FINDING/PHASE blocks
- [ ] Emit logging notifications for key findings
- [ ] Expose agent results as Resources (agent://results/{id})
- [ ] Implement tasks/get, tasks/list, tasks/cancel handlers
- [ ] Update agent system prompts to include FINDING format
- [ ] Test parallel agent spawning and result querying
- [ ] Add parent agent examples showing orchestration
