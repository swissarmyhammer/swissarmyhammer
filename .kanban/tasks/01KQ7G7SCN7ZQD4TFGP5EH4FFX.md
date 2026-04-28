---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffc680
project: spatial-nav
title: Remove unauthorized FocusIndicator "ring" variant — one indicator only
---
## What

The implementer added a second visual variant of `<FocusIndicator>` (`variant="ring"`) without approval and applied it to the nav bar. The architectural contract was: **one** focus indicator visual, the cursor-bar, rendered in **one** component. The `ring` variant breaks that contract — it adds a parallel visual decoration, threads a `focusIndicatorVariant` prop through `<FocusScope>`, `<FocusZone>`, and `<Focusable>`, and forces every nav button to opt into a non-default visual. Delete it.

### Scope of removal

**Production code:**

- `kanban-app/ui/src/components/focus-indicator.tsx`
  - Delete the `FocusIndicatorVariant` type export.
  - Delete the `variant` prop from `FocusIndicatorProps`.
  - Delete the `if (variant === "ring") { … }` branch.
  - The component returns null when `!focused`, otherwise the bar `<span>`. Update the file-level docstring to drop the "Variants" section.

- `kanban-app/ui/src/components/focus-scope.tsx`
  - Remove `focusIndicatorVariant?` from `FocusScopeOwnProps`.
  - Remove the destructured default `focusIndicatorVariant = "bar"` from the function signature.
  - Remove `focusIndicatorVariant` from the props passed to the inner indicator-rendering helper, and from any internal type that mirrors it.
  - Drop the corresponding docstring blob.

- `kanban-app/ui/src/components/focus-zone.tsx`
  - Same as `focus-scope.tsx` — symmetric prop, symmetric removal.

