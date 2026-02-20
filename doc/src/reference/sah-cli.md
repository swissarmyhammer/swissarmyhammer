# Command-Line Help for `swissarmyhammer`

This document contains the help content for the `swissarmyhammer` command-line program.

## Installation

```bash
brew install swissarmyhammer/tap/swissarmyhammer
```

**Command Overview:**

* [`swissarmyhammer`↴](#swissarmyhammer)
* [`swissarmyhammer serve`↴](#swissarmyhammer-serve)
* [`swissarmyhammer serve http`↴](#swissarmyhammer-serve-http)
* [`swissarmyhammer init`↴](#swissarmyhammer-init)
* [`swissarmyhammer deinit`↴](#swissarmyhammer-deinit)
* [`swissarmyhammer doctor`↴](#swissarmyhammer-doctor)
* [`swissarmyhammer prompt`↴](#swissarmyhammer-prompt)
* [`swissarmyhammer flow`↴](#swissarmyhammer-flow)
* [`swissarmyhammer completion`↴](#swissarmyhammer-completion)
* [`swissarmyhammer validate`↴](#swissarmyhammer-validate)
* [`swissarmyhammer model`↴](#swissarmyhammer-model)
* [`swissarmyhammer model list`↴](#swissarmyhammer-model-list)
* [`swissarmyhammer model show`↴](#swissarmyhammer-model-show)
* [`swissarmyhammer model use`↴](#swissarmyhammer-model-use)
* [`swissarmyhammer agent`↴](#swissarmyhammer-agent)
* [`swissarmyhammer agent acp`↴](#swissarmyhammer-agent-acp)

## `swissarmyhammer`


swissarmyhammer is an MCP (Model Context Protocol) server that manages
prompts as markdown files. It supports file watching, template substitution,
and seamless integration with Claude Code.

Global arguments can be used with any command to control output and behavior:
  --verbose     Show detailed information and debug output
  --format      Set output format (table, json, yaml) for commands that support it  
  --debug       Enable debug mode with comprehensive tracing
  --quiet       Suppress all output except errors
  --model       Override model for all use cases (runtime only, doesn't modify config)

Main commands:
  serve         Run as MCP server (default when invoked via stdio)
  doctor        Diagnose configuration and setup issues
  prompt        Manage and test prompts with interactive capabilities
  flow          Execute and manage workflows for complex task automation
  agent         Manage and interact with specialized agents for specific use cases
  validate      Validate prompt files and workflows for syntax and best practices
  completion    Generate shell completion scripts

Example usage:
  swissarmyhammer serve                           # Run as MCP server
  swissarmyhammer doctor                          # Check configuration
  swissarmyhammer --verbose prompt list          # List prompts with details
  swissarmyhammer --format=json prompt list      # List prompts as JSON
  swissarmyhammer --debug prompt test help       # Test prompt with debug info
  swissarmyhammer agent list                     # List available agents
  swissarmyhammer agent use claude-code          # Apply Claude Code agent to project
  swissarmyhammer flow run code-review           # Execute code review workflow


**Usage:** `swissarmyhammer [OPTIONS] [COMMAND]`

###### **Subcommands:**

* `serve` — Run as MCP server (default when invoked via stdio)
* `init` — Initialize sah MCP server in Claude Code settings
* `deinit` — Remove sah MCP server from Claude Code settings
* `doctor` — Diagnose configuration and setup issues
* `prompt` — Manage and test prompts
* `flow` — Execute and manage workflows
* `completion` — Generate shell completion scripts
* `validate` — Validate prompt files and workflows for syntax and best practices
* `model` — Manage and interact with models
* `agent` — Manage and interact with Agent Client Protocol server

###### **Options:**

* `-v`, `--verbose` — Enable verbose logging
* `-d`, `--debug` — Enable debug logging
* `-q`, `--quiet` — Suppress all output except errors
* `--format <FORMAT>` — Global output format

  Possible values: `table`, `json`, `yaml`

* `--model <MODEL>` — Override model for all use cases (runtime only, doesn't modify config)



## `swissarmyhammer serve`


Run as MCP server. This is the default mode when
invoked via stdio (e.g., by Claude Code). The server will:

- Load all prompts from builtin, user, and local directories
- Watch for file changes and reload prompts automatically  
- Expose prompts via the MCP protocol
- Support template substitution with {{variables}}

Example:
  swissarmyhammer serve        # Stdio mode (default)
  swissarmyhammer serve http   # HTTP mode
  # Or configure in Claude Code's MCP settings


**Usage:** `swissarmyhammer serve [COMMAND]`

###### **Subcommands:**

* `http` — Start HTTP MCP server



## `swissarmyhammer serve http`


Start HTTP MCP server for web clients, debugging, and LlamaAgent integration.
The server exposes MCP tools through HTTP endpoints and provides:

- RESTful MCP protocol implementation
- Health check endpoint at /health
- Support for random port allocation (use port 0)
- Graceful shutdown with Ctrl+C

Example:
  swissarmyhammer serve http --port 8080 --host 127.0.0.1
  swissarmyhammer serve http --port 0  # Random port


**Usage:** `swissarmyhammer serve http [OPTIONS]`

###### **Options:**

* `-p`, `--port <PORT>` — Port to bind to (use 0 for random port)

  Default value: `8000`
* `-H`, `--host <HOST>` — Host to bind to

  Default value: `127.0.0.1`



## `swissarmyhammer init`


Initialize SwissArmyHammer for use with Claude Code.

This command:
1. Registers sah as an MCP server in Claude Code settings
2. Creates the .swissarmyhammer/ project directory (workflows/) and .prompts/

The command is idempotent - safe to run multiple times.

Targets:
  project   Write to .mcp.json (default, shared with team via git)
  local     Write to ~/.claude.json per-project config (personal, not committed)
  user      Write to ~/.claude.json global config (all projects)

Examples:
  sah init              # Project-level setup (default)
  sah init user         # Global setup for all projects
  sah init local        # Personal setup, not committed to git


**Usage:** `swissarmyhammer init [TARGET]`

###### **Arguments:**

* `<TARGET>` — Where to install the MCP server configuration

  Default value: `project`

  Possible values:
  - `project`:
    Project-level settings (.claude/settings.json)
  - `local`:
    Local project settings, not committed (.claude/settings.local.json)
  - `user`:
    User-level settings (~/.claude/settings.json)




## `swissarmyhammer deinit`


Remove SwissArmyHammer MCP server configuration from Claude Code settings.

By default, only the MCP server entry is removed from the settings file.
Use --remove-directory to also delete the .swissarmyhammer/ project directory.

Examples:
  sah deinit                     # Remove from project settings
  sah deinit user                # Remove from user settings
  sah deinit --remove-directory  # Also remove .swissarmyhammer/


**Usage:** `swissarmyhammer deinit [OPTIONS] [TARGET]`

###### **Arguments:**

* `<TARGET>` — Where to remove the MCP server configuration from

  Default value: `project`

  Possible values:
  - `project`:
    Project-level settings (.claude/settings.json)
  - `local`:
    Local project settings, not committed (.claude/settings.local.json)
  - `user`:
    User-level settings (~/.claude/settings.json)


###### **Options:**

* `--remove-directory` — Also remove .swissarmyhammer/ project directory



## `swissarmyhammer doctor`

Diagnose and troubleshoot your SwissArmyHammer setup in seconds.

Save hours of debugging time with comprehensive automated checks that identify
configuration issues, permission problems, and integration errors before they
impact your workflow.

WHAT IT CHECKS

The doctor command runs a complete health assessment of your environment:
• PATH Configuration - Verifies swissarmyhammer is accessible from your shell
• Claude Code Integration - Validates MCP server configuration and connectivity
• Prompt System - Checks directories, file permissions, and YAML syntax
• File Watching - Tests file system event monitoring capabilities
• System Resources - Validates required dependencies and system capabilities

WHY USE DOCTOR

• Quick Diagnosis - Complete system check in seconds, not hours
• Clear Reporting - Easy-to-understand pass/fail results with actionable guidance
• Early Detection - Catch configuration problems before they cause failures
• Setup Validation - Verify your installation is working correctly
• Integration Testing - Ensure Claude Code and MCP are properly connected

UNDERSTANDING RESULTS

Exit codes indicate the severity of findings:
  0 - All checks passed - System is healthy and ready
  1 - Warnings found - System works but has recommendations
  2 - Errors found - Critical issues preventing proper operation

COMMON WORKFLOWS

First-time setup verification:
  swissarmyhammer doctor

Detailed diagnostic output:
  swissarmyhammer doctor --verbose

After configuration changes:
  swissarmyhammer doctor

CI/CD health checks:
  swissarmyhammer doctor && echo "System ready"

EXAMPLES

Basic health check:
  swissarmyhammer doctor

Detailed diagnostics with fix suggestions:
  swissarmyhammer doctor --verbose

Quiet mode for scripting:
  swissarmyhammer doctor --quiet

The doctor command gives you confidence that your development environment
is properly configured and ready for AI-powered workflows.

**Usage:** `swissarmyhammer doctor`



## `swissarmyhammer prompt`


Manage and test prompts with a clean, simplified interface.

The prompt system provides two main commands:
• list - Display all available prompts from all sources  
• test - Test prompts interactively with sample data

Use global arguments to control output:
  --verbose         Show detailed information
  --format FORMAT   Output format: table, json, yaml
  --debug           Enable debug mode
  --quiet           Suppress output except errors

Examples:
  sah prompt list                           # List all prompts
  sah --verbose prompt list                 # Show detailed information
  sah --format=json prompt list             # Output as JSON
  sah prompt test code-review               # Interactive testing
  sah prompt test help --var topic=git      # Test with parameters  
  sah --debug prompt test plan              # Test with debug output


**Usage:** `swissarmyhammer prompt [ARGS]...`

###### **Arguments:**

* `<ARGS>` — Subcommand and arguments for prompt (handled dynamically)



## `swissarmyhammer flow`

Automate complex development workflows with powerful, resumable state machines.

Workflows orchestrate multi-step processes including code reviews, deployments,
testing, and AI-powered operations. Define once, run reliably, resume anywhere.

WORKFLOW POWER

State Machine Architecture:
• Define complex processes as declarative state machines
• Execute actions, tools, and AI commands in sequence or parallel
• Handle conditional logic, loops, and error recovery
• Resume interrupted workflows exactly where they stopped
• Track execution state, variables, and progress

Built for Reliability:
• Pause and resume workflows without losing state
• Interactive mode for step-by-step control and debugging
• Dry-run mode to preview execution without side effects
• Comprehensive logging and status tracking
• Automatic error handling and recovery options

AI Integration:
• Execute Claude commands within workflow steps
• Pass variables and context between AI operations
• Chain AI-powered analysis, planning, and implementation
• Combine automated and AI-assisted tasks seamlessly

COMMANDS

The flow system provides comprehensive workflow management:

• run - Start a new workflow execution with variables
• resume - Continue a paused or interrupted workflow run
• list - Display all available workflows from all sources
• status - Check execution state and progress of a run
• logs - View detailed execution logs and step history

WORKFLOW DISCOVERY

Workflows are loaded from multiple sources:
• Built-in workflows - Standard development workflows included
• User workflows (~/.swissarmyhammer/workflows/) - Personal automations
• Project workflows (./workflows/) - Project-specific processes

COMMON WORKFLOWS

Start a workflow with parameters:
  swissarmyhammer flow run code-review --vars file=main.rs

Preview execution without running:
  swissarmyhammer flow run deploy --dry-run

Resume after interruption:
  swissarmyhammer flow resume a1b2c3d4

Interactive step-through debugging:
  swissarmyhammer flow resume a1b2c3d4 --interactive

Monitor workflow status:
  swissarmyhammer flow status a1b2c3d4 --watch

View execution history:
  swissarmyhammer flow logs a1b2c3d4

List available workflows:
  swissarmyhammer flow list --format json

EXECUTION OPTIONS

Pass variables to workflows:
  --vars key=value              # Single variable
  --vars file=main.rs --vars author=jane  # Multiple variables

Control execution:
  --interactive                 # Step-by-step confirmation
  --dry-run                     # Show plan without executing

EXAMPLES

Run code review workflow:
  swissarmyhammer flow run code-review --vars file=main.rs --vars severity=high

Test deployment workflow:
  swissarmyhammer flow run deploy --dry-run

Resume interrupted workflow:
  swissarmyhammer flow resume a1b2c3d4 --interactive

Check workflow status:
  swissarmyhammer flow status a1b2c3d4

View execution logs:
  swissarmyhammer flow logs a1b2c3d4 --format json

List available workflows:
  swissarmyhammer flow list

Workflows bring automation, reliability, and AI-powered intelligence to your
development processes. Define complex operations once and execute them
consistently every time.

**Usage:** `swissarmyhammer flow [ARGS]...`

###### **Arguments:**

* `<ARGS>` — Workflow name or 'list' command followed by arguments



## `swissarmyhammer completion`


Generates shell completion scripts for various shells. Supports:
- bash
- zsh
- fish
- powershell

Examples:
  # Bash (add to ~/.bashrc or ~/.bash_profile)
  swissarmyhammer completion bash > ~/.local/share/bash-completion/completions/swissarmyhammer
  
  # Zsh (add to ~/.zshrc or a file in fpath)
  swissarmyhammer completion zsh > ~/.zfunc/_swissarmyhammer
  
  # Fish
  swissarmyhammer completion fish > ~/.config/fish/completions/swissarmyhammer.fish
  
  # PowerShell
  swissarmyhammer completion powershell >> $PROFILE


**Usage:** `swissarmyhammer completion <SHELL>`

###### **Arguments:**

* `<SHELL>` — Shell to generate completion for

  Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`




## `swissarmyhammer validate`

Catch configuration errors before they cause failures with comprehensive validation.

The validate command ensures quality and correctness across your entire
SwissArmyHammer configuration, detecting syntax errors, structural issues,
and best practice violations before they impact your workflows.

## Quality Assurance

Comprehensive Validation:
• Prompt files from all sources (builtin, user, project)
• Workflow definitions from standard locations
• MCP tool schemas and CLI integration (with --validate-tools)
• Template syntax and variable usage
• YAML frontmatter structure
• Required field presence and format
• Best practice compliance

Early Error Detection:
• Find syntax errors before execution
• Identify missing required fields
• Detect template variable mismatches
• Validate workflow state machine structure
• Check MCP tool schema correctness
• Verify CLI integration compatibility

CI/CD Integration:
• Automated quality checks in build pipelines
• Exit codes indicate validation results
• Quiet mode for clean CI output
• JSON output for tool integration
• Fast execution for rapid feedback

## What Gets Validated

Prompt Files:
• YAML frontmatter syntax correctness
• Required fields: title, description
• Template variable declarations match usage
• Liquid template syntax validity
• Parameter definitions and types
• Default value correctness
• Partial marker handling

Workflow Files:
• State machine structure integrity
• State connectivity and transitions
• Action and tool references
• Variable declarations and usage
• Conditional logic syntax
• Loop and iteration constructs
• Error handling configuration

MCP Tools (with --validate-tools):
• JSON schema correctness
• Parameter type definitions
• Required vs optional field specifications
• Tool description completeness
• CLI integration requirements
• Documentation quality
• Best practice adherence

## Validation Modes

Standard validation (prompts and workflows):
```bash
sah validate
```

Comprehensive validation (including MCP tools):
```bash
sah validate --validate-tools
```

CI/CD mode (errors only, no warnings):
```bash
sah validate --quiet
sah validate --validate-tools --quiet
```

Machine-readable output:
```bash
sah validate --format json
sah validate --validate-tools --format json
```

## Exit Codes

- `0` - All validation passed, no errors or warnings
- `1` - Warnings found but no errors
- `2` - Errors found that require fixes

Use exit codes in scripts and CI pipelines:
```bash
sah validate || exit 1
```

## Discovery and Sources

Prompts validated from:
• Built-in prompts (embedded in binary)
• User prompts (~/.prompts/)
• Project prompts (./.prompts/)

Workflows validated from:
• Built-in workflows (embedded in binary)
• User workflows (~/.swissarmyhammer/workflows/)
• Project workflows (./workflows/)

MCP tools validated from:
• SwissArmyHammer tool definitions
• CLI command integration points
• Tool parameter schemas

## Common Use Cases

Pre-commit validation:
```bash
sah validate --quiet && git commit
```

CI pipeline check:
```bash
sah validate --validate-tools --format json > validation-report.json
```

Development workflow validation:
```bash
sah validate --verbose
```

Quality gate in deployment:
```bash
sah validate --validate-tools --quiet || exit 1
```

## Validation Checks

YAML Frontmatter:
• Syntax correctness
• Required fields present
• Field types match expectations
• Valid enum values

Template Syntax:
• Liquid template parsing
• Variable references exist
• Filter syntax correctness
• Control flow validity
• Partial references resolve

Workflow Structure:
• All states are reachable
• Transitions are valid
• Actions reference existing tools
• Variables are declared before use
• Error handlers are properly configured

MCP Tool Schemas:
• JSON schema validity
• Parameter type correctness
• Required field specification
• Tool description quality
• CLI integration completeness

Best Practices:
• Descriptive titles and descriptions
• Proper parameter documentation
• Sensible default values
• Clear error messages
• Consistent naming conventions

## Examples

Basic validation:
```bash
sah validate
```

Full system validation:
```bash
sah validate --validate-tools
```

Quiet mode for CI:
```bash
sah validate --quiet
```

Detailed output:
```bash
sah --verbose validate
```

JSON output for tooling:
```bash
sah validate --format json | jq '.errors'
```

Validate after changes:
```bash
sah validate --validate-tools --verbose
```

## Output Formats

Table format (default):
• Human-readable tabular output
• Color-coded error/warning levels
• File paths and line numbers
• Clear error descriptions

JSON format:
• Machine-parseable structured output
• Complete error and warning details
• Suitable for CI integration
• Easy tool consumption

YAML format:
• Human-readable structured output
• Hierarchical error organization
• Good for documentation
• Easy diff comparison

## Troubleshooting

Validation errors in prompts:
• Check YAML frontmatter syntax
• Verify all required fields present
• Ensure template variables declared
• Test Liquid template syntax

Validation errors in workflows:
• Verify state machine structure
• Check all state transitions
• Ensure action references valid
• Validate variable declarations

Validation errors in tools:
• Review JSON schema correctness
• Check parameter type definitions
• Verify required fields specified
• Ensure documentation complete

## Integration with Development Workflow

Pre-commit hook:
```bash
#!/bin/bash
sah validate --quiet || {
  echo "Validation failed. Fix errors before committing."
  exit 1
}
```

Git hook (.git/hooks/pre-commit):
```bash
#!/bin/bash
sah validate --validate-tools --quiet
```

Make target:
```makefile
validate:
	sah validate --validate-tools --quiet

.PHONY: validate
```

CI pipeline (GitHub Actions):
```yaml
- name: Validate Configuration
  run: sah validate --validate-tools --format json
```

## Benefits

Catch Errors Early:
• Find problems before runtime
• Prevent workflow failures
• Avoid wasted execution time
• Reduce debugging effort

Ensure Quality:
• Enforce best practices
• Maintain consistent standards
• Improve documentation quality
• Promote good patterns

Enable Confidence:
• Deploy with certainty
• Refactor safely
• Share configuration reliably
• Integrate automatically

Support Automation:
• CI/CD quality gates
• Automated testing
• Pre-commit validation
• Continuous quality monitoring

The validate command is your quality assurance system for SwissArmyHammer
configuration, ensuring that prompts, workflows, and tools are correct,
complete, and ready for reliable operation.

**Usage:** `swissarmyhammer validate [OPTIONS]`

###### **Options:**

* `-q`, `--quiet` — Suppress all output except errors. In quiet mode, warnings are hidden from both output and summary
* `--format <FORMAT>` — Output format

  Default value: `table`

  Possible values: `table`, `json`, `yaml`

* `--validate-tools` — Validate MCP tool schemas for CLI compatibility



## `swissarmyhammer model`

Manage and interact with models in the SwissArmyHammer system.

Models provide specialized AI execution environments and configurations for specific
development workflows. They enable you to switch between different AI models, 
execution contexts, and toolchains based on your project's needs.

MODEL DISCOVERY AND PRECEDENCE

Models are loaded from multiple sources with hierarchical precedence:
• Built-in models (lowest precedence) - Embedded in the binary
• Project models (medium precedence) - ./models/*.yaml in your project
• User models (highest precedence) - ~/.swissarmyhammer/models/*.yaml

Higher precedence models override lower ones by name. This allows you to
customize built-in models or create project-specific variants.

BUILT-IN MODELS

The system includes these built-in models:
• claude-code    - Default Claude Code integration with shell execution
• qwen-coder     - Local Qwen3-Coder model with in-process execution

COMMANDS

The model system provides two main commands:
• list - Display all available models from all sources with descriptions
• use - Apply a model configuration to the current project

When you 'use' a model, it creates or updates .swissarmyhammer/sah.yaml in your
project with the model's configuration. This configures how SwissArmyHammer 
executes AI workflows in your project.

COMMON WORKFLOWS

1. Explore available models:
   sah model list

2. Apply a model to your project:
   sah model use claude-code

3. Switch to a different model:
   sah model use qwen-coder

4. View detailed model information:
   sah --verbose model list

Use global arguments to control output:
  --verbose         Show detailed information and descriptions
  --format FORMAT   Output format: table, json, yaml
  --debug           Enable debug mode with comprehensive tracing
  --quiet           Suppress output except errors

Examples:
  sah model list                           # List all available models
  sah --verbose model list                 # Show detailed information and descriptions
  sah --format=json model list             # Output as structured JSON
  sah model use claude-code                # Apply Claude Code model to project
  sah model use qwen-coder                 # Switch to local Qwen3-Coder model
  sah --debug model use custom-model       # Apply model with debug output

CUSTOMIZATION

Create custom models by adding .yaml files to:
• ./models/ (project-specific models)
• ~/.swissarmyhammer/models/ (user-wide models)

Custom models can override built-in models by using the same name, or
provide entirely new configurations for specialized workflows.

**Usage:** `swissarmyhammer model [COMMAND]`

###### **Subcommands:**

* `list` — List available models
* `show` — Show current model use case assignments
* `use` — Use a specific model



## `swissarmyhammer model list`


List all available models from built-in, project, and user sources.

Models are discovered with hierarchical precedence where user models override
project models, which override built-in models. This command shows all available
models with their sources and descriptions.

Built-in models are embedded in the binary and provide default configurations
for common workflows. Project models (./models/*.yaml) allow customization for
specific projects. User models (~/.swissarmyhammer/models/*.yaml) provide
personal configurations that apply across all projects.

Output includes:
• Model name and source (built-in, project, or user)
• Description when available
• Current model status (if one is applied to the project)

Examples:
  sah model list                           # List all models in table format
  sah model list --format json            # Output as JSON for processing
  sah --verbose model list                 # Include detailed descriptions
  sah --quiet model list                   # Only show model names


**Usage:** `swissarmyhammer model list [OPTIONS]`

###### **Options:**

* `--format <FORMAT>` — Output format

  Default value: `table`

  Possible values: `table`, `json`, `yaml`




## `swissarmyhammer model show`


Display current model use case assignments showing which model is configured
for each use case in the project.

SwissArmyHammer supports configuring different models for different use cases:
• root      - Default model for general operations
• workflows - Model for workflow execution (plan, review, implement, etc.)

This command shows the current assignment for each use case, including whether
the assignment comes from explicit configuration or falls back to the root model.

Examples:
  sah model show                           # Show use case assignments
  sah model                               # Same as 'show' (default)


**Usage:** `swissarmyhammer model show [OPTIONS]`

###### **Options:**

* `--format <FORMAT>` — Output format

  Default value: `table`

  Possible values: `table`, `json`, `yaml`




## `swissarmyhammer model use`


Apply a specific model configuration to the current project or for a specific use case.

This command finds the specified model by name and applies its configuration
to the project by creating or updating .swissarmyhammer/sah.yaml. The model
configuration determines how SwissArmyHammer executes AI workflows in your
project, including which AI model to use and how to execute tools.

SwissArmyHammer supports configuring different models for different use cases:
• root      - Default model for general operations (default when use case not specified)
• workflows - Model for workflow execution (plan, review, implement, etc.)

Model precedence (highest to lowest):
• User models: ~/.swissarmyhammer/models/<name>.yaml
• Project models: ./models/<name>.yaml
• Built-in models: embedded in the binary

The command preserves any existing configuration sections while updating
only the model configuration. This allows you to maintain project-specific
settings alongside model configurations.

Common model types:
• claude-code    - Uses Claude Code CLI for AI execution
• qwen-coder     - Uses local Qwen3-Coder model with in-process execution
• custom models  - User-defined configurations for specialized workflows

Examples:
  sah model use claude-code                # Apply Claude Code model to root use case
  sah model use root claude-code           # Apply Claude Code model to root use case (explicit)
  sah model use workflows claude-code      # Apply Claude Code to workflows use case
  sah --debug model use claude-code        # Apply with debug output


**Usage:** `swissarmyhammer model use <first> [second]`

###### **Arguments:**

* `<first>` — First argument: either model name (sets root) OR use case (root, workflows)
* `<second>` — Second argument: model name (required when first argument is a use case)



## `swissarmyhammer agent`


Manage and interact with Agent Client Protocol (ACP) server.

The agent command provides integration with ACP-compatible code editors,
enabling local LLaMA models to be used as coding assistants in editors
like Zed and JetBrains IDEs.

Subcommands:
  acp     Start ACP server over stdio for editor integration

Examples:
  sah agent acp                        # Start ACP server (stdio)
  sah agent acp --config config.yaml  # Start with custom config


**Usage:** `swissarmyhammer agent [COMMAND]`

###### **Subcommands:**

* `acp` — Start ACP server over stdio



## `swissarmyhammer agent acp`


Start Agent Client Protocol (ACP) server for code editor integration.

The ACP server enables SwissArmyHammer to work with ACP-compatible code editors
like Zed and JetBrains IDEs. The server communicates over stdin/stdout using
JSON-RPC 2.0 protocol.

Features:
• Local LLaMA model execution for coding assistance
• Session management with conversation history
• File system operations (read/write)
• Terminal execution
• Tool integration via MCP servers
• Permission-based security model

Examples:
  sah agent acp                        # Start with default config
  sah agent acp --config acp.yaml      # Start with custom config
  sah agent acp --permission-policy auto-approve-reads
  sah agent acp --allow-path /home/user/projects --block-path /home/user/.ssh
  sah agent acp --max-file-size 5242880 --terminal-buffer-size 2097152

Configuration:
Options can be specified via:
1. Command-line flags (highest priority)
2. Configuration file (--config)
3. Default values (lowest priority)

Command-line flags override configuration file settings.

For editor configuration:
• Zed: Add to agents section in settings
• JetBrains: Install ACP plugin and configure


**Usage:** `swissarmyhammer agent acp [OPTIONS]`

###### **Options:**

* `-c`, `--config <CONFIG>` — Path to ACP configuration file (optional)
* `--permission-policy <POLICY>` — Permission policy: always-ask, auto-approve-reads
* `--allow-path <PATH>` — Allowed filesystem paths (can be specified multiple times)
* `--block-path <PATH>` — Blocked filesystem paths (can be specified multiple times)
* `--max-file-size <BYTES>` — Maximum file size for read operations in bytes
* `--terminal-buffer-size <BYTES>` — Terminal output buffer size in bytes
* `--graceful-shutdown-timeout <SECONDS>` — Graceful shutdown timeout in seconds



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
