---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffd980
title: 'Fix: accepting a tag from autocomplete does not save to perspective filter (and disconnects editor)'
---
## What

**Bug — STILL UNFIXED after four passes. User is out of patience.**

User quote (2026-04-17, after pass 4): *"this still is not working. from that i can tell if [sic] the filter you DID NOT FIX IT AT ALL. if i hit enter, i get only 'one save', and then the filter editor is broken and no longer saves. if [I] make multiple edits, even without enter/esc being pressed after the first autosave the filter is no longer able to be edited until i select another perspective and come back."*

**CRITICAL NEW INFORMATION**: the bug is NOT about Enter. **Any autosave** breaks the editor. After the FIRST debounced autosave fires, subsequent typing does nothing. The editor becomes inert until the user switches perspectives and comes back (which remounts via `key={perspectiveId}`).

This rules out every prior pass's theory. It's not `handleCommit`, it's not `commitAndExit`, it's not `committedRef`. It's the **dispatch → backend → event → parent-re-render → child-prop-change cascade** breaking the editor.

## Prior passes (all failed)

1. Widened autocomplete yield. Didn't fix.
2. `handleCommit` → `handleFlush`. Didn't fix.
3. `onCommit` → `onSubmit`. Didn't fix.
4. Refactored TextEditor to a "pure string primitive" and split commit logic to callers. Tests pass, real browser still broken.

Every pass added tests that passed while the app was broken. **The test harness is not exercising the real failure mode.** This must be fixed this pass.

## The real failure mode (finally named)

When the debounced save fires:
1. `apply(text)` dispatches `perspective.filter` via Tauri IPC (async).
2. Backend writes to disk (~10-100ms).
3. Backend fires `entity-field-changed` event.
4. `PerspectiveContext` updates its in-memory cache from the event.
5. React re-renders. `FilterFormulaBar` receives a new `filter` prop value.
6. `FilterEditor` passes that new `filter` prop down to `TextEditor` as `value`.
7. **Something in the render chain here destroys the editor's ability to accept further input.**

The prior tests mock Tauri dispatch synchronously without the async round-trip or the prop-update loop. The scenarios test uses a synchronous mock. The real app has a ~50ms+ async loop. **That timing difference is critical — by the time the re-render arrives, the user may already have typed more characters.** When the editor re-renders with a stale `filter` prop that doesn't match the current doc, `@uiw/react-codemirror` likely force-resets the doc, wiping the user's subsequent typing AND tearing down listeners.

## Hard mandates for this pass

1. **DO NOT CLAIM DONE BASED ON TESTS. Tests have lied four times.**
2. **Reproduce in a real running app before writing a fix.** Launch `pnpm tauri dev` or `pnpm dev` (whichever works without a GUI if possible) and use Playwright to drive it. If Playwright can't drive Tauri, run `pnpm dev` for a Vite-only browser session and mock the Tauri invoke bridge via `window.__TAURI_INTERNALS__`.
3. **The failing test MUST include the full async round-trip**: mock dispatch returns a Promise that resolves after 50ms, AND the mock fires a prop update back to the editor via the parent wrapper. Test scenario: type `#b`, wait 400ms for debounce, verify `#b` dispatched, THEN type `u`, wait 400ms, verify `#bu` also dispatched. Currently pass 4's tests use synchronous mocks and miss this.
4. **Check `@uiw/react-codemirror` value-prop behavior explicitly.** It is a CONTROLLED component — when `value` prop changes and doesn't match the current doc, it force-resets. The prior refactor to a "pure string primitive" passes `value` through but may still be hitting this. Fix: make `TextEditor` accept `value` only as `initialValue` (one-time seed) OR detect and skip the reset when the incoming value matches a recent dispatch the editor itself originated.
5. **Consider whether `@uiw/react-codemirror` should be used at all.** A direct CodeMirror 6 integration (`EditorView`, `EditorState.create`) without the React wrapper may avoid the value-reset problem entirely. Evaluate the blast radius.

## The actual test that must fail on current code and pass on the fix

```typescript
it("dispatch → backend event → prop update does not break the editor", async () => {
  // Simulates the real round-trip: dispatch resolves after 50ms, parent
  // re-renders with the new filter prop the backend would persist.
  const ParentWrapper = () => {
    const [filter, setFilter] = useState("");
    mockInvoke.mockImplementation(async (cmd, args) => {
      if (cmd === "dispatch_command" && args.cmd === "perspective.filter") {
        await new Promise(r => setTimeout(r, 50));
        setFilter(args.args.filter);  // simulate backend event updating prop
      }
    });
    return <FilterEditor filter={filter} perspectiveId="p1" />;
  };

  const { container, view } = await renderEditor(<ParentWrapper />);

  // First edit
  await userEvent.type(view, "#b");
  await new Promise(r => setTimeout(r, 400));  // past debounce + round-trip
  expect(filterDispatches()).toEqual([{ filter: "#b" }]);

  // Second edit — the one that currently breaks
  await userEvent.type(view, "u");
  await new Promise(r => setTimeout(r, 400));
  expect(filterDispatches()).toEqual([{ filter: "#b" }, { filter: "#bu" }]);  // WILL FAIL on current code
});
```

If this test passes on current code, rewrite it until it fails the way the user reports.

## Mode matrix (unchanged)

All scenarios must pass for `keymap_mode = "vim" | "cua" | "emacs"`.

## Acceptance criteria

- [ ] Failing test (as above) committed and demonstrated to fail on current `main`. Trace output pasted into task body.
- [ ] Root cause named concretely (file:line) in task body. "It's the value-prop reset" is not specific enough — identify the exact line in `@uiw/react-codemirror` or `TextEditor` that triggers the break.
- [ ] Fix applied that makes the failing test pass AND keeps all 30 scenarios passing AND full suite green.
- [ ] `pnpm tsc --noEmit` clean.
- [ ] **MANDATORY real-app verification.** If you cannot run Playwright against Tauri, you must document what you DID do (e.g. "built a Vite-only dev page with mocked invoke, ran it in headless Chromium, executed scenario 4, observed XYZ"). Screenshots / logs from the real app are required, not tests.
- [ ] Do NOT move to `review` until all of the above are in the task body.

## Architectural backstop

If after genuine investigation the root cause is `@uiw/react-codemirror`'s controlled-value behavior, **replace the React wrapper with a direct CodeMirror 6 integration** (`EditorView`, `EditorState.create`, imperative dispatch). This is a larger change but may be the only correct fix.

#bug #kanban-app #perspectives #frontend #refactor