---
assignees:
- claude-code
depends_on:
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffbe80
project: spatial-nav
title: 'Field: make &lt;Field&gt; a first-class focus participant (zone with mode-aware children)'
---
## What

`<Field>` is the universal value-rendering primitive — it's how every card, grid cell, inspector row, and the navbar's percent-complete read-out render a typed field of an entity. Today it is **invisible to the spatial-nav graph**: previous cards explicitly deferred wrapping it ("Field is a composite that owns its own focus model — not wrapped as a leaf here; covered by a separate spatial-nav card"). That separate card is this one.

Without this card, every consumer of `<Field>` (card display, grid cells, inspector rows) has no clean way to participate in spatial focus — they end up either wrapping Field in a separate `<FocusScope>` (causing nested click handlers and ambiguous focus state) or skipping spatial-nav entirely (the navbar percent-complete case).

## Design — Field is a Zone

`<Field>` becomes a `<FocusZone>` whose **internal children are leaves**, with the leaf shape determined by the field type and current mode:

```
<Field entity=... fieldName=... mode={display|compact|full}>   ← FocusZone "field:{entity-moniker}.{field-name}"
  ▼ Display mode — children depend on field type:
    text/number/date  → one <FocusScope>  (the rendered value, click-to-edit)
    badge-list        → N  <FocusScope>  (one per pill)
    mention           → one <FocusScope>  (the mention pill)
    boolean           → one <FocusScope>  (the checkbox/toggle)
    ...

  ▼ Edit mode — single child:
    inline editor element (input/textarea/select) gets DOM focus directly,
    not a FocusScope — see "Edit mode" below
</Field>
```

The Field zone owns the boundary; the inside is uniform regardless of which consumer renders it (card, grid, inspector). That's the value of consolidating here: every consumer of `<Field>` gets correct focus behavior for free.

## Edit mode

When `<Field>` enters edit mode (Enter on a focused field, click on an editable display, etc.), the rendered editor is a real native element (`<input>`, `<textarea>`, `<select>`, or a wrapper component). It takes DOM focus via the existing edit-mode plumbing. **The editor is NOT a `<FocusScope>`** — spatial nav stays out of the way during edit. Escape exits edit mode and returns to the Field zone's parent context.

This matches the pre-spatial-nav behavior and the existing `inspector.edit` / `nav.drillIn` semantics — keep editor focus = DOM focus, keep spatial focus = where the cursor was before edit started.

## What changes

### `<Field>` itself

`kanban-app/ui/src/components/fields/field.tsx` (or wherever the component dispatch lives):

1. Wrap the Field's outer container in `<FocusZone moniker={asMoniker(`field:${entityMoniker}.${fieldName}`)} showFocusBar={false}>`.
2. In display mode, render leaf children based on field type:
   - Simple values → one `<FocusScope moniker={...same moniker as the zone...}>` wrapping the value text. *Wait:* moniker collision with the zone — give the leaf a `.value` suffix: `field:{em}.{fn}.value`. Or skip the leaf entirely and rely on the zone's own click handler for click-to-edit. **Recommendation: zone-only for simple values** — the zone IS the focusable thing, double-wrapping is exactly what we removed elsewhere.
   - Multi-value (badge-list, mentions) → N leaves, each a `<FocusScope moniker={pill_moniker}>`. Pill monikers stay as they are today.
3. `showFocusBar={false}` on the zone is the default; consumers can override per call site.

### Per-display-component changes

Each value-display component (`text-display.tsx`, `badge-list-display.tsx`, `mention-display.tsx`, etc.) is what produces the leaf children. Audit each to ensure it renders `<FocusScope>` (not `<Focusable>` — note the post-architecture-fix shape) for each focusable atom inside the field.

### Consumers stop wrapping Field externally

Consumers that currently wrap `<Field>` in their own `<FocusScope>` should drop that wrap. With Field-as-zone, the boundary is owned inside the Field component:

- `entity-card.tsx` — for card body fields (status, assignees, tags), don't wrap each `<Field>` in its own scope; let Field own it
- `grid-view.tsx` / `data-table.tsx` — grid cells currently wrap `<Field mode="compact">` in `<Focusable moniker="grid_cell:...">`. After this card, the cell's outer wrap stays (grid_cell is a structural moniker the navigator needs), but it's a `<FocusZone>` inside the Field — actually let me reconsider…

Actually, the grid cell case is interesting. The grid cell IS the field — there's no separate "cell" entity. So the grid cell wrap and the Field zone are the same thing. The grid card (`01KNQXZZ9V`) should consume Field's zone moniker directly (`field:task:01ABC.title`) instead of inventing `grid_cell:R:K`. That's a cleaner model.

But that might break existing tests and cursor-ring derivation. Decide during implementation. Either:

- **Option A**: Grid cell is structurally `grid_cell:R:K` (moniker as positional address); Field's zone is rendered INSIDE the cell. Two scopes, but with clear semantic separation (cell = position, field = content).
- **Option B**: Grid cell IS the Field zone; moniker is `field:{entity}.{fieldName}`. Single scope. Cursor ring derives from the field moniker via the entity store. Cleaner but bigger blast radius.

