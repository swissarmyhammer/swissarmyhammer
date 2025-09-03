# Workflows

Workflows enable complex, multi-step AI interactions with state management, conditional logic, and parallel execution.

## Overview

SwissArmyHammer workflows are state machines defined in markdown files that can:

- Execute sequences of prompts and shell commands
- Handle conditional branching based on results
- Run actions in parallel for efficiency
- Manage state transitions with validation
- Integrate with git for automated development workflows

## Workflow Structure

Workflows are markdown files with YAML front matter:

```markdown
---
name: code-review-workflow
description: Complete code review process with automated checks
version: "1.0"
initial_state: setup
timeout_ms: 300000
variables:
  - name: project_type
    description: Type of project being reviewed
    default: "web"
  - name: strict_mode
    description: Enable strict review mode
    type: boolean
    default: false
---

# Code Review Workflow

This workflow performs a comprehensive code review process.

## States

### setup
**Description**: Initialize the review process

**Actions:**
- shell: `git status`
- prompt: Use 'code' prompt to get initial assessment
- conditional: Check if tests exist

**Transitions:**
- If tests found → `run-tests`
- If no tests → `static-analysis`

### run-tests
**Description**: Execute the test suite

**Actions:**
- shell: `npm test` (parallel with coverage)
- shell: `cargo test` (if Rust project)

**Transitions:**
- If tests pass → `static-analysis`
- If tests fail → `fix-tests`

### static-analysis
**Description**: Perform static code analysis

**Actions:**
- shell: `cargo clippy` (if Rust)
- shell: `eslint .` (if JavaScript/TypeScript)
- prompt: Use 'review' prompt for manual analysis

**Transitions:**
- Always → `generate-report`

### generate-report
**Description**: Create comprehensive review report

**Actions:**
- prompt: Use 'documentation' prompt to generate report
- shell: `git add review-report.md`

**Transitions:**
- Always → `complete`

### complete
**Description**: Review process completed
```

## Front Matter Reference

### Required Fields

| Field | Description | Example |
|-------|-------------|---------|
| `name` | Unique workflow identifier | `"deploy-process"` |
| `description` | What the workflow does | `"Deploy application to production"` |
| `initial_state` | Starting state name | `"validate"` |

### Optional Fields

| Field | Description | Default |
|-------|-------------|---------|
| `version` | Workflow version | `"1.0"` |
| `timeout_ms` | Overall timeout | `300000` (5 min) |
| `max_parallel` | Max parallel actions | `4` |
| `on_error` | Error handling state | `"error"` |
| `on_timeout` | Timeout handling state | `"timeout"` |

### Parameters

Define structured parameters with type safety and validation:

```yaml
parameters:
  - name: environment
    description: Deployment environment
    type: choice
    choices: [dev, staging, prod]
    default: dev
    required: true
    
  - name: skip_tests
    description: Skip test execution
    type: boolean
    default: false
    
  - name: replicas
    description: Number of service replicas
    type: number
    default: 3
    validation:
      min: 1
      max: 10
```

**Parameter Features:**
- **Type Safety**: String, boolean, number, choice, and multi-choice types
- **Validation**: Pattern matching, ranges, string lengths, and custom rules
- **CLI Integration**: Automatic CLI switch generation (`--environment prod`)
- **Interactive Prompting**: User-friendly prompts for missing parameters
- **Conditional Parameters**: Parameters required based on other values
- **Parameter Groups**: Organize related parameters for better UX

For comprehensive parameter documentation, see [Workflow Parameters](workflow-parameters.md).

#### Legacy Variables (Deprecated)

The older `variables` syntax is still supported but deprecated:

```yaml
variables:
  - name: environment
    description: Deployment environment
    type: string
    default: "dev"
```

**Migration:** Convert `variables` to `parameters` for enhanced features. See the [Migration Guide](examples/workflow-parameters/migration-guide.md).

## State Definitions

States define the workflow steps and transitions.

### State Structure

