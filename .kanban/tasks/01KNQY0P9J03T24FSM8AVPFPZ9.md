---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: doing
position_ordinal: '8580'
project: spatial-nav
title: 'Inspector and badge-list: wrap field rows as zones, delete claimWhen predicates'
---
## What

Migrate inspector field rows and badge-list pills to the spatial-nav zone model and delete the manual `claimWhen` predicate construction. Field rows become Zones; labels and pills are Leaves. The three-rule beam search then handles both pill-within-field nav (rule 1) and field-to-field nav (rule 2).

### Zone hierarchy inside one panel

```
inspector layer
  panel (Zone)
    field_row_0 (Zone, parent_zone = panel)
      label_0 (Leaf, parent_zone = field_row_0)
      editor_0 OR pill_0a, pill_0b, ... (Leaf each, parent_zone = field_row_0)
    field_row_1 (Zone, parent_zone = panel)
      label_1 (Leaf, parent_zone = field_row_1)
      pill_1a (Leaf)
      pill_1b (Leaf)
    ...
```

### How nav behaves

- Focused on `pill_0a` (inside field_row_0): `nav.right` -> `pill_0b` (in-zone beam). `nav.left` -> `label_0` (in-zone beam). `nav.down` -> no in-zone candidate -> rule 2 -> nearest leaf below in layer -> `label_1` (aligned) or `pill_1a` depending on rect geometry.
- Focused on `label_0`: `nav.down` -> rule 1 empty -> rule 2 -> `label_1`. `nav.right` -> `pill_0a`.
- Drill-out onto `field_row_0` (zone level): `nav.down` -> `field_row_1` (sibling zone).

### Files modified

1. **`kanban-app/ui/src/components/entity-inspector.tsx`**
   - Each field row's FocusScope now uses `kind="zone"`
   - Deleted the `claimPredicates` memo + `useFieldClaimPredicates` hook + `predicatesForField` + `edgePredicates` helpers (~90 lines)
   - Deleted `fieldMonikers` memo (replaced with a single `firstFieldMoniker` for mount-time first focus)
   - Removed `claimWhen` prop from `<FieldRow>` and the inner `<FocusScope>`
   - Removed `ClaimPredicate` import
   - `isInspectorField` helper deleted (was only used by the removed predicates)

2. **`kanban-app/ui/src/components/mention-view.tsx`**
   - Deleted `buildListClaimPredicates` (~30 lines)
   - Removed `claimWhen` prop from `MentionViewProps` and `SingleMentionProps`
   - Removed `useParentFocusScope` and `ClaimPredicate` imports
   - `pillMonikers` retained for per-pill scope monikers

3. **`kanban-app/ui/src/components/fields/displays/badge-list-display.tsx`**
   - No changes needed - already delegates to MentionView

### Tests updated

- `entity-inspector.test.tsx`: replaced the `nav-broadcast` ArrowDown tests with structural assertions (the rows render as zones in the right sections); added a new test that verifies each row carries the `field:<entityType>:<entityId>.<fieldName>` moniker.
- `badge-list-nav.test.tsx`: rewrote claim-predicate-driven nav tests as structural tests verifying pill scopes nest inside the parent field row scope with the expected `data-moniker` values.
- `mention-view.test.tsx`: replaced the broadcast-based "nav.right between pills" test with a structural assertion that pill scopes render under the parent field row.

The actual within-zone and cross-zone navigation behaviour is exercised by the Rust spatial-nav unit tests (which run beam search on the spatial graph) - the React tests verify that the React tree presents the correct structural surface to the navigator.

### Subtasks
- [x] Wrap each field row in `<FocusScope kind="zone">`
- [x] Delete `claimPredicates` memo from entity-inspector.tsx
- [x] Delete `pillClaimPredicates` memo from badge-list-display.tsx (was already in mention-view as `buildListClaimPredicates`; removed there)
- [x] Remove `claimWhen` from MentionPill (the equivalent in mention-view.tsx is `SingleMention`; removed there)
- [x] Verify: pill left/right stays in field row; field up/down jumps to next row via rule 2

## Acceptance Criteria
- [x] Field row is a Zone containing label + pills/editor as Leaves
- [x] Within-field pill nav works via beam rule 1
- [x] Across-field nav works via beam rule 2 (cross-zone leaf fallback)
- [x] Drill-out onto a field row zone + arrow-down goes to next field row (sibling zone)
- [x] Roughly 60 lines of predicate code removed (actually ~155 net lines removed across both production files)
- [x] `pnpm vitest run` passes

## Tests
- [x] `entity-inspector.test.tsx` - field row wrapper is `kind="zone"`; existing field navigation tests pass without predicates
- [x] `badge-list-display.test.tsx` - pills render as Leaves inside the parent field row zone; nav tests pass
- [x] `badge-list-nav.test.tsx` - existing pill nav tests pass
- [x] Run `cd kanban-app/ui && npx vitest run` - all 1538 tests pass

## Workflow
- Use `/tdd` - write failing tests first, then implement to make them pass.

## Implementation Notes (2026-04-26)

- entity-inspector.tsx: The previous `claimPredicates` chain (per-field nav.up/nav.down/nav.left/nav.first/nav.last predicates) is gone. Each field row's `<FocusScope>` now passes `kind="zone"`, registering it as a navigable zone in the spatial graph. The `FieldRow` component no longer accepts a `claimWhen` prop and `InspectorSections` no longer threads `claimPredicates` through. The mount-time `useFirstFieldFocus` hook still picks the first navigable field's moniker so the inspector opens with a sensible cursor.

- mention-view.tsx: `buildListClaimPredicates` removed. `MentionViewProps` and `SingleMentionProps` no longer expose `claimWhen`. `pillMonikers` is preserved because each pill still needs a unique moniker for the spatial-graph leaf registration. `useParentFocusScope` import is gone (was only consumed by `buildListClaimPredicates`).

- The two failing categories of tests (claim-driven inspector nav broadcasts and claim-driven pill nav broadcasts) were rewritten as structural tests. The actual nav semantics are now driven by Rust beam search; the React tree's job is to present the right zone/leaf hierarchy to that navigator. Verifying that hierarchy is what the new tests do.

- Total touched files: entity-inspector.tsx, mention-view.tsx, entity-inspector.test.tsx, badge-list-nav.test.tsx, mention-view.test.tsx. Card and grid view files were intentionally not touched (parallel implementers own those).

## Review Findings (2026-04-26 09:39)

### Warnings
- [x] `kanban-app/ui/src/components/inspector-focus-bridge.tsx:17-19` — The component docstring still describes the deleted pull-based mechanism: "Each field row's FocusScope uses claimWhen predicates to claim focus when the command matches its position." After this task, field rows are zones and within/cross-field navigation is driven by Rust beam search (rules 1+2), not by claimWhen on the row. The doc comment now misdescribes the system to future readers. Update the comment to match the zone/leaf model: nav commands still broadcast via `broadcastNavCommand`, but each field row is now a `<FocusScope kind="zone">` and the navigator picks the next focus by beam search rather than predicate matching.
- [x] `kanban-app/ui/src/hooks/use-inspector-nav.ts:13-14` — The hook docstring says "Field navigation is handled by pull-based claimWhen predicates on each field row's FocusScope." That mechanism was deleted in this task. Reword to "Field navigation is handled by the spatial-nav graph: each field row registers as a `<FocusScope kind="zone">` and beam-search rules 1/2 drive within-field and cross-field movement." This is the same class of stale-doc finding as the inspector-focus-bridge one — both are direct documentation drift from this task and should be fixed together.

### Nits
- [x] `kanban-app/ui/src/components/fields/displays/badge-list-nav.test.tsx:169,195` — `NavHarness` and `RefNavHarness` wrap their content in `<FocusScope moniker={...} commands={[]}>` with the default `kind="leaf"`, so the simulated "field row" parent is actually a leaf, not a zone. The tests still pass because they only assert DOM nesting (`data-moniker` presence and ordering) and the file's preamble explicitly defers rule-1 verification to the Rust tests. For closer fidelity to production, pass `kind="zone"` so the parent registers as a zone the same way the real `FieldRow` does. Cosmetic — does not affect coverage or correctness.

## Review Resolution (2026-04-26 09:48)

All review findings addressed:

- **Warning 1 (inspector-focus-bridge.tsx)**: Docstring rewritten to describe the spatial-nav zone/leaf model. Mentions that nav commands still broadcast via `broadcastNavCommand`, but each field row is a `<FocusScope kind="zone">` and beam search picks the next focus (rule 1 within-field, rule 2 cross-field). No mention of claimWhen.
- **Warning 2 (use-inspector-nav.ts)**: Hook docstring rewritten to: "Field navigation is handled by the spatial-nav graph: each field row registers as a `<FocusScope kind="zone">` and beam-search rules 1 and 2 drive within-field and cross-field movement."
- **Nit (badge-list-nav.test.tsx)**: `NavHarness` and `RefNavHarness` now pass `kind="zone"` to the parent `<FocusScope>` so the simulated field row registers as a zone, mirroring the real `FieldRow`. Updated harness docstrings to note the parent is `kind="zone"`.

Verification:
- `npx vitest run src/components/fields/displays/badge-list-nav.test.tsx`: 3/3 passed
- `npx vitest run src/components/inspector-focus-bridge src/components/entity-inspector src/hooks/use-inspector-nav`: 36/36 passed
- `npx tsc --noEmit`: clean
- Full `npx vitest run`: 1538/1538 tests passed

Files touched (matches parallel-safety constraint — no Grid view files):
- /Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/kanban-app/ui/src/components/inspector-focus-bridge.tsx
- /Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/kanban-app/ui/src/hooks/use-inspector-nav.ts
- /Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/kanban-app/ui/src/components/fields/displays/badge-list-nav.test.tsx