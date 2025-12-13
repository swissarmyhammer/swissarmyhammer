# ACP Plan Consistency Review - claude-agent vs llama-agent Plan

## Summary
Reviewed claude-agent implementation comprehensively. The plan is mostly consistent, but needs the following adjustments:

## Key Findings

### ✅ Matches Plan (No Changes Needed)
1. **Agent Trait Methods**: claude-agent implements all baseline ACP methods (initialize, authenticate, new_session, load_session, set_session_mode, prompt, cancel, ext_method)
2. **Session Loading**: claude-agent advertises `load_session: true` and fully implements it with history replay
3. **Session Modes**: Fully implemented with mode tracking and notifications
4. **Agent Plans**: Comprehensive implementation with status tracking and updates
5. **File System**: Implements read_text_file and write_text_file via ext_method
6. **Terminal**: Full terminal management with create, output, wait, kill, release
7. **Streaming**: Uses async streaming with session/update notifications

### ⚠️ Differences to Address in Plan

#### 1. **Capabilities Advertisement**
**Issue**: claude-agent capabilities don't include filesystem/terminal in the struct, but implements them.

**claude-agent capabilities:**
```rust
AgentCapabilities {
    load_session: true,
    prompt_capabilities: PromptCapabilities {
        audio: true,
        embedded_context: true,
        image: true,
        meta: Some({"streaming": true}),
    },
    mcp_capabilities: McpCapabilities {
        http: true,
        sse: false,
    },
}
```

**Action**: Update plan - llama-agent should match this. File system and terminal are NOT advertised in capabilities but are available via ext_method based on CLIENT capabilities.

#### 2. **MCP Transport Support**
**Issue**: claude-agent only supports HTTP MCP (not SSE), with comment: "We only support HTTP MCP connections, not SSE (which is deprecated in MCP spec)"

**Action**: Update plan - llama-agent should advertise:
```rust
mcp_capabilities: McpCapabilities {
    http: true,
    sse: false,
}
```

#### 3. **Prompt Capabilities**
**Issue**: claude-agent advertises audio/image support

**Action**: Update plan - llama-agent should initially advertise:
```rust
prompt_capabilities: PromptCapabilities {
    audio: false,  // Not supported yet
    embedded_context: false,  // Not supported yet
    image: false,  // Not supported yet
    meta: Some({"streaming": true}),
}
```
We can expand these later when llama models support them.

#### 4. **Slash Commands Implementation**
**Issue**: claude-agent implements slash commands as:
- Integration with Claude CLI's init message (slash_commands array)
- MCP server prompts exposed as slash commands
- Session-level `available_commands` tracking
- `SessionUpdate::AvailableCommandsChanged` notifications

