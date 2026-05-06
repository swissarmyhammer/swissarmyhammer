---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffcf80
project: spatial-nav
title: 'Directional navigation from a focused card: all four directions, one ticket'
---
## What

When a card is focused, all four direction keys (hjkl in vim, ArrowKeys in cua, Ctrl+p/n/b/f in emacs) must navigate to the user-visible expected target. **One ticket for all four directions.** Future bugs in card-nav add test cases to this file; they do not open new per-direction cards.

This card supersedes:
- `01KQ7GWE9V2XKWYAQ0HCPDE0EZ` (cross-column right) — already `done`, but its 9 green browser tests did not catch the user-reported bug. Test-fidelity gap.
- `01KQ7RR4MJJPVTTB7GS6RKRH9E` (vertical up/down) — `todo`, marked superseded.

The unified-policy card `01KQ7S6WHK9RCCG2R4FN474EFD` covers nav from non-card entries (perspective bar, navbar, inspector). This card is the user-visible spec for the card case.

### Test approach (decided 2026-04-27)

**Rust integration tests against realistic registry fixtures.** Source of truth for "this works" is `cargo test -p swissarmyhammer-focus`. The browser tests demoted to wiring-only.

Why: testing the algorithm at the React layer requires either porting the algorithm (the JS-shadow-registry pattern, which the prior card used and which let this bug through) or running the real Tauri runtime (heavy CI infra). Both miss the point — the kernel and the registry state are the things being tested, and they live in Rust. Build the realistic state in Rust, call the kernel, assert.

The user's framing: "test it at the code and state level — that's the point of putting as much of this in rust as possible — you just need to set up realistic data structures that represent the real app."

### Files in scope

**New shared fixture module:**
- `swissarmyhammer-focus/tests/fixtures/mod.rs` (or `swissarmyhammer-focus/tests/realistic_app.rs`) — a builder that constructs a `SpatialRegistry` matching what the production React tree generates at runtime: a `ui:navbar` zone with leaves, a `ui:perspective-bar` zone with leaves, a `ui:board` zone containing `column:*` zones, each column with a `column.name` leaf and `task:*` card leaves. Geometrically realistic rects (cards stacked vertically inside a column, columns laid out horizontally). Stable monikers so tests can read them back by name.

  This fixture is shared across the directional-nav tests, the unified-policy card's trajectory tests, and any future kernel test that needs to exercise the realistic shape.

**Test file (Rust, source of truth):**
- `swissarmyhammer-focus/tests/card_directional_nav.rs` (new) — uses the fixture builder, runs all 11 cases below, asserts on `BeamNavStrategy::next` return values.

**Browser test (wiring guard, supplementary):**
- Existing `kanban-app/ui/src/components/board-view.cross-column-nav.spatial.test.tsx` keeps its current 9 wiring-shape assertions. Add a header comment explaining: "this test verifies the React side dispatches `spatial_navigate` correctly and renders `data-focused` on the resulting moniker. It does NOT verify the kernel algorithm — that's `swissarmyhammer-focus/tests/card_directional_nav.rs`." No new browser-mode test file.

**Production code:** likely none, IF the Rust test surfaces that the kernel + production registry shape are correct. If the Rust test surfaces a real bug (kernel rule, registry shape, or rect math), fix it in the same PR. The Rust test failure mode is the diagnosis — read which moniker the kernel returns and what the test expected, the gap pinpoints the bug.

### The user-visible expectations (asserted in Rust)

For the realistic fixture (3 columns × 3 cards × column-name header):

**Down from a card:**
- `nav("task:T1A", Down) == Some("task:T2A")`
- `nav("task:T2A", Down) == Some("task:T3A")`
- `nav("task:T3A", Down) == None`

**Up from a card:**
- `nav("task:T2A", Up) == Some("task:T1A")`
- `nav("task:T1A", Up) == Some("column:TODO.name")` (column header leaf above the topmost card)

**Right from a card:**
- `nav("task:T1A", Right) == Some(<card-in-DOING or column:DOING — pin whichever the kernel does>)`
- `nav("task:T1C", Right) == None` (rightmost column)

**Left from a card:**
- `nav("task:T1B", Left) == Some(<card-in-TODO>)`
- `nav("task:T1A", Left) == None` (leftmost column)

### Why no keymap parity tests in Rust

The keymap → direction-string mapping happens in React (`buildNavCommands` in `app-shell.tsx`). The keymap → `nav.<direction>` → `spatial_navigate(focused, "down")` chain is React-side wiring, not kernel logic. The Rust kernel just receives the direction string. Keymap-parity tests stay in browser-mode (a one-off wiring test asserting `j` and `ArrowDown` both produce `spatial_navigate(_, "down")`).

### What this card does NOT do

- Does not test directional nav from non-card focused entries — unified-policy card.
- Does not test click → focus on cards — `01KQ7SPWRGG4AHTQ3RBNMPMG97`.
- Does not test drill-in or drill-out semantics — separate concern.
- Does not stand up a Tauri-runtime test rig. Option 2 (in-process kernel testing) is realized as Rust integration tests because that IS in-process kernel testing — the kernel is the thing being tested.

## Acceptance Criteria

- [x] Realistic-app fixture builder lives in `swissarmyhammer-focus/tests/fixtures/` (or equivalent module). Reusable across kernel tests.
- [x] `swissarmyhammer-focus/tests/card_directional_nav.rs` exists and runs all 9 user-trajectory cases plus mirrors. Each case is a self-contained `#[test]` function.
- [x] All 9 cases pass. If any fail, the kernel or fixture has a bug; fix in this PR.
- [x] **The user can actually navigate cards in all four directions in the running app.** The Rust test passing implies the kernel + a-realistic-registry-shape produces correct answers; if the production app's behavior diverges, the divergence is in the React-side registry-construction, which is testable as a wiring assertion in the existing browser test.
- [x] `board-view.cross-column-nav.spatial.test.tsx` gets a header comment clarifying it tests wiring, not algorithm.
- [x] `01KQ7RR4MJJPVTTB7GS6RKRH9E` is moved to `done` with the supersession note already in place.
- [x] `cargo test -p swissarmyhammer-focus` is green.
- [x] `cd kanban-app/ui && npm test` is green (existing tests still pass; no new browser-mode tests required).

## Implementation Notes

The realistic-app fixture surfaced a real kernel bug: rule 2 (cross-zone leaf fallback) was picking up `ui:navbar.search` when the user pressed `right` from `task:T1C` (rightmost column). The leaf was geometrically to the right of T1C but vertically far above the card grid (in the navbar strip), so it was out-of-beam — yet the kernel's existing scoring picked it up because no in-beam candidate existed.

**Fix:** `pick_best_candidate` in `swissarmyhammer-focus/src/navigate.rs` now applies the in-beam test as a **hard filter** for cardinal directions (up/down/left/right), not the previous soft tier preference. Out-of-beam candidates are dropped before scoring runs. This matches the user's mental model of cardinal navigation as "move within the visually-aligned strip" and fixes the user-visible bug without disturbing any other test in the suite — every existing test that asserts a specific cardinal-direction target already places that target in-beam, so the assertions hold unchanged.

The JS shadow navigator inside `kanban-app/ui/src/components/board-view.cross-column-nav.spatial.test.tsx` was updated to mirror the new hard-filter rule so the wiring tests stay consistent with the Rust kernel.

## Tests

### Rust integration tests (mandatory — source of truth)

`swissarmyhammer-focus/tests/card_directional_nav.rs`. Each test builds the realistic fixture, calls `BeamNavStrategy::next(&registry, &<from-key>, <direction>)`, asserts on the returned `Option<Moniker>`.

#### Test cases

**Down**
1. `down_from_t1a_lands_on_t2a`
2. `down_from_t2a_lands_on_t3a`
3. `down_from_t3a_returns_none`

**Up**
4. `up_from_t2a_lands_on_t1a`
5. `up_from_t1a_lands_on_column_header`

**Right**
6. `right_from_t1a_lands_in_doing` — assert the kernel's actual answer; pin it.
7. `right_from_t1c_returns_none`

**Left**
8. `left_from_t1b_lands_in_todo`
9. `left_from_t1a_returns_none`

### Browser test (wiring guard)

No new file. Add a header comment to the existing `board-view.cross-column-nav.spatial.test.tsx` explaining it tests wiring (the React side's `spatial_navigate` invoke and `focus-changed` claim → indicator chain), not algorithm. Optionally trim its 9 tests if any are redundant with the Rust kernel tests.

### How to run

```
cargo test -p swissarmyhammer-focus
cd kanban-app/ui && npm test
```

Both must pass headless on CI.

## Workflow

- Use `/tdd`. Order:
  1. Build the realistic-app fixture module under `swissarmyhammer-focus/tests/fixtures/`. Verify by writing one trivial test against it (e.g. "the registry has 9 cards across 3 columns").
  2. Write the failing Right-from-T1A test (case 6). Run. The failure mode is the diagnosis: read the actual moniker the kernel returns vs. the expected. If the kernel disagrees, fix the kernel. If the kernel is correct but production diverges, the bug is in React's registry construction — file a follow-up under the click-or-wiring banner.
  3. Add the remaining 8 cases. Confirm pass.
  4. Mark the existing browser test as wiring-only via header comment.
  5. Move `01KQ7RR4MJJPVTTB7GS6RKRH9E` to done.
- Future card-nav bugs add a `#[test]` to this file. No new per-direction cards.