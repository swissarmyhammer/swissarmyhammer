# Serve Command

Start the SwissArmyHammer MCP (Model Context Protocol) server for AI tool integration.

## Usage

```bash
sah serve
```

## Description

The serve command starts an MCP server that provides AI tools and capabilities through the Model Context Protocol. This enables integration with AI applications like Claude Code.

When started, the server:
- Initializes the MCP server with available tools
- Sets up stdio transport for communication
- Runs in blocking mode until the client disconnects or an error occurs

## Features

- **Tool Integration**: Exposes SwissArmyHammer tools through MCP protocol
- **Stdio Transport**: Uses standard input/output for client communication  
- **Graceful Shutdown**: Handles client disconnection and cleanup
- **Comprehensive Logging**: Detailed logging for debugging and monitoring
- **Error Handling**: Robust error handling with appropriate exit codes

## Exit Codes

- `0`: Server started and stopped successfully
- `1`: Server encountered warnings or stopped unexpectedly  
- `2`: Server failed to start or encountered critical errors

## Technical Details

The serve command:
1. Creates and initializes the MCP server instance
2. Sets up the stdio transport layer
3. Starts the server in blocking mode
4. Waits for client communication or shutdown signals
5. Performs cleanup on exit

The server integrates with the SwissArmyHammer tool ecosystem, providing access to:
- File operations (read, write, edit, glob)
- Search and indexing capabilities
- Issue tracking and management
- Memoranda storage and retrieval
- Shell execution and system integration
- Web fetching and search functionality
- Workflow and prompt management

## Logging

In serve mode, logs are written to `.swissarmyhammer/mcp.log` for debugging purposes. The log level is set to DEBUG to provide detailed information about server operation and client interactions.