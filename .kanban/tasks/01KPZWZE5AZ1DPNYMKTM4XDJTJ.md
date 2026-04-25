---
assignees:
- claude-code
depends_on:
- 01KPZWY4B79QJFF6XFEG1JR4RJ
position_column: todo
position_ordinal: ff8d80
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
- [ ] Add `kind: "leaf" | "zone"` prop to FocusScope (default `"leaf"`)
- [ ] Internally delegate to `<Focusable>` or `<FocusZone>` based on `kind`
- [ ] Remove key-generation, ResizeObserver, and registration logic from FocusScope (now in primitives)
- [ ] Keep CommandScopeProvider, FocusHighlight, context menu, entity-focus behaviors
- [ ] Pass `navOverride` through to the primitive
- [ ] Ensure existing `<FocusScope moniker="...">` call sites compile and behave the same as before (default kind = leaf)
- [ ] Delete any `claimWhen` wiring that the primitives don't cover (final removal happens in card `01KNQY1GQ9...`)

## Acceptance Criteria
- [ ] `<FocusScope>` wraps exactly one primitive based on `kind`
- [ ] Every existing call site of `<FocusScope>` compiles unchanged (default kind is `"leaf"`)
- [ ] `moniker` prop is typed as `Moniker` (branded); TS rejects raw `string` at call sites
- [ ] CommandScopeProvider, context menu, FocusHighlight behaviors unchanged
- [ ] Click-to-focus now goes through the primitive's `spatial_focus` invoke (not the old `setFocus(moniker)` path)
- [ ] No duplicate registration — primitive registers once, FocusScope does not register separately
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `focus-scope.test.tsx` — `<FocusScope moniker={Moniker(...)}>` renders a `<Focusable>` when `kind` omitted
- [ ] `focus-scope.test.tsx` — `<FocusScope kind="zone">` renders a `<FocusZone>`
- [ ] `focus-scope.test.tsx` — `navOverride` prop forwarded to the primitive
- [ ] `focus-scope.test.tsx` — CommandScopeProvider sees `commands` prop as before
- [ ] `focus-scope.test.tsx` — click on the rendered element invokes `spatial_focus` with the primitive's key
- [ ] Existing tests in `focus-scope.test.tsx` still pass without modification beyond type updates
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.