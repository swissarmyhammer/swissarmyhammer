---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffa780
project: spatial-nav
title: 'React primitives: Focusable, FocusZone, FocusLayer components (peer with Rust types)'
---
## What

Implement the three **React primitive components** that peer with the Rust types of the same name. These are thin DOM wrappers: each primitive generates its own branded key, reads parent contexts, registers with Rust via Tauri, and unregisters on unmount. No entity binding, no command scope — those live in the composite `FocusScope` wrapper (separate card).

### Peer mapping

| React component                       | Rust type                                         | Role                  |
|---------------------------------------|---------------------------------------------------|-----------------------|
| `<Focusable>` (`components/focusable.tsx`)  | `swissarmyhammer_focus::Focusable`   | Leaf focusable point  |
| `<FocusZone>` (`components/focus-zone.tsx`) | `swissarmyhammer_focus::FocusZone`   | Navigable container   |
| `<FocusLayer>` (`components/focus-layer.tsx`) | `swissarmyhammer_focus::FocusLayer` | Modal layer boundary  |

These three are **primitives**: each registers a single thing with Rust and provides one bit of context to descendants (parent layer / parent zone). They do **not** create a `CommandScope` — that's the composite `<FocusScope>` wrapper's job (separate card, `01KPZWZE5A...`). Use a primitive when the component is non-entity chrome (e.g. `<FocusZone moniker="ui:toolbar.actions">`) and doesn't need command dispatch.

Rust types are in `swissarmyhammer-focus/src/` (cards `01KNQXW7HH...` and `01KQ2E7RPBPJ8...`). Tauri commands are in `kanban-app/src/commands.rs`.

### Branded types (TypeScript)

First, define the TS types in `kanban-app/ui/src/types/spatial.ts` (shared with card `01KNM3YHHFJ3...`):

```typescript
export type WindowLabel = string & { readonly __tag: "WindowLabel" };
export type SpatialKey  = string & { readonly __tag: "SpatialKey" };
export type LayerKey    = string & { readonly __tag: "LayerKey" };
export type Moniker     = string & { readonly __tag: "Moniker" };
export type LayerName   = string & { readonly __tag: "LayerName" };
export type Pixels      = number & { readonly __tag: "Pixels" };

export const WindowLabel = (s: string) => s as WindowLabel;
export const SpatialKey  = (s: string) => s as SpatialKey;
export const LayerKey    = (s: string) => s as LayerKey;
export const Moniker     = (s: string) => s as Moniker;
export const LayerName   = (s: string) => s as LayerName;
export const Pixels      = (n: number) => n as Pixels;

export type Direction = "up" | "down" | "left" | "right" | "first" | "last" | "rowStart" | "rowEnd";

export type NavOverride = Partial<Record<Direction, Moniker | null>>;

export interface Rect {
  x: Pixels;
  y: Pixels;
  width: Pixels;
  height: Pixels;
}
```

### Contexts

```typescript
// FocusLayerContext — which layer are we inside?
export const FocusLayerContext = createContext<LayerKey | null>(null);
export const useCurrentLayerKey = (): LayerKey => {
  const k = useContext(FocusLayerContext);
  if (!k) throw new Error("FocusScope must be inside a <FocusLayer>");
  return k;
};

// FocusZoneContext — what's the nearest ancestor zone?
export const FocusZoneContext = createContext<SpatialKey | null>(null);
export const useParentZoneKey = (): SpatialKey | null => useContext(FocusZoneContext);
```

### `<Focusable>` primitive

