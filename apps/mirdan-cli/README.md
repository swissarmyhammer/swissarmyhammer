<div align="center">

# mirdan

**Universal package manager for AI coding agent skills and validators.**

</div>

---

Mirdan manages skills and validators for AI coding agents. Install community packages, scaffold your own, and publish to the registry. Works across Claude Code, Cursor, Windsurf, and other MCP-compatible agents.

## Install

### macOS (Homebrew)

```bash
brew install swissarmyhammer/tap/mirdan
```

### Linux

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/mirdan-cli-installer.sh | sh
```

### From source

```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer mirdan-cli
```

## Commands

| Command | Description |
|---------|-------------|
| `mirdan agents` | Detect and list installed AI coding agents |
| `mirdan install <package>` | Install a skill or validator (type auto-detected) |
| `mirdan uninstall <name>` | Remove an installed package |
| `mirdan list` | List installed skills and validators |
| `mirdan search <query>` | Search the registry |
| `mirdan info <name>` | Show package details |
| `mirdan outdated` | Check for newer versions |
| `mirdan update [name]` | Update installed packages |
| `mirdan new skill <name>` | Scaffold a new skill |
| `mirdan new validator <name>` | Scaffold a new validator |
| `mirdan publish [path]` | Publish to the registry |
| `mirdan unpublish <name@ver>` | Remove a published version |
| `mirdan login` | Authenticate with registry |
| `mirdan logout` | Revoke token and delete credentials |
| `mirdan whoami` | Show current authenticated user |
| `mirdan doctor` | Diagnose setup and configuration |

## Works With

Claude Code, Cursor, Windsurf, or any MCP-compatible agent.
