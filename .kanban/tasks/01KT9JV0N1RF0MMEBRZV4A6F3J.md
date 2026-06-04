---
assignees:
- claude-code
depends_on:
- 01KT9JTDE3EX2BQNQ4F3HMZYTP
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffef80
project: plugin-arch
title: 'Notification single-source-of-truth: payload struct = published payload + coverage guard + declare commands/executed'
---
The anti-drift foundation for the whole notification effort. Establishes the pattern every event-migration card follows, and declares the one event that genuinely fires on the bridge today.

PRINCIPLE (user): declared ⟺ raised. The `#[notification]` metadata must never mismatch what's actually published. Solve it structurally, not by vigilance.

## Mechanism
1. **Struct = payload.** A `#[notification]`-decorated struct (from card 01KT9JTDE3) ALSO `#[derive(Serialize)]` and IS the published payload. Publish via `McpNotification::new(payload.method(), serde_json::to_value(&payload)?)` (+ provenance stamp where applicable). The declared param schema (from struct fields) and the emitted params (serialized struct) come from the SAME fields → cannot drift. An unconstructed `#[notification]` struct is dead code (caught by clippy).
2. **Coverage-guard test pattern.** A reusable test asserting, per owning service, that the set of declared notification methods (`io.swissarmyhammer/notifications` `_meta`) EQUALS the set of methods actually published by that service. Fails in either direction (declared-but-unpublished / published-but-undeclared). Document the pattern so each migration card adds its own.

## Reference conversion
- Convert `commands/executed` (the only app-wired bridge publisher today — `BridgeActionSink`, command-service/bootstrap.rs:154; built at txn.rs:129/service.rs:562) to the struct=payload form, and declare it on the command service's operation_tool! (`service.rs:285`). Add its coverage-guard test.

## Acceptance
- The command service declares `commands/executed` in `_meta` and the coverage guard passes (declared == published).
- The struct=payload + guard pattern is in place for migration cards to reuse.
- Unblocks the SDK `.on()` card (one real declared notification now exists to resolve against).