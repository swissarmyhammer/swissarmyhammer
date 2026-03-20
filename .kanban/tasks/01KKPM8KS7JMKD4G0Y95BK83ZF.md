---
depends_on:
- 01KKPM87XAZYJ4K2BQKY2TQADN
position_column: done
position_ordinal: ffffffffffa280
title: Callback infrastructure
---
## What
Add `on_message` callback registration to election config. Callbacks are serviced by an internal SUB socket thread that connects to the proxy backend and invokes registered callbacks for each received message.

**Files**: `swissarmyhammer-leader-election/src/bus.rs` (extend), `src/election.rs` (config + guard changes)

**Design**: Callbacks stored as `Arc<Mutex<Vec<Box<dyn Fn(&M) + Send>>>>`. Internal SUB thread subscribes to all topics (`b""`), loops recv → deserialize → invoke callbacks. Thread owned by both guard types, stopped on drop.

Separate from proxy thread: proxy forwards blindly, callback thread consumes.

## Acceptance Criteria
- [ ] `on_message(callback)` can be registered on config or election builder
- [ ] Callbacks fire for every message flowing through the bus
- [ ] Callback thread starts on election, stops on guard drop
- [ ] Multiple callbacks can be registered

## Tests
- [ ] Register callback, publish message, verify callback fires (use atomic counter)
- [ ] Multiple callbacks all invoked
- [ ] Callback thread stops cleanly on guard drop
- [ ] `cargo test -p swissarmyhammer-leader-election` passes