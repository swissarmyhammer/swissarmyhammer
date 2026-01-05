---
title: generate-rules
description: Generate rules (acceptance criteria) from a draft plan.
parameters:
  - name: plan_filename
    description: Path to the original specification file
    required: true
---

## Goal

Read the draft plan and extract acceptance criteria to create permanent rules that define what correct code looks like.

Use the draft plan from: `.swissarmyhammer/tmp/DRAFT_PLAN.md`

Generate rules using the `rules_create` tool for each acceptance criterion.

## Guidelines

### Creating Rules

Look for acceptance criteria and requirements that can be checked automatically.
Think of these rules like linting or static analysis rules.

- Use the `rules_create` tool with parameters:
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

## Process

1. **Read the draft plan** from `.swissarmyhammer/tmp/DRAFT_PLAN.md`

2. **Identify acceptance criteria**:
   - Look for requirements that define success
   - Look for constraints that must be maintained
   - Look for quality criteria that can be checked

3. **Create rules**:
   - For each criterion, use `rules_create` tool
   - Make rules specific and testable
   - Group related rules using the name path (e.g., "api/endpoints", "api/validation")
   - Reference the original spec file {{ plan_filename }} in rule content where appropriate

## Output

Create rules using the `rules_create` tool for each acceptance criterion found in the draft plan.