```markdown
### state-name
**Description**: What this state does

**Actions:**
- action-type: action-specification
- action-type: action-specification (parallel)

**Transitions:**
- condition → target-state
- condition → target-state
- Always → default-state

**Error Handling:**
- On error → error-state
- On timeout → timeout-state
```

### Action Types

#### Prompt Actions

Execute prompts with Claude:

```markdown
**Actions:**
- Execute prompt "prompt-name"
- Execute prompt "prompt-name" with result="variable_name"
- Execute prompt "prompt-name" with arg1="value1" arg2="value2"
```

#### Shell Actions

Run shell commands:

```markdown
**Actions:**
- Run `git status`
- Run `npm test -- --coverage`
- Run `cargo build --release`
```

#### Log Actions

Output messages with different levels:

```markdown
**Actions:**
- Log "information message"
- Log warning "warning message"
- Log error "error message"
```

#### Set Variable Actions

Set workflow variables:

```markdown
**Actions:**
- Set variable_name="value"
- Set result="${previous_result}"
```

#### Wait Actions

Add delays or wait for user input:

```markdown
**Actions:**
- Wait 5 seconds
- Wait 2 minutes
- Wait for user input
```

#### Sub-workflow Actions

Run other workflows:

```markdown
**Actions:**
- Run workflow "workflow-name"
- Delegate to "workflow-name"
```

#### Abort Actions

Terminate workflow execution:

```markdown
**Actions:**
- Abort "Reason for termination"
```

### Transition Conditions

#### Simple Conditions

```markdown
**Transitions:**
- Always → next-state
- On success → success-state
- On failure → error-state
- On timeout → timeout-state
```

#### Variable-Based Conditions

```markdown
**Transitions:**
- If environment == "prod" → production-deploy
- If skip_tests == true → deploy
- If test_results.failed > 0 → fix-issues
```

#### Command Result Conditions

```markdown
**Transitions:**
- If last_command.exit_code == 0 → success
- If last_command.output contains "error" → handle-error
- If file_exists("target/release/app") → deploy
```

#### Complex Conditions

```markdown
**Transitions:**
- If (environment == "prod" AND test_results.passed == true) → deploy
- If (file_changed("Cargo.toml") OR dependencies_updated) → rebuild
```

## Execution Model

### Sequential Execution

Default behavior - actions run one after another:

```markdown
**Actions:**
- shell: `cargo build`
- shell: `cargo test`
- prompt: code-review
```

### Sequential Execution

Actions in workflows run sequentially by default. Each action completes before the next one begins.

```markdown
**Actions:**
- Log "Starting build process"
- Run `cargo build`
- Run `cargo test`
- Log "Build complete"
```

## Built-in Variables

SwissArmyHammer provides built-in variables:

| Variable | Description | Example |
|----------|-------------|---------|
| `workflow.name` | Current workflow name | `"deploy-process"` |
| `workflow.version` | Workflow version | `"1.0"` |
| `state.current` | Current state name | `"build"` |
| `state.previous` | Previous state name | `"test"` |
| `execution.start_time` | Workflow start time | `"2024-01-15T10:30:00Z"` |
| `execution.elapsed_ms` | Elapsed time | `45000` |
| `git.branch` | Current git branch | `"feature/auth"` |
| `git.commit` | Current commit hash | `"a1b2c3d"` |
| `env.*` | Environment variables | `env.NODE_ENV` |
| `last_command.exit_code` | Last shell command exit code | `0` |
| `last_command.output` | Last shell command output | `"Tests passed"` |
| `last_prompt.result` | Last prompt result | `"Code looks good"` |

## Error Handling

### Error States

Define dedicated error handling states:

```markdown
### error
**Description**: Handle errors and cleanup

**Actions:**
- Log error "Error occurred: {{error.message}} in state {{state.current}}"
- Run `git checkout main`
- Run `rm -rf temp/`

**Transitions:**
- If error.recoverable → retry-state
- Always → failed
```

### Retry Logic

