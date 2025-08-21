# Prompts

Prompts are the core building blocks of SwissArmyHammer - reusable templates that structure interactions with AI assistants.

## Prompt Structure

Every prompt is a markdown file with YAML front matter:

```markdown
---
title: Code Review Assistant
description: Helps review code for quality, style, and best practices
version: "1.0"
tags: ["code", "review", "quality"]
arguments:
  - name: language
    description: Programming language being reviewed
    required: true
    type: string
  - name: file_path
    description: Path to the file being reviewed
    required: false
    type: string
  - name: focus_areas
    description: Specific areas to focus on
    required: false
    type: array
    default: ["style", "performance", "bugs"]
---

# Code Review: {{language}} Code

I need you to review this {{language}} code{% if file_path %} from `{{file_path}}`{% endif %}.

## Focus Areas
{% for area in focus_areas %}
- {{area | capitalize}}
{% endfor %}

Please provide:

1. **Overall Assessment** - Code quality rating and summary
2. **Specific Issues** - Line-by-line feedback on problems
3. **Improvements** - Concrete suggestions for enhancement  
4. **Best Practices** - Recommendations following {{language}} conventions

Make your feedback constructive and actionable.
```

## Front Matter Reference

### Required Fields

| Field | Description | Example |
|-------|-------------|---------|
| `title` | Human-readable prompt name | `"Code Review Assistant"` |
| `description` | What the prompt does | `"Helps review code quality"` |

### Optional Fields

| Field | Description | Example |
|-------|-------------|---------|
| `version` | Prompt version | `"1.2.0"` |
| `tags` | Categorization tags | `["code", "review"]` |
| `author` | Prompt creator | `"Jane Developer"` |
| `created` | Creation date | `"2024-01-15"` |
| `updated` | Last update | `"2024-01-20"` |
| `license` | Usage license | `"MIT"` |

### Arguments

Arguments define the variables that can be passed to the prompt:

```yaml
arguments:
  - name: variable_name        # Required: variable name
    description: "What it does" # Required: human description  
    required: true             # Optional: is it required? (default: false)
    type: string              # Optional: data type (string, number, boolean, array)
    default: "default_value"   # Optional: default if not provided
    choices: ["a", "b", "c"]   # Optional: allowed values
    pattern: "^[a-z]+$"       # Optional: regex validation
    min_length: 1             # Optional: minimum string length
    max_length: 100           # Optional: maximum string length
```

#### Argument Types

- **string**: Text values
- **number**: Numeric values  
- **boolean**: true/false values
- **array**: Lists of values

Example with all types:

```yaml
arguments:
  - name: title
    description: "Document title"
    required: true
    type: string
    min_length: 1
    max_length: 100
    
  - name: priority
    description: "Priority level"
    type: number
    default: 5
    choices: [1, 2, 3, 4, 5]
    
  - name: include_examples
    description: "Include code examples"
    type: boolean
    default: true
    
  - name: sections
    description: "Sections to include"
    type: array
    default: ["introduction", "usage", "examples"]
```

## Template System

SwissArmyHammer uses the Liquid template engine for dynamic content.

### Variable Substitution

Basic variable substitution:

```liquid
Hello {{name}}, welcome to {{project}}!
```

With arguments:
- `name: "Alice"`
- `project: "SwissArmyHammer"`

Renders as:
```
Hello Alice, welcome to SwissArmyHammer!
```

### Conditionals

```liquid
{% if language == "rust" %}
Use `cargo test` to run tests.
{% elsif language == "python" %}
Use `pytest` to run tests.
{% else %}
Refer to your language's testing framework.
{% endif %}
```

### Loops

```liquid
## Requirements
{% for req in requirements %}
- {{req}}
{% endfor %}

## Steps
{% for step in steps %}
{{forloop.index}}. {{step.title}}
   {{step.description}}
{% endfor %}
```

### Filters

Liquid filters transform values:

```liquid
{{name | capitalize}}           <!-- "john" → "John" -->
{{text | truncate: 50}}        <!-- Limit to 50 characters -->
{{items | join: ", "}}         <!-- Array to comma-separated -->
{{code | escape}}              <!-- HTML-safe escaping -->
{{date | date: "%Y-%m-%d"}}    <!-- Format date -->
```

#### Built-in Filters

