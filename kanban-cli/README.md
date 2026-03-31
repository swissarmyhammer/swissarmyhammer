<div align="center">

# kanban

**Task board CLI for AI coding agents and humans.**

</div>

---

Kanban provides a command-line kanban board that both you and your AI agent can use. Manage tasks, track progress, and organize work from the terminal or through the GUI app.

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

## Usage

All kanban operations are exposed as subcommands:

```bash
kanban task add --title "Fix login bug"
kanban task list
kanban task update --id ABC123 --column doing
```

Open the GUI app for a project:

```bash
kanban open .
kanban open /path/to/project
```

## Works With

Claude Code, Cursor, Windsurf, or any MCP-compatible agent. The kanban board is also available as an MCP tool for direct agent integration.
