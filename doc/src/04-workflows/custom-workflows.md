# Custom Workflows

SwissArmyHammer's workflow system enables you to create sophisticated, state-driven automation sequences that combine multiple tools and actions. Custom workflows allow you to encode complex development processes, automate repetitive tasks, and ensure consistent execution of multi-step procedures.

## Overview

Custom workflows provide:
- **State-based execution**: Define workflows as state machines with transitions
- **Action composition**: Combine multiple actions in sequence or parallel
- **Conditional logic**: Branch execution based on conditions and variables
- **Error handling**: Robust error recovery and rollback mechanisms
- **Template integration**: Use Liquid templates throughout workflow definitions
- **External integrations**: Execute shell commands, API calls, and tool invocations

## Workflow Fundamentals

### Workflow Structure

Workflows are defined as Markdown files with YAML front matter:

```markdown
---
name: feature-development
description: Complete feature development workflow
initial_state: research
variables:
  feature_name: ""
  complexity: "medium"
---

# Feature Development Workflow

This workflow guides through the complete feature development process.

## States

### research
**Description**: Research existing implementations and patterns
**Actions**:
- Search codebase for related functionality
- Create research memo with findings
- Identify dependencies and requirements

**Transitions**:
- `found_examples` → `design`
- `no_examples` → `architecture_review`

### design
**Description**: Create detailed design specifications
**Actions**:
- Create design document
- Review with team
- Update issue with design details

**Transitions**:
- `approved` → `implement`
- `needs_revision` → `design`

### implement
**Description**: Implement the feature
**Actions**:
- Create feature branch
- Implement core functionality
- Add tests and documentation

**Transitions**:
- `complete` → `review`
- `blocked` → `research`
```

### Core Components

**States**: Discrete phases of workflow execution
```yaml
states:
  research:
    description: "Initial research and planning"
    actions:
      - type: search_query
        params:
          query: "{{ feature_name }} implementation"
      - type: create_memo
        params:
          title: "Research: {{ feature_name }}"
    transitions:
      complete: "design"
      blocked: "help_needed"
```

**Actions**: Individual operations within states
```yaml
actions:
  - type: shell_command
    params:
      command: "git checkout -b feature/{{ feature_name }}"
  - type: create_issue
    params:
      name: "{{ feature_name }}"
      content: "{{ issue_template }}"
```

**Transitions**: Movement between states based on conditions
```yaml
transitions:
  success: "next_state"
  failure: "error_handling"
  timeout: "manual_review"
```

## Built-in Actions

### Issue Management Actions

**Create Issue**:
```yaml
- type: create_issue
  params:
    name: "FEATURE_{{ timestamp }}_{{ feature_name | slugify }}"
    content: |
      # {{ feature_name }}
      
      ## Description
      {{ description }}
      
      ## Requirements
      {{ requirements }}
```

**Update Issue**:
```yaml
- type: update_issue
  params:
    name: "{{ current_issue }}"
    content: |
      ## Progress Update
      {{ progress_summary }}
    append: true
```

**Show Issue**:
```yaml
- type: issue_show
  params:
    name: "{{ issue_name }}"
```

### Memoranda Actions

**Create Memo**:
```yaml
- type: create_memo
  params:
    title: "{{ memo_title }}"
    content: |
      # {{ topic }}
      
      ## Key Points
      {{ key_points }}
      
      ## Action Items
      {{ action_items }}
```

**Search Memos**:
```yaml
- type: search_memos
  params:
    query: "{{ search_terms }}"
    limit: 5
  output_var: "related_memos"
```

### Search Actions

**Semantic Search**:
```yaml
- type: search_query
  params:
    query: "{{ feature_name }} {{ technology_stack }}"
    limit: 10
  output_var: "search_results"
```

**Index Files**:
```yaml
- type: search_index
  params:
    patterns:
      - "src/**/*.rs"
      - "lib/**/*.rs"
    force: false
```

