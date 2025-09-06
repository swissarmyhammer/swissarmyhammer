# CLI Reference

Complete reference for all SwissArmyHammer command-line interface commands.

## Global Options

Available for all commands:

```bash
sah [GLOBAL_OPTIONS] <COMMAND> [COMMAND_OPTIONS]
```

| Option | Description | Default |
|--------|-------------|---------|
| `--help, -h` | Show help information | |
| `--version, -V` | Show version information | |
| `--config, -c <FILE>` | Configuration file path | Auto-detected |
| `--log-level <LEVEL>` | Log level (trace, debug, info, warn, error) | `info` |
| `--quiet, -q` | Suppress output | `false` |
| `--verbose, -v` | Verbose output | `false` |

## Main Commands

### `sah serve`

Run SwissArmyHammer as an MCP server for Claude Code integration.

```bash
sah serve [OPTIONS]
```

**Options:**
- `--stdio` - Use stdin/stdout transport (default when run by Claude Code)
- `--port <PORT>` - TCP port to bind to
- `--host <HOST>` - Host address to bind to (default: localhost)
- `--timeout <MS>` - Request timeout in milliseconds (default: 30000)

**Examples:**
```bash
# Run as MCP server (typical usage)
sah serve

# Run on specific port
sah serve --port 8080 --host 0.0.0.0

# Run with custom timeout
sah serve --timeout 60000
```

### `sah doctor`

Diagnose installation and configuration issues.

```bash
sah doctor [OPTIONS]
```

**Options:**
- `--fix` - Automatically fix detected issues
- `--check <CHECK>` - Run specific check only
- `--format <FORMAT>` - Output format (table, json, markdown)

**Examples:**
```bash
# Run all diagnostic checks
sah doctor

# Fix issues automatically
sah doctor --fix

# Check only MCP integration
sah doctor --check mcp

# Output as JSON
sah doctor --format json
```

## Prompt Commands

### `sah prompt list`

List available prompts from all sources.

```bash
sah prompt list [OPTIONS]
```

**Options:**
- `--source <SOURCE>` - Filter by source (builtin, user, local)
- `--tag <TAG>` - Filter by tag
- `--format <FORMAT>` - Output format (table, json, list)


**Examples:**
```bash
# List all prompts
sah prompt list

# List built-in prompts only
sah prompt list --source builtin

# List prompts by category
sah prompt list --category "development"

# List prompts with specific tag
sah prompt list --tag "review"
```

### `sah prompt show`

Show detailed information about a prompt.

```bash
sah prompt show <PROMPT_NAME> [OPTIONS]
```

**Options:**
- `--raw` - Show raw markdown content
- `--format <FORMAT>` - Output format (yaml, json, markdown)

**Examples:**
```bash
# Show prompt details
sah prompt show code-review

# Show raw markdown
sah prompt show code-review --raw

# Output as JSON
sah prompt show code-review --format json
```

### `sah prompt test`

Test a prompt by rendering it with variables.

```bash
sah prompt test <PROMPT_NAME> [OPTIONS]
```

**Options:**
- `--var <KEY=VALUE>` - Set template variable (can be repeated)
- `--vars-file <FILE>` - Load variables from JSON/YAML file
- `--output <FILE>` - Write output to file instead of stdout
- `--format <FORMAT>` - Output format (text, markdown, html)

**Examples:**
```bash
# Test with inline variables
sah prompt test code-review --var language=rust --var file=main.rs

# Load variables from file
sah prompt test code-review --vars-file variables.json

# Save output to file
sah prompt test code-review --var language=rust --output review.md
```

### `sah prompt render`

Render a prompt and output the result (alias for `test`).

```bash
sah prompt render <PROMPT_NAME> [OPTIONS]
```

Same options as `sah prompt test`.

### `sah prompt validate`

Validate prompt syntax and structure.

```bash
sah prompt validate [PROMPT_NAME] [OPTIONS]
```

**Options:**
- `--strict` - Enable strict validation mode
- `--format <FORMAT>` - Output format (table, json)
- `--fix` - Attempt to fix validation issues

**Examples:**
```bash
# Validate specific prompt
sah prompt validate my-prompt

# Validate all prompts
sah prompt validate

# Strict validation with fixes
sah prompt validate --strict --fix
```

## Flow (Workflow) Commands

### `sah flow list`

List available workflows.

```bash
sah flow list [OPTIONS]
```

**Options:**
- `--source <SOURCE>` - Filter by source (builtin, user, local)
- `--format <FORMAT>` - Output format (table, json, list)
- `--search <TERM>` - Search workflow names and descriptions

### `sah flow show`

Show workflow details and structure.

```bash
sah flow show <WORKFLOW_NAME> [OPTIONS]
```

**Options:**
- `--diagram` - Generate Mermaid diagram
- `--format <FORMAT>` - Output format (yaml, json, markdown)

### `sah flow run`

Execute a workflow.

```bash
sah flow run <WORKFLOW_NAME> [OPTIONS]
```