```typescript
// kanban-app/ui/src/components/focusable.tsx
interface FocusableProps {
  moniker: Moniker;
  navOverride?: NavOverride;
  children: React.ReactNode;
  // Passthrough HTML attrs
  className?: string;
  style?: React.CSSProperties;
  // ... selected HTMLAttributes
}

export function Focusable({ moniker, navOverride, children, ...rest }: FocusableProps) {
  const key = useRef(SpatialKey(ulid())).current;
  const layerKey = useCurrentLayerKey();
  const parentZone = useParentZoneKey();   // null at layer root
  const [focused, setFocused] = useState(false);
  const ref = useRef<HTMLDivElement | null>(null);

  // Register claim callback
  useFocusClaim(key, setFocused);

  // Register rect with Rust on mount + ResizeObserver
  useEffect(() => {
    if (!ref.current) return;
    const ro = new ResizeObserver(() => registerRect());
    ro.observe(ref.current);
    registerRect();  // initial
    return () => {
      ro.disconnect();
      invoke("spatial_unregister_scope", { key });
    };
    function registerRect() {
      const r = ref.current!.getBoundingClientRect();
      invoke("spatial_register_focusable", {
        key,
        moniker,
        rect: { x: Pixels(r.x), y: Pixels(r.y), width: Pixels(r.width), height: Pixels(r.height) },
        layerKey,
        parentZone,
        overrides: navOverride ?? {},
      });
    }
  }, [key, moniker, layerKey, parentZone, navOverride]);

  return (
    <div
      ref={ref}
      data-moniker={moniker}
      data-focused={focused || undefined}
      onClick={() => invoke("spatial_focus", { key })}
      {...rest}
    >
      {children}
    </div>
  );
}
```

### `<FocusZone>` primitive

Same as `Focusable`, but:
- Calls `spatial_register_zone` instead of `spatial_register_focusable`
- Publishes its own `SpatialKey` via `FocusZoneContext.Provider` so descendants pick it up as their `parent_zone`

```typescript
return (
  <FocusZoneContext.Provider value={key}>
    <div ref={ref} data-zone-moniker={moniker} ...>{children}</div>
  </FocusZoneContext.Provider>
);
```

### `<FocusLayer>` primitive

```typescript
interface FocusLayerProps {
  name: LayerName;
  parentLayerKey?: LayerKey;   // optional override for portaled content
  children: React.ReactNode;
}

export function FocusLayer({ name, parentLayerKey, children }: FocusLayerProps) {
  const key = useRef(LayerKey(ulid())).current;
  const ancestorLayer = useContext(FocusLayerContext);  // may be null at window root
  const parent = parentLayerKey ?? ancestorLayer ?? null;

  useEffect(() => {
    invoke("spatial_push_layer", { key, name, parent });
    return () => { invoke("spatial_pop_layer", { key }); };
  }, [key, name, parent]);

  return <FocusLayerContext.Provider value={key}>{children}</FocusLayerContext.Provider>;
}
```

### Wire-format parity

Every Tauri invoke argument is a branded type. Since brands are erased at runtime, the JSON stays identical to what Rust expects (plain strings/numbers). TypeScript simply refuses to let a raw `string` or `number` sneak into an invoke call that expects a `Moniker`/`SpatialKey`/`Pixels`.

### Subtasks
- [x] Create `kanban-app/ui/src/types/spatial.ts` with branded types + brand helpers
- [x] Create `FocusLayerContext` and `FocusZoneContext`
- [x] Implement `<Focusable>` primitive with ResizeObserver and Tauri invokes
- [x] Implement `<FocusZone>` primitive, publishing its key to `FocusZoneContext`
- [x] Implement `<FocusLayer>` primitive with optional `parentLayerKey` prop
- [x] Implement `useFocusClaim(key, callback)` hook to integrate with the claim registry from card `01KNM3YHHFJ3...`
- [x] Wrap the App root (both main and quick-capture windows) in `<FocusLayer name={LayerName("window")}>`

## Acceptance Criteria
- [x] All three primitives exist as peer components with matching Rust type names
- [x] Every Tauri invoke uses branded types in its argument object; TS blocks unbranded strings/numbers at call sites
- [x] `Focusable` registers via `spatial_register_focusable`; `FocusZone` via `spatial_register_zone`; `FocusLayer` via `spatial_push_layer`
- [x] `FocusZone` publishes its `SpatialKey` via `FocusZoneContext` so descendants see the right `parent_zone`
- [x] `FocusLayer` resolves parent via (prop > context > null)
- [x] A primitive outside `<FocusLayer>` throws (fail-fast — every scope must live in a layer)
- [x] Existing `<FocusScope>` (composite) still works — it gets refactored in a follow-up card
- [x] `pnpm vitest run` passes

