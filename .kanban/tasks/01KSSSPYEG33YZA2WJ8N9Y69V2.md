---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffcb80
title: 'SessionStateCache: true LRU + byte-budget eviction'
---
Follow-up from review of d4a69cbe8 (card 01KSSS5H82YC0TX0CM6SQV8CRP, warnings on queue.rs:192 and save_session_state). Pre-existing in the batch path, but the streaming change now exercises it on every ACP turn.

Two issues in `queue.rs`:
1. `evict_oldest_session_states` is NOT LRU despite the name — it iterates `HashMap::keys()` in arbitrary order and drops the first `len-limit`. It can evict the ACTIVE session and keep stale ones, turning a warm turn into an unpredictable cold full-reprocess. Track access/insertion order (e.g. an index map or last-used timestamp) for real LRU.
2. Eviction is by ENTRY COUNT (cpu/2), not bytes. `save_session_state` copies the FULL llama context state (`get_state_size`/`copy_state_data`) per turn; for a 27B model with a large context that can be hundreds of MB per entry. Peak memory = count × full-state-size with no byte ceiling. Add a byte-budget eviction policy.

Both make the KV-reuse win reliable and bound memory on large models.