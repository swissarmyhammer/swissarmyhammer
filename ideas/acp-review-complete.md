# ACP Plan Review Complete

## Status: ✅ Plan Updated and Consistent with claude-agent

The ACP integration plan for llama-agent has been thoroughly reviewed against the claude-agent reference implementation and updated for consistency.

## Changes Made to Plan

### 1. Capabilities Structure ✅
- Updated to match claude-agent's exact capability advertisement
- `load_session: true`
- `prompt_capabilities`: audio/image/embedded_context = false (start conservative)
- `mcp_capabilities`: http=true, sse=false
- Filesystem/terminal NOT advertised in capabilities (handled via ext_method)

### 2. Client Capability Enforcement ✅
- Added client capabilities storage in AcpSessionState
- Added capability checks before filesystem operations
- Added capability checks before terminal operations
- Client declares what it supports, agent enforces it

### 3. File System via ext_method ✅
- Moved from RequestHandler trait to ext_method implementation
- `fs/read_text_file` checks client.fs.read_text_file capability
- `fs/write_text_file` checks client.fs.write_text_file capability
- Matches claude-agent pattern

### 4. Terminal via ext_method ✅
- Moved from RequestHandler trait to ext_method implementation
- All terminal/* methods check client.terminal capability
- Added TerminalState enum (Created, Running, Finished, Killed, Released)
- Added graceful shutdown timeout configuration
- Matches claude-agent pattern

### 5. Slash Commands Detail ✅
- Query MCP servers for available prompts
- Map MCP prompts to AvailableCommand entries
- Track per session with available_commands field
- Send AvailableCommandsChanged notifications
- Detect command changes and notify client
- Matches claude-agent pattern

### 6. Integration Simplification ✅
- No need to modify AgentServer
- Use existing generate_stream() method
- Consume stream and convert to ACP notifications
- Clean separation of concerns

## Final Task Count

**Total: 244 tasks** (was 239, added 5 for client capability tracking)

All tasks are:
- In dependency order
- Concrete and actionable
- Ready for AI implementation
- Consistent with claude-agent reference implementation

## Key Architectural Decisions Confirmed

1. **Module Integration**: ACP as module in llama-agent (not separate crate) ✅
2. **Streaming**: Use existing generate_stream(), consume and convert ✅
3. **Permissions**: Auto-approve for now, structured for future expansion ✅
4. **Sessions**: Implement load_session with history replay ✅
5. **Filesystem**: Via ext_method with client capability checks ✅
6. **Terminal**: Via ext_method with full state management ✅
7. **Plans**: Convert TodoItems to ACP plans with status tracking ✅
8. **Commands**: MCP prompts as slash commands via AvailableCommands ✅

## Documentation

- Main plan: `ideas/acp.md` (updated)
- Consistency review notes: `ideas/acp-consistency-notes.md`
- This summary: `ideas/acp-review-complete.md`

## Ready for Implementation ✅

The plan is now complete, consistent, and ready for systematic implementation following the 244-task dependency-ordered checklist.
