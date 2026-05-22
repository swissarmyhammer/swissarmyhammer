---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffa480
title: Integrate AI panel elicitation form into spatial-nav / jump-to system
---
What: Wire ElicitationFields controls (elicitation.tsx) and the action buttons + url prompt (ai-panel.tsx) into the app's focus-scope/spatial-nav/jump-to system using AiPanelPressable/AiPanelFocusScope so each control is a spatial leaf; Enter activates a focused button (Submit/Decline/Cancel/Done) or drills into a focused input. TDD: failing spatial test first, then implement. Acceptance Criteria: each elicitation control registers a spatial leaf path-descendant of the ui:ai-panel zone; Enter on Submit fires accept, Enter on Decline/Cancel fires their action; existing non-spatial ai-panel.test.tsx and elicitation.test.tsx stay green. Tests: new ai-panel-elicitation.spatial.test.tsx (fail->pass); npx tsc --noEmit clean; npx vitest run green.