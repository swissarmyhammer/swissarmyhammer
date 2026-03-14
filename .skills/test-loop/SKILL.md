---
name: test-loop
description: Continuously run tests, create failure cards, and implement fixes until the suite is fully green. Uses ralph to prevent stopping between iterations.
metadata:
  author: swissarmyhammer
  version: "1.0"
hooks:
  Stop:
    - hooks:
        - type: command
          command: "sah tool ralph ralph check --"
---

# Test Loop

Continuously run tests and fix failures until the entire suite is green.

This skill is an **orchestrator**. It does not write code or run tests itself. It delegates to `/test` (which finds failures and creates kanban cards) and `/implement` (which picks up those cards and fixes them), and uses `ralph` to stay alive between iterations.

## Process

1. **Set ralph**: call `ralph` with `op: "set ralph"` and instruction "Run tests and fix failures until all green".
2. **Run `/test`** — it runs the suite, creates `test-failure` kanban cards for any failures.
3. **Check for failure cards**: query `kanban` for tasks tagged `test-failure`.
4. **If failure cards exist**: run `/implement` to pick up and fix each one.
5. **Loop**: go back to step 2.
6. **If no failure cards and tests are green**: `clear ralph` and report.

## Constraints

### Ralph

- **First action**: set ralph. The Stop hook blocks you from stopping while ralph is active.
- Only call `ralph` with `op: "clear ralph"` when `/test` reports all green with zero failures, zero warnings, and zero skipped tests.

### Delegation

- `/test` owns test execution, failure analysis, and card creation. Do not run tests yourself.
- `/implement` owns card pickup and code fixes. Do not write code yourself.
- If `/implement` reports it is stuck on a card, skip it and continue the loop — do not try to fix it yourself.

### Scope

- Only fix what the tests surface. No bonus refactoring, no unrelated changes.
- The kanban board is the single source of truth. Do not use TodoWrite, TaskCreate, or other task tracking.

### When done

- Present a summary: how many iterations, what was fixed, final test status.
