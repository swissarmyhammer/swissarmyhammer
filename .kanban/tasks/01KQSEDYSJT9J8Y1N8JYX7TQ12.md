---
assignees:
- claude-code
depends_on:
- 01KQSDP4ZJY5ERAJ68TFPVFRRE
- 01KQSEC2KJ1K1CVTHYNXGZZG2C
position_column: todo
position_ordinal: d280
project: spatial-nav
title: 'Spatial-nav follow-up C: sweep <FocusZone> JSX → <FocusScope>; delete focus-zone.tsx'
---
## Reference

Parent: `01KQSDP4ZJY5ERAJ68TFPVFRRE`. Predecessor: sub-task B (`01KQSEC2KJ1K1CVTHYNXGZZG2C`) — IPC + React adapter must land first.

After this task lands, every `<FocusZone>` callsite has been renamed to `<FocusScope>`, the `focus-zone.tsx` file is deleted, and `pnpm -C kanban-app/ui exec tsc --noEmit` passes for the entire component tree. Test files (~120 files) are still partially broken — sub-task D handles those.

## What

Mechanical sweep of every `<FocusZone>` JSX callsite and import. The `FocusScope` component already accepts `children: ReactNode` (verified during planning); the only difference between today's `<FocusZone>` and `<FocusScope>` was the type-level distinction and a few zone-only props that already moved onto `FocusScope` in sub-task A.

### Files to modify (component layer)

Run `grep -rln "FocusZone" kanban-app/ui/src/components` to confirm the complete list. Known callsites:

- `kanban-app/ui/src/components/nav-bar.tsx` — outer `<FocusZone moniker="ui:navbar">` and `<FocusZone moniker="ui:navbar.board-selector">`
- `kanban-app/ui/src/components/board-view.tsx` — `BoardSpatialZone` → `<FocusZone moniker="ui:board">`
- `kanban-app/ui/src/components/view-container.tsx` — `ViewSpatialZone` (NOTE: task `01KQPW61ZNS010FV8PV844KY0K` may have already deleted this — verify before touching)
- `kanban-app/ui/src/components/perspective-container.tsx` — `PerspectiveSpatialZone` → `<FocusZone moniker="ui:perspective">`
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — `PerspectiveBarSpatialZone` and the `PerspectiveTabFocusable` zone
- `kanban-app/ui/src/components/left-nav.tsx` — `<FocusZone moniker="ui:left-nav">`
- `kanban-app/ui/src/components/column-view.tsx` — column body zones
- `kanban-app/ui/src/components/grid-view.tsx` — `GridSpatialZone`
- `kanban-app/ui/src/components/entity-card.tsx` — card zones
- `kanban-app/ui/src/components/data-table.tsx` — row Zone (after the `01KQM6VWQTK6KCQMQNKS0BX5V3` migration)
- `kanban-app/ui/src/components/fields/field.tsx` — `Field` mounts a `<FocusZone>`
- `kanban-app/ui/src/components/board-selector.tsx`
- `kanban-app/ui/src/components/inspectable.tsx`
- `kanban-app/ui/src/components/inspectors-container.tsx`
- Any other JSX callsite or import surfaced by grep

### Mechanical transformations

For every callsite:

- `<FocusZone moniker={...} ... />` → `<FocusScope moniker={...} ... />` (same prop set; FocusScope after sub-task A accepts the same prop shape).
- `import { FocusZone, FocusZoneProps } from "@/components/focus-zone"` → `import { FocusScope, FocusScopeProps } from "@/components/focus-scope"`.
- `import { FocusZoneContext, useParentZoneFq } from "@/components/focus-zone"` → use whatever context lives on `focus-scope.tsx` after sub-task A. If `useParentZoneFq` is renamed (e.g. `useParentScopeFq`), update the call. If the context already exists on `focus-scope.tsx` and just needs to be re-exported, add the export there.

### Files to delete

- `kanban-app/ui/src/components/focus-zone.tsx` — delete entirely. Move any unique exports (e.g. context types, hook names) into `focus-scope.tsx` first.

### Out of scope for this sub-task

- Test files — sub-task D handles all `.test.ts` / `.test.tsx` / `.spatial.test.tsx` / `.guards.node.test.ts` updates.
- Kernel-side changes — sub-task A.
- IPC bridge — sub-task B.
- README rewrite — sub-task D.

### Decision dependencies (already approved by user)

- `FocusScope` after this PR has every prop that `FocusZone` had today: `moniker`, `showFocusBar`, `className`, `children`, etc. Sub-task A handled the kernel-side struct; sub-task B handled the registration call shape; this sub-task's job is purely the JSX rename.

## Acceptance Criteria

- [ ] `kanban-app/ui/src/components/focus-zone.tsx` no longer exists.
- [ ] `grep -r "FocusZone" kanban-app/ui/src/components` returns no source-code matches (test files excluded — they're sub-task D).
- [ ] `grep -r "from \"@/components/focus-zone\"" kanban-app/ui/src` returns no matches.
- [ ] Every JSX `<FocusZone>` use is replaced with `<FocusScope>`.
- [ ] `pnpm -C kanban-app/ui exec tsc --noEmit` is clean across the entire component tree (test files may still fail at this point — that's sub-task D's territory).
- [ ] `pnpm -C kanban-app/ui exec vitest run --typecheck-only` (if available) or equivalent: clean for production code; test files may still error.
- [ ] No behaviour change in any component — same props, same rendering, same context wiring.

## Tests

- [ ] Existing component-level tests under `kanban-app/ui/src/components/*.test.tsx` will partially fail — DO NOT fix them in this sub-task. Sub-task D handles the test sweep.
- [ ] Run `pnpm -C kanban-app/ui exec tsc --noEmit` and confirm only test-file errors remain (production-code errors are zero).
- [ ] Document the surviving test-file errors in the implementer's report so sub-task D has a starting list.

## Workflow

- Land sub-task B first — this sub-task depends on the unified `useRegisterScope` adapter being in place.
- Use mechanical find/replace per file. Verify each file builds (typecheck) before moving to the next.
- Order: rename imports first across all files, then rename JSX, then delete `focus-zone.tsx` last.
- Do NOT touch `.test.tsx` / `.test.ts` files — sub-task D handles those.
- Do NOT rewrite the README — sub-task D handles that.
- If a callsite has unusual zone-specific behaviour (e.g. ref-forwarding patterns that differ between FocusScope and FocusZone in some edge case), STOP and report — do not improvise.
#spatial-nav-redesign