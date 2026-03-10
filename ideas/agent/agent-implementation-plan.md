# Agent Tool Implementation Plan

5 sequential todos to upgrade the existing agent tool with subagent spawning and lifecycle management.

**Existing tool:** `swissarmyhammer-tools/src/mcp/tools/agent/mod.rs`
- Already has `AgentMcpTool` implementing `McpTool` trait
- Current operations: `list agent`, `use agent`, `search agent`
- Uses `AgentLibrary`, `PromptLibrary`, `AgentContext`

---

## Todo 1: Extend Agent Tool with New Operations

**Goal:** Add spawn, kill, notify, and grep operations to the existing agent tool.

**What to build:**
1. Define new Operation structs in `swissarmyhammer-agents` crate:
   - `SpawnAgent` (verb: spawn, noun: agent)
   - `KillAgent` (verb: kill, noun: agent)
   - `GrepAgent` (verb: grep, noun: agent)
   - `NotifyAgent` (verb: notify, noun: agent)
2. Add new variants to `AgentOperation` enum
3. Add to `AGENT_OPERATIONS` static array in `mod.rs`
4. Update `parse_input` to handle new operations
5. Wire new match arms into `execute` method

**Key considerations:**
- Follow existing pattern (static Lazy instances, Operation trait)
- Spawn returns agent_id immediately (MCP Tasks CreateTaskResult)
- `use agent` may be deprecated once `spawn agent` works
- Keep existing list/use/search operations working

**Acceptance criteria:**
- All 7 operations registered in AGENT_OPERATIONS (list, use, search + spawn, kill, grep, notify)
- Schema generation includes new operations
- Existing tests still pass
- New operations parse correctly from JSON input

---

## Todo 2: Agent Session & State Management

**Goal:** Build the runtime infrastructure to manage agent sessions and state.

**What to build:**
1. Create `swissarmyhammer-tools/src/mcp/tools/agent/agent_state.rs` with:
   - `AgentSession` struct (id, role, task, status, created_at, etc.)
   - `AgentStatus` enum (Pending, Running, Completed, Failed, Killed)
   - `AgentManager` for tracking active agents
2. Extend session manager to support agent subagents:
   - Parent session → child session relationship
   - Each subagent gets its own session
3. Create agent lifecycle handlers:
   - `spawn_agent_session()` — create new session for agent
   - `get_agent_status()` — return current status
   - `kill_agent_session()` — clean up session
4. Implement agent_id generation (stable, predictable)

**Key considerations:**
- Agent sessions reuse existing SessionManager infrastructure
- Each agent is a separate session rooted to a parent
- Status includes progress field for notifications
- Cleanup on kill (delete transcript, free resources)

**Acceptance criteria:**
- AgentSession struct can be created and tracked
- Agent-parent relationships maintained
- Agent status accessible and updateable
- Can retrieve agent by ID

---

## Todo 3: Notify Agent Tool & Session Routing

**Goal:** Implement the `notify agent` operation so subagents can send notifications to their parent, and parents can inject prompts back into subagents.

**What to build:**
1. Create `swissarmyhammer-tools/src/mcp/tools/agent/agent_notifications.rs` with:
   - `NotifyAgentHandler` — tool handler for the `notify agent` operation
   - `SessionRouter` — routes messages between parent and child sessions
   - `NotificationEmitter` — emits MCP `notifications/message` to target session
2. Implement `notify agent` tool handler:
   - Accept `target` (agent_id or "root"), `message`, `severity`, and optional `metadata`
   - Look up the target agent's session via AgentManager
   - Route the notification to the correct MCP session
3. Implement session-to-session routing:
   - Subagent → parent: emit MCP `notifications/message` on parent's session (S1)
   - Parent → subagent: inject message into subagent's conversation history (S2)
4. Wire into agent execution loop:
   - Register `notify agent` as an available tool in subagent sessions
   - Subagent system prompt teaches tool usage (see agent-system-prompt-design.md)

**Key considerations:**
- Subagent calls `notify agent` as a normal MCP tool (no output parsing needed)
- Notifications are structured data from the tool call, not extracted from text
- Parent receives MCP `notifications/message` with agent_id, severity, metadata
- Parent can respond via `notify agent` targeting the subagent for prompt injection
- Multiple notifications per agent session are expected

**Acceptance criteria:**
- Subagent can call `notify agent` and parent receives MCP notification
- Parent can call `notify agent` targeting a subagent and message is injected
- Notifications include correct agent_id, severity, and metadata
- Routing works across separate MCP sessions (S1 ↔ S2)

---

## Todo 4: Result Querying (Grep & Search)

**Goal:** Implement grep and search operations on agent transcripts.

**What to build:**
1. Create `swissarmyhammer-tools/src/mcp/tools/agent/agent_query.rs` with:
   - `GrepAgent` implementation — regex search of transcript
   - `SearchAgent` implementation — semantic search (embeddings)
2. Implement grep:
   - Regex pattern matching on agent transcript
   - Return line numbers + context
   - Handle large transcripts efficiently
3. Implement search:
   - Use existing embeddings from swissarmyhammer_embedding
   - Chunk transcript (similar to shell state)
   - Semantic similarity search
   - Return top-K chunks with scores
4. Store agent transcripts:
   - In-memory during execution
   - Persist to .agent_transcripts/ directory
   - Index for semantic search

**Key considerations:**
- Grep works on entire session transcript
- Search needs embeddings (chunked + indexed)
- Both operations work on running or completed agents
- Results include context (lines before/after)

**Acceptance criteria:**
- Grep agent finds patterns correctly
- Search agent returns semantically relevant results
- Results include context and metadata
- Works on large transcripts

---

## Todo 5: Integration with Agent Execution Loop

**Goal:** Wire everything together so agents actually execute, emit notifications, and can be queried.

**What to build:**
1. Modify spawn agent handler:
   - Create new session via session_manager
   - Start agent in background (tokio::spawn)
   - Return agent_id immediately
   - Emit notification via notification system
2. Create agent execution loop:
   - Subscribe to agent session messages
   - On each message from agent:
     - Process `notify agent` tool calls (route to parent session)
     - Store in transcript
   - On completion:
     - Mark session as completed
     - Final result available
3. Integrate with RequestQueue:
   - Agent execution queues requests to shared model
   - Multiple agents can be running (RequestQueue serializes)
   - Each agent waits its turn for model
4. Hook up termination:
   - kill agent cancels background task
   - Cleans up session
   - Emits kill notification

**Key considerations:**
- Agent execution is async (tokio task)
- Notification emission is fast (doesn't block agent)
- Transcript stored incrementally
- Embeddings indexed in background (like shell state)
- Parent can grep/search while agent still running

**Acceptance criteria:**
- Can spawn agent and get agent_id back
- Agent actually generates (uses model)
- Notifications emit during execution
- Can grep/search running agent
- Agent completes and returns result
- Can list all agents and see statuses
- Can kill running agent
