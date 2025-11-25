---
title: plan
description: Generate a step by step development plan from specification(s).
parameters:
  - name: plan_filename
    description: Path to the specific plan markdown file to process (optional)
    required: true
---

## Goal

Turn specification(s) into rules and todos.

Process the specific plan file: {{ plan_filename }}

Generate:
1. **Rules** - Permanent criteria defining what correct code looks like
2. **Todos** - Implementation steps with rich markdown context

Use the rule_create tool for acceptance criteria and the todo_create tool for implementation steps.


## Guidelines

### General
- DO NOT code at this step, we are just creating the plan
- DO make sure to review the existing codebase and architecture before creating the implementation plan
- DO make sure each todo is a single focused task
- DO create many, small, incremental todos. Ideally each step should result in less than 250 lines of code changed
- DO make sure that each todo builds on the previous steps
- DO NOT leave hanging or orphaned code that isn't integrated into a previous step
- DO NOT plan security features unless specifically asked by the user
- DO NOT plan performance features unless specifically asked by the user
- DO NOT plan backward compatibility features unless specifically asked by the user
- Each todo you create should include the phrase "Refer to {{ plan_filename }}" in its context
- Iterate until you feel that the steps are right sized for this project

### Creating Rules

Look for rules that need to be created from the specification.
It is best to think of these rules in a similar way to linting or static analysis rules that can be checked automatically. 
- Use the `rule_create` tool with parameters:
  - `name`: Path like "project-name/requirement-name" (will create `.swissarmyhammer/rules/project-name/requirement-name.md`)
  - `content`: The acceptance criteria in markdown - what must be true for this to be considered complete
  - `severity`: One of "error", "warning", "info", or "hint" (use "error" for must-have requirements)
  - `tags`: Optional array of tags for organization (e.g., ["api", "database"])
- Rules define WHAT success looks like, not HOW to implement it
- Rules are permanent and will be checked with `rules_check` tool
- Examples of good rules:
  - "API endpoint /users must exist and return 200 with valid user data"
  - "All public functions must have documentation comments"
  - "Database migrations must be reversible"
  - "All user inputs must be validated before processing"

### Creating Todos

For each implementation step:
- Use the `todo_create` tool with parameters:
  - `task`: Brief description of what needs to be done (e.g., "Implement user authentication endpoint")
  - `context`: Rich markdown content with implementation guidance
- The `context` field supports full markdown including:
  - **Mermaid diagrams** for architecture and flow
  - **Code examples** showing implementation patterns
  - **Multi-paragraph explanations** of the approach
  - **References** to the spec file and relevant rules
  - **Implementation notes** and considerations
- Break work into small, focused tasks that build incrementally
- Each todo should be completable in a single focused session
- Todos are ephemeral (auto-deleted after completion)

## Process

1. **Read and analyze** the specified plan file: {{ plan_filename }}

2. **Review context**:
   - Review the existing code to determine what parts of the specification might already be implemented
   - Unless explicitly instructed otherwise in the specification, do not add new systems/services when existing patterns and systems can be extended to achieve the goals

3. **Draft the plan**:
   - Draft a detailed, step-by-step plan to meet the specification
   - Write this out to a temp file `.swissarmyhammer/tmp/DRAFT_PLAN.md`
   - Refer to this draft plan to refresh your memory as you work

4. **Create rules**:
   - Identify new rules implied by the specification
   - For each criterion, use `rule_create` tool to create a permanent, executable rule
   - Use a consistent naming scheme like "spec-name/requirement-name"

5. **Create todos for implementation steps**:
   - Break the plan down into small, iterative chunks that build on each other incrementally
   - Review and make sure the steps are small enough to be implemented safely (< 250 lines each), but big enough to move the project forward
   - For each step, use `todo_create` tool with:
     - `task`: Brief description
     - `context`: Rich markdown with diagrams, examples, references to {{ plan_filename }}
   - Ensure todos build incrementally on each other
   - Include the phrase "Refer to {{ plan_filename }}" in each todo's context
