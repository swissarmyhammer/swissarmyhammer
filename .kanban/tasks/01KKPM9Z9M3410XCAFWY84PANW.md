---
depends_on:
- 01KKPM9FXQ4VWMWKW47VWM1V2F
- 01KKPM87XAZYJ4K2BQKY2TQADN
position_column: done
position_ordinal: ffffffffe480
title: 'HebContext: XDG-aware entry point'
---
## What
Implement `HebContext` that wraps leader election with XDG path resolution. `open()` contests the election, sets up paths, connects to bus. `publish()` writes SQLite first (always), then sends via ZMQ. `subscribe()` and `replay()` round out the API.

**Files**: `heb/src/context.rs` (new), `heb/src/lib.rs` (extend)

**XDG paths**:
- `data_dir`: `$XDG_DATA_HOME/heb/` (default `~/.local/share/heb/`) — database
- `runtime_dir`: `$XDG_RUNTIME_DIR/heb/` (fallback to temp dir) — discovery + IPC sockets

**Key invariant**: `publish()` = SQLite open/write/close FIRST, then ZMQ send. Most reliable path wins.

## Acceptance Criteria
- [ ] `HebContext::open()` contests election and returns working context
- [ ] `publish()` persists to SQLite before sending via ZMQ
- [ ] `subscribe()` returns `HebSubscriber` connected to proxy backend
- [ ] `replay()` reads from SQLite with seq + category filtering
- [ ] XDG paths resolve correctly (with and without env vars set)
- [ ] No discovery file triggers election (not graceful degradation)

## Tests
- [ ] Open context as leader, publish event, verify in SQLite via replay
- [ ] Open two contexts (leader + follower), publish from both, verify all events persisted
- [ ] XDG path resolution with custom env vars
- [ ] `cargo test -p heb` passes