**Options:**
- `--var <KEY=VALUE>` - Set workflow variable
- `--vars-file <FILE>` - Load variables from file
- `--start-state <STATE>` - Start from specific state
- `--dry-run` - Show execution plan without running
- `--parallel` - Enable parallel execution where possible
- `--timeout <MS>` - Workflow timeout in milliseconds

**Examples:**
```bash
# Run workflow
sah flow run my-workflow

# Run with variables
sah flow run my-workflow --var project=myapp --var env=prod

# Dry run to see execution plan
sah flow run my-workflow --dry-run

# Start from specific state
sah flow run my-workflow --start-state deploy
```

### `sah flow validate`

Validate workflow syntax and logic.

```bash
sah flow validate [WORKFLOW_NAME] [OPTIONS]
```

**Options:**
- `--strict` - Enable strict validation
- `--check-cycles` - Check for circular dependencies
- `--format <FORMAT>` - Output format (table, json)

## Issue Management Commands

### `sah issue list`

List issues with their status.

```bash
sah issue list [OPTIONS]
```

**Options:**
- `--status <STATUS>` - Filter by status (active, complete, all)
- `--format <FORMAT>` - Output format (table, json, markdown)
- `--sort <FIELD>` - Sort by field (name, created, status)

### `sah issue create`

Create a new issue.

```bash
sah issue create [OPTIONS]
```

**Options:**
- `--name <NAME>` - Issue name (will be used in branch name)
- `--content <TEXT>` - Issue content as text
- `--file <FILE>` - Load content from file
- `--template <TEMPLATE>` - Use issue template
- `--editor` - Open editor for content

**Examples:**
```bash
# Create named issue
sah issue create --name "feature-auth" --content "# Authentication Feature\n\nImplement JWT auth"

# Create from file
sah issue create --name "bugfix" --file issue-template.md

# Create with editor
sah issue create --name "refactor" --editor
```

### `sah issue show`

Show issue details.

```bash
sah issue show <ISSUE_NAME> [OPTIONS]
```

**Options:**
- `--raw` - Show raw markdown content
- `--format <FORMAT>` - Output format (markdown, json)

Special issue names:
- `current` - Show issue for current git branch
- `next` - Show next pending issue

### `sah issue work`

Start working on an issue (creates/switches to branch).

```bash
sah issue work <ISSUE_NAME> [OPTIONS]
```

**Options:**
- `--create-branch` - Force branch creation even if exists
- `--base <BRANCH>` - Base branch for new branch (default: current)

### `sah issue complete`

Mark an issue as complete.

```bash
sah issue complete <ISSUE_NAME> [OPTIONS]
```

**Options:**
- `--merge` - Merge branch back to source branch
- `--delete-branch` - Delete the issue branch after completion
- `--message <MSG>` - Completion commit message

### `sah issue update`

Update issue content.

```bash
sah issue update <ISSUE_NAME> [OPTIONS]
```

**Options:**
- `--content <TEXT>` - New content as text
- `--file <FILE>` - Load content from file
- `--append` - Append to existing content
- `--editor` - Open editor for content

### `sah issue merge`

Merge issue branch back to source branch using git merge-base.

```bash
sah issue merge <ISSUE_NAME> [OPTIONS]
```

**Options:**
- `--delete-branch` - Delete branch after merge
- `--squash` - Squash commits when merging
- `--message <MSG>` - Merge commit message

## Memoranda (Notes) Commands

### `sah memo list`

List all memos.

```bash
sah memo list [OPTIONS]
```

**Options:**
- `--format <FORMAT>` - Output format (table, json, list)
- `--sort <FIELD>` - Sort by field (title, created, updated)
- `--limit <N>` - Limit number of results

### `sah memo create`

Create a new memo.

```bash
sah memo create [OPTIONS]
```

**Options:**
- `--title <TITLE>` - Memo title
- `--content <TEXT>` - Memo content as text
- `--file <FILE>` - Load content from file
- `--editor` - Open editor for content

**Examples:**
```bash
# Create memo with inline content
sah memo create --title "Meeting Notes" --content "# Team Meeting\n\nDiscussed project timeline"

# Create from file
sah memo create --title "Architecture" --file architecture-notes.md

# Create with editor
sah memo create --title "Ideas" --editor
```

### `sah memo show`

Show memo content.

```bash
sah memo show <MEMO_ID> [OPTIONS]
```

**Options:**
- `--raw` - Show raw markdown content
- `--format <FORMAT>` - Output format (markdown, json)

### `sah memo update`

Update memo content.

```bash
sah memo update <MEMO_ID> [OPTIONS]
```

**Options:**
- `--content <TEXT>` - New content as text
- `--file <FILE>` - Load content from file
- `--editor` - Open editor for content

### `sah memo delete`

Delete a memo.

```bash
sah memo delete <MEMO_ID> [OPTIONS]
```

**Options:**
- `--confirm` - Skip confirmation prompt

### `sah memo search`

Search memos by content.