| Filter | Description | Example |
|--------|-------------|---------|
| `capitalize` | Capitalize first letter | `{{name \| capitalize}}` |
| `downcase` | Convert to lowercase | `{{text \| downcase}}` |
| `upcase` | Convert to uppercase | `{{text \| upcase}}` |
| `truncate` | Limit string length | `{{text \| truncate: 100}}` |
| `strip` | Remove whitespace | `{{text \| strip}}` |
| `escape` | HTML escape | `{{html \| escape}}` |
| `join` | Join array elements | `{{items \| join: ", "}}` |
| `split` | Split string | `{{text \| split: ","}}` |
| `size` | Get length | `{{array \| size}}` |
| `first` | Get first element | `{{array \| first}}` |
| `last` | Get last element | `{{array \| last}}` |
| `sort` | Sort array | `{{items \| sort}}` |
| `uniq` | Remove duplicates | `{{items \| uniq}}` |
| `reverse` | Reverse array | `{{items \| reverse}}` |
| `default` | Default if nil | `{{value \| default: "none"}}` |

#### Custom Filters

SwissArmyHammer includes custom filters for development:

| Filter | Description | Example |
|--------|-------------|---------|
| `snake_case` | Convert to snake_case | `{{text \| snake_case}}` |
| `kebab_case` | Convert to kebab-case | `{{text \| kebab_case}}` |
| `pascal_case` | Convert to PascalCase | `{{text \| pascal_case}}` |
| `camel_case` | Convert to camelCase | `{{text \| camel_case}}` |
| `pluralize` | Make plural | `{{word \| pluralize}}` |
| `singularize` | Make singular | `{{word \| singularize}}` |
| `markdown_escape` | Escape markdown | `{{text \| markdown_escape}}` |
| `code_block` | Wrap in code block | `{{code \| code_block: "rust"}}` |

### Environment Variables

Access environment variables in templates:

```liquid
Project: {{PROJECT_NAME | default: "Unknown"}}
Environment: {{NODE_ENV | default: "development"}}
User: {{USER | default: "unknown"}}
Home: {{HOME}}
```

### Advanced Features

#### Include Other Files

```liquid
{% include "common/header.md" %}

Main content here...

{% include "common/footer.md" %}
```

#### Assign Variables

```liquid
{% assign formatted_name = name | capitalize | strip %}
{% assign item_count = items | size %}

Hello {{formatted_name}}, you have {{item_count}} items.
```

#### Capture Content

```liquid
{% capture error_message %}
Error in {{file}}:{{line}}: {{message}}
{% endcapture %}

{% if show_errors %}
{{error_message}}
{% endif %}
```

## Prompt Discovery

SwissArmyHammer discovers prompts from multiple locations with precedence:

### 1. Built-in Prompts

Embedded prompts always available:

```bash
sah prompt list --source builtin
```

Common built-in prompts:
- `code` - Code review and analysis
- `documentation` - Generate documentation  
- `debug` - Debug assistance
- `test` - Test writing guidance
- `refactor` - Refactoring suggestions

### 2. User Prompts

Personal prompts in `~/.swissarmyhammer/prompts/`:

```bash
# List user prompts
sah prompt list --source user

# Create a user prompt
mkdir -p ~/.swissarmyhammer/prompts
editor ~/.swissarmyhammer/prompts/my-prompt.md
```

### 3. Local Prompts

Project-specific prompts in `./.swissarmyhammer/prompts/`:

```bash
# Create project prompt
mkdir -p .swissarmyhammer/prompts
editor .swissarmyhammer/prompts/project-specific.md
```

### Precedence Rules

When prompts have the same name:
1. **Local** (`./.swissarmyhammer/prompts/`) - highest precedence
2. **User** (`~/.swissarmyhammer/prompts/`) - medium precedence  
3. **Built-in** (embedded) - lowest precedence

## Using Prompts

### CLI Usage

```bash
# Test a prompt
sah prompt test my-prompt --var name="value"

# Render a prompt to file
sah prompt render my-prompt --var name="value" --output result.md

# List available prompts
sah prompt list

# Show prompt details
sah prompt show my-prompt

# Validate prompt syntax
sah prompt validate my-prompt
```

### MCP Usage (Claude Code)

```
/my-prompt name="value" other_arg="value2"
```

### Library Usage