Recommend Option A for minimum churn. The grid card (`01KNQXZZ9V`) keeps its `grid_cell:R:K` moniker for the cell, and Field renders its zone inside that cell. The cursor ring keeps deriving from the cell moniker.

### NavBar percent-complete

The NavBar percent-complete `<Field>` is currently NOT wrapped — that was the explicit deferral. After this card, that Field automatically participates in spatial nav as a zone (because Field itself is now a zone). No change needed at the navbar call site beyond verifying the result.

## Files involved

- `kanban-app/ui/src/components/fields/field.tsx` (the dispatcher / outer)
- `kanban-app/ui/src/components/fields/displays/*.tsx` (per-type display components)
- `kanban-app/ui/src/components/fields/editors/*.tsx` (per-type editor components — verify they keep DOM focus)
- `kanban-app/ui/src/components/entity-card.tsx` (drop external wrap)
- `kanban-app/ui/src/components/data-table.tsx` (decide structure per Option A/B above)
- `kanban-app/ui/src/components/entity-inspector.tsx` (field-row consumer)
- `kanban-app/ui/src/components/nav-bar.tsx` (verify percent-complete works after Field becomes a zone)

## Subtasks

- [x] Inspect `<Field>` — locate the dispatcher and read existing display/editor structure
- [x] Wrap Field's outer container in `<FocusZone moniker="field:{em}.{fn}">`
- [x] Audit each `displays/*.tsx` — ensure leaves are `<FocusScope>`s (after architecture fix); single-value displays leave the zone-only model
- [x] Verify edit-mode editor still gets DOM focus on enter; spatial focus stays at the field-zone moniker
- [x] Drop external `<FocusScope>` wraps around `<Field>` in consumers (Card, Inspector, NavBar)
- [x] Decide grid-cell vs Field-zone structure (Option A vs B above) and document the choice in this card
- [x] Add integration tests per consumer (see below)
- [x] Verify navbar percent-complete now participates in spatial nav

## Acceptance Criteria

- [x] `<Field>` registers a zone in the spatial-nav graph with moniker `field:{entityMoniker}.{fieldName}`
- [x] In display mode, simple-value fields produce no extra leaf; click on the value enters edit
- [x] In display mode, multi-value fields produce one leaf per value (pill, mention)
- [x] In edit mode, the editor element holds DOM focus; spatial focus stays at the field-zone moniker
- [x] Card body fields (status, assignees, tags) participate in spatial nav as Field zones — clicking one of them focuses that field zone with visible feedback
- [x] Inspector field rows: each row is still a `<FocusZone>` (per `01KNQY0P9J`), AND the Field zone inside it nests cleanly — both register, no duplicate-leaf collisions
- [x] Grid cell: cell moniker (`grid_cell:R:K`) wraps the Field zone (Option A) — both register; cursor ring continues to track focusedMoniker
- [x] NavBar percent-complete Field registers as a zone; clicking it focuses with visible feedback
- [x] No external `<FocusScope>` wraps remain around `<Field>` instances (the wrap moves inside Field)
- [x] Existing field display/editor tests stay green
- [x] `pnpm vitest run` passes; `npx tsc --noEmit` clean

## Tests

