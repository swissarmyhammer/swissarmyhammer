# Features Overview

SwissArmyHammer Tools provides 28+ tools organized into logical categories. Each tool is designed to work independently while also composing well with other tools for complex workflows.

## Tool Categories

### File Operations

Comprehensive file system access with security validation.

**Tools:**
- `files_read` - Read file contents with partial reading support
- `files_write` - Atomic file writes with encoding preservation
- `files_edit` - Precise string replacement editing
- `files_glob` - Pattern-based file discovery with gitignore support
- `files_grep` - Content search using ripgrep

**Use Cases:**
- Read configuration files
- Update source code
- Search codebases
- Find files by pattern
- Safe file modifications

**Learn more:** [File Operations](features/files.md)

### Semantic Search

Vector-based code search for intelligent code navigation.

**Tools:**
- `search_index` - Index files using tree-sitter and embeddings
- `search_query` - Semantic similarity search

**Use Cases:**
- Find code by meaning, not keywords
- Understand unfamiliar codebases
- Locate similar implementations
- Discover related functionality

**Learn more:** [Search Tools](features/search.md)

### Issue Management

Git-integrated issue tracking with markdown storage.

**Tools:**
- `issue_create` - Create work items with automatic branching
- `issue_list` - List issues with filtering
- `issue_show` - Display issue details
- `issue_update` - Update issue content
- `issue_mark_complete` - Complete and archive issues
- `issue_all_complete` - Check if all issues are done

**Use Cases:**
- Track feature development
- Manage bug fixes
- Plan refactoring work
- Coordinate tasks with AI assistants

**Learn more:** [Issue Management](features/issues.md)

### Memoranda System

Note-taking and knowledge management for AI context.

**Tools:**
- `memo_create` - Create notes with ULID identification
- `memo_get` - Retrieve specific memo
- `memo_list` - List all memos
- `memo_get_all_context` - Get aggregated context

**Use Cases:**
- Document decisions and rationale
- Capture project knowledge
- Provide context to AI assistants
- Share information across sessions

**Learn more:** [Memoranda System](features/memos.md)

### Todo Management

Ephemeral task tracking for development sessions.

**Tools:**
- `todo_create` - Create task items
- `todo_show` - Show specific or next todo
- `todo_mark_complete` - Complete tasks

**Use Cases:**
- Break down complex work
- Track session progress
- Plan implementation steps
- Manage temporary tasks

**Learn more:** [Todo Management](features/todos.md)

### Git Integration

Track file changes with branch detection and parent tracking.

**Tools:**
- `git_changes` - List changed files on branch

**Use Cases:**
- Understand scope of changes
- Prepare for code review
- Track feature development
- Identify modified files

**Learn more:** [Git Integration](features/git.md)

### Shell Execution

Execute commands with proper output handling.

**Tools:**
- `shell_execute` - Run shell commands with environment control

**Use Cases:**
- Run build commands
- Execute tests
- Interact with CLI tools
- Automate workflows

**Learn more:** [Shell Execution](features/shell.md)

### Code Outline

Generate structured code overviews using tree-sitter.

**Tools:**
- `outline_generate` - Extract symbols and structure

**Use Cases:**
- Understand code organization
- Generate documentation
- Analyze structure
- Navigate large files

**Learn more:** [Code Outline](features/outline.md)

### Rules Engine

Check code quality against defined standards.

**Tools:**
- `rules_check` - Validate code against rules

**Use Cases:**
- Enforce coding standards
- Identify violations
- Maintain consistency
- Quality assurance

**Learn more:** [Rules Engine](features/rules.md)

### Web Tools

Fetch and search web content with markdown conversion.

**Tools:**
- `web_fetch` - Fetch and convert HTML to markdown
- `web_search` - DuckDuckGo search integration

**Use Cases:**
- Research documentation
- Find examples
- Access web resources
- Search for solutions

**Learn more:** [Web Tools](features/web.md)

### Workflow Execution

Execute workflows with AI agent coordination.

**Tools:**
- `flow` - Run workflow definitions

**Use Cases:**
- Multi-step automation
- Complex task orchestration
- AI-driven workflows
- Process automation

**Learn more:** [Workflow Execution](features/workflows.md)

### Workflow Control

Signal workflow termination.

**Tools:**
- `abort_create` - Create abort signal file

**Use Cases:**
- Cancel running workflows
- Signal termination
- Error handling
- Emergency stops

## Common Patterns

### Pattern 1: Code Understanding

```text
1. search_index - Index the codebase
2. search_query - Find relevant code
3. files_read - Read specific files
4. outline_generate - Understand structure
```text

### Pattern 2: Feature Development

```text
1. issue_create - Create feature issue
2. memo_create - Document design decisions
3. files_edit - Implement changes
4. rules_check - Validate quality
5. issue_mark_complete - Close issue
```text

### Pattern 3: Code Review

```text
1. git_changes - See what changed
2. files_read - Review specific files
3. search_query - Find related code
4. rules_check - Check standards
5. memo_create - Document findings
```text

### Pattern 4: Documentation

```text
1. outline_generate - Extract structure
2. files_read - Read source comments
3. search_query - Find examples
4. files_write - Create documentation
```text

## Tool Composition

Tools are designed to work together. For example:

**Find and fix issues:**
```text
1. files_grep - Search for problematic pattern
2. files_read - Read files with issues
3. files_edit - Fix the problems
4. rules_check - Verify fixes are correct
```text

**Research and implement:**
```text
1. web_search - Find implementation approaches
2. web_fetch - Read documentation
3. memo_create - Document the approach
4. files_edit - Implement the solution
```text

## Next Steps

Explore individual tool categories for detailed documentation:

- **[File Operations](features/files.md)** - Complete file system access
- **[Search Tools](features/search.md)** - Semantic code search
- **[Issue Management](features/issues.md)** - Track work items
- **[Memoranda](features/memos.md)** - Knowledge management
- **[Todo Management](features/todos.md)** - Task tracking
- **[Git Integration](features/git.md)** - Change tracking
- **[Shell Execution](features/shell.md)** - Command execution
- **[Code Outline](features/outline.md)** - Structure analysis
- **[Rules Engine](features/rules.md)** - Quality checks
- **[Web Tools](features/web.md)** - Web access
- **[Workflows](features/workflows.md)** - Automation
