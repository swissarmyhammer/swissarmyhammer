---
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8380
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
- [x] `ai/commands.ts` either re-expressed on top of the generic bus OR left as-is with a doc note that it is the precedent (do not regress AI behavior — keep its tests green). *(Option B taken 2026-06-10: precedent doc note added to the "# Why a module-level registry" section referencing `@/lib/webview-command-bus`; `ai/commands.test.ts` green.)*
- [x] No command is DEFINED by the bus; it only routes execution of plugin-owned ids.
- [x] GUARD shipped: the presentation-only invariant is documented in the bus module and enforced by `webview-command-bus.guard.node.test.ts` (self-proving detector + live scan; green). Cards C–F each carry a matching acceptance line.

## Tests
- [x] New unit test `apps/kanban-app/ui/src/lib/webview-command-bus.test.ts`: register handler for an id → lookup returns it; cleanup clears only the owned slot; a remount registration is not wiped by an older cleanup (mirror the ai/commands ownership test).
- [x] Dispatch test (extend `apps/kanban-app/ui/src/lib/command-scope` test or add `command-scope.webview-bus.test.tsx`): a command id with a registered webview handler dispatches to the handler and does NOT call the backend; an id without a handler falls through to backend dispatch. *(Verified satisfied by 2026-06-10 review: `command-scope.webview-bus.test.tsx` exists and covers short-circuit, fallthrough, return-value propagation, cleanup fallback, and execute fast-path precedence.)*
- [x] `npm test` (vitest) for the new/changed UI test files is green. *(Re-verified 2026-06-10 after the doc-note + nit edits: scoped vitest over webview-command-bus.test.ts, webview-command-bus.guard.node.test.ts, command-scope.webview-bus.test.tsx, command-scope.test.tsx, ai/commands.test.ts — 5 files, 75/75 passed; `npx tsc --noEmit` exit 0.)*

## Workflow
- Use `/tdd` — write the failing bus + dispatch tests first, then implement. Automated tests only (vitest/browser harness).

## Review Findings (2026-06-10 07:05)

Unchecked-item dispositions at current HEAD (verified by reading the files and running the scoped vitest suite — `tsc --noEmit && vitest run` over `webview-command-bus.test.ts`, `webview-command-bus.guard.node.test.ts`, `command-scope.webview-bus.test.tsx`, `command-scope.test.tsx`, `ai/commands.test.ts`: 5 files, 75/75 tests passed):

- **AC "ai/commands.ts re-expressed OR doc-noted as precedent"** — NOT satisfied. `src/ai/` contains zero uses of `registerWebviewCommandHandler` (not re-expressed), and `ai/commands.ts` contains no mention of `webview-command-bus` or "precedent" — its header still cites `perspective-tab-bar.tsx` as "the established pattern". The cross-reference is one-directional (bus → ai only). AI behavior is NOT regressed: `src/ai/commands.test.ts` is green. Only the doc-note artifact is missing — check this AC after adding it. *(RESOLVED 2026-06-10: doc note added.)*
- **Test "dispatch test"** — SATISFIED. `apps/kanban-app/ui/src/lib/command-scope.webview-bus.test.tsx` exists and covers: registered handler short-circuits the backend (`callCommandTool` not invoked), unregistered id falls through to backend dispatch, handler return value propagated, unregistered-after-cleanup falls back to backend, and scope `execute` fast-path still wins over the bus. All green. Check this box. *(Box checked.)*
- **Test "npm test green"** — SATISFIED for the new/changed UI test files (75/75 passed, typecheck clean). Check this box once the doc-note edit above lands and the scoped run is re-verified. *(Re-verified post-edits: 75/75, tsc clean. Box checked.)*

### Warnings
- [x] `apps/kanban-app/ui/src/ai/commands.ts:1` — Missing the AC-required doc note. The module doc-comment should state that this AI module-bus is the precedent the generic `webview-command-bus.ts` was generalized from, and that it is intentionally left on its own purpose-keyed registry (window-layer `execute` closures call `triggerAi*` directly) rather than migrated onto the id-keyed bus. Add a short paragraph in the "# Why a module-level registry" section referencing `@/lib/webview-command-bus`. Doc-only change; keep `ai/commands.test.ts` green. *(FIXED 2026-06-10: precedent paragraph added after the `perspective-tab-bar.tsx` paragraph; doc-only, `ai/commands.test.ts` green.)*

### Nits
- [x] `apps/kanban-app/ui/src/lib/command-scope.tsx:521` — `if (hasWebviewCommandHandler(cmdId)) { return getWebviewCommandHandler(cmdId)!(opts); }` does two map lookups and needs a non-null assertion. `const webviewHandler = getWebviewCommandHandler(cmdId); if (webviewHandler) { return webviewHandler(opts); }` is one lookup and assertion-free. *(FIXED 2026-06-10: collapsed to single `getWebviewCommandHandler` lookup with null check; the now-unused `hasWebviewCommandHandler` import removed from command-scope.tsx — the export itself remains in the bus module per AC.)*