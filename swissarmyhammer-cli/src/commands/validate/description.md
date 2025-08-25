Validates prompt files, workflows, and optionally MCP tools for syntax errors and best practices.

This command comprehensively validates:
- All prompt files from builtin, user, and local directories
- All workflow files from standard locations (builtin, user, local)
- MCP tools (when --validate-tools flag is specified)

NOTE: The --workflow-dir parameter is deprecated and will be ignored.
Workflows are now only loaded from standard locations.

Validation checks:
- YAML front matter syntax (skipped for .liquid files with {% partial %} marker)
- Required fields (title, description)
- Template variables match arguments
- Liquid template syntax
- Workflow structure and connectivity
- MCP tool schema validation (with --validate-tools)
- CLI integration compatibility (with --validate-tools)
- Best practice recommendations

Examples:
  swissarmyhammer validate                 # Validate all prompts and workflows
  swissarmyhammer validate --validate-tools  # Validate prompts, workflows AND MCP tools
  swissarmyhammer validate --quiet         # CI/CD mode - only shows errors, hides warnings
  swissarmyhammer validate --validate-tools --quiet  # Validate all including tools, quiet mode
  swissarmyhammer validate --format json   # JSON output for tooling
  swissarmyhammer validate --validate-tools --format json  # JSON output with tool validation