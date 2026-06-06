---
assignees:
- claude-code
position_column: todo
position_ordinal: d180
title: Commands do not re-mount on Vite hot reload (require full app restart)
---
REOPENED 2026-06-06 — prior work (a lifecycle guard test only; no production fix) was discarded.

## OWNER CONTEXT
On a Vite HMR hot reload the command system does not re-mount — the user must fully restart the app to recover commands. A prior investigation concluded the unit-testable frontend lifecycle (useCommandList / transport / keybinding cleanup) was already correct and shipped only a guard test; that was discarded.

This is very likely entangled with the broader command-surfacing root cause the owner is now driving (commands not reaching the OS menu / palette / jump surfaces). Revisit AFTER the OS-menu + palette command-surfacing work lands, since fixing how commands are surfaced may resolve or reframe the HMR symptom. Keep the focus on the navigation OS menu first. TDD, RED first, and the fix must reproduce the real defect (not a vacuous guard).