# Agent Notification Types

Concrete specifications for MCP logging notifications emitted when subagents call `notify agent`.

---

## MCP Logging Notification Envelope

When a subagent calls `notify agent`, the tool handler creates an MCP logging notification following RFC 5424 severity levels:

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
      "message": "[human-readable summary]",
      "[category-specific fields]": "..."
    }
  }
}
```

### Standard Fields

**Required (per MCP spec):**
- `level` — String: "debug" | "info" | "notice" | "warning" | "error" | "critical" | "alert" | "emergency"
- `logger` — String: "agent:{role}" (e.g., "agent:code-reviewer", "agent:tester")
- `data` — Object: Arbitrary JSON-serializable content

**Data Object Fields:**
- `agent_id` — String: ID of subagent sending notification
- `type` — String: Notification category (finding, question, update, completion)
- `message` — String: Human-readable summary (1-2 sentences)
- `timestamp` — ISO8601: When notification was sent
- `[category-specific fields]` — Additional fields based on notification type

---

## Notification Types by Category

### Type: "finding" (Most Common)

Agent found an issue or important observation.

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/message",
  "params": {
    "level": "critical",
    "logger": "agent:code-reviewer",
    "data": {
      "agent_id": "agent-review-auth-123",
      "type": "finding",
      "message": "Found critical SQL injection vulnerability at auth.rs:234",
      "finding_type": "security",
      "severity": "critical",
      "location": "auth.rs:234",
      "description": "User input not properly escaped before SQL query",
      "remediation": "Use parameterized queries (prepared statements)",
      "timestamp": "2026-03-09T14:32:00Z"
    }
  }
}
```

**Metadata fields:**
- `finding_type` — String: Category of finding (security, performance, style, test-failure, coverage-gap, etc.)
- `severity` — String: "critical" | "warning" | "info" (maps to MCP level)
- `location` — String: File path and line number (e.g., "auth.rs:234" or "tests/auth_test.rs:45-67")
- `description` — String: Detailed description of the finding
- `remediation` — String (optional): Suggested fix or action
- `context` — String (optional): Additional context or reasoning

**MCP level mapping:**
- severity: "critical" → level: "critical"
- severity: "warning" → level: "warning"
- severity: "info" → level: "info"

**Examples by role:**

**Code Reviewer - Security Finding:**
```json
{
  "level": "critical",
  "logger": "agent:code-reviewer",
  "data": {
    "agent_id": "agent-review-auth-123",
    "type": "finding",
    "message": "SQL injection in query construction at auth.rs:234",
    "finding_type": "security",
    "severity": "critical",
    "location": "auth.rs:234",
    "description": "User input directly interpolated into SQL string",
    "remediation": "Use prepared statements with parameterized queries",
    "cwe": "CWE-89: SQL Injection"
  }
}
```

**Code Reviewer - Performance Finding:**
```json
{
  "level": "warning",
  "logger": "agent:code-reviewer",
  "data": {
    "agent_id": "agent-review-auth-123",
    "type": "finding",
    "message": "O(n²) loop detected in token processing at parser.rs:89",
    "finding_type": "performance",
    "severity": "warning",
    "location": "parser.rs:89-95",
    "description": "Nested loop over tokens causes exponential processing time",
    "remediation": "Replace linear search with HashMap for O(1) lookup",
    "impact": "Processes 100KB files in seconds instead of minutes"
  }
}
```

**Tester - Test Failure:**
```json
{
  "level": "critical",
  "logger": "agent:tester",
  "data": {
    "agent_id": "agent-test-suite-456",
    "type": "finding",
    "message": "test_sql_injection_protection FAILED",
    "finding_type": "test-failure",
    "severity": "critical",
    "location": "tests/auth_test.rs:234",
    "test_name": "test_sql_injection_protection",
    "error_message": "Expected query to be parameterized",
    "error_type": "AssertionError"
  }
}
```

**Tester - Coverage Gap:**
```json
{
  "level": "warning",
  "logger": "agent:tester",
  "data": {
    "agent_id": "agent-test-suite-456",
    "type": "finding",
    "message": "Untested error handling path in auth.rs:150-180",
    "finding_type": "coverage-gap",
    "severity": "warning",
    "location": "auth.rs:150-180",
    "description": "Fallback auth mechanism has zero test coverage",
    "remediation": "Add tests for when primary auth fails",
    "lines_affected": 30
  }
}
```

