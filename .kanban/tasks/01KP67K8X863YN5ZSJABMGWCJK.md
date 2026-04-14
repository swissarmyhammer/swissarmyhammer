---
assignees: []
position_column: todo
position_ordinal: cc80
project: pill-via-cm6
title: 'Filter autocomplete: accepted tag lost when perspective is toggled before debounce fires'
---
## What

**Bug repro**: In the perspective formula bar, type `#blo`, pause until the autocomplete dropdown shows `#BLOCKING`, press Enter to accept. The editor visually shows `#BLOCKING` (correct). Then toggle to another perspective and back — the saved filter is `#blo`, which matches nothing.

**Root cause** — a race between the 300 ms debounced autosave and perspective remount:

1. `FilterFormulaBar` is keyed by `activePerspective.id` (`kanban-app/ui/src/components/perspective-tab-bar.tsx` near line 219), so switching perspectives **unmounts** `FilterEditor`.
2. `useFilterDispatch` in `kanban-app/ui/src/components/filter-editor.tsx` autosaves via `useDebouncedTimer` with a 300 ms delay (`AUTOSAVE_DELAY_MS`).
3. `useDebouncedTimer` cleanup (`useEffect(() => cancel, [cancel])`) **cancels** any pending timer on unmount — it does not flush it.
4. Timing that triggers the bug:
   - `t=0…300`: user types `#blo`. Each keystroke restarts the debounce.
   - `t~600`: debounce fires with `#blo` → `perspective.filter { filter: "#blo" }` is persisted while the user is still reading the dropdown.
   - `t~700`: user presses Enter. Autocomplete's `apply: "#${slug}"` (`kanban-app/ui/src/lib/cm-mention-autocomplete.ts` line 61) replaces the doc text with `#BLOCKING`. `updateListener` fires → `handleChange` → debounce restarts, scheduled for `t~1000`.
   - `t~800`: user toggles perspective. `FilterFormulaBar` remounts → `useDebouncedTimer` cleanup cancels the `t~1000` save → `#BLOCKING` is never persisted. The already-saved `#blo` stays.

The visual correctness is real: `findMentionsInText` (`kanban-app/ui/src/lib/mention-finder.ts`) does exact slug matching, so the pill only renders when the doc text is exactly `#BLOCKING`. The editor state IS `#BLOCKING` post-acceptance — we just never save it.

### Fix approach — two complementary changes

**Fix 1 — Flush-on-autocomplete-accept (primary signal)**: When a completion is accepted, bypass the debounce and dispatch the save immediately. Accepting a completion is a strong commit signal — the user picked that specific tag. This covers the common case and also handles "click away to a non-perspective target" (no remount, just loss of focus during the debounce window).

CM6's `@codemirror/autocomplete` attaches a `pickedCompletion` annotation to the transaction when a completion is applied (also sets `userEvent: "input.complete"`). We can detect it in an `EditorView.updateListener`.

**Fix 2 — Flush-on-unmount (safety net)**: Even with Fix 1, the debounce race still exists for non-completion edits (raw typing followed by a perspective toggle). Flush pending saves on unmount so no in-flight debounced write is silently dropped by React reconciliation.

### Implementation

- [ ] In `kanban-app/ui/src/components/filter-editor.tsx`, refactor `useDebouncedTimer`:
  - Store the pending callback alongside the timer (new `pendingFnRef`).
  - Add a `flush()` method: if a timer is pending, clear it and invoke the stored callback synchronously.
  - Change the unmount effect from `useEffect(() => cancel, [cancel])` to `useEffect(() => flush, [flush])`.
  - Keep `cancel` for the clear-button path (`handleClear` must still drop, not flush — clear supersedes any pending save).
  - Return `{ schedule, cancel, flush }` from the hook.
- [ ] In `useFilterDispatch`, expose `flush` through the returned API (call it `handleFlush` or similar) so the editor can invoke it on completion accept.
- [ ] In `FilterEditor`, add a CM6 extension that detects completion-accept and triggers `flush`:
  - Import `pickedCompletion` from `@codemirror/autocomplete`.
  - Build an `EditorView.updateListener` extension that iterates `update.transactions`; if any carries a `pickedCompletion` annotation, call the `flush` callback **after** the transaction settles (use `queueMicrotask` or `setTimeout(0)` so `handleChange` — the debounce-scheduler — fires first on the same doc change; then `flush` runs the just-scheduled callback immediately).
  - Add this extension to the `extraExtensions` array passed to `TextEditor` alongside `mentionExts`.
- [ ] Keep the flush extension in `filter-editor.tsx` — do not add it to `useMentionExtensions`. Mention extensions are reused by other editors (task description, etc.) where immediate-dispatch-on-accept isn't the right behavior. The flush-on-accept is specific to the formula-bar autosave model.
- [ ] Verify `handleCommit` (Enter without active completion) already dispatches immediately — it does, via `cancel()` + `apply(text)` — no change needed.
- [ ] Do not change `handleBlur` behavior in `text-editor.tsx`.

### Out of scope

- Shortening `AUTOSAVE_DELAY_MS` (doesn't fix the race).
- Flushing on blur for field editors (separate concern — user didn't report blur-related loss and field editors use a different save model).
- Routing completion-accept-flush through `useMentionExtensions` for all CM6 editors (only the formula bar has debounced autosave; descriptions save via explicit commit).

## Acceptance Criteria

- [ ] After typing `#blo`, accepting `#BLOCKING` from the autocomplete dropdown, and immediately switching perspectives, the saved filter on the originating perspective is `#BLOCKING` (not `#blo`).
- [ ] Accepting a completion dispatches `perspective.filter` immediately (before the 300 ms debounce elapses) — verifiable because `mockInvoke` is called synchronously after the autocomplete transaction.
- [ ] The existing debounced-autosave behavior still works for ordinary typing within a single perspective (no regressions in `filter-editor.test.tsx` autosave tests).
- [ ] Clearing the filter via the × button still cancels pending saves (does not flush stale text after a clear).
- [ ] Enter-to-commit (without active completion) still dispatches immediately.
- [ ] Task-description editors and other mention-autocomplete users are unaffected — no immediate-dispatch behavior added outside the formula bar.

## Tests

- [ ] Add a test in `kanban-app/ui/src/components/filter-editor.test.tsx` (sibling to the existing `autosave` describe): "flushes immediately when a completion is accepted". Render `<FilterEditor filter="" perspectiveId="p1" />`. Acquire the `EditorView`, dispatch a transaction that inserts `#BLOCKING` with a `pickedCompletion.of({label: "#BLOCKING", apply: "#BLOCKING"})` annotation, and assert `mockInvoke` was called with `perspective.filter { filter: "#BLOCKING", perspective_id: "p1" }` **within one microtask/tick** — not after 300 ms.
- [ ] Add a test: "flushes pending autosave on unmount". Dispatch an insert for `#BLOCKING` **without** the `pickedCompletion` annotation (i.e. raw typing), unmount the component before 300 ms elapses, assert `mockInvoke` was called with `#BLOCKING`.
- [ ] Add a test: "does not flush after clear". Render with a filter, click the clear button, then unmount. Assert no extra `perspective.filter` dispatch fires on unmount (only the `perspective.clearFilter` from the button).
- [ ] Add a test: "autocomplete accept then remount preserves accepted tag". Dispatch a doc change to `#blo` (no annotation), wait 400 ms so `#blo` saves, then dispatch a replacement transaction for `#BLOCKING` **with** `pickedCompletion` annotation, then unmount within 100 ms. Assert the final `mockInvoke` call for `perspective.filter` used `#BLOCKING`.
- [ ] Run: `cd kanban-app/ui && npm test -- filter-editor` — all tests pass.
- [ ] Run the full UI suite: `cd kanban-app/ui && npm test` — no regressions.

## Workflow

- Use `/tdd` — write the four new tests first (they should fail against the current `cancel`-on-unmount + no-accept-detection behavior), then add the `flush` method, the `pickedCompletion` updateListener extension, and the unmount-flush effect to make them pass.
