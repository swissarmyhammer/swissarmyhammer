<div align="center">

<img src="icon.png" alt="kanban" width="128" height="128">

# kanban

**A git-native task board for humans and AI coding agents.**

Plain files you can version. One board, three faces — a **GUI**, a **CLI**, and an **MCP** server. Plug your agents in over **MCP** and **ACP**.

[![MCP](https://img.shields.io/badge/MCP-server-green.svg)](https://modelcontextprotocol.io)
[![ACP](https://img.shields.io/badge/ACP-agents-blue.svg)](https://agentclientprotocol.com)
[![Rust](https://img.shields.io/badge/rust-single%20binary-orange.svg)](https://www.rust-lang.org/)
[![Storage](https://img.shields.io/badge/storage-git--native-purple.svg)](#version-your-board-not-a-database)

</div>

---

kanban gives your AI coding agent a real board to work — and gives *you* one to watch. Tasks live as **plain Markdown + YAML files in `.kanban/`, right next to your code**, so the board is something you can `git diff`, `git blame`, branch, and merge. The same board drives a CLI for your scripts, an MCP tool for your agent, and a native desktop app for you — no database, no daemon, no cloud.

## Agents forget. Your board shouldn't.

Chat context evaporates. Scratchpad TODOs vanish between sessions. "What was I doing?" becomes a tax the agent pays every time it wakes up. kanban fixes that by giving the agent a durable board it *owns*: tasks persist across sessions, multiple agents coordinate on the same cards, and you can see — live — exactly what your agent is working on and what it'll pick up next.

## What makes it different

- 🗂️ **Versionable, not a database.** The board is Markdown + YAML + an append-only JSONL changelog per task. Review a status change in a PR. `git blame` who closed a card. Branch your plan, merge it back. Your task history travels with your code — no SQL blob, no export step. ([details ↓](#version-your-board-not-a-database))
- 🖥️ **One board, three faces.** A native **desktop GUI**, a scriptable **CLI**, and an **MCP server** — all reading and writing the same `.kanban/` files. Humans, shell scripts, and agents on one source of truth.
- 🔌 **Plugs into your tools — MCP *and* ACP.** Expose the board to Claude Code, Cursor, Windsurf, or any MCP-compatible agent. Or drive an agent — Claude Code, or a **local llama model** — right inside the GUI over ACP. Bring your own agent; the board is the contract.
- 🎯 **Ready-task planning with real dependencies.** `next task` returns the oldest *unblocked* card, honoring `depends_on`, so the agent always knows the right next thing — no re-planning, no picking blocked work.
- 👥 **Human + agent co-ownership, in real time.** Assignees are typed `human` or `agent` (`claude-code` is just another teammate). Watch cards move across the board live as your agent works.
- 🦀 **Local-first, single Rust binary.** Fast startup, no runtime deps, no Docker, no service to babysit. The board is files; the tool is one binary.

The agent uses the board the way a good teammate does: plan the work as cards, move them across columns, record progress, and pick up the next ready thing.

## Version your board, not a database

Every task is a Markdown file with YAML frontmatter — so a move from `doing` to `done` is just a line in a diff your team can review:

```diff
  # .kanban/tasks/01KSSTMWWS064Q6QC9BD3J1DN5.md
  ---
  assignees:
  - claude-code
- position_column: doing
+ position_column: done
  title: 'kanban init/deinit: adopt mirdan per-agent strategy'
  ---
  ## What
  Bring `kanban init/deinit` in line with the shelltool/sah pattern...
```

The card body is real Markdown, so agents write rich, durable task notes — acceptance criteria, references, decisions — that survive across sessions and show up in code review. Alongside each task, an append-only `.jsonl` changelog records every create/update with a timestamp and a patch: a full, git-versioned audit trail of how the work evolved.

Compare that to a board locked in a SQL database or a separate service: with kanban there's nothing to export, nothing to reconcile, and nothing that drifts from the commit it belongs to.

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

When you drag `Kanban.app` to `/Applications` from a DMG, there is no package manager to link the CLI. Instead, the app self-installs the `kanban` CLI onto your `PATH` at launch: it creates a `kanban` symlink in a directory that is both user-writable and on the default `PATH` (preferring your Homebrew `bin`). If no user-writable `PATH` directory exists, the app falls back to `/usr/local/bin` — and since that directory is root-owned, it shows an explanatory dialog followed by the macOS admin password prompt to create the symlink there. Self-install is gated solely on the symlink: if the `kanban` CLI is not linked (you declined, or the link was later removed), the app offers to install it again on a later launch; once it is linked, the app stays silent.

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

## What's on disk

The whole board is a directory you can read, diff, and commit:

```
.kanban/
  tasks/            # one Markdown + YAML file per card, plus an append-only
                    #   .jsonl changelog per task (full, versioned history)
  projects/         # projects as YAML
  tags/             # tags as YAML
  perspectives/     # saved board views as YAML
```

No proprietary format, no database file, no cloud. Check it into git and your task history lives — and merges — right alongside your code.

## How it compares

kanban is part of a young, exciting category of agent-native task tools — [beads](https://github.com/steveyegge/beads) and [kata](https://github.com/kenn-io/kata) are both worth a look. Here's where kanban stakes its ground:

| | **kanban** | Database-backed trackers | Service/ledger trackers |
|---|---|---|---|
| **Where the board lives** | Plain files **in your repo** (`.kanban/`) | A version-controlled **SQL database** | A **local service** beside the repo |
| **Versioning** | Normal `git diff` / `blame` / branch / merge on text | The database's own versioning | Export/import for backup & migration |
| **Human UI** | Native **desktop GUI** + CLI | CLI | Terminal **TUI** + CLI |
| **Agent integration** | **MCP** (expose the board) **+ ACP** (run agents in the GUI) | MCP / CLI | CLI + events / webhooks |
| **Runtime** | One Rust binary, **no daemon** | Database engine | Optional remote daemon |

The bet: your task board belongs in your repo, in a format your team — and your `git` — already understands, reachable from whatever surface you're working in.

## Works With

Claude Code, Cursor, Windsurf, or any MCP-compatible agent — over MCP. Drive an agent (Claude Code or a local llama model) directly inside the desktop app — over ACP. And every operation is a plain CLI subcommand for your scripts and CI. One board, every surface.
