Manage and interact with agents in the SwissArmyHammer system.

Agents provide specialized AI execution environments and configurations for specific
development workflows. They enable you to switch between different AI models, 
execution contexts, and toolchains based on your project's needs.

AGENT DISCOVERY AND PRECEDENCE

Agents are loaded from multiple sources with hierarchical precedence:
• Built-in agents (lowest precedence) - Embedded in the binary
• Project agents (medium precedence) - ./agents/*.yaml in your project
• User agents (highest precedence) - ~/.swissarmyhammer/agents/*.yaml

Higher precedence agents override lower ones by name. This allows you to
customize built-in agents or create project-specific variants.

BUILT-IN AGENTS

The system includes these built-in agents:
• claude-code    - Default Claude Code integration with shell execution
• qwen-coder     - Local Qwen3-Coder model with in-process execution

COMMANDS

The agent system provides two main commands:
• list - Display all available agents from all sources with descriptions
• use - Apply an agent configuration to the current project

When you 'use' an agent, it creates or updates .swissarmyhammer/sah.yaml in your
project with the agent's configuration. This configures how SwissArmyHammer 
executes AI workflows in your project.

COMMON WORKFLOWS

1. Explore available agents:
   sah agent list

2. Apply an agent to your project:
   sah agent use claude-code

3. Switch to a different agent:
   sah agent use qwen-coder

4. View detailed agent information:
   sah --verbose agent list

Use global arguments to control output:
  --verbose         Show detailed information and descriptions
  --format FORMAT   Output format: table, json, yaml
  --debug           Enable debug mode with comprehensive tracing
  --quiet           Suppress output except errors

Examples:
  sah agent list                           # List all available agents
  sah --verbose agent list                 # Show detailed information and descriptions
  sah --format=json agent list             # Output as structured JSON
  sah agent use claude-code                # Apply Claude Code agent to project
  sah agent use qwen-coder                 # Switch to local Qwen3-Coder model
  sah --debug agent use custom-agent       # Apply agent with debug output

CUSTOMIZATION

Create custom agents by adding .yaml files to:
• ./agents/ (project-specific agents)
• ~/.swissarmyhammer/agents/ (user-wide agents)

Custom agents can override built-in agents by using the same name, or
provide entirely new configurations for specialized workflows.