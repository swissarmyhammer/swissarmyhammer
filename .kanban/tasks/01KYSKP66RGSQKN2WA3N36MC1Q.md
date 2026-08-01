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
- actor: claude-code
  id: 01kyyyty5brex5rqrewm30m51f
  text: |-
    ### commit — changed
    - evidence: 08a64a965 fix(kanban): reject dangling actor refs in add/update task assignees (^n36mc1q)
    - next: none
  timestamp: 2026-08-01T15:24:44.587369+00:00
- actor: claude-code
  id: 01kyyzh0x5qgn0fh28fkszwmxn
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (08a64a965). Engine counts: 7 findings, 7 confirmed, 3 refuted, 16 attempted. After the ^j4d2613 blame check: 1 in-scope — crates/swissarmyhammer-kanban/src/dispatch.rs:401 (singular `assignee` alias uses `get_string`, not `list_param`, so a list or stringified-array shape under the singular key is silently dropped). 6 out of scope: add.rs:101, add.rs:106, update.rs:97, update.rs:102, update.rs:107, update.rs:156 — every cited line blames to another commit (090b3f3ab9, 74d0cacc48, 7b47bf029c, d47ba21a, eb375282) and holds unrelated code; the add.rs/update.rs delta is doc comments only.
    - next: fix dispatch.rs:401 to use `list_param(op, "assignee")`, matching `tag_refs`. Task stays in `review`. Atomic rejection, `from_entity_error` IO/NotFound split, `op.actor` fallback, and description.md accuracy all verified clean.
  timestamp: 2026-08-01T15:36:48.293760+00:00
- actor: claude-code
  id: 01kyyzjrmtsgqm0fhxdz65p8fs
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 4 files (dispatch.rs, task/add.rs, task/update.rs, description.md)
    - test: green — cargo nextest -p swissarmyhammer-kanban -p swissarmyhammer-tools -p swissarmyhammer-fields, 3249 run / 3249 passed / 0 skipped
    - commit: 08a64a965
    - review: findings — crates/swissarmyhammer-kanban/src/dispatch.rs:401 (singular `assignee` alias uses `get_string`, not `list_param`, so a list shape under the singular key is silently dropped; the doc comment added in this commit over-claims that it accepts the same forgiving shapes)
    - also: 6 engine findings ruled out of scope by blame check (add.rs:101/106, update.rs:97/102/107/156 — blame 090b3f3ab9, 74d0cacc48, 7b47bf029c). Fourth consecutive occurrence of ^j4d2613; not this delta's work.
  timestamp: 2026-08-01T15:37:45.370217+00:00
- actor: claude-code
  id: 01kyz00jfpcerf5v79w3ep0qwq
  text: |-
    Review finding worked. One line of production code, proved RED first.

    **The fix.** `explicit_assignee_refs` (crates/swissarmyhammer-kanban/src/dispatch.rs) ended with `Ok(op.get_string("assignee").map(|a| vec![a.to_string()]))`. It now ends with `list_param(op, "assignee")`, the same call `tag_refs` makes for the singular `tag`.

    **RED evidence.** 5 new tests written before the change. `cargo nextest run -p swissarmyhammer-kanban -E 'test(singular_assignee)'` reported `6 tests run: 1 passed, 5 failed`:

    - `dispatch_add_task_singular_assignee_array_shape_persists` — `left: [], right: ["alice"]`
    - `dispatch_update_task_singular_assignee_array_shape_persists` — `left: [], right: ["alice"]`
    - `dispatch_add_task_singular_assignee_stringified_array_persists` — same empty-list failure
    - `dispatch_add_task_unknown_singular_assignee_array_creates_nothing` — the create succeeded instead of erroring
    - `dispatch_update_task_unknown_singular_assignee_stringified_array_errors` — `expected ActorNotFound, got: Err(ActorNotFound { id: "[\"nosuchactor\"]" })`

    The one that passed is the pre-existing scalar case `dispatch_add_task_unknown_singular_assignee_creates_nothing`, which confirms the RED set isolates the shape gap and not the actor check.

    The last failure is worth keeping in mind: a stringified array under the singular key DID error, but it reported the whole raw string `["nosuchactor"]` as the actor id. So the old code did not always drop silently — sometimes it produced a nonsense error message. Both faces of the bug are gone.

    **GREEN.** `cargo nextest run -p swissarmyhammer-kanban -E 'test(assignee)'` — 27 tests run, 27 passed.

    **Sibling audit of dispatch.rs.** Every ref-list param and its singular alias was checked. No second instance of the asymmetry exists.

    - `tags` / `tag` — `tag_refs` already routes both through `list_param`. Correct.
    - `depends_on` — `list_param`, no singular alias. Correct.
    - `attachments` — `attachment_param` reads `op.get_param` and falls through to `ref_list`. No singular alias exists, so there is no alias to narrow.
    - `fields` / `sort` on perspectives — `parse_json_array` errors on a wrong shape rather than dropping. Different helper, not a silent drop, no alias.
    - `assignee` on `assign task` / `unassign task` and `tag` on `tag task` / `untag task` are single-ref commands (`AssignTask::new(id, assignee)`), not ref-lists. `req` is the right reader there. Left alone.

    **Discovery — filed as a new card, not worked here.** ^qd9z2xt. `crates/swissarmyhammer-kanban/src/schema.rs` advertises `{"op": "list tasks", "assignee": "alice", "exclude_done": true}`, but the `list tasks` arm of `execute_task_query_operation` reads only column, filter, project, page, page_size, and detail. `exclude_done` matches exactly one line in the whole workspace — the schema example itself. Both params are dropped behind an `ok`. The existing test `dispatch_list_tasks_with_assignee_filter` cannot catch it: the board holds one task, so `count == 1` passes whether the filter runs or not.

    **Docs.** Two claims that the old code could not reach were corrected, not extended:
    - The `explicit_assignee_refs` doc comment said the singular key "is accepted as a one-element alias". It now says the singular key is an alias read through the same `list_param` path, so it takes every shape the plural key takes — the alias names the key, it does not narrow the shape.
    - `crates/swissarmyhammer-tools/src/mcp/tools/kanban/description.md` carried the same sentence. Corrected the same way.

    **No existing test was edited.** `git diff -U0 -- crates/` shows exactly 4 deleted lines: 3 doc-comment lines and the one `get_string` line. Every one of the 117 additions in dispatch.rs is new. Nothing inside `#[cfg(test)]` was removed or altered.
  timestamp: 2026-08-01T15:45:17.814305+00:00
