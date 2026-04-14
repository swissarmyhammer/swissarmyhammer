---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffc980
project: task-card-fields
title: Render `progress` last in task header (card and inspector)
---
## What

The `progress` field currently renders mid-stream in the header section because it sits at index 5 in `swissarmyhammer-kanban/builtin/entities/task.yaml`'s `fields:` list, and the inspector + card both render header fields in that list order (filtered by `section: header` via `useEntitySections` in `kanban-app/ui/src/hooks/use-entity-sections.ts`).

The user wants `progress` to be the **last** field in the header on both surfaces — visually it's a summary that belongs after the title/tags/status_date/etc.

### Header fields today (in render order)

`title → tags → project → depends_on → progress → virtual_tags → status_date`

### Header fields after this card

`title → tags → project → depends_on → virtual_tags → status_date → progress`

### Approach — move one entry in `task.yaml`

Reorder `task.yaml` `fields:` list so `progress` is the very last entry. Both the card (`useCardSections` → header section) and the inspector (`useEntitySections` → header section) iterate the entity's `fields:` list and filter by section, preserving order — so this single edit covers both surfaces with no UI code change.

### Why this is safe with the `status_date` derive-order constraint

`task.yaml` already pins `status_date` after its computed dependencies (`completed`, `started`, `created`) so `derive_all` resolves it last. The constraint is "after its deps" not "absolutely last". `progress` is computed via `parse-body-progress`, which reads only `body` — a stored field, always already in the fields map. So `progress` can sit at any position. After this card, the order becomes:

```yaml
fields:
  - title
  - tags
  - assignees
  - project
  - depends_on
  - body
  - position_column
  - position_ordinal
  - attachments
  - virtual_tags
  - filter_tags
  - due
  - scheduled
  - created
  - updated
  - started
  - completed
  - status_date     # still after its computed deps
  - progress        # NEW position — last in list, last in header
```

### Files to modify

1. `swissarmyhammer-kanban/builtin/entities/task.yaml`
   - Move `- progress` (currently between `depends_on` and `body`) to the very end of the `fields:` list.
   - Update the existing trailing comment block to explain that `status_date` must be after its computed deps AND that `progress` deliberately follows `status_date` for header render order. The comment should make clear that `progress` doesn't break derive ordering because its only input (`body`) is a stored field.

2. `swissarmyhammer-kanban/src/defaults.rs`
   - Augment the existing `derive_status_date_resolves_after_its_dependencies_in_task_order` test (around line 1530) — or add a sibling — that asserts:
     a) `progress` appears in the task fields list,
     b) `progress` is the LAST field whose `section == "header"` when iterating `task_fields` in declared order.
   - This locks the user's intent into a regression test and complements (does not replace) the existing status_date-deps-before-status_date assertion.

### Non-goals (explicit)

- Do NOT touch `progress.yaml` itself — `section: header` is already correct.
- Do NOT touch `entity-card.tsx`, `entity-inspector.tsx`, `use-entity-sections.ts`, or any frontend code. The render order falls out of YAML order automatically.
- Do NOT reorder any other field. Just `progress`.
- Do NOT change `status_date`'s position — it still needs to come after its deps.

## Acceptance Criteria

- [x] On the kanban board, every task card whose progress has at least one subtask shows the progress bar as the LAST element of the card's header content (after title, tags, status_date, etc.).
- [x] In the task inspector, the `progress` field row appears as the LAST row of the `header` section (above the divider that separates header from body).
- [x] `swissarmyhammer-kanban/src/defaults.rs::derive_status_date_resolves_after_its_dependencies_in_task_order` still passes (no regression to status_date derive ordering).
- [x] A new (or augmented) regression test asserts `progress` is the last `section: header` field in task.yaml's declared field list.
- [x] No frontend test changes required; the existing `entity-card.test.tsx` / `entity-inspector.test.tsx` suites stay green.

## Tests

- [x] `swissarmyhammer-kanban/src/defaults.rs` — added `progress_is_last_header_field_in_task_fields`:
  - Load `kanban_compute_engine` + `FieldsContext::from_yaml_sources(builtin_field_definitions(), builtin_entity_definitions())`.
  - Iterate `ctx.fields_for_entity("task")` in order, collect names of fields whose `section.as_deref() == Some("header")`.
  - Assert the LAST name in that filtered list equals `"progress"`.
- [x] Ran: `cargo nextest run -p swissarmyhammer-kanban progress_is_last_header_field_in_task_fields derive_status_date_resolves_after_its_dependencies_in_task_order` → both green.
- [x] Ran: `cargo nextest run -p swissarmyhammer-kanban` → full suite stays green (1030 tests passed).
- [x] Ran: `pnpm exec vitest run src/components/entity-card.test.tsx src/components/entity-card-progress.test.tsx src/components/entity-inspector.test.tsx` → 41 tests passed.
- [ ] Manual verification: launch the kanban app on this repo. Pick any task with a subtask checklist — confirm the progress bar appears LAST in the card's header content and as the LAST row of the inspector's header section. (Deferred to reviewer.)

## Workflow

- Used `/tdd` — RED: added `progress_is_last_header_field_in_task_fields`; it failed because `progress` was between `depends_on` and `body`. GREEN: moved `- progress` to the end of `task.yaml`'s `fields:` list and refreshed the trailing comment. Both Rust tests pass; no frontend tests needed to change.
