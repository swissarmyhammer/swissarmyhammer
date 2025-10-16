# Examples and Tutorials

This section provides practical, step-by-step examples for common SwissArmyHammer Tools workflows.

## Quick Start Examples

### Example 1: Your First Semantic Search

Learn how to index and search your codebase.

**Step 1: Index your codebase**

For a Rust project:
```
You: "Index all Rust source files for semantic search"

Claude uses: search_index
{
  "patterns": ["src/**/*.rs", "tests/**/*.rs"]
}
```

For a TypeScript project:
```
You: "Index all TypeScript files"

Claude uses: search_index
{
  "patterns": ["src/**/*.ts", "src/**/*.tsx"]
}
```

**Step 2: Search for functionality**
```
You: "Find authentication logic"

Claude uses: search_query
{
  "query": "user authentication login password",
  "limit": 10
}

Results show relevant files ranked by similarity
```

**Step 3: Examine results**
```
You: "Show me the authentication module"

Claude uses: files_read
{
  "path": "/workspace/src/auth/mod.rs"
}
```

**Expected outcome**: You can now quickly navigate large codebases by semantic meaning rather than exact keyword matches.

### Example 2: Creating and Tracking an Issue

Track a feature from creation to completion.

**Step 1: Create the issue**
```
You: "Create an issue for adding email notifications"

Claude uses: issue_create
{
  "name": "feature_email_notifications",
  "content": "# Email Notifications\n\n## Goal\nImplement email notifications for important events\n\n## Requirements\n- Send welcome emails\n- Send password reset emails\n- Send notification emails\n"
}
```

**Step 2: Create a branch**
```bash
git checkout -b issue/feature_email_notifications
```

**Step 3: Work on implementation**
```
You: "Create src/email/mod.rs with basic email structure"

Claude uses: files_write
{
  "file_path": "/workspace/src/email/mod.rs",
  "content": "//! Email notification system\n\npub struct EmailService {\n    // implementation\n}"
}
```

**Step 4: Track changes**
```
You: "What files have changed on this branch?"

Claude uses: git_changes
{
  "branch": "issue/feature_email_notifications"
}

Returns: List of all modified files
```

**Step 5: Complete the issue**
```
You: "Mark the email notifications issue as complete"

Claude uses: issue_mark_complete
{
  "name": "feature_email_notifications"
}
```

**Expected outcome**: Structured workflow for implementing features with full history and tracking.

### Example 3: Bulk Code Refactoring

Safely refactor code across multiple files.

**Step 1: Find all occurrences**
```
You: "Search for uses of the old Configuration struct"

Claude uses: files_grep
{
  "pattern": "Configuration",
  "type": "rust",
  "output_mode": "content"
}
```

**Step 2: Understand the structure**
```
You: "Generate an outline of src/config/"

Claude uses: outline_generate
{
  "patterns": ["src/config/**/*.rs"]
}
```

**Step 3: Make the changes**
```
You: "Replace 'Configuration' with 'AppConfig' in all Rust files"

Claude uses multiple files_edit calls:
{
  "file_path": "/workspace/src/config/mod.rs",
  "old_string": "pub struct Configuration {",
  "new_string": "pub struct AppConfig {",
  "replace_all": false
}
```

**Step 4: Verify the changes**
```
You: "Run the tests to verify everything still works"

Claude uses: shell_execute
{
  "command": "cargo nextest run"
}
```

**Step 5: Check for any remaining references**
```
You: "Search for any remaining 'Configuration' references"

Claude uses: files_grep
{
  "pattern": "Configuration",
  "type": "rust"
}
```

**Expected outcome**: Safe, verified refactoring across the entire codebase with test validation.

## Feature-Specific Examples

### Working with Memos

**Create a decision memo**
```
You: "Create a memo documenting our decision to use PostgreSQL"

Claude uses: memo_create
{
  "title": "Database Selection: PostgreSQL",
  "content": "# Database Selection\n\n## Decision\nUse PostgreSQL for production database\n\n## Rationale\n- ACID compliance\n- JSON support\n- Excellent performance\n- Wide ecosystem support\n\n## Date\n2025-10-16"
}
```

**Retrieve memos for context**
```
You: "Show me all our architectural memos"

Claude uses: memo_list
{}

Then uses: memo_get
{
  "title": "Database Selection: PostgreSQL"
}
```

**Get complete context**
```
You: "Load all memos for context"

Claude uses: memo_get_all_context
{}

Returns all memos sorted by date
```

### Using Todo Tracking

**Break down a complex task**
```
You: "I need to implement JWT authentication. Break this down into tasks."

Claude creates todos:
- Implement JWT token generation
- Implement JWT token validation
- Add middleware for protected routes
- Write tests for JWT functionality
- Update documentation
```

