# Serve - Bridge AI to Your Development Environment

Connect Claude Code and other AI applications to SwissArmyHammer's powerful
tool ecosystem through the Model Context Protocol. The serve command transforms
SwissArmyHammer into an AI-accessible development platform.

## AI Integration Made Simple

The serve command starts an MCP server that exposes SwissArmyHammer's complete
toolset to AI applications. This enables Claude Code and other MCP-compatible
AI tools to directly access file operations, todo management,
workflows, and more.

```bash
sah serve
```

## What MCP Server Provides

Complete Tool Ecosystem:
• File Operations - Read, write, edit, glob pattern matching
• Todo Management - Track, create, update development tasks
• Shell Integration - Execute commands and scripts
• Web Capabilities - Fetch content and search the web
• Workflow Execution - Run automated development workflows
• Prompt Management - Access and execute prompt templates

AI-Native Protocol:
• Standard MCP protocol for broad AI compatibility
• Structured tool definitions with schemas
• Type-safe parameter passing
• Rich error handling and feedback
• Tool discovery and documentation

## Why Use MCP Server

Direct AI Access:
• Claude Code can directly manipulate your development environment
• No manual file operations or copy-paste workflows
• AI understands your project structure through code analysis
• Seamless integration between AI reasoning and tool execution

Enhanced AI Capabilities:
• AI can read and write files without user intervention
• Todo tracking integrates with AI planning and execution
• Workflows enable complex multi-step AI operations
• Web access for documentation and reference lookup

Development Efficiency:
• AI-assisted coding with full project context
• Automated todo execution and implementation
• Context-aware planning and refactoring

## Server Operation

When started, the server:
1. Initializes MCP server with all available tools
2. Sets up stdio transport for client communication
3. Runs in blocking mode serving AI requests
4. Handles graceful shutdown on client disconnect
5. Logs all operations for debugging and monitoring

Communication:
• Uses standard input/output (stdio) for MCP protocol
• Blocking operation until client disconnects
• Automatic cleanup on exit
• Robust error handling and recovery

## Usage

Start MCP server for Claude Code:
```bash
sah serve
```

The server runs until:
• Client disconnects (normal shutdown)
• User interrupts with Ctrl+C
• Critical error occurs

## Exit Codes

- `0` - Server started and stopped successfully (normal operation)
- `1` - Server encountered warnings or unexpected stop
- `2` - Server failed to start or critical error occurred

## Logging and Debugging

Comprehensive logging to `.swissarmyhammer/mcp.log`:
• DEBUG level for detailed operation information
• All tool invocations and parameters
• Client communication events
• Error details and stack traces
• Performance and timing information

Use logs to:
• Debug integration issues with AI clients
• Monitor tool usage patterns
• Troubleshoot client communication
• Analyze performance characteristics
• Audit AI operations on your codebase

## Available Tools

The MCP server exposes these tool categories:

File System:
• files_read - Read file contents
• files_write - Create or overwrite files
• files_edit - Precise string replacements
• files_glob - Pattern-based file discovery
• files_grep - Content search with ripgrep

Development Workflow:
• todo_create, todo_show, todo_list - Todo tracking
• todo_mark_complete - Todo completion

Execution:
• shell_execute - Run shell commands
• workflow operations - Execute automated workflows
• prompt operations - Render and test prompts

Web Access:
• web_fetch - Download and process web content
• web_search - Search the web for information

## Integration Setup

Configure Claude Code to use SwissArmyHammer MCP server:

1. Add to Claude Code MCP configuration (~/.claude-code/mcp.json):
```json
{
  "mcpServers": {
    "swissarmyhammer": {
      "command": "sah",
      "args": ["serve"]
    }
  }
}
```

2. Restart Claude Code to activate the integration

3. Verify connection in Claude Code logs

## Common Workflows

Claude Code development:
```bash
# Terminal 1: Start MCP server
sah serve

# Terminal 2: Use Claude Code with SwissArmyHammer tools
```

Automated todo execution:
```bash
# Claude Code uses MCP to:
# 1. List todos with todo_list
# 2. Read todo details with todo_show
# 3. Edit code with files_edit
# 4. Run tests with shell_execute
# 5. Mark complete with todo_mark_complete
```

## Troubleshooting

Server won't start:
• Check port availability and permissions
• Verify sah is in PATH
• Review .swissarmyhammer/mcp.log for errors
• Try with --debug flag for detailed output

Client can't connect:
• Verify MCP configuration in client
• Check stdio transport is working
• Review client-side MCP logs
• Ensure no firewall blocking communication

Tools not working:
• Check .swissarmyhammer/mcp.log for tool errors
• Verify file system permissions
• Ensure git repository is properly initialized
• Review tool-specific requirements

## Security Considerations

The MCP server provides AI with direct access to:
• File system (within project directory)
• Shell command execution
• Git operations
• Web access

Best practices:
• Run server only when actively using AI features
• Monitor .swissarmyhammer/mcp.log for unusual activity
• Review AI-generated changes before committing
• Use in trusted development environments
• Shut down server when not in use

The serve command transforms SwissArmyHammer into an AI-accessible development
platform, enabling Claude Code and other AI tools to work directly with your
codebase, tools, and workflows through the standard Model Context Protocol.