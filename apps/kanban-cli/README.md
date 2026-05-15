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

## Desktop app

<div align="center">

<img src="icon.png" alt="kanban desktop app" width="128" height="128">

</div>

The kanban desktop app is a Tauri-based GUI for browsing and editing the same `.kanban/` board the CLI and MCP server use. The CLI's `kanban open .` command launches it.

**Installing the app also gives you the `kanban` CLI** — the standalone CLI is bundled inside `Kanban.app` (at `Contents/MacOS/kanban`, signed and notarized with the bundle). You do not have to install the CLI separately on macOS; pick whichever install method you prefer below and you get both the app and the command.

### macOS (Homebrew cask)

```bash
brew install --cask swissarmyhammer/tap/kanban
```

The cask carries a `binary` stanza, so Homebrew links the bundled `kanban` CLI onto your `PATH` automatically. No further action — open a terminal and run `kanban`. (The cask also declares `conflicts_with formula: "kanban"` so it never collides with the standalone CLI formula below.)

### macOS (direct download)

Grab the signed, notarized DMG from the latest GitHub release:

```
https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/Kanban_aarch64.dmg
```

When you drag `Kanban.app` to `/Applications` from a DMG, there is no package manager to link the CLI. Instead, the app self-installs the `kanban` CLI onto your `PATH` on first launch: it creates a `kanban` symlink in a directory that is both user-writable and on the default `PATH` (preferring your Homebrew `bin`). If no user-writable `PATH` directory exists, the app falls back to `/usr/local/bin` and shows a single one-time macOS admin password prompt to create the symlink there. The prompt appears at most once — whether you accept or decline, the app does not ask again on later launches.

### From source

Requires the [Tauri prerequisites](https://tauri.app/start/prerequisites/) and Node.js 22+:

```bash
git clone https://github.com/swissarmyhammer/swissarmyhammer
cd swissarmyhammer/apps/kanban-app
cargo tauri build
```

The built `.app` lands under `target/release/bundle/` and bundles the `kanban` CLI inside it.

> macOS (Apple Silicon) is the only platform with prebuilt binaries today. On other platforms, build from source.

### Installing just the CLI

You do not need the desktop app to use the CLI. The standalone `kanban` CLI installs are listed under [Install](#install) above (`brew install swissarmyhammer/tap/kanban`, the Linux installer script, or `cargo install`), and remain the right choice for headless, Linux, and CI environments where a GUI app is not wanted. On macOS, the cask's `conflicts_with formula: "kanban"` ensures the standalone formula and the app-bundled CLI never both try to own `kanban` on your `PATH`.

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
