<div align="center">

<img src="shelltool.png" alt="shelltool" width="256" height="256">

# shelltool

**A shell that saves tokens — run, search, retrieve.**

</div>

---

A virtual shell for AI agents that works the way a person uses a terminal.

Instead of dumping raw command output into the agent's context window — wasting thousands of tokens on build logs, test results, and diagnostic output — shelltool keeps a persistent, searchable shell session. The agent runs commands, then searches or retrieves just the lines it needs.

## Why

Built-in Bash tools flood the context with every byte of stdout. A single `cargo test` can burn 10,000+ tokens of output the agent will never look at again. shelltool stores all output in a searchable history and returns only what matters:

- **Run a command** — get a summary with exit code and line count
- **Grep the output** — regex search across history for error codes, function names, patterns
- **Semantic search** — find output by meaning ("authentication error" matches "403 forbidden")
- **Get specific lines** — retrieve exactly the range you need for context
- **Manage processes** — list running commands, kill hung processes

The agent uses the shell like a person does: run something, scan the results, dig into the interesting parts.

## Install

```bash
brew install swissarmyhammer/tap/shelltool-cli
```

or

```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer shelltool-cli
```

Then set up the tool:

```bash
shelltool init
```

This registers the MCP server, blocks the built-in Bash tool (replaced by shelltool), deploys the shell skill, and creates `.shell/config.yaml` for security configuration.

## Commands

| Command | Description |
|---------|-------------|
| `shelltool serve` | Run MCP server over stdio |
| `shelltool init [project\|local\|user]` | Install shelltool for your agent |
| `shelltool deinit [project\|local\|user]` | Remove shelltool |
| `shelltool doctor` | Diagnose setup issues |

## Shell Security

Commands are validated against permit/deny pattern rules in a stacked config:

1. **Builtin** — embedded security defaults
2. **User** — `~/.shell/config.yaml`
3. **Project** — `.shell/config.yaml` (at git root)

Permit patterns are checked first (short-circuit allow), then deny patterns block matches. Unmatched commands are allowed by default.

## Works With

Claude Code, Cursor, Windsurf, or any MCP-compatible agent.
