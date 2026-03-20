---
position_column: done
position_ordinal: ffff9080
title: In-memory entity cache with content hashing
---
Add an in-memory cache layer to `swissarmyhammer-entity` so all reads come from memory and writes update cache + disk atomically.

## Scope

- Define `CachedEntity` struct: `entity: Entity`, `hash: u64` (content hash of serialized form), `version: u64` (monotonic counter)
- Build `EntityCache` wrapping `EntityContext` — `HashMap<(String, String), CachedEntity>` keyed by (entity_type, id)
- `load_all(entity_type)` — reads all entities of a type from disk into cache on startup
- `get(entity_type, id)` — returns from cache, never disk
- `get_all(entity_type)` — returns all cached entities of a type
- `write(entity)` — updates cache + writes to disk, computes new hash, bumps version
- `delete(entity_type, id)` — removes from cache + disk
- `refresh_from_disk(entity_type, id)` — re-reads from disk, compares hash, returns whether changed
- Content hash uses a fast hasher (e.g., `xxhash` or `std::hash`) on the serialized YAML bytes
- Version counter is per-cache-instance, monotonically increasing

## Testing

- Test: `load_all` populates cache from disk, `get` returns entities without disk read
- Test: `write` updates cache and disk, hash changes, version bumps
- Test: `write` with same content does not change hash
- Test: `delete` removes from cache and disk
- Test: `refresh_from_disk` detects external change (modify file, call refresh, hash differs)
- Test: `refresh_from_disk` returns false when file unchanged
- Test: concurrent reads from cache don't block