```rust
use swissarmyhammer::prelude::*;
use std::collections::HashMap;

// Create prompt library
let mut library = PromptLibrary::new();
library.add_directory("~/.swissarmyhammer/prompts")?;

// Get and render prompt
let prompt = library.get("my-prompt")?;
let mut args = HashMap::new();
args.insert("name".to_string(), "Alice".to_string());
let rendered = prompt.render(&args)?;

println!("{}", rendered);
```

## Best Practices

### Prompt Design

1. **Clear Purpose** - Each prompt should have a single, well-defined purpose
2. **Good Documentation** - Use descriptive titles and detailed descriptions  
3. **Flexible Arguments** - Support optional arguments with sensible defaults
4. **Structured Output** - Guide the AI to provide well-formatted responses
5. **Error Handling** - Handle missing or invalid arguments gracefully

### Argument Design

```yaml
arguments:
  # Good: Clear, descriptive, with defaults
  - name: programming_language
    description: "The programming language being used"
    required: true
    type: string
    choices: ["rust", "python", "javascript", "typescript"]
    
  - name: include_examples
    description: "Whether to include code examples in the response"
    type: boolean
    default: true
    
  # Avoid: Vague names and descriptions
  - name: thing
    description: "A thing"
    required: true
```

### Template Best Practices

1. **Escape User Input** - Use `| escape` filter for untrusted content
2. **Provide Defaults** - Use `| default: "fallback"` for optional values
3. **Validate Conditionally** - Check if variables exist before using them
4. **Format Consistently** - Use filters to ensure consistent formatting

Example of good template practices:

```liquid
# {{title | default: "Untitled" | capitalize}}

{% if description %}
**Description:** {{description | strip}}
{% endif %}

{% if tags and tags.size > 0 %}
**Tags:** {{tags | join: ", " | downcase}}
{% endif %}

{% assign lang = language | default: "text" | downcase %}
{% if lang == "rust" %}
This is Rust-specific guidance...
{% endif %}

{% for item in items | default: array %}
- {{item | escape | capitalize}}
{% endfor %}
```

### Organization

1. **Use Tags** - Categorize prompts with meaningful tags
2. **Version Control** - Track prompt changes with version numbers
3. **Modular Design** - Break complex prompts into reusable components
4. **Consistent Naming** - Use clear, descriptive filenames

### Testing

```bash
# Test with various inputs
sah prompt test my-prompt --var lang="rust"
sah prompt test my-prompt --var lang="python"
sah prompt test my-prompt --var lang="invalid"

# Test required arguments
sah prompt test my-prompt
sah prompt test my-prompt --var required_arg="value"

# Validate syntax
sah validate --prompts
```

## Advanced Features

### Prompt Inheritance

Create base prompts that others can extend:

```markdown
<!-- base-review.md -->
---
title: Base Review Template
description: Common review structure
arguments:
  - name: type
    description: Type of review
    required: true
---

# {{type | capitalize}} Review

## Analysis
[Analysis goes here]

## Recommendations  
[Recommendations go here]
```

```markdown
<!-- code-review.md -->
---
title: Code Review
description: Code-specific review
extends: base-review
arguments:
  - name: language
    description: Programming language
    required: true
---

{% assign type = "code" %}
{% include "base-review" %}

## Code Quality Metrics
- Language: {{language}}
- [Additional code-specific content]
```

### Dynamic Argument Loading

Load arguments from files or environment:

```yaml
arguments:
  - name: config
    description: "Configuration file path"
    type: string
    default: "config.json"
    load_from_file: true
    
  - name: api_key
    description: "API key for service"
    type: string
    load_from_env: "API_KEY"
    required: false
```

### Prompt Libraries

Create shareable prompt collections:

```
my-prompt-library/
├── README.md
├── package.toml
├── prompts/
│   ├── web-dev/
│   │   ├── react-component.md
│   │   └── api-endpoint.md
│   └── data-science/
│       ├── analysis.md
│       └── visualization.md
└── templates/
    ├── common/
    │   ├── header.liquid
    │   └── footer.liquid
    └── helpers/
        └── formatting.liquid
```

## Real-World Prompt Examples

### 1. Pull Request Review Prompt

