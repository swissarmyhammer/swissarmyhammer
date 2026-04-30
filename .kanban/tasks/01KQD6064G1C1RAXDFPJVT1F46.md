---
assignees:
- claude-code
depends_on:
- 01KQD8X3PYXQAJN593HR11T7R4
- 01KQD8XM2T0FWHXANCK0KVDJH1
- 01KQD8Y496CCACWN656SNFTRT8
position_column: todo
position_ordinal: a980
project: spatial-nav
title: 'Path monikers as spatial keys: SegmentMoniker (input) + FullyQualifiedMoniker (key); collapse SpatialKey/Moniker into one identifier'
---
## What

The spatial-nav kernel uses **one** identifier shape per primitive: a fully-qualified path through the focus hierarchy. The path IS the spatial key. The current dual identifier — UUID-based `SpatialKey` plus string `Moniker` — collapses into a single newtype.

Two newtypes:

1. **`SegmentMoniker`** — what consumers pass when constructing a `<FocusLayer>`, `<FocusZone>`, or `<FocusScope>`. A relative path segment: `"field:T1.title"`, `"card:T1"`, `"inspector"`. Consumers only declare the segment.

2. **`FullyQualifiedMoniker`** (the kernel's canonical identity, AND the spatial key) — the path: `/window/inspector/field:T1.title`. Constructed by appending segments through nested React context. Available to consumers via `useFullyQualifiedMoniker()`. Used by every focus dispatch.

User direction (collected across messages):

> "the actual effective moniker needs to be a fully qualified 'path' not just the leaf — when we focus a thing it is like /window/layer/zone.../scope instead of pretending the identifier of the scope is unique."

> "when you make a scope or zone — you make a 'relative moniker' — but each needs to have a fully qualified moniker available via hook, and when you focus via moniker it needs to be fully qualified."

> "this also means the spatial key can be — IS — the fully qualified path."

> "you can eliminate this dual identifier 'problem'."

> "make sure it is a newtype though — something like FullyQualifiedMoniker and SegmentMoniker."

> "on a given FocusScope node in react I only need to declare the relative path — the fully qualified can and will be constructed via nesting context."

## Concrete bug confirmed in the running app's log

After opening an inspector on `task:01KQAWVDS931PADB0559F2TVCS`:

```
duplicate moniker registered against two distinct keys —
spatial_focus_by_moniker will resolve non-deterministically
moniker=field:task:01KQAWVDS931PADB0559F2TVCS.title
op="find_by_moniker"
first_key=4ab0e988-9d12-48c6-94f3-2bfb783a7fdb
second_key=0fac21b6-2849-4661-ae49-222bf08149e2
```

The card on the board AND the inspector panel both register `<FocusZone moniker="field:T1.title">`. The kernel sees the same flat moniker registered against two different UUID keys. `find_by_moniker` picks one non-deterministically (the BOARD's). Focus advances into the board. ArrowDown cascades on the board. User sees "nav spilled out of the inspector". Path-as-key eliminates this structurally.

## API surface

### Constructing a primitive — segment only

Consumers declare only the segment. Nesting context constructs the FQM.

```tsx
<FocusLayer name="window">                        // segment = "window"; FQM = "/window"
  <FocusZone moniker="board">                     // segment = "board"; FQM = "/window/board"
    <FocusZone moniker="column:todo">             // segment = "column:todo"; FQM = "/window/board/column:todo"
      <FocusZone moniker="card:T1">               // FQM = "/window/board/column:todo/card:T1"
        <FocusZone moniker="field:title">         // FQM = "/window/board/column:todo/card:T1/field:title"
          ...
```

```tsx
<FocusLayer name="inspector">                     // FQM = "/window/inspector"
  <FocusZone moniker="field:T1.title">            // FQM = "/window/inspector/field:T1.title"
                                                  //       ↑ DIFFERENT path from board's title field
```

Two different FQMs for the "same" logical entity. Kernel disambiguates by FQM. No more duplicates.

### Reading the FQM — hook

```ts
function useFullyQualifiedMoniker(): FullyQualifiedMoniker
```

Reads from `FullyQualifiedMonikerContext`, which every spatial primitive provides for its descendants. Throws when called outside a primitive.

Companion utility for pre-mount composition (e.g., when the parent wants to compute a child's FQM before the child renders):

```ts
function composeFq(parent: FullyQualifiedMoniker, child: SegmentMoniker): FullyQualifiedMoniker
```

### Focus dispatch — strict FQM

```ts
setFocus(moniker: FullyQualifiedMoniker | null): void
spatial_focus_by_moniker(moniker: FullyQualifiedMoniker): Promise<void>
```

A `SegmentMoniker` passed to `setFocus` is a **TS compile error**. The TS type system is the safety net. Same on Rust side: `find_by_fq(&FullyQualifiedMoniker) -> Option<&RegisteredScope>` is the only lookup; takes only FQM.

No leaf-form fallback. No topmost-layer heuristic. The path is the key, and the key is exact-match.

### IPC — single identifier

`focus-changed` event payload carries just the FQM. No separate `key` field. The FQM IS the key.

`spatial_register_zone` IPC accepts the FQM as the identifier — the React side has it from context, so it sends the composed value:

```ts
invoke("spatial_register_zone", {
  moniker: useFullyQualifiedMoniker(),  // FQM, computed in React from nesting context
  parentZoneFq: parentZoneFqOrNull,
  layerFq: layerFq,
  rect, overrides,
})
```

The kernel stores entries keyed by FQM. No UUID generation. No `crypto.randomUUID()` on the React side for spatial keys.

## Approach — TDD

### Layer 1 — Rust kernel tests (cargo)

`swissarmyhammer-focus/tests/path_monikers.rs` (new file).

- [ ] `register_zone_keyed_by_fq_moniker` — register zone at `/window/inspector/field:T1.title`; assert `find_by_fq("/window/inspector/field:T1.title")` returns it.
- [ ] `two_zones_same_segment_different_layers_have_distinct_fq_keys` — register `field:T1.title` once at `/window/board/.../card:T1/` and once at `/window/inspector/`. Assert both findable, both stored as distinct entries (no duplicate-fq warning).
- [ ] `find_by_fq_unknown_path_returns_none_and_traces_error` — per the no-silent-dropout contract.
- [ ] `cascade_does_not_cross_layers` — register the duplicate fields, focus the inspector's, dispatch `next(... Down)`, no candidate from the board's FQM appears.
- [ ] `segment_moniker_does_not_compile_at_fq_lookup_callsite` — the TS-side analog asserts a compile error when a `SegmentMoniker` is passed to `setFocus`. The Rust analog: `find_by_fq(SegmentMoniker(...))` does not compile. Use type-tagged newtypes.
- [ ] `register_with_duplicate_fq_logs_error_and_replaces` — same FQM registered twice → `tracing::error!` flags the bug-class (a real duplicate is a programmer mistake), replaces. Mirrors today's "replaces any prior scope under the same key" behavior on the new identifier.

### Layer 2 — React kernel-driven tests (vitest browser)

`kanban-app/ui/src/components/path-monikers.kernel-driven.browser.test.tsx` (new file).

- [ ] `inspector_field_zone_fq_matches_inspector_layer_path` — mount production tree with open inspector; capture `useFullyQualifiedMoniker()` from inside the inspector's title field's render via probe; assert it equals `/window/inspector/field:T1.title` (or whatever the actual layer name resolves to).
- [ ] `card_field_zone_fq_matches_board_path` — same scene; assert the board's card-title FQM matches the deep board path.
- [ ] `useFullyQualifiedMoniker_outside_primitive_throws` — render the hook outside any layer/zone/scope; assert error.
- [ ] `composeFq_appends_segment_with_slash` — utility test.
- [ ] `setFocus_with_fq_moniker_advances_kernel_focus` — call `setFocus("/window/inspector/field:T1.title")`; assert kernel-simulator's focused entry is that FQM.
- [ ] `setFocus_with_segment_moniker_is_compile_error` — TS-level assertion via tsc.
- [ ] `no_duplicate_moniker_warning_when_inspector_opens` — open inspector over board; capture warnings from simulator; assert zero "duplicate moniker" warnings.

### Layer 3 — manual log verification (mandatory before done)

Run `npm run tauri dev`:

- [ ] Open an inspector on a task. `log show --last 1m --predicate 'subsystem == "com.swissarmyhammer.kanban"' --info --debug | grep duplicate` — assert zero output.
- [ ] Click a field in the inspector. Press ArrowDown. Same log query — zero duplicate warnings AND subsequent `ui.setFocus` `scope_chain` log lines contain only paths starting with `/window/inspector/...`.
- [ ] Press ArrowDown at the last field. Focus echoes. No `card:*` / `column:*` paths appear.
- [ ] Press Escape. Inspector closes. Focus restores via `last_focused`.

Layer 3 is mandatory before declaring done. Tests-pass-but-production-broken has happened twice on this surface.

## Implementation outline

### Step 1 — Rust kernel: collapse SpatialKey + Moniker into FQM

`swissarmyhammer-focus/src/types.rs` (or wherever `Moniker` lives today):

- Define `SegmentMoniker(String)` — what consumers pass.
- Define `FullyQualifiedMoniker(String)` — the path, the canonical key.
- `FullyQualifiedMoniker::compose(parent: &FullyQualifiedMoniker, segment: &SegmentMoniker) -> FullyQualifiedMoniker`.
- Delete `SpatialKey` (the UUID type). The FQM replaces it everywhere.
- All places that today take `SpatialKey` take `&FullyQualifiedMoniker`.

### Step 2 — Kernel registry

`swissarmyhammer-focus/src/registry.rs`:

- `RegisteredScope` carries the `fq` plus the original `segment` (for human-readable logging only).
- The internal `HashMap` is keyed by `FullyQualifiedMoniker`, not by UUID.
- `register_zone(fq, segment, parent_fq, layer_fq, rect, overrides)` — kernel just inserts. The React side composed the FQM.
- `register_scope` similarly.
- `push_layer(fq, segment, parent_fq)` — same shape.
- `find_by_fq(&FullyQualifiedMoniker) -> Option<&RegisteredScope>` — exact match.

### Step 3 — Kernel cascade & focus

`swissarmyhammer-focus/src/state.rs`:

- `SpatialState` tracks the focused FQM (no UUID).
- `focus(fq)` sets the focused FQM.
- `clear_focus()` — same as before, just operates on FQMs.
- The cascade in `navigate.rs` already works on registered entries; just swap UUIDs for FQMs as the entry identifier.
- `focus-changed` event payload: `{ fq_moniker: FullyQualifiedMoniker | null, segment_moniker: SegmentMoniker | null }`.

### Step 4 — Tauri command boundary

`kanban-app/src/commands.rs`:

- `spatial_register_zone(fq, segment, parent_fq, layer_fq, rect, overrides)`.
- `spatial_unregister_scope(fq)`.
- `spatial_focus(fq)`.
- `spatial_navigate(focused_fq, direction)`.
- `spatial_focus_by_moniker` — REDUNDANT with `spatial_focus(fq)`. Delete or alias.
- `spatial_clear_focus()` — unchanged.

All take `FullyQualifiedMoniker` (no separate key argument).

### Step 5 — React adapter — segment-input, context-derived FQM

`kanban-app/ui/src/components/focus-layer.tsx`, `focus-zone.tsx`, `focus-scope.tsx`:

- Consumer prop: `moniker: SegmentMoniker` (typed). Internally compose FQM via `useContext(FullyQualifiedMonikerContext)` + the consumer's segment.
- Provide the composed FQM via `<FullyQualifiedMonikerContext.Provider value={fq}>` for descendants.
- Hook: `useFullyQualifiedMoniker(): FullyQualifiedMoniker` reads from the context. Throws if absent (no primitive ancestor).
- `crypto.randomUUID()` is gone — no more UUID minting on the React side. The FQM is the key.
- `spatial_register_zone({ moniker: fq, ... })` IPC sends the composed FQM directly.

### Step 6 — entity-focus-context

`kanban-app/ui/src/lib/entity-focus-context.tsx`:

- `setFocus(fq: FullyQualifiedMoniker | null)` — strict.
- The store's `focused_moniker` is the FQM.
- The bridge subscribes to `focus-changed` and writes the FQM.
- `useFocusedScope()`, `useFocusedMonikerRef()` — return FQM.
- `useFocusedSegmentMoniker()` — derived (the last segment of the FQM) for legacy display callers.

### Step 7 — Migration sweep

Every `setFocus(...)`, `spatial_focus(...)`, `find_by_moniker(...)`, etc., callsite is updated:

- TS callers must pass `FullyQualifiedMoniker`. The compile-error wave guides the migration. For each callsite:
  - If inside a primitive context: `useFullyQualifiedMoniker()`.
  - If composing a not-yet-mounted descendant's FQM: `composeFq(parent_fq, child_segment)`.
- Tests updated: any test mock that uses a flat moniker for setFocus needs an FQM. Browser tests that wire the simulator: simulator records FQM-keyed entries.

### Step 8 — Drop legacy types

After the sweep:

- Delete `SpatialKey` from the codebase (Rust + TS).
- Delete `Moniker` (the flat string type). Replace every reference with either `SegmentMoniker` or `FullyQualifiedMoniker`.
- The IPC payload shape stabilizes around the new types.

## Acceptance Criteria

- [ ] `SegmentMoniker` and `FullyQualifiedMoniker` are distinct newtypes (Rust + TS). No `String` aliases. `as` casts and `.into()` conversions go through controlled construction (e.g., `FullyQualifiedMoniker::compose`).
- [ ] `SpatialKey` (UUID) is deleted from the codebase. The FQM is the only spatial-primitive identifier.
- [ ] `find_by_fq` is the only lookup-by-identifier API. Takes only `FullyQualifiedMoniker`. Exact match.
- [ ] `setFocus` and `spatial_focus_by_moniker` (or whatever the renamed kernel command is) take only `FullyQualifiedMoniker`. Segment-form callers fail at compile time.
- [ ] React consumers declare only `SegmentMoniker` for their primitives. The FQM is constructed via `FullyQualifiedMonikerContext` nesting and exposed via `useFullyQualifiedMoniker()`.
- [ ] No `duplicate moniker registered against two distinct keys` warning in the running app's log when an inspector opens.
- [ ] ArrowDown / ArrowUp inside an inspector stays inside `/window/inspector/...`. No `card:*` / `column:*` paths in `ui.setFocus` `scope_chain` log lines.
- [ ] ArrowDown at the last field echoes (focus stays put).
- [ ] Escape closes the inspector and restores focus to the originating card via `last_focused`.
- [ ] `cargo test -p swissarmyhammer-focus` passes (incl. new path-moniker tests).
- [ ] `bun run test:browser` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] All existing tests pass (or have been migrated to FQM).

## Workflow

- **Strict TDD with manual log verification.** Layers 1 + 2 first. Layer 3 (manual log inspection in the running Tauri app) is mandatory before done.
- **Newtypes are the safety net.** Use `SegmentMoniker` and `FullyQualifiedMoniker` as distinct types. Do not weaken with `String` aliases or `as` casts. The compile-error wave is the migration guide.
- The user's collected directives (this task's intro) are the spec. No claim-on-mount tricks. No leaf-form fallback. No topmost-layer heuristic. No dual UUID + moniker identifiers. Path is the key.
- Cross-reference: `01KQD0WK54G0FRD7SZVZASA9ST` (kernel-as-source-of-focus refactor — partial precursor; this task supersedes its remaining edge cases). `01KQAW97R9XTCNR1PJAWYSKBC7` (no-silent-dropout contract — preserved).
