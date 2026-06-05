---
assignees:
- claude-code
depends_on:
- 01KT6R6HR3KJT6JVNDRAJV8V4T
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe980
project: short-ids
title: 'Short IDs: enforce uniqueness at create + doctor check'
---
Make the 7-char short id board-unique so resolution never needs a fallback.

## Scope
- In the kanban task-creation path (`add task`), after minting the ULID, compute its short id and check it against all existing tasks' short ids. On collision, regenerate the ULID and retry until unique.
  - The retry loop lives at the kanban creation layer (has board context) — NOT the board-unaware `Ulid::new()` mint in swissarmyhammer-entity/store (`crates/swissarmyhammer-entity/src/io.rs`, `crates/swissarmyhammer-store/src/id.rs`).
- Add a lightweight `doctor`/validation assertion that no two existing tasks share a short id — safety net for the ~2524 tasks minted before this invariant (collision among them is ~0.009%, unprovable without a check).

## Acceptance
- Forced-collision test: inject/seed a ULID whose last-7 already exists on the board, call create, assert a different ULID is minted whose short id is unique (not a unit test of the slice).
- Doctor check passes on a clean board and fails when two tasks are made to collide.

Depends on core derivation/resolver.