```markdown
---
title: Pull Request Reviewer
description: Comprehensive PR review focusing on code quality and maintainability
version: "2.1"
tags: ["pr", "review", "git", "collaboration"]
arguments:
  - name: pr_url
    description: GitHub PR URL for context
    required: true
    type: string
  - name: changed_files
    description: List of files changed in the PR
    required: true
    type: array
  - name: review_depth
    description: Level of review detail
    type: string
    default: "standard"
    choices: ["quick", "standard", "thorough"]
  - name: team_standards
    description: Team-specific coding standards
    type: string
    load_from_file: "./.swissarmyhammer/team-standards.md"
---

# Pull Request Review

Reviewing PR: {{pr_url}}

## Changed Files
{% for file in changed_files %}
- **{{file}}**{% if file contains "test" %} (Test file){% endif %}
{% endfor %}

## Review Criteria

{% if review_depth == "quick" %}
**Quick Review Focus:**
- Compilation and basic functionality
- Critical security issues
- Breaking changes
{% elsif review_depth == "thorough" %}
**Thorough Review Focus:**
- Code architecture and design patterns
- Performance implications
- Test coverage and quality
- Documentation completeness
- Accessibility considerations
- Security best practices
{% else %}
**Standard Review Focus:**
- Code quality and readability
- Logic correctness
- Error handling
- Testing adequacy
{% endif %}

{% if team_standards %}
## Team Standards
{{team_standards}}
{% endif %}

Please provide:

1. **Summary**: Overall assessment and recommendation (approve/request changes/comment)
2. **Critical Issues**: Must-fix problems that block merging
3. **Suggestions**: Improvements that would enhance code quality
4. **Praise**: What was done well in this PR
5. **Learning**: Any new patterns or approaches worth noting

Format your response with specific line numbers and file references where applicable.
```

### 2. API Documentation Generator

```markdown
---
title: API Documentation Generator  
description: Generates comprehensive API documentation from code analysis
version: "1.5"
tags: ["api", "documentation", "openapi"]
arguments:
  - name: api_type
    description: Type of API being documented
    type: string
    default: "REST"
    choices: ["REST", "GraphQL", "gRPC"]
  - name: language
    description: Programming language of the API
    required: true
    type: string
  - name: base_url
    description: Base URL for the API
    type: string
    default: "https://api.example.com"
  - name: authentication
    description: Authentication method used
    type: string
    default: "Bearer Token"
  - name: include_examples
    description: Include usage examples
    type: boolean
    default: true
---

# {{api_type}} API Documentation

## Overview
This documentation covers the {{language}} {{api_type}} API.

**Base URL**: `{{base_url}}`
**Authentication**: {{authentication}}

## Getting Started

### Authentication
{% if authentication contains "Bearer" %}
Include your API token in the Authorization header:
```
Authorization: Bearer YOUR_TOKEN_HERE
```
{% elsif authentication contains "API Key" %}
Include your API key in the request headers:
```
X-API-Key: YOUR_API_KEY_HERE
```
{% endif %}

### Rate Limits
- Authenticated requests: 1000 requests per hour
- Unauthenticated requests: 100 requests per hour

## Endpoints

{% if api_type == "REST" %}
*[SwissArmyHammer will analyze your code and generate endpoint documentation here]*

For each endpoint, please include:
- HTTP method and path
- Description and purpose
- Request parameters (path, query, body)
- Response format and examples
- Possible error codes and meanings
{% elsif api_type == "GraphQL" %}
*[SwissArmyHammer will analyze your schema and generate query documentation here]*

Please document:
- Available queries and mutations
- Input types and validation rules
- Response types and nested relationships
- Example queries with variables
{% endif %}

{% if include_examples %}
## Code Examples

### Python
```python
import requests

# Basic request example
response = requests.get(
    "{{base_url}}/endpoint", 
    headers={"Authorization": "Bearer YOUR_TOKEN"}
)
print(response.json())
```

### JavaScript
```javascript
const response = await fetch("{{base_url}}/endpoint", {
    headers: {
        "Authorization": "Bearer YOUR_TOKEN",
        "Content-Type": "application/json"
    }
});
const data = await response.json();
```

### cURL
```bash
curl -X GET "{{base_url}}/endpoint" \
     -H "Authorization: Bearer YOUR_TOKEN" \
     -H "Content-Type: application/json"
