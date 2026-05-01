---
assignees:
- claude-code
depends_on:
- 01KQ7GM77B1E6YH8Z893K05VKY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffcc80
project: spatial-nav
title: 'Refactor: replace inspectOnDoubleClick prop with &lt;Inspectable&gt; wrapper component'
---
## What

Once the `inspectOnDoubleClick?: boolean` fix from `01KQ7GM77B1E6YH8Z893K05VKY` lands and stops the perspective-tab-opens-inspector bug, replace that boolean prop with a dedicated `<Inspectable>` wrapper component. The boolean is the right user-visible fix; the wrapper is the right architecture.

### Why this is its own task

The boolean prop on `<FocusScope>` / `<FocusZone>` works, but it has design smells that the wrapper-component approach eliminates:

- **Domain leakage.** `<FocusScope>` and `<FocusZone>` are generic spatial primitives. Carrying an `inspectOnDoubleClick` prop means the primitive imports `useDispatchCommand("ui.inspect")` — kanban-domain knowledge — into the spatial-nav infrastructure.
- **Implicit naming.** `inspectOnDoubleClick={true}` on a `<FocusScope moniker="task:01">` is a flag whose meaning is buried in conventions. `<Inspectable moniker="task:01">` *names* the architectural concept ("this DOM subtree is an inspectable entity").
- **Composability.** A future change that needs both inspect-on-dblclick and a focus zone (e.g. an editable card) gets `<Inspectable><FocusZone>…</FocusZone></Inspectable>` cleanly. The boolean has to be duplicated on both `<FocusScope>` AND `<FocusZone>`, with no obvious answer to "what if both nest?"
- **Type-system enforcement.** A wrapper component can demand a branded `EntityMoniker` newtype that excludes `ui:*`, `cell:*`, `perspective_tab:*` prefixes at compile time. A boolean offers no such enforcement.
- **Cost on chrome.** A `<FocusScope>` with no `<Inspectable>` ancestor never registers `useDispatchCommand("ui.inspect")` at all — the registry walk is paid once per inspectable entity, not once per focusable.

### Implementation notes

`<Inspectable>` is built on a `useInspectOnDoubleClick(moniker)` hook so that consumers whose host element cannot be a `<div>` (table rows under `<tbody>`) can attach the same handler directly. Both share the same `useDispatchCommand("ui.inspect")` call site (`inspectable.tsx`), so Guard A's "one non-test file owns the dispatch" contract still holds. The data-table row uses the hook directly because DOM rules forbid `<div>` between `<tbody>` and `<tr>`.

Guard A is enforced as "the inspect-on-double-click *dispatch route* is single-sourced". An explicit allowlist permits the four production callers that dispatch `ui.inspect` from non-double-click sources (keyboard inspect command in `board-view.tsx`, the navbar Inspect button in `nav-bar.tsx`, the command-palette Inspect row in `command-palette.tsx`, and the card's `<InspectButton>` "i" icon in `entity-card.tsx`). Each is documented in the guard with a comment naming its gesture.

### Files in scope

**New file:**
- `kanban-app/ui/src/components/inspectable.tsx` — exports `<Inspectable moniker: Moniker>` and the sibling hook `useInspectOnDoubleClick(moniker)`.

**Primitive contract change (remove inspect logic introduced by the prerequisite task):**
- `kanban-app/ui/src/components/focus-scope.tsx` — `inspectOnDoubleClick` prop, the `useDispatchCommand("ui.inspect")` registration, the `<InspectDoubleClickRegistrar>` child, and the `handleDoubleClick` callbacks all removed. Docstring updated to "FocusScope is a pure spatial primitive. Inspector dispatch lives in `<Inspectable>` — see `inspectable.tsx`."
- `kanban-app/ui/src/components/focus-zone.tsx` — symmetric removal.

**Entity call sites — migrated from `inspectOnDoubleClick` prop to `<Inspectable>` wrapper:**
- `entity-card.tsx`, `column-view.tsx`, `board-view.tsx`, `fields/field.tsx`, `mention-view.tsx`, `fields/displays/attachment-display.tsx` — wrapped in `<Inspectable>`.
- `data-table.tsx` — `EntityRow` calls `useInspectOnDoubleClick` directly because DOM table rules prevent `<div>` between `<tbody>` and `<tr>`.

**Architectural guard — extended `focus-architecture.guards.node.test.ts`:**
- Guard A — single dispatch site; explicit allowlist for non-double-click callers.
- Guard B — `<Inspectable>` only wraps entity monikers.
- Guard C — entity-monikered `<FocusScope>` / `<FocusZone>` calls have an `<Inspectable>` element with matching prefix in the same file. Per-call-site `// inspect:exempt` comment is the documented escape hatch; `renderContainer={false}` is also exempt because it doesn't render a DOM element.
- The prerequisite task's CHROME-prefix and entity-prefix guards are deleted — subsumed by Guards B and C.
- `grep -r "inspectOnDoubleClick" kanban-app/ui/src/` returns zero matches outside docstrings explaining the migration.

### What this task does NOT do

- Does not re-litigate the per-entity inspect contract — same entities (task, tag, column, board, field, attachment) inspect on dblclick. Only the *plumbing* changes.
- Does not touch the keyboard inspect path or context-menu inspect — only the double-click route.
- Does not introduce new monikers.
- Does not introduce the optional `EntityMoniker` brand — deferred per the original card. Runtime guards (B + C) are the enforcement.

## Acceptance Criteria

- [x] `kanban-app/ui/src/components/inspectable.tsx` exists, exports `<Inspectable moniker: Moniker>` (plus sibling hook `useInspectOnDoubleClick`), has a complete file-level docstring.
- [x] `<FocusScope>` and `<FocusZone>` no longer carry an `inspectOnDoubleClick` prop. `grep -r "inspectOnDoubleClick" kanban-app/ui/src/` returns zero matches outside the deleted-strings guard / migration docstrings.
- [x] `useDispatchCommand("ui.inspect")` appears in exactly one non-test file owning the double-click route: `inspectable.tsx`. Other production callers (board-view, nav-bar, command-palette, entity-card) own non-double-click gestures and are explicitly allowlisted in Guard A.
- [x] Double-clicking a perspective tab still does NOT open the inspector (regression preserved — `perspective-tab-bar.no-inspect-on-dblclick.spatial.test.tsx` still green).
- [x] Double-clicking any `ui:*` chrome zone or leaf still does NOT open the inspector.
- [x] Double-clicking a card, column, field row DOES open the inspector — preserved via `<Inspectable>` wrappers (and via `useInspectOnDoubleClick` on the data-table row's `<tr>`).
- [x] All three guards in `focus-architecture.guards.node.test.ts` pass: single dispatch site, entity-only Inspectable monikers, entity-monikered focusables have an Inspectable in the same file.
- [x] `cd kanban-app/ui && npm test` is green — 1718/1718 (down 5 from 1723 because 16 prerequisite-task inspect tests were replaced with 11 wrapper-aware tests).
- [x] `cargo test -p swissarmyhammer-kanban` and `cargo test -p swissarmyhammer-commands` are green (no Rust changes).

## Tests

### Browser Tests (mandatory)

Run under Vitest browser mode (`vitest-browser-react` + Playwright Chromium).

#### Test files

1. `kanban-app/ui/src/components/inspectable.spatial.test.tsx` (new) — exercises the wrapper component (8 cases — all listed in the original card description).
2. `perspective-tab-bar.no-inspect-on-dblclick.spatial.test.tsx` updated — docstring rewritten to reference both cards; assertions unchanged because they pin user-visible behavior.
3. `focus-scope.inspect-opt-in.spatial.test.tsx` deleted — the eight `<Inspectable>` cases plus Guards A/B/C cover the same ground at the right architectural level.
4. `focus-architecture.guards.node.test.ts` extended with Guards A–C (and the prerequisite task's two opt-in guards deleted).
5. `focus-scope.test.tsx` legacy unit tests for inspect-on-double-click deleted — the dispatch is no longer a `<FocusScope>` concern; the new file covers the wrapper.

### How to run

```
cd kanban-app/ui && npm test
```

Headless on CI.
