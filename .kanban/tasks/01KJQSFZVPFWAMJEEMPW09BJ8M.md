---
position_column: done
position_ordinal: e9
title: 'WARNING: no bounds on JSONL changelog file growth'
---
**Resolution:** Added Storage section to module docs documenting growth characteristics. A `limit`/`since_timestamp` parameter was considered but deferred — both require reading the full file anyway without an index, so the optimization is minimal. Compaction noted as a future option in the docs.\n\n- [x] Document expected changelog growth in module docs\n- [x] Consider adding a `limit` or `since_timestamp` parameter to `read_changelog` — deferred, no real perf win without indexing