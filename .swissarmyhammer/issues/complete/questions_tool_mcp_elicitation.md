# Implement Questions MCP Tools with Elicitation and Persistence

## Overview

Create two new MCP tools for interactive question/answer workflows:
1. `question_ask` - Asks user a question via MCP elicitation and persists answer
2. `question_summary` - Returns all Q&A pairs as YAML for agent context

These tools enable agents to gather user input through out-of-band elicitation, persist the answers for future reference, and inject the full Q&A history into their context.

## MCP Elicitation Protocol

### Reference Documentation
- [Elicitation Specification](https://modelcontextprotocol.io/specification/2025-06-18/schema#elicitation%2Fcreate)
- [Client Elicitation Guide](https://modelcontextprotocol.io/specification/2025-06-18/client/elicitation)

### Protocol Flow
1. Agent calls `question_ask` tool with a question
2. Tool sends `elicitation/create` request to client
3. Tool execution blocks (no timeout) waiting for response
4. User provides answer through client UI
5. Client returns answer to server
6. Tool saves Q&A pair to `.swissarmyhammer/questions/<timestamp>_<id>.yaml`
7. Tool returns answer to agent
8. Agent can later call `question_summary` to get all Q&A history

## Tool 1: `question_ask`

### Requirements

#### Conditional Registration
- **MUST** check client capabilities before registering the tool
- Only register `question_ask` tool if client supports elicitation
- Use `tracing` to log registration decisions:
  - `tracing::info!` when tool is registered (client supports elicitation)
  - `tracing::debug!` when tool is not registered (client lacks elicitation support)

#### Tool Parameters

Simple schema with only a string property for the question:

```rust
{
  "question": "What is your preferred approach?"  // the question to ask the user
}
```

#### Tool Response

```rust
{
  "answer": "Option A",  // user's answer as a string
  "saved_to": ".swissarmyhammer/questions/20250605_133045_question.yaml"
}
```

#### Blocking Behavior
- Tool execution **MUST** block until user responds
- **NO timeout** - wait indefinitely for user input
- Handle client disconnection gracefully
- Support cancellation via MCP cancellation protocol

#### Elicitation Schema

The tool creates a simple elicitation request with a single string input:

```rust
// Elicitation request sent to client
{
  "schema": {
    "type": "object",
    "properties": {
      "answer": {
        "type": "string",
        "description": "<the question text>"
      }
    },
    "required": ["answer"]
  }
}
```

#### Persistence Format

Each question/answer is saved to a separate YAML file in `.swissarmyhammer/questions/`:

**Filename Pattern:** `<timestamp>_question.yaml`
- Timestamp: `YYYYMMDD_HHMMSS` format
- Example: `20250605_133045_question.yaml`

**YAML Structure:**
```yaml
# Saved at 2025-06-05 13:30:45 UTC
timestamp: "2025-06-05T13:30:45.123Z"
question: "What is your preferred approach?"
answer: "Option A"
```

## Tool 2: `question_summary`

### Purpose
Returns all persisted question/answer pairs as a single YAML string that agents can inject into their context.

### Requirements

#### Registration
- Always register this tool (no capability check needed)
- Does not depend on elicitation support
- Works even if `question_ask` is not registered

#### Tool Parameters

```rust
{
  "limit": 10  // optional: max number of Q&A files to include (default: all)
}
```

#### Tool Response

```rust
{
  "summary": "# Question/Answer History\n\n...",  // YAML string
  "count": 3  // total number of Q&A pairs included
}
```

#### Summary Format

The `summary` field contains all Q&A pairs merged into a single YAML string:

```yaml
# Question/Answer History
# Generated: 2025-06-05T14:00:00.000Z
# Total Q&A Pairs: 3

entries:
  - timestamp: "2025-06-05T13:30:45.123Z"
    question: "What is your preferred approach?"
    answer: "Option A"
  
  - timestamp: "2025-06-05T13:45:12.456Z"
    question: "Select deployment target"
    answer: "staging"
  
  - timestamp: "2025-06-05T14:00:00.789Z"
    question: "Which database should we use?"
    answer: "PostgreSQL"
```

#### Sorting and Limits
- Entries sorted by timestamp (oldest first)
- If `limit` specified, return most recent N entries
- Include metadata about total count even when limited

## Implementation Details

### File Structure
```
swissarmyhammer-tools/src/mcp/tools/
├── question_ask/
│   ├── mod.rs              # Main implementation
│   ├── description.md      # Tool description for MCP
│   ├── persistence.rs      # YAML file save/load
│   └── tests.rs           # Unit tests
└── question_summary/
    ├── mod.rs              # Main implementation
    ├── description.md      # Tool description
    └── tests.rs           # Unit tests
```

### Persistence Implementation

```rust
// In question_ask/persistence.rs

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct QuestionAnswerEntry {
    pub timestamp: String,
    pub question: String,
    pub answer: String,
}

pub fn save_entry(question: &str, answer: &str) -> Result<PathBuf> {
    let timestamp_str = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("{}_question.yaml", timestamp_str);
    
    let questions_dir = std::env::current_dir()?
        .join(".swissarmyhammer")
        .join("questions");
    
    std::fs::create_dir_all(&questions_dir)?;
    
    let entry = QuestionAnswerEntry {
        timestamp: Utc::now().to_rfc3339(),
        question: question.to_string(),
        answer: answer.to_string(),
    };
    
    let file_path = questions_dir.join(filename);
    let yaml_content = format!(
        "# Saved at {}\ntimestamp: \"{}\"\nquestion: \"{}\"\nanswer: \"{}\"\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        entry.timestamp,
        entry.question,
        entry.answer
    );
    
    std::fs::write(&file_path, yaml_content)?;
    
    Ok(file_path)
}
```

### Capability Detection

```rust
// In unified_server.rs or tool registry

let supports_elicitation = client_capabilities
    .elicitation
    .as_ref()
    .map(|e| e.enabled)
    .unwrap_or(false);

if supports_elicitation {
    tracing::info!("Client supports elicitation, registering 'question_ask' tool");
    registry.register_tool(question_ask::tool());
} else {
    tracing::debug!("Client does not support elicitation, 'question_ask' tool not available");
}

// Always register question_summary
registry.register_tool(question_summary::tool());
```

### Elicitation Request

```rust
// In question_ask/mod.rs

use rmcp::elicitation::{ElicitationRequest, ElicitationSchema};
use serde_json::json;

async fn ask_question(ctx: &ToolContext, question: &str) -> Result<String> {
    let schema = json!({
        "type": "object",
        "properties": {
            "answer": {
                "type": "string",
                "description": question
            }
        },
        "required": ["answer"]
    });
    
    let request = ElicitationRequest {
        schema,
        ..Default::default()
    };
    
    // This blocks until user responds (no timeout)
    let response = ctx.client.elicit(request).await?;
    
    let answer = response
        .get("answer")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No answer provided"))?;
    
    Ok(answer.to_string())
}
```

### Error Handling

- Client disconnection during elicitation
- Malformed responses from client
- Missing answer in response
- File system errors when saving
- YAML serialization errors
- Missing or corrupted question files when generating summary

### Testing Strategy

1. **Unit Tests** (question_ask):
   - Parameter validation
   - Response formatting
   - YAML persistence
   - Error handling for invalid inputs
   - Filename generation

2. **Unit Tests** (question_summary):
   - Reading multiple YAML files
   - Merging into single summary
   - Sorting by timestamp
   - Limiting results
   - Handling empty directory

3. **Integration Tests**:
   - Mock client that supports elicitation
   - Test blocking behavior
   - Test response mapping
   - Test cancellation
   - End-to-end: ask question, save, retrieve summary

4. **Manual Tests**:
   - Test with real MCP client that supports elicitation
   - Test with client that doesn't support elicitation (question_ask not registered)
   - Verify YAML files are human-readable
   - Test question_summary with multiple entries

## Use Cases

### Interactive Workflow with Context Persistence

```
Agent: Let me ask the user about their preferences.
[calls question_ask with "What is your preferred approach?"]
[execution blocks]
User: [provides answer through client UI: "Option A"]
Agent: Got it, saving answer for future reference.
[question_ask saves to .swissarmyhammer/questions/20250605_133045_question.yaml]

... later in the session or in a new session ...

Agent: Let me check what the user told me before.
[calls question_summary]
Agent: According to the previous answers, the user prefers Option A...
```

### Configuration Choices
```
Agent: I need to know the deployment target.
[calls question_ask with "Select deployment target"]
[waits for user response]
User: [types "staging"]
[saved to .swissarmyhammer/questions/20250605_134512_question.yaml]
Agent: Deploying to staging environment...
```

### Multi-Session Context
```
Agent: Let me review all the decisions made in this project.
[calls question_summary]
Agent: I can see you've already answered:
  - Preferred approach: Option A
  - Deployment target: staging
  - Database choice: PostgreSQL
Let me proceed with these choices...
```

## Non-Goals

- This tool does NOT use the existing `AskUserQuestion` permission-style prompts
- This tool does NOT have a timeout - it waits indefinitely
- This tool does NOT fall back to a different mechanism if client doesn't support elicitation (it simply isn't registered)
- question_summary does NOT require elicitation support (works standalone)
- Does NOT edit or delete existing question files (append-only)
- Does NOT support complex question schemas (only simple string input)
- Does NOT support multiple questions in a single request

## Acceptance Criteria

### question_ask Tool
- [ ] Tool is conditionally registered based on client elicitation capability
- [ ] Tool properly uses `elicitation/create` protocol with simple string schema
- [ ] Tool blocks without timeout until user responds
- [ ] Tool accepts only a single `question` parameter (string)
- [ ] Tool returns user's answer as a string
- [ ] Tool saves Q&A pair to YAML file in `.swissarmyhammer/questions/`
- [ ] YAML filename includes timestamp
- [ ] YAML structure is human-readable and well-formatted
- [ ] Tool returns file path in response
- [ ] Tool handles cancellation gracefully
- [ ] Tool handles client disconnection with appropriate error
- [ ] Registration decisions are logged with `tracing`
- [ ] `.swissarmyhammer/questions/` directory is created if needed

### question_summary Tool
- [ ] Tool is always registered (no capability check)
- [ ] Tool reads all YAML files from `.swissarmyhammer/questions/`
- [ ] Tool merges entries into single YAML string
- [ ] Entries are sorted by timestamp (oldest first)
- [ ] Tool respects `limit` parameter if provided
- [ ] Tool includes metadata (count)
- [ ] Tool handles empty questions directory gracefully
- [ ] Tool handles corrupted YAML files gracefully
- [ ] Summary format is valid YAML

### Testing
- [ ] Unit tests cover parameter validation for question_ask
- [ ] Unit tests cover YAML persistence and loading
- [ ] Unit tests cover filename generation
- [ ] Unit tests cover question_summary merging logic
- [ ] Integration tests cover elicitation flow with mock client
- [ ] Integration tests cover end-to-end: ask, save, summarize
- [ ] Both tool description.md files document requirements

### Documentation
- [ ] question_ask description.md mentions elicitation requirement
- [ ] question_ask description.md shows simple string schema format
- [ ] question_summary description.md explains YAML format
- [ ] Examples show typical usage patterns
- [ ] Examples show how to use summary in agent context

## References

- MCP Elicitation Specification: https://modelcontextprotocol.io/specification/2025-06-18/schema#elicitation%2Fcreate
- Client Elicitation Guide: https://modelcontextprotocol.io/specification/2025-06-18/client/elicitation
- rmcp crate documentation for elicitation API


## Claude Code Elicitation Support Status

### Current Status: Unknown

We cannot directly verify if Claude Code currently supports MCP elicitation without implementing and testing the tool. Elicitation is a **client capability** that was newly introduced in MCP protocol version 2025-06-18.

### How It Works

1. **Client advertises capabilities** during MCP initialization:
   ```json
   {
     "capabilities": {
       "elicitation": {}
     }
   }
   ```

2. **Server checks capabilities** and conditionally registers tools:
   ```rust
   let supports_elicitation = client_capabilities
       .elicitation
       .is_some();
   ```

3. **Graceful handling**:
   - If Claude Code supports elicitation → `question_ask` tool is registered
   - If Claude Code doesn't support elicitation → `question_ask` tool is not registered
   - Either way, `question_summary` tool is always registered

### Testing Approach

1. **Implement with conditional registration** as specified
2. **Test with Claude Code**:
   - If tool appears in tools list → elicitation is supported
   - If tool doesn't appear → elicitation is not yet supported
3. **Log registration decision** with tracing for debugging

### Fallback Behavior

If Claude Code doesn't currently support elicitation:
- The `question_ask` tool will not be registered (logged at debug level)
- The `question_summary` tool will still work (always registered)
- No errors or failures - just graceful absence of the interactive feature
- Users can still manually create question YAML files if needed

This design ensures the implementation is forward-compatible and won't break if elicitation support is added to Claude Code in the future.
## Proposed Solution

Based on analysis of the existing SwissArmyHammer codebase, I'll implement the two MCP tools following these steps:

### 1. Create Directory Structure
```
swissarmyhammer-tools/src/mcp/tools/questions/
├── mod.rs              # Module re-exports
├── ask/
│   ├── mod.rs          # question_ask implementation
│   └── description.md  # Tool documentation
└── summary/
    ├── mod.rs          # question_summary implementation
    └── description.md  # Tool documentation
```

### 2. Implement Shared Persistence Layer
Create a reusable persistence module for Q&A storage:
- Location: `swissarmyhammer-tools/src/mcp/tools/questions/persistence.rs`
- Functions:
  - `save_question_answer(question: &str, answer: &str) -> Result<PathBuf>`
  - `load_all_questions() -> Result<Vec<QuestionAnswerEntry>>`
- YAML format with timestamp, question, and answer fields
- Directory: `.swissarmyhammer/questions/`
- Filename pattern: `YYYYMMDD_HHMMSS_question.yaml`

### 3. Implement `question_ask` Tool
- Check client capabilities during registration (conditional registration)
- Use `rmcp::elicitation` API to send elicitation requests
- Tool accepts single `question` parameter (string)
- Create simple elicitation schema with string "answer" field
- Block indefinitely until user responds (no timeout)
- Save Q&A pair to YAML file using persistence layer
- Return answer and file path in response
- Log registration decision with `tracing::info!` or `tracing::debug!`

### 4. Implement `question_summary` Tool
- Always register (no capability check needed)
- Read all YAML files from `.swissarmyhammer/questions/`
- Sort entries by timestamp (oldest first)
- Support optional `limit` parameter
- Merge into single YAML string with metadata
- Return summary string and count

### 5. Register Tools in Registry
- Add `register_questions_tools()` function in `swissarmyhammer-tools/src/mcp/tools/questions/mod.rs`
- Call from `swissarmyhammer-tools/src/mcp/server.rs` line ~189
- For `question_ask`: Check `client_capabilities.elicitation` before registration
- For `question_summary`: Always register

### 6. Testing Strategy
- Unit tests for persistence layer (save/load)
- Unit tests for filename generation and YAML formatting
- Unit tests for question_summary merging and sorting logic
- Mock elicitation client for integration tests
- Tests for conditional registration logic
- Tests for empty directory handling

### Implementation Notes

**Key Design Decisions:**
1. **Conditional Registration Pattern**: Following MCP best practices, only register `question_ask` when client supports elicitation. This prevents errors and provides graceful degradation.

2. **Separate Persistence Module**: Shared code between both tools avoids duplication and ensures consistency in file format.

3. **No New Crate**: Both tools live in `swissarmyhammer-tools` since they're lightweight MCP tool implementations that don't need separate library infrastructure.

4. **Elicitation Schema**: Simple single-field schema makes it easy for clients to render and for users to respond.

5. **Append-Only Design**: Q&A files are never edited or deleted, creating an audit trail of all user interactions.

**Potential Challenges:**
1. **rmcp Elicitation API**: Need to verify exact API for sending elicitation requests and receiving responses. The Cargo.toml shows rmcp 0.8.4 with "elicitation" feature enabled.

2. **Client Capability Detection**: Need to find where client capabilities are available during tool registration. Server.rs shows `InitializeRequestParam` in initialize() but tools are registered before that.

3. **Blocking Behavior**: Ensure the elicitation request properly blocks without timeout. May need to use tokio channels or futures for clean async handling.

**Next Steps:**
1. Research rmcp elicitation API documentation
2. Determine where client capabilities are available for conditional registration
3. Implement persistence layer with tests
4. Implement question_ask with conditional registration
5. Implement question_summary
6. Add integration tests
7. Update tool registry and server registration
## Implementation Challenge: Conditional Registration Not Feasible

After analyzing the codebase architecture, I've identified a fundamental issue with the conditional registration approach described in the requirements:

### The Problem

**Tools are registered before client capabilities are known:**
1. `McpServer::new()` (line 52-198 in server.rs) registers all tools during server creation
2. Tools are registered via `register_*_tools()` functions (lines 177-189)
3. Client capabilities are only received later during `initialize()` handler (line 593-640)
4. The `InitializeRequestParam` contains client capabilities, but this happens AFTER tools are already registered

**This means we cannot conditionally register `question_ask` based on whether the client supports elicitation.**

### Proposed Alternative Approach

Instead of conditional registration, implement **runtime capability checking**:

1. **Always Register Both Tools**: Register `question_ask` and `question_summary` unconditionally
2. **Runtime Check in execute()**: When `question_ask` is invoked, check if elicitation is supported
3. **Graceful Failure**: If elicitation is not supported, return a clear error message

### Implementation Strategy

```rust
async fn execute(&self, arguments: Args, context: &ToolContext) -> Result<CallToolResult> {
    // Check if we have elicitation support at runtime
    // This requires storing client capabilities in ToolContext
    
    if !context.supports_elicitation() {
        return Err(McpError::invalid_request(
            "This tool requires client elicitation support, which is not available. \
             Please upgrade your MCP client or manually create question files in \
             .swissarmyhammer/questions/",
            None
        ));
    }
    
    // Proceed with elicitation...
}
```

### Required Changes to Support This

1. **Store Client Capabilities in ToolContext**: Modify `ToolContext` to include client capabilities
2. **Update Initialize Handler**: Store capabilities when client connects
3. **Runtime Checking**: Check capabilities in `question_ask.execute()`

### Alternative: Skip Conditional Registration Entirely

**Simpler approach**: Just implement the tool and let it fail gracefully if elicitation is not supported. Document that the tool requires MCP 2025-06-18 with elicitation support.

This is actually the most pragmatic approach because:
- SwissArmyHammer is forward-looking and targets modern MCP clients
- The error message will be clear if an older client tries to use it
- No complex capability plumbing needed through the codebase
- `question_summary` works independently regardless

### Recommendation

I recommend the **simpler approach**: Implement both tools without conditional logic. If `question_ask` is called and elicitation fails, the error from rmcp will be clear. Add good documentation stating the requirement.

This aligns with the SwissArmyHammer philosophy of "ALL functionality ALL the time" (see Cargo.toml line 2).

Awaiting decision on which approach to take before proceeding with implementation.
## Implementation Discovery: Need to Add Peer to ToolContext

After researching the rmcp API and codebase architecture, I've identified the critical requirement:

### The Core Issue

**Elicitation requires access to `rmcp::Peer<RoleServer>`**, but:
- Tools execute via `McpTool::execute(arguments, context: &ToolContext)`
- ToolContext does not currently contain the Peer
- Peer is only available in ServerHandler methods (like `call_tool()`)

### The Solution

**Add `peer: Option<Arc<rmcp::Peer<RoleServer>>>` to ToolContext:**

1. Modify `ToolContext` struct to include optional Peer
2. Update `ServerHandler::call_tool()` to pass Peer to ToolContext when executing tools
3. `question_ask` tool can then use `context.peer` to call `create_elicitation()`

### Implementation Plan

1. **Modify ToolContext** (tool_registry.rs):
   - Add `peer: Option<Arc<rmcp::Peer<RoleServer>>>` field
   - Update constructors
   - Add method to set peer

2. **Modify ServerHandler::call_tool()** (server.rs):
   - Clone peer from RequestContext
   - Create temporary ToolContext with peer for this execution
   - Pass to tool.execute()

3. **Implement question_ask**:
   - Check if `context.peer` is Some
   - If None, return error that elicitation is not available
   - If Some, call `peer.create_elicitation()` with schema
   - Block until user responds
   - Save Q&A to file
   - Return answer

This approach provides graceful degradation - if for some reason Peer is not available, the tool returns a clear error rather than panicking.

Proceeding with implementation...