---
assignees:
- claude-code
depends_on:
- 01KS7W1QP1VEHJCANMFHXB4126
- 01KS7W0H79DD7P6DK1SZA1EJ2W
- 01KS7W0SQQDKYZHHB5S3ZYRZMB
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffa080
project: ai-panel
title: Elicitation round-trip regression test (no more silent decline)
---
#elicitation

## Context / Why
The original bug: the agent saw "declined to respond" but the UI never asked. Once the ACP client (advertise+handle), the conversation hook, the form UI, and the agent-wrapper bridges are in place, we need one regression test that exercises the whole client-side seam end to end so this never silently regresses to an auto-decline.

This is a client-side ACP-boundary integration test (it does NOT spawn a real Claude CLI / llama model — those are covered by the per-crate wrapper tests). It drives a mock ACP **agent** that issues a `session/elicitation` request against the real `createKanbanClient` + `useConversation` + `ElicitationPrompt` stack.

## What
Add an integration test (co-located with the AI stack, e.g. `apps/kanban-app/ui/src/ai/elicitation-roundtrip.test.tsx` or extend `acp-client.node.test.ts`):
- [x] Stand up the real client via `createKanbanClient` wired to a real `useConversation` (render the panel/`ElicitationPrompt`), connected to a mock agent over an in-memory stream (reuse the harness already used in `acp-client.node.test.ts`).
- [x] Mock agent: after `initialize`/`newSession`, send a form-mode `unstable_createElicitation` (use the SAH `ask question` shape: `{answer: string}` plus a richer multi-field schema in a second case).
- [x] Assert: the UI surfaces the form (no `methodNotFound`), the user filling + submitting yields an `accept` `CreateElicitationResponse` with correctly typed `content` delivered back to the agent.
- [x] Assert the decline and cancel paths deliver `{action:"decline"}` / `{action:"cancel"}` — and that an unanswered, then `newConversation`-reset elicitation does not strand the agent.
- [x] Assert the negative regression: the client now advertises the elicitation capability at `initialize` and never returns `methodNotFound` for `elicitation/create`.

## Acceptance Criteria
- [x] A mock-agent `session/elicitation` is rendered, answered, and the typed response reaches the agent — for both the single-field and multi-field schemas.
- [x] Decline/cancel actions propagate correctly.
- [x] Capability is advertised; no `methodNotFound` regression.

## Tests
- [x] The new integration test passes: run the kanban-app ui vitest suite — all green.
- [x] Full UI suite stays green: `cd apps/kanban-app/ui && npm test`.

## Workflow
- Use `/tdd`. Build the mock-agent harness from the existing acp-client test setup; assert the round trip.

## Implementation Notes
Added `apps/kanban-app/ui/src/ai/elicitation-roundtrip.test.tsx` — a browser-project (`*.test.tsx`) end-to-end test. It runs the real `createKanbanClient` against a hand-written `MockAgent` (`AgentSideConnection`) over the same in-memory `TransformStream` pair `acp-client.node.test.ts` uses, plugs that real client into the real `useConversation` hook by rendering the full `AiPanel` with a `createConnect` factory that builds the genuine client, and drives the real `ElicitationPrompt` via DOM. The agent issues `unstable_createElicitation` over the wire; the form renders, the user fills/submits, and the typed `accept` content travels back to the agent. 6 tests: capability advertisement, single-field ask-question round trip, multi-field typed coercion, decline, cancel, and a `newConversation` reset of an unanswered request. The browser project hosts both the React render and the in-memory stream (Chromium has `TransformStream` natively), so no production code changed — the panel already wires `useConversation` + `ElicitationPrompt`. New test: 6 passed. Full suite: 252 files / 2400 tests passed; `tsc --noEmit` clean.