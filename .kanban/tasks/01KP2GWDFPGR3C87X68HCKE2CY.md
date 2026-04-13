---
assignees:
- claude-code
position_column: todo
position_ordinal: c680
title: Hide due/scheduled from task card view (inspector-only)
---
## What

`due` (hard deadline) and `scheduled` (earliest start) are user-facing date fields tagged `section: dates`. After card 01KP24RHG1FARV7J1F4VMAN59F (declarative sections) landed, `swissarmyhammer-kanban/builtin/entities/task.yaml` declares the `dates` section with `on_card: true`, which causes both fields to render below the card header on the board view.

The user wants these dates to remain available in the inspector but **disappear from the card**. The board card view should stay dense — title, tags, status_date, etc. The detailed dates belong in the inspector when the user explicitly opens it.

### Approach

One-line YAML edit: drop `on_card: true` from the `dates` section entry in `task.yaml`. With `on_card` defaulting to `false` (per `SectionDef` in `swissarmyhammer-fields/src/types.rs`), the section keeps rendering in the inspector (where everything renders by default) and stops rendering on the card.

### Files to modify

1. `swissarmyhammer-kanban/builtin/entities/task.yaml`
   - Remove the `on_card: true` line under `- id: dates`. Keep the `label: Dates` so the inspector still shows the section heading.

After the edit, the `sections` block reads:

```yaml
sections:
  - id: header
    on_card: true
  - id: body
  - id: dates
    label: Dates
  - id: system
    label: System
  - id: footer
```

### Non-goals (explicit)

- Do NOT change which `section:` each individual field declares — `due.yaml` and `scheduled.yaml` still belong in `dates`.
- Do NOT touch the `header` section's `on_card: true` — `status_date`, `title`, `tags`, `progress`, etc. still belong on the card.
- Do NOT remove the `Dates` label — the inspector grouping still benefits from it.
- Do NOT touch `entity-card.tsx`, `entity-inspector.tsx`, or any test that uses a hand-built schema. Those test the generic `on_card` mechanism and are unaffected — they build their own `SectionDef` arrays inline.

## Acceptance Criteria

- [ ] On the kanban board, no task card shows `due` or `scheduled` rows. The card body stays at header-only fields (title, tags, status_date, …).
- [ ] Opening a task's inspector still renders the `dates` section with the "Dates" label and both `due` and `scheduled` editors visible.
- [ ] No keyboard-nav regression in the inspector — ArrowDown still walks header → body → dates → system → footer.
- [ ] All existing tests remain green; the unit test in `entity-card.test.tsx` that uses a hand-built schema with `on_card: true` for a `dates` section continues to pass (it does not depend on `task.yaml`).

## Tests

- [ ] `swissarmyhammer-fields/src/types.rs` already has `entity_def_sections_yaml_round_trip` covering `on_card: true|false|absent` round-tripping. No new test needed there.
- [ ] Add a regression assertion in `swissarmyhammer-kanban/src/defaults.rs` (or wherever the existing builtin-entity loading tests live — grep `builtin_entity_definitions` to find the right module): assert that `task.yaml`'s `sections` list contains `dates` and that its `on_card` is `false`. Prevents accidental re-introduction.
- [ ] Run: `cargo nextest run -p swissarmyhammer-fields -p swissarmyhammer-kanban` → green.
- [ ] Run: `cd kanban-app/ui && pnpm test -- entity-card entity-inspector` → green (existing tests still pass; the hand-built `on_card: true` test is unaffected).
- [ ] Manual verification: launch the kanban app on this repo. On the board, no card shows due/scheduled rows. Open any task — the inspector still shows the `Dates` section with both fields.

## Workflow

- Use `/tdd` — RED: add the regression assertion (`task.yaml` `dates` section has `on_card == false`); it will fail. GREEN: drop the `on_card: true` line. Verify manually in the running app.
