---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff9d80
title: Declarative field sections with separators on inspector and card
---
## What

Make field grouping metadata-driven. Today the inspector hardcodes `header` / `body` / `footer` / `hidden` (see `useFieldSections` in `kanban-app/ui/src/components/entity-inspector.tsx`), and the card only ever renders `header` (`useHeaderFields` in `entity-card.tsx`). Fields already declare things like `section: dates` (due, scheduled) and `section: system` (created, updated, started, completed) in YAML, but those sections silently fall through to `body` in the inspector and are invisible on cards.

Goal: let entity YAML declare the ordered list of sections — each with an optional label and an `on_card` opt-in — and have both the inspector and the card render them in that order with dividers (and an optional small section heading). Default to the current behaviour when an entity omits `sections` (backcompat for tag/actor).

### Target layout for `task`

- **Inspector**: `header → divider → body → divider → "Dates" label → dates → divider → "System" label → system → divider → footer`.
- **Card**: header fields at top, then a divider, then the `dates` section compact-rendered at the bottom. `system` and `body` do NOT appear on cards.

### Files to modify / create

1. **Rust — extend `EntityDef`**
   `swissarmyhammer-fields/src/types.rs`
   - Add a new `pub struct SectionDef { pub id: String, pub label: Option<String>, #[serde(default)] pub on_card: bool }` with the usual `Deserialize`/`Serialize`/`Debug`/`Clone`/`PartialEq`.
   - Add `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub sections: Vec<SectionDef>` on `EntityDef`.
   - YAML round-trip test next to the existing ones.

2. **YAML — declare task sections**
   `swissarmyhammer-kanban/builtin/entities/task.yaml` — add after `body_field`:
   ```yaml
   sections:
     - id: header
       on_card: true
     - id: body
     - id: dates
       label: Dates
       on_card: true
     - id: system
       label: System
     - id: footer
   ```
   Do NOT touch individual field YAMLs — their `section:` values are already correct (`dates` for due/scheduled, `system` for created/updated/started/completed, `header` for title/tags/progress/project/depends_on/status_date, `footer` for attachments).

3. **TS — mirror the schema type**
   `kanban-app/ui/src/types/kanban.ts`
   - Add `export interface SectionDef { id: string; label?: string; on_card?: boolean }`.
   - Extend `EntityDef` with `sections?: SectionDef[]`.

4. **Inspector — render by declared sections**
   `kanban-app/ui/src/components/entity-inspector.tsx`
   - Replace the hardcoded `useFieldSections` with a helper that takes `entity.sections` (from the schema) + the full `fields` list and returns `Array<{ def: SectionDef; fields: FieldDef[] }>` in declared order.
   - Default sections when YAML omits them: `[{ id: "header" }, { id: "body" }, { id: "footer" }]` — preserves current tag/actor rendering.
   - Fields with `section: "hidden"` stay excluded. Fields whose `section` doesn't appear in the declared list fall into `body` (so unknown values don't vanish).
   - Replace `InspectorSections` with a generic renderer that iterates the array: for each non-empty section, render a divider (except before the first one), an optional `<div className="text-[11px] uppercase tracking-wide text-muted-foreground/70 mb-1" data-testid={`inspector-section-label-${id}`}>{label}</div>` when `label` is present, then the rows.
   - `navigableFields` stays a flat list in declared section order so keyboard nav walks `header → body → dates → system → footer` in that order.
   - The `data-testid="inspector-header"` / `"inspector-body"` / `"inspector-footer"` hooks already used in tests become generic `data-testid={`inspector-section-${id}`}` — update any test that reads the old names (see list below).

5. **Card — render `on_card` sections**
   `kanban-app/ui/src/components/entity-card.tsx`
   - Replace `useHeaderFields` with a `useCardSections(entityType)` helper that returns the ordered list of sections where `on_card === true` (default: only `header` — backcompat).
   - In `CardFields`, after rendering the header section's fields, iterate the remaining `on_card` sections. Each renders with a thin top divider (`<div className="my-1.5 h-px bg-border/50" />`) and the section's fields in `mode="compact"` via existing `CardField`. No label heading on cards (labels belong to the inspector — cards stay dense).

### Non-goals (explicit)

- No change to which `section:` each individual field declares — all six date fields and the new `status_date` are already correctly tagged.
- No reordering of fields within a section — `entity.fields` list order still drives that.
- No per-section collapse/expand UI. Just dividers + optional labels.
- No change to grid/table views — those use `position_column`-style fields, not sections.
- Does NOT conflict with card 01KP23V1DJFTZW45CX8SG45W2Q (hide empty progress rows) or card 01KP24H4ADRYEC1T6KC6SY6K02 (status_date). Both touch `entity-inspector.tsx`; merge order is independent — the empty-filter logic runs on the per-section field list after sectioning.

## Acceptance Criteria

