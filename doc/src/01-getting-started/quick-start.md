# Quick Start

Get up and running with SwissArmyHammer in 5 minutes. This guide will show you how to use full auto coding to turn specifications into working code.

## Full Auto Coding

SwissArmyHammer's most powerful feature is its ability to transform natural language specifications into complete, tested applications. Here's how:

### Step 1: Verify Installation

First, make sure SwissArmyHammer is properly installed:

```bash
sah --version
sah doctor
```

The `doctor` command will check your installation and suggest any needed fixes.

### Step 2: Create a Specification

Create a markdown file describing what you want to build. You can use plain language, use cases, or code snippets:

```bash
mkdir -p specification
cat > specification/index.md << 'EOF'
# Calculator Application

Build a command-line calculator that:
- Supports basic operations: add, subtract, multiply, divide
- Has a REPL interface for interactive use
- Handles errors gracefully
- Includes comprehensive tests
- Has proper documentation
EOF
```

### Step 3: Generate the Plan

Run the planning workflow to analyze your specification and create implementation issues:

```bash
sah plan specification/index.md
```

This generates a set of issues in the `./issues` directory. Each issue represents a specific task.

### Step 4: Commit the Plan

```bash
git add issues
git commit -m "plan: add calculator implementation issues"
```

### Step 5: Execute the Implementation

Now let SwissArmyHammer implement your specification:

```bash
sah flow run implement
```

This will:
- Work through each issue automatically
- Write the code
- Run tests and fix any failures
- Commit changes as it progresses
- Continue until all issues are complete

The implementation typically takes a few hours, but it's faster than manual coding and produces high-quality, tested code.

### Step 6: Review the Results

Once complete, you'll have a fully working application with:
- Complete source code
- Passing tests
- Documentation
- Git history showing the implementation process

## Example: Calcutron

For a complete example, check out [Calcutron](https://github.com/swissarmyhammer/calcutron), a sample calculator built entirely with SwissArmyHammer:

```bash
# Clone the example
git clone git@github.com:swissarmyhammer/calcutron.git
cd calcutron

# Run the preflight check
sah doctor

# Generate the plan
sah plan specification/index.md
git add issues
git commit -am 'plan'

# Let it build
sah flow run implement
```

## Manual Workflow Examples

For more targeted tasks, you can use SwissArmyHammer's manual workflows and prompts.

### Create Your First Prompt

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
claude mcp add --scope user sah sah serve

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

1. **Try Full Auto Coding**: Create your own specification and let SwissArmyHammer build it
2. **Explore Built-in Workflows**: Run `sah flow list` to see available workflows
3. **Create Custom Prompts**: Build prompts tailored to your specific needs
4. **Integrate with Claude Code**: Use SwissArmyHammer as an MCP server for enhanced AI assistance

## Getting Help

- Run `sah --help` for command help
- Use `sah doctor` to diagnose issues
- Visit the [GitHub repository](https://github.com/swissarmyhammer/swissarmyhammer) for issues and discussions