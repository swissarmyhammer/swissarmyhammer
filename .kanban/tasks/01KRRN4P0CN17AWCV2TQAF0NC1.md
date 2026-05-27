---
assignees:
- claude-code
depends_on:
- 01KRRN463S53X13YE1PQ1H8P53
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff580
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
- [x] The client connects via `ClientSideConnection` and completes `initialize` + `newSession` (+ `setSessionMode` if used).
- [x] `newSession.mcpServers` carries the HTTP entry for the board's full SAH MCP toolset.
- [x] `prompt` runs and `cancel` aborts; every `Client` interface method is implemented (none stubbed `throw`-only except the deliberate capability refusals).
- [x] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [x] Vitest integration test against the SDK's example agent (the `@agentclientprotocol/sdk` examples) or a mock agent over an in-memory stream: assert the `initialize` handshake, a `prompt` round-trip, a `requestPermission` round-trip, and that an HTTP `mcpServers` entry was sent in `newSession`.
- [x] Test the protocol-version mismatch path surfaces a clear error.
- [x] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the handshake + prompt round-trip test against a mock/example agent first.

## Implementation Notes

### Files
- New `apps/kanban-app/ui/src/ai/acp-client.ts` — the ACP client.
- New `apps/kanban-app/ui/src/ai/acp-client.node.test.ts` — 8 integration tests (unit project, `*.node.test.ts`).

### SDK API specifics (`@agentclientprotocol/sdk` v0.21.x, verified against installed `.d.ts`)
- `new ClientSideConnection(toClient, stream)` where `toClient: (agent: Agent) => Client`. The constructed `ClientSideConnection` itself *implements* `Agent`, so it is the object `initialize`/`newSession`/`prompt`/`cancel`/`setSessionMode` are called on.
- `Client` has two required methods (`requestPermission`, `sessionUpdate`); all fs/terminal/ext methods are optional. The task asked for them present anyway as explicit refusals.
- `PROTOCOL_VERSION` is exported as the constant `1`. `InitializeResponse.protocolVersion` is what the agent negotiated.
- `McpServer` is a discriminated union; the HTTP variant is `{ type: "http", name, url, headers: HttpHeader[] }`. The board's loopback `mcpUrl` becomes the single entry with `headers: []`.

### Capability honesty
`initialize` sends `clientCapabilities.fs = { readTextFile: false, writeTextFile: false }` and omits `terminal`. The `Client`'s `readTextFile`/`writeTextFile`/`createTerminal`/`terminalOutput`/`releaseTerminal`/`waitForTerminalExit`/`killTerminal` and `extMethod` reject with `RequestError.methodNotFound(...)` — the same response the SDK synthesizes for an unimplemented optional method, but explicit and self-documenting. `extNotification` resolves silently (notifications expect no response).

### Handler injection
`createKanbanClient(options)` takes `onSessionUpdate` and `onRequestPermission` callbacks. The conversation store is owned by a later task, so this module does not invent it — `sessionUpdate` forwards to the injected `SessionUpdateHandler`, `requestPermission` to the injected `RequestPermissionHandler`. `boardDir` and `mcpUrl` (from `ai_start_agent`) are also injected; `mcpUrl: null` yields an empty `mcpServers` list.

### Version-mismatch handling
After `initialize`, if `initializeResponse.protocolVersion !== PROTOCOL_VERSION` the client throws a typed `AcpProtocolVersionError` carrying `clientVersion` and `agentVersion` with a clear "update the kanban app or the AI agent" message — failing loudly at the handshake rather than deep inside a later `newSession`/`prompt`.

### Statelessness
`createKanbanClient` returns a `KanbanAcpClient` whose `startSession()` issues a fresh `newSession` each call and returns an `AcpSession` (a `sessionId` plus `prompt`/`cancel`/`setMode` bound to it). Nothing is persisted.

### Tests
TDD: a `MockAgent` (`AgentSideConnection` backed by a hand-written `Agent`) is wired to the real client over an in-memory pair of `TransformStream`s through `ndJsonStream` — the same in-process wiring the SDK's own `acp.test.ts` uses. Coverage: initialize handshake (honest capabilities asserted), HTTP `mcpServers` entry in `newSession`, empty `mcpServers` when `mcpUrl` is null, `prompt` round-trip with `sessionUpdate` forwarding, `requestPermission` round-trip (injected handler's choice reaches the agent), `cancel`, `setSessionMode`, and the protocol-version-mismatch error path.

### Verification
- `npm run build` (`tsc && vite build`) — succeeds, exit 0.
- `npm test` (`tsc --noEmit && vitest run`) — `tsc` clean; new `acp-client.node.test.ts` is 8/8 passing. The only failures are the known pre-existing ones excluded by this task: `slugify.parity.node.test.ts` (2 tests) + `board-integration.browser.test.tsx` + `editor-save.test.tsx` are the 3 stale-fixture suites (`01KRS426Q36ZN3DYBX2S0AS82T`); `ai-elements.smoke.test.tsx > CodeBlock` is the Shiki flake (`01KRVG4QSXPQ2FW5SG61M8EHAP`). Zero NEW failures.

## Review Findings (2026-05-17 16:31)

### Nits
- [x] `apps/kanban-app/ui/src/ai/acp-client.ts:236-284` — `buildClient` implements 10 of the SDK `Client` interface's 12 methods. The two optional `unstable_createElicitation` and `unstable_completeElicitation` (verified in `@agentclientprotocol/sdk/dist/acp.d.ts:805,815`) are not present, so the literal "every remaining `Client` method implemented — nothing left out" wording is not strictly met. Behavior is nonetheless correct: the SDK's agent-side router synthesizes `RequestError.methodNotFound` for an absent `unstable_createElicitation` and returns silently for an absent `unstable_completeElicitation` (`acp.js:529-554`) — identical to the deliberate explicit refusals already in place. They are `unstable_`/experimental, not part of the spec, and v1 advertises no elicitation capability. Optional follow-up: add explicit refusal stubs for the two elicitation methods (mirroring the fs/terminal pattern) so the `Client` object visibly enumerates the full interface and self-documents the v1 stance on elicitation. Not a correctness or design defect — accept as-is or address in a tiny follow-up.

### Resolution (2026-05-17)
Addressed: `buildClient` now explicitly implements all 12 `Client` methods. `unstable_createElicitation` rejects with `RequestError.methodNotFound("elicitation/create")` (mirroring the fs/terminal request-refusal pattern); `unstable_completeElicitation` resolves silently (mirroring `extNotification` — it is a fire-and-forget notification). Verified method names, signatures, and types against the installed SDK `.d.ts` (`acp.d.ts:805,815`, `CLIENT_METHODS` in `schema/index.js`). Added `acp-client.node.test.ts` test "refuses elicitation" driving both methods from a `MockAgent` turn: the request rejects with a `method not found` error, the notification resolves silently. `acp-client.node.test.ts` is now 9/9 passing; `tsc --noEmit` clean; full suite `3 failed | 2176 passed` — the 3 failures are the known pre-existing stale-fixture/Shiki ones, zero new.