---

### Type: "question" (Agent Needs Clarification)

Agent needs parent's input to proceed.

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/message",
  "params": {
    "level": "notice",
    "logger": "agent:code-reviewer",
    "data": {
      "agent_id": "agent-review-auth-123",
      "type": "question",
      "message": "Ambiguous requirement: Should I review test files or only production code?",
      "question": "Should I review test files or only production code?",
      "options": ["production-only", "production-and-tests", "tests-only"],
      "blocking": true,
      "timestamp": "2026-03-09T14:32:30Z"
    }
  }
}
```

**Metadata fields:**
- `question` — String: The question being asked
- `options` — Array (optional): Suggested answers or choices
- `blocking` — Boolean: Whether agent is blocked and waiting for response
- `context` — String (optional): Why the question came up

**Examples:**

**Clarification needed:**
```json
{
  "level": "notice",
  "logger": "agent:code-reviewer",
  "data": {
    "agent_id": "agent-review-auth-123",
    "type": "question",
    "message": "Should I check the password reset flow as well?",
    "question": "Found SQL injection in login. Should I also review password reset (different module)?",
    "context": "Both modules use similar query patterns",
    "blocking": false
  }
}
```

**Blocked on decision:**
```json
{
  "level": "notice",
  "logger": "agent:researcher",
  "data": {
    "agent_id": "agent-researcher-789",
    "type": "question",
    "message": "Which version of the framework should I research?",
    "question": "Framework has 2 major versions in active use. Which should I focus on?",
    "options": ["v3.x (legacy)", "v4.x (current)", "both"],
    "blocking": true,
    "context": "Analysis scope depends on version choice"
  }
}
```

---

### Blocking Question Mechanism

When an agent sends a question with `blocking: true`, the `notify agent` tool call
**blocks** (does not return) until the parent responds. This integrates with MCP Tasks
to provide a structured input-required flow.

**Lifecycle:**

1. Agent calls `notify agent` with `blocking: true`. The tool call blocks.
2. The MCP Tasks status for this agent transitions to `input_required`.
3. Parent sees `input_required` when polling via `tasks/get`.
4. Parent calls `tasks/result`, which returns the question payload.
5. Parent responds by calling `notify agent` with `target=<agent_id>` and the answer.
6. The agent's blocked `notify agent` call returns with the parent's response.
7. The MCP Tasks status transitions back to `working`.

**Timeout behavior:**

If the parent does not respond within a configurable timeout (default: 5 minutes),
the blocked `notify agent` call returns with a timeout indicator:

```json
{
  "timed_out": true,
  "message": "Parent did not respond within 300 seconds"
}
```

The agent can then decide to proceed with a default, retry the question, or abort.

**Example: Agent blocks on question**

Agent calls (this call blocks):
```json
{
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "notify agent",
      "target": "parent",
      "message": "Which version of the framework should I research?",
      "blocking": true,
      "options": ["v3.x", "v4.x", "both"]
    }
  }
}
```

Parent polls and sees `input_required`:
```json
{
  "taskId": "agent-researcher-789",
  "status": "input_required",
  "statusMessage": "Which version of the framework should I research?"
}
```

Parent responds:
```json
{
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "notify agent",
      "target": "agent-researcher-789",
      "message": "Focus on v4.x only."
    }
  }
}
```

Agent's blocked call returns:
```json
{
  "timed_out": false,
  "response": "Focus on v4.x only."
}
```

---

### Type: "update" (Progress or Milestone)

Agent is making progress, changing direction, or reaching a milestone.

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/message",
  "params": {
    "level": "info",
    "logger": "agent:code-reviewer",
    "data": {
      "agent_id": "agent-review-auth-123",
      "type": "update",
      "message": "Completed security analysis, now checking performance",
      "phase": "analyzing",
      "phase_transition": "security → performance",
      "progress": "67%",
      "findings_so_far": 3,
      "timestamp": "2026-03-09T14:33:00Z"
    }
  }
}
```

**Metadata fields:**
- `phase` — String: Current phase (analyzing, testing, synthesizing, etc.)
- `phase_transition` — String (optional): What changed (e.g., "security → performance")
- `progress` — String: Progress indicator (percentage, count, description)
- `context` — String (optional): What's being done now

**Examples:**

