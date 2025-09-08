# MCP Integration

SwissArmyHammer provides comprehensive Model Context Protocol (MCP) integration, allowing AI language models to interact directly with your development tools and workflows. This creates a seamless bridge between AI assistance and your development environment.

## Overview

MCP integration enables:
- **Direct tool access**: AI models can use SwissArmyHammer tools directly
- **Workflow automation**: AI can execute complex development workflows
- **Context-aware assistance**: AI has access to your project state and history
- **Bidirectional communication**: Tools can provide feedback and results to AI
- **Secure operation**: Controlled access to development resources

## MCP Architecture

### Protocol Foundation

MCP (Model Context Protocol) is a standard for AI-tool integration:
- **Server-Client Architecture**: SwissArmyHammer runs as MCP server
- **Tool Registry**: Exposes capabilities to AI clients
- **Request-Response Pattern**: Structured communication protocol
- **Type Safety**: Strongly typed interface definitions
- **Error Handling**: Comprehensive error propagation and reporting

### Tool Categories

SwissArmyHammer exposes several categories of MCP tools:

**Issue Management Tools**:
- `issue_create` - Create new issues
- `issue_list` - List and filter issues  
- `issue_show` - Display issue details
- `issue_update` - Modify issue content
- `issue_complete` - Mark issues complete
- `issue_work` - Start work on issues
- `issue_merge` - Merge completed work

**Memoranda Tools**:
- `memo_create` - Create new memos
- `memo_list` - List all memos
- `memo_search` - Search memo content
- `memo_get` - Retrieve specific memos
- `memo_update` - Modify memo content

**Search Tools**:
- `search_index` - Index files for semantic search
- `search_query` - Perform semantic searches
- `outline_generate` - Generate code outlines

**Workflow Control**:
- `abort_create` - Signal workflow termination
- `issue_all_complete` - Check completion status

## Getting Started

### Server Setup

SwissArmyHammer automatically runs as an MCP server when used with compatible AI clients:

```bash
# Server starts automatically with compatible clients
# No manual configuration required
```

### Client Configuration

Configure your AI client to use SwissArmyHammer as an MCP server. Example configuration:

```json
{
  "mcpServers": {
    "swissarmyhammer": {
      "command": "sah",
      "args": ["--mcp"],
      "env": {
        "SAH_PROJECT_ROOT": "/path/to/your/project"
      }
    }
  }
}
```

### Verification

Test MCP connectivity:
```bash
# Check available tools
sah --mcp list-tools

# Verify server status  
sah --mcp status
```

## Tool Reference

### Issue Management

**Create Issue**:
```json
{
  "tool": "issue_create",
  "parameters": {
    "name": "feature_user_auth",
    "content": "# User Authentication\n\nImplement login system..."
  }
}
```

**List Issues**:
```json
{
  "tool": "issue_list", 
  "parameters": {
    "show_completed": false,
    "show_active": true,
    "format": "table"
  }
}
```

**Show Issue**:
```json
{
  "tool": "issue_show",
  "parameters": {
    "name": "current"  // or specific issue name
  }
}
```

**Work on Issue**:
```json
{
  "tool": "issue_work",
  "parameters": {
    "name": "FEATURE_001_user-auth"
  }
}
```

### Memoranda Operations

**Create Memo**:
```json
{
  "tool": "memo_create",
  "parameters": {
    "title": "API Design Decisions",
    "content": "# REST API Guidelines\n\n## Authentication\n..."
  }
}
```

**Search Memos**:
```json
{
  "tool": "memo_search",
  "parameters": {
    "query": "authentication patterns OAuth"
  }
}
```

**Get All Context**:
```json
{
  "tool": "memo_get_all_context",
  "parameters": {}
}
```

### Semantic Search

**Index Files**:
```json
{
  "tool": "search_index",
  "parameters": {
    "patterns": ["**/*.rs", "**/*.py"],
    "force": false
  }
}
```

**Search Query**:
```json
{
  "tool": "search_query",
  "parameters": {
    "query": "error handling patterns",
    "limit": 10
  }
}
```

**Generate Outline**:
```json
{
  "tool": "outline_generate",
  "parameters": {
    "patterns": ["src/**/*.rs"],
    "output_format": "yaml"
  }
}
```

## Advanced Usage

### Workflow Integration

AI can execute complex workflows using MCP tools:

```
1. Research Phase:
   - search_query("existing authentication systems")
   - memo_create("Research Findings", content)

2. Planning Phase:
   - issue_create("implement OAuth integration")
   - issue_work("FEATURE_001_oauth")

3. Development Phase:  
   - search_index(["**/*.rs"]) 
   - outline_generate(["src/auth/**/*.rs"])

4. Completion Phase:
   - issue_complete("FEATURE_001_oauth")
   - memo_create("Implementation Notes", lessons_learned)
```

### Context Management

AI maintains context across tool calls:
- **Project state**: Current branch, active issues
- **Search history**: Previous queries and results
- **Memo database**: Accumulated knowledge and decisions
- **Issue tracking**: Work progress and relationships

### Error Handling

