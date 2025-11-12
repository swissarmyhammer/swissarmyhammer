---
title: are_rules_passing
description: "Check if all code rules are passing on changed files."
---

## Goal

We want to know if all coding rules pass on changed files.

## Rules

If you run rules check, on any ERROR or WARNING violations, respond only with NO
If you run rules check, and all rules pass, respond only with YES
Create todo items for each rule violation found

## Process

- Use the rules_check MCP tool with `changed: true` to check only changed files
- Use `max_errors: 1` to stop on first error for faster correction loop
- If there are any violations, use todo_create to create a todo item for each violation with the file path, line number, and violation message
- Respond with only YES or NO based on whether violations exist
