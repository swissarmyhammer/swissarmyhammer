---
title: Do Code Review
description: Code up the code review
---

## Goal

You job is to code up the needed changes in the code review.

## Rules

{% render "workflow_guards" %}

### Rust

- Run tests with `cargo nextest run --failure-output immediate --hide-progress-bar --status-level fail --final-status-level fail`
- Be patient and let the test run finish before moving on

## Process

{% render "todo", todo_file: "./CODE_REVIEW.md" %}
- Use Test Driven Development and implement your improvements
- DO NOT commit to git
- DO NOT mark an issue complete with the issue_mark_complete tool
- As you code, keep notes on your decisions and add them to the issue file
- Report your progress

{% render "review_format" %}
