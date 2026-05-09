---
assignees:
- assistant
position_column: review
position_ordinal: '8280'
title: 'Fix nav-bar focus: remove viewport-spanning ui:navbar FocusScope wrapper'
---
## What

The outer `<FocusScope moniker="ui:navbar">` in `kanban-app/ui/src/components/nav-bar.tsx` is a viewport-spanning rectangle that swallows clicks and beam-search nav targeting any of its inner leaves. The kernel resolves clicks on bar whitespace and from-below beam searches to `ui:navbar` (the parent), so inner leaves never get focus.

This is the same class of bug that was fixed for `ui:board` in commit `8232b25cc`.

## Fix

- Remove the outer `<FocusScope moniker="ui:navbar">` wrapper from `nav-bar.tsx`.
- Replace with a plain `<div role="banner" className="...">`.
- Inner scopes (`ui:navbar.board-selector`, `ui:navbar.inspect`, `ui:navbar.search`, board-name `<Field>`) are already mounted under `<FocusLayer name="window">` via the FQM composition context — they will register as peer leaves of left-nav and perspective-bar without the swallowing parent.

## Acceptance Criteria

- Clicking on the board-name field moves focus there visibly (data-focused flips on).
- Arrow-nav from left-nav rightward reaches nav-bar leaves.
- Arrow-nav from perspective-bar upward reaches nav-bar leaves.
- `pnpm -C kanban-app/ui exec tsc --noEmit` passes clean.
- Targeted nav-bar tests stay green; if they fail because the tree shape changed, update the assertions to match the new tree, do not revert the production fix.

## Tests

- Run `pnpm -C kanban-app/ui exec tsc --noEmit` (must be clean).
- Run any nav-bar focused tests; update assertions if they are coupled to the old wrapper shape.