---
name: test-loop
description: Run tests again and again. Create a task for each failure. Send each fix to /implement until the suite is fully green. Use ralph to stop the agent from halting between rounds.
license: MIT OR Apache-2.0
compatibility: This skill needs the `kanban` and `ralph` MCP tools, plus a harness that supports Stop hooks, for example Claude Code. The harness must use the Stop hook to run the agent again for each round. The skill does not work on a harness without Stop hooks or without these MCP tools.
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

Run tests again and again. Fix failures until the whole suite is green.

This skill is an **orchestrator**. It does not write code, and it does not run tests itself. It sends work to `/test`, which finds failures and creates kanban tasks. It sends work to `/implement`, which picks up each task and fixes it. It uses `ralph` to stay active between rounds.

## Process

1. **Set ralph**: `{"op": "set ralph", "instruction": "Run tests and fix failures until all green"}`.
2. **Run `/test`.** It runs the suite and creates `test-failure` kanban tasks.
3. **Query kanban** for tasks tagged `test-failure`.
4. **If any tasks exist, run `/implement` once.** This picks up one task and fixes it. Then go back to step 2.
5. **Check the stop condition.** Before you clear ralph, query kanban for `test-failure` tasks on your own. If any exist, continue, even if `/test` reported success. You may `clear ralph` and report only when kanban shows zero `test-failure` tasks, and `/test` reports zero failures, zero warnings, and zero skipped tests.

## Constraints

**Ralph**
- Set ralph as your first action. While ralph is active, the Stop hook blocks you from stopping.
- Clear ralph only when both conditions are true: kanban has zero `test-failure` tasks, and `/test` reports all green.
- Do not rely only on the summary text from `/test`. Always check the board again.

**Delegation**
- `/test` owns test execution, analysis, and task creation. Do not run tests yourself.
- `/implement` owns task pickup and fixes. Do not write code yourself.
- If `/implement` is stuck on a task, skip it and continue. Do not take over the task yourself.

**Scope**
- Fix only what the tests show. Do not add extra refactoring.
- Kanban is the single source of truth. Do not use TodoWrite or TaskCreate.

**Done**
- Report the number of rounds, what you fixed, and the final test status.