- actor: claude-code
  id: 01kyz00t2nzazag29wdmfas0k4
  text: |-
    ### implement — changed
    - evidence: 2 files — crates/swissarmyhammer-kanban/src/dispatch.rs, crates/swissarmyhammer-tools/src/mcp/tools/kanban/description.md. `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-tools -p swissarmyhammer-fields --no-fail-fast` — 3254 run, 3254 passed, 0 skipped, 0 failed (baseline 3249 + 5 new tests). `cargo fmt --all --check` clean. `cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-tools --all-targets -- -D warnings` clean. The known flake `review_working_emits_progress_notifications_per_pair_when_token_supplied` passed.
    - next: ready for /review. New card ^qd9z2xt filed for the `list tasks` `assignee` / `exclude_done` drop found during the sibling audit.
  timestamp: 2026-08-01T15:45:25.589923+00:00
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

## Review Findings (2026-08-01 10:25)

Scope: `HEAD~1..HEAD` (08a64a965 only).

- [x] `crates/swissarmyhammer-kanban/src/dispatch.rs:401` — In `explicit_assignee_refs`, the singular `assignee` alias reads `op.get_string("assignee")` instead of `list_param(op, "assignee")`, so it does not get the forgiving-shape tolerance that `tag_refs` gives the singular `tag`. `assignee: ["alice"]` or `assignee: "[\"alice\"]"` makes `get_string` return `None`, so the function returns `Ok(None)` and the requested assignee is silently ignored — `add task` then falls back to `op.actor` and `update task` leaves the list unchanged, both acked `ok`. That is the same silent-drop class this card fixes. The doc comment written on this function in this commit states "`assignees` takes the same forgiving shapes as every other ref-list param (see [`ref_list`]); the singular `assignee` is accepted as a one-element alias", which holds for the plural key but over-claims for the singular one. Use `list_param(op, "assignee")` to match `tag_refs`. **FIXED 2026-08-01** — `explicit_assignee_refs` now ends with `list_param(op, "assignee")`. Its doc comment was rewritten to state what the code does: the singular key is an alias read through the same `list_param` path, so it takes every shape the plural key takes. `description.md` was corrected the same way. 5 new tests, all proved RED first.

### Out of scope — engine mis-citation (^j4d2613, fourth consecutive occurrence)

The engine returned 7 findings. Six describe code that this commit never touched, and all seven cited a line that does not contain the code described. `add.rs` and `update.rs` in this delta changed **doc comments only** — no builder signature and no `execute` method was modified.

| Engine citation | What the cited line holds | Described code really at | Real blame |
|---|---|---|---|
| dispatch.rs:317 | `let name = req(op, "name")?;` (blames 899c0267) | dispatch.rs:401 | 08a64a965 — in scope, kept above with the corrected line |
| add.rs:101 | `/// Set the position (column, ordinal)...` (blames d47ba21a) | add.rs:115 `with_depends_on` | 090b3f3ab9, 2026-02-01 |
| add.rs:106 | `}` (blames 090b3f3ab9) | add.rs:121 `with_tags` | 74d0cacc48, 2026-07-30 |
| update.rs:97 | `due: None,` (blames eb375282) | update.rs:115 `with_assignees` | 090b3f3ab9, 2026-02-01 |
| update.rs:102 | `/// Set the title` (blames 090b3f3ab9) | update.rs:121 `with_depends_on` | 090b3f3ab9, 2026-02-01 |
| update.rs:107 | blank line (blames 090b3f3ab9) | update.rs:127 `with_tags` | 74d0cacc48, 2026-07-30 |
| update.rs:156 | `/// Set the earliest start date (ISO 8601).` (blames eb375282) | update.rs:232 `execute` | 7b47bf029c, 2026-02-03 |

The six builder/`execute` findings ask to change pre-existing public signatures (`Vec<T>` to `impl IntoIterator`) and to add a doc comment to a pre-existing trait method. None is work created by this commit. Do not action them here.

### Verified clean on this delta

- Rejection is complete and atomic. Both the plural and singular ref paths funnel through `resolve_explicit_assignees`, which resolves the whole list before any write, so one bad ref rejects the operation with nothing stored.
- `from_entity_error` (crates/swissarmyhammer-kanban/src/error.rs:153) maps only `EntityError::NotFound { entity_type: "actor" }` to `ActorNotFound`; every other variant falls through to `EntityError(err)`, so a genuine IO error is not flattened.
- The `op.actor` fallback skips an unregistered actor rather than failing the create, and skips it rather than passing it on, so the echoed `assignees` equals what was stored.
- `description.md` matches behavior, including the paragraph separating the top-level `actor` key from `assignees`.
- The new `AddTask.assignees` / `UpdateTask.assignees` doc comments disclaim rather than over-claim: both state the struct does not check ids itself and that a direct caller still hits the fields-layer pruning.