**Action**: Update plan - llama-agent should:
- Expose MCP prompts as available commands (similar to claude-agent)
- Track available_commands per session
- Send AvailableCommandsChanged notifications when they change
- NOT integrate with Claude CLI (we don't have that)

#### 5. **File System via ext_method**
**Issue**: claude-agent implements fs/read_text_file and fs/write_text_file as extension methods (not as part of RequestHandler trait), checking CLIENT capabilities first.

**Action**: Update plan - implement filesystem in ext_method handler, not as separate RequestHandler trait methods. Check client.fs.read_text_file and client.fs.write_text_file capabilities before allowing.

#### 6. **Terminal via ext_method**
**Issue**: claude-agent implements terminal/* methods as extension methods, checking CLIENT terminal capability.

**Action**: Update plan - implement terminal in ext_method handler, checking client.terminal capability.

#### 7. **Plan Integration**
**Issue**: claude-agent has sophisticated plan integration with:
- `todowrite_to_acp_plan` converter function
- Plan updates sent via SessionUpdate::AgentPlan
- Priority levels (high, medium, low)
- Status tracking (pending, in_progress, completed, failed, cancelled)
- Active form for in-progress items

**Action**: Plan already includes this, but emphasize the `todowrite_to_acp_plan` function needs to be robust.

#### 8. **Permission System**
**Issue**: claude-agent has full permission system with PolicyEngine, but we decided to auto-approve.

**Action**: Keep auto-approve as planned, but structure it similarly (have a PermissionPolicy enum that can be expanded later).

## Plan Updates Needed

### Update 1: Capabilities Structure
Change initialization to match claude-agent's capability advertisement:

```diff
### Agent Trait Implementation - Initialization
- [ ] Implement `Agent::initialize` method
- [ ] Negotiate protocol version
-- [ ] Advertise agent capabilities (filesystem, terminal, modes, etc.)
++ [ ] Advertise agent capabilities matching claude-agent:
++     - load_session: true
++     - prompt_capabilities: audio=false, embedded_context=false, image=false, streaming=true
++     - mcp_capabilities: http=true, sse=false
++     - meta: include available tools list
+ [ ] Return `InitializeResponse` with server info
+ [ ] Handle version compatibility checking
```

### Update 2: File System Implementation
Move filesystem from RequestHandler to ext_method:

```diff
### File System Operations
- [ ] Create `llama-agent/src/acp/filesystem.rs`
- [ ] Implement path validation (absolute paths only, no traversal)
- [ ] Implement path security checks (allowed/blocked lists)
-- [ ] Implement `RequestHandler::read_text_file`
++ [ ] Implement `fs/read_text_file` in ext_method handler
++ [ ] Check client.fs.read_text_file capability before allowing
- [ ] Validate file path
- [ ] Check read permissions
- [ ] Check file size limits
- [ ] Read file content
- [ ] Return `ReadTextFileResponse`
- [ ] Handle errors (file not found, permission denied, etc.)
-- [ ] Implement `RequestHandler::write_text_file`
++ [ ] Implement `fs/write_text_file` in ext_method handler
++ [ ] Check client.fs.write_text_file capability before allowing
- [ ] Validate file path
- [ ] Check write permissions
- [ ] Write content atomically
- [ ] Return `WriteTextFileResponse`
- [ ] Handle errors
-- [ ] Add filesystem tools to agent tool registry
-- [ ] Create MCP-style tool wrappers for file operations
```

### Update 3: Terminal Implementation
Move terminal from RequestHandler to ext_method:

```diff
### Terminal Management
- [ ] Create `llama-agent/src/acp/terminal.rs`
- [ ] Define `TerminalManager` struct
- [ ] Define `TerminalState` struct (process, output buffer, exit status)
- [ ] Implement terminal ID generation
-- [ ] Implement `RequestHandler::create_terminal`
++ [ ] Implement `terminal/create` in ext_method handler
++ [ ] Check client.terminal capability before allowing
- [ ] Validate terminal command
- [ ] Spawn process with async
- [ ] Setup output capture
- [ ] Store terminal state
- [ ] Return `CreateTerminalResponse` with terminal ID
-- [ ] Implement `RequestHandler::terminal_output`
++ [ ] Implement `terminal/output` in ext_method handler
++ [ ] Check client.terminal capability
- [ ] Get terminal by ID
- [ ] Read buffered output since last read
- [ ] Return `TerminalOutputResponse`
-- [ ] Implement `RequestHandler::wait_for_exit`
++ [ ] Implement `terminal/wait_for_exit` in ext_method handler
-- [ ] Implement `RequestHandler::terminal_release`
++ [ ] Implement `terminal/release` in ext_method handler
-- [ ] Implement `RequestHandler::terminal_kill`
++ [ ] Implement `terminal/kill` in ext_method handler
- [ ] Get terminal by ID
- [ ] Send kill signal to process
- [ ] Keep terminal in storage for output/status
- [ ] Return success
- [ ] Add output buffering and streaming
- [ ] Handle process cleanup on session close
- [ ] Handle zombie process prevention
```

### Update 4: Slash Commands Detail
Add MCP prompt integration:

```diff
### Slash Commands
- [ ] Create `llama-agent/src/acp/commands.rs`
- [ ] Define `SlashCommandRegistry`
- [ ] Define `SlashCommand` struct with name, description, parameters
- [ ] Implement command registration
-- [ ] Add built-in commands (/test, /review, etc.)
-- [ ] Integrate with swissarmyhammer workflow system
-- [ ] Map workflows to slash commands
++ [ ] Query MCP servers for available prompts
++ [ ] Map MCP prompts to AvailableCommand entries
++ [ ] Track available_commands per session in AcpSessionState
- [ ] Implement command advertisement
- [ ] Send `SessionUpdate::AvailableCommandsChanged` notifications
-- [ ] Implement command execution hooks
++ [ ] Update available commands when MCP servers change
- [ ] Handle custom command parameters
```

### Update 5: Remove Deprecated Items
Remove items that aren't in claude-agent or aren't needed:

- Remove "Add filesystem tools to agent tool registry" (filesystem is via ext_method, not tools)
- Remove "Create MCP-style tool wrappers for file operations" (not how claude-agent does it)

### Update 6: Client Capabilities Tracking
Add client capabilities tracking:

```diff
### Agent Trait Implementation - Initialization
+ [ ] Store client capabilities from initialize request
+ [ ] Track client.fs.read_text_file capability
+ [ ] Track client.fs.write_text_file capability
+ [ ] Track client.terminal capability
+ [ ] Use client capabilities to gate ext_method operations
```

## Conclusion

The plan is fundamentally sound and matches claude-agent's architecture. The main changes are:

1. **Capabilities**: Match claude-agent's structure (no fs/terminal in capabilities struct)
2. **MCP**: Only HTTP, no SSE
3. **Prompt Caps**: Start with audio/image/embedded_context = false
4. **Filesystem/Terminal**: Implement via ext_method, not RequestHandler trait
5. **Slash Commands**: Use MCP prompts, track per session, send change notifications
6. **Client Capabilities**: Track and enforce what client supports

These are refinements, not major architectural changes. The plan's dependency ordering and task breakdown remain valid.