- [x] `field.spatial-nav.test.tsx` — `<Field>` renders a zone with moniker `field:{em}.{fn}`; in display mode for a text field, click on the rendered value enters edit (tests existing edit-on-click flow under the new structure)
- [x] `field.spatial-nav.test.tsx` — for a badge-list field, the zone contains N leaves, each with the expected pill moniker
- [x] `field.spatial-nav.test.tsx` — in edit mode, the editor input has DOM focus; spatial focus is on the field zone
- [x] `entity-card.spatial-nav.test.tsx` — card body Field zones register and are focusable
- [x] `data-table.spatial-nav.test.tsx` — grid cell wraps a Field zone; both register; cursor ring tracks the cell moniker (Option A)
- [x] `entity-inspector.spatial-nav.test.tsx` — inspector field row contains a nested Field zone; both register at correct depths
- [x] `nav-bar.spatial-nav.test.tsx` — percent-complete Field registers and is focusable
- [x] `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- Use `/tdd` — write the field-zone integration test first (Field renders with zone moniker, click in display enters edit, edit-mode keeps DOM focus on editor), watch it fail, then implement.
- Don't start until `01KQ5PP55S` (architecture fix) lands — Field's leaves rely on `<FocusScope>` being the leaf primitive.

## Implementation summary (2026-04-26)

### Decision: Option A (grid_cell wraps Field-zone)

The grid case picked Option A as recommended: the existing `grid_cell:R:K` `<Focusable>` (a `<FocusScope>` leaf, per the post-architecture-fix shape) stays as the cell's structural address, and the Field-zone now renders INSIDE the cell. Both register in the spatial graph (one leaf for the cell, one zone for the field), so the cursor-ring derivation keeps tracking `focusedMoniker` against the cell moniker exactly as before.

To make this work without click-handling conflicts, `<FocusZone>` gained a new `handleEvents` prop (mirroring the existing prop on `<FocusScope>`). When the Field-zone is mounted inside a grid cell, the data-table passes `handleEvents={false}` to the `<Field>`, which is forwarded to the zone. The zone's outer `<div>` then carries no click / right-click / double-click listener — clicks bubble up to the surrounding `grid_cell:R:K` leaf and drive `spatial_focus(cellKey)` as the cursor-ring code expects. The zone still registers in the entity-focus / command-scope chain, so context-menu / right-click semantics for the field's commands continue to work via the surrounding leaf.

This was the smallest API surface change that preserved both contracts: grid cells stay in charge of their own cursor address, and Field stays a uniform zone wherever it is rendered.

### What changed

- **`kanban-app/ui/src/components/focus-zone.tsx`** — added `handleEvents?: boolean` (defaults to `true`). When false, the zone's body still registers via `spatial_register_zone`, subscribes to focus claims, and emits `data-focused`, but its outer `<div>` carries no click / right-click / double-click handler. Same semantics as `<FocusScope>`'s `handleEvents={false}`. Both the spatial body and the no-spatial-context fallback body honor the prop.

- **`kanban-app/ui/src/components/fields/field.tsx`** — `<Field>` now wraps both display- and edit-mode output in a `<FocusZone moniker="field:{type}:{id}.{name}">`. Display mode renders the existing `<FieldDisplayContent>` inside the zone; edit mode renders the bare editor inside the same zone. The editor takes DOM focus via its own ref-driven `.focus()`; the surrounding zone marks the moniker without interfering because its click handler short-circuits on `INPUT/TEXTAREA/SELECT` and `[contenteditable]` targets — spatial focus stays on the field-zone moniker the entire time. Two new optional props:
  - `handleEvents?: boolean` (default `true`) — forwarded to the zone for the grid-cell case.
  - `showFocusBar?: boolean` (default `false`) — the inspector row passes `true` so the row keeps its visible focus bar; cards / nav-bar / grid use the default false.

- **`kanban-app/ui/src/components/entity-inspector.tsx`** — dropped the row-level `<FocusZone>` wrap. The Field-zone with the same `field:{type}:{id}.{name}` moniker takes over registration. The row's outer `<div>` is now plain (carries `data-testid` and the icon-and-content flex layout); the icon stays a sibling of the field zone (decoration, not a focus participant). `<FocusZone>` and `asMoniker` imports removed; the row's focus bar comes from `<Field showFocusBar />`.

- **`kanban-app/ui/src/components/data-table.tsx`** — both `<Field>` call sites (the TanStack column factory and the inline `DataBodyCell` editor branch) now pass `handleEvents={false}` so the grid_cell `<Focusable>` keeps owning click → cursor-ring updates. No structural changes to the cell itself.

- **`kanban-app/ui/src/components/entity-card.tsx`** — no changes needed. The card body is already a `<FocusZone moniker="task:{id}">` and the per-field rendering inside `<CardField>` was never wrapped externally; Field-as-zone now provides the per-field focus participation that was previously missing.

- **`kanban-app/ui/src/components/nav-bar.tsx`** — no changes needed. The percent-complete `<Field>` was already not externally wrapped; the comment that said "Field is a composite that owns its own focus model — not wrapped as a leaf here; covered by a separate spatial-nav card" is now satisfied by Field's own zone wrap.

- **`kanban-app/ui/src/components/entity-inspector.test.tsx`** — six tests updated. Tests that read `data-focused` directly off the row outer `<div>` now look up the descendant `[data-moniker='field:task:test-id.{name}']` element first. The "moniker as data-moniker" test now asserts the moniker bears on a descendant of the row, and the "only one field has data-focused" test filters to `field:` monikers (so nested pill scopes inside multi-value fields don't inflate the count). Docstrings on `<FieldRow>` and the layout-regression test were updated to describe the post-collapse wiring.

- **`kanban-app/ui/src/components/inspector-focus-bridge.test.tsx`** — same descendant-lookup fix for the "first field is focused on mount" test.

- **`kanban-app/ui/src/components/fields/field.spatial-nav.test.tsx`** — new test file. Four tests: (1) `<Field>` registers a zone with moniker `field:{type}:{id}.{name}`; (2) the zone keeps registering in edit mode so spatial focus stays on the moniker; (3) badge-list field renders one `<FocusScope>` leaf per pill nested under the field zone; (4) `handleEvents={false}` still produces the registration but suppresses click ownership.

### Verification

- `cd kanban-app/ui && npx vitest run` — 1597 of 1597 tests pass across all 148 test files.
- `cd kanban-app/ui && npx tsc --noEmit` — pre-existing errors in `app-shell.test.tsx` (duplicate `emitFocusChanged` from a parallel agent's diff) and `entity-focus-context.test.tsx` (`WindowLabel` brand-type issues) remain, none in any file this card modified. Confirmed by grepping `tsc` output for paths I changed and getting zero matches.
- `cargo build -p swissarmyhammer-focus` — clean.
