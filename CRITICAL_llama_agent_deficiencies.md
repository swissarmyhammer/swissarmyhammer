# CRITICAL: llama-agent Session Setup Deficiencies

**Date:** 2025-12-15
**Status:** llama-agent is NOT spec-compliant for session setup
**Severity:** HIGH - Core functionality missing

---

## ❌ CRITICAL DEFICIENCIES CONFIRMED

### 1. Session Has NO cwd Field

**Evidence:**

**llama-agent Session struct** (llama-agent/src/types/sessions.rs:229):
```rust
pub struct Session {
    pub id: SessionId,
    pub messages: Vec<Message>,
    pub mcp_servers: Vec<MCPServerConfig>,
    // ... NO cwd FIELD ...
    pub transcript_path: Option<PathBuf>,
    pub context_state: Option<ContextState>,
    // ...
}
```

**claude-agent Session struct** (claude-agent/src/session.rs:173):
```rust
pub struct Session {
    pub id: SessionId,
    pub created_at: SystemTime,
    pub context: Vec<Message>,
    pub cwd: PathBuf,  // ✅ HAS CWD - Line 181
    pub mcp_servers: Vec<String>,
    // ...
}
```

**Impact:**
- ❌ llama-agent sessions have NO working directory context
- ❌ File operations cannot use session-specific cwd
- ❌ Violates ACP spec requirement: "cwd: Absolute path serving as file system context"

---

### 2. new_session IGNORES cwd Parameter

**Code** (llama-agent/src/acp/server.rs:917):
```rust
async fn new_session(&self, _request: NewSessionRequest) -> Result<...> {
    //                        ^^^^^^^^ underscore prefix = INTENTIONALLY UNUSED

    let llama_session = self.agent_server.create_session().await?;
    //                  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    //                  create_session() takes NO parameters
}
```

**Spec Requirement:**
> "cwd (string, absolute path): File system context for session operations"

**What llama does:** Completely ignores the cwd parameter

**What claude does** (claude-agent/src/agent.rs:3121):
```rust
let session_id = self.session_manager
    .create_session(request.cwd.clone(), client_caps)  // ✅ Passes cwd
    .map_err(...)?;
```

---

### 3. new_session IGNORES mcpServers Parameter

**Code** (same line 917):
```rust
async fn new_session(&self, _request: NewSessionRequest) -> Result<...> {
    //                        ^^^^^^^^ request contains mcpServers but is unused

    // No validation of MCP servers
    // No storage of MCP servers
    // No connection to MCP servers
}
```

**Spec Requirement:**
> "mcpServers (array): MCP server configurations"
> "Agents SHOULD connect to all MCP servers specified by the Client"

**What llama does:** Ignores completely

**What claude does:**
1. Validates MCP transport capabilities (lines 3106-3112)
2. Stores MCP configuration (lines 3131-3145)
3. Connects to servers (via MCP client)

---

### 4. Session Modes Not Implemented

**Code** (llama-agent/src/acp/server.rs:970):
```rust
tracing::warn!("Session modes are not yet implemented, mode_id will be ignored");

let mut meta = serde_json::Map::new();
meta.insert("mode_set".to_string(), serde_json::Value::Bool(false));
//                                                                ^^^^^ Returns false!
```

**What this means:**
- Accepts the request
- Returns success
- But doesn't actually set the mode
- Meta field admits "mode_set: false"

**Spec Requirement:** Set session mode

**What claude does** (claude-agent/src/agent.rs):
```rust
self.session_manager.update_session(&session_id, |session| {
    session.current_mode = Some(mode_id_string.clone());  // ✅ Actually sets mode
})?;
```

---

## Why Tests Passed (False Positive)

**Current tests only check:**
1. ✅ Method returns a response
2. ✅ Response has sessionId field
3. ✅ No crash or error

**Tests DON'T check:**
1. ❌ Is cwd actually stored in session?
2. ❌ Are MCP servers validated?
3. ❌ Are MCP servers connected?
4. ❌ Does session mode actually change?
5. ❌ Can file operations use session cwd?

**This is why llama-agent "passed" despite not implementing the features.**

---

## Actual Spec Compliance

### llama-agent: 29% COMPLIANT ❌

| Spec Requirement | Implemented | Notes |
|------------------|-------------|-------|
| Accept cwd parameter | ⚠️ Yes | But ignores it |
| Store cwd in session | ❌ NO | Session has no cwd field |
| Use cwd as fs context | ❌ NO | Not possible without storing |
| Accept mcpServers | ⚠️ Yes | But ignores it |
| Validate MCP transports | ❌ NO | No validation |
| Connect to MCP servers | ❌ NO | Uses NoOpMCPClient |
| Store MCP config | ❌ NO | Not stored |
| Return unique sessionId | ✅ YES | ULID-based |
| Implement set-mode | ❌ NO | Stub with mode_set:false |

**Score:** 1/9 requirements fully met (11%)

**Note:** Even the "accepts parameters" doesn't count since they're ignored

