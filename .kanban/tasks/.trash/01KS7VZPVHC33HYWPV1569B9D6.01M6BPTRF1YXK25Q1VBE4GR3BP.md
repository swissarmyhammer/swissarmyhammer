---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9880
project: ai-panel
title: 'ACP client: advertise + handle elicitation instead of refusing it'
---
#elicitation

## Context / Why
The webview ACP client (`apps/kanban-app/ui/src/ai/acp-client.ts`) currently REFUSES `unstable_createElicitation` with `RequestError.methodNotFound("elicitation/create")` and advertises NO elicitation capability in `initialize` (clientCapabilities is `fs` only). That refusal is exactly the bug the user reported: "the calling agent sees declined to respond, but the UI never actually asked." The fix is to advertise the capability and forward the request to an injected handler — mirroring the existing `onRequestPermission` seam.

ACP TS SDK types (read `apps/kanban-app/ui/node_modules/@agentclientprotocol/sdk/dist/schema/types.gen.d.ts`):
- `CreateElicitationRequest` = `(ElicitationFormMode & {mode:"form"}) | (ElicitationUrlMode & {mode:"url"})` + `message: string` + `_meta`. Form mode carries `requestedSchema: ElicitationSchema` + scope (`sessionId`/`toolCallId` or `requestId`). Url mode carries `elicitationId` + `url`.
- `CreateElicitationResponse` = `(ElicitationAcceptAction & {action:"accept"}) | {action:"decline"} | {action:"cancel"}`; `ElicitationAcceptAction.content?: {[k]: ElicitationContentValue}`.
- `CompleteElicitationNotification` = `{ elicitationId }` (URL-mode completion / cancellation).
- Client capability field: `ClientCapabilities.elicitation?: ElicitationCapabilities` with `form?` / `url?` sub-caps.

## What
In `apps/kanban-app/ui/src/ai/acp-client.ts`:
- [x] Add `clientCapabilities.elicitation = { form: {}, url: {} }` to the `initialize` call in `createKanbanClient` (alongside the existing `fs` caps).
- [x] Add an `ElicitationHandler` type: `(params: CreateElicitationRequest) => Promise<CreateElicitationResponse>` and a `CompleteElicitationHandler` type `(params: CompleteElicitationNotification) => void`.
- [x] Add `onElicitation: ElicitationHandler` and `onCompleteElicitation: CompleteElicitationHandler` to `KanbanClientOptions`; thread both into `buildClient`.
- [x] Replace `unstable_createElicitation`'s refusal with a forward to `onElicitation` (return its promise).
- [x] Replace `unstable_completeElicitation`'s silent drop with a forward to `onCompleteElicitation` (still returns `Promise<void>`).
- [x] Update the module/`buildClient` docstrings that currently say elicitation is a "deliberate refusal" / "v1 advertises no elicitation capability" so they describe the new behavior.

## Acceptance Criteria
- [x] `initialize` advertises `elicitation: { form: {}, url: {} }`.
- [x] `unstable_createElicitation` resolves with whatever the injected handler returns (accept/decline/cancel), never `methodNotFound`.
- [x] `unstable_completeElicitation` invokes the injected completion handler.
- [x] No other agent->client method behavior changes.

## Tests (`apps/kanban-app/ui/src/ai/acp-client.node.test.ts`)
- [x] Test: a mock agent issues `unstable_createElicitation`; assert the injected `onElicitation` is called with the request and the agent receives the returned `CreateElicitationResponse` (cover accept-with-content, decline, cancel).
- [x] Test: `initialize` handshake sends `clientCapabilities.elicitation` with `form` and `url`.
- [x] Test: `unstable_completeElicitation` calls `onCompleteElicitation`.
- [x] Run: `cd apps/kanban-app/ui && npx vitest run acp-client` — 13 passed. (Project-wide `npm test`'s `tsc --noEmit` step now reports a single expected error in `ai-panel.tsx`: it calls `createKanbanClient` without the two new required options. That wiring is owned by the dependent task 01KS7W1712S865B2Z0TM75BY8A, which threads the handlers through `conversation.ts` -> `aiPanelConnectFactory`. That is by design of the task split — this task intentionally blocks it.)

## Workflow
- Use `/tdd` — write the failing acp-client tests first, then implement.