MCP tools provide structured error responses:

```json
{
  "error": {
    "code": "ISSUE_NOT_FOUND",
    "message": "Issue 'FEATURE_999' does not exist",
    "details": {
      "available_issues": ["FEATURE_001", "FEATURE_002"],
      "suggestions": ["Check issue name spelling", "Use issue_list to see available issues"]
    }
  }
}
```

## Security Considerations

### Access Control

MCP integration operates within defined boundaries:
- **File system access**: Limited to project directories
- **Git operations**: Only standard development commands
- **Network access**: No external API calls required
- **Process isolation**: Runs in controlled environment

### Data Privacy

All operations are local:
- **No external services**: All processing happens locally
- **No data transmission**: Project data stays on your machine  
- **No logging**: Sensitive information not logged remotely
- **Full control**: Complete visibility into all operations

### Safe Operations

Tools designed for safe automated use:
- **Non-destructive defaults**: Safe operations by default
- **Confirmation patterns**: Critical operations require explicit confirmation
- **Rollback capability**: Git integration enables easy rollback
- **Audit trail**: All operations tracked in Git history

## Integration Examples

### AI-Assisted Development

**Feature Development Flow**:
```
AI: "I'll help implement user authentication. Let me start by researching existing patterns."

1. search_query("authentication patterns JWT session")
2. memo_create("Auth Research", findings)
3. issue_create("implement_user_auth", requirements)
4. issue_work("FEATURE_001_user_auth")
5. outline_generate(["src/auth/**/*.rs"])
6. [Development work with other tools]
7. issue_complete("FEATURE_001_user_auth")
```

**Code Review Assistance**:
```
AI: "Let me review the recent changes and provide feedback."

1. search_query("error handling in authentication")
2. issue_show("current") 
3. outline_generate(["src/**/*.rs"])
4. memo_create("Code Review Notes", analysis)
```

**Knowledge Management**:
```
AI: "I'll help organize the team's knowledge about the authentication system."

1. memo_search("authentication login OAuth")
2. memo_get_all_context()
3. search_query("auth implementation patterns")
4. memo_create("Auth System Overview", consolidated_knowledge)
```

### Custom Workflows

AI can execute custom workflows defined in SwissArmyHammer:

```markdown
# AI Development Assistant Workflow

## Research Phase
- Use semantic search to understand existing code
- Create memos with findings and decisions
- Reference related issues and documentation

## Implementation Phase  
- Create focused issues for development tasks
- Switch to appropriate Git branches
- Generate code outlines for understanding structure

## Review Phase
- Search for related implementations  
- Check issue completion status
- Create summary memos with lessons learned
```

## Troubleshooting

### Connection Issues

**MCP server not responding**:
```bash
# Check server status
sah --mcp status

# Restart server
pkill sah && sah --mcp
```

**Tool registration problems**:
```bash  
# Verify tool availability
sah --mcp list-tools

# Check client configuration
# Ensure correct command and arguments
```

### Authentication and Permissions

**File access denied**:
- Verify project directory permissions
- Check that SAH_PROJECT_ROOT is set correctly
- Ensure Git repository is accessible

**Git operation failures**:
- Verify Git repository status
- Check for uncommitted changes
- Ensure branch switching is possible

### Performance Issues

**Slow tool responses**:
- Large search indices may be slow initially  
- First semantic search loads model (normal delay)
- Check available memory for large operations

**High memory usage**:
- Semantic search models use significant memory
- Close unused AI sessions
- Restart MCP server if needed

### Common Errors

**"Project root not found"**: Set SAH_PROJECT_ROOT environment variable
**"Git repository not initialized"**: Run `git init` in project directory
**"Search index not found"**: Run `search_index` before querying
**"Invalid issue name"**: Check issue naming conventions and existing issues

## Best Practices

### Tool Usage

**Efficient workflows**:
- Use search_index before multiple queries
- Batch related operations together
- Cache frequently accessed memo content
- Leverage Git branching for issue work

**Error recovery**:
- Handle tool errors gracefully
- Provide fallback strategies
- Validate inputs before tool calls
- Use issue_all_complete for status checks

### AI Integration

**Context management**:
- Use memo_get_all_context for comprehensive background
- Search existing knowledge before creating new content
- Reference related issues and memos in new content
- Maintain consistent naming and tagging

**Workflow design**:
- Break complex tasks into discrete tool operations
- Provide clear success/failure indicators  
- Enable easy rollback and recovery
- Document decisions and rationale

## Future Enhancements

The MCP integration is designed for extensibility:

**Planned tool additions**:
- Configuration management tools
- Test execution and reporting tools
- Deployment and environment tools
- Code generation and refactoring tools

**Protocol improvements**:
- Enhanced error reporting and recovery
- Streaming responses for long operations
- Progress reporting for complex workflows
- Enhanced security and access controls

**AI capabilities**:
- Multi-step workflow execution
- Context-aware decision making
- Learning from project patterns
- Proactive assistance and suggestions

MCP integration transforms SwissArmyHammer into a powerful AI development assistant, enabling sophisticated automation while maintaining full control over your development environment and data.