```markdown
### flaky-operation
**Actions:**
- Run `network-dependent-command`

**Transitions:**
- On success → next-state
- On failure → error
```

### Cleanup Actions

```markdown
### cleanup
**Description**: Cleanup resources

**Actions:**
- Run `docker stop $(docker ps -q)`
- Run `rm -rf temp/`
- Log "Cleanup completed"
```

## Advanced Features

### Advanced Workflow Features

SwissArmyHammer workflows support sophisticated branching and state management through transitions. Complex conditional logic is handled through state transitions rather than within individual actions.

```markdown
---
name: conditional-deploy
initial_state: check-environment
---

### check-environment
**Actions:**
- Log "Checking deployment environment"
- Set environment_checked="true"

**Transitions:**
- If environment == "prod" → production-deploy
- If environment == "staging" → staging-deploy
- Always → development-deploy
```

## Integration Patterns

### Git Integration

```markdown
### git-workflow
**Actions:**
- Run `git checkout -b feature/{{issue_name}}`
- Run `git add -A`
- Run `git commit -m "{{commit_message}}"`

**Transitions:**
- On success → push-branch
```

### CI/CD Integration

```markdown
### ci-workflow
**Actions:**
- Run `docker build -t app:{{git.commit}} .`
- Run `docker push app:{{git.commit}}`
- Run `kubectl set image deployment/app app=app:{{git.commit}}`
- Log "Deployment completed for commit {{git.commit}}"
```

## Testing Workflows

### Validation

```bash
# Validate workflow syntax
sah flow validate my-workflow

# Check for cycles
sah flow validate my-workflow --check-cycles

# Validate all workflows
sah validate --workflows
```

### Dry Run

```bash
# See execution plan without running
sah flow run my-workflow --dry-run

# Show state diagram
sah flow show my-workflow --diagram
```

### Unit Testing