## Tests
- [x] `types/spatial.test.ts` — brand helpers produce typed values; raw strings rejected at invoke call sites (compile-time, verified via type-only tests or ts-expect-error)
- [x] `focusable.test.tsx` — mount calls `spatial_register_focusable` with branded args; unmount calls `spatial_unregister_scope`
- [x] `focus-zone.test.tsx` — mount calls `spatial_register_zone`; publishes key on FocusZoneContext so a child `Focusable` receives it as `parentZone`
- [x] `focus-layer.test.tsx` — mount generates a `LayerKey`; calls `spatial_push_layer` with (key, name, parent) where parent is explicit prop > ancestor context > null
- [x] `focus-layer.test.tsx` — root `<FocusLayer>` (no ancestor) sends `parent: null`
- [x] Integration: nested `<FocusLayer><FocusZone><Focusable>` registers three entries with correct hierarchy (`Focusable.parent_zone == FocusZone.key`, all `layer_key == FocusLayer.key`)
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-04-25 07:25)

### Blockers
- [x] `kanban-app/ui/src/components/focusable.tsx` (`onClick` handler) and `kanban-app/ui/src/components/focus-zone.tsx` (`onClick` handler) — neither click handler calls `e.stopPropagation()`. When a `<Focusable>` lives inside a `<FocusZone>` (the canonical layout per the integration test on line 235 of `focusable.test.tsx`), a click on the leaf bubbles to the zone and fires `spatial_focus` twice in sequence: once with the leaf key (correct), then with the zone key (wrong) — racing focus state and clobbering the user's intent. The pre-existing `<FocusScope>` calls `e.stopPropagation()` for exactly this reason (`focus-scope.tsx` line 173); the new primitives diverge from that established pattern. The comment in `focus-zone.tsx` even claims the zone fires "when nothing inside it absorbed the event", but nothing inside actually absorbs. Fix: add `e.stopPropagation()` after the input/contenteditable guards in both handlers, before the `focus(key)` call. Also add a regression test mounting `<FocusZone><Focusable></Focusable></FocusZone>`, clicking the leaf, and asserting exactly one `spatial_focus` invocation (with the leaf's key).
  - Resolved (2026-04-26): added `e.stopPropagation()` after the editable guards in both `focusable.tsx` and `focus-zone.tsx`. Added regression tests in `focusable.test.tsx` (leaf-inside-zone fires once) and `focus-zone.test.tsx` (inner-zone-inside-outer-zone fires once). 1455 frontend tests pass.

### Warnings
- [x] `kanban-app/ui/src/components/focusable.tsx` (`useEffect` dep list) and `kanban-app/ui/src/components/focus-zone.tsx` (`useEffect` dep list) — `navOverride` is read through `navOverrideRef.current` inside the effect but is intentionally omitted from the dep list. The ref captures the latest value on every render, but the *effect* only re-fires when one of (`key`, `moniker`, `layerKey`, `parentZone`, `registerFocusable`/`registerZone`, `unregisterScope`, `updateRect`) changes. If a caller mutates `navOverride` while those deps stay stable, the Rust-side overrides go stale until something else changes. The comment justifies this as "do not force unregister/re-register churn on every parent render" — but the current implementation silently drops the *value* updates as well, not just the identity churn. Pick one: (a) join `navOverride` to deps and accept the churn, (b) introduce a `spatial_update_overrides` Tauri command and call it when the ref's value differs from the last-pushed value, or (c) document explicitly that `navOverride` is read once on mount and per-identity-change of the other deps, and ignored otherwise.
  - Resolved (2026-04-26): chose option (c). The `navOverrideRef` is now accompanied by a multi-line block comment in both `focusable.tsx` and `focus-zone.tsx` that explicitly documents the contract — `navOverride` is snapshotted into the Rust registry only when the registration effect runs (mount, or when `key`/`moniker`/`layerKey`/`parentZone` flip identity), and mid-life value changes are intentionally ignored. The comment also tells callers what to do if they genuinely need overrides to flip on the fly (encode the variant into the moniker tail to force a re-register). Option (b) was deliberately rejected because it would require a new Rust-side Tauri command, which is out of scope for the React-primitives card and could conflict with parallel implementers touching the Rust crate.
