---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffe080
title: 'Fix Enter key in filter bar: yield to autocomplete before submitting'
---
## What

Pressing Enter in the filter formula bar when an autocomplete suggestion is selected does nothing — it neither accepts the completion nor submits the filter. The Enter key is swallowed by the submit binding and the completion is left open.

### Root cause

All three Enter-handling paths in `kanban-app/ui/src/lib/cm-submit-cancel.ts` register at `Prec.highest`, which fires **before** CM6's autocomplete plugin can process the key:

1. **`buildCuaExtensions`** — `Prec.highest keymap.of([{ key: "Enter", run: () => { onSubmitRef.current?.(); return true; } }])`
2. **`buildVimEnterExtension` (alwaysSubmitOnEnter: true)** — same Prec.highest pattern
3. **`buildVimEnterExtension` (DOM capture listener)** — capture-phase listener on `view.dom` that also intercepts Enter before autocomplete

When `autocompletion()` is active and a completion is selected, CM6's autocomplete plugin needs to handle Enter to accept the completion. Our `Prec.highest` binding fires first, returns `true` (consumed), and the autocomplete never sees the event.

### Fix: `kanban-app/ui/src/lib/cm-submit-cancel.ts`

Import `completionStatus` from `@codemirror/autocomplete`. In every Enter handler, check `completionStatus(view.state) === "active"` and yield if so:

**`buildCuaExtensions` Enter handler:**
```ts
{
  key: "Enter",
  run: (view) => {
    if (completionStatus(view.state) === "active") return false;
    onSubmitRef.current?.();
    return true;
  },
},
```

**`buildVimEnterExtension` (alwaysSubmitOnEnter path):**
```ts
run: (view) => {
  if (completionStatus(view.state) === "active") return false;
  const text = view.state.doc.toString();
  if (text.length > 0) onSubmitRef.current?.();
  return true;
},
```

**`buildVimEnterExtension` (DOM capture listener path):**
```ts
const handler = (event: KeyboardEvent) => {
  if (event.key !== "Enter") return;
  const cm = getCM(view);
  if (cm?.state?.vim?.insertMode) return;
  if (completionStatus(view.state) === "active") return; // yield to autocomplete
  const text = view.state.doc.toString();
  if (text.length > 0) {
    event.preventDefault();
    event.stopPropagation();
    onSubmitRef.current?.();
  }
};
```

No other files need changes — `buildSubmitCancelExtensions` is the single place all Enter handlers are built.

## Acceptance Criteria
- [ ] Pressing Enter with an autocomplete suggestion selected accepts the completion (inserts the `#tag` or `@user` text into the filter bar)
- [ ] Pressing Enter with no autocomplete showing still submits the filter as before
- [ ] Both CUA and vim modes work correctly

## Tests

All tests in `kanban-app/ui/src/lib/cm-submit-cancel.test.ts`.

Use the real `autocompletion()` + `createMentionAutocomplete` from `@codemirror/autocomplete` to put the editor into an active-completion state, then simulate Enter and assert `onSubmitRef` was NOT called.

- [ ] `"CUA Enter does not submit when autocomplete is active"` — create editor with `autocompletion({ override: [...] })` plus CUA submit-cancel extensions; trigger completion; simulate Enter on `contentDOM`; assert `onSubmitRef` not called
- [ ] `"vim alwaysSubmitOnEnter: Enter does not submit when autocomplete is active"` — same with vim mode + `alwaysSubmitOnEnter: true`
- [ ] `"vim DOM-listener Enter: does not submit when autocomplete is active"` — same with standard vim mode (capture-phase path)
- [ ] Run: `cd kanban-app/ui && npx vitest run src/lib/cm-submit-cancel.test.ts` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.