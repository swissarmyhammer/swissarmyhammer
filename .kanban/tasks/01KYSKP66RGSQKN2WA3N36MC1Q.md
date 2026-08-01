---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyyxk5thvc59qhxxfkf5t6gp
  text: |-
    Picked up. Research notes.

    Chose option 1 (dispatch-layer resolution). Option 2 changes a fields-layer policy that covers every `reference` field on every entity, and a reference can legitimately dangle after its target is deleted, so a hard error on write could make existing boards unwritable.

    Precedents found in the same crate:
    - `resolve_depends_on` (dispatch.rs) — shape tolerance via `ref_list`, then per-element resolution, error on an unresolvable ref.
    - `AssignTask` (task/assign.rs) — already reads `actor` and returns `KanbanError::ActorNotFound`. So `assign task` rejected an unknown actor while `add`/`update task` dropped it. The fix makes the three agree.
    - `resolve_comment_author` (comment/mod.rs) — same existence check via `ectx.read("actor", id)` plus `KanbanError::from_entity_error`, which maps `EntityError::NotFound{actor}` to `ActorNotFound` and lets a real IO error through unchanged. Reused that mapping.

    Discovery: an actor id is NOT a ULID. `add actor` takes a caller-chosen slug (`alice`, `claude-code`), and `resolve_short_ref` is `TaskId`-typed and ULID-shaped. So the short id / `^short` / unique-prefix forms `resolve_depends_on` accepts have no meaning for an actor ref. Resolution here is an exact-id existence check. Nothing that worked before is narrowed — an exact actor id was the only form that survived the old pruning either.

    Scope note: the `op.actor` auto-assign fallback on `add task` is deliberately NOT validated. `actor` names the dispatching caller for attribution, not an assignment the caller asked for, and an unregistered caller must still be able to create a task.
  timestamp: 2026-08-01T15:03:01.713300+00:00
- actor: claude-code
  id: 01kyyyjf1ymjtvsf2dadcqnvf4
  text: |-
    Adversarial double-check returned REVISE with three findings. All three fixed, each proved RED first.

    1. The same silent drop was still live on `add task` through the `op.actor` auto-assign fallback, and it was worse there: `AddTask` echoes the entity it built before the write, so the response reported `assignees: ["ghostactor"]` while the fields layer pruned the id and `get task` reported `[]`. Fixed by skipping the fallback when the actor is not registered. The create still succeeds — `actor` is attribution, not a requested assignment — but the ack now equals what is stored. Proved RED: with the old fallback restored, `dispatch_add_task_unregistered_actor_fallback_is_skipped` failed with `left: Array [String("ghostactor")], right: Array []`.

    2. The `resolve_assignees` docstring claimed fallback behavior no test reached. Two tests added: unregistered fallback is skipped, registered fallback persists to storage.

    3. Doc parity. `description.md` was updated but the `AddTask.assignees` / `UpdateTask.assignees` doc comments generate the CLI help, where `--tags` and `--depends_on` already spell out their error contract and `--assignees` said nothing. Both extended. They state what the code does: `execute_operation` checks the id, the struct itself does not, so a direct Rust caller still meets the fields-layer prune.

    Also added a paragraph to `description.md` separating the top-level `actor` key from `assignees`, so no reader carries the new hard-error rule over to `actor`.
  timestamp: 2026-08-01T15:20:06.974300+00:00
- actor: claude-code
  id: 01kyyyjpa93v2rbmsm13q88k7c
  text: |-
    ### implement — changed
    - evidence: 4 files — crates/swissarmyhammer-kanban/src/dispatch.rs, crates/swissarmyhammer-kanban/src/task/add.rs, crates/swissarmyhammer-kanban/src/task/update.rs, crates/swissarmyhammer-tools/src/mcp/tools/kanban/description.md
    - tests: `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-tools -p swissarmyhammer-fields --no-fail-fast` — 3249 run, 3249 passed, 0 skipped. `cargo fmt --check` clean. `cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-tools --all-targets -- -D warnings` clean.
    - next: ready for /review
  timestamp: 2026-08-01T15:20:14.409339+00:00
- actor: claude-code
  id: 01kyyyrsh2kh2edd6d05f9m6t8
  text: |
    ### test — green
    - evidence: `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-tools -p swissarmyhammer-fields --no-fail-fast` — 3249 run, 3249 passed, 0 skipped, 0 failed (matches implementer's reported count exactly). `cargo fmt --check` clean. `cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-tools --all-targets -- -D warnings` clean, no warnings.
    - test-hunk audit (`git diff -- crates/`): every hunk inside `#[cfg(test)]` in `dispatch.rs` is a pure ADDITION (6 new tests appended after the existing test module: `dispatch_update_task_unknown_assignee_errors_and_keeps_list`, `dispatch_update_task_mixed_assignees_rejects_whole_list`, `dispatch_add_task_unknown_assignee_creates_nothing`, `dispatch_add_task_unregistered_actor_fallback_is_skipped`, `dispatch_add_task_registered_actor_fallback_persists`, `dispatch_add_task_unknown_singular_assignee_creates_nothing`). No pre-existing assertion was modified. Non-test hunks are doc-comment updates only (`add.rs`, `update.rs`, `description.md`).
    - check 2 (valid path unnarrowed): confirmed. `dispatch_add_task_registered_actor_fallback_persists` shows a registered actor id still auto-assigns and persists. Checked `ActorId` (`types/ids.rs`: `define_id!(ActorId, ...)`, a plain slug newtype, no ULID) and `AddActor { id: ActorId }` (`actor/add.rs`) — an actor id is always the exact slug the caller chose at `add actor`. Confirmed no short-id/`^`-prefix expansion mechanism exists for `ActorId` anywhere in the crate (`resolve_task_ref`/`resolve_depends_on`'s short-id logic is `TaskId`-only); the new `resolve_explicit_assignees` does an exact `ectx.read("actor", &raw)` check, so no previously-working id form is now rejected.
    - check 3 (no partial card on unknown assignee): confirmed by `dispatch_add_task_unknown_assignee_creates_nothing` and `dispatch_add_task_unknown_singular_assignee_creates_nothing` — both assert `ActorNotFound` is returned AND a follow-up `list tasks` shows `count: 0`.
    - known flake `review_working_emits_progress_notifications_per_pair_when_token_supplied`: ran and PASSED (slow, ~11.6s) in this run — did not need isolation re-run.
    - did not run rdeps closure or full workspace, per instruction.
  timestamp: 2026-08-01T15:23:34.306364+00:00
position_column: doing
position_ordinal: '8480'
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