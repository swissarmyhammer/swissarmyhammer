---
position_column: done
position_ordinal: fffff780
title: Module-level Map caches in mention system may leak
---
**W4: Module-level Map caches grow unbounded**

Several mention-related modules use module-level `Map` objects as caches (slug→color, slug→entity). These are never cleared and grow with each new entity encountered.

**Fix:** Use WeakRef-based caches, add a size cap with LRU eviction, or tie cache lifetime to the EntityStore refresh cycle.