<div align="center">

<img src="icon.png" alt="kanban" width="256" height="256">

# kanban

**Persistent task memory for AI coding agents.**

</div>

---

kanban is an MCP server and CLI that gives AI coding agents a real kanban board. Instead of losing context between sessions or drowning in a sprawling TODO list, the agent reads and writes tasks on a file-backed board that lives next to your code. The same board is also a first-class CLI for humans, and powers the kanban desktop app.

## Why

LLM agents forget. Chat context evaporates, scratchpad TODOs vanish between sessions, and "what was I doing?" becomes a tax the agent pays every time it starts up. kanban fixes that by giving the agent a durable board it owns — tasks persist across sessions, multiple agents can coordinate on the same board, and humans can see exactly what the agent is working on.

- **Persistent task memory** — the board is plain files in `.kanban/`, committed with your code and survives restarts
- **Agent-native** — operations are exposed as a single `kanban` MCP tool with verb-noun operations (`add task`, `move task`, `next task`) and forgiving input
- **Human-friendly** — the same operations are available as CLI subcommands (`kanban task add`, `kanban task list`) and in the kanban desktop app
- **Ready-task planning** — `next task` returns the oldest actionable card, respecting dependencies, so the agent always knows what to pick up next
- **Tags, projects, dependencies** — model real work without leaving the terminal or the agent loop

The agent uses the board like a teammate does: plan the work as cards, move them through the board, leave comments, and pick up the next ready thing.

## Install

### macOS (Homebrew)

```bash
brew install swissarmyhammer/tap/kanban
```

### Linux

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/kanban-cli-installer.sh | sh
```

### From source

```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer kanban-cli
```

Then set up the tool:

```bash
kanban init
```

This registers the MCP server with your agent, deploys the builtin `kanban` skill that teaches the agent how to use the board, and prepares the project for task tracking.

## Commands

| Command | Description |
|---------|-------------|
| `kanban serve` | Run MCP server over stdio |
| `kanban init [project\|local\|user]` | Install kanban for your agent |
| `kanban deinit [project\|local\|user]` | Remove kanban |
| `kanban doctor` | Diagnose setup issues |
| `kanban task add --title "..."` | Add a task from the CLI |
| `kanban task list` | List tasks |
| `kanban open .` | Open the kanban desktop app for this project |

All board, task, column, tag, and project operations are exposed as noun/verb subcommands. Run `kanban --help` to see the full list.

## Works With

Claude Code, Cursor, Windsurf, or any MCP-compatible agent. The same board is also available directly from the CLI and from the kanban desktop app.
