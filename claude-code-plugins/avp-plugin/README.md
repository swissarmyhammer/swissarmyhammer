# AVP Plugin for Claude Code

Agent Validator Protocol (AVP) plugin that hooks into all Claude Code events for general-purpose validation and processing.

## Prerequisites

The `avp` binary must be installed and available in your PATH:

```bash
# From the swissarmyhammer repository root
cargo install --path avp-cli
```

Or build and add to PATH manually:

```bash
cargo build --release -p avp-cli
# Add target/release to your PATH, or copy the binary
cp target/release/avp ~/.local/bin/
```

## Installation

### Option 1: From GitHub

```bash
/plugin marketplace add https://github.com/wballard/swissarmyhammer
/plugin install avp@swissarmyhammer-plugins
```

### Option 2: From local clone

```bash
/plugin marketplace add /path/to/swissarmyhammer
/plugin install avp@swissarmyhammer-plugins
```

### Option 3: For development/testing

```bash
claude --plugin-dir /path/to/swissarmyhammer/claude-code-plugins/avp-plugin
```

## What it does

This plugin registers the `avp` command as a hook for all Claude Code events:

- **PreToolUse** - Before Claude uses any tool
- **PostToolUse** - After successful tool execution
- **PostToolUseFailure** - After tool execution fails
- **PermissionRequest** - When a permission dialog is shown
- **UserPromptSubmit** - When user submits a prompt
- **Notification** - When Claude Code sends notifications
- **Stop** - When Claude attempts to stop
- **SubagentStart** - When a subagent is started
- **SubagentStop** - When a subagent stops
- **Setup** - During initialization
- **SessionStart** - At session start
- **SessionEnd** - At session end
- **PreCompact** - Before conversation compaction

## Configuration

The `avp` binary reads JSON from stdin and outputs JSON to stdout. Configure AVP behavior through its own configuration mechanisms (see avp-cli documentation).

## Exit Codes

- `0` - Success, continue execution
- `2` - Blocking error, reject the action
