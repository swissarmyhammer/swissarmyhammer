---
title: are_rules_passing
description: "Check if all code rules are passing on changed files."
---

## Goal

We want to know if all coding rules pass on changed files.

Use the rules_check MCP tool with 
  - `changed: true` to check only changed files
  - `create_todo: true` so that we have a todo to correct the error

If you run rules check, on any ERROR or WARNING violations, `cel_set` are_rules_passing to `false`
If you run rules check, and all rules pass, `cel_set` are_rules_passing to `true`
