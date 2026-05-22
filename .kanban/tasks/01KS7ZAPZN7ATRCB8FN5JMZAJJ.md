---
assignees:
- claude-code
depends_on:
- 01KS7W0184W1CZPSC1T78ZV5J3
position_column: todo
position_ordinal: 8f80
project: ai-panel
title: 'Elicitation field controls (AI element): render ElicitationField descriptors as shadcn controls'
---
#elicitation

## Context / Why
Split out of the ElicitationPrompt task to keep each unit under the size limit. This is the presentational layer: a controlled component that turns the `ElicitationField` descriptors (from `apps/kanban-app/ui/src/ai/elicitation.ts`) into shadcn `@/components/ui` controls. Keeping it a pure controlled component (value in, onChange out, no ACP/promise logic) makes every field kind testable in isolation, and keeps it reusable as an AI element. The container/wiring lives in the sibling ElicitationPrompt task.

## What
Create `apps/kanban-app/ui/src/components/ai-elements/elicitation.tsx`:
- [ ] `ElicitationFields` component: props `{ fields: ElicitationField[], values, onChange(key, value), errors? }`; renders one labeled control per field with required marker + per-field error text.
- [ ] Map each field `kind` to a control: `text`→`Input`, `textarea`→`Textarea`, `select`→shadcn `Select`, `boolean`→`Checkbox`, `number`/`integer`→numeric `Input`, `multiselect`→checkbox group.
- [ ] Controlled only — emit typed values via `onChange`; hold no internal form state and contain no submit/accept/decline logic.

## Acceptance Criteria
- [ ] Each `ElicitationField.kind` renders the correct shadcn control, labeled, with required markers.
- [ ] Editing a control fires `onChange(key, value)` with the value in the field's natural type.
- [ ] Provided `errors` render next to the relevant field.

## Tests (`apps/kanban-app/ui/src/components/ai-elements/elicitation.test.tsx`)
- [ ] One render+interaction test per field kind asserting the control type and the `onChange` payload type.
- [ ] An error passed for a field renders next to it.
- [ ] Run the vitest suite for this file — all green.

## Workflow
- Use `/tdd`. Build from the field descriptors produced by the elicitation form-model module.