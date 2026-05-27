---
assignees:
- claude-code
depends_on:
- 01KS7W1712S865B2Z0TM75BY8A
- 01KS7ZAPZN7ATRCB8FN5JMZAJJ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9f80
project: ai-panel
title: 'AI panel: ElicitationPrompt container + conversation wiring'
---
#elicitation

## Context / Why
The panel already renders an inline `PermissionPrompt` from `useConversation`'s `permissionRequest`/`respondPermission` (`apps/kanban-app/ui/src/components/ai-panel.tsx`, `AiPanelConversation` + `PermissionPrompt`). This task adds the sibling `ElicitationPrompt` container and wires it to `elicitationRequest`/`respondElicitation` from the conversation hook. The per-field rendering is delegated to the `ElicitationFields` component (sibling task); this task owns the message, validation orchestration, actions, URL mode, form state, and panel wiring. This is the surface the user asked for: "a UI in our agent panel for elicitation ... using our AI elements ... allow user feedback to elicitation responses."

Uses the pure helpers from `apps/kanban-app/ui/src/ai/elicitation.ts`: `parseElicitation`, `initialFormState`, `validateForm`, `toAcceptResponse`, `declineResponse`, `cancelResponse`; and the `ElicitationFields` control component.

## What
In `apps/kanban-app/ui/src/components/ai-panel.tsx`:
- [x] Add `ElicitationPrompt` mirroring `PermissionPrompt`'s structure (bordered card, `data-slot="ai-elicitation-prompt"`, `role="group"`, `message` heading). Form mode: render `<ElicitationFields>`; below it, **Submit** (accept → `toAcceptResponse`), **Decline**, **Cancel**. URL mode: render the `message` + link to the url + Done/Cancel (no form).
- [x] Hold form state with `useState` seeded by `initialFormState`; reset when the request identity changes; run `validateForm` on submit and block accept (showing errors) when invalid.
- [x] In `AiPanelConversation`, destructure `elicitationRequest` + `respondElicitation` and render `<ElicitationPrompt>` beside the existing `permissionRequest` block in `ConversationContent`.
- [x] Keep interactive controls reachable by spatial nav consistent with `PermissionPrompt`/message actions (reuse the existing pattern, don't invent one).

## Acceptance Criteria
- [x] When `elicitationRequest` is set, the prompt renders the message + `ElicitationFields` (form mode) or the link (url mode).
- [x] Valid submit calls `respondElicitation` with an `accept` response whose `content` matches the typed schema; Decline/Cancel call it with the respective action.
- [x] Missing required field shows an error and does NOT call `respondElicitation`.

## Tests (`apps/kanban-app/ui/src/components/ai-panel.test.tsx`)
- [x] Render `AiPanelConversation` (or `ElicitationPrompt`) with a stubbed conversation exposing a form `elicitationRequest`; fill, submit, assert the `accept` payload to `respondElicitation`.
- [x] Decline and Cancel paths send the right action.
- [x] Missing-required validation blocks submit.
- [x] URL-mode renders a link and no form fields.
- [x] Run the ai-panel vitest suite — all green.

## Workflow
- Use `/tdd`. Use `PermissionPrompt` + its test as the structural template; compose `ElicitationFields` for the inputs.

## Implementation Notes
- `ElicitationPrompt` splits via `parseElicitation` into `ElicitationFormPrompt` (form mode) and `ElicitationUrlPrompt` (url mode), sharing an `ElicitationCard` shell that mirrors `PermissionPrompt`'s bordered card.
- Form state reset is driven by a `key` on `ElicitationFormPrompt` (`elicitationResetKey`): the request object identity (url `elicitationId`, or scope id + serialized schema for form mode), so a new request remounts with a fresh `initialFormState`.
- Buttons follow `PermissionPrompt`'s `Button onClick` pattern (no `AiPanelPressable` wrapper — the permission prompt does not use it either), keeping the two prompts structurally identical.
- Tests scope queries to `[data-slot='ai-elicitation-prompt']` because the composer's send control also has the accessible name "Submit".
- `npx tsc --noEmit` clean; `npx vitest run src/components/ai-panel.test.tsx` 18 passed.