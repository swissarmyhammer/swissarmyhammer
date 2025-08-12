# Basic Examples

Simple, practical examples to get you started with SwissArmyHammer prompts and workflows.

## Simple Prompts

### Task Helper

A basic prompt for general assistance:

**File**: `~/.swissarmyhammer/prompts/helper.md`
```markdown
---
title: Task Helper
description: General purpose task assistance
arguments:
  - name: task
    description: What you need help with
    required: true
  - name: detail_level
    description: Level of detail needed
    choices: ["brief", "detailed", "comprehensive"]
    default: "detailed"
---

I need help with: **{{task}}**

{% if detail_level == "brief" %}
Please provide a concise answer with key points only.
{% elsif detail_level == "comprehensive" %}
Please provide a thorough explanation with examples, alternatives, and best practices.
{% else %}
Please provide a detailed explanation with practical steps.
{% endif %}

Focus on actionable advice and practical solutions.
```

**Usage**:
```bash
sah prompt test helper --var task="setting up a Rust project"
sah prompt test helper --var task="debugging memory leaks" --var detail_level="comprehensive"
```

### Code Reviewer

A prompt for reviewing code:

**File**: `~/.swissarmyhammer/prompts/code-reviewer.md`
```markdown
---
title: Code Reviewer
description: Review code for quality and best practices
arguments:
  - name: language
    description: Programming language
    required: true
    choices: ["rust", "python", "javascript", "typescript", "go"]
  - name: code
    description: Code to review
    required: true
  - name: focus
    description: Review focus areas
    type: array
    default: ["bugs", "performance", "style"]
---

## Code Review: {{language | capitalize}}

Please review this {{language}} code:

```{{language}}
{{code}}
```

### Focus Areas
{% for area in focus %}
- {{area | capitalize}}
{% endfor %}

### Please provide:
1. **Overall Assessment** - Quality rating and summary
2. **Specific Issues** - Line-by-line feedback
3. **Improvements** - Concrete suggestions
4. **Best Practices** - {{language}} conventions

Make feedback constructive and specific.
```

**Usage**:
```bash
sah prompt test code-reviewer \
  --var language="rust" \
  --var code="fn main() { println!(\"Hello\"); }" \
  --var focus='["bugs", "style"]'
```

### Documentation Generator

Generate documentation for code:

**File**: `~/.swissarmyhammer/prompts/doc-gen.md`
```markdown
---
title: Documentation Generator
description: Generate documentation for code or APIs
arguments:
  - name: type
    description: Type of documentation
    choices: ["api", "function", "class", "module"]
    required: true
  - name: name
    description: Name of the item to document
    required: true
  - name: code
    description: Code to document
    required: false
  - name: format
    description: Output format
    choices: ["markdown", "html", "rst"]
    default: "markdown"
---

# Documentation for {{type | capitalize}}: {{name}}

{% if code %}
## Code
```
{{code}}
```
{% endif %}

Please generate comprehensive {{format}} documentation including:

{% if type == "api" %}
- Endpoint description
- Request/response schemas  
- Example requests
- Error codes
- Authentication requirements
{% elsif type == "function" %}
- Purpose and behavior
- Parameters and types
- Return value
- Examples
- Edge cases
{% elsif type == "class" %}
- Class purpose
- Constructor parameters
- Public methods
- Properties
- Usage examples
{% else %}
- Module overview
- Key functions/classes
- Usage examples
- Dependencies
- Installation notes
{% endif %}

Use clear, professional language suitable for developers.
```

## Simple Workflows

### Code Review Workflow

A basic workflow for reviewing code changes:

**File**: `~/.swissarmyhammer/workflows/code-review.md`
```markdown
---
name: code-review
description: Simple code review workflow
initial_state: analyze
variables:
  - name: language
    description: Programming language
    default: "rust"
---

## Code Review Workflow

### analyze
**Description**: Analyze the code for issues

**Actions:**
- prompt: code-reviewer language={{language}} code="$(cat src/main.rs)" focus='["bugs", "performance"]'

**Transitions:**
- Always → report

### report
**Description**: Generate review report

**Actions:**
- prompt: doc-gen type="function" name="review_summary" format="markdown"

**Transitions:**
- Always → complete

### complete
**Description**: Review completed
```

**Usage**:
```bash
sah flow run code-review --var language="rust"
```

### Test and Build Workflow

Simple CI-like workflow:

**File**: `~/.swissarmyhammer/workflows/test-build.md`
```markdown
---
name: test-build
description: Run tests and build if they pass
initial_state: test
---

## Test and Build Workflow

### test
**Description**: Run test suite

**Actions:**
- shell: `cargo test`

**Transitions:**
- On success → build
- On failure → test-failed

### build
**Description**: Build the project

**Actions:**
- shell: `cargo build --release`

**Transitions:**
- On success → complete
- On failure → build-failed

### test-failed
**Description**: Handle test failures

**Actions:**
- prompt: helper task="debugging failed tests" detail_level="detailed"

**Transitions:**
- Always → failed

### build-failed
**Description**: Handle build failures

**Actions:**
- prompt: helper task="fixing build errors" detail_level="detailed"

**Transitions:**
- Always → failed

### failed
**Description**: Workflow failed

### complete
**Description**: All steps completed successfully
```

