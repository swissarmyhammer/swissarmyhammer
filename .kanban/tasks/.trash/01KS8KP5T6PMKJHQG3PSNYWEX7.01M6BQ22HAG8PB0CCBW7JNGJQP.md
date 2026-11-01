---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffa180
project: ai-panel
title: Elicitation TextInput drill-out on Escape releases DOM focus
---
#elicitation

What: Mirror ComposerEditorDrillOutWiring for elicitation editable inputs (TextInputControl/TextareaControl) in apps/kanban-app/ui/src/components/ai-elements/elicitation.tsx. On Escape inside the input: blur the active element and dispatch nav.focus with the field scope fq so the panel stops trapping keys. Acceptance Criteria: Escape in a text/number/integer/textarea field blurs DOM focus and returns spatial focus to the field leaf; bare ElicitationFields unit tests stay green (inert no-op outside spatial stack). Tests: Extend ai-panel-elicitation.spatial.test.tsx with a drill-in then Escape assertion (fail before, pass after); tsc --noEmit clean; npx vitest run green.

Done: Implemented useFieldDrillOut + per-kind drill-out (Escape→blur+nav.focus). Verified working in the GUI by the user. Spatial test coverage audited and extended (revert→fail→restore verified) to pin leaf registration per control kind, Enter-activation per button, drill-in per kind, and drill-out per kind. Full UI suite green (2412 tests), tsc clean.