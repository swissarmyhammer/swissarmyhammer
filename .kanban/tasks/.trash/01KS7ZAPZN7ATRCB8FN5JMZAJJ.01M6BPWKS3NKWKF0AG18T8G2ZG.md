---
assignees:
- claude-code
depends_on:
- 01KS7W0184W1CZPSC1T78ZV5J3
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9c80
project: ai-panel
title: 'Elicitation field controls (AI element): render ElicitationField descriptors as shadcn controls'
---
#elicitation

## Context / Why
Split out of the ElicitationPrompt task to keep each unit under the size limit. This is the presentational layer: a controlled component that turns the `ElicitationField` descriptors (from `apps/kanban-app/ui/src/ai/elicitation.ts`) into shadcn `@/components/ui` controls. Keeping it a pure controlled component (value in, onChange out, no ACP/promise logic) makes every field kind testable in isolation, and keeps it reusable as an AI element. The container/wiring lives in the sibling ElicitationPrompt task.

## What
Create `apps/kanban-app/ui/src/components/ai-elements/elicitation.tsx`:
- [x] `ElicitationFields` component: props `{ fields: ElicitationField[], values, onChange(key, value), errors? }`; renders one labeled control per field with required marker + per-field error text.
- [x] Map each field `kind` to a control: `text`→`Input`, `textarea`→`Textarea`, `select`→shadcn `Select`, `boolean`→`Checkbox`, `number`/`integer`→numeric `Input`, `multiselect`→checkbox group.
- [x] Controlled only — emit typed values via `onChange`; hold no internal form state and contain no submit/accept/decline logic.

## Acceptance Criteria
- [x] Each `ElicitationField.kind` renders the correct shadcn control, labeled, with required markers.
- [x] Editing a control fires `onChange(key, value)` with the value in the field's natural type.
- [x] Provided `errors` render next to the relevant field.

## Tests (`apps/kanban-app/ui/src/components/ai-elements/elicitation.test.tsx`)
- [x] One render+interaction test per field kind asserting the control type and the `onChange` payload type.
- [x] An error passed for a field renders next to it.
- [x] Run the vitest suite for this file — all green.

## Workflow
- Use `/tdd`. Build from the field descriptors produced by the elicitation form-model module.

## Implementation Notes
- New file `apps/kanban-app/ui/src/components/ai-elements/elicitation.tsx` exports `ElicitationFields` (controlled, no internal form state).
- Two shadcn UI wrappers were added because they did not exist yet, following the existing `select.tsx` `radix-ui` wrapping pattern: `apps/kanban-app/ui/src/components/ui/checkbox.tsx` and `apps/kanban-app/ui/src/components/ui/label.tsx`.
- Tests: `apps/kanban-app/ui/src/components/ai-elements/elicitation.test.tsx` — 17 tests, all green in the browser (Chromium) vitest project. `tsc --noEmit` clean; prettier clean.
- Did NOT touch `ai/conversation.ts` or `ai-panel.tsx` (owned by a concurrent task).