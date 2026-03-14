---
depends_on:
- 01KKPM76VA867FDV0JPDW41A3H
position_column: done
position_ordinal: z00
title: Publisher + Subscriber types
---
## What
Implement `Publisher<M>` and `Subscriber<M>` in the leader-election crate. These wrap ZMQ PUB/SUB sockets with typed send/recv via the `BusMessage` trait.

**Files**: `swissarmyhammer-leader-election/src/bus.rs` (extend), `src/error.rs` (add bus error variants)

**Key risk**: `zmq::Socket` is `!Send`. Publisher must be created on the thread that uses it, OR we wrap the socket in a dedicated thread with channel communication. Simplest: create socket lazily on first `send()` in the calling thread, or accept that Publisher is `!Send` and construct it where needed.

## Acceptance Criteria
- [ ] `Publisher<M>` connects PUB socket to frontend, sends typed messages as multipart (topic + frames)
- [ ] `Subscriber<M>` connects SUB socket to backend, receives and deserializes typed messages
- [ ] Topic filtering works (subscribe to specific categories)
- [ ] Error types cover ZMQ and serialization failures

## Tests
- [ ] Publisher/Subscriber round-trip through a running proxy with `NullMessage`
- [ ] Round-trip with a custom `TestMessage` that has real topic + frames
- [ ] Topic filtering: subscriber only receives subscribed categories
- [ ] `cargo test -p swissarmyhammer-leader-election` passes