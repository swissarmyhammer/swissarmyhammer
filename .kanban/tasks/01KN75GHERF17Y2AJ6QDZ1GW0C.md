---
assignees:
- claude-code
depends_on:
- 01KN75G72CGNV9P5TPNVX5V2K6
position_column: done
position_ordinal: ffffffffffffffffff9d80
title: Register PerspectiveStore in StoreContext
---
In kanban-app/src/state.rs, create PerspectiveStore + StoreHandle and register in StoreContext during BoardHandle::open().