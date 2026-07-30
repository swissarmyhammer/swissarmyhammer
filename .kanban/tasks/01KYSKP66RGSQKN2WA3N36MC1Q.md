---
assignees:
- claude-code
position_column: todo
position_ordinal: b880
title: assignees silently drops an unknown actor ref
---
Found by the sibling-field audit on ^1t92gnj.

`add task` and `update task` accept an `assignees` list. An actor id that names no actor is dropped on write with no error and no warning.

```
add actor is NOT called
update task { id, assignees: ["nosuchactor"] }  -> { ok: true }
get task { id }                                 -> { assignees: [] }
```

## Cause

`assignees` is a `reference` field to `actor` (crates/swissarmyhammer-kanban/builtin/definitions/assignees.yaml). `default_reference_validation` in crates/swissarmyhammer-fields/src/validation.rs prunes dangling ids on write. Its own doc says "No error thrown — broken references are cleaned up, not rejected".

So this is a policy of the fields layer, not of the kanban dispatch layer, and it covers every reference field, not only `assignees`. ^1t92gnj fixed the shape drop on `assignees` (a scalar or a stringified array used to vanish) but left this ref drop, because changing the pruning policy is a wider design decision.

## Why it matters

The same reason the `tags` defect mattered: the response says `ok`, the caller has no way to learn the input was lost, and an agent that trusts the ack writes unassigned cards forever.

## Options

1. Resolve actor refs in `dispatch_add_task` / `dispatch_update_task` and error on an unknown actor, the way `resolve_depends_on` errors on an unknown task. Narrow, kanban-only.
2. Give the fields layer a per-field "strict reference" flag and set it on `assignees`. Wider, fixes every reference field the same way.

## Acceptance

- `update task { assignees: ["nosuchactor"] }` returns an error and leaves the assignee list unchanged. Test must fail before the change.
- `add task` with an unknown assignee does not create the task.
- The note in crates/swissarmyhammer-tools/src/mcp/tools/kanban/description.md that says an unknown actor id is dropped gets updated to match the new behavior. #bug #kanban