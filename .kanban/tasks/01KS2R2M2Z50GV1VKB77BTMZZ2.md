---
assignees:
- claude-code
position_column: todo
position_ordinal: 9a80
title: 'AI panel: switching boards must start a fresh ACP session with the new cwd'
---
## What

When the user opens or switches to a different kanban board, the AI panel must start a **brand-new ACP session** so the agent (and the tools it spawns) see the new board's directory as `cwd` and the new board's per-board MCP server.

Today, only **model switches** force a fresh session. `apps/kanban-app/ui/src/components/ai-panel.tsx` mounts `AiPanelConversation` with `key={modelId}` (file header comment "Switching models starts a fresh session"). Board switches are not in the key:

- `AiPanelContainer` (`apps/kanban-app/ui/src/components/ai-panel-container.tsx`) rebuilds `createConnect` via `useProductionConnect(boardDir)` when `boardPath` changes — the *factory* sees the new `boardDir` / new `mcpUrl`.
- But `useConversation` (`apps/kanban-app/ui/src/ai/conversation.ts:1146–1158`) caches `clientRef`/`sessionRef` once built. Changing the `connect` identity rebuilds `ensureSession`, but the cached client + session for the **old** board are not dropped: `ensureSession` short-circuits on `clientRef.current !== null` and never calls the new factory.

Net effect: the second board reuses the first board's `newSession` with the first board's `cwd` (see `apps/kanban-app/ui/src/ai/acp-client.ts:416–420` where `cwd: boardDir` is passed exactly once at `newSession`). The recent commit `78c6c50ea fix(claude-agent): feed per-session MCP servers into the spawned CLI` wired per-session MCP through the spawn config, but per-session MCP is only honored on `newSession` — so this bug also defeats per-board MCP isolation.

### Fix

Key the conversation surface on the active board path in addition to the model id, matching the documented "fresh session per model" pattern. Specifically:

1. `apps/kanban-app/ui/src/components/ai-panel.tsx`
   - In `AiPanel`, change the `AiPanelConversation` `key` from `modelId` to a stable composite of `boardDir` and `modelId` (e.g. `` `${boardDir}::${modelId}` ``). Remove the `void boardDir;` discard and pass `boardDir` through.
   - Update the file-header docstring section "Switching models starts a fresh session" to also state that switching boards starts a fresh session, and update the `key={modelId}` inline comment.

2. `apps/kanban-app/ui/src/components/ai-panel-container.tsx`
   - No code change needed for the keying itself — `boardPath` already flows in via `useActiveBoardPath()` and is passed as `boardDir`. Confirm that `useProductionConnect(boardPath ?? "")` is still the only `createConnect` source on the production path so the new factory carries the new `boardDir`/`mcpUrl`.

3. Defense-in-depth (only if the keying alone is insufficient for the tests below): `apps/kanban-app/ui/src/ai/conversation.ts` — when `connect` identity changes, drop `clientRef.current` and `sessionRef.current` (and any pending `permissionResolverRef`) so the next `sendPrompt` re-runs `connect(handlers)` and `client.startSession()`. Prefer keying-only first; only add this if a test forces it.

Out of scope: any change to the claude-agent crate or to `ai_start_agent` — the backend already handles per-session `cwd`/MCP correctly once a fresh `newSession` is issued.

## Acceptance Criteria

- [ ] Switching the active board in the same window causes the AI panel to tear down the prior ACP client + session and start a fresh `newSession` on the next prompt, with `cwd` equal to the **new** board directory and `mcpServers` reflecting the new board's MCP URL.
- [ ] Switching the active board **without** changing the model still forces a fresh session (today this is broken).
- [ ] Switching the model still forces a fresh session (existing behavior preserved).
- [ ] Re-selecting the same board (no-op switch) does **not** tear the session down.
- [ ] No regression in per-board persistence (`AiPanelState` open/width/model) — already keyed on `boardPath` and unaffected.

## Tests

- [ ] Extend `apps/kanban-app/ui/src/components/ai-panel-container.test.tsx` with a regression test: render the Container with `boardPath="/tmp/board-a"`, send a prompt to trigger `ensureSession`, capture the `cwd` passed to the mocked `newSession`; rerender with `boardPath="/tmp/board-b"`, send another prompt, assert a **second** `newSession` was issued with `cwd: "/tmp/board-b"`. Use the test's existing `ActiveBoardPathProvider` rerender pattern (`apps/kanban-app/ui/src/components/ai-panel-container.test.tsx:115,137`).
- [ ] Extend `apps/kanban-app/ui/src/components/ai-panel.test.tsx` (or add a sibling test file) with a unit test on `AiPanel` directly: render with `boardDir="/a"` + `modelId="m1"`, then rerender with `boardDir="/b"` + `modelId="m1"`, assert the injected `createConnect` is invoked again (mock counter increments) and that the `AiPanelConversation` was remounted (e.g. via an unmount spy / fresh `useConversation` instance).
- [ ] Add a unit test on `useConversation` in `apps/kanban-app/ui/src/ai/conversation.ts` (or its existing test file if one exists — otherwise the container test above is sufficient) verifying that when `connect` identity changes, the next `sendPrompt` invokes the new `connect` factory rather than reusing the cached client. Skip if the keying-only fix in `ai-panel.tsx` makes this unreachable from the production path.
- [ ] `cd apps/kanban-app/ui && pnpm test -- ai-panel-container` passes.
- [ ] `cd apps/kanban-app/ui && pnpm test -- ai-panel` passes.

## Workflow

- Use `/tdd` — write the failing board-switch regression test in `ai-panel-container.test.tsx` first, watch it fail (the second `newSession` will not be issued, or it will be issued with the stale `cwd`), then make it pass via the keying change in `ai-panel.tsx`.