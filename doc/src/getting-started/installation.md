# Installation

Install SwissArmyHammer and configure it for use with Claude Code.

## Prerequisites

- **Claude Code** — For MCP integration (recommended)
- **Git** — For version control features

## Install from Homebrew

```bash
brew install swissarmyhammer/tap/swissarmyhammer-cli
```

This installs all three CLIs: `sah`, `avp`, and `mirdan`.

## Verify Installation

```bash
sah --version
sah doctor
```

The `doctor` command checks your installation and configuration.

## Claude Code Integration

Initialize SwissArmyHammer for your project:

```bash
sah init
```

This does two things:
1. Registers `sah` as an MCP server in `.mcp.json`
2. Creates the project directory with skills and workflows

Set up validators:

```bash
avp init
```

This registers AVP hooks in Claude Code settings and creates the `.avp/` directory.

Verify everything:

```bash
sah doctor
avp doctor
```

## Scope Options

Both `sah init` and `avp init` support different scopes:

| Scope | File | Use Case |
|-------|------|----------|
| `project` | `.mcp.json` / `.claude/settings.json` | Shared with team (default) |
| `local` | Per-project local config | Personal, not committed |
| `user` | `~/.claude.json` / `~/.claude/settings.json` | Applies to all projects |

```bash
sah init user      # Install globally
avp init user      # Install hooks globally
```

## Shell Completions (Optional)

```bash
# Zsh
sah completions zsh > ~/.zfunc/_sah

# Bash
sah completions bash > ~/.bash_completion.d/sah

# Fish
sah completions fish > ~/.config/fish/completions/sah.fish
```

## Next Steps

- [Quick Start](quick-start.md) — Start using the integrated SDLC
