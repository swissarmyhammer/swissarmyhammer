---
position_column: done
position_ordinal: h4
title: Add unwrap_or guard in get_entity_schema serde_json::to_value
---
**Done.** Replaced `unwrap_or(Value::Null)` with proper error propagation via `map_err` + `collect::<Result<Vec<_>, _>>()?`. Serialization failures now return a clear error to the frontend instead of injecting null.