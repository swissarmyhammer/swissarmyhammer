---
assignees:
- wballard
depends_on:
- 01KQW6JF6P7QHXFARAR5RTZVX4
position_column: todo
position_ordinal: dc80
project: spatial-nav
title: 'spatial-nav redesign step 13: cutover (4/4) — move overlap-warning to JS dev-mode against LayerScopeRegistry'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**. Final cutover step.

## Goal

Re-implement the "needless-nesting / overlap" detection that motivated this whole redesign — but in JS, against the React-side `LayerScopeRegistry`, in dev mode only. The Rust `check_overlap_warning` was already deleted in step 12; this step replaces its behavioral value at the new layer.

## What to build

### Dev-mode hook on registry insertion

In `LayerScopeRegistry.add(fq, entry)`:

```ts
function add(fq, entry) {
  entries.set(fq, entry);
  if (import.meta.env.DEV) {
    queueMicrotask(() => detectNeedlessNesting(fq, entry));
  }
}
```

`queueMicrotask` ensures detection runs after layout settles — `getBoundingClientRect()` on a freshly mounted element returns its painted rect, not its pre-layout zero rect. (Or use `requestAnimationFrame` if microtask is too eager.)

### `detectNeedlessNesting`

```ts
function detectNeedlessNesting(newFq, newEntry) {
  const newRect = newEntry.ref.current?.getBoundingClientRect();
  if (!newRect) return;

  for (const [otherFq, otherEntry] of entries) {
    if (otherFq === newFq) continue;
    const otherRect = otherEntry.ref.current?.getBoundingClientRect();
    if (!otherRect) continue;

    if (rectsOverlapTightly(newRect, otherRect)) {
      console.warn(
        "[spatial-nav] needless-nesting: two scopes share rect",
        { newFq, otherFq, rect: newRect, newEntry, otherEntry },
      );
      // include the React component-stack via captured stack trace
    }
  }
}
```

`rectsOverlapTightly` returns true when both rects are within a few pixels of each other on all four sides — the same threshold the Rust check used.

### Why this works without false positives during drag

The original Rust `check_overlap_warning` fired on every `update_rect`, so drag animations triggered it constantly. This JS version runs only on `LayerScopeRegistry.add` — i.e., when a NEW scope mounts. Drag-drop doesn't mount new scopes, so it doesn't trigger detection. Animation that just moves rects doesn't trigger detection. The original symptom from the user's `kanban-app[49073]` log goes away.

The check still catches the structural bug it was meant to catch: a `<FocusScope>` whose only child is another `<FocusScope>` at the same rect — that produces two `add` calls back-to-back at the same rect, and the second one fires the warning.

### Production builds

`if (import.meta.env.DEV)` strips the entire detection path from production bundles. Zero overhead at runtime.

### Component-stack in the warning

React DevTools / `console.warn` natively show the component stack. Make sure the warning is structured so the dev sees both the new scope's monikers AND the React tree path. Helpful to include in the log: `entry.segment` for both partners.

## Tests

- Mount two `<FocusScope>` elements with the same rect (synthetic overlap) → warning fires once with both FQs.
- Mount a `<FocusScope>` whose only child is another `<FocusScope>` (the literal "needless-nesting" pattern) → warning fires for the inner one against the outer.
- Drag a card across the column (rect changes, no new mounts) → no warning.
- Filter changes that unmount and remount cards at different positions → no warning unless two end up at the same position.
- Production-build smoke: warning is a no-op in production (the `import.meta.env.DEV` branch is dead code).

## Acceptance criteria

- Dev-mode JS overlap detection in place, runs against `LayerScopeRegistry`
- Production builds carry no detection cost
- Drag-drop scenario does NOT fire the warning (proves the original bug class is closed)
- Synthetic needless-nesting test still fires (proves the warning's purpose is preserved)
- All tests green

## Files

- `kanban-app/ui/src/lib/spatial-focus-context.tsx` (or `layer-scope-registry-context.tsx`) — add detection hook
- New test file: `kanban-app/ui/src/lib/needless-nesting-detection.test.tsx`

## After this step

The redesign is complete. Recommended follow-up:

- Re-evaluate `01KQSF0VCEWW523VXCBTYX4W0B` (nav.left collapse to engine root) — likely fixed or now diagnosable cleanly against the simpler architecture
- Update `MEMORY.md` reference about spatial-nav architecture if any entries point to the old replicated kernel #stateless-nav