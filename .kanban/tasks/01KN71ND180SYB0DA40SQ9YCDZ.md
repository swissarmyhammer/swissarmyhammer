---
assignees:
- claude-code
depends_on:
- 01KN71N48WTS4MKX2DQ71KC267
position_column: done
position_ordinal: fffffffffffffffffffff680
title: Update flush_changes() to include store_name in event payloads
---
StoreHandle::flush_changes() should emit payloads with store name so the bridge can map events to entity types