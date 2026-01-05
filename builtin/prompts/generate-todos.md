---
title: generate-todos
description: Generate todos (implementation steps) from a draft plan.
parameters:
  - name: plan_filename
    description: Path to the original specification file
    required: true
---

## Goal

Read the draft plan and create detailed, incremental todos for implementation.

Use the draft plan from: `.swissarmyhammer/tmp/DRAFT_PLAN.md`

Generate todos using the `todo_create` tool for each implementation step.

## Guidelines

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

- Each task needs a reference to the spec file: "Refer to {{ plan_filename }}"
- Break work into small, focused tasks that build incrementally
- Each todo should result in less than 250 lines of code changed
- Todos should build on each other incrementally
- No hanging or orphaned code - each step should integrate with previous work

## Process

1. **Read the draft plan** from `.swissarmyhammer/tmp/DRAFT_PLAN.md`

2. **Break down into steps**:
   - Identify the implementation steps from the plan
   - Ensure steps are small enough (< 250 lines each)
   - Ensure steps build incrementally
   - Ensure no orphaned code

3. **Create todos**:
   - For each step, use `todo_create` tool
   - Include rich context with:
     - What needs to be done
     - How it fits with previous steps
     - Relevant code examples or patterns
     - Reference to {{ plan_filename }}
     - Any important considerations

4. **Review and refine**:
   - Review the created todos
   - Make sure they're right-sized for the project
   - Make sure they build incrementally
   - Iterate if needed

## Output

Create todos using the `todo_create` tool for each implementation step from the draft plan.