**Phase change:**
```json
{
  "level": "info",
  "logger": "agent:tester",
  "data": {
    "agent_id": "agent-test-suite-456",
    "type": "update",
    "message": "Unit tests passed. Starting integration tests.",
    "phase": "testing",
    "phase_transition": "unit → integration",
    "progress": "50%",
    "tests_passed": 234,
    "tests_total": 468
  }
}
```

**Milestone reached:**
```json
{
  "level": "info",
  "logger": "agent:researcher",
  "data": {
    "agent_id": "agent-researcher-789",
    "type": "update",
    "message": "Found 5 authoritative sources on Rust security best practices",
    "milestone": "source-gathering",
    "sources_found": 5,
    "progress": "Analyzing sources..."
  }
}
```

---

### Type: "completion" (Analysis or Task Complete)

Agent finished its work and is providing final summary.

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/message",
  "params": {
    "level": "info",
    "logger": "agent:code-reviewer",
    "data": {
      "agent_id": "agent-review-auth-123",
      "type": "completion",
      "message": "Code review complete. Found 3 critical security issues, 2 performance issues.",
      "status": "completed",
      "summary": {
        "total_findings": 5,
        "by_severity": {
          "critical": 3,
          "warning": 2,
          "info": 0
        },
        "by_type": {
          "security": 3,
          "performance": 2
        }
      },
      "duration_seconds": 87,
      "tokens_used": 3421,
      "timestamp": "2026-03-09T14:34:00Z"
    }
  }
}
```

**Metadata fields:**
- `status` — String: "completed" | "failed" | "cancelled"
- `summary` — Object (optional): Aggregate results
- `duration_seconds` — Number: How long the analysis took
- `tokens_used` — Number (optional): LLM tokens consumed
- `error` — String (optional, if failed): Error message
- `reason_killed` — String (optional, if killed): Why it was killed

**Examples:**

**Successful completion:**
```json
{
  "level": "info",
  "logger": "agent:tester",
  "data": {
    "agent_id": "agent-test-suite-456",
    "type": "completion",
    "message": "Test suite complete. 234/234 tests passed.",
    "status": "completed",
    "summary": {
      "total_tests": 234,
      "passed": 234,
      "failed": 0,
      "skipped": 0,
      "coverage_percent": 87.3
    },
    "duration_seconds": 156,
    "timestamp": "2026-03-09T14:34:00Z"
  }
}
```

**Failure:**
```json
{
  "level": "error",
  "logger": "agent:tester",
  "data": {
    "agent_id": "agent-test-suite-456",
    "type": "completion",
    "message": "Test suite failed. 234/234 tests passed, but 1 test FAILED.",
    "status": "failed",
    "summary": {
      "total_tests": 234,
      "passed": 233,
      "failed": 1,
      "skipped": 0
    },
    "first_failure": {
      "test_name": "test_sql_injection_protection",
      "location": "tests/auth_test.rs:234",
      "error": "Expected query to be parameterized"
    },
    "duration_seconds": 156,
    "timestamp": "2026-03-09T14:34:00Z"
  }
}
```

**Terminated by parent:**
```json
{
  "level": "warning",
  "logger": "agent:code-reviewer",
  "data": {
    "agent_id": "agent-review-auth-123",
    "type": "completion",
    "message": "Analysis killed by parent agent",
    "status": "killed",
    "reason_killed": "Parent injected: focus on auth module only",
    "findings_at_termination": 7,
    "progress_percent": 45,
    "duration_seconds": 32,
    "timestamp": "2026-03-09T14:32:45Z"
  }
}
```

---

## Notification Target Routing

The `target` field in `notify agent` determines where a notification is routed.
Only ancestors in the spawn tree are valid targets -- agents cannot send messages
to arbitrary agents.

### Valid Target Values

| Target | Resolves To | Description |
|--------|-------------|-------------|
| `"root"` | Top-level root agent | Always resolves to the outermost agent, regardless of nesting depth |
| `"parent"` | Immediate parent agent | The agent that spawned this agent |
| `"<agent_id>"` | Specific agent by ID | Must be an ancestor or sibling in the spawn tree |

### Routing Rules

1. **Ancestors only** -- An agent can target `root`, `parent`, or any ancestor agent
   by ID. It can also target a sibling (another agent spawned by the same parent).
2. **No downward messaging** -- A parent uses `notify agent` with `target=<agent_id>`
   to inject prompts into a child. Children cannot address arbitrary descendants.
3. **Sub-subagents can reach root** -- A deeply nested agent can target `"root"` to
   skip intermediate parents and notify the top-level agent directly.
4. **Invalid targets** -- If an agent specifies a target that is not an ancestor or
   sibling, the `notify agent` call returns an error.

### Example: Nested Agent Hierarchy

```
Root Agent (S1)
  ├── CodeReviewer (S2)     ← spawned by Root
  │   └── SecurityAuditor (S3)  ← spawned by CodeReviewer
  └── Tester (S4)           ← spawned by Root
