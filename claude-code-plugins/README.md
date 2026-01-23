# SwissArmyHammer Claude Code Plugins

Plugins for Claude Code from the SwissArmyHammer project.

## Available Plugins

### avp

Agent Validator Protocol - General purpose hook processor that integrates with all Claude Code events.

## Installation

The marketplace manifest is at the repo root (`.claude-plugin/marketplace.json`) for GitHub compatibility.

### From GitHub

```bash
/plugin marketplace add https://github.com/wballard/swissarmyhammer
/plugin install avp@swissarmyhammer-plugins
```

### From local clone

```bash
/plugin marketplace add /path/to/swissarmyhammer
/plugin install avp@swissarmyhammer-plugins
```

## Directory Structure

```
swissarmyhammer/
├── .claude-plugin/
│   └── marketplace.json    # Marketplace manifest (repo root)
└── claude-code-plugins/
    └── avp-plugin/         # AVP plugin
        ├── .claude-plugin/
        │   └── plugin.json
        ├── hooks/
        │   └── hooks.json
        └── README.md
```
