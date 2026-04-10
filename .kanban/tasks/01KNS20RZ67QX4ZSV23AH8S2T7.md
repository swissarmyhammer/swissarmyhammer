---
assignees:
- claude-code
depends_on:
- 01KNS205FV6083EVT3SDGM75KH
position_column: done
position_ordinal: ffffffffffffffffffff9780
project: code-context-cli
title: Write README and installation documentation for code-context-cli
---
## What
Create `code-context-cli/README.md` mirroring `shelltool-cli/README.md` structure exactly.

### Exact structure (from shelltool-cli/README.md):

```markdown
<div align="center">

<img src="code-context.png" alt="code-context" width="256" height="256">

# code-context

**Code intelligence for AI agents — symbols, search, and call graphs.**

</div>

---

[2–3 sentence description of what code-context does]

## Why

[Bullet-point motivation: what problem it solves, what it replaces, key benefits]

## Install

### macOS (Homebrew)

```bash
brew install swissarmyhammer/tap/code-context
```

### Linux

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/code-context-cli-installer.sh | sh
```

### From source

```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer code-context-cli
```

Then set up the tool:

```bash
code-context init
```

## Commands

| Command | Description |
|---------|-------------|
| `code-context serve` | Run MCP server over stdio |
| `code-context init [project\|local\|user]` | Install for your agent |
| `code-context deinit [project\|local\|user]` | Remove from agent config |
| `code-context doctor` | Diagnose setup issues |
| `code-context skill` | Deploy code-context skill to .skills/ |

## Operations

[Table of all 23+ operations grouped by verb: get, search, list, grep, query, find, build, clear, lsp, detect]

Use `--json` for machine-readable output.

## How It Works

[Brief explanation: .code-context/ index, tree-sitter parsing, LSP integration, auto-population]

## Works With

Claude Code, Cursor, Windsurf, or any MCP-compatible agent.
```

### Key requirements:
- `<img src="code-context.png" ...>` tag at top (references the icon in the crate root)
- Linux installer URL: `code-context-cli-installer.sh` (matches cargo-dist naming convention)
- Homebrew: `brew install swissarmyhammer/tap/code-context` (formula name from Cargo.toml `[package.metadata.dist]`)
- "Works With" section at bottom
- Operations table covers all 23+ ops

## Acceptance Criteria
- [ ] `code-context-cli/README.md` exists
- [ ] Contains `<img src="code-context.png"` tag
- [ ] Install section has Homebrew, Linux curl, and cargo install variants
- [ ] Commands table present
- [ ] Operations table covers all 23+ operations
- [ ] "Works With" section present

## Tests
- [ ] `cargo doc -p code-context-cli` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.