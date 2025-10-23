# Quick Start

This guide will walk you through using SwissArmyHammer Tools for the first time. In just a few minutes, you'll have the MCP server running and be able to use it with Claude Code.

## Prerequisites

Make sure you have completed the [Installation](getting-started.md#installation) steps.

## Step 1: Start the MCP Server

Open a terminal and start the server in stdio mode:

```bash
sah serve
```

You should see output indicating the server has started:

```
SwissArmyHammer MCP Server v0.1.0
Mode: stdio
Registered 28 tools
Ready for requests
```

## Step 2: Configure Claude Code

Add SwissArmyHammer as an MCP server to Claude Code:

```bash
claude mcp add --scope user sah sah serve
```

Restart Claude Code to load the new configuration.

## Step 3: Verify Tools Are Available

In Claude Code, ask:

```
What SwissArmyHammer tools are available?
```

Claude should respond with a list of available tools organized by category.

## Your First Tasks

### Task 1: Create and Manage a Memo

Memos are perfect for capturing project knowledge and decisions.

**Create a memo:**

```
Create a memo with title "Project Setup Notes" and content:
- Using Rust 1.70
- MCP server integration complete
- Claude Code configured successfully
```

**List memos:**

```
List all memos
```

**Retrieve the memo:**

```
Show me the "Project Setup Notes" memo
```

### Task 2: Semantic Code Search

Semantic search helps you find code based on meaning, not just keywords.

**Index your codebase:**

Navigate to a Rust project and ask Claude:

```
Index all Rust files in this project for semantic search
```

**Search for code:**

```
Search for "error handling" code in this project
```

**View code structure:**

```
Generate an outline of src/main.rs
```

### Task 3: Issue Tracking

Issues are tracked as markdown files in `.swissarmyhammer/issues/`.

**Create an issue:**

```
Create an issue named "add-logging" with content:
# Add Structured Logging

Add structured logging using the tracing crate throughout the application.

## Tasks
- [ ] Add tracing dependencies
- [ ] Initialize tracing subscriber
- [ ] Replace println! with tracing macros
- [ ] Add span instrumentation to key functions
```

**List issues:**

```
List all issues
```

**Show issue details:**

```
Show the "add-logging" issue
```

**Mark complete:**

```
Mark the "add-logging" issue as complete
```

### Task 4: File Operations

File tools provide safe, validated file system access.

**Read a file:**

```
Read the contents of Cargo.toml
```

**Find files by pattern:**

```
Find all Rust source files in the src directory
```

**Search file contents:**

```
Search for the word "async" in all Rust files
```

**Edit a file:**

```
In Cargo.toml, replace the line:
version = "0.1.0"

with:
version = "0.2.0"
```

### Task 5: Git Integration

Track what files have changed on your branch.

**Check changed files:**

```
What files have changed on my current branch?
```

This is useful for understanding the scope of your work and what needs to be reviewed.

### Task 6: Shell Execution

Execute shell commands with proper output capture.

**Run a command:**

```
Run the command: cargo build --release
```

**Check git status:**

```
Execute: git status
```

### Task 7: Code Quality Rules

Check your code against defined quality standards.

**Check for violations:**

```
Run rules check on all Rust files
```

This will report any violations of configured coding standards.

## Understanding Tool Organization

SwissArmyHammer tools are organized into categories:

- **files_**: File system operations (read, write, edit, glob, grep)
- **search_**: Semantic code search (index, query)
- **issue_**: Issue management (create, list, show, update, complete)
- **memo_**: Note-taking (create, get, list, get_all_context)
- **todo_**: Task tracking (create, show, mark_complete)
- **git_**: Git integration (changes)
- **shell_**: Command execution (execute)
- **outline_**: Code analysis (generate)
- **rules_**: Quality checks (check)
- **web_**: Web tools (fetch, search)
- **flow**: Workflow execution
- **abort_**: Workflow control (create)

## Common Patterns

### Pattern 1: Research → Document → Implement

1. Use `search_query` to find relevant code
2. Use `outline_generate` to understand structure
3. Create an `issue` for the work
4. Use `memo_create` to capture decisions
5. Use `files_edit` to make changes
6. Use `rules_check` to validate quality

### Pattern 2: Code Review Workflow

1. Use `git_changes` to see what changed
2. Use `files_read` to review specific files
3. Use `search_query` to find related code
4. Use `rules_check` to verify standards
5. Use `memo_create` to document findings

### Pattern 3: Documentation Generation

1. Use `outline_generate` to extract structure
2. Use `files_read` to read source files
3. Use `files_write` to create documentation
4. Use `search_index` to make it searchable

## Working Directory

The MCP server operates in a working directory, which defaults to where you started the server. You can change this:

```bash
sah --cwd /path/to/project serve
```

All relative paths in tool operations are resolved relative to this working directory.

## Data Storage

SwissArmyHammer stores data in two locations:

**Project Data** (`./.swissarmyhammer/`):
- Issues
- Memos
- Workflow state
- Should be committed to git

**User Data** (`~/.swissarmyhammer/`):
- Personal memos
- Search indices
- Configuration
- Should not be committed

## Tips for Effective Use

### Tip 1: Use Semantic Search Early

Index your codebase right after starting work on a project. This helps Claude understand your code much faster.

### Tip 2: Capture Context in Memos

Use memos to capture decisions, gotchas, and context that you'll want to reference later or share with Claude in future sessions.

### Tip 3: Track Work with Issues

Create issues for all non-trivial work. This provides structure, tracking, and a record of what was done.

### Tip 4: Combine Tools

The tools are most powerful when combined. For example:
- Search for relevant code
- Read the files found
- Create an issue for the work
- Make edits
- Check rules compliance

### Tip 5: Use Todo for Session Planning

Use `todo_create` to break down complex tasks into steps that can be completed in a single session. Todos are ephemeral and don't persist after completion.

## Next Steps

Now that you've completed the quick start:

- **[Configuration](configuration.md)**: Customize SwissArmyHammer for your workflow
- **[Architecture](architecture.md)**: Understand how the system is designed
- **[Features](features.md)**: Explore all tools in depth with more examples
- **[Troubleshooting](troubleshooting.md)**: Solutions to common problems

## Getting Help

If you run into issues:

1. Check the [Troubleshooting Guide](troubleshooting.md)
2. Enable debug logging: `SAH_LOG_LEVEL=debug sah serve`
3. Review server logs for error messages
4. Check that tools are registered: ask Claude "List all SwissArmyHammer tools"
5. Report issues at https://github.com/swissarmyhammer/swissarmyhammer-tools/issues
