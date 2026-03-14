---
position_column: done
position_ordinal: w8
title: Remove ActorType enum and actor_type field entirely
---
Remove the human/agent distinction from actors. An actor is just an actor.

- [ ] Delete `ActorType` enum from `actor/add.rs`, simplify `AddActor` to just `new(id, name)`
- [ ] Remove `AddActor::human()` and `AddActor::agent()` factory methods
- [ ] Remove `actor_type.yaml` field definition
- [ ] Remove `actor_type` from `actor.yaml` entity fields list
- [ ] Remove `actor_type` filter from `ListActors` (and `humans()`/`agents()` methods)
- [ ] Update MCP tool handler to stop branching on type
- [ ] Update schema examples to remove actor_type
- [ ] Update all tests
- [ ] Remove actor_type from frontend test data
- [ ] `cargo test` and `npx vitest run` pass