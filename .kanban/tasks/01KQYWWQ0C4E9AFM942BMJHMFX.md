---
assignees:
- claude-code
depends_on:
- 01KQYWV9DC866DGRPBRFR17ZEY
position_column: todo
position_ordinal: de80
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

- [ ] All 7 test cases pass.
- [ ] Test does not stub `JumpToOverlay`, `enumerateVisibleScopes`, or `generateSneakCodes` — they're exercised end-to-end.
- [ ] No flakiness — no timing-dependent assertions (no `setTimeout` waiting for the overlay; the overlay is synchronous on key press).

## Tests

- [ ] `kanban-app/ui/src/spatial-nav-jump-to.spatial.test.tsx` exists with the 7 cases above.
- [ ] Test command: `cd kanban-app/ui && pnpm test spatial-nav-jump-to` — passes.
- [ ] Run all spatial tests after this lands: `cd kanban-app/ui && pnpm test spatial` — every pre-existing spatial test still passes.

## Workflow

- Use `/tdd` only on incremental cases — write case 1, watch it fail, make it pass, then case 2, etc. Going one case at a time avoids fix-everything-at-once and keeps the integration debugged piece-by-piece. #nav-jump