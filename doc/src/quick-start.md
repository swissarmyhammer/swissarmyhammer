# Quick Start

Get up and running with SwissArmyHammer in 5 minutes. This guide will walk you through creating your first prompt and using it with Claude Code.

## Step 1: Verify Installation

First, make sure SwissArmyHammer is properly installed:

```bash
sah --version
sah doctor
```

The `doctor` command will check your installation and suggest any needed fixes.

## Step 2: Create Your First Prompt

Create a personal prompts directory and your first prompt:

```bash
# Create the directory structure
mkdir -p ~/.swissarmyhammer/prompts

# Create a simple helper prompt
cat > ~/.swissarmyhammer/prompts/task-helper.md << 'EOF'
---
title: Task Helper
description: Helps with various programming tasks
arguments:
  - name: task
    description: What you need help with
    required: true
  - name: context
    description: Additional context (optional)
    required: false
    default: "general programming"
---

I need help with: **{{task}}**

Context: {{context}}

Please provide:
1. Clear, step-by-step guidance
2. Code examples if applicable
3. Best practices to follow
4. Common pitfalls to avoid

Make your response practical and actionable.
EOF
```

## Step 3: Test Your Prompt

Test the prompt using the CLI:

```bash
# Test with required argument
sah prompt test task-helper --var task="debugging a Rust application"

# Test with both arguments
sah prompt test task-helper \
  --var task="implementing error handling" \
  --var context="web API development"
```

You should see the rendered prompt with your variables substituted.

## Step 4: Configure Claude Code Integration

Add SwissArmyHammer as an MCP server for Claude Code:

```bash
# Add the MCP server
claude mcp add sah sah serve

# Verify it's working
claude mcp list
claude mcp status sah
```

## Step 5: Use in Claude Code

Now you can use your prompt directly in Claude Code. Start a conversation and use:

```
/task-helper task="setting up CI/CD pipeline" context="GitHub Actions for Rust project"
```

Claude will use your prompt template and provide structured assistance.

## Step 6: Explore Built-in Prompts

SwissArmyHammer comes with 20+ built-in prompts. List them:

```bash
sah prompt list --source builtin
```

Try some useful ones:

```bash
# Code review helper
sah prompt test code --var task="review this function for performance issues"

# Documentation generator  
sah prompt test documentation --var task="document this API endpoint"

# Debug helper
sah prompt test debug --var error="segmentation fault in C program"
```

## Step 7: Create a Simple Workflow

Workflows allow you to chain multiple prompts and actions. Create your first workflow:

```bash
mkdir -p ~/.swissarmyhammer/workflows

cat > ~/.swissarmyhammer/workflows/code-review.md << 'EOF'
---
name: code-review
description: Complete code review workflow
initial_state: analyze
---

## States

### analyze
Analyze the code for issues and improvements.

**Actions:**
- prompt: Use the 'code' prompt to analyze the code
- shell: Run any necessary tests

**Next**: report

### report
Generate a comprehensive review report.

**Actions:**
- prompt: Use the 'documentation' prompt to suggest documentation improvements

**Next**: complete

### complete
Review workflow completed.
EOF
```

Run the workflow:

```bash
sah flow run code-review
```

## Step 8: Set Up Issue Management

SwissArmyHammer includes git-integrated issue tracking:

```bash
# Create an issue (in a git repository)
sah issue create --name "feature-auth" --content "# User Authentication

Implement JWT-based user authentication system with:
- Login/logout endpoints
- Token validation middleware
- User session management"

# List issues
sah issue list

# Work on an issue (creates/switches to branch)
sah issue work feature-auth

# Complete the issue
sah issue complete feature-auth
```

## Step 9: Try Memoranda (Notes)

SwissArmyHammer includes a note-taking system:

```bash
# Create a memo
sah memo create --title "Project Notes" --content "# Meeting Notes

## Action Items
- [ ] Set up database schema
- [ ] Implement user API
- [ ] Write integration tests"

# List memos
sah memo list

# Search memos
sah memo search "database"
```

## Step 10: Set Up Semantic Search

Index your codebase for AI-powered semantic search:

```bash
# Index Rust files
sah search index "**/*.rs"

# Search for specific concepts
sah search query "error handling patterns"

# Search for specific functionality
sah search query "database connection management"
```

## Common Patterns

### Project-Specific Prompts

Create prompts specific to your project:

```bash
# In your project directory
mkdir -p .swissarmyhammer/prompts

# Create a project-specific prompt
cat > .swissarmyhammer/prompts/api-docs.md << 'EOF'
---
title: API Documentation
description: Generate API documentation for this project
arguments:
  - name: endpoint
    description: API endpoint to document
    required: true
---

Generate comprehensive API documentation for the {{endpoint}} endpoint.

Include:
- Request/response schemas
- Example requests
- Error responses
- Authentication requirements

Use our project's documentation style and format.
EOF
```

### Template Variables

Use liquid template features for dynamic prompts:

```markdown
---
title: Conditional Helper
arguments:
  - name: difficulty
    description: Task difficulty level
    required: false
    default: "medium"
---

{% if difficulty == "beginner" %}
Let's start with the basics:
{% elsif difficulty == "advanced" %}
Here's an advanced approach:
{% else %}
Here's a practical solution:
{% endif %}

[Rest of your prompt...]
```

### Environment Integration

Use environment variables in prompts:

```markdown
---
title: Project Context
---

Working on project: {{PROJECT_NAME | default: "unknown project"}}
Environment: {{NODE_ENV | default: "development"}}

[Your prompt content...]
```

## Next Steps

Now that you have SwissArmyHammer working:

1. **Explore Features**: Read about [Prompts](prompts.md), [Workflows](workflows.md), and [Templates](templates.md)
2. **Advanced Usage**: Check out the [CLI Reference](cli-reference.md) for all commands
3. **Integration**: Learn about [MCP Integration](mcp.md) for deeper Claude Code integration
4. **Examples**: Browse [Examples](examples/basic.md) for inspiration
5. **Customize**: Set up [Configuration](configuration.md) to match your workflow

## Getting Help

- Run `sah --help` for command help
- Use `sah doctor` to diagnose issues
- Check [Troubleshooting](troubleshooting.md) for common problems
- Visit the [GitHub repository](https://github.com/swissarmyhammer/swissarmyhammer) for issues and discussions