```
{% endif %}

## Error Handling

All errors follow a consistent format:
```json
{
    "error": {
        "code": "ERROR_CODE",
        "message": "Human-readable description",
        "details": {}
    }
}
```

## Changelog

Document API version changes and migration notes here.
```

### 3. Testing Strategy Prompt

```markdown
---
title: Test Strategy Generator
description: Creates comprehensive testing strategies for software projects
version: "1.3"
tags: ["testing", "qa", "strategy"]
arguments:
  - name: project_type
    description: Type of project being tested
    required: true
    type: string
    choices: ["web-app", "api", "mobile", "desktop", "library"]
  - name: tech_stack
    description: Main technologies used
    required: true
    type: array
  - name: team_size
    description: Size of the development team
    type: number
    default: 5
  - name: release_frequency
    description: How often releases are made
    type: string
    default: "bi-weekly"
    choices: ["daily", "weekly", "bi-weekly", "monthly"]
  - name: critical_features
    description: Features that require extra testing attention
    type: array
    default: []
---

# Testing Strategy for {{project_type | title}} Project

## Project Context
- **Technology Stack**: {% for tech in tech_stack %}{{tech}}{% unless forloop.last %}, {% endunless %}{% endfor %}
- **Team Size**: {{team_size}} developers
- **Release Cycle**: {{release_frequency}}
- **Critical Features**: {% if critical_features.size > 0 %}{% for feature in critical_features %}{{feature}}{% unless forloop.last %}, {% endunless %}{% endfor %}{% else %}None specified{% endif %}

## Testing Pyramid

### Unit Tests (70% of tests)
**Goal**: Fast feedback on individual components

{% if project_type == "web-app" %}
- Component unit tests
- Utility function tests  
- State management tests
- Hook/composable tests
{% elsif project_type == "api" %}
- Service layer tests
- Database model tests
- Utility function tests
- Middleware tests
{% elsif project_type == "mobile" %}
- Business logic tests
- State management tests
- Utility function tests
- Platform-specific component tests
{% endif %}

**Coverage Target**: 90%+ for business logic

### Integration Tests (20% of tests)
**Goal**: Verify component interactions

{% if project_type == "web-app" %}
- API integration tests
- Database integration tests
- Third-party service integration tests
- Feature workflow tests
{% elsif project_type == "api" %}
- Database integration tests
- External service integration tests
- Authentication flow tests
- API contract tests
{% endif %}

### End-to-End Tests (10% of tests)
**Goal**: Validate critical user journeys

{% for feature in critical_features %}
- {{feature}} complete workflow
{% endfor %}
- Happy path scenarios
- Error recovery scenarios

## Test Automation Strategy

### Continuous Integration
```yaml
# GitHub Actions example
name: Test Suite
on: [push, pull_request]
jobs:
  test:
    steps:
      - uses: actions/checkout@v3
      - name: Run Unit Tests
        run: npm test
      - name: Run Integration Tests  
        run: npm run test:integration
      - name: E2E Tests
        run: npm run test:e2e
```

### Test Data Management
- Use factories/fixtures for consistent test data
- Database seeding for integration tests
- Mock external services in unit tests
- Use test-specific data in staging environment

### Performance Testing
{% if release_frequency == "daily" or release_frequency == "weekly" %}
- Automated performance regression tests
- Load testing on every release
{% else %}
- Monthly performance validation
- Load testing before major releases
{% endif %}

## Quality Gates

### Pre-commit Hooks
- Lint and format checks
- Unit test execution
- Type checking (if applicable)

### Pull Request Requirements
- All tests passing
- Code coverage maintained above 80%
- Performance benchmarks within acceptable range

### Release Criteria
- Full test suite passes
- Manual testing of critical features completed
- Performance benchmarks validated
- Security scans passed

## Testing Tools and Frameworks

### Recommended Stack
{% if tech_stack contains "JavaScript" or tech_stack contains "TypeScript" %}
- **Unit Testing**: Jest/Vitest
- **Integration Testing**: Supertest (APIs), Testing Library (components)
- **E2E Testing**: Playwright or Cypress
- **Performance**: Lighthouse CI
{% elsif tech_stack contains "Python" %}
- **Unit Testing**: pytest
- **Integration Testing**: pytest with fixtures
- **E2E Testing**: Selenium or Playwright
- **Performance**: locust
{% elsif tech_stack contains "Rust" %}
- **Unit Testing**: Built-in test framework
- **Integration Testing**: Custom integration test crates
- **E2E Testing**: Selenium or custom tooling
- **Performance**: Criterion benchmarks
{% endif %}

### Monitoring and Reporting
- Test result dashboards
- Coverage reporting with trends
- Flaky test identification and tracking
- Performance regression alerts

## Risk Assessment

### High-Risk Areas Requiring Extra Testing
{% for feature in critical_features %}
- **{{feature}}**: Critical to business operations
{% endfor %}
- Authentication and authorization
- Data persistence and integrity
- Payment processing (if applicable)
- Third-party integrations

### Testing Schedule
{% if release_frequency == "daily" %}
- Unit tests: Every commit
- Integration tests: Every commit
- E2E tests: Nightly
- Performance tests: Weekly
{% else %}
- Unit tests: Every commit
- Integration tests: Every PR
- E2E tests: Before release
- Performance tests: Monthly
{% endif %}

This strategy balances thorough testing with development velocity for your {{release_frequency}} release cycle.
```

