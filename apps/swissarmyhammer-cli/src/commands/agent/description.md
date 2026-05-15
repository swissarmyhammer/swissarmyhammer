
Manage and interact with Agent Client Protocol (ACP) server.

The agent command provides integration with ACP-compatible code editors,
enabling local LLaMA models to be used as coding assistants in editors
like Zed and JetBrains IDEs.

Subcommands:
  acp     Start ACP server over stdio for editor integration

Examples:
  sah agent acp                        # Start ACP server (stdio)
  sah agent acp --config config.yaml  # Start with custom config