```markdown
---
name: test-workflow
description: Test the main workflow
test_mode: true
---

### test-setup
**Actions:**
- Run `mkdir -p test-temp`
- Run `cp test-data/* test-temp/`

### run-main-workflow
**Actions:**
- Run workflow "main-workflow"
    
### verify-results
**Actions:**
- Run `test -f test-temp/output.json`
- Log "Output file verification complete"
  
### cleanup
**Actions:**
- Run `rm -rf test-temp`
- Log "Test cleanup completed"
```

## Best Practices

### Design Principles

1. **Single Responsibility**: Each state should have one clear purpose
2. **Idempotent Actions**: Actions should be safe to retry
3. **Error Recovery**: Always include error handling paths
4. **Resource Cleanup**: Clean up resources in error cases
5. **Clear Transitions**: Make state transitions obvious and documented

### Performance Optimization

```yaml
# Use parallel execution where possible
max_parallel: 4

# Set appropriate timeouts
timeout_ms: 300000

# Minimize state transitions
# Combine related actions in single states

# Cache expensive operations
variables:
  - name: build_cache_key
    value: "{{git.commit}}-{{file_hash('Cargo.toml')}}"
```

### Security Considerations

```yaml
# Limit allowed commands
allowed_commands: ["git", "cargo", "npm", "docker"]

# Validate inputs
variables:
  - name: branch_name
    pattern: "^[a-zA-Z0-9/_-]+$"

# Use secure credential handling
environment:
  - name: API_TOKEN
    from_env: true
    required: false
```

### Documentation

```markdown
# Workflow Title

**Purpose**: Clear description of what this workflow accomplishes

**Prerequisites**: 
- Git repository
- Node.js installed
- Docker available

**Usage**:
```bash
sah flow run my-workflow --var environment=prod
```

**Variables**:
- `environment`: Target environment (dev/staging/prod)
- `skip_tests`: Skip test execution (default: false)

**States Overview**:
1. **setup**: Initialize and validate prerequisites
2. **build**: Compile and build artifacts  
3. **test**: Run test suites
4. **deploy**: Deploy to target environment
5. **verify**: Verify deployment success
```

## Step-by-Step Workflow Tutorials

### Tutorial 1: Creating Your First Workflow

Let's create a simple issue management workflow from scratch.

#### Step 1: Create the Workflow File

Create `./workflows/issue-workflow.md`:

```bash
mkdir -p ./.swissarmyhammer/workflows
cd ./.swissarmyhammer/workflows
```

#### Step 2: Define Basic Structure

```markdown
---
name: issue-workflow
description: Simple issue creation and tracking workflow
version: "1.0"
initial_state: create_issue
timeout_ms: 60000
variables:
  - name: issue_title
    description: Title for the new issue
    required: true
    type: string
  - name: issue_type
    description: Type of issue
    default: "FEATURE"
    choices: ["FEATURE", "BUG", "TASK"]
---

# Issue Management Workflow

This workflow creates and tracks an issue through completion.

## States

### create_issue
**Description**: Create a new issue

**Actions**:
- Create issue with title and type

**Next**: work_on_issue

### work_on_issue  
**Description**: Switch to working on the issue

**Actions**:
- Start work on the created issue
- Create git branch

**Next**: complete_issue

### complete_issue
**Description**: Mark issue as complete

**Actions**:
- Mark issue as completed
- Merge git branch

**Next**: END
```

#### Step 3: Test the Workflow

```bash
# Run the workflow
sah flow run issue-workflow --var issue_title="Add user authentication" --var issue_type="FEATURE"

# Check workflow status
sah flow status <run_id>

# View workflow logs
sah flow logs <run_id>
```

#### Step 4: Understanding the Output

The workflow will:
1. Create a new issue: `FEATURE_001_add-user-authentication`
2. Switch to a git branch: `issue/FEATURE_001_add-user-authentication`
3. Mark the issue as completed when done
4. Merge the branch back to the source branch

#### Step 5: Customize for Your Needs

Add variables for more control:

```markdown
variables:
  - name: issue_title
    required: true
    type: string
  - name: issue_type
    default: "FEATURE"
    choices: ["FEATURE", "BUG", "TASK", "REFACTOR"]
  - name: assignee
    description: Person assigned to work on this issue
    type: string
    default: "unassigned"
  - name: priority
    description: Issue priority level
    default: "medium"
    choices: ["low", "medium", "high", "urgent"]
```

### Tutorial 2: Advanced Workflow with Conditional Logic

Let's create a comprehensive development workflow with branching logic.

#### Step 1: Create Development Workflow

Create `./workflows/development-workflow.md`:

```markdown
---
name: development-workflow
description: Complete development workflow with testing and deployment
version: "2.0"
initial_state: analyze_changes
timeout_ms: 1800000  # 30 minutes
variables:
  - name: feature_name
    description: Name of feature being developed
    required: true
    type: string
  - name: environment
    description: Target environment
    default: "staging"
    choices: ["development", "staging", "production"]
  - name: run_tests
    description: Whether to run automated tests
    type: boolean
    default: true
  - name: auto_deploy
    description: Automatically deploy if tests pass
    type: boolean
    default: false
---

# Development Workflow

Comprehensive development workflow with conditional logic.

## States

### analyze_changes
**Description**: Analyze what needs to be done

**Actions**:
- search: Find existing implementations
- memo: Create analysis memo

**Transitions**:
- If existing code found → design_enhancement
- If no existing code → create_from_scratch

### create_from_scratch
**Description**: Create new feature from scratch

**Actions**:
- issue: Create comprehensive issue
- branch: Create feature branch
- memo: Document approach

**Next**: implement

### design_enhancement  
**Description**: Design enhancement to existing code

**Actions**:
- memo: Document enhancement plan
- issue: Create focused issue
- branch: Create enhancement branch

**Next**: implement

### implement
**Description**: Implement the feature

**Actions**:
- memo: Update with implementation notes
- Conditional: If run_tests → run_tests, else → manual_review

### run_tests
**Description**: Execute automated test suite

**Actions**:
- shell: Run test commands
- Conditional: If tests pass AND auto_deploy → deploy, else → manual_review

### manual_review
**Description**: Manual review and decision point

**Actions**:
- memo: Create review checklist
- Wait for manual decision

**Transitions**:  
- If approved → deploy
- If needs_changes → implement
- If rejected → cleanup

### deploy
**Description**: Deploy to target environment

**Actions**:
- shell: Deploy commands based on environment
- memo: Record deployment details

**Next**: verify_deployment

### verify_deployment
**Description**: Verify deployment worked correctly

**Actions**:
- shell: Health checks
- memo: Record verification results

**Transitions**:
- If verification successful → complete
- If verification failed → rollback

### rollback
**Description**: Rollback failed deployment

**Actions**:
- shell: Rollback commands
- memo: Record rollback details

**Next**: manual_review

### complete
**Description**: Mark workflow as complete

**Actions**:
- issue: Mark as complete
- memo: Final summary
- branch: Merge and cleanup

**Next**: END

### cleanup
**Description**: Clean up after rejection

**Actions**:
- branch: Delete feature branch
- memo: Record cleanup actions

**Next**: END
```

#### Step 2: Understanding Advanced Features

**Conditional Transitions**: Based on previous action results
```markdown
**Transitions**:
- If tests pass → deploy  
- If tests fail → fix_issues
- If no tests → manual_review
```

**Environment-Specific Actions**: Different behavior per environment
```markdown
**Actions**:
- If environment == "production" → production_deploy_actions
- If environment == "staging" → staging_deploy_actions  
- else → development_deploy_actions
```

**Parallel Actions**: Run multiple actions simultaneously
```markdown
**Actions**:
- parallel:
  - shell: Run unit tests
  - shell: Run integration tests  
  - shell: Run security scans
```

#### Step 3: Running the Advanced Workflow

```bash
# Development environment
sah flow run development-workflow \
  --var feature_name="user-profile-page" \
  --var environment="development" \
  --var run_tests=true \
  --var auto_deploy=false

# Production deployment (with confirmation)  
sah flow run development-workflow \
  --var feature_name="user-profile-page" \
  --var environment="production" \
  --var run_tests=true \
  --var auto_deploy=false \
  --interactive
```

### Tutorial 3: Team Collaboration Workflow

Create a workflow that coordinates multiple team members.

#### Step 1: Create Team Workflow

Create `./workflows/team-collaboration.md`:

```markdown
---
name: team-collaboration-workflow
description: Workflow for coordinated team development
version: "1.5"
initial_state: plan_sprint
timeout_ms: 604800000  # 1 week
variables:
  - name: sprint_name
    description: Name of the sprint
    required: true
    type: string
  - name: team_members
    description: List of team members
    required: true
    type: array
  - name: features
    description: Features to implement this sprint
    required: true
    type: array
  - name: sprint_duration
    description: Sprint duration in days
    type: number
    default: 14
---

# Team Collaboration Workflow

Coordinates development across multiple team members.

## States

### plan_sprint
**Description**: Plan the sprint with the team

**Actions**:
- memo: Create sprint planning document
- For each feature in features:
  - issue: Create feature issue
  - assign: Auto-assign to team members (round-robin)

**Next**: daily_standups

### daily_standups
**Description**: Track daily progress

**Actions**:
- Every day for sprint_duration:
  - memo: Update daily standup notes
  - For each team member:
    - check: Issue progress
    - update: Status tracking

**Transitions**:
- If all issues complete → sprint_review
- If sprint_duration reached → sprint_review
- Continue → daily_standups

### sprint_review
**Description**: Review sprint results

**Actions**:
- memo: Create sprint review document
- For each completed issue:
  - review: Code review process
  - merge: Merge completed work
- For each incomplete issue:
  - analyze: Why not completed
  - decide: Move to next sprint or close

**Next**: retrospective

### retrospective
**Description**: Team retrospective

**Actions**:
- memo: Create retrospective document with:
  - What went well
  - What could be improved  
  - Action items for next sprint

**Next**: END
```

#### Step 2: Workflow Execution with Team Coordination

```bash
# Start team workflow
sah flow run team-collaboration-workflow \
  --var sprint_name="Q1-Sprint-3" \
  --var 'team_members=["alice", "bob", "charlie"]' \
  --var 'features=["user-auth", "payment-gateway", "dashboard"]' \
  --var sprint_duration=10

# Monitor progress
sah flow status <run_id> --watch

# Check team progress
sah issue list --format table
sah memo search "sprint standup"
```

### Tutorial 4: CI/CD Integration Workflow

Integrate workflows with CI/CD systems.

#### Step 1: Create CI/CD Workflow

Create `./workflows/cicd-integration.md`:

```markdown
---
name: cicd-integration-workflow  
description: Integrates with CI/CD pipeline for automated deployments
version: "2.1"
initial_state: trigger_build
timeout_ms: 2700000  # 45 minutes
variables:
  - name: git_branch
    description: Git branch to build
    required: true
    type: string
  - name: build_type
    description: Type of build to create
    default: "release"
    choices: ["debug", "release", "test"]
  - name: deploy_targets
    description: Deployment targets
    type: array
    default: ["staging"]
  - name: slack_webhook
    description: Slack webhook for notifications
    type: string
    load_from_env: "SLACK_WEBHOOK_URL"
---

# CI/CD Integration Workflow

Coordinates with external CI/CD systems.

## States

### trigger_build
**Description**: Trigger the CI/CD build

**Actions**:
- shell: `git checkout {{git_branch}}`
- shell: `git pull origin {{git_branch}}`
- api_call: Trigger CI build via API
- notification: Send build start notification

**Transitions**:
- If build triggered → wait_for_build
- If trigger failed → build_failed

### wait_for_build
**Description**: Wait for build completion

**Actions**:
- poll: Check build status every 30 seconds
- timeout: 1800 seconds (30 minutes)

**Transitions**:
- If build successful → run_tests
- If build failed → build_failed  
- If timeout → build_timeout

### run_tests
**Description**: Execute test suite

**Actions**:
- parallel:
  - shell: `npm run test:unit`
  - shell: `npm run test:integration`  
  - shell: `npm run test:e2e`
  - shell: `npm run test:security`
- collect: Test results and coverage

**Transitions**:
- If all tests pass → deploy_staging
- If any test fails → test_failed

### deploy_staging
**Description**: Deploy to staging environment

**Actions**:
- shell: Deploy to staging
- shell: Run smoke tests
- notification: Notify team of staging deployment

**Transitions**:
- If "production" in deploy_targets → deploy_production
- else → deployment_complete

### deploy_production
**Description**: Deploy to production (with approval)

**Actions**:
- require_approval: Manual approval required
- shell: Deploy to production with blue-green deployment
- shell: Run production smoke tests
- notification: Notify team of production deployment

**Next**: deployment_complete

### deployment_complete
**Description**: Finalize deployment

**Actions**:
- memo: Record deployment details
- notification: Send success notification
- cleanup: Clean up temporary resources

**Next**: END

### build_failed
**Description**: Handle build failure

**Actions**:
- memo: Record build failure details
- notification: Send failure notification to team
- analysis: Analyze build logs for common issues

**Next**: END

### test_failed
**Description**: Handle test failure

**Actions**:
- memo: Record test failure details
- notification: Send test failure notification
- shell: Generate test report

**Next**: END

### build_timeout
**Description**: Handle build timeout

**Actions**:
- memo: Record timeout details
- notification: Send timeout notification
- shell: Cancel running build

**Next**: END
```

#### Step 2: Integration with External Systems

**GitHub Actions Integration**:
```yaml
# .github/workflows/swissarmyhammer.yml
name: SwissArmyHammer Workflow
on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  run-workflow:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install SwissArmyHammer
        run: cargo install swissarmyhammer
      - name: Run CI/CD Workflow
        run: |
          sah flow run cicd-integration-workflow \
            --var git_branch="${GITHUB_REF_NAME}" \
            --var build_type="release" \
            --var 'deploy_targets=["staging"]'
        env:
          SLACK_WEBHOOK_URL: ${{ secrets.SLACK_WEBHOOK_URL }}
```

### Tutorial 5: Debugging and Troubleshooting Workflows

Learn to debug workflows effectively.

#### Step 1: Enable Debug Mode

```bash
# Run workflow with debug output
sah flow run my-workflow --debug --var param="value"

# Enable trace logging
RUST_LOG=swissarmyhammer::workflow=trace sah flow run my-workflow

# Step through workflow interactively
sah flow run my-workflow --interactive --var param="value"
```

#### Step 2: Common Issues and Solutions

**Issue**: Workflow gets stuck in a state
```bash
# Check current state
sah flow status <run_id>

# View detailed logs
sah flow logs <run_id> --follow

# Manual state transition (emergency)
sah flow transition <run_id> --to-state "next_state"
```

**Issue**: Variables not resolving correctly
```markdown
# Add debug actions to your workflow
**Actions**:
- debug: Print all variables
- debug: Print specific variable values
- conditional: if debug_mode → detailed_logging
```

**Issue**: Actions failing silently
```bash
# Check action output
sah flow logs <run_id> --filter "action_result"

# Test actions individually
sah flow test my-workflow --dry-run --var param="value"
```

#### Step 3: Workflow Validation

```bash
# Validate workflow syntax
sah flow validate ./workflows/my-workflow.md

# Check for common issues
sah flow lint ./workflows/my-workflow.md

# Test workflow without execution
sah flow test ./workflows/my-workflow.md --dry-run
```

### Best Practices for Workflow Development

#### 1. Start Simple
- Begin with linear workflows
- Add complexity incrementally
- Test each addition thoroughly

#### 2. State Design Patterns
```markdown
# Good: Single responsibility states
### validate_input
**Description**: Validate all input parameters
**Actions**: validation_logic_only

### process_data  
**Description**: Process the validated data
**Actions**: processing_logic_only

# Avoid: Multi-purpose states
### validate_and_process
**Description**: Validate input and process data
**Actions**: too_much_complexity
```

#### 3. Error Handling
```markdown
# Every state should handle errors
### normal_operation
**Actions**:
- main_action
- on_error → error_handling_state

### error_handling_state
**Actions**:
- log_error
- notify_team
- cleanup_resources
**Transitions**:
- if recoverable → retry_state
- else → failed_state
```

#### 4. Resource Management
```markdown
# Always include cleanup
### resource_intensive_task
**Actions**:
- acquire_resources
- main_processing
- cleanup_resources (always runs)

### cleanup_state
**Actions**:
- release_connections
- delete_temporary_files
- update_status
```

#### 5. Testing Strategies

**Unit Test Individual States**:
```bash
# Test specific workflow state
sah flow test-state my-workflow setup --var param="value"
```

**Integration Testing**:
```bash
# Test complete workflow path
sah flow test my-workflow --path "setup→process→complete"
```

**Load Testing**:
```bash
# Run multiple workflow instances
for i in {1..10}; do
  sah flow run my-workflow --var id="$i" &
done
wait
```

### Workflow Monitoring and Metrics

#### Set up Monitoring
```bash
# Monitor all running workflows
sah flow monitor --dashboard

# Get workflow metrics
sah flow metrics --workflow my-workflow --period "last-week"

# Set up alerts
sah flow alert --on-failure --on-timeout --webhook "https://hooks.slack.com/..."
```

#### Performance Optimization
```markdown
# Profile workflow execution
---
enable_profiling: true
collect_metrics: true
---

# Optimize slow states
### slow_state
**Actions**:
- parallel: # Run actions in parallel where possible
  - action_1
  - action_2  
  - action_3
- cache: # Cache expensive computations
  key: "computation_result"
  action: expensive_computation
```

Workflows provide powerful automation capabilities while maintaining clarity and maintainability through their state machine design. These tutorials provide a solid foundation for creating sophisticated development automation.