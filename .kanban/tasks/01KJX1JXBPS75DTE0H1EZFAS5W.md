---
position_column: done
position_ordinal: e680
title: Mode indicator bottom bar — Normal/Command/Search
---
Phase 1 deliverable from app-architecture.md.

Bottom bar showing the current mode (Normal, Command, Search) plus context info.

## What to build

### Bottom bar component
- Fixed at bottom of the app
- Shows mode: `-- NORMAL --`, `-- COMMAND --`, `-- SEARCH --`
- Shows active view name (for Phase 2, placeholder for now)
- Shows sort/filter indicator (for Phase 5, placeholder for now)

### Mode state
- Three modes: Normal, Command, Search
- Normal: keystrokes dispatch commands via binding table → scope chain
- Command: palette is open (launched by : or Mod+Shift+P)
- Search: / (vim) or Mod+F (cua) — future, placeholder for now

### React integration
- `useAppMode()` hook — exposes current mode and setter
- Mode context provider wraps the app
- Command palette opening sets mode to Command, closing sets back to Normal

## Files
- `ui/src/components/mode-indicator.tsx` — the bottom bar
- `ui/src/lib/app-mode-context.tsx` — mode state context
- Tests

## Checklist
- [ ] AppMode context provider (Normal/Command/Search)
- [ ] useAppMode() hook
- [ ] ModeIndicator bottom bar component
- [ ] Mode switches when palette opens/closes
- [ ] Style matches architecture doc layout
- [ ] Tests
- [ ] Run test suite