---
name: test-loop
description: Continuously run tests, create failure tasks, and delegate fixes to /implement until the suite is fully green. Uses ralph to prevent stopping between iterations.
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` and `ralph` MCP tools , plus a Stop-hook-capable harness (e.g. Claude Code) so the declared Stop hook can re-invoke the agent across iterations. Will not function on harnesses that lack Stop hooks or these MCP tools.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
hooks:
  Stop:
    - hooks:
        - type: command
          command: "sah tool ralph ralph check --"
---

# Test Loop

Continuously run tests and fix failures until the entire suite is green.

This skill is an **orchestrator** — it does not write code or run tests itself. It delegates to `/test` (finds failures, creates kanban tasks) and `/implement` (picks them up and fixes them), and uses `ralph` to stay alive between iterations.

## Process

1. **Set ralph**: `{"op": "set ralph", "instruction": "Run tests and fix failures until all green"}`.
2. **Run `/test`** — it runs the suite and creates `test-failure` kanban tasks.
3. **Query kanban** for tasks tagged `test-failure`.
4. **If any exist**: run `/implement` once (picks up one task, fixes it). Back to step 2.
5. **Stop condition**: independently query kanban for `test-failure` tasks before clearing — if any exist, continue regardless of what `/test` reported. Only when kanban shows zero `test-failure` tasks **and** `/test` reports green (zero failures/warnings/skipped) may you `clear ralph` and report.

## Constraints

**Ralph**
- First action: set ralph. The Stop hook blocks stopping while active.
- Clear ralph only when both conditions hold: kanban has zero `test-failure` tasks AND `/test` reports all green.
- Never rely on `/test`'s prose summary alone — always re-check the board.

**Delegation**
- `/test` owns execution, analysis, task creation. Don't run tests yourself.
- `/implement` owns task pickup and fixes. Don't write code yourself.
- If `/implement` is stuck on a task, skip and continue — don't take it over.

**Scope**
- Only fix what tests surface. No bonus refactoring.
- Kanban is the single source of truth. No TodoWrite/TaskCreate.

**Done**
- Report: iteration count, what was fixed, final test status.