### Shell Actions

**Execute Commands**:
```yaml
- type: shell_command
  params:
    command: "cargo test {{ test_pattern }}"
    timeout: 300
    working_directory: "{{ project_root }}"
  output_var: "test_results"
```

**Conditional Execution**:
```yaml
- type: shell_command
  params:
    command: "git push origin {{ branch_name }}"
    condition: "{{ auto_push == true }}"
```

### File Operations

**Read File**:
```yaml
- type: read_file
  params:
    path: "{{ config_file }}"
  output_var: "config_content"
```

**Write File**:
```yaml
- type: write_file
  params:
    path: "{{ output_file }}"
    content: |
      # Generated Configuration
      {{ config_template | render }}
```

### Template Actions

**Render Template**:
```yaml
- type: render_template
  params:
    template: "{{ template_name }}"
    context:
      project: "{{ project_name }}"
      author: "{{ author_name }}"
  output_var: "rendered_content"
```

## Advanced Workflow Patterns

### Parallel Execution

Execute multiple actions simultaneously:

```yaml
states:
  setup:
    description: "Parallel initialization tasks"
    actions:
      - type: parallel
        actions:
          - type: search_index
            params:
              patterns: ["**/*.rs"]
          - type: shell_command
            params:
              command: "cargo check"
          - type: create_memo
            params:
              title: "Setup Started"
              content: "Beginning feature setup..."
    transitions:
      complete: "development"
```

### Conditional Branching

Branch execution based on conditions:

```yaml
states:
  analysis:
    description: "Analyze codebase complexity"
    actions:
      - type: search_query
        params:
          query: "{{ feature_type }}"
        output_var: "existing_implementations"
      - type: conditional
        condition: "{{ existing_implementations | size > 5 }}"
        then:
          - type: set_variable
            params:
              complexity: "high"
        else:
          - type: set_variable
            params:
              complexity: "low"
    transitions:
      high_complexity: "architecture_review"
      low_complexity: "direct_implementation"
```

### Error Handling and Retry

Handle failures gracefully:

```yaml
states:
  build_and_test:
    description: "Build and run tests"
    actions:
      - type: shell_command
        params:
          command: "cargo build"
        retry:
          attempts: 3
          delay: 5
        on_error: "build_failed"
      - type: shell_command
        params:
          command: "cargo test"
        on_error: "test_failed"
    transitions:
      success: "deployment"
      build_failed: "dependency_check"
      test_failed: "fix_tests"
```

### Loop Constructs

Iterate over collections or repeat actions:

```yaml
states:
  process_files:
    description: "Process each source file"
    actions:
      - type: for_each
        collection: "{{ source_files }}"
        item_var: "current_file"
        actions:
          - type: shell_command
            params:
              command: "rustfmt {{ current_file }}"
          - type: create_memo
            params:
              title: "Processed {{ current_file }}"
    transitions:
      complete: "review_changes"
```

## Variable Management

### Variable Declaration

Define workflow variables with types and defaults:

```yaml
variables:
  feature_name:
    type: string
    required: true
    description: "Name of the feature to implement"
  
  complexity:
    type: enum
    values: ["low", "medium", "high"]
    default: "medium"
  
  auto_deploy:
    type: boolean
    default: false
  
  team_members:
    type: array
    default: []
  
  config:
    type: object
    default:
      timeout: 300
      retries: 3
```

### Variable Scoping

Variables have different scopes:

```yaml
# Global variables (available throughout workflow)
variables:
  project_name: "my-project"

# State-local variables
states:
  research:
    variables:
      search_terms: "{{ feature_name }} implementation"
    actions:
      - type: search_query
        params:
          query: "{{ search_terms }}"  # Uses local variable
```

### Dynamic Variable Assignment

Set variables during execution:

