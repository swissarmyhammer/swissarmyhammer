---
assignees:
- claude-code
depends_on:
- 01KT9FY7SBW0MVVAZ4A1WZP4SS
- 01KT9FYTVE2CMAGZQW29G1M6Q6
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff380
project: plugin-arch
title: 'Event API: real-pipeline integration test (plugin subscribes to commands/executed, real execute fires the callback)'
---
End-to-end proof of the SDK event-subscription API through the REAL pipeline — not a mock-boundary unit test (see feedback: real-path-tests-not-mocks, fixture-only-anti-pattern). Depends on the host + SDK cards.

## The test
In crates/swissarmyhammer-plugin/tests/ (reuse the existing plugin-load integration harness): load a real plugin bundle through a real V8 isolate whose `load()` calls `this.commands.on("executed", cb)`. Then execute a real command through the command service so the existing production publisher (`BridgeActionSink` → `notifications/commands/executed`, command-service/src/bootstrap.rs:136-158 + service.rs:562) fires. Assert the plugin's callback ran with the executed command's params.

Why commands/executed: it is the ONE notification plane with a real production publisher today (per the current-state map), so the API is provable with zero new publishers.

## Also cover
- `off()` / unsubscribe stops further deliveries (execute again → callback not called).
- plugin unload stops deliveries (no leak / no panic on a published notification after unload).
- (optional) an example bundle under examples/plugins/ exercising `.on()` if it fits the existing example-suite pattern — otherwise leave examples to the plugin-examples project.

## Acceptance
A committed integration test loads a real isolate, subscribes via `.on()`, triggers a real `execute`, and observes the callback firing with correct params; teardown paths verified.