```

Valid targets from SecurityAuditor (S3):
- `"root"` -- routes to Root (S1)
- `"parent"` -- routes to CodeReviewer (S2)
- `"agent-code-reviewer-id"` -- routes to CodeReviewer (S2) by ID

Invalid targets from SecurityAuditor (S3):
- `"agent-tester-id"` -- Tester is not an ancestor (it is a sibling of S3's parent, not of S3)

Valid targets from CodeReviewer (S2):
- `"root"` -- routes to Root (S1)
- `"parent"` -- routes to Root (S1)
- `"agent-tester-id"` -- Tester is a sibling (both spawned by Root)

---

## Severity Levels

Map from agent severity to MCP logging level:

| Agent Severity | MCP Level | MCP Integer | Meaning |
|---|---|---|---|
| critical | critical | 2 | Blocks deployment, security risk |
| warning | warning | 3 | Should be fixed, significant issue |
| info | info | 4 | Nice to have, suggestion |
| (blocking question) | notice | 5 | Agent waiting for input |
| (progress/update) | info | 4 | Informational progress |

---

## Notification Flow

1. **Subagent calls notify agent tool** (in its session S2)
2. **Tool handler extracts metadata** from the call arguments
3. **Tool handler creates MCP notification** with agent_id, metadata
4. **Notification routed to parent's MCP session** (S1)
5. **Parent receives notification** in its message stream
6. **Parent can respond** by calling notify agent with target=agent_id and message (prompt injection)

Example JSON-RPC flow:

```
S2 (Subagent):
→ {"jsonrpc": "2.0", "id": 45, "method": "tools/call", "params": {"name": "agent", "arguments": {...}}}

ACP Server (routes notification):
← {"jsonrpc": "2.0", "method": "notifications/message", "params": {...}}  [sent to S1]

S1 (Parent):
← receives notification

S1 (Parent response):
→ {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
   "params": {"name": "agent", "arguments": {"op": "notify agent", "target": "agent-xyz", "message": "..."}}}

ACP Server (injects message):
← {"jsonrpc": "2.0", "method": "notifications/message", "params": {...}}  [sent to S2]

S2 (Subagent):
← receives injected message in context
```

---

## Parent Agent Processing

When parent receives a notification, it can:

1. **Log it** — Store for later review
2. **React to it** — Change behavior based on finding type/severity
3. **Aggregate it** — Collect findings across multiple agents
4. **Respond** — Call notify agent with prompt injection
5. **Skip it** — Ignore if not relevant

Example parent logic:

```
receive notification:
  if notification.type == "finding" && notification.severity == "critical":
    # Critical findings should be acted on immediately
    inject_prompt(agent_id, "This is critical. Can you verify this finding?")

  if notification.type == "question" && notification.blocking:
    # Blocking questions need immediate response
    respond_with_answer(agent_id, notification.question, "...")

  if notification.type == "completion":
    # Completion, collect findings and move forward
    collect_findings(notification.summary)
    mark_agent_done(notification.agent_id)
```

---

## Special Cases

### Agent Spawns Another Agent

A subagent can spawn a sub-subagent, creating a hierarchy:

```
Root Agent (S1)
  ├─ CodeReviewer Agent (S2)
  │  └─ SecurityAuditor SubAgent (S3)
  └─ Tester Agent (S4)
