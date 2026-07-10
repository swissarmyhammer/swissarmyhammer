---
assignees:
- claude-code
position_column: todo
position_ordinal: c780
title: 'Bug: Cannot jump (nav.jump) to the (i) inspect button in the nav bar'
---
## What
Reported by user: the jump overlay (`nav.jump`) cannot target the board **(i) Inspect** button in the top nav bar.

That button registers as a `<Pressable moniker="ui:navbar.inspect">` leaf in `apps/kanban-app/ui/src/components/nav-bar.tsx:107` (the `<Info>` icon button, only rendered when `board` is truthy). Per the NavBar doc comment, each actionable child registers as its own peer top-level `<FocusScope>` leaf under `<FocusLayer name="window">`, sibling to `ui:left-nav` and `ui:perspective-bar`.

The jump overlay is `apps/kanban-app/ui/src/components/jump-to-overlay.tsx`; jump assigns sneak codes (`apps/kanban-app/ui/src/lib/sneak-codes.ts`) to currently-registered focus leaves. If `ui:navbar.inspect` is absent from the jump target set, it either (a) never publishes a non-zero rect to the focus kernel, (b) is filtered out of the jump candidate enumeration, or (c) isn't enumerated because the navbar leaves aren't included in the layer the jump overlay reads. Determine which.

Reproduce: open a board, trigger jump (`nav.jump` / `Mod+G`), observe whether the (i) inspect button receives a sneak code/label. Compare with the search button (`ui:navbar.search`) — does it get a code?

NOTE: likely shares a root cause with the blank-Navigation-menu and command-palette-launch bugs (focus/nav command + leaf surfacing at runtime). Cross-check before fixing in isolation.

## Acceptance Criteria
- [ ] Triggering jump assigns a sneak code to the `ui:navbar.inspect` leaf, and selecting it focuses/activates the inspect button.
- [ ] Root cause identified (missing rect publish vs. jump-candidate filtering vs. layer enumeration).

## Tests
- [ ] Extend the spatial/jump test suite (`apps/kanban-app/ui/src/spatial-nav-jump-to.spatial.test.tsx` and/or `apps/kanban-app/ui/src/components/jump-to-overlay.*.test.tsx`) to assert `ui:navbar.inspect` is among the enumerated jump targets when a board is open.
- [ ] Regression test failing before the fix, passing after.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug