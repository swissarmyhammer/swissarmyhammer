---
assignees:
- claude-code
position_column: todo
position_ordinal: bb80
title: _plan.entries is always empty — build_plan_data reads an object as an array
---
`build_plan_data` in `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` never produces a single plan entry. Every `_plan` the kanban MCP tool attaches carries `"entries": []`.

## Cause

`build_plan_data` calls `ListTasks::new().execute(ctx)` and then `tasks.as_array()`.

`ListTasks::execute` (`crates/swissarmyhammer-kanban/src/task/list.rs`) returns an OBJECT, not an array:

```json
{ "tasks": [ ... ], "count": 12 }
```

So `as_array()` gives `None`, `unwrap_or(&Vec::new())` supplies an empty list, and the `.map()` that builds the entries never runs. The failure is silent — there is no error and no warning.

## Evidence

A live `move task` call against this board, which holds hundreds of cards:

```json
"_plan": {
  "_meta": { "affected_task_id": "01KYSFNAHGT9827596R1T92GNJ",
             "source": "swissarmyhammer_kanban",
             "trigger": "move task" },
  "entries": []
}
```

`_meta.affected_task_id` is correct. Only `entries` is dead.

## Why this matters

The whole purpose of `_plan` is the entries list. The module header quotes the ACP rule "Complete plan lists must be resent with each update". The tool resends an empty list every time, so an ACP agent that emits Plan notifications from `_plan` shows the user an empty plan. `task_to_plan_entry` and the `PlanEntryStatus` / `PlanEntryPriority` mapping beside it are dead code today.

## Required change

1. Read the array out of the object: `tasks["tasks"].as_array()`. Verify against `ListTasks::execute` rather than assuming the key.
2. A shape mismatch must not stay silent. An unreadable task list should warn or error, the same way the `Err` arm already does, instead of degrading to an empty plan.
3. Check `task_to_plan_entry` against a REAL `list tasks` element. It reads `task["position"]["column"]`; confirm that the enriched shape `list tasks` returns actually carries that path, because nothing has ever exercised it.

## Acceptance

- A read-back test asserts `_plan.entries` names the cards on the board — at minimum, that the affected task appears in `entries[]._meta.id` for an operation that leaves the card on the board. The test must fail before the change.
- Entry `status` maps from the column: done to completed, doing to in_progress, else pending.
- The twelve `*_plan_carries_affected_task_id` tests in `mcp::tools::kanban::tests` stay green.

Found while adding the `affected_task_id` read-back tests for ^1t92gnj. Deliberately NOT fixed there: that card is test-only and on its final review round, and this is a production behavior change to every `_plan` consumer. #bug #kanban