- [x] `cargo nextest run -p swissarmyhammer-fields entity_def_sections` — new YAML round-trip test passes for an `EntityDef` containing a `sections` list.
- [x] `EntityDef` omitting `sections` still deserializes (backcompat with tag.yaml, actor.yaml, board.yaml, column.yaml).
- [x] `task.yaml` declares the five-section list shown above.
- [x] In the task inspector, sections render in declared order: `header`, `body`, `dates` (with visible "Dates" label above), `system` (with visible "System" label), `footer`. Dividers appear between non-empty sections.
- [x] In the inspector for an entity that does NOT declare `sections` (e.g. tag), only `header` / `body` / `footer` render — no regression.
- [x] In the task card, the `header` fields render at the top and the `dates` fields (due, scheduled) render at the bottom separated by a thin divider. `system` fields (created/updated/started/completed) do NOT render on the card.
- [x] Arrow-key navigation in the inspector walks the visible fields in declared section order (including into the `dates` and `system` sections).
- [x] No field whose `section:` value is unknown to the entity's declared list disappears — it still shows in `body`.

## Tests

- [x] `swissarmyhammer-fields/src/types.rs` — add `entity_def_sections_yaml_round_trip` covering `sections: [{id: dates, label: Dates, on_card: true}, {id: header}]`.
- [x] `kanban-app/ui/src/components/entity-inspector.test.tsx`
  - [x] Rename `inspector-header` / `inspector-body` / `inspector-footer` assertions to `inspector-section-header` / `inspector-section-body` / `inspector-section-footer`.
  - [x] New test: task schema with `sections: [header, body, dates]` renders `inspector-section-dates` with its label when both fields (due/scheduled) are set.
  - [x] New test: empty section is not rendered (and no dangling divider).
  - [x] New test: keyboard ArrowDown from the last field in `body` focuses the first field in `dates`.
- [x] `kanban-app/ui/src/components/entity-card.test.tsx`
  - [x] New test: `on_card: true` `dates` section renders below header fields with a divider; `on_card` unset sections do not render.
- [x] Update any other browser/unit tests that reference the old `inspector-header` / `inspector-body` / `inspector-footer` testids (grep: `inspector-header`, `inspector-body`, `inspector-footer` in `kanban-app/ui/src/**/*.{ts,tsx}`).
- [x] Run: `cargo nextest run -p swissarmyhammer-fields` — green.
- [x] Run: `cd kanban-app/ui && pnpm test -- entity-inspector entity-card` — green, no regressions.
- [x] Run the full UI suite: `cd kanban-app/ui && pnpm test` — green (except 16 pre-existing failures in `editor-save.test.tsx` for `due (date)` / `scheduled (date)` — unrelated to this card; introduced by sister card 01KP24H4ADRYEC1T6KC6SY6K02 / date-fields commit eb375282c).

## Workflow

- Use `/tdd` — RED: write the Rust round-trip test + the failing "renders dates section with label" inspector test + the "dates on card" card test. GREEN: extend `EntityDef` serde, add the `sections` list to task.yaml, generalise the inspector section renderer, extend the card renderer. Refactor the section iteration into a shared `useEntitySections` hook last if both inspector and card can share it.

## Implementation Notes

- Added `SectionDef` struct and `sections: Vec<SectionDef>` field on `EntityDef` in `swissarmyhammer-fields/src/types.rs`. Backcompat preserved via `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
- Updated all EntityDef literal constructions across context.rs, validation.rs, types.rs tests, derive.rs, swissarmyhammer-kanban/src/derive_handlers.rs, swissarmyhammer-entity/src/store.rs, swissarmyhammer-entity/src/io.rs to include `sections: vec![]`.
- Added `sections:` block to `swissarmyhammer-kanban/builtin/entities/task.yaml` with the five-section layout.
- Added `SectionDef` interface and `sections?` field to `EntityDef` in `kanban-app/ui/src/types/kanban.ts`.
- Created shared hook `kanban-app/ui/src/hooks/use-entity-sections.ts` exposing `resolveEntitySections` (pure function) and `useEntitySections` (memoised React wrapper). Used by both inspector and card.
- Refactored `entity-inspector.tsx`: removed hardcoded `useFieldSections`/`FieldSections`/`InspectorFooter`; added new `SectionBlock` component that renders optional top divider, optional label heading, and the section's rows. Navigation walks `sections.flatMap(s => s.fields)` in declared order.
- Refactored `entity-card.tsx`: replaced `useHeaderFields` with `useCardSections` that returns `on_card: true` sections (defaulting to just the `header` section when an entity omits `sections`). `CardFields` iterates sections with a thin `my-1.5 h-px bg-border/50` divider between non-empty ones.
- Added `use-entity-sections.test.ts` with 8 unit tests covering default fallback, empty-sections fallback, declared order, hidden fields, unknown section ids routing to body, absent section treated as body, empty buckets, and non-default-body fallback.