```yaml
actions:
  - type: shell_command
    params:
      command: "git rev-parse --short HEAD"
    output_var: "commit_hash"
  
  - type: set_variable
    params:
      branch_name: "feature/{{ feature_name }}-{{ commit_hash }}"
  
  - type: calculate
    expression: "{{ file_count * complexity_multiplier }}"
    output_var: "estimated_time"
```

## Template Integration

### Liquid Template Usage

Use Liquid templates throughout workflow definitions:

```yaml
actions:
  - type: create_issue
    params:
      name: "{{ issue_type | upcase }}_{{ '%03d' | sprintf: issue_number }}_{{ feature_name | slugify }}"
      content: |
        # {{ feature_name | title }}
        
        **Type**: {{ issue_type | capitalize }}
        **Priority**: {{ priority | default: "normal" }}
        **Assigned**: {{ assignee | default: "unassigned" }}
        
        ## Description
        {{ description | strip | default: "No description provided" }}
        
        ## Acceptance Criteria
        {% for criterion in acceptance_criteria %}
        - [ ] {{ criterion }}
        {% endfor %}
        
        {% if related_issues %}
        ## Related Issues
        {% for issue in related_issues %}
        - {{ issue }}
        {% endfor %}
        {% endif %}
```

### Custom Filters

Define workflow-specific filters:

```yaml
filters:
  slugify:
    description: "Convert string to URL-safe slug"
    implementation: |
      {{ input | downcase | replace: ' ', '-' | replace: '[^a-z0-9\-]', '' }}
  
  format_duration:
    description: "Format seconds as human-readable duration"
    implementation: |
      {% assign hours = input | divided_by: 3600 %}
      {% assign minutes = input | modulo: 3600 | divided_by: 60 %}
      {% if hours > 0 %}{{ hours }}h {% endif %}{{ minutes }}m
```

## Workflow Composition

### Subworkflows

Break complex workflows into reusable components:

```yaml
# main-workflow.md
states:
  setup:
    actions:
      - type: execute_workflow
        params:
          workflow: "setup-environment"
          variables:
            project_type: "rust"
    transitions:
      complete: "development"

# setup-environment.md
---
name: setup-environment
description: Environment setup workflow
---

states:
  initialize:
    actions:
      - type: shell_command
        params:
          command: "rustup update"
      - type: shell_command
        params:
          command: "cargo install cargo-edit"
```

### Workflow Inheritance

Extend base workflows with specific customizations:

```yaml
# base-development.md
---
name: base-development
description: Base development workflow
---

states:
  setup:
    actions:
      - type: create_issue
  implement:
    actions:
      - type: placeholder  # To be overridden
  finalize:
    actions:
      - type: issue_mark_complete

# rust-development.md
---
name: rust-development
extends: base-development
description: Rust-specific development workflow
---

states:
  implement:
    actions:
      - type: shell_command
        params:
          command: "cargo check"
      - type: shell_command
        params:
          command: "cargo test"
      - type: shell_command
        params:
          command: "cargo fmt"
```

## Testing and Debugging

### Workflow Testing

Test workflows in isolation:

```yaml
# test-workflow.md
---
name: test-feature-workflow
description: Test version of feature workflow
test_mode: true
---

# Override external dependencies for testing
test_overrides:
  shell_command:
    mock_output: "Success"
  create_issue:
    mock_response: { "name": "TEST_001", "id": "12345" }

states:
  test_setup:
    actions:
      - type: assert_variable
        params:
          variable: "feature_name"
          expected: "test-feature"
```

### Debug Mode

Enable detailed execution logging:

```bash
# Run workflow with debug output
sah workflow execute feature-development --debug --variables feature_name=user-auth

# Step through workflow interactively
sah workflow step feature-development --interactive
```

### Validation

Validate workflow definitions:

```bash
# Validate syntax and structure
sah workflow validate feature-development.md

# Check variable dependencies
sah workflow check --variables feature_name=test complexity=high
```

## Real-World Examples

### Complete Feature Development

