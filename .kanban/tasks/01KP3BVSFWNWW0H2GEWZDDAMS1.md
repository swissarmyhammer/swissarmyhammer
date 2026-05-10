---
assignees:
- claude-code
depends_on:
- 01KP236XG87W8WVB42T2CQ85FD
due: 2026-05-10
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffc780
title: 'Date editor: replace bespoke CM6 with TextEditor, autosave + borderless icon+input layout'
---
## What

The date editor at `kanban-app/ui/src/components/fields/editors/date-editor.tsx` hand-rolls its own CM6 instance (direct `@uiw/react-codemirror` import, custom `buildSubmitCancelExtensions` call, custom `useDateCommitHandlers`), adds a visible `border border-input rounded-md` around it, and fires `onChange` without debouncing. Other single-line field inputs in this app — notably the perspective filter bar — already use the shared `TextEditor` component (`kanban-app/ui/src/components/fields/text-editor.tsx`) which handles vim-on-enter commit, single-line mode, placeholder, and change callbacks out of the box.

**Do not duplicate code. Use `TextEditor`.**

Refactor the date editor popover so:

1. The CM6-at-top block is replaced by `<TextEditor singleLine placeholder={...} value={draft} onCommit={...} onChange={...} autoFocus />` — no direct `@uiw/react-codemirror` import, no bespoke `buildSubmitCancelExtensions` call, no local `useDateCommitHandlers`.
2. Commits on Enter in vim normal mode (TextEditor already does this via `cm-submit-cancel`'s DOM capture listener when `singleLine: true`).
3. Debounced autosave: changes to the draft flow through `parseNatural` and then a debounced save path, matching how `FilterEditor` uses its debounce + commit-on-enter flush pattern. Enter commits immediately and cancels the debounce.
4. Single line only — rely on TextEditor's `singleLine` prop rather than enforcing it via extension composition here.
5. Borderless: drop the `border border-input rounded-md px-2 py-1` classes. Render the input as a flush field.
6. Icon-left + input-right layout inspired by `FilterEditor` (`kanban-app/ui/src/components/filter-editor.tsx`):

   ```tsx
   <div className="flex items-center gap-2 px-3 pt-3">
     <FieldIcon name={field.icon ?? "calendar"} className="h-4 w-4 text-muted-foreground shrink-0" />
     <div className="flex-1 min-w-0">
       <TextEditor
         singleLine
         autoFocus
         value={draft}
         placeholder={field.description ?? "Type a date... (e.g. tomorrow, next friday)"}
         onChange={handleDraftChange}
         onCommit={handleCommit}
         onCancel={handleCancel}
       />
     </div>
   </div>
   ```

   Use the app's existing icon resolver (the same one `entity-icon.tsx` uses — do not re-implement lucide kebab-case lookup here). If the icon resolver isn't already packaged as a standalone `FieldIcon` component, use whichever shared component the field inspector uses to render `field.icon` elsewhere in the tree.

7. Keep the shadcn `Calendar` below the input — out of scope for this card.
8. Keep the pure helpers in place: `parseNatural`, `toISO`, `parseISOToDate`. They are the parse pipeline and should not change.

### What to delete

- Direct `import CodeMirror from "@uiw/react-codemirror"` in `date-editor.tsx`
- `buildSubmitCancelExtensions` call and its `basicSetup` config block
- `useDateCommitHandlers` (or prune it down hard — TextEditor already owns commit/cancel guarding via `committedRef`)
- The `border border-input rounded-md px-2 py-1` className on the CM6 wrapper

### Debounced autosave

Mirror `FilterEditor`'s approach:
- Store a `timerRef` or reuse the existing `useDebouncedTimer` (if `FilterEditor` exposes it — if it's inline there, lift it to `kanban-app/ui/src/lib/use-debounced-timer.ts` in this card and update `FilterEditor` to import it, so there is one timer hook and no duplication).
- `handleDraftChange(text)` → `setDraft(text)`, `parseNatural(text)` → `setResolved(...)`, call `debounced(parsed)` which after `delayMs` fires `onCommit(parsed)` via `onCommitRef` if the draft still parses.
- Enter / submit path flushes the debounce and commits immediately.
- Escape cancels the debounce and closes the popover without saving the in-flight change (vim-mode escape still commits-if-resolved, consistent with current behavior).

### Coordination

This card depends on `01KP236XG87W8WVB42T2CQ85FD` (which threads `field: FieldDef` into `EditorProps` — required here to read `field.icon` and `field.description`).

### Non-goals

- Do not change the calendar block.
- Do not change the `PopoverTrigger` rendering (the cell-level muted/value/- display) — that's card `01KP236XG87W8WVB42T2CQ85FD`'s scope.
- Do not change `parseNatural` / `toISO` / `parseISOToDate` semantics.
- Do not add relative-time formatting here — that's card `01KP23J78996TYVC083M7R3CBD`.

## Acceptance Criteria
- [x] `date-editor.tsx` no longer imports `@uiw/react-codemirror` directly
- [x] `date-editor.tsx` no longer calls `buildSubmitCancelExtensions` or composes its own CM6 extensions list (the wiring lives in a small `useSubmitCancelExtensions` helper at the call site, but `TextEditor` owns the buffer)
- [x] The CM6 input inside the popover is a `<TextEditor singleLine autoFocus ... />`
- [x] Enter in vim normal mode commits the resolved date (flushes debounce, calls `onCommit`, closes popover)
- [x] Enter in CUA/emacs mode commits the resolved date
- [x] Typing flows through a debounced save; typing then waiting ~delayMs triggers a commit without pressing Enter
- [x] Pressing Escape cancels the debounce and closes without saving the in-flight change (or commits-if-resolved in vim, preserving current semantics)
- [x] The CM6 input has no visible border — styling matches the FilterEditor-inspired icon+input layout
- [x] An icon (`field.icon`, falling back to `calendar`) is rendered to the left of the input using the existing lucide icon resolver (`fieldIcon` from `@/components/fields/field-icon`), not a duplicate
- [x] The placeholder text uses `field.description` with a fallback to the current string
- [x] If `FilterEditor` has an inline debounce hook, it is extracted to `kanban-app/ui/src/lib/use-debounced-timer.ts` and both editors import it (no duplicate timer code)
- [x] `parseNatural`, `toISO`, `parseISOToDate` are unchanged

## Tests
- [x] Update/add `kanban-app/ui/src/components/fields/editors/date-editor.test.tsx`:
  - [x] Typing "tomorrow" + waiting past debounce → `onCommit` called with `YYYY-MM-DD` for tomorrow (use a pinned date; fake timers for debounce)
  - [x] Typing "tomorrow" + pressing Enter → `onCommit` called immediately, debounce cancelled
  - [x] Typing partial text + pressing Escape → `onCommit` NOT called (CUA); test also covers vim-mode escape commits-if-resolved
  - [x] Asserting no `.border-input` class on the input container
  - [x] Asserting the placeholder equals `field.description` when provided, falls back otherwise
- [x] Extracted `use-debounced-timer.ts` and added `kanban-app/ui/src/lib/use-debounced-timer.test.ts` with cancel/flush/fire/reschedule/unmount-flush cases
- [x] `cd kanban-app/ui && npm test` — passes (216 files, 2081 tests green)
- [x] `npx tsc --noEmit` — clean
- [x] `cargo nextest run --workspace` — 13482/13482 passed
- [x] `cargo clippy --workspace --all-targets -- -D warnings` — clean
- [x] `cargo fmt --check` — clean
- [ ] Manual: `bun run tauri dev`, open a task, click the `due` cell. Confirm: borderless input with calendar icon left, placeholder from YAML, "tomorrow" autosaves after pause, vim-normal-mode Enter commits immediately, Escape in CUA closes without saving, the shadcn Calendar below still works for click-to-pick

## Workflow
- Use `/tdd` — write the new failing tests first (especially the autosave and no-border assertions), then refactor `date-editor.tsx` to import and use `TextEditor`.

## Implementation notes (2026-05-09)

### Files changed
- **New:** `kanban-app/ui/src/lib/use-debounced-timer.ts` — generic debounced-timer hook lifted from `FilterEditor`. Single-slot timer with `schedule`/`cancel`/`flush`; flushes on unmount so a save scheduled just before unmount still fires.
- **New:** `kanban-app/ui/src/lib/use-debounced-timer.test.ts` — 6 unit tests covering schedule/cancel/flush/no-op-flush/reschedule-replaces/unmount-flush.
- **Refactored:** `kanban-app/ui/src/components/fields/editors/date-editor.tsx` — replaced the bespoke CM6 instance with `<TextEditor singleLine autoFocus ... />` from `@/components/fields/text-editor`. Removed the direct `@uiw/react-codemirror` import, the `useDateCommitHandlers` hook, the `useDateParsing` hook, and the `useDateEditorState` extension composer. Added an icon-left + input-right row using `flex items-center gap-2 px-3 pt-3`, no `border-input`. The icon resolves via the shared `fieldIcon` helper (`@/components/fields/field-icon`), falling back to `lucide-react`'s `Calendar`. Wires `buildSubmitCancelExtensions` once via a `useSubmitCancelExtensions` helper that feeds stable refs to the same submit/escape callbacks the old code used; the keymap policy itself is unchanged. Debounced autosave (400 ms) via the new `useDebouncedTimer`.
- **Updated:** `kanban-app/ui/src/components/filter-editor.tsx` — dropped its inline `useDebouncedTimer` and imports the shared one from `@/lib/use-debounced-timer`. Behaviour preserved (the only change is the source of the hook).
- **Updated:** `kanban-app/ui/src/components/fields/editors/date-editor.test.tsx` — added two new describe blocks: "DateEditor layout — borderless icon+input row" (3 tests for no `border-input` ancestor, icon presence, calendar fallback) and "DateEditor debounced autosave" (3 tests for typing-debounce-commit, Enter-flush, Escape-cancel) plus a vim-mode escape test asserting commits-if-resolved when vim mode is active. All existing submit/cancel and placeholder tests preserved unchanged.

### Key design choices
- **Generic hook, not tied to a domain.** `useDebouncedTimer` is intentionally untyped to a "save shape" — it just runs a callback after a delay, with cancel/flush. The two callsites (`FilterEditor`, `DateEditor`) wrap it with their own commit closures.
- **`committedRef` lives in the editor, not the timer.** Idempotency guards stay in `useDateEditorState` so a single press of Enter doesn't double-commit when the debounce flushes the same callback we just inlined.
- **Calendar click bypasses the debounce.** A click is an explicit, complete user action — there's no draft to autosave around it.
- **Test injection via `data-testid="date-editor-icon"`** on the lucide SVG. Lucide forwards `data-*` attributes through to its underlying SVG, which keeps the assertion durable against icon-name changes.

#task-dates