```bash
sah memo search <QUERY> [OPTIONS]
```

**Options:**
- `--limit <N>` - Limit number of results
- `--format <FORMAT>` - Output format (table, json, list)

## Search Commands

### `sah search index`

Index files for semantic search.

```bash
sah search index <PATTERN> [OPTIONS]
```

**Options:**
- `--force` - Force re-indexing of all files
- `--language <LANG>` - Limit to specific language
- `--exclude <PATTERN>` - Exclude files matching pattern
- `--max-size <BYTES>` - Maximum file size to index

**Examples:**
```bash
# Index Rust files
sah search index "**/*.rs"

# Index multiple languages
sah search index "**/*.{rs,py,js,ts}"

# Force re-index
sah search index "**/*.rs" --force

# Index with exclusions
sah search index "**/*.py" --exclude "**/test_*.py"
```

### `sah search query`

Perform semantic search query.

```bash
sah search query <QUERY> [OPTIONS]
```

**Options:**
- `--limit <N>` - Number of results to return (default: 10)
- `--format <FORMAT>` - Output format (table, json, detailed)
- `--threshold <SCORE>` - Minimum similarity score (0.0-1.0)

**Examples:**
```bash
# Basic search
sah search query "error handling"

# Limit results
sah search query "async functions" --limit 5

# Detailed output
sah search query "database connection" --format detailed

# High threshold for exact matches
sah search query "specific function name" --threshold 0.8
```

## Validation Commands

### `sah validate`

Validate configurations, prompts, and workflows.

```bash
sah validate [OPTIONS]
```

**Options:**
- `--config` - Validate configuration files only
- `--prompts` - Validate prompts only
- `--workflows` - Validate workflows only
- `--strict` - Enable strict validation
- `--format <FORMAT>` - Output format (table, json)
- `--fix` - Attempt to fix validation issues

**Examples:**
```bash
# Validate everything
sah validate

# Validate only configuration
sah validate --config

# Strict validation with fixes
sah validate --strict --fix
```

## Configuration Commands

### `sah config show`

Show current configuration.

```bash
sah config show [OPTIONS]
```

**Options:**
- `--format <FORMAT>` - Output format (toml, json, yaml)
- `--section <SECTION>` - Show specific section only

### `sah config set`

Set configuration value.

```bash
sah config set <KEY> <VALUE> [OPTIONS]
```

**Options:**
- `--user` - Set in user configuration
- `--local` - Set in local project configuration
- `--type <TYPE>` - Value type (string, number, boolean)

**Examples:**
```bash
# Set log level
sah config set logging.level debug

# Set user-level setting
sah config set general.auto_reload true --user

# Set project-level setting
sah config set workflow.max_parallel_actions 8 --local
```

### `sah config get`

Get configuration value.

```bash
sah config get <KEY> [OPTIONS]
```

**Options:**
- `--default` - Show default value if not set
- `--source` - Show which config file the value comes from

## Utility Commands

### `sah completions`

Generate shell completions.

```bash
sah completions <SHELL>
```

**Supported shells:**
- `bash`
- `zsh`  
- `fish`
- `powershell`

**Examples:**
```bash
# Generate bash completions
sah completions bash > ~/.bash_completion.d/sah

# Generate zsh completions
sah completions zsh > ~/.zfunc/_sah
```

### `sah version`

Show version information.

```bash
sah version [OPTIONS]
```

**Options:**
- `--short` - Show version number only
- `--build` - Include build information

### `sah help`

Show help information.

```bash
sah help [COMMAND]
```

Show help for specific command or general help.

## Exit Codes

SwissArmyHammer uses standard exit codes:

- `0` - Success
- `1` - General error
- `2` - Misuse of shell command
- `3` - Configuration error
- `4` - Validation error
- `5` - Network error
- `6` - Permission error
- `7` - Not found error
- `8` - Timeout error

## Examples

### Common Workflows

```bash
# Set up new project
sah doctor
sah config set workflow.max_parallel_actions 4 --local
sah search index "**/*.rs"

# Daily development workflow
sah issue create --name "feature-api" --editor
sah issue work feature-api
# ... do development work ...
sah issue complete feature-api --merge --delete-branch

# Code review workflow
sah prompt test code-review --var file=src/main.rs --var language=rust
sah memo create --title "Review Notes" --editor

# Search and discovery
sah search query "authentication middleware"
sah prompt list --category "testing"
sah memo search "architecture"
```

### Integration with Other Tools

```bash
# Use with git hooks
#!/bin/bash
# .git/hooks/pre-commit
sah validate --strict --fix

# Use in CI/CD
sah validate --config --format json
sah search index "**/*.rs" --force
sah prompt validate --strict

# Use with editors (VS Code task example)
{
  "label": "Test Prompt",
  "type": "shell", 
  "command": "sah",
  "args": ["prompt", "test", "${input:promptName}", "--var", "file=${file}"]
}
```

This comprehensive CLI reference covers all SwissArmyHammer commands and options for efficient prompt and workflow management.