## Issue Management Examples

### Creating Issues

```bash
# Simple bug report
sah issue create --name "fix-memory-leak" --content "
# Memory Leak in Parser

## Description
Memory usage grows continuously when parsing large files.

## Steps to Reproduce
1. Parse file > 100MB
2. Monitor memory usage
3. Memory never gets freed

## Expected
Memory should be freed after parsing.
"

# Feature request
sah issue create --name "add-json-output" --content "
# Add JSON Output Format

## Description
Add --format json flag to all commands for machine-readable output.

## Acceptance Criteria
- [ ] All list commands support JSON
- [ ] All show commands support JSON
- [ ] JSON schema is documented
- [ ] Tests cover JSON output
"
```

### Working with Issues

```bash
# Start working on an issue
sah issue work fix-memory-leak

# Update issue with progress
sah issue update fix-memory-leak --append --content "
## Progress Update
- Identified leak in tokenizer
- Need to add Drop implementation
"

# Complete the issue
sah issue complete fix-memory-leak --merge --delete-branch
```

## Memoranda Examples

### Meeting Notes

```bash
sah memo create --title "Team Standup 2024-01-15" --content "
# Team Standup - January 15, 2024

## Attendees
- Alice (Lead)
- Bob (Backend)
- Carol (Frontend)

## Progress
- Alice: Working on authentication system
- Bob: Database migration almost complete
- Carol: New UI components ready for review

## Blockers
- Need staging environment for testing
- Waiting for design approval on checkout flow

## Action Items
- [ ] Alice: Set up staging environment
- [ ] Bob: Review Carol's UI components
- [ ] Carol: Follow up with design team
"

# Search meeting notes
sah memo search "staging environment"
```

### Technical Notes

```bash
sah memo create --title "Architecture Decision: Database Choice" --content "
# Database Choice for User Service

## Context
Need to choose database for new user management service.

## Options Considered

### PostgreSQL
**Pros**: ACID compliance, mature ecosystem, good performance
**Cons**: More complex setup, overkill for simple use cases

### SQLite
**Pros**: Simple setup, embedded, good for development
**Cons**: Not suitable for high concurrency

### MongoDB
**Pros**: Flexible schema, good for rapid prototyping
**Cons**: Eventual consistency, learning curve

## Decision
PostgreSQL - provides reliability and performance we need.

## Consequences
- Need to set up database infrastructure
- Team needs PostgreSQL training
- Migration strategy required for existing data
"
```

## Search Examples

### Indexing Code

```bash
# Index Rust project
sah search index "**/*.rs" --exclude "**/target/**"

# Index multiple languages
sah search index "**/*.{rs,py,js,ts}" --exclude "{**/target/**,**/node_modules/**,**/__pycache__/**}"

# Force re-index after major changes
sah search index "src/**/*.rs" --force
```

### Searching Code

```bash
# Find error handling patterns
sah search query "error handling patterns"

# Find async/await usage
sah search query "async await implementation"

# Find database connection code
sah search query "database connection setup"

# Find specific API patterns
sah search query "REST API endpoint handlers" --limit 5
```

## Configuration Examples

### Basic Configuration

**File**: `~/.swissarmyhammer/sah.toml`
```toml
[general]
auto_reload = true
default_timeout_ms = 30000

[logging]
level = "info"
format = "compact"

[template]
cache_size = 500

[search]
embedding_model = "nomic-embed-code"
max_file_size = 1048576

[workflow]
max_parallel_actions = 2
```

### Project-Specific Configuration

**File**: `./.swissarmyhammer/sah.toml`
```toml
[workflow]
# This project has resource constraints
max_parallel_actions = 1

[search]
# Index only specific directories
include_patterns = ["src/**/*.rs", "tests/**/*.rs"]
exclude_patterns = ["target/**", "**/*.bak"]

[issues]
# Use feature branch pattern
branch_pattern = "feature/{{name}}"
```

## Environment Integration

### Using Environment Variables

```markdown
---
title: Environment-Aware Deploy
arguments:
  - name: service
    description: Service to deploy
    required: true
---

Deploying {{service}} to {{NODE_ENV | default: "development"}}.

Target URL: {{DEPLOY_URL | default: "http://localhost:3000"}}

Configuration:
- Database: {{DATABASE_URL | default: "sqlite://local.db"}}
- Redis: {{REDIS_URL | default: "redis://localhost:6379"}}

{% if NODE_ENV == "production" %}
⚠️  **PRODUCTION DEPLOYMENT** - Extra care required!
{% endif %}
```

### Shell Integration

```bash
#!/bin/bash
# deploy.sh - Integration script

set -e

echo "🔨 Running pre-deployment checks..."
sah flow run pre-deploy-checks

echo "🚀 Deploying application..."
sah prompt test deploy-prompt \
  --var service="$SERVICE_NAME" \
  --var environment="$NODE_ENV" \
  --output deploy-plan.md

echo "📝 Creating deployment issue..."
sah issue create \
  --name "deploy-$SERVICE_NAME-$(date +%Y%m%d)" \
  --file deploy-plan.md

echo "✅ Deployment process initiated!"
```

These basic examples provide a foundation for building more complex prompts, workflows, and integrations with SwissArmyHammer.