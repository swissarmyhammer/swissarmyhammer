---
assignees:
- claude-code
position_column: todo
position_ordinal: '7e80'
project: spatial-nav
title: 'Redesign FocusIndicator: dotted border inset (no overflow / clipping issues)'
---
## What

Replace the current `<FocusIndicator>` cursor-bar (an absolutely-positioned vertical stripe at `-left-2 w-1` *outside* the host's box) with a fine dotted line painted *inside* the host's box using the primary color.

This eliminates the entire class of "focus indicator clipped by `overflow: hidden`" bugs (e.g. tasks `01KQSEYBXN834FYK12J55MJ6SP` for board.name and percent_complete) AND removes the "host needs left padding to make room for the indicator" coupling that today forces `pl-2` / `gap-2` layout invariants on every column-strip consumer (see `nav-bar.tsx:36-51` for the prose contract that becomes obsolete).

### Visual reference

The existing `<FocusDebugOverlay>` (kanban-app/ui/src/components/focus-debug-overlay.tsx) paints a dashed border at `absolute inset-0 border border-dashed` inside the host — three concentric boxes for layer/zone/scope debug. The new production indicator follows the SAME positional pattern (absolute inset-0 inside the host) but is:

- **Dotted, not dashed** (visually quieter — debug uses dashed; production uses dotted)
- **One color** (`--primary` from the design system, the same color used as the main button background — the prominent accent color visible against the page bg). Tailwind: `border-primary`.
- **No label, no handle** (production indicator is not a debug aid)

### Implementation

`kanban-app/ui/src/components/focus-indicator.tsx` — rewrite the rendered span:

```tsx
return (
  <span
    data-testid="focus-indicator"
    aria-hidden="true"
    className="pointer-events-none absolute inset-0 border border-dotted border-primary rounded-[inherit]"
  />
);
```

Key properties:
- `absolute inset-0` — fills the host's box exactly. Host primitives already declare `position: relative` so this works without any consumer change.
- `border border-dotted border-primary` — 1px dotted border in the primary color.
- `rounded-[inherit]` — when the host has rounded corners (e.g. `rounded-md` on cards), the indicator follows them.
- `pointer-events-none` — never intercepts clicks.
- Keep `data-testid="focus-indicator"` and `aria-hidden="true"` so existing test selectors and a11y semantics stay intact.

### Cleanup that follows from this change

- `kanban-app/ui/src/components/nav-bar.tsx` — the long doc comment at lines 33–61 about the `gap-2` / `-left-2` / `px-4` layout contract is now obsolete. Replace with a one-line note: "FocusIndicator paints inside the host's box; no special gap/padding required."
- `kanban-app/ui/src/components/perspective-tab-bar.tsx:238-246` — the same `pl-2 gap-2` justification comment becomes obsolete.
- Any other component-level prose justifying layout to make room for the bar — sweep and shorten.

The `pl-2 gap-2` classes themselves can stay (they were also general spacing) — but they're no longer load-bearing for the indicator.

### Tests to update

Many tests assert on the indicator's structure or position. Sweep:

- `kanban-app/ui/src/components/focus-indicator.test.tsx` — direct unit tests on the component. Update style/className assertions.
- `kanban-app/ui/src/components/focus-indicator.single-variant.spatial.test.tsx` — pins the "single variant" rule. Confirm the new indicator still satisfies it (one component, no parallel CSS bar).
- `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts` — source-level guards on indicator structure. Update if it asserts on the `-left-2 w-1` className tokens specifically.
- `kanban-app/ui/src/components/nav-bar.focus-indicator.browser.test.tsx`
- `kanban-app/ui/src/components/perspective-tab-bar.focus-indicator.browser.test.tsx`
- Any other test that asserts on the indicator's left position or width — these become "indicator is inside the host's bounding box."

### Files to modify

- `kanban-app/ui/src/components/focus-indicator.tsx` — the actual component (~10 lines of JSX changed plus updated docstring).
- `kanban-app/ui/src/components/focus-indicator.test.tsx` — unit tests.
- `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts` — source guards (small change).
- `kanban-app/ui/src/components/nav-bar.tsx` and `perspective-tab-bar.tsx` — shorten obsolete doc comments.
- Browser-mode regression tests (`*focus-indicator.browser.test.tsx`) — update positional assertions.

## Acceptance Criteria

- [ ] `<FocusIndicator focused={true}>` renders a `<span>` with `position: absolute`, `inset: 0` (or equivalent), and a 1px dotted border in the `--primary` color. The span sits INSIDE the host's box, not outside.
- [ ] No `-left-2`, no `w-1`, no `bg-primary` on the new indicator (the bar variant is gone).
- [ ] When the host has rounded corners (`rounded-md` etc.), the indicator follows them via `rounded-[inherit]`.
- [ ] Focusing a `<Field>` whose ancestor has `overflow: hidden` (e.g. the toolbar `truncate` wrapper at `board-selector.tsx:92`) shows a visible indicator — test asserts the indicator's `getBoundingClientRect()` has non-zero width and height AND lies entirely within its nearest `overflow: hidden` ancestor (no clipping).
- [ ] Indicator paints in dark mode AND light mode (the `--primary` token swaps between modes via the existing `index.css` token definitions).
- [ ] No regression in single-variant rule: still one `<FocusIndicator>` component, no parallel CSS bar driven by `[data-focused]`.
- [ ] `pnpm -C kanban-app/ui test` passes (with updated assertions).
- [ ] `pnpm -C kanban-app/ui typecheck` passes.

## Tests

- [ ] Update `kanban-app/ui/src/components/focus-indicator.test.tsx` to assert the new structure: rendered span has `data-testid="focus-indicator"`, has `border` / `border-dotted` / `border-primary` / `rounded-[inherit]` classNames, and computed style shows `position: absolute` with all-zero inset.
- [ ] Add a regression test in `focus-indicator.test.tsx` (or a new `focus-indicator.no-clip.browser.test.tsx`): mount a focused `<FocusScope>` whose parent `<div>` has `overflow: hidden`. Assert `getBoundingClientRect()` of the indicator is contained within the parent's bounding rect (no clip).
- [ ] Add a "follows rounded corners" assertion: mount a focused scope whose host has `rounded-lg`, and confirm the indicator's computed `border-radius` is non-zero (inherits from parent).
- [ ] Run `pnpm -C kanban-app/ui test focus-indicator focus-architecture.guards nav-bar.focus-indicator perspective-tab-bar.focus-indicator` and confirm all green.
- [ ] Run the full `pnpm -C kanban-app/ui test` to catch any test elsewhere that asserted on the old `-left-2 w-1` shape.

## Workflow

- Use `/tdd`. Write the no-clip and rounded-corners regressions first against the current code (RED). Rewrite the component. Sweep dependent tests until everything is GREEN. Shorten obsolete doc comments in `nav-bar.tsx` and `perspective-tab-bar.tsx` last.
- After this lands, the task `01KQSEYBXN834FYK12J55MJ6SP` (toolbar field indicators invisible) is partially solved by this change for the `overflow: hidden` clipping case — the `showFocusBar={false}` default question on the navbar percent field still needs handling there. Update or close that task as appropriate after this PR.
