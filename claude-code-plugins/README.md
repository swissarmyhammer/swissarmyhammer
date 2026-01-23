# SwissArmyHammer Claude Code Plugins

Local marketplace for Claude Code plugins from the SwissArmyHammer project.

## Available Plugins

### avp

Agent Validator Protocol - General purpose hook processor that integrates with all Claude Code events.

## Adding this Marketplace

```bash
/plugin marketplace add /path/to/swissarmyhammer/claude-code-plugins
```

Or via CLI:

```bash
claude plugin marketplace add /path/to/swissarmyhammer/claude-code-plugins
```

## Installing Plugins

After adding the marketplace:

```bash
# Interactive
/plugin

# Or directly
/plugin install avp@swissarmyhammer-plugins
```

## Directory Structure

```
claude-code-plugins/
├── .claude-plugin/
│   └── marketplace.json    # Marketplace manifest
├── avp-plugin/             # AVP plugin
│   ├── .claude-plugin/
│   │   └── plugin.json     # Plugin manifest
│   ├── hooks/
│   │   └── hooks.json      # Hook configurations
│   └── README.md
└── README.md               # This file
```

## Development

To test a plugin during development:

```bash
claude --plugin-dir ./avp-plugin
```
