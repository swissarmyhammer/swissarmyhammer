---
title: Add keymap mode context, persistence, and nav bar selector
position:
  column: done
  ordinal: b6
---
Create a React context for editor keymap mode (CUA/Vim/Emacs) with localStorage persistence, and add a dropdown selector to the nav bar.

**KeymapContext (`ui/src/lib/keymap-context.tsx` new):**
- `KeymapMode` type: `'cua' | 'vim' | 'emacs'`
- `KeymapContext` React context providing `{ mode, setMode }`
- `KeymapProvider` component: reads initial value from `localStorage.getItem('editor-keymap')`, defaults to `'cua'`. On change, writes to localStorage.
- Export `useKeymap()` hook for consumers

**Nav bar changes (`ui/src/components/nav-bar.tsx`):**
- Add a keymap mode dropdown on the right side of the nav bar (after the `ml-auto` spacer)
- Shows current mode label (e.g., "CUA", "Vim", "Emacs") with a small dropdown icon
- Three menu items — click switches mode via `setMode()`
- Active mode gets a check mark, same pattern as the board switcher
- Keep it compact — icon or short label, not a big button

**App.tsx changes:**
- Wrap the app tree in `<KeymapProvider>` so all editors share the setting

**Files:**
- `ui/src/lib/keymap-context.tsx` (new)
- `ui/src/components/nav-bar.tsx` (modify)
- `ui/src/App.tsx` (modify — add provider)

**Verify:**
- Keymap dropdown appears in nav bar
- Switching modes persists across page reload
- Default is CUA

## Checklist
- [ ] Create KeymapContext with provider, hook, and localStorage persistence
- [ ] Add keymap dropdown to nav bar right side
- [ ] Wrap App in KeymapProvider
- [ ] Verify persistence across reload
- [ ] Write tests for context