---
depends_on:
- 01KKPM6TJZA2FKANFJ84W8GZT6
position_column: done
position_ordinal: ffffffffff9c80
title: Discovery file + ZMQ proxy thread
---
## What
Add `zmq` C bindings dependency. Implement discovery file read/write and XPUB/XSUB proxy thread in leader-election crate.

**Files**: `swissarmyhammer-leader-election/src/discovery.rs` (new), `src/proxy.rs` (new), `Cargo.toml`, workspace `Cargo.toml`

Discovery file at `{base_dir}/{prefix}-bus-{hash}.addr` — two lines: front_addr, back_addr. IPC addresses: `ipc://{base_dir}/{prefix}-bus-{hash}-{front|back}.sock`.

Proxy: bind XSUB on front, XPUB on back, `zmq::proxy()` forwarding loop. `ProxyHandle` struct owns thread + stop flag. Drop stops proxy cleanly via context destruction.

## Acceptance Criteria
- [ ] `zmq` dependency added to workspace and leader-election Cargo.toml
- [ ] Discovery file write/read/remove works
- [ ] Proxy starts, forwards messages, stops cleanly
- [ ] `ProxyHandle::drop()` stops proxy thread within 200ms

## Tests
- [ ] Discovery file round-trip (write, read, verify, remove)
- [ ] Proxy start/stop lifecycle
- [ ] `cargo test -p swissarmyhammer-leader-election` passes