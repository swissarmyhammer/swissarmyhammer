---
title: issue_code
description: Code up an issue
---

## Goal

You job is to resolve an issue by coding a solution.

## Rules

{% render "workflow_guards" %}

## Process

{% render "issue/on_worktree" %}
- Evaluate the issue, think deeply about it, and decide how you will implement as code
  - Describe your proposed solution and use issue_update to add it to the issue file
    - Create a new markdown section in the issue like:

      ```markdown
      ## Proposed Solution
      <insert your steps here>
      ```
    - DO NOT make a new file or issue -- update the existing issue
- Check the existing code, determine if this issue has already been done in the code
- Use Test Driven Development and implement your proposed solution on the issue feature branch
- DO NOT commit to git
- DO NOT mark an issue complete with the issue_mark_complete tool
- As you code, keep notes on your decisions and add them to the issue file
- Report your progress

{% render "review_format" %}
