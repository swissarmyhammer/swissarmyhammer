---
position_column: review
position_ordinal: '8280'
project: ui-command-cleanup
title: Card B — Generalize a webview command handler-bus
---
## What
FOUNDATION. Extract and generalize the existing AI module-bus (`apps/kanban-app/ui/src/ai/commands.ts` — `registerAiCommandHandlers` / `triggerAi*` / handler map keyed by purpose) into a REUSABLE webview command handler registry keyed by **plugin command id**. Webview-only command behaviors (overlay open-state, grid manipulation, field/pressable activation, editor drill-in) register a handler against the plugin command id; `useDispatchCommand` invokes the local handler for ids marked "handled in webview" instead of routing to a backend op.

New module: `apps/kanban-app/ui/src/lib/webview-command-bus.ts` (mirror the ai/commands.ts doc-comment style and cleanup-function semantics):
- `registerWebviewCommandHandler(commandId: string, handler: (ctx: CommandContext) => void | Promise<unknown>): () => void` — install/replace by id, returns an unmount cleanup that only clears the slot it still owns (copy the ownership-guarded delete from `registerAiCommandHandlers`).
- `getWebviewCommandHandler(commandId)` / `hasWebviewCommandHandler(commandId)` — lookup used by the dispatch path.
- `resetWebviewCommandBusForTest()` — test isolation (mirror `resetAiCommandsForTest`).

Define how a plugin-defined command marks "handled in webview" vs "backend execute": a plugin command whose `execute` returns a sentinel (recommend a typed `{ webview: true }` marker, or no backend op at all) is dispatched to the bus. Decide the marker in `apps/kanban-app/ui/src/lib/command-scope.tsx` where `useDispatchCommand`/`runBackendDispatch` live — add the bus lookup BEFORE backend dispatch: if `hasWebviewCommandHandler(id)`, run the handler and skip the backend. Keep `resolveCommand`'s existing execute fast-path untouched in THIS card (it is removed later in Card I once no scope execs remain).

Do NOT migrate any specific behavior here — only the mechanism. Cards C/D/E/F and nav.jump (Card A) depend on this.

## Handler invariant (the guardrail downstream cards must keep)
A bus handler is PURE PRESENTATION — it may touch live webview state (DOM focus, editor instance, grid handle, edit-mode, onPress) and nothing else. It MUST NOT do a durable mutation inline (no store write, no constructing a CommandDef, no MCP transport). Durable effects route BACK through `useDispatchCommand` to a backend-op plugin command (e.g. grid.deleteRow → `${entity}.archive`, board.newTask → `entity.addTask`). This keeps all command logic in Rust and stops the bus becoming a client-side command-logic dumping ground. The invariant is documented in `webview-command-bus.ts` and enforced mechanically by `webview-command-bus.guard.node.test.ts` (fails when a handler-registration site also imports `@/lib/mcp-transport`).

## Acceptance Criteria
- [x] `webview-command-bus.ts` exists with register (id-keyed, ownership-guarded cleanup), lookup, has-check, and test-reset.
- [x] `useDispatchCommand` consults the bus before backend dispatch; a registered webview handler short-circuits the backend call for that id.
- [x] The bus is independent of AI specifics — generic over any plugin command id.
- [ ] `ai/commands.ts` either re-expressed on top of the generic bus OR left as-is with a doc note that it is the precedent (do not regress AI behavior — keep its tests green).
- [x] No command is DEFINED by the bus; it only routes execution of plugin-owned ids.
- [x] GUARD shipped: the presentation-only invariant is documented in the bus module and enforced by `webview-command-bus.guard.node.test.ts` (self-proving detector + live scan; green). Cards C–F each carry a matching acceptance line.

## Tests
- [x] New unit test `apps/kanban-app/ui/src/lib/webview-command-bus.test.ts`: register handler for an id → lookup returns it; cleanup clears only the owned slot; a remount registration is not wiped by an older cleanup (mirror the ai/commands ownership test).
- [ ] Dispatch test (extend `apps/kanban-app/ui/src/lib/command-scope` test or add `command-scope.webview-bus.test.tsx`): a command id with a registered webview handler dispatches to the handler and does NOT call the backend; an id without a handler falls through to backend dispatch.
- [x] Guard test `apps/kanban-app/ui/src/lib/webview-command-bus.guard.node.test.ts`: detector is unit-proven (flags an mcp-transport import, ignores unrelated imports), the bus module stays transport-free, and no handler-registration site imports the transport. `npm test` green.
- [ ] `npm test` (vitest) for the new/changed UI test files is green.

## Workflow
- Use `/tdd` — write the failing bus + dispatch tests first, then implement. Automated tests only (vitest/browser harness).