### 4. Architecture Decision Record (ADR) Template

```markdown
---
title: Architecture Decision Record Template
description: Template for documenting architectural decisions with context and rationale
version: "2.0"
tags: ["architecture", "documentation", "decision"]
arguments:
  - name: decision_title
    description: Brief title for the decision
    required: true
    type: string
  - name: decision_date
    description: Date of the decision
    type: string
    default: "{{ 'now' | date: '%Y-%m-%d' }}"
  - name: status
    description: Status of the decision
    type: string
    default: "Proposed"
    choices: ["Proposed", "Accepted", "Deprecated", "Superseded"]
  - name: stakeholders
    description: People involved in or affected by the decision
    type: array
    default: []
---

# ADR-XXX: {{decision_title}}

**Status**: {{status}}
**Date**: {{decision_date}}
{% if stakeholders.size > 0 %}**Stakeholders**: {% for person in stakeholders %}{{person}}{% unless forloop.last %}, {% endunless %}{% endfor %}{% endif %}

## Context

*What is the situation that requires a decision? Include relevant background information, constraints, and requirements.*

## Decision

*What is the change we're making? State the decision clearly and concisely.*

## Rationale  

*Why are we making this decision? What factors influenced this choice?*

### Options Considered

1. **Option 1**: [Description]
   - Pros: [Benefits]
   - Cons: [Drawbacks]

2. **Option 2**: [Description]  
   - Pros: [Benefits]
   - Cons: [Drawbacks]

3. **Selected Option**: [Description]
   - Pros: [Benefits]
   - Cons: [Drawbacks]

## Consequences

### Positive
- *What benefits do we expect?*
- *What capabilities does this enable?*

### Negative  
- *What trade-offs are we making?*
- *What challenges might this create?*

### Neutral
- *What else changes as a result of this decision?*

## Implementation

### Action Items
- [ ] Task 1
- [ ] Task 2
- [ ] Task 3

### Timeline
- **Phase 1** (Week 1-2): [Description]
- **Phase 2** (Week 3-4): [Description]
- **Complete by**: [Date]

### Success Metrics
- Metric 1: [How to measure]
- Metric 2: [How to measure]

## References

- [Link to relevant documentation]
- [Related ADRs or decisions]
- [External resources that influenced the decision]

## Follow-up

*When should this decision be revisited? What might trigger a reconsideration?*
```

## Advanced Prompt Techniques

### Dynamic Content Loading

Load content from files during prompt rendering:

```markdown
---
title: Context-Aware Code Review
description: Code review with dynamic project context
arguments:
  - name: file_path
    required: true
    type: string
  - name: project_readme
    type: string
    load_from_file: "./README.md"
---

# Code Review with Project Context

## Project Overview
{{project_readme | truncate: 500}}

## Coding Standards  
{{coding_standards}}

## File to Review
File: {{file_path}}

*[File content would be loaded by your AI assistant]*

Please review this file considering the project context and coding standards above.
```

### Conditional Logic and Complex Templates