```yaml
---
name: full-feature-development
description: Complete feature development lifecycle
initial_state: planning
variables:
  feature_name: { type: string, required: true }
  estimated_hours: { type: number, default: 8 }
---

states:
  planning:
    description: "Research and planning phase"
    actions:
      - type: search_query
        params:
          query: "{{ feature_name }} existing implementation"
        output_var: "existing_code"
      
      - type: create_memo
        params:
          title: "Feature Planning: {{ feature_name }}"
          content: |
            # {{ feature_name | title }} Planning
            
            ## Research Findings
            {{ existing_code | format_search_results }}
            
            ## Estimated Effort
            {{ estimated_hours }} hours
            
            ## Next Steps
            - Create detailed design
            - Break down into tasks
            - Begin implementation
      
      - type: create_issue
        params:
          name: "FEATURE_{{ '%03d' | sprintf: feature_counter }}_{{ feature_name | slugify }}"
          content: |
            # {{ feature_name | title }}
            
            ## Planning Complete
            See memo: {{ memo_id }}
            
            ## Estimated Effort
            {{ estimated_hours }} hours
        output_var: "feature_issue"
    
    transitions:
      ready: "implementation"
      needs_research: "deep_research"

  implementation:
    description: "Core implementation phase"
    actions:
      - type: shell_command
        params:
          command: "cargo check"
        output_var: "check_result"
      
      - type: conditional
        condition: "{{ check_result.exit_code != 0 }}"
        then:
          - type: update_issue
            params:
              name: "{{ feature_issue.name }}"
              content: "\n## Build Issues\n{{ check_result.stderr }}"
              append: true
        else:
          - type: shell_command
            params:
              command: "cargo test"
            output_var: "test_result"
    
    transitions:
      tests_pass: "documentation"
      tests_fail: "fix_tests"
      build_fail: "fix_build"

  documentation:
    description: "Add documentation and examples"
    actions:
      - type: create_memo
        params:
          title: "{{ feature_name }} Documentation"
          content: |
            # {{ feature_name | title }} Implementation
            
            ## Overview
            {{ feature_description }}
            
            ## API Documentation
            {{ api_docs | generate_from_code }}
            
            ## Usage Examples
            {{ usage_examples }}
      
      - type: shell_command
        params:
          command: "cargo doc"
    
    transitions:
      complete: "review"

  review:
    description: "Code review and finalization"
    actions:
      - type: shell_command
        params:
          command: "git add -A && git commit -m 'Complete {{ feature_name }} implementation'"
      
      - type: update_issue
        params:
          name: "{{ feature_issue.name }}"
          content: |
            
            ## Implementation Complete
            - ✅ Core functionality implemented
            - ✅ Tests passing
            - ✅ Documentation added
            - ✅ Ready for review
          append: true
      
      - type: issue_mark_complete
        params:
          name: "{{ feature_issue.name }}"
    
    transitions:
      complete: "finalize"
      needs_changes: "implementation"

  finalize:
    description: "Finalize completed feature"
    actions:
      - type: create_memo
        params:
          title: "Feature Complete: {{ feature_name }}"
          content: |
            # {{ feature_name | title }} - Complete
            
            **Status**: ✅ Merged and deployed
            **Time Spent**: {{ actual_hours }} hours
            **Commits**: {{ commit_count }}
            
            ## Lessons Learned
            {{ lessons_learned }}
```

### Bug Fix Workflow

