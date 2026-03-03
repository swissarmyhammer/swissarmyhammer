---
position_column: done
position_ordinal: f7
title: 'KanbanLookup asymmetry: column uses EntityContext, others use typed I/O'
---
**Done.** The asymmetry was already gone (all types use entity_context). Collapsed the duplicated per-type match arms into generic implementations using KNOWN_TYPES. ~110 lines of boilerplate reduced to ~25.\n\n- [x] Asymmetry resolved — all types unified on entity_context path\n- [x] Boilerplate eliminated — single generic impl for get() and list()\n- [x] JSON shape consistent — all use Entity::to_json()