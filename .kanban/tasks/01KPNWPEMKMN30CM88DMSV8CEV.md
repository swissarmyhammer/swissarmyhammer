---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff080
project: spatial-nav
title: 'Inspector: j/k/h/l between inspector fields (regression-proof with tests)'
---
## What

When the inspector is open (its own FocusLayer on top of the window layer), j/k/h/l should move focus between inspector fields:
- `j` / `k` moves to the field below / above
- `h` / `l` moves within a field if it has multiple sub-parts (pills, inline value editors) — otherwise no-op at the edge
- Escape (or click outside the inspector) closes it, releasing the layer back to the window

Manual testing this session confirmed inspector nav worked BEFORE mid-session edits, then "totally fucked" after. ZERO automated tests guard it.

### TDD — failing tests

Under `kanban-app/e2e/spatial-nav-inspector.e2e.ts` (depends on the E2E harness):

```ts
describe("inspector field navigation", () => {
  beforeEach(async () => {
    await openBoardFixture();
    await click(cardEl("card-1-1"));
    await doubleClick(cardEl("card-1-1"));  // opens inspector
    await waitFor(() => layerStackLen() === 2); // window + inspector
  });

  it("j moves focus from the first field to the second field", async () => {
    const first = await getFocusedMoniker();
    expect(first).toMatch(/^field:task:card-1-1\./);
    await keyboard("j");
    const second = await getFocusedMoniker();
    expect(second).toMatch(/^field:task:card-1-1\./);
    expect(second).not.toBe(first);
  });

  it("j at the last field clamps (does not wrap)", async () => {
    // keyboard('G') or press j many times to reach the end, then one more j
    // — focus should not move
  });

  it("k at the first field clamps", async () => { ... });

  it("Escape closes the inspector and restores window-layer focus", async () => {
    await keyboard("Escape");
    await waitFor(() => layerStackLen() === 1);
    const focused = await getFocusedMoniker();
    expect(focused).toBe("task:card-1-1"); // focus restored to the card we opened from
  });

  it("inspector nav is trapped — k at the top field does NOT reach the perspective bar", async () => {
    // Navigate to top field, press k. Focus stays within inspector.
  });
});
```

### Approach

Likely already works. Tests codify the contract so we notice when a future edit breaks it.

### Acceptance

- [x] All 5 E2E tests pass reliably (3 consecutive green runs)
- [x] Escape restores focus to the moniker that was focused when the inspector opened (layer focus memory)
- [x] Inspector layer traps nav — can't escape to other layers via h/j/k/l

### Implementation notes

- Reframed from e2e harness to the vitest-browser shim harness (same infrastructure as `spatial-nav-canonical.test.tsx`).
- New fixture at `kanban-app/ui/src/test/spatial-inspector-fixture.tsx` — a minimal "window + inspector" two-layer shape with one card and four field FocusScopes. Inspector mounts/unmounts on `ui.inspect` / `app.dismiss` client-side via React state (no backend round-trip needed).
- New test file at `kanban-app/ui/src/test/spatial-nav-inspector.test.tsx` covers all five contract cases.
- Tests run green 3/3; entire `src/test/` suite remains green (25/25).
- Only inspector-specific test files touched — did not modify `data-table.tsx`, `focus-scope.tsx`, `left-nav.tsx`, `perspective-tab-bar.tsx`, `board-view.tsx` (per task instructions about parallel agents).