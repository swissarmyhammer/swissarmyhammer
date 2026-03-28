<div align="center">

<img src="shelltool.png" alt="shelltool" width="256" height="256">

# shelltool

**Replaces Bash/exec with a searchable shell that saves tokens.**

</div>

---

shelltool replaces built-in Bash and exec CLI tools with a persistent, searchable virtual shell. Instead of dumping thousands of tokens of raw command output into the agent's context window, shelltool stores everything in history. The agent runs commands, then searches or retrieves just the lines it needs.

## Why

Built-in Bash and exec tools flood the context with every byte of stdout. A single `cargo test` can burn 10,000+ tokens of output the agent will never look at again. shelltool replaces them with a shell that stores all output in searchable history and returns only what matters:

- **Run a command** — get a summary with exit code and line count
- **Grep the output** — regex search across history for error codes, function names, patterns
- **Semantic search** — find output by meaning ("authentication error" matches "403 forbidden")
- **Get specific lines** — retrieve exactly the range you need for context
- **Manage processes** — list running commands, kill hung processes

The agent uses the shell like a person does: run something, scan the results, dig into the interesting parts.

## Install

```bash
brew install swissarmyhammer/tap/shelltool
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