- `kanban-app/ui/src/components/nav-bar.tsx`
  - Remove all three `focusIndicatorVariant="ring"` attributes (board-selector, inspect, search).
  - Drop the "Focus indicator variant" docstring section that justifies the ring.
  - The cursor-bar is the only indicator. If the bar visually fails for tiny icon buttons inside a `gap-3` row, **fix the layout** (e.g. wrap each button in a `Focusable` whose box reserves left padding ≥ 8px so the bar has somewhere to live, or change the navbar gap, or change the bar's positioning **inside** `<FocusIndicator>` for all consumers). Do NOT add a second variant. The bar must remain visible on a focused nav button.

**Test code (delete cases that exercise the variant; do not just disable):**

- `kanban-app/ui/src/components/focus-indicator.test.tsx` — delete the three `variant="ring"` cases.
- `kanban-app/ui/src/components/focus-zone.test.tsx` — delete the three tests under the `// focusIndicatorVariant — symmetric with <FocusScope>` block (defaults-to-bar, forwards-ring, no-effect-when-showFocusBar-false).
- `kanban-app/ui/src/components/focus-scope.test.tsx` (if it has a parallel block) — same.
- `kanban-app/ui/src/components/nav-bar.spatial-nav.test.tsx` — drop or rewrite the test asserting the ring variant is forwarded; replace with a positive test that the cursor-bar `[data-testid="focus-indicator"]` mounts and is visible (non-zero bounding rect, opacity > 0) when a nav button is focused.

**Architecture guard tightening:**

- `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts`
  - Add a guard that greps `kanban-app/ui/src/` for the literal `focusIndicatorVariant` and the literal `FocusIndicatorVariant` and asserts both are absent (allowing the guard test file itself).
  - Add a guard that greps `focus-indicator.tsx` for `"ring"` (case-sensitive) and asserts it is absent — the only allowed variant string in the file should be the bar's class names, none of which contain the word `ring`.

### Why this matters

Every variant prop is a chance for two consumers to pick differently and produce inconsistent UX. The user explicitly asked for a single indicator and rejected the ring variant. Removing it eliminates: an unused type export, a prop on three primitives, a docstring rationale that doesn't apply anywhere else, and a test surface that exists only to validate code we are about to delete. After this task, "where does the focus visual come from?" has exactly one answer.

## Acceptance Criteria

- [ ] `grep -r "FocusIndicatorVariant" kanban-app/ui/src/` returns zero matches.
- [ ] `grep -r "focusIndicatorVariant" kanban-app/ui/src/` returns zero matches.
- [ ] `grep -nE 'variant\s*=\s*"ring"' kanban-app/ui/src/` returns zero matches.
- [ ] `kanban-app/ui/src/components/focus-indicator.tsx` exports a single component whose props interface is `{ focused: boolean }` — no `variant` field.
- [ ] `<FocusScope>`, `<FocusZone>`, and `<Focusable>` (if it still exists as a re-export shim) accept no `focusIndicatorVariant` prop. TypeScript rejects the prop at compile time.
- [ ] `nav-bar.tsx` shows the cursor-bar on a focused nav button — verified by a browser test (see Browser Tests below). If layout adjustment was needed to make the bar visible, the change is contained to `nav-bar.tsx` (no new variants, no new props).
- [ ] All four guard tests in `focus-architecture.guards.node.test.ts` pass: (1) no CSS reads `[data-focused]`, (2) only `<FocusIndicator>` paints the visual, (3) `FocusIndicatorVariant` literal absent, (4) `focusIndicatorVariant` literal absent.
- [ ] `cd kanban-app/ui && npm test` is green (`tsc --noEmit && vitest run`).

## Tests

### Browser Tests (mandatory)

Run under Vitest browser mode (`vitest-browser-react` + Playwright Chromium). They prove the cursor-bar is the **only** focus visual and that nav-bar buttons still show focus after the variant is gone.

#### Test file
`kanban-app/ui/src/components/focus-indicator.single-variant.spatial.test.tsx` (new file)

#### Setup
- Mock `@tauri-apps/api/core` and `@tauri-apps/api/event` per the canonical pattern in `grid-view.nav-is-eventdriven.test.tsx` (`vi.hoisted` + `mockInvoke` + `mockListen` + `fireFocusChanged` helper).
- Render the component under test inside `<SpatialFocusProvider><FocusLayer name="test">…</FocusLayer></SpatialFocusProvider>`.

#### Required test cases

1. **Type-level: variant prop removed** — a `// @ts-expect-error` test that proves `<FocusIndicator focused variant="ring" />` no longer compiles. Same for `<FocusScope … focusIndicatorVariant="ring">` and `<FocusZone … focusIndicatorVariant="ring">`.

2. **Runtime: focused indicator is the bar everywhere** — render a `<NavBar>` (the highest-stakes consumer), drive focus to each nav-bar leaf via `fireFocusChanged(<key>)` for each registered moniker (`ui:navbar.board-selector`, `ui:navbar.inspect`, `ui:navbar.search`), and assert that the mounted `[data-testid="focus-indicator"]` element has the bar class signature: `pointer-events-none absolute -left-2 top-0.5 bottom-0.5 w-1 rounded-full bg-primary shadow-sm`. None of them carry `inset-0` or `ring-2 ring-ring`.

3. **Runtime: bar is visible on a nav button** — assert the focused nav button's indicator has a non-zero bounding rect (`getBoundingClientRect().width > 0` and `.height > 0`) and is not visually clipped (its left edge is within the viewport, i.e. `rect.left >= 0`). This catches the historic "the bar lives in `gap` dead space and is invisible" failure mode that motivated the ring variant in the first place — the layout fix this task ships must make the bar genuinely visible without a variant.

4. **Architecture: only one indicator per focused entity** — render the full nav bar with one entity focused; the document contains exactly one `[data-testid="focus-indicator"]` element. (Already covered by the `focus-architecture.guards` node test for source-level uniqueness; this is the runtime symmetric.)

### Source-level guards

- [ ] Update `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts` to assert `FocusIndicatorVariant` and `focusIndicatorVariant` literals are absent from `kanban-app/ui/src/` (excluding the guard file itself).

### How to run

```
cd kanban-app/ui && npm test
```

The test must pass headless. The CI workflow `.github/workflows/*.yml` already runs this command.

## Workflow

- Use `/tdd` — write the failing browser test plus the failing guard tests first, then delete the variant code path until they go green.