**Work through the list**
```
You: "What's the next task?"

Claude uses: todo_show
{
  "item": "next"
}

Returns: "Implement JWT token generation"
```

**Complete tasks as you go**
```
You: "I've finished JWT token generation. Mark it complete."

Claude uses: todo_mark_complete
{
  "id": "01K1KQM85501ECE8XJGNZKNJQW"
}
```

### Code Quality Checks

**Check for common issues**
```
You: "Check all Rust files for unwrap usage"

Claude uses: rules_check
{
  "rule_names": ["no-unwrap"],
  "file_paths": ["src/**/*.rs"]
}
```

**Full quality audit**
```
You: "Run a complete code quality check"

Claude uses: rules_check
{
  "file_paths": ["src/**/*.rs"],
  "severity": "warning"
}

Returns violations with file paths and line numbers
```

### Web Integration

**Fetch external documentation**
```
You: "Fetch the latest Rust async book chapter on futures"

Claude uses: web_fetch
{
  "url": "https://rust-lang.github.io/async-book/02_execution/02_future.html",
  "timeout": 30
}

Returns: Markdown conversion of the page
```

**Search for examples**
```
You: "Search for Rust async examples with tokio"

Claude uses: web_search
{
  "query": "rust async tokio examples tutorial",
  "category": "it",
  "results_count": 10
}

Returns: Search results with optional content
```

## Complete Workflows

### Workflow 1: Feature Implementation

From idea to production-ready code.

**1. Create and plan**
```
You: "I want to add rate limiting to our API"

Claude:
- Creates issue: "feature_rate_limiting"
- Searches for existing rate limiting code
- Checks for related libraries
- Creates implementation todos
```

**2. Research**
```
You: "Search for Rust rate limiting examples"

Claude:
- Uses web_search for external examples
- Uses search_query for internal patterns
- Fetches relevant documentation
```

**3. Implement**
```
You: "Implement rate limiting middleware"

Claude:
- Creates new files for rate limiting
- Implements middleware logic
- Adds configuration support
- Updates routing to use middleware
```

**4. Test**
```
You: "Add comprehensive tests"

Claude:
- Creates unit tests
- Creates integration tests
- Runs test suite
- Checks code coverage
```

**5. Quality Check**
```
You: "Check code quality"

Claude:
- Runs rules_check for violations
- Generates outline for review
- Checks for common issues
- Verifies documentation
```

**6. Complete**
```
You: "Everything looks good. Mark this complete."

Claude:
- Marks all todos complete
- Marks issue complete
- Shows summary of changes
```

### Workflow 2: Bug Investigation and Fix

Systematic debugging workflow.

**1. Document the bug**
```
You: "Create an issue for the login timeout bug"

Claude uses: issue_create
{
  "name": "bug_login_timeout",
  "content": "# Bug: Login Timeout\n\n## Description\nUsers report login timing out after 30 seconds\n\n## Steps to Reproduce\n1. Navigate to login page\n2. Enter credentials\n3. Wait 30+ seconds\n4. Login fails\n"
}
```

**2. Find relevant code**
```
You: "Search for login authentication code"

Claude uses: search_query
{
  "query": "login authentication timeout session"
}
```

**3. Examine implementation**
```
You: "Show me the login handler"

Claude uses: files_read
{
  "path": "/workspace/src/auth/login.rs"
}

Claude uses: outline_generate
{
  "patterns": ["src/auth/**/*.rs"]
}
```

**4. Find the issue**
```
You: "Search for timeout configuration"

Claude uses: files_grep
{
  "pattern": "timeout.*=.*30",
  "output_mode": "content"
}
```

**5. Fix the issue**
```
You: "Change the timeout from 30 to 300 seconds"

Claude uses: files_edit
{
  "file_path": "/workspace/src/auth/config.rs",
  "old_string": "timeout: Duration::from_secs(30)",
  "new_string": "timeout: Duration::from_secs(300)"
}
```

**6. Verify the fix**
```
You: "Run the authentication tests"

Claude uses: shell_execute
{
  "command": "cargo nextest run auth"
}
```

**7. Update and close**
```
You: "Update the issue with the fix and mark it complete"

Claude uses: issue_update
{
  "name": "bug_login_timeout",
  "content": "...\n\n## Resolution\nIncreased timeout from 30 to 300 seconds in src/auth/config.rs\n\n## Verification\nAll auth tests pass",
  "append": true
}

Claude uses: issue_mark_complete
{
  "name": "bug_login_timeout"
}
```

### Workflow 3: Code Documentation

Generate comprehensive documentation.

**1. Generate code outline**
```
You: "Create an outline of the entire API module"

Claude uses: outline_generate
{
  "patterns": ["src/api/**/*.rs"],
  "output_format": "yaml"
}
```

