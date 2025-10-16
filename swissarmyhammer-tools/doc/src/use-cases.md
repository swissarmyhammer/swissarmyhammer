# Use Cases and Best Practices

This guide provides practical use cases and best practices for using SwissArmyHammer Tools effectively in your development workflows.

## Common Use Cases

### Code Navigation and Understanding

**Problem**: Large codebases are difficult to navigate and understand.

**Solution**: Use semantic search to find relevant code quickly.

```
# First, index your codebase
"Index all Rust files for semantic search"

# Then search by functionality
"Find authentication logic"
"Show me error handling patterns"
"Where is database connection management?"
```

**Best Practices**:
- Index files after major changes or when joining a project
- Use natural language queries instead of keyword searches
- Combine semantic search with outline generation for comprehensive understanding
- Limit search results to stay focused on the most relevant code

**Example Workflow**:
1. Index codebase: `search_index` with patterns for your language
2. Search for functionality: `search_query` with natural language
3. Generate outline: `outline_generate` for detailed structure
4. Read specific files: `files_read` to examine implementations

### Feature Implementation Workflow

**Problem**: Implementing new features requires tracking work, making changes, and ensuring quality.

**Solution**: Use the complete issue tracking and workflow system.

```
# Create an issue for the feature
"Create an issue for implementing user authentication"

# Switch to issue branch
git checkout -b issue/user-auth

# Make code changes using file tools
"Edit src/auth.rs to add password hashing"

# Track progress with todos
"Add a todo to implement password validation"

# Check code quality
"Check src/auth.rs for rule violations"

# Mark issue complete when done
"Mark the authentication issue as complete"
```

**Best Practices**:
- Create issues before starting work for better tracking
- Use descriptive issue names that match branch names
- Break large features into smaller issues
- Check code quality before marking issues complete
- Keep issue content updated with progress and decisions

### Refactoring Large Codebases

**Problem**: Refactoring across many files is error-prone and time-consuming.

**Solution**: Use search and batch file operations.

```
# Find all occurrences of the pattern
"Search for usages of old_function"

# Generate outline to understand structure
"Create an outline of files using old_function"

# Make changes across files
"Replace old_function with new_function in all files"

# Verify changes with tests
"Run cargo nextest run"

# Check for issues
"Search for any remaining references to old_function"
```

**Best Practices**:
- Always search before replacing to understand impact
- Use `files_edit` with specific old_string for precise replacements
- Test after each batch of changes
- Use git to track changes and enable rollback
- Document refactoring decisions in memos for future reference

### Documentation and Knowledge Management

**Problem**: Project knowledge is scattered or lost over time.

**Solution**: Use the memo system for persistent knowledge capture.

```
# Create memos for important decisions
"Create a memo about our authentication architecture"

# Document patterns and conventions
"Create a memo about our error handling patterns"

# Capture meeting notes
"Create a memo about today's planning meeting"

# Retrieve knowledge when needed
"Show me the memo about authentication"

# Get all context for comprehensive understanding
"Show me all memos for context"
```

**Best Practices**:
- Create memos for architectural decisions
- Document patterns and conventions as you discover them
- Use descriptive titles for easy retrieval
- Keep memos focused on single topics
- Update memos rather than creating duplicates
- Use memos to onboard new team members

### Code Review and Quality Assurance

**Problem**: Maintaining code quality across contributions.

**Solution**: Use rules engine and systematic review process.

```
# Check code against quality rules
"Check all Rust files for rule violations"

# Check specific areas
"Check src/**/*.rs for unwrap usage"

# Generate outline to understand changes
"Create an outline of changed files"

# Review git changes
"Show me all files changed on this branch"

# Check test coverage
"Run tests and show coverage"
```

**Best Practices**:
- Run rules checks before committing
- Review outlines to understand code structure
- Check for common issues (unwrap, panic, etc.)
- Verify tests pass before marking work complete
- Use git_changes to focus review on modified files

### Multi-Step Workflow Automation

**Problem**: Complex development tasks involve many steps.

**Solution**: Use workflow execution for automation.

```
# List available workflows
"Show me available workflows"

# Execute a workflow
"Run the deployment workflow"

# Track workflow progress
"Check the status of the current workflow"

# Handle workflow errors
"Show me any workflow errors"
```

**Best Practices**:
- Define workflows as YAML for repeatability
- Break workflows into clear, testable steps
- Use descriptive step names for clarity
- Handle errors gracefully with retry logic
- Monitor workflow progress with notifications
- Document workflow parameters and requirements

### Cross-Project Code Search

**Problem**: Finding examples across multiple repositories.

**Solution**: Use web search and local semantic search together.

```
# Search the web for examples
"Search for rust async programming examples"

# Fetch documentation
"Fetch the content from the Rust async book"

# Search local examples
"Find async functions in our codebase"

# Compare patterns
"Create an outline of our async code"
```

**Best Practices**:
- Use web search to find external examples and documentation
- Index multiple projects for cross-project search
- Compare external patterns with your codebase
- Document adopted patterns in memos
- Create examples directory for reference implementations

## Best Practices by Feature

### File Operations

**Always use absolute paths**:
```
✓ /Users/name/project/src/main.rs
✗ ./src/main.rs
✗ ../src/main.rs
```

**Read before editing**:
- Always read file content before editing
- Verify exact strings to replace
- Account for whitespace and invisible characters

**Use appropriate tools**:
- `files_read`: View file content
- `files_edit`: Precise replacements
- `files_write`: Create or completely rewrite files
- `files_glob`: Find files by pattern
- `files_grep`: Search content

### Semantic Search

**Index strategically**:
- Index when starting work on a project
- Re-index after major changes
- Use `force: true` to rebuild complete index
- Index only languages you work with

**Query effectively**:
- Use natural language descriptions
- Be specific but not too narrow
- Adjust limit based on your needs
- Review similarity scores to gauge relevance

**Maintain the index**:
- Add `.swissarmyhammer/search.db` to `.gitignore`
- Rebuild index if results seem stale
- Clear index if it becomes too large

### Issue Management

**Naming conventions**:
- Use descriptive names: `feature_auth` not `issue1`
- Match branch names: `issue/feature-auth`
- Include type prefix: `feature_`, `bug_`, `refactor_`

**Content structure**:
```markdown
# Feature: User Authentication

## Goal
Implement secure user authentication system

## Requirements
- Password hashing with bcrypt
- Session management
- Login/logout endpoints

## Implementation Notes
- Use existing crypto library
- Follow OWASP guidelines

## Test Plan
- Unit tests for password hashing
- Integration tests for auth flow
```

**Lifecycle management**:
- Create issues before starting work
- Update issues as you progress
- Use `issue_show current` to view active issue
- Mark complete only when fully done

### Memo System

**Organization**:
- Use clear, descriptive titles
- One topic per memo
- Include dates for time-sensitive information
- Cross-reference related memos

**Content structure**:
```markdown
# Authentication Architecture

## Decision
Use JWT tokens for authentication

## Rationale
- Stateless authentication
- Works well with microservices
- Industry standard

## Implementation
See src/auth/jwt.rs for implementation

## Related
- See "API Security" memo for additional context
```

**Maintenance**:
- Review and update memos periodically
- Delete outdated memos
- Consolidate related information
- Use `memo_get_all_context` for comprehensive reviews

### Todo Tracking

**Use for ephemeral tasks**:
- Break down current work
- Track implementation steps
- Manage debugging tasks

**Not for long-term tracking**:
- Use issues for persistent work items
- Todos are session-specific
- Automatically cleaned up when complete

**Effective usage**:
```
# Break down implementation
"Add todo to implement JWT signing"
"Add todo to implement JWT verification"
"Add todo to add JWT tests"

# Work through list
"What's the next todo?"
# Complete each todo as you go
"Mark current todo complete"
```

## Anti-Patterns to Avoid

### Don't: Duplicate Search Indices
Semantic search indices are large. Don't commit them to git or sync them.

**Do**: Add to `.gitignore` and build locally:
```
.swissarmyhammer/search.db
```

### Don't: Use Relative Paths
Tools expect absolute paths and relative paths may fail.

**Do**: Use absolute paths:
```
realpath ./src/main.rs  # Convert to absolute
```

### Don't: Edit Without Reading
Blind edits often fail due to formatting differences.

**Do**: Read first, then edit:
```
"Show me src/main.rs"
# Verify exact content
"Replace 'old string' with 'new string' in src/main.rs"
```

### Don't: Create Too Many Issues
Too many issues becomes hard to manage.

**Do**: Use hierarchical tracking:
- Issues for major work items
- Todos for implementation steps
- Memos for decisions and knowledge

### Don't: Ignore Rule Violations
Code quality issues accumulate quickly.

**Do**: Check rules regularly:
```
"Check for rule violations before committing"
```

### Don't: Over-Parameterize Queries
Too many filters may miss relevant results.

**Do**: Start broad, then narrow:
```
# Start broad
"Search for authentication"

# Then narrow if needed
"Search for authentication in src/auth/"
```

## Integration Patterns

### With Claude Code

SwissArmyHammer Tools works seamlessly with Claude Code:

- Describe tasks in natural language
- Claude Code selects appropriate tools
- Progress is tracked automatically
- Context is preserved across sessions

**Example conversation**:
```
You: "I need to implement user registration"

Claude: "I'll help you implement user registration. Let me start by creating an issue and then we'll work through the implementation."

[Claude uses issue_create, creates files, checks quality, runs tests]

Claude: "Registration is implemented and all tests pass. The issue is marked complete."
```

### With Git Workflows

Integrate with standard git workflows:

```bash
# Create feature branch
git checkout -b issue/user-auth

# Work with SwissArmyHammer
"Create issue for user auth"
"Implement authentication"

# Track changes
"Show me what changed on this branch"

# Commit and push
git add .
git commit -m "Implement user authentication"
git push
```

### With CI/CD Pipelines

Use tools in automated workflows:

```yaml
# .github/workflows/quality.yml
- name: Check code quality
  run: sah rules_check --severity error

- name: Generate documentation
  run: sah outline_generate src/**/*.rs
```

## Performance Tips

### Optimize Semantic Search
- Index incrementally (default behavior)
- Use specific patterns instead of `**/*`
- Limit result counts appropriately
- Rebuild index if performance degrades

### Efficient File Operations
- Use `files_glob` to find files once
- Batch related edits together
- Use `offset` and `limit` for large files
- Avoid reading binary files unnecessarily

### Workflow Efficiency
- Define reusable workflows for common tasks
- Use parallel execution where possible
- Set appropriate timeouts
- Monitor and optimize slow steps

## Collaboration Patterns

### Team Knowledge Sharing
- Use memos for architectural decisions
- Document patterns and conventions
- Share issue tracking practices
- Maintain consistent naming conventions

### Code Review Workflow
1. Generate outline of changed files
2. Review with `git_changes`
3. Check rules for violations
4. Verify tests pass
5. Review memos for context

### Onboarding New Team Members
1. Share memo collection for context
2. Generate code outlines for understanding
3. Create example issues for practice
4. Document team conventions in memos

## Next Steps

- [Features Overview](./features.md): Detailed feature documentation
- [Examples](./examples.md): Concrete examples and tutorials
- [Architecture](./architecture.md): System design and internals
- [Troubleshooting](./troubleshooting.md): Solutions to common problems
