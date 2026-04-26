---
assignees:
- claude-code
position_column: done
position_ordinal: ffffff8680
title: 'heb/store.rs: seq field in header_json diverges from actual DB seq — stored value is always 0'
---
heb/src/store.rs:43-58

`log_event()` serializes the `EventHeader` to `header_json` before insert (line 52), but `header.seq` is 0 at that point. The actual `seq` is assigned by `AUTOINCREMENT` after the insert (line 58). The stored `header_json` will always have `seq: 0`, so when `replay()` deserializes `header_json` the returned headers have the wrong seq (0 instead of their real sequence number).

This is a data correctness bug. Callers using `replay()` will receive headers with seq=0 and cannot use them to resume a subscription position accurately.

Suggestion: after the `INSERT`, update the header's seq field and rewrite `header_json` in a second UPDATE, or store seq as a separate column and reconstruct on read. #review-finding