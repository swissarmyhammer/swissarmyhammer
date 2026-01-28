---
title: create-draft-plan
description: Generate a detailed draft implementation plan from specification(s).
parameters:
  - name: plan_filename
    description: Path to the specific plan markdown file to process
    required: true
---

## Goal

Read and analyze the specification file to create a detailed, step-by-step implementation plan.

Process the specific plan file: {{ plan_filename }}

Generate a comprehensive draft plan that will be used to:
1. Generate rules (acceptance criteria)
2. Generate todos (implementation steps)

## Guidelines

### General
- DO NOT code at this step, we are just creating the plan
- DO make sure to review the existing codebase and architecture before creating the implementation plan
- DO make sure each planned step is a single focused task
- DO create many, small, incremental steps. Ideally each step should result in less than 250 lines of code changed
- DO make sure that each step builds on the previous steps
- DO NOT leave hanging or orphaned code that isn't integrated into a previous step
- DO NOT plan security features unless specifically asked by the user
- DO NOT plan performance features unless specifically asked by the user
- DO NOT plan backward compatibility features unless specifically asked by the user

### Plan Structure

Your draft plan should include:

1. **Overview**: Summary of what needs to be built
2. **Architecture Review**: What exists in the codebase that can be leveraged
3. **Acceptance Criteria**: Clear, testable rules that define success (these will become rules)
4. **Implementation Steps**: Detailed, incremental steps (these will become todos)
5. **Considerations**: Any important notes, gotchas, or design decisions

## Process

1. **Read and analyze** the specified plan file: {{ plan_filename }}

2. **Review context**:
   - Review the existing code to determine what parts of the specification might already be implemented
   - Unless explicitly instructed otherwise in the specification, do not add new systems/services when existing patterns and systems can be extended to achieve the goals

3. **Draft the plan**:
   - Create a comprehensive, detailed plan that covers:
     - What needs to be built
     - What existing code/patterns can be leveraged
     - Clear acceptance criteria (that will become rules)
     - Step-by-step implementation approach (that will become todos)
   - Write this plan to `.swissarmyhammer/tmp/DRAFT_PLAN.md`
   - Make sure the plan is thorough enough that someone reading it could:
     - Understand exactly what success looks like
     - Follow the implementation steps without the original spec
     - Know how each step builds on previous work

## Output

Write the complete draft plan to: `.swissarmyhammer/tmp/DRAFT_PLAN.md`

This file will be used by subsequent prompts to generate:
- Todos via `generate-todos` prompt
