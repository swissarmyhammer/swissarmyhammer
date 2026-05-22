---
assignees:
- claude-code
depends_on:
- 01KS7W1QP1VEHJCANMFHXB4126
- 01KS7W0H79DD7P6DK1SZA1EJ2W
- 01KS7W0SQQDKYZHHB5S3ZYRZMB
position_column: todo
position_ordinal: '8e80'
project: ai-panel
title: Elicitation round-trip regression test (no more silent decline)
---
#elicitation

## Context / Why
The original bug: the agent saw "declined to respond" but the UI never asked. Once the ACP client (advertise+handle), the conversation hook, the form UI, and the agent-wrapper bridges are in place, we need one regression test that exercises the whole client-side seam end to end so this never silently regresses to an auto-decline.

This is a client-side ACP-boundary integration test (it does NOT spawn a real Claude CLI / llama model — those are covered by the per-crate wrapper tests). It drives a mock ACP **agent** that issues a `session/elicitation` request against the real `createKanbanClient` + `useConversation` + `ElicitationPrompt` stack.

## What
Add an integration test (co-located with the AI stack, e.g. `apps/kanban-app/ui/src/ai/elicitation-roundtrip.test.tsx` or extend `acp-client.node.test.ts`):
- [ ] Stand up the real client via `createKanbanClient` wired to a real `useConversation` (render the panel/`ElicitationPrompt`), connected to a mock agent over an in-memory stream (reuse the harness already used in `acp-client.node.test.ts`).
- [ ] Mock agent: after `initialize`/`newSession`, send a form-mode `unstable_createElicitation` (use the SAH `ask question` shape: `{answer: string}` plus a richer multi-field schema in a second case).
- [ ] Assert: the UI surfaces the form (no `methodNotFound`), the user filling + submitting yields an `accept` `CreateElicitationResponse` with correctly typed `content` delivered back to the agent.
- [ ] Assert the decline and cancel paths deliver `{action:"decline"}` / `{action:"cancel"}` — and that an unanswered, then `newConversation`-reset elicitation does not strand the agent.
- [ ] Assert the negative regression: the client now advertises the elicitation capability at `initialize` and never returns `methodNotFound` for `elicitation/create`.

## Acceptance Criteria
- [ ] A mock-agent `session/elicitation` is rendered, answered, and the typed response reaches the agent — for both the single-field and multi-field schemas.
- [ ] Decline/cancel actions propagate correctly.
- [ ] Capability is advertised; no `methodNotFound` regression.

## Tests
- [ ] The new integration test passes: run the kanban-app ui vitest suite — all green.
- [ ] Full UI suite stays green: `cd apps/kanban-app/ui && npm test`.

## Workflow
- Use `/tdd`. Build the mock-agent harness from the existing acp-client test setup; assert the round trip.