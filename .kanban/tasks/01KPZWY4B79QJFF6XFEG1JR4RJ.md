---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
position_column: todo
position_ordinal: ff8c80
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
- [ ] Create `kanban-app/ui/src/types/spatial.ts` with branded types + brand helpers
- [ ] Create `FocusLayerContext` and `FocusZoneContext`
- [ ] Implement `<Focusable>` primitive with ResizeObserver and Tauri invokes
- [ ] Implement `<FocusZone>` primitive, publishing its key to `FocusZoneContext`
- [ ] Implement `<FocusLayer>` primitive with optional `parentLayerKey` prop
- [ ] Implement `useFocusClaim(key, callback)` hook to integrate with the claim registry from card `01KNM3YHHFJ3...`
- [ ] Wrap the App root (both main and quick-capture windows) in `<FocusLayer name={LayerName("window")}>`

## Acceptance Criteria
- [ ] All three primitives exist as peer components with matching Rust type names
- [ ] Every Tauri invoke uses branded types in its argument object; TS blocks unbranded strings/numbers at call sites
- [ ] `Focusable` registers via `spatial_register_focusable`; `FocusZone` via `spatial_register_zone`; `FocusLayer` via `spatial_push_layer`
- [ ] `FocusZone` publishes its `SpatialKey` via `FocusZoneContext` so descendants see the right `parent_zone`
- [ ] `FocusLayer` resolves parent via (prop > context > null)
- [ ] A primitive outside `<FocusLayer>` throws (fail-fast — every scope must live in a layer)
- [ ] Existing `<FocusScope>` (composite) still works — it gets refactored in a follow-up card
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `types/spatial.test.ts` — brand helpers produce typed values; raw strings rejected at invoke call sites (compile-time, verified via type-only tests or ts-expect-error)
- [ ] `focusable.test.tsx` — mount calls `spatial_register_focusable` with branded args; unmount calls `spatial_unregister_scope`
- [ ] `focus-zone.test.tsx` — mount calls `spatial_register_zone`; publishes key on FocusZoneContext so a child `Focusable` receives it as `parentZone`
- [ ] `focus-layer.test.tsx` — mount generates a ULID `LayerKey`; calls `spatial_push_layer` with (key, name, parent) where parent is explicit prop > ancestor context > null
- [ ] `focus-layer.test.tsx` — root `<FocusLayer>` (no ancestor) sends `parent: null`
- [ ] Integration: nested `<FocusLayer><FocusZone><Focusable>` registers three entries with correct hierarchy (`Focusable.parent_zone == FocusZone.key`, all `layer_key == FocusLayer.key`)
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.