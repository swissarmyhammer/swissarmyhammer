<div align="center">

# avp

**Agent Validator Protocol -- hook processor for AI coding agents.**

</div>

---

AVP intercepts Claude Code hooks and validates agent actions against configurable rulesets. Define what your agent can and cannot do, and AVP enforces it at the hook level -- blocking disallowed actions before they execute.

## Why

AI coding agents are powerful but need guardrails. AVP lets you define validation rules as declarative rulesets that run as Claude Code hooks. When the agent tries to do something your rules don't allow, AVP blocks it with exit code 2 before the action happens.

## Install

### macOS (Homebrew)

```bash
brew install swissarmyhammer/tap/avp
```

### Linux

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/avp-cli-installer.sh | sh
```

### From source

```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer avp-cli
```

Then set up the hooks:

```bash
avp init
```

This installs AVP hooks into your Claude Code settings and creates the `.avp/` directory for rulesets.

## Commands

| Command | Description |
|---------|-------------|
| `avp` (stdin) | Process a hook event from JSON stdin |
| `avp init [project\|local\|user]` | Install AVP hooks into Claude Code settings |
| `avp deinit [project\|local\|user]` | Remove AVP hooks and .avp directory |
| `avp doctor` | Diagnose AVP configuration and setup |
| `avp new <name>` | Create a new RuleSet from template |
| `avp edit <name>` | Edit an existing RuleSet in $EDITOR |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (action allowed) |
| 1 | Error |
| 2 | Blocking error (action rejected by rules) |

## Works With

Claude Code and any agent that supports hook-based validation.