### claude-agent: 100% COMPLIANT ✅

| Spec Requirement | Implemented | Notes |
|------------------|-------------|-------|
| Accept cwd parameter | ✅ YES | request.cwd |
| Store cwd in session | ✅ YES | session.cwd field exists |
| Use cwd as fs context | ✅ YES | Used in operations |
| Accept mcpServers | ✅ YES | request.mcp_servers |
| Validate MCP transports | ✅ YES | CapabilityRequirementChecker |
| Connect to MCP servers | ✅ YES | Actual MCP client |
| Store MCP config | ✅ YES | session.mcp_servers |
| Return unique sessionId | ✅ YES | ULID-based |
| Implement set-mode | ✅ YES | session.current_mode |

**Score:** 9/9 requirements fully met (100%)

---

## Required Fixes for llama-agent

### Priority 1: Add cwd to Session struct

**File:** `llama-agent/src/types/sessions.rs`

```rust
pub struct Session {
    pub id: SessionId,
    pub messages: Vec<Message>,
    pub cwd: PathBuf,  // ← ADD THIS
    pub mcp_servers: Vec<MCPServerConfig>,
    // ...
}
```

### Priority 2: Accept and use cwd in new_session

**File:** `llama-agent/src/acp/server.rs:915`

```rust
// BEFORE:
async fn new_session(&self, _request: NewSessionRequest) -> Result<...> {
    let llama_session = self.agent_server.create_session().await?;
}

// AFTER:
async fn new_session(&self, request: NewSessionRequest) -> Result<...> {
    let llama_session = self.agent_server.create_session_with_cwd(request.cwd).await?;
}
```

### Priority 3: Validate MCP transports

**File:** `llama-agent/src/acp/server.rs`

```rust
// Add validation like claude-agent:
for server in &request.mcp_servers {
    match server {
        McpServer::Http(_) => {
            if !self.config.capabilities.mcp_capabilities.http {
                return Err(Error::invalid_params());
            }
        }
        McpServer::Sse(_) => {
            if !self.config.capabilities.mcp_capabilities.sse {
                return Err(Error::invalid_params());
            }
        }
        McpServer::Stdio(_) => {} // Always supported
    }
}
```

### Priority 4: Store MCP servers in session

**File:** `llama-agent/src/acp/server.rs`

```rust
// Store MCP servers
if !request.mcp_servers.is_empty() {
    // Convert and store in session
    session.mcp_servers = convert_mcp_servers(request.mcp_servers);
}
```

### Priority 5: Implement session modes OR stop advertising

**Option A:** Implement

```rust
self.session_manager.update_session(&session_id, |session| {
    session.current_mode = Some(mode_id.to_string());
})?;
```

**Option B:** Stop advertising

```rust
// In initialize:
.meta({
    "supports_modes": false,  // ← Change to false
})
```

---

## Test Improvements Needed

### Add Behavioral Tests

1. **test_cwd_stored_in_session**
   ```rust
   let response = agent.new_session(NewSessionRequest::new(cwd)).await?;
   // Somehow verify session actually has this cwd
   // (Requires introspection or behavioral test)
   ```

2. **test_mcp_transport_validation**
   ```rust
   // Send HTTP server when http:false advertised
   let http_server = McpServer::Http(...);
   let request = NewSessionRequest::new(cwd).mcp_servers(vec![http_server]);
   let result = agent.new_session(request).await;
   assert!(result.is_err()); // Should fail
   ```

3. **test_session_mode_persistence**
   ```rust
   agent.set_session_mode(request).await?;
   // Verify mode is actually set (needs session introspection)
   ```

---

## Corrected Compliance Report

### llama-agent: MAJOR DEFICIENCIES ❌

**Initialization Protocol:** ✅ 100% compliant
**Session Setup Protocol:** ❌ 11% compliant

**Critical gaps:**
- Missing cwd field in Session struct
- Ignores cwd parameter completely
- Ignores mcpServers parameter
- No MCP transport validation
- Session modes unimplemented

**Production Readiness:** NOT READY for ACP session functionality

### claude-agent: FULLY COMPLIANT ✅

**Initialization Protocol:** ✅ 100% compliant (logic)
**Session Setup Protocol:** ✅ 100% compliant

**Complete implementation:**
- Has cwd field, uses it
- Validates and stores MCP servers
- Validates transport capabilities
- Implements session modes
- Full session/load replay

**Production Readiness:** READY (once API migration completes)

---

## Conclusion

**The user was correct** - the differences point to significant deficiencies in llama-agent, not just "different approaches."

**llama-agent needs:**
1. Add cwd field to Session
2. Accept and use cwd parameter
3. Validate MCP transport capabilities
4. Store MCP server configurations
5. Complete session modes OR stop advertising
6. Replace NoOpMCPClient with real implementation

**Estimated effort:** 1-2 days of development + testing

**Current state:** llama-agent has a minimal stub implementation that returns valid responses but doesn't implement the required session functionality.