```markdown
---
title: Multi-Language Documentation
description: Generates documentation based on detected language
arguments:
  - name: language
    required: true
    type: string
  - name: complexity
    type: string
    default: "medium"
    choices: ["simple", "medium", "complex"]
---

# {{language | title}} Documentation

{% case language %}
  {% when "rust" %}
    ## Rust-Specific Guidelines
    - Use `cargo doc` for documentation
    - Follow Rust naming conventions
    - Include examples in doc comments
  {% when "python" %}
    ## Python-Specific Guidelines
    - Use docstrings for all public functions
    - Follow PEP 8 style guidelines
    - Include type hints where appropriate  
  {% when "javascript" %}
    ## JavaScript-Specific Guidelines
    - Use JSDoc for function documentation
    - Follow ESLint recommended rules
    - Include usage examples
  {% else %}
    ## General Guidelines
    - Clear and concise documentation
    - Include usage examples
    - Follow language-specific conventions
{% endcase %}

{% if complexity == "complex" %}
## Advanced Topics
Please include:
- Architecture diagrams
- Performance considerations
- Security implications
- Integration patterns
{% elsif complexity == "simple" %}
## Basic Documentation
Focus on:
- Basic usage instructions
- Simple examples
- Getting started guide
{% endif %}
```

### Environment-Specific Prompts

```markdown
---
title: Environment-Aware Deploy Guide
description: Deployment instructions that vary by environment
arguments:
  - name: environment
    required: true
    type: string
    choices: ["development", "staging", "production"]
  - name: app_name
    required: true
    type: string
  - name: db_host
    type: string
    load_from_env: "DATABASE_HOST"
  - name: deploy_key
    type: string
    load_from_env: "DEPLOY_KEY"
    required: false
---

# Deploy {{app_name}} to {{environment | title}}

{% if environment == "production" %}
## ⚠️  Production Deployment Checklist

**CRITICAL**: This is a production deployment. Ensure:
- [ ] All tests are passing
- [ ] Database migrations are reviewed
- [ ] Rollback plan is prepared
- [ ] Team is notified
- [ ] Monitoring is active

{% elsif environment == "staging" %}
## Staging Deployment

This deployment will:
- Update the staging environment
- Run integration tests
- Validate changes before production

{% else %}
## Development Deployment

Quick development deployment for testing changes.
{% endif %}

## Configuration

**Database**: {% if db_host %}{{db_host}}{% else %}*Configure DATABASE_HOST environment variable*{% endif %}
{% if deploy_key %}**Deploy Key**: Configured from environment{% else %}**Deploy Key**: *Set DEPLOY_KEY environment variable*{% endif %}

## Commands

```bash
# Set environment
export NODE_ENV={{environment}}

{% if environment == "production" %}
# Production-specific setup
npm ci --only=production
npm run build:production
npm run migrate:production
{% else %}  
# Development/staging setup
npm install
npm run build
npm run migrate
{% endif %}

# Deploy
npm run deploy:{{environment}}
```

{% if environment == "production" %}
## Post-Deploy Verification

1. Check application health: `curl https://{{app_name}}.com/health`
2. Verify database connectivity
3. Check error logs for any issues
4. Validate key user journeys
5. Monitor performance metrics

## Rollback Plan

If issues are detected:
```bash
npm run rollback:production
```
{% endif %}
```

## Prompt Debugging Guide

### Common Template Issues

#### 1. Variable Not Rendered

**Problem**: Variables show as `{{variable_name}}` in output
```markdown
Hello {{user_name}}!
```
Output: `Hello {{user_name}}!`

**Solutions**:
```bash
# Check variable is defined
sah prompt test my-prompt --var user_name="Alice" --debug

# Verify variable name spelling
sah prompt validate my-prompt.md

# Check front matter argument definition
---
arguments:
  - name: user_name  # Must match exactly
    required: true
---
```

#### 2. Conditional Logic Not Working

**Problem**: Conditions always evaluate incorrectly
```markdown
{% if user_role == "admin" %}
Admin content
{% endif %}
```

**Debug Steps**:
```bash
# Test with debug output
sah prompt test my-prompt --var user_role="admin" --debug

# Check variable type - strings need quotes
{% if user_role == "admin" %}  # Correct
{% if user_role == admin %}    # Wrong - no quotes
```

**Common Issues**:
- Missing quotes around string values
- Case sensitivity: `"Admin" != "admin"`
- Type mismatches: `"5" != 5`

#### 3. Loop Not Iterating

**Problem**: For loops don't execute
```markdown
{% for item in items %}
- {{item}}
{% endfor %}
```

**Solutions**:
```bash
# Ensure items is an array
sah prompt test my-prompt --var 'items=["a","b","c"]' --debug

# Check array syntax in CLI
--var 'items=["item1", "item2"]'  # JSON format
--var items.0="item1" --var items.1="item2"  # Individual items
```

#### 4. File Loading Fails

**Problem**: `load_from_file` doesn't work
```yaml
arguments:
  - name: config
    load_from_file: "./config.yaml"
```

**Debug**:
```bash
# Check file exists and is readable
ls -la ./config.yaml
cat ./config.yaml

# Use absolute paths if relative paths fail
load_from_file: "/full/path/to/config.yaml"

# Verify file format matches expected type
# YAML files are parsed, text files loaded as strings
```

### Debugging Tools and Techniques

#### Enable Debug Mode

```bash
# Verbose output shows variable resolution
sah prompt test my-prompt --debug

# Show template parsing steps
RUST_LOG=swissarmyhammer::template=debug sah prompt test my-prompt

# Validate prompt syntax
sah prompt validate my-prompt.md
```

#### Template Testing Workflow

```bash
# 1. Validate syntax first
sah prompt validate my-prompt.md

# 2. Test with minimal variables
sah prompt test my-prompt --var required_var="test"

# 3. Add variables incrementally
sah prompt test my-prompt --var var1="value1" --var var2="value2"

# 4. Test edge cases
sah prompt test my-prompt --var empty_string="" --var zero_number=0
```

#### Variable Inspection

```markdown
---
title: Debug Template
---

# Variable Debugging

## All Variables
{% for variable in __variables__ %}
- {{variable[0]}}: {{variable[1]}} ({{variable[1] | type}})
{% endfor %}

## Specific Variable Analysis
- user_name: "{{user_name}}" (length: {{user_name | size}})
- is_admin: {{is_admin}} (type: {{is_admin | type}})
- items count: {{items | size}}
```

#### Common Filter Issues

```markdown
# String filters on non-strings
{{number | upcase}}  # Error - upcase only works on strings
{{number | string | upcase}}  # Fixed - convert to string first

# Array filters on non-arrays  
{{string | first}}  # Error - first only works on arrays
{{string | split: "," | first}}  # Fixed - split creates array

# Chaining incompatible filters
{{text | split: "," | upcase}}  # Error - upcase doesn't work on arrays
{{text | split: "," | map: "upcase"}}  # Fixed - map applies upcase to each item
```

### Performance Debugging

#### Slow Template Rendering

```bash
# Profile template complexity
sah prompt profile my-prompt.md

# Identify bottlenecks
RUST_LOG=swissarmyhammer::template=debug sah prompt test my-prompt 2>&1 | grep "duration"
```

**Common Performance Issues**:

1. **Large file loading**:
```yaml
# Slow - loads entire file
load_from_file: "./huge-file.json"

# Better - load and truncate
load_from_file: "./huge-file.json"
filter: "truncate:1000"
```

2. **Complex loops**:
```markdown
# Slow - nested loops with complex logic
{% for user in users %}
  {% for role in user.roles %}
    {% if role.permissions contains "admin" %}
      Complex processing...
    {% endif %}
  {% endfor %}
{% endfor %}

# Better - pre-filter data or use simpler logic
{% assign admin_users = users | where: "roles", "admin" %}
{% for user in admin_users %}
  Simple processing...
{% endfor %}
```

### Error Recovery Strategies

```markdown
---
title: Robust Template with Error Handling
arguments:
  - name: optional_data
    required: false
    type: string
---

# Robust Content

## Safe Variable Access
{% if optional_data and optional_data != "" %}
Data: {{optional_data}}
{% else %}
No data provided
{% endif %}

## Array Safety
{% if items and items.size > 0 %}
Items:
{% for item in items %}
- {{item | default: "Unknown item"}}
{% endfor %}
{% else %}
No items available
{% endif %}

## File Loading with Fallback
{% assign config = config_file | default: "No configuration loaded" %}
Configuration: {{config}}

## Division Safety
{% assign rate = total | divided_by: count | default: 0 %}
Success rate: {{rate}}%
```

This comprehensive prompt system provides the foundation for consistent, reusable AI interactions across all your projects, with robust debugging capabilities and real-world examples to guide implementation.