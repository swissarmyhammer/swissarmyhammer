---
position_column: todo
position_ordinal: b0
title: PALETTE array has duplicate color entries in tag-inspector
---
In `tag-inspector.tsx` lines 14-31, the PALETTE array has duplicate colors: `0e8a16` appears at indices 3 and 11, `006b75` at indices 4 and 12, `1d76db` at indices 5 and 13. Since the array is rendered with `key={color}`, React will emit duplicate key warnings. Either deduplicate or use index-based keys. #warning