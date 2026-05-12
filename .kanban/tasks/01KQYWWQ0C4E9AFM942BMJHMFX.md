---
assignees:
- claude-code
depends_on:
- 01KQYWV9DC866DGRPBRFR17ZEY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffbb80
project: spatial-nav
title: End-to-end spatial test for Jump-To overlay
---
## What

Full end-to-end vitest browser-mode test that exercises the Jump-To feature against the real `App.tsx` provider tree (not stubbed dependencies). Validates the integration of YAML registry → keybinding → app-shell state → overlay → spatial focus dispatch.

New file: `kanban-app/ui/src/spatial-nav-jump-to.spatial.test.tsx`. Pattern this after the existing `spatial-nav-end-to-end.spatial.test.tsx`:

```ts
import { test, expect } from "vitest";
import { render, fireEvent } from "@testing-library/react";
// ... seed a board with multiple columns + cards via the existing test fixtures
```

Required cases:

1. **vim `s` opens the overlay**
   - Set keymap_mode to `vim`.
   - Render `<App />` with a seeded board (e.g., 3 columns × 4 cards).
   - Fire `keydown` for `s` on document.body.
   - Assert the overlay element is in the DOM (e.g., a label pill with `data-jump-code` attr).
   - Assert the count of code pills equals the number of currently-visible focusable scopes (use `enumerateVisibleScopes()` directly to compute the expected count).

2. **cua `Mod+G` opens the overlay**
   - Same as above with keymap_mode `cua` and `keydown` for `g` with `metaKey: true`.

3. **Typing a code moves focus to that scope**
   - Open the overlay (vim `s`).
   - Read the `data-jump-code` from a chosen pill and the matching FQM (or read it from a debug DOM attribute).
   - Type each letter of the code as separate `keydown` events.
   - Assert the spatial focus event was emitted for that FQM (listen to the same `focus-changed` channel the rest of the spatial-nav tests use).
   - Assert the overlay is no longer in the DOM.

4. **Esc dismisses without focus change**
   - Open the overlay.
   - Capture the currently-focused FQM.
   - Fire `keydown` for `Escape`.
   - Assert overlay closed and focused FQM unchanged.

5. **Non-matching letter dismisses without focus change**
   - Open the overlay.
   - Fire `keydown` for a letter known not to be a code prefix (e.g., generate codes, find a letter not in any code).
   - Assert overlay closed, focus unchanged.

6. **Multi-letter code requires both keystrokes**
   - Seed enough scopes that codes spill into 2-letter range (>26 scopes — may need to render a grid view with many cards, or stub `enumerateVisibleScopes` for this case).
   - Open overlay; type the first letter of a 2-letter code; assert overlay still open, no focus dispatch.
   - Type the second letter; assert focus dispatched.

7. **Global keybindings suppressed while overlay is open**
   - Open overlay.
   - Fire `keydown` for `j` (vim `nav.down`) — but `j` happens to be a code letter, so use a key that is definitely not a code AND is bound globally (e.g., `Tab` for `nav.right` in cua mode if testing in cua).
   - Assert no focus change happened (the global handler did not fire).

## Acceptance Criteria

- [x] All 7 test cases pass.
- [x] Test does not stub `JumpToOverlay`, `enumerateVisibleScopes`, or `generateSneakCodes` — they're exercised end-to-end.
- [x] No flakiness — no timing-dependent assertions (no `setTimeout` waiting for the overlay; the overlay is synchronous on key press).

## Tests

- [x] `kanban-app/ui/src/spatial-nav-jump-to.spatial.test.tsx` exists with the 7 cases above.
- [x] Test command: `cd kanban-app/ui && pnpm test spatial-nav-jump-to` — passes.
- [x] Run all spatial tests after this lands: `cd kanban-app/ui && pnpm test spatial` — every pre-existing spatial test still passes.

## Workflow

- Use `/tdd` only on incremental cases — write case 1, watch it fail, make it pass, then case 2, etc. Going one case at a time avoids fix-everything-at-once and keeps the integration debugged piece-by-piece. #nav-jump

## Implementation notes

- `generate_jump_codes` is faithfully ported from `swissarmyhammer-focus/src/sneak.rs` into the test bootstrap-invoke handler. The Rust kernel is not running in browser mode, so the IPC boundary has to answer the same way the kernel would. This is the same pattern the existing spatial-nav suite uses for `spatial_navigate` etc. — a bridge for the Tauri boundary, not a stub of the React-side implementation under test.
- For case 5 the non-matching letter is `i`, `l`, or `o` — those three are deliberately omitted from the sneak alphabet (high-confusion letters) so they are guaranteed to never appear in any code, regardless of how many scopes the fixture mounts.
- For case 6 the 3×3 fixture board produces well over 23 enumerable scopes (cards, columns, perspective tabs, nav-bar leaves, the board entity zone, the perspective bar, etc.), which forces the sneak generator to spill into 2-letter codes naturally — no extra harness needed.
- For case 7 the assertion is "no spatial_focus IPC lands on a non-sentinel target". The overlay's claim has placed focus on the sentinel inside its single-scope `<FocusLayer name="jump-to">`, so the kernel's cascade has nowhere to navigate from there. The user-observable invariant — global nav cannot escape the overlay — holds without the overlay needing to call `stopPropagation` on every non-letter keydown.

## Review Findings (2026-05-08 19:05)

### Nits

- [x] `kanban-app/ui/src/spatial-nav-jump-to.spatial.test.tsx:518-522` — Stale comment block directly contradicts the code below it. The comment says "we set both `metaKey` and `ctrlKey` so the binding fires regardless of which platform branch `normalizeKeyEvent` takes", but line 526 sets only `metaKey: true`. The replacement comment on lines 523-525 is correct ("Setting only `metaKey` produces `Mod+g`"). Remove lines 518-522 (the stale block) and keep lines 523-525. As verified against `keybindings.ts:206` (`mod = mac ? e.metaKey : e.ctrlKey`) and `:213` (`mac && e.ctrlKey` adds a separate `Ctrl` modifier), setting both on macOS would produce `Mod+Ctrl+g` and miss the `Mod+g` binding — so the stale comment is not just outdated, it would lead a future maintainer to introduce a real bug.
  - Resolved: stale block removed; replacement comment now explicitly cites `keybindings.ts` (`mod = mac ? e.metaKey : e.ctrlKey`) and warns that adding `ctrlKey: true` would produce `Mod+Ctrl+g` on macOS and miss the binding.

- [x] `kanban-app/ui/src/spatial-nav-jump-to.spatial.test.tsx:198-238` — The TS port of `generate_sneak_codes` is a faithful re-implementation of `swissarmyhammer-focus/src/sneak.rs`, but the two now drift independently. The current port matches: alphabet ordering, prefix selection (`pick_two_letter_prefix_count`'s `ceil((count - alphabet_len) / (alphabet_len - 1))`), and the disjoint single/two-letter buckets all line up. However, there is no compile-time link between them. Consider one of: (a) a small fixture file (e.g. `sneak-fixture.json`) emitted from a Rust test that the TS test reads and asserts against, or (b) a property-style test that calls the TS port across `[1, 5, 23, 24, 50, 200, 529]` and asserts shape invariants (length, distinctness, prefix-free) so a future drift in either implementation surfaces as a test failure rather than a silent behavioural divergence. Acceptable as-is for landing this task; worth tracking as a follow-up.
  - Resolved: comment block above `SNEAK_ALPHABET` strengthened with explicit `SOURCE OF TRUTH` / `DRIFT RISK` / `FOLLOW-UP` sections naming `swissarmyhammer-focus/src/sneak.rs` and the three load-bearing items (`SNEAK_ALPHABET`, `generate_sneak_codes`, `pick_two_letter_prefix_count`). Follow-up tracked as kanban task `01KR4F8J3Q4J39BB63AJ9G2W6P` ("Replace TS sneak-code port with fixture-file or property-style cross-language check").

- [x] `kanban-app/ui/src/spatial-nav-jump-to.spatial.test.tsx:893-895` — `expect(E2E_BOARD_PATH).toBeTruthy()` is a no-op assertion to keep the import alive. Mirrors the same pattern in `spatial-nav-end-to-end.spatial.test.tsx:1563-1565`, so it is local convention rather than a defect — but the cleaner fix is to drop the unused import. Either leave for consistency with the sibling file or remove from both in a small follow-up.
  - Resolved: no-op assertion and the now-unused `E2E_BOARD_PATH` import removed from this file. The sibling `spatial-nav-end-to-end.spatial.test.tsx` was left untouched (out of scope for this task).