- [x] `kanban-app/ui/src/App.tsx` — the acceptance criterion and subtask both say "Wrap the App root (both main and quick-capture windows) in `<FocusLayer name={LayerName(\"window\")}>`", and both checkboxes are checked. But `QuickCaptureApp` is not wrapped in `<SpatialFocusProvider>` or `<FocusLayer>` — only the main `App` is. Either wrap `QuickCaptureApp` to match the criterion (low risk: quick-capture's children don't currently use spatial primitives, so wrapping is harmless but future-proofs), or update the acceptance criterion to scope the requirement to the main window only.
  - Resolved (2026-04-26): wrapped `QuickCaptureApp` in `<SpatialFocusProvider>` + `<FocusLayer name={WINDOW_LAYER_NAME}>` to match the acceptance criterion. Reuses the same module-scope `WINDOW_LAYER_NAME` constant as the main `App` so the `LayerName` identity is stable. Updated the docstring to call out the wrapping and explain the future-proofing rationale.

### Nits
- [x] `kanban-app/ui/src/types/spatial.ts:42` — the doc comment says "ULID per instance" but the implementation in `focusable.tsx:95`, `focus-zone.tsx:107`, and `focus-layer.tsx:162` mints keys via `crypto.randomUUID()` (UUIDv4, not ULID). Functionally equivalent for the registry, but the doc and runtime disagree. Update either (preferred: drop "ULID" from the comment since the registry only requires uniqueness, and UUIDs are already in the standard browser API).
  - Resolved (2026-04-26): replaced the `SpatialKey` and `LayerKey` doc comments to drop the "ULID" wording, call out `crypto.randomUUID()` as the implementation, and explicitly note that the key shape is an implementation detail of the React primitives — the registry only requires uniqueness.
- [x] `kanban-app/ui/src/components/focusable.tsx:136` and `focus-zone.tsx:145` — the inner ResizeObserver callback re-reads `ref.current` and shadows the outer `node` binding. This is intentional (the observer fires asynchronously and `ref.current` may have been swapped) but slightly hard to follow without comment. A one-liner like "// re-read ref.current — observer fires async and the node may have changed" would help.
  - Resolved (2026-04-26): added explanatory comments inside the `ResizeObserver` callbacks in both files clarifying that the inner `ref.current` re-read is intentional (the observer fires asynchronously and the node may have been swapped between the initial register call and the resize callback).

## Review Findings (2026-04-26 07:39)

### Nits
- [x] `kanban-app/ui/src/components/focus-layer.tsx` (Lifecycle docstring near the top of the file) — the bullet still says "mints a fresh `LayerKey` (ULID per instance)" while the implementation uses `crypto.randomUUID()` (UUIDv4). The same wording was cleaned up in `spatial.ts` for the `SpatialKey` and `LayerKey` doc comments in the prior review pass, but this `<FocusLayer>` lifecycle bullet was missed. Suggested fix: drop "(ULID per instance)" — either delete the parenthetical or replace it with "(unique per instance)" — so the doc matches what the file actually does just below it.
  - Resolved (2026-04-26): updated the lifecycle bullet in `focus-layer.tsx` to read "mints a fresh `LayerKey` (unique per instance via `crypto.randomUUID()`)" — matches the `crypto.randomUUID()` wording that `spatial.ts` adopted for `SpatialKey` and `LayerKey` in the prior pass, so the docstring no longer disagrees with the call on the next line.
