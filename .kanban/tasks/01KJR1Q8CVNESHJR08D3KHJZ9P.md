---
position_column: done
position_ordinal: f2
title: EntityContext recreated on every delegation call from KanbanContext
---
**Resolution:** Already fixed. `entity_context()` uses `get_or_try_init` (OnceCell) to lazy-initialize and cache the EntityContext. It is built once on first access and reused for all subsequent calls. No per-call PathBuf clone or repeated initialization.\n\n- [x] Evaluate caching — already cached via OnceCell\n- [x] Doc comment — already has "Lazy-initialized on first access" doc\n- [x] No performance concern — single initialization