---
name: test-loop
description: Continuously run tests, create failure cards, and delegate fixes to /implement until the suite is fully green. Uses ralph to prevent stopping between iterations.
metadata:
  author: "swissarmyhammer"
  version: "0.10.1"
---

# Test Loop

Continuously run tests and fix failures until the entire suite is green.

This skill is an **orchestrator**. It does not write code or run tests itself. It delegates to `/test` (which finds failures and creates kanban cards) and `/implement` (which picks up those cards and fixes them), and uses `ralph` to stay alive between iterations.

## Process

1. **Set ralph**: call `ralph` with `op: "set ralph"` and instruction "Run tests and fix failures until all green".
2. **Run `/test`** — it runs the suite, creates `test-failure` kanban cards for any failures.
3. **Check for failure cards**: query `kanban` for tasks tagged `test-failure`.
4. **If failure cards exist**: run `/implement` once — it picks up exactly one card and fixes it. Then go back to step 2 to re-run tests and check for remaining failures. Repeat until no failure cards remain.
5. **Stop condition**: before clearing ralph, independently verify by querying kanban for any tasks tagged `test-failure` — if any exist, continue the loop regardless of what `/test` reported. Only when the kanban board has zero `test-failure` cards **and** `/test` reports a fully green run (zero failures, zero warnings, zero skipped) may you `clear ralph` and report.

## Constraints

### Ralph

- **First action**: set ralph. The Stop hook blocks you from stopping while ralph is active.
- Only call `ralph` with `op: "clear ralph"` when **both** conditions are met:
  1. The kanban board has zero tasks tagged `test-failure` (verified by a direct kanban query).
  2. `/test` reports all green with zero failures, zero warnings, and zero skipped tests.
- Never rely solely on `/test`'s prose summary — always re-check the kanban board independently before clearing ralph.

### Delegation

- `/test` owns test execution, failure analysis, and card creation. Do not run tests yourself.
- `/implement` owns card pickup and code fixes. Do not write code yourself.
- If `/implement` reports it is stuck on a card, skip it and continue the loop — do not try to fix it yourself.

### Scope

- Only fix what the tests surface. No bonus refactoring, no unrelated changes.
- The kanban board is the single source of truth. Do not use TodoWrite, TaskCreate, or other task tracking.

### When done

- Present a summary: how many iterations, what was fixed, final test status.
