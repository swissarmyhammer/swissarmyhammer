---
title: Do Code Review
description: Code up the code review
---

## Goal

You job is to code up the needed changes in the code review.

## Rules

{% render "workflow_guards" %}

## Process

{% render "issue/assert_on_worktree" %}
{% render "todo", todo_file: "./CODE_REVIEW.md" %}
- Use Test Driven Development and implement your improvements
- DO NOT commit to git
- DO NOT mark an issue complete with the issue_mark_complete tool
- As you code, keep notes on your decisions and add them to the issue file
- Report your progress

{% render "review_format" %}
