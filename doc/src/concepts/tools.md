# Tools

Tools are the capabilities that agents use to interact with the outside world. They're exposed as MCP (Model Context Protocol) endpoints — the agent calls them like functions to read files, run commands, search code, manage tasks, and more.

## What a Tool Is

A tool is an MCP endpoint with a defined schema (name, description, parameters) and an implementation. When the agent decides it needs to read a file or run a shell command, it invokes the appropriate tool. SwissArmyHammer's MCP server (`sah serve`) exposes all built-in tools to the connected agent.

Tools are the lowest layer of the system. Skills and agents don't hard-code which tools to use — they make decisions based on the task and invoke tools as needed.

## Built-in Tools

### File Operations

| Tool | Purpose |
|------|---------|
| **read** | Read file contents |
| **write** | Create or overwrite files |
| **edit** | Surgical string replacement in files |
| **glob** | Find files by pattern |
| **grep** | Search file contents with regex |

### Execution

| Tool | Purpose |
|------|---------|
| **shell** | Execute shell commands with history and process management |

### Code Intelligence

| Tool | Purpose |
|------|---------|
| **code_context** | Unified code context index — symbols, call graphs, blast radius |

### Project Management

| Tool | Purpose |
|------|---------|
| **kanban** | Create, update, and query kanban cards for task tracking |

### Git

| Tool | Purpose |
|------|---------|
| **git changes** | Query git diff and change information |

### Communication

| Tool | Purpose |
|------|---------|
| **question** | Ask the user clarifying questions |
| **summary** | Provide structured summaries |

### Agent Orchestration

| Tool | Purpose |
|------|---------|
| **agent** | Spawn subagents for delegated work |
| **skill** | Invoke a skill by name |
| **ralph** | Autonomous execution coordinator |

### Web

| Tool | Purpose |
|------|---------|
| **web** | Fetch web content for research |

## How Tools Fit In

Tools sit beneath skills and agents in the stack. Here's the relationship:

- A **skill** (e.g., `/test`) defines the workflow.
- An **agent mode** (e.g., tester) shapes how the AI approaches the task.
- **Tools** (e.g., shell, files) are what the agent actually calls to do the work.
- **Validators** (e.g., command-safety) check each tool invocation before it executes.

The agent has access to all tools and chooses which to use based on context. The skill and agent mode influence these choices through their instructions, but tools themselves are general-purpose.

## MCP Protocol

All tools are served via the Model Context Protocol. When you run `sah serve`, it starts an MCP server (stdio by default, HTTP optionally) that exposes these tools to any connected agent. Claude Code discovers them automatically via the MCP configuration created by `sah init`.

This means SwissArmyHammer's tools work alongside any other MCP servers you have configured. The agent sees a unified tool palette from all sources.