```

Notifications flow up the chain:
- S3 calls notify agent (target: S2) → routes to S2's MCP session
- S2 can forward to S1 via notify agent (target: root)
- Root receives all notifications

### Aggregate Notifications

Parent can aggregate findings from multiple agents:

```json
{
  "level": "warning",
  "logger": "agent:parent",
  "data": {
    "type": "completion",
    "message": "Aggregate review complete across 3 reviewers",
    "aggregation": {
      "code_reviewer_1": { "findings": 3, "critical": 1 },
      "code_reviewer_2": { "findings": 2, "critical": 0 },
      "security_auditor": { "findings": 5, "critical": 3 }
    },
    "total_findings": 10,
    "total_critical": 4
  }
}
```

---

## Refined Examples by Scenario

### Scenario 1: Code Review with Follow-up

**Reviewer finds issue and notifies:**
```json
{
  "method": "notifications/message",
  "params": {
    "level": "critical",
    "logger": "agent:code-reviewer",
    "data": {
      "agent_id": "agent-review-1",
      "type": "finding",
      "message": "SQL injection found at auth.rs:234",
      "finding_type": "security",
      "severity": "critical",
      "location": "auth.rs:234",
      "description": "User input directly concatenated into SQL string",
      "remediation": "Use parameterized queries",
      "code_snippet": "query = f'SELECT * FROM users WHERE id = {user_input}'",
      "timestamp": "2026-03-09T14:32:00Z"
    }
  }
}
```

**Parent responds with follow-up:**
```json
{
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "notify agent",
      "target": "agent-review-1",
      "message": "Good catch. Does this pattern appear elsewhere in the auth module? Check lines 400-500."
    }
  }
}
```

**Reviewer continues and finds related issue:**
```json
{
  "method": "notifications/message",
  "params": {
    "level": "critical",
    "logger": "agent:code-reviewer",
    "data": {
      "agent_id": "agent-review-1",
      "type": "finding",
      "message": "Same SQL injection pattern at auth.rs:456",
      "finding_type": "security",
      "severity": "critical",
      "location": "auth.rs:456",
      "related_to": "agent-review-1",
      "description": "Session lookup uses same unsafe pattern",
      "remediation": "Refactor both to use prepared statements"
    }
  }
}
```

### Scenario 2: Test Failure with Debugging

**Tester encounters failure:**
```json
{
  "method": "notifications/message",
  "params": {
    "level": "critical",
    "logger": "agent:tester",
    "data": {
      "agent_id": "agent-test-1",
      "type": "finding",
      "message": "test_authentication_flow FAILED",
      "finding_type": "test-failure",
      "severity": "critical",
      "location": "tests/auth_test.rs:234",
      "test_name": "test_authentication_flow",
      "error_type": "AssertionError",
      "error_message": "Expected HTTP 200, got 401",
      "timestamp": "2026-03-09T14:33:15Z"
    }
  }
}
```

**Parent asks for more context:**
```json
{
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "notify agent",
      "target": "agent-test-1",
      "message": "Run this test in isolation with debug logging. Does it pass when run alone?"
    }
  }
}
```

### Scenario 3: Agent Needs Clarification (Blocking)

**Researcher hits ambiguity:**
```json
{
  "method": "notifications/message",
  "params": {
    "level": "notice",
    "logger": "agent:researcher",
    "data": {
      "agent_id": "agent-research-1",
      "type": "question",
      "message": "Which Python version should I research security best practices for?",
      "question": "Codebase supports Python 3.8+. Should I focus on latest (3.12) or oldest (3.8)?",
      "options": ["3.8-only", "3.12-only", "3.8-through-3.12"],
      "blocking": true,
      "context": "Security practices differ significantly across versions"
    }
  }
}
```

**Parent responds (unblocks agent):**
```json
{
  "method": "tools/call",
  "params": {
    "name": "agent",
    "arguments": {
      "op": "notify agent",
      "target": "agent-research-1",
      "message": "Focus on 3.8 as the minimum supported version. That's our lowest bar."
    }
  }
}
```

**Researcher continues:**
```json
{
  "method": "notifications/message",
  "params": {
    "level": "info",
    "logger": "agent:researcher",
    "data": {
      "agent_id": "agent-research-1",
      "type": "update",
      "message": "Research complete. Found 7 critical Python 3.8+ security practices",
      "phase": "complete",
      "findings_count": 7,
      "categories": {
        "input-validation": 2,
        "cryptography": 3,
        "dependency-management": 2
      }
    }
  }
}
```

### Scenario 4: Multiple Agents in Parallel

**Agent 1 reports:**
```json
{
  "method": "notifications/message",
  "params": {
    "level": "warning",
    "logger": "agent:code-reviewer",
    "data": {
      "agent_id": "agent-review-1",
      "type": "finding",
      "message": "O(n²) algorithm in parser",
      "agent_batch_id": "review-batch-1",
      "batch_position": "1/3"
    }
  }
}
```

**Agent 2 reports (in parallel):**
```json
{
  "method": "notifications/message",
  "params": {
    "level": "warning",
    "logger": "agent:security-auditor",
    "data": {
      "agent_id": "agent-review-2",
      "type": "finding",
      "message": "Missing TLS certificate validation",
      "agent_batch_id": "review-batch-1",
      "batch_position": "2/3"
    }
  }
}
```

**Agent 3 reports (in parallel):**
```json
{
  "method": "notifications/message",
  "params": {
    "level": "info",
    "logger": "agent:performance-analyst",
    "data": {
      "agent_id": "agent-review-3",
      "type": "update",
      "message": "Performance analysis complete, no major bottlenecks found",
      "agent_batch_id": "review-batch-1",
      "batch_position": "3/3"
    }
  }
}
```

---

## Notification Storage

Notifications are **not persisted** in a central store. Instead:
- Parent's MCP session receives notifications in its message stream
- Parent can store them in memory or write to transcript
- Subagent's session transcript contains agent's internal messages
- Parent can grep/search subagent transcript if needed

This keeps llama-agent lightweight and prevents central bottleneck.

---

## Edge Cases & Best Practices

### Edge Case 1: Nested Agent Notification

A sub-subagent notifies its parent (intermediate agent), which may forward to root:

```json
{
  "method": "notifications/message",
  "params": {
    "level": "error",
    "logger": "agent:security-auditor",
    "data": {
      "agent_id": "agent-security-sub-1",
      "parent_agent_id": "agent-review-1",
      "type": "finding",
      "message": "Found cryptographic weakness",
      "original_target": "root"
    }
  }
}
```

Intermediate agent can choose to:
- Forward as-is to root
- Aggregate with other findings
- Filter/suppress if not relevant

### Edge Case 2: Rate Limiting Long Analyses

Agent producing many findings should batch notifications:

**Bad (too many notifications):**
```
notification: finding 1
notification: finding 2
notification: finding 3
... (100 more)
```

**Better (batched progress):**
```json
{
  "type": "update",
  "message": "Analyzed 50 functions, found 5 issues so far",
  "progress": "50%",
  "findings_count": 5
}
```

Then emit detailed findings periodically.

### Edge Case 3: Agent Encounters Unexpected Error

Agent shouldn't silently fail — notify parent:

```json
{
  "method": "notifications/message",
  "params": {
    "level": "error",
    "logger": "agent:tester",
    "data": {
      "agent_id": "agent-test-1",
      "type": "question",
      "message": "Test setup failed. Unable to connect to test database.",
      "error": "Connection refused on localhost:5432",
      "blocking": true,
      "question": "Should I skip database tests or abort?",
      "options": ["skip-db-tests", "abort-all", "retry"]
    }
  }
}
```

### Edge Case 4: Parent Terminates Agent Mid-Execution

When parent calls `kill agent`:

```json
{
  "method": "notifications/message",
  "params": {
    "level": "warning",
    "logger": "agent:code-reviewer",
    "data": {
      "agent_id": "agent-review-1",
      "type": "completion",
      "status": "killed",
      "reason": "Parent killed: focusing on auth module only",
      "findings_at_termination": 7,
      "progress_percent": 45,
      "partial_results_available": true
    }
  }
}
```

Parent can still grep/search the partial results.

### Best Practice: Severity Matches MCP Level

Map finding severity to MCP level correctly:

| Finding Severity | MCP Level | Use When |
|---|---|---|
| critical | critical | Blocks deployment, security risk |
| warning | warning | Should be fixed, significant issue |
| info | info | Suggestion or minor improvement |
| (blocking question) | notice | Agent waiting for parent input |
| (progress/update) | info | Agent progressing |

### Best Practice: Include Actionable Data

Don't just say "bad" — include what to do:

```json
{
  "severity": "warning",
  "description": "Hardcoded API key in config.js",
  "location": "config.js:42",
  "remediation": "Move to environment variable: process.env.API_KEY",
  "related_files": ["src/auth.js", "src/api.js"],
  "cwe": "CWE-798: Use of Hard-coded Credentials"
}
```

### Best Practice: Timestamp and Correlation

Include timestamps and IDs for tracing:

```json
{
  "agent_id": "agent-review-1",
  "timestamp": "2026-03-09T14:32:00.123Z",
  "correlation_id": "req-abc123",
  "request_id": "parent-call-1"
}
```
