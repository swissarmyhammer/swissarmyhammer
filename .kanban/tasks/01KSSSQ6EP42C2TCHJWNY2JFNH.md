---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
title: 'KV-cache reuse requires single-worker: document or add per-session serialization'
---
Follow-up from review of d4a69cbe8 (card 01KSSS5H82YC0TX0CM6SQV8CRP, concurrency warning).

`SessionStateCache` (`queue.rs:29`, `Arc<Mutex<HashMap<session_id, Vec<u8>>>>`) has no per-session lock around the restore→generate→save sequence. Default `worker_threads: 1` (configs.rs) serializes all turns, so it is safe today. With `worker_threads > 1`, two concurrent turns of the SAME session can interleave (worker A restores while worker B saves) and corrupt KV state. The batch path shares this assumption.

Action: either document loudly (config docs + a debug_assert / startup warning) that KV-cache reuse assumes a single worker, or add a per-session lock so the restore→generate→save critical section is atomic per session, allowing multi-worker safely. Pairs with the content-fingerprint card (01KSSSPN67B23A0B8TRPCRNC34) which would also harden this.