**2. Document architecture**
```
You: "Create a memo documenting the API architecture"

Claude:
- Reads multiple API files
- Analyzes structure
- Creates comprehensive memo
```

**3. Create examples**
```
You: "Create example code for using the API"

Claude:
- Finds usage patterns in tests
- Creates example files
- Documents common use cases
```

**4. Verify examples**
```
You: "Verify all examples compile"

Claude uses: shell_execute
{
  "command": "cargo build --examples"
}
```

## Integration Examples

### With Claude Desktop

SwissArmyHammer Tools integrates seamlessly with Claude Desktop.

**Configuration** (`claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "swissarmyhammer": {
      "command": "sah",
      "args": ["serve"]
    }
  }
}
```

**Usage**:
Simply describe what you want in natural language:
```
You: "Help me understand this codebase"

Claude:
- Indexes the codebase
- Generates outlines
- Searches for key components
- Explains architecture
```

### With Git Hooks

Use SwissArmyHammer in git hooks for automation.

**Pre-commit hook** (`.git/hooks/pre-commit`):
```bash
#!/bin/bash

# Check code quality before commit
sah rules_check --severity error

if [ $? -ne 0 ]; then
    echo "Code quality check failed. Fix issues before committing."
    exit 1
fi

# Generate outline for changed files
sah outline_generate "$(git diff --staged --name-only '*.rs')"
```

### With CI/CD

Integrate with continuous integration.

**GitHub Actions** (`.github/workflows/quality.yml`):
```yaml
name: Code Quality

on: [push, pull_request]

jobs:
  quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Install SwissArmyHammer
        run: cargo install swissarmyhammer

      - name: Check code quality
        run: sah rules_check --severity error

      - name: Generate code outline
        run: sah outline_generate 'src/**/*.rs' --output-format json > outline.json

      - name: Upload outline
        uses: actions/upload-artifact@v2
        with:
          name: code-outline
          path: outline.json
```

## Advanced Examples

### Custom Workflow Execution

Define and execute custom workflows.

**Workflow definition** (`.swissarmyhammer/workflows/deploy.yaml`):
```yaml
name: deploy
description: Deploy application to production
parameters:
  - name: environment
    description: Target environment
    required: true

steps:
  - name: Run tests
    command: cargo nextest run

  - name: Build release
    command: cargo build --release

  - name: Deploy
    command: ./deploy.sh {{environment}}
```

**Execute workflow**:
```
You: "Run the deploy workflow for staging"

Claude uses: flow_mcp
{
  "flow_name": "deploy",
  "parameters": {
    "environment": "staging"
  }
}
```

### Multi-Repository Search

Search across multiple repositories.

**Index multiple projects**:
```
You: "Index all Rust files in workspace"

Claude:
cd /workspace/project1 && sah search_index 'src/**/*.rs'
cd /workspace/project2 && sah search_index 'src/**/*.rs'
cd /workspace/project3 && sah search_index 'src/**/*.rs'
```

**Search across projects**:
```
You: "Find authentication implementations across all projects"

Claude searches each indexed project and aggregates results
```

### Code Migration

Migrate from one pattern to another.

**Example: Migrate from unwrap to proper error handling**

**1. Find all unwrap calls**:
```
Claude uses: files_grep
{
  "pattern": "\\.unwrap\\(\\)",
  "type": "rust",
  "output_mode": "content"
}
```

**2. Analyze each occurrence**:
```
Claude:
- Reads surrounding context
- Determines appropriate error handling
- Suggests replacement
```

**3. Replace systematically**:
```
Claude:
- Replaces unwrap with ? operator
- Adds Result return types
- Updates function signatures
- Adds error context
```

**4. Verify changes**:
```
Claude:
- Runs full test suite
- Checks for compilation errors
- Verifies error handling works
```

## Tips for Effective Usage

### Start Simple
Begin with basic operations before complex workflows.

### Use Natural Language
Describe what you want, not how to do it:
```
✓ "Find authentication logic"
✗ "Run search_query with query='auth'"
```

### Combine Tools
Use multiple tools together for powerful workflows:
```
1. search_query - Find relevant code
2. files_read - Examine implementation
3. outline_generate - Understand structure
4. files_edit - Make changes
5. shell_execute - Run tests
```

### Maintain Context
Use memos and issues to preserve knowledge:
```
- Memos for decisions and patterns
- Issues for tracking work
- Todos for current tasks
```

### Verify Changes
Always test after making changes:
```
- Run tests
- Check for errors
- Verify functionality
- Review with outline
```

## Next Steps

- [Use Cases](./use-cases.md): Best practices and patterns
- [Features](./features.md): Complete feature documentation
- [Tool Catalog](./reference/tools.md): Detailed tool reference
- [Troubleshooting](./troubleshooting.md): Common issues and solutions
