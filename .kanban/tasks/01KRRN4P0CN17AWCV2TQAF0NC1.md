---
assignees:
- claude-code
depends_on:
- 01KRRN463S53X13YE1PQ1H8P53
position_column: todo
position_ordinal: '8680'
project: ai-panel
title: ACP Client in TypeScript — ClientSideConnection, Client interface, session lifecycle
---
## What
The ACP client — entirely TypeScript, in the webview, using the official `@agentclientprotocol/sdk`.

- New `apps/kanban-app/ui/src/ai/acp-client.ts`.
- `new ClientSideConnection(toClient, stream)` with the WebSocket stream from the "WebSocket ACP message stream" task. `toClient` returns a `Client` implementation handling every agent->client method:
  - `sessionUpdate` — forwarded to the conversation store (next task).
  - `requestPermission` — forwarded to the UI, returns the user's selected option.
  - `readTextFile` / `writeTextFile` / terminal methods — implemented, refuse (capabilities not advertised in v1; the agent does files/shell through the SAH MCP toolset instead).
  - Every remaining `Client` method implemented — nothing left out.
- Drive the `Agent` interface exposed by the connection: `initialize` (send honest client capabilities — fs/terminal not supported), `newSession({ cwd: <board dir>, mcpServers: [ McpServer::Http: the board's full-SAH-toolset URL (`mcpUrl` from `ai_start_agent`) ] })`, `prompt`, `cancel`. Optionally `setSessionMode`.
- Stateless: a fresh `newSession` per chat; nothing persisted.
- The TS SDK and the Rust agent crate version independently — assert a compatible ACP protocol-version negotiation at `initialize` and surface a clear error on mismatch.

## Acceptance Criteria
- [ ] The client connects via `ClientSideConnection` and completes `initialize` + `newSession` (+ `setSessionMode` if used).
- [ ] `newSession.mcpServers` carries the HTTP entry for the board's full SAH MCP toolset.
- [ ] `prompt` runs and `cancel` aborts; every `Client` interface method is implemented (none stubbed `throw`-only except the deliberate capability refusals).
- [ ] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [ ] Vitest integration test against the SDK's example agent (the `@agentclientprotocol/sdk` examples) or a mock agent over an in-memory stream: assert the `initialize` handshake, a `prompt` round-trip, a `requestPermission` round-trip, and that an HTTP `mcpServers` entry was sent in `newSession`.
- [ ] Test the protocol-version mismatch path surfaces a clear error.
- [ ] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the handshake + prompt round-trip test against a mock/example agent first.