# Agent Tool + MCP Tasks Integration

How agent spawning maps to MCP's Tasks API (call-now-fetch-later pattern).

---

## Overview

Agents are spawned via task-augmented `tools/call` requests. This gives us:
- **Immediate response** with agent_id (non-blocking)
- **Polling via tasks/get** to monitor progress
- **Blocking result retrieval via tasks/result** when parent wants to wait
- **Standard MCP mechanism** for task cancellation and lifecycle

---

## Task Lifecycle for Spawn Agent

### 1. Parent Spawns Agent (Call-Now)

Parent calls agent tool with task augmentation:

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
    },
    "task": {
      "ttl": 3600000
    }
  }
}
```

**Key fields:**
- `task` parameter with `ttl` (time-to-live in milliseconds)
- ttl=3600000 means keep task/results for 1 hour after creation

### 2. Server Accepts Immediately (Returns Task ID)

Server returns `CreateTaskResult` immediately:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "task": {
      "taskId": "agent-review-auth-123",
      "status": "working",
      "statusMessage": "Agent initializing and beginning analysis",
      "createdAt": "2026-03-09T14:30:00Z",
      "lastUpdatedAt": "2026-03-09T14:30:00Z",
      "ttl": 3600000,
      "pollInterval": 5000
    }
  }
}
```

**Key points:**
- Returns immediately (non-blocking)
- Status starts as `working`
- taskId is the agent_id
- pollInterval suggests polling frequency (5 seconds)
- No actual tool result yet — that comes later

### 3. Parent Polls for Progress (Fetch-While-Working)

Parent calls `tasks/get` to check progress:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tasks/get",
  "params": {
    "taskId": "agent-review-auth-123"
  }
}
```

**Response (still working):**

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "taskId": "agent-review-auth-123",
    "status": "working",
    "statusMessage": "Completed security analysis, now checking performance",
    "createdAt": "2026-03-09T14:30:00Z",
    "lastUpdatedAt": "2026-03-09T14:32:15Z",
    "ttl": 3600000,
    "pollInterval": 5000
  }
}
```

Parent can **continue doing other work** while polling. No blocking.

### 4. Agent Sends Notifications During Execution

While task is `working`, agent calls `notify agent`:

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
    },
    "_meta": {
      "io.modelcontextprotocol/related-task": {
        "taskId": "agent-review-auth-123"
      }
    }
  }
}
```

**Key:** The `_meta.io.modelcontextprotocol/related-task` field associates the notification with the task.

Parent receives notification in its stream while continuing to poll tasks/get.

### 5. Task Completes

Eventually, task reaches terminal status:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tasks/get",
  "params": {
    "taskId": "agent-review-auth-123"
  }
}
```

**Response (completed):**

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "taskId": "agent-review-auth-123",
    "status": "completed",
    "statusMessage": "Analysis complete. Found 3 critical, 5 warning issues.",
    "createdAt": "2026-03-09T14:30:00Z",
    "lastUpdatedAt": "2026-03-09T14:35:45Z",
    "ttl": 3600000,
    "pollInterval": 5000
  }
}
```

Status is now `completed` (terminal).

### 6. Parent Retrieves Final Result (Fetch-Now)

Once complete, parent calls `tasks/result` to get the actual agent output:

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tasks/result",
  "params": {
    "taskId": "agent-review-auth-123"
  }
}
```

**Response (final result):**

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "## Code Review Summary\n\n### Critical Issues (3)\n1. SQL injection at auth.rs:234\n2. Session token not invalidated at auth.rs:456\n3. ...\n\n### Warnings (5)\n...\n\nReview complete. Total findings: 8."
      }
    ],
    "isError": false,
    "_meta": {
      "io.modelcontextprotocol/related-task": {
        "taskId": "agent-review-auth-123"
      }
    }
  }
}
```

This is the actual tool result (what `tools/call` would normally return).

---

## Task Statuses and Meanings for Agents

### working
Agent is initializing or actively analyzing. Parent can:
- Poll via `tasks/get` for progress updates
- Receive notifications from agent via `notify agent`
- Optionally cancel via `tasks/cancel`

### input_required
Agent is blocked and needs input from parent. Parent should:
- Call `tasks/result` to receive elicitation/input requests
- Respond to those requests via the `notify agent` tool
- Agent will transition back to `working` when it gets the input

Example: Agent analyzing code needs clarification about which modules to review.

### completed
Agent finished successfully. Parent can:
- Call `tasks/result` to get final output
- Call `tasks/get` to see the status
- Results are retained for `ttl` duration

### failed
Agent encountered an error. Parent can:
- Call `tasks/result` to get error details
- Results are retained for `ttl` duration

### cancelled
Parent cancelled the agent. Parent can:
- Call `tasks/result` to see partial results if available
- Results may be deleted immediately or after ttl expires

---

## Comparison: With vs Without Tasks

### Without Tasks (Bad)
```
Parent: "Spawn agent"
Server: (Waits for agent to complete... blocks for minutes)
Server: (Finally returns results)
Parent: Can now continue
```
Problem: Parent blocked for entire agent execution.

### With Tasks (Good)
```
Parent: "Spawn agent with task"
Server: (Returns immediately with taskId)
Parent: (Gets control back instantly)
Parent: Can do other work while agent runs
Parent: Polls tasks/get to check progress
Parent: Receives notifications about findings
Parent: When ready, calls tasks/result to get full output
```
Benefit: Parent can be responsive and handle multiple agents in parallel.

---

## Multi-Agent Orchestration Example

```
Parent spawns 3 agents in parallel:

Call 1: spawn agent (code-reviewer) → returns taskId-1 immediately
Call 2: spawn agent (security-auditor) → returns taskId-2 immediately
Call 3: spawn agent (performance-analyst) → returns taskId-3 immediately

Parent: All 3 tasks now running in parallel

Parent continues:
  Poll tasks/get taskId-1 → status: working
  Poll tasks/get taskId-2 → status: working
  Poll tasks/get taskId-3 → status: working

Meanwhile receive notifications:
  - taskId-1 notification: "Found SQL injection"
  - taskId-3 notification: "Performance analysis complete"
  - taskId-2 notification: "3 security vulnerabilities found"

Continue polling:
  Poll tasks/get taskId-1 → status: completed
  Poll tasks/get taskId-2 → status: completed
  Poll tasks/get taskId-3 → status: completed

Retrieve final results:
  tasks/result taskId-1 → detailed code review
  tasks/result taskId-2 → security audit report
  tasks/result taskId-3 → performance analysis

Synthesize all findings into final report.
```

All 3 agents ran in parallel. Parent never blocked.

---

## Server-Side Implementation Notes

### Agent Execution in Background

When server receives `spawn agent` with task augmentation:

```rust
// 1. Accept request immediately
let agent_id = generate_unique_id();
let task = Task::new(
  taskId: agent_id,
  status: "working",
  ttl: params.task.ttl,
  pollInterval: 5000
);
storage.store_task(task);

// 2. Return immediately to client
return CreateTaskResult { task };

// 3. Spawn agent execution in background
tokio::spawn(async move {
  let result = run_agent(agent_id, params).await;
  // Update task status
  task.status = if result.is_err() { "failed" } else { "completed" };
  task.result = result;
  storage.update_task(task);
});
```

### Handling Notifications from Agent

When agent calls `notify agent`:

```rust
// Agent's session S2 calls notify agent
// Server receives notification tool call in S2's context

let notification = extract_notification_data(arguments);

// Add related-task metadata
notification._meta = {
  "io.modelcontextprotocol/related-task": {
    "taskId": agent_id
  }
};

// Route to parent's MCP session (S1)
parent_session.send_notification(notification);

// Update task status message if needed
task.statusMessage = notification.message;
task.lastUpdatedAt = now();
storage.update_task(task);
```

### Task Cancellation

When parent calls `tasks/cancel`:

```rust
let task = storage.get_task(taskId);

if task.status in ["completed", "failed", "cancelled"] {
  return error(-32602, "Cannot cancel terminal task");
}

// Signal agent to stop
agent_runtime.cancel(taskId);

// Update task
task.status = "cancelled";
task.lastUpdatedAt = now();
storage.update_task(task);

return task;
```

---

## Integration with Existing Agent Patterns

### List Agents

`list agents` is a **tool operation** that the parent invokes via `tools/call`.
The server implementation internally queries `tasks/list` and returns the results.
The parent does NOT call `tasks/list` directly — it uses the agent tool interface.

**Parent calls:**

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "list agents"
    }
  }
}
```

**Server implementation internally uses `tasks/list`:**

```rust
// Server receives tools/call with op: "list agents"
// Internally queries tasks/list to get all agent tasks
let tasks = storage.list_tasks();

// Returns agent-friendly response via tools/call result
return {
  "agents": tasks.iter().map(|t| {
    AgentInfo {
      agent_id: t.taskId,
      status: t.status,
      message: t.statusMessage,
      created_at: t.createdAt,
    }
  }).collect()
};
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\"agents\": [{\"agent_id\": \"agent-review-auth-123\", \"status\": \"working\", \"message\": \"Analyzing performance\"}, {\"agent_id\": \"agent-test-456\", \"status\": \"completed\", \"message\": \"Tests passed\"}]}"
      }
    ]
  }
}
```

### Terminate Agent

`kill agent` is a **tool operation** that the parent invokes via `tools/call`.
The server implementation internally calls `tasks/cancel` to stop the agent.
The parent does NOT call `tasks/cancel` directly — it uses the agent tool interface.

**Parent calls:**

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "kill agent",
      "agent_id": "agent-review-auth-123"
    }
  }
}
```

**Server implementation internally uses `tasks/cancel`:**

```rust
// Server receives tools/call with op: "kill agent"
// Internally calls tasks/cancel to stop the agent
let task = storage.get_task(agent_id);
let cancellation_result = cancel_task(agent_id);

return {
  "agent_id": agent_id,
  "status": cancellation_result.status,
  "message": "Agent killed"
};
```

### Get Agent Status

There is no separate `get agent` tool operation. Parents poll agent status directly
via the native MCP Tasks protocol method `tasks/get`. This avoids wrapping a standard
protocol method in a redundant tool call. See "Task Lifecycle for Spawn Agent" step 3
above for the `tasks/get` request/response format.

---

## TTL and Resource Cleanup

Server-side cleanup:

```rust
// Periodically (e.g., every 10 minutes):
async fn cleanup_expired_tasks() {
  let tasks = storage.list_all_tasks();
  let now = SystemTime::now();

  for task in tasks {
    let age = now.duration_since(task.createdAt);
    if age > Duration::from_millis(task.ttl) {
      storage.delete_task(task.id);
    }
  }
}
```

Default ttl: 1 hour (3600000 ms)
- Keeps results accessible for review
- Prevents indefinite storage
- Parent can request longer via ttl parameter

---

## Security Considerations

### Task ID Access Control

Task IDs are sensitive — only parent agent should access their own tasks:

```rust
// When parent calls tasks/get(taskId)
let task = storage.get_task(taskId);

// Verify parent_session_id matches task ownership
if task.parent_session_id != current_session.id {
  return error(-32602, "Task not found");  // Don't reveal existence
}

return task;
```

### Rate Limiting

Prevent brute-force enumeration of task IDs:

```rust
let parent_id = current_session.id;
if too_many_failed_lookups(parent_id) {
  // Rate limit this parent
  return error(-32603, "Rate limited");
}
```

---

## Benefits of MCP Tasks for Agents

1. **Standard Protocol** — Not custom, already defined by MCP spec
2. **Immediate Response** — Caller gets control back instantly
3. **Natural Parallelization** — Multiple tasks run concurrently
4. **Built-in Polling** — tasks/get provides progress
5. **Blocking Result** — tasks/result lets parent wait when ready
6. **Cancellation Support** — Standard tasks/cancel
7. **Resource Management** — TTL prevents indefinite storage
8. **Notifications Integration** — Agent notifications already carry task ID
9. **Multi-Session** — Tasks bind to parent session automatically
10. **Status Lifecycle** — working → input_required → completed/failed/cancelled

---

## Summary

Spawn agent = task-augmented tools/call:
- Parent: call with `task: { ttl: ... }`
- Server: return CreateTaskResult immediately
- Parent: poll tasks/get for progress
- Server: send notifications with related-task metadata
- Parent: call tasks/result to get final result
- Parent: call tasks/cancel to stop agent

This is the "call-now-fetch-later" pattern using standard MCP Tasks protocol.
