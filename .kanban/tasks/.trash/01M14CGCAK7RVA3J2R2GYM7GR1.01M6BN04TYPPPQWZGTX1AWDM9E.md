---
assignees:
- claude-code
position_column: todo
position_ordinal: fff980
title: Make the TypeScript actor-colour hash fold UTF-8 bytes, as Rust does
---
## Problem

`apps/kanban-app/ui/src/lib/actor-colors.ts::deriveActorColor` folds
`id.charCodeAt(index)` — UTF-16 code units. The Rust hash it mirrors,
`swissarmyhammer-kanban/src/auto_color.rs::palette_color`, folds `s.bytes()` —
UTF-8 bytes. For an id outside ASCII the two answer different colours, so the
avatar the UI draws disagrees with the colour Rust stored on the actor.

No colour is wrong today: agent ids are slugified to ASCII, and human ids are
OS usernames, which have been ASCII so far.

## Task

Fold the UTF-8 bytes in TypeScript (`new TextEncoder().encode(id)`) so the two
implementations agree for every id. Extend the table in
`apps/kanban-app/ui/src/lib/actor-colors.test.ts` with at least one non-ASCII
id, and add the same id to `PINNED_ACTOR_COLORS` in
`apps/kanban-app/src/state.rs`, so the two tables keep proving the agreement.

This changes the colour a non-ASCII actor is drawn with. That is the point of
the fix, and no such actor is known to exist.

## Found by

Card ^mv8tvs0, while sharing the one djb2 helper. Out of that card's scope
because that card is a behaviour-preserving refactor. #kanban