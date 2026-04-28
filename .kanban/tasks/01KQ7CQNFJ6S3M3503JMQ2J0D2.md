---
assignees:
- claude-code
depends_on:
- 01KNQXZ81QBSS1M9WFD7VQJNAJ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffc780
project: spatial-nav
title: Distinguish Shift+Tab from Tab in keybinding normalizer
---
## What

`normalizeKeyEvent` in `kanban-app/ui/src/lib/keybindings.ts` produces the same canonical key `"Tab"` for both Tab and Shift+Tab. The Shift modifier is only prepended for length-1 letter keys; symbolic keys like Tab/Enter/Escape never get the `"Shift+"` prefix even when `e.shiftKey` is true.

The board-view spatial navigation contract (card `01KNQXZ81Q...`) wants:
- `Tab` → `spatial_navigate(boardKey, "right")` (cycle to next column)
- `Shift+Tab` → `spatial_navigate(boardKey, "left")` (cycle to previous column)

This cannot be expressed with the current normalizer because both keystrokes hash to the same canonical string. Tab navigation tests in `board-view.spatial.test.tsx` were deferred to this follow-up card.

## Files to fix

- `kanban-app/ui/src/lib/keybindings.ts` — `normalizeKeyEvent` Shift handling
- `kanban-app/ui/src/lib/keybindings.test.ts` (or equivalent) — add coverage for `Shift+Tab`, `Shift+Enter`, `Shift+Escape`, etc.

## Likely fix

Extend the Shift-prefix logic to apply when `e.shiftKey` is true AND the key is one of a known set of symbolic keys (Tab, Enter, Escape, ArrowUp, ArrowDown, ArrowLeft, ArrowRight, Home, End, PageUp, PageDown, Insert, Delete, Backspace, F1-F12). For these keys, the canonical form becomes `"Shift+Tab"`, `"Shift+Enter"`, etc. Letter and punctuation handling stays as-is.

## Acceptance Criteria

- [x] `normalizeKeyEvent({ key: "Tab", shiftKey: true })` returns `"Shift+Tab"`
- [x] `normalizeKeyEvent({ key: "Tab", shiftKey: false })` returns `"Tab"`
- [x] Same distinction for Enter, Escape, all arrow keys, Home/End/PageUp/PageDown
- [x] Existing letter+Shift bindings (e.g. `"Mod+Shift+P"`) continue to work
- [x] Existing punctuation bindings (e.g. `":"` from Shift+`;`) continue to work
- [x] Update `board-view.spatial.test.tsx` to enable the deferred Tab/Shift+Tab tests once the normalizer fix lands
- [x] `Shift+Space` canonicalises to `"Shift+Space"` (not `"Space"`) — same disambiguation as Tab

## Tests

- [x] `keybindings.test.ts` — `Shift+Tab` and `Tab` produce distinct canonical strings
- [x] `keybindings.test.ts` — `Shift+Enter` distinct from `Enter`
- [x] `keybindings.test.ts` — `Shift+Space` distinct from `Space`
- [x] `board-view.spatial.test.tsx` — Tab/Shift+Tab tests removed from skip list, pass green
- [x] Run `cd kanban-app/ui && pnpm vitest run` — full suite green (1685/1685, +1 from the new Shift+Space regression)
- [x] `pnpm tsc --noEmit` clean
- [x] `cargo build --workspace` clean
- [x] `cargo clippy --workspace -- -D warnings` clean

## Implementation Notes

- Added `SHIFT_PREFIXED_SYMBOLIC_KEYS` constant in `keybindings.ts` enumerating Tab, Enter, Escape, the four arrows, Home/End/PageUp/PageDown, Insert/Delete/Backspace, and F1–F12.
- Extended `normalizeKeyEvent`'s Shift branch with a third clause that prefixes `Shift+` when `e.shiftKey` is true AND the key is in that set.
- Added `Tab → nav.right` and `Shift+Tab → nav.left` to `BINDING_TABLES.cua` so the board-view's deferred test (which fires `keyDown(document, { key: "Tab", shiftKey: false/true })` against the focused board zone) actually has a binding to dispatch. Inspector scopes' `inspector.nextField` / `inspector.prevField` shadow these globals so the form-style "Tab moves between fields" behaviour stays intact.
- Un-skipped the deferred test in `board-view.spatial.test.tsx`, mirrored the arrow-key test's structure with a `tabExpectations` array of two entries, and updated the file's header docblock to document the now-live behaviour.
- Added six new normalizer tests (Shift+Tab, Shift+Enter/Escape, all arrows, Home/End block, Insert/Delete/Backspace, F1–F12) plus a Mod+Shift+Tab combo test and a punctuation-still-works regression.

## Implementation Notes (2026-04-27 follow-up)

- Resolved the latent `Shift+Space` nit raised in the prior review.
- Reordered `normalizeKeyEvent` so the spacebar rewrite (`" "` → `"Space"`) runs *before* the Shift-prefix check. With this ordering, the set-membership test in the Shift branch sees the canonical `"Space"` token, which means `Shift+Space` now produces `"Shift+Space"` rather than `"Space"`.
- Added `"Space"` to `SHIFT_PREFIXED_SYMBOLIC_KEYS` so the set stays semantically clean (canonical names only — no literal `" "` mixed in).
- Updated the comment block above the rewrite to call out the load-bearing ordering, and refreshed the `normalizeKeyEvent` and `SHIFT_PREFIXED_SYMBOLIC_KEYS` docstrings to mention `"Shift+Space"` and `Space`.
- Added the regression test `prefixes Shift on Space to distinguish Shift+Space from Space` to `keybindings.test.ts`, alongside the existing Space tests.

## Review Findings (2026-04-27 08:26)

### Nits
- [x] `kanban-app/ui/src/lib/keybindings.ts` — The Space rewrite (`if (key === " ") key = "Space";`) happens AFTER the Shift-prefix check, and `" "` is not a member of `SHIFT_PREFIXED_SYMBOLIC_KEYS`. As a result, `normalizeKeyEvent({ key: " ", shiftKey: true })` returns `"Space"`, not `"Shift+Space"` — the same disambiguation bug this task fixed for Tab. **Resolved (2026-04-27 follow-up):** Moved the Space rewrite above the Shift-prefix check and added `"Space"` to `SHIFT_PREFIXED_SYMBOLIC_KEYS`; regression test added.