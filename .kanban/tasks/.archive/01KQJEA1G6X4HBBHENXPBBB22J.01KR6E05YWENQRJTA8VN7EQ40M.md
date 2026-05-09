---
assignees:
- claude-code
position_column: todo
position_ordinal: ae80
title: 'vitest: 6 pre-existing browser-mode failures (inspector / board-view drill-in / inspectors-container)'
---
Reproduces on baseline (with perspective-tab-bar changes stashed). Six failing tests across four files:

- src/components/board-view.enter-drill-in.browser.test.tsx
  - enter_on_focused_column_drills_into_first_card
  - enter_on_focused_column_with_remembered_focus_drills_into_remembered_card
- src/components/entity-inspector.field-enter-drill.browser.test.tsx
  - enter_on_pill_field_drills_into_first_pill
  - escape_from_pill_drills_back_to_field_zone
- src/components/inspector.kernel-focus-advance.browser.test.tsx
  - "ArrowDown from the last field stays put, kernel's focused key remains the last field" — expected '/window/inspector/task:T1/field:task:T1.body' but kernel's currentFocus.fq remained '/window/inspector/task:T1' (drill-in did not occur)
- src/components/inspectors-container.test.tsx
  - "opening a second panel does not push another inspector layer" — TypeError: Cannot read properties of undefined (reading 'startsWith') at line 463 (z.moniker is undefined)

Symptoms: kernel does not drill into field/pill zones on Enter, and panel zones registered by InspectorsContainer have undefined moniker. Likely a registration or projection bug in the spatial-nav kernel after the FQM Layer 2b sweep.

Test command:
  cd kanban-app/ui && npx vitest run src/components/inspectors-container.test.tsx src/components/board-view.enter-drill-in.browser.test.tsx src/components/entity-inspector.field-enter-drill.browser.test.tsx src/components/inspector.kernel-focus-advance.browser.test.tsx

NOT caused by branch kanban / 01KQAXPRTCNH8ARTYJJEBTYWW0 (perspective-tab-bar Enter rename) — verified by stashing those changes. #test-failure