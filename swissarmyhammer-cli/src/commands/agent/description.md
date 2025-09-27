Manage and interact with agents in the SwissArmyHammer system.
Agents provide specialized functionality through dedicated workflows
and tools for specific use cases.

The agent system provides two main commands:
• list - Display all available agents from all sources
• use - Apply or execute a specific agent

Use global arguments to control output:
  --verbose         Show detailed information
  --format FORMAT   Output format: table, json, yaml
  --debug           Enable debug mode
  --quiet           Suppress output except errors

Examples:
  sah agent list                           # List all agents
  sah --verbose agent list                 # Show detailed information  
  sah --format=json agent list             # Output as JSON
  sah agent use code-reviewer              # Apply code-reviewer agent
  sah --debug agent use planner            # Use agent with debug output