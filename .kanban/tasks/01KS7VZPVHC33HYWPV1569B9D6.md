---
assignees:
- claude-code
position_column: todo
position_ordinal: '8880'
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
- [ ] Add `clientCapabilities.elicitation = { form: {}, url: {} }` to the `initialize` call in `createKanbanClient` (alongside the existing `fs` caps).
- [ ] Add an `ElicitationHandler` type: `(params: CreateElicitationRequest) => Promise<CreateElicitationResponse>` and a `CompleteElicitationHandler` type `(params: CompleteElicitationNotification) => void`.
- [ ] Add `onElicitation: ElicitationHandler` and `onCompleteElicitation: CompleteElicitationHandler` to `KanbanClientOptions`; thread both into `buildClient`.
- [ ] Replace `unstable_createElicitation`'s refusal with a forward to `onElicitation` (return its promise).
- [ ] Replace `unstable_completeElicitation`'s silent drop with a forward to `onCompleteElicitation` (still returns `Promise<void>`).
- [ ] Update the module/`buildClient` docstrings that currently say elicitation is a "deliberate refusal" / "v1 advertises no elicitation capability" so they describe the new behavior.

## Acceptance Criteria
- [ ] `initialize` advertises `elicitation: { form: {}, url: {} }`.
- [ ] `unstable_createElicitation` resolves with whatever the injected handler returns (accept/decline/cancel), never `methodNotFound`.
- [ ] `unstable_completeElicitation` invokes the injected completion handler.
- [ ] No other agent->client method behavior changes.

## Tests (`apps/kanban-app/ui/src/ai/acp-client.node.test.ts`)
- [ ] Test: a mock agent issues `unstable_createElicitation`; assert the injected `onElicitation` is called with the request and the agent receives the returned `CreateElicitationResponse` (cover accept-with-content, decline, cancel).
- [ ] Test: `initialize` handshake sends `clientCapabilities.elicitation` with `form` and `url`.
- [ ] Test: `unstable_completeElicitation` calls `onCompleteElicitation`.
- [ ] Run: `cd apps/kanban-app/ui && npm test -- acp-client` (or the repo's vitest invocation) — all green.

## Workflow
- Use `/tdd` — write the failing acp-client tests first, then implement.