---
assignees:
- claude-code
depends_on:
- 01KPZWY4B79QJFF6XFEG1JR4RJ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffab80
project: spatial-nav
title: 'FocusScope wrapper refactor: entity-aware composite over the primitives'
---
## What

Refactor the existing `<FocusScope>` (in `kanban-app/ui/src/components/focus-scope.tsx`) so that it composes the primitives (`<Focusable>` or `<FocusZone>`) added in the primitives card. `FocusScope` keeps its existing entity-oriented responsibilities — CommandScopeProvider, click-to-focus routing, data attributes, context menu, focus bar — but delegates **registration / rect measurement / keys** to whichever primitive fits.

### Peer-type summary

| Rust type           | React component       | Registration           |
|---------------------|-----------------------|------------------------|
| `struct Focusable`  | `<Focusable>`         | leaf, direct           |
| `struct FocusZone`  | `<FocusZone>`         | zone, direct           |
| `struct FocusLayer` | `<FocusLayer>`        | layer, direct          |
| `enum FocusScope`   | `<FocusScope>`        | composite → wraps primitive |

On the Rust side, `enum FocusScope` is the stored sum type. On React, `<FocusScope>` is the entity-aware wrapper most call sites use. Direct use of primitives is also fine for non-entity chrome.

See the canonical terminology section in the kernel card `01KNQXW7HH...` for definitions of Layer / Zone / Focusable / Scope. In short: this composite `<FocusScope>` is what you reach for when wrapping an entity (task, column, field row); its job is to produce **both** a spatial entry (Focusable or Zone, via the primitive it composes) **and** a `CommandScope` (the command-dispatch boundary — a separate, pre-existing concept). Primitives produce only the spatial entry.

### Props

```typescript
import type { Moniker, NavOverride } from "@/types/spatial";

interface FocusScopeProps {
  moniker: Moniker;
  kind?: "leaf" | "zone";          // default "leaf"
  navOverride?: NavOverride;
  commands?: CommandDef[];          // existing — passed to CommandScopeProvider
  showFocusBar?: boolean;           // existing
  handleEvents?: boolean;           // existing
  children: React.ReactNode;
  // plus existing passthrough HTMLAttributes
}
```

### Structure

```typescript
export function FocusScope({ moniker, kind = "leaf", navOverride, commands, ...rest }: FocusScopeProps) {
  const Primitive = kind === "zone" ? FocusZone : Focusable;

  return (
    <FocusScopeContext.Provider value={moniker}>
      <CommandScopeProvider commands={commands ?? EMPTY_COMMANDS}>
        <Primitive moniker={moniker} navOverride={navOverride} {...rest}>
          {/* existing focus-bar / context-menu chrome */}
          <FocusHighlight>{children}</FocusHighlight>
        </Primitive>
      </CommandScopeProvider>
    </FocusScopeContext.Provider>
  );
}
```

The primitive handles: key generation, rect measurement, Tauri registration, claim callback, data-moniker/data-focused attrs, click-to-focus. `FocusScope` stacks the entity plumbing on top.

### What moves where

| Concern                                     | Before (in FocusScope) | After                                   |
|---------------------------------------------|------------------------|-----------------------------------------|
| ULID key generation                         | FocusScope             | Primitive                               |
| ResizeObserver + `spatial_register_*`       | (did not exist)        | Primitive                               |
| Unregister on unmount                       | (did not exist)        | Primitive                               |
| data-moniker / data-focused attrs           | FocusScope             | Primitive                               |
| Click → focus dispatch                      | FocusScope             | Primitive (`spatial_focus` invoke)      |
| CommandScopeProvider                        | FocusScope             | FocusScope (unchanged)                  |
| FocusHighlight / focus bar visual           | FocusScope             | FocusScope (unchanged)                  |
| Context menu                                | FocusScope             | FocusScope (unchanged)                  |
| Legacy `claimWhen` predicate registration   | FocusScope             | **Deleted** (see card `01KNQY1GQ9...`)  |
| `claimWhen` → `navOverride` rename          | —                      | **FocusScope passes through to primitive** |