```yaml
---
name: bug-fix-workflow
description: Systematic bug fixing process
initial_state: reproduce
variables:
  bug_description: { type: string, required: true }
  severity: { type: enum, values: ["low", "medium", "high", "critical"], default: "medium" }
---

states:
  reproduce:
    description: "Reproduce and understand the bug"
    actions:
      - type: create_issue
        params:
          name: "BUG_{{ timestamp }}_{{ bug_description | slugify }}"
          content: |
            # Bug: {{ bug_description }}
            
            **Severity**: {{ severity }}
            **Reported**: {{ timestamp | date: '%Y-%m-%d %H:%M' }}
            
            ## Description
            {{ bug_description }}
            
            ## Reproduction Steps
            - [ ] Step 1
            - [ ] Step 2
            - [ ] Step 3
            
            ## Expected Behavior
            
            ## Actual Behavior
            
            ## Environment
            - OS: {{ os_info }}
            - Version: {{ app_version }}
        output_var: "bug_issue"
      
      - type: search_query
        params:
          query: "{{ bug_description }} error handling"
        output_var: "related_code"
    
    transitions:
      reproduced: "investigate"
      cannot_reproduce: "needs_info"

  investigate:
    description: "Investigate root cause"
    actions:
      - type: create_memo
        params:
          title: "Bug Investigation: {{ bug_description }}"
          content: |
            # Bug Analysis
            
            ## Related Code
            {{ related_code | format_search_results }}
            
            ## Hypothesis
            {{ investigation_notes }}
            
            ## Potential Fixes
            {{ fix_options }}
    
    transitions:
      root_cause_found: "fix"
      need_more_info: "reproduce"

  fix:
    description: "Implement and test fix"
    actions:
      - type: shell_command
        params:
          command: "cargo test {{ test_pattern }}"
        output_var: "pre_fix_tests"
      
      - type: conditional
        condition: "{{ pre_fix_tests.exit_code == 0 }}"
        then:
          - type: update_issue
            params:
              name: "{{ bug_issue.name }}"
              content: "\n## Fix Applied\n{{ fix_description }}\n"
              append: true
          
          - type: shell_command
            params:
              command: "cargo test"
            output_var: "post_fix_tests"
    
    transitions:
      tests_pass: "validate"
      tests_fail: "investigate"

  validate:
    description: "Validate fix and prepare for merge"
    actions:
      - type: shell_command
        params:
          command: "cargo build --release"
      
      - type: update_issue
        params:
          name: "{{ bug_issue.name }}"
          content: |
            
            ## Fix Validated
            - ✅ Tests passing
            - ✅ Build successful
            - ✅ Manual testing complete
          append: true
      
      - type: issue_mark_complete
        params:
          name: "{{ bug_issue.name }}"
    
    transitions:
      validated: "finalize"

  finalize:
    description: "Finalize bug fix"
    actions:
      - type: create_memo
        params:
          title: "Bug Fixed: {{ bug_description }}"
          content: |
            # Bug Fix Complete
            
            **Issue**: {{ bug_issue.name }}
            **Resolution Time**: {{ resolution_time }}
            
            ## Root Cause
            {{ root_cause }}
            
            ## Fix Description
            {{ fix_description }}
            
            ## Prevention
            {{ prevention_measures }}
```

## Best Practices

### Workflow Design

**State Granularity**:
- Keep states focused on single responsibilities
- Use meaningful state names that describe the current phase
- Avoid overly complex states with too many actions

**Error Handling**:
- Define clear error transitions for each state
- Implement retry logic for transient failures  
- Provide rollback mechanisms for critical operations

**Variable Management**:
- Use descriptive variable names
- Provide sensible defaults where possible
- Document variable purposes and types
- Validate inputs early in the workflow

### Testing Strategy

**Unit Testing**:
- Test individual actions in isolation
- Mock external dependencies  
- Verify variable transformations
- Test error conditions

**Integration Testing**:
- Test complete workflow execution
- Verify state transitions work correctly
- Test with realistic data and scenarios
- Validate external integrations

### Documentation

**Inline Documentation**:
- Document the purpose of each state
- Explain complex conditions and transitions
- Provide examples of variable usage
- Include troubleshooting notes

**User Guides**:
- Create step-by-step execution guides
- Document required variables and setup
- Provide examples of different execution scenarios
- Include troubleshooting common issues

Custom workflows transform SwissArmyHammer into a powerful automation platform, enabling you to codify complex development processes and ensure consistent execution across your team.