---
name: test-loop
description: Continuously run tests, create failure tasks, and delegate fixes to /implement until the suite is fully green. Uses ralph to prevent stopping between iterations.
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` and `ralph` MCP tools , plus a Stop-hook-capable harness (e.g. Claude Code) so the declared Stop hook can re-invoke the agent across iterations. Will not function on harnesses that lack Stop hooks or these MCP tools.
metadata:
  author: swissarmyhammer
  version: 0.12.11
---

# Test Loop

Continuously run tests and fix failures until the entire suite is green.

This skill is an **orchestrator**. It does not write code or run tests itself. It delegates to `/test` (which finds failures and creates kanban tasks) and `/implement` (which picks up those tasks and fixes them), and uses `ralph` to stay alive between iterations.

## Process

1. **Set ralph**: call `ralph` with `op: "set ralph"` and instruction "Run tests and fix failures until all green".
2. **Run `/test`** — it runs the suite, creates `test-failure` kanban tasks for any failures.
3. **Check for failure tasks**: query `kanban` for tasks tagged `test-failure`.
4. **If failure tasks exist**: run `/implement` once — it picks up exactly one task and fixes it. Then go back to step 2 to re-run tests and check for remaining failures. Repeat until no failure tasks remain.
5. **Stop condition**: before clearing ralph, independently verify by querying kanban for any tasks tagged `test-failure` — if any exist, continue the loop regardless of what `/test` reported. Only when the kanban board has zero `test-failure` tasks **and** `/test` reports a fully green run (zero failures, zero warnings, zero skipped) may you `clear ralph` and report.

## Constraints

### Ralph

- **First action**: set ralph. The Stop hook blocks you from stopping while ralph is active.
- Only call `ralph` with `op: "clear ralph"` when **both** conditions are met:
  1. The kanban board has zero tasks tagged `test-failure` (verified by a direct kanban query).
  2. `/test` reports all green with zero failures, zero warnings, and zero skipped tests.
- Never rely solely on `/test`'s prose summary — always re-check the kanban board independently before clearing ralph.

### Delegation

- `/test` owns test execution, failure analysis, and task creation. Do not run tests yourself.
- `/implement` owns task pickup and code fixes. Do not write code yourself.
- If `/implement` reports it is stuck on a task, skip it and continue the loop — do not try to fix it yourself.

### Scope

- Only fix what the tests surface. No bonus refactoring, no unrelated changes.
- The kanban board is the single source of truth. Do not use TodoWrite, TaskCreate, or other task tracking.

### When done

- Present a summary: how many iterations, what was fixed, final test status.