### Usage parity

After refactor, every existing call site of `<FocusScope moniker="...">` keeps working. Zone-ness is opt-in via the new `kind` prop:

```tsx
// Leaf — default, unchanged for existing call sites
<FocusScope moniker={Moniker(`task:${id}.title`)}>
  {title}
</FocusScope>

// Zone — new, for columns, cards, field rows
<FocusScope moniker={Moniker(`task:${id}`)} kind="zone">
  <TaskCardBody />
</FocusScope>

// Non-entity chrome can skip FocusScope entirely and use primitives:
<FocusZone moniker={Moniker("ui:toolbar.actions")}>
  <FiltersButton />
  <NewTaskButton />
</FocusZone>
```

### Subtasks
- [x] Add `kind: "leaf" | "zone"` prop to FocusScope (default `"leaf"`)
- [x] Internally delegate to `<Focusable>` or `<FocusZone>` based on `kind`
- [x] Remove key-generation, ResizeObserver, and registration logic from FocusScope (now in primitives)
- [x] Keep CommandScopeProvider, FocusHighlight, context menu, entity-focus behaviors
- [x] Pass `navOverride` through to the primitive
- [x] Ensure existing `<FocusScope moniker="...">` call sites compile and behave the same as before (default kind = leaf)
- [x] Delete any `claimWhen` wiring that the primitives don't cover (final removal happens in card `01KNQY1GQ9...`)

## Acceptance Criteria
- [x] `<FocusScope>` wraps exactly one primitive based on `kind`
- [x] Every existing call site of `<FocusScope>` compiles unchanged (default kind is `"leaf"`)
- [x] `moniker` prop is typed as `Moniker` (branded); TS rejects raw `string` at call sites
- [x] CommandScopeProvider, context menu, FocusHighlight behaviors unchanged
- [x] Click-to-focus now goes through the primitive's `spatial_focus` invoke (not the old `setFocus(moniker)` path)
- [x] No duplicate registration — primitive registers once, FocusScope does not register separately
- [x] `pnpm vitest run` passes

## Tests
- [x] `focus-scope.test.tsx` — `<FocusScope moniker={Moniker(...)}>` renders a `<Focusable>` when `kind` omitted
- [x] `focus-scope.test.tsx` — `<FocusScope kind="zone">` renders a `<FocusZone>`
- [x] `focus-scope.test.tsx` — `navOverride` prop forwarded to the primitive
- [x] `focus-scope.test.tsx` — CommandScopeProvider sees `commands` prop as before
- [x] `focus-scope.test.tsx` — click on the rendered element invokes `spatial_focus` with the primitive's key
- [x] Existing tests in `focus-scope.test.tsx` still pass without modification beyond type updates
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Implementation Notes (2026-04-26)

The implementation was already in place in working tree when this task was picked up. Verification work:

1. Verified `focus-scope.tsx` correctly composes `<Focusable>` (kind="leaf") or `<FocusZone>` (kind="zone"), with a no-FocusLayer fallback to a plain div. Implementation matches spec.
2. Added five new tests in `focus-scope.test.tsx` under a `primitive composition` describe block:
   - renders `<Focusable>` when `kind` is omitted (verifies `spatial_register_focusable` call)
   - renders `<FocusZone>` when `kind="zone"` (verifies `spatial_register_zone` call)
   - forwards `navOverride` to the primitive registration's `overrides` field
   - click invokes `spatial_focus` with the primitive's minted key
   - falls back to a plain div when no `<FocusLayer>` ancestor is mounted
3. The existing 25 tests still pass with only type-update changes (raw string monikers wrapped with `asMoniker(...)`).
4. Updated all call sites listed in the task to use `asMoniker(...)`:
   - `entity-card.tsx`, `column-view.tsx`, `data-table.tsx`, `entity-inspector.tsx`, `mention-view.tsx`, `inspector-focus-bridge.tsx`, `command-palette.tsx`, `avatar.tsx`, `board-view.tsx`, `attachment-display.tsx`
5. Updated test files: `app-shell.test.tsx`, `attachment-display.test.tsx`, `badge-list-nav.test.tsx`, `mention-view.test.tsx`, `entity-focus-context.test.tsx`.
6. `entity-card.tsx` simplified: switched from `<FocusScope>` + `SpatialZoneIfAvailable` (which double-registered as both leaf and zone in production) to `<FocusScope kind="zone">`. The `<FocusScope>` composite now does the right thing in both production (composes `<FocusZone>`) and test isolation (falls back to plain div without a `<FocusLayer>` ancestor) — no manual zone-availability check needed. Removes a workaround the new composite eliminated.
7. `store-container.tsx` switched from `<FocusScope renderContainer={false}>` to `<CommandScopeProvider>`. The store scope is purely structural (contributes a moniker to the scope chain so the backend resolves the store path) — there is no entity to focus, no spatial-nav rect, and no DOM surface. Sibling structural containers (`AppModeContainer`, `WindowContainer`, `BoardContainer`) follow the same pattern. This also avoids transitively pulling the spatial primitives' Tauri-event imports into tests that mount `StoreContainer` in isolation.

Test result: 138 test files, 1513 tests, all passing. `tsc --noEmit` clean.

## Review Findings (2026-04-26 08:53)

### Nits
- [x] `kanban-app/ui/src/components/data-table.tsx:830` — Doc comment on `GridCellScope` claims `FocusScope` "renders as a `<td>` element (via the underlying FocusHighlight)". Both clauses are now stale: this refactor replaced the inner `FocusHighlight` with an in-file `FocusScopeBody` (which deliberately does not emit `data-focused` so the primitive owns it), and `FocusScope` does not render as a `<td>` — it renders inside the `TableCell`. Suggest: drop the parenthetical and reword to something like "Wraps `children` in a `FocusScope` mounted inside a `TableCell` so the row/cell carries the entity-focus + claim-predicate plumbing without breaking table HTML structure."
- [x] `kanban-app/ui/src/components/store-container.test.tsx:71,80` — Test name `"does not render a wrapping container div (renderContainer=false)"` and the inline comment "FocusScope with renderContainer=false should not add a FocusHighlight wrapper" both refer to the pre-refactor implementation. `StoreContainer` now uses `CommandScopeProvider` directly, so there is no `renderContainer` flag and no `FocusHighlight` involved. The assertion (`querySelector("[data-moniker]") === null`) still holds for the right reason — `CommandScopeProvider` is a pure context provider — so the test is still meaningful, but the wording should be updated to reflect the new structure (e.g. `"renders no DOM wrapper — CommandScopeProvider is context-only"`).

### Nit Resolution (2026-04-26 08:58)

Both nits addressed:

1. **`data-table.tsx`** — Replaced the stale doc comment on `GridCellScope` with the suggested wording: "Wraps `children` in a `FocusScope` mounted inside a `TableCell` so the row/cell carries the entity-focus + claim-predicate plumbing without breaking table HTML structure." Param tags retained.
2. **`store-container.test.tsx`** — Renamed the test to `"renders no DOM wrapper — CommandScopeProvider is context-only"` and rewrote the inline comment to reflect that `StoreContainer` is now a pure context provider (`CommandScopeProvider`), not a `FocusScope` with `renderContainer=false`. The `querySelector("[data-moniker]") === null` assertion is unchanged — it still asserts the right thing, just with accurate justification.

Verification:
- `npx vitest run src/components/store-container.test.tsx src/components/data-table.test.tsx` — 14 / 14 pass.
- `npx vitest run` (full UI suite) — 138 files, 1513 tests, all passing (no regression).
- `npx tsc --noEmit` — clean.