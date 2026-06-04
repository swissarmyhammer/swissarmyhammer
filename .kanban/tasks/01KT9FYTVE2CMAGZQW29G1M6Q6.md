---
assignees:
- claude-code
depends_on:
- 01KT9FY7SBW0MVVAZ4A1WZP4SS
- 01KT9JTDE3EX2BQNQ4F3HMZYTP
- 01KT9JV0N1RF0MMEBRZV4A6F3J
position_column: todo
position_ordinal: b380
project: plugin-arch
title: 'SDK: wire this.&lt;server&gt;.on(event, cb) on the dispatch proxy (replace inert reservedHandler)'
---
The ergonomic `.on()` proxy surface, METADATA-DRIVEN. The low-level wire primitive (`Transport.subscribe`/`unsubscribe`, host registry + pump) already landed with the host card (01KT9FY7SBW0MVVAZ4A1WZP4SS). The notification vocabulary is declared per-service in `_meta["io.swissarmyhammer/notifications"]` (cards 01KT9JTDE3 macro + 01KT9JV0N1 declarations). This card builds `this.<server>.on(event, cb)` resolving the event against that `_meta` — exactly mirroring how operations resolve — and replaces the inert reservedHandler.

## Chosen surface (user-approved)
```ts
const off = this.commands.on("executed", (params) => { /* params typed from the decl */ });
off();   // unsubscribe + dispose
```

## Scope (sdk/plugin.ts)
1. **Read `io.swissarmyhammer/notifications` from `_meta`** — add `NOTIFICATIONS_META_KEY` + a `notificationsOf(tool)` reader mirroring `operationsOf` (plugin.ts:711) and a `lookupNotification(server, event) -> method` mirroring `lookupOp` (plugin.ts:725). Resolve via the server's cached tool defs (same `this.tools(server)` cache the dispatch path uses).
2. **Wire `on`/`subscribe` in `makeDispatcher`'s Proxy `get`** (plugin.ts:747-756): return a real `(event, cb) => off` bound to the rooted server. It resolves `event → method` via the notifications `_meta`; throws `UnknownNotification` listing the valid events on a miss (mirror `UnknownOperation`); calls `transport.subscribe(method, cb)` (already implemented). `once` wraps to auto-unsubscribe after first fire. Keep `then` reported absent; keep `RESERVED` only for names that must not extend the path.
3. **Return an off handle** that calls `transport.unsubscribe(method, id)` AND disposes the local callback (`__sahDisposeCallback`) — full teardown (host stops delivery + isolate frees the fn). Auto-cleans on unload via the ledger, so `off()` is optional.
4. Replace the stale "intentionally inert / not part of this SDK task" doc comments on the reserved handler with the real contract.

## Tests
- `this.commands.on("executed", cb)` resolves to `notifications/commands/executed` from `_meta` and round-trips through the transport primitive (extend the real-pipeline test 01KT9FZ8GZ to drive `.on()` against a service that declares the notification).
- Unknown event throws `UnknownNotification` listing valid events.
- `off()` stops delivery and disposes the local callback.

## Acceptance
A plugin calls `this.commands.on("executed", cb)` (no raw transport access), resolved against the service's declared notifications; receives invocations on `notifications/commands/executed`; `off()` stops them and frees the isolate callback; an undeclared event errors clearly.