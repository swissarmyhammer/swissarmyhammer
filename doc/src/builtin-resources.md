# Built-in Resources

SwissArmyHammer includes production-ready prompts and workflows embedded in the binary. These are immediately available after installation.

## Built-in Prompts

### Code Quality

#### `code`
General code analysis and suggestions.
```bash
sah prompt test code --var language=rust --var context="authentication module"
```

#### `review/code`
Comprehensive code review with quality checklist.
```bash
sah prompt test review/code --var author="developer" --var files="src/auth.rs"
```

#### `review/security`
Security-focused code review.
```bash
sah prompt test review/security --var component="payment processing"
```

#### `review/accessibility`
Accessibility review for user interfaces.
```bash
sah prompt test review/accessibility --var interface="login form"
```

#### `review/patterns`
Review code patterns and architectural decisions.
```bash
sah prompt test review/patterns --var pattern="repository pattern"
```

#### `test`
Test generation and validation strategies.
```bash
sah prompt test test --var function="user_authentication" --var language=rust
```

#### `coverage`
Code coverage analysis and improvement suggestions.
```bash
sah prompt test coverage --var module="user_service"
```

### Documentation

#### `documentation`
General documentation generation with Liquid templating.
```bash
sah prompt test documentation --var project="MyApp" --var type="API"
```

#### `docs/readme`
Generate README files for projects.
```bash
sah prompt test docs/readme --var project="SwissArmyHammer"
```

#### `docs/comments`
Generate inline code documentation.
```bash
sah prompt test docs/comments --var language=rust --var function="process_user_input"
```

#### `docs/project`
Comprehensive project documentation.
```bash
sah prompt test docs/project --var name="MyProject" --var language=python
```

#### `docs/review`
Review and improve existing documentation.
```bash
sah prompt test docs/review --var document="API documentation"
```

#### `docs/correct`
Fix documentation errors and inconsistencies.
```bash
sah prompt test docs/correct --var section="installation guide"
```

### Development Process

#### `plan`
Project and feature planning assistance.
```bash
sah prompt test plan --var feature="user dashboard" --var scope="MVP"
```

#### `principals`
Development principles and best practices guidance.
```bash
sah prompt test principals --var language=rust --var domain="web backend"
```

#### `standards`
Coding standards enforcement and guidance.
```bash
sah prompt test standards --var team_size="5" --var language=typescript
```

#### `coding_standards`
Liquid-templated coding standards.
```bash
sah prompt test coding_standards --var language=python --var framework=django
```

#### `review_format`
Structured review format templates.
```bash
sah prompt test review_format --var type="architecture" --var scope="microservices"
```

### Debugging and Analysis

#### `debug/error`
Error analysis and debugging assistance.
```bash
sah prompt test debug/error --var error_message="connection timeout" --var context="database"
```

#### `debug/logs`
Log analysis and interpretation.
```bash
sah prompt test debug/logs --var log_level="ERROR" --var service="payment_service"
```

### Issue Management

#### `issue/code`
Code-related issue analysis and resolution.
```bash
sah prompt test issue/code --var issue="memory leak" --var language=rust
```

#### `issue/code_review`
Code review issue handling.
```bash
sah prompt test issue/code_review --var reviewer="senior_dev" --var priority="high"
```

#### `issue/review`
General issue review and triage.
```bash
sah prompt test issue/review --var type="bug" --var severity="critical"
```

#### `issue/complete`
Issue completion and closure procedures.
```bash
sah prompt test issue/complete --var issue_id="PROJ-123" --var resolution="fixed"
```

#### `issue/merge`
Issue merge and integration procedures.
```bash
sah prompt test issue/merge --var branch="feature/auth" --var target="develop"
```

#### `issue/on_worktree`
Issue workflow for worktree-based development.
```bash
sah prompt test issue/on_worktree --var worktree="feature-branch"
```

#### `issue/review_for_placeholders`
Review issues for placeholder content.
```bash
sah prompt test issue/review_for_placeholders --var component="user interface"
```

### Workflow Management

#### `todo`
TODO list generation and task management.
```bash
sah prompt test todo --var project="web_app" --var milestone="v1.0"
```

#### `commit`
Commit message generation and formatting.
```bash
sah prompt test commit --var changes="authentication fixes" --var type="bugfix"
```

#### `empty`
Empty template for custom prompts.
```bash
sah prompt test empty --var context="custom_use_case"
```

### Utility Prompts

#### `help`
General help and guidance.
```bash
sah prompt test help --var topic="workflow setup"
```

#### `example`
Example prompt demonstrating basic usage.
```bash
sah prompt test example --var name="test_prompt"
```

#### `say-hello`
Simple greeting prompt for testing.
```bash
sah prompt test say-hello --var name="World"
```

#### `abort`
Workflow abort and termination procedures.
```bash
sah prompt test abort --var reason="user_requested" --var workflow="deployment"
```

### Status Check Prompts

#### `are_issues_complete`
Check if all issues are completed.
```bash
sah prompt test are_issues_complete --var project="current"
```

#### `are_reviews_done`
Verify all reviews are completed.
```bash
sah prompt test are_reviews_done --var milestone="release_1.0"
```

#### `are_tests_passing`
Check test suite status.
```bash
sah prompt test are_tests_passing --var suite="integration"
```

### Meta-Prompts

#### `prompts/create`
Create new prompts programmatically.
```bash
sah prompt test prompts/create --var purpose="API documentation" --var domain="fintech"
```

#### `prompts/improve`
Improve existing prompts.
```bash
sah prompt test prompts/improve --var prompt_name="code_review" --var issue="too_verbose"
```

## Built-in Workflows

### Basic Examples

#### `hello-world`
Simple workflow demonstrating basic state transitions.
```bash
sah flow run hello-world
```
**States**: greeting → farewell → complete

#### `greeting`
Interactive greeting workflow.
```bash
sah flow run greeting --var name="Developer"
```
**States**: welcome → personalize → complete

#### `example-actions`
Demonstrates different action types (shell, prompt, conditional).
```bash
sah flow run example-actions
```
**States**: setup → execute → validate → complete

### Development Workflows

#### `tdd`
Test-driven development workflow.
```bash
sah flow run tdd --var feature="user_login" --var language="rust"
```
**States**: write_test → run_test → implement → refactor → complete

#### `implement`
General feature implementation workflow.
```bash
sah flow run implement --var feature="payment_processing"
```
**States**: plan → code → test → review → complete

#### `plan`
Planning and design workflow.
```bash
sah flow run plan --var scope="user_dashboard" --var timeline="2_weeks"
```
**States**: requirements → architecture → tasks → review → complete

### Issue Management Workflows

#### `code_issue`
End-to-end issue resolution workflow.
```bash
sah flow run code_issue --var issue_type="bug" --var priority="high"
```
**States**: triage → investigate → fix → test → review → complete

#### `do_issue`
Execute work on an existing issue.
```bash
sah flow run do_issue --var issue_id="PROJ-123"
```
**States**: start_work → implement → test → submit → complete

#### `complete_issue`
Issue completion and cleanup workflow.
```bash
sah flow run complete_issue --var issue_id="PROJ-456"
```
**States**: final_review → merge → cleanup → document → complete

#### `review_issue`
Issue review and validation workflow.
```bash
sah flow run review_issue --var issue_id="PROJ-789" --var reviewer="tech_lead"
```
**States**: review_code → test_changes → approve → complete

### Documentation Workflows

#### `document`
Documentation generation workflow.
```bash
sah flow run document --var type="API" --var format="markdown"
```
**States**: outline → draft → review → publish → complete

#### `review_docs`
Documentation review and quality check.
```bash
sah flow run review_docs --var document="user_guide"
```
**States**: content_review → format_check → accuracy_check → approve → complete

## Using Built-in Resources

### List Available Resources
```bash
# List all prompts (including built-in)
sah prompt list

# List all workflows  
sah flow list

# Filter for built-in only
sah prompt list --builtin
sah flow list --builtin
```

### Test Before Using
```bash
# Test a prompt with variables
sah prompt test code --var language=rust --var context="auth module"

# Validate prompt syntax
sah prompt validate code

# Render without executing
sah prompt render documentation --var project=MyApp
```

### Workflow Execution
```bash
# Run a workflow
sah flow run tdd --var feature=login --var language=python

# Check workflow status
sah flow status

# View workflow history
sah flow history

# Stop a running workflow
sah flow stop workflow_id
```

### Customization

You can override built-in resources by creating files with the same name in your user or local directories:

```bash
# Override built-in 'code' prompt
cp ~/.swissarmyhammer/prompts/code.md ~/.swissarmyhammer/prompts/code.md
# Edit the file to customize

# Create project-specific override
mkdir -p .swissarmyhammer/prompts
cp ~/.swissarmyhammer/prompts/team-review.md .swissarmyhammer/prompts/code.md
# Customize for project needs
```

**Precedence Order**:
1. Local directory (`.swissarmyhammer/`)
2. User directory (`~/.swissarmyhammer/`)
3. Built-in resources (embedded)

## Integration Examples

### Claude Code Usage
Built-in prompts are automatically available in Claude Code:
```bash
# Configure MCP
claude mcp add sah sah serve

# Use in Claude Code
/code language="typescript" context="React component"
/plan feature="user authentication" scope="MVP"
/review/security component="payment processing"
```

### Workflow Automation
```bash
# Chain workflows together
sah flow run plan --var project=MyApp && \
sah flow run tdd --var feature=auth && \
sah flow run document --var type=API
```

### Custom Integration
```bash
# Use prompts in scripts
REVIEW_OUTPUT=$(sah prompt render review/code --var author="$USER" --var files="$CHANGED_FILES")
echo "$REVIEW_OUTPUT" | mail -s "Code Review" team@company.com

# Integrate with CI/CD
sah flow run code_issue --var issue_type=ci_failure --var build_id="$BUILD_ID"
```

These built-in resources provide a solid foundation for development workflows. You can use them as-is or customize them for your specific needs.