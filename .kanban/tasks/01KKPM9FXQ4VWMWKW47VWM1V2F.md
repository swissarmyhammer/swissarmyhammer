---
depends_on:
- 01KKPM6TJZA2FKANFJ84W8GZT6
position_column: done
position_ordinal: ffffffffffad80
title: 'heb crate: HebEvent + SQLite store'
---
## What
Create the `heb` crate with `HebEvent` (implementing `BusMessage`), `EventHeader`, `EventCategory`, and SQLite WAL persistence (open/write/close per event).

**Files**: `heb/Cargo.toml` (new), `heb/src/lib.rs` (new), `heb/src/header.rs` (new), `heb/src/store.rs` (new), workspace `Cargo.toml` (add member)

**Dependencies**: swissarmyhammer-leader-election, rusqlite (with bundled feature), serde, serde_json, chrono, thiserror

**EventHeader fields**: seq, timestamp, session_id, cwd, category, event_type, source
**EventCategory**: Hook, Session, Agent, Card, System
**SQLite**: WAL mode, `PRAGMA synchronous=NORMAL`, events table with indexes on (session_id, seq), (category, seq), (cwd, seq)

## Acceptance Criteria
- [ ] `HebEvent` implements `BusMessage` (topic = category bytes, frames = [header_json, body])
- [ ] `EventHeader` serializes/deserializes correctly
- [ ] `log_event()` opens SQLite, writes event, closes connection
- [ ] `replay()` reads events filtered by seq and optional category
- [ ] Database created with correct schema on first write

## Tests
- [ ] HebEvent round-trip through BusMessage trait
- [ ] EventHeader JSON serialization round-trip
- [ ] log_event + replay round-trip
- [ ] Replay with category filter
- [ ] Concurrent writes don't corrupt (WAL mode)
- [ ] `cargo test -p heb` passes