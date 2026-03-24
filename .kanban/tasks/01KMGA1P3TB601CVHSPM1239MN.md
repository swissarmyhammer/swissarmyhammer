---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffaa80
title: 'App.tsx: add loading indicator during board load/switch'
---
## What

When `refreshBoards()` is in flight (initial mount, board switch via `handleSwitchBoard`, or structural refresh), the user sees either a stale board or the static \"No board loaded\" placeholder with no visual indication that data is loading. Add a `loading` state to `App.tsx` that shows a spinner/skeleton while board data is being fetched.

**Files to modify:**
- `kanban-app/ui/src/App.tsx` — add `const [loading, setLoading] = useState(true)`, set it `true` before `refreshBoards()` calls (lines 207, 432, 462, 476), set it `false` after. Replace the `!board` branch (line 578) with a loading indicator when `loading` is true, and only show \"No board loaded\" when `loading` is false and `board` is still null.

**Approach:** Wrap the `refreshBoards` call sites in `setLoading(true)` / `setLoading(false)`. Use a simple centered spinner (lucide `Loader2` with `animate-spin`) — no skeleton, no shimmer. Keep it minimal.

## Acceptance Criteria
- [ ] A spinning loader is visible while board data is being fetched on initial mount
- [ ] A spinning loader is visible when switching between boards via the board selector
- [ ] \"No board loaded\" only appears after loading completes with no board available
- [ ] The loader does not flash — if the load is fast (<200ms), no visible flicker (use a short delay or transition)

## Tests
- [ ] `kanban-app/ui/src/App.test.tsx` — test that the loading indicator renders when `refreshBoards` is pending (mock invoke to return a delayed promise, assert spinner is in the DOM)
- [ ] `kanban-app/ui/src/App.test.tsx` — test that \"No board loaded\" appears only after loading resolves with empty boards
- [ ] Run `cd kanban-app/ui && npx vitest run App` — all tests pass