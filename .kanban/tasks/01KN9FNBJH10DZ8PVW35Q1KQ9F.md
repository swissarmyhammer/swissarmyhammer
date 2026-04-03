---
assignees:
- claude-code
position_column: todo
position_ordinal: '8980'
title: 'Pattern divergence: PerspectiveContext uses single Option<Arc<StoreHandle>> vs EntityContext''s RwLock<HashMap<String, Arc<StoreHandle>>>'
---
**Severity**: Info (acknowledged difference)\n**Layer**: Design / Pattern following\n**Files**: `swissarmyhammer-perspectives/src/context.rs:38`, `swissarmyhammer-entity/src/context.rs:36`\n\nEntityContext uses `RwLock<HashMap<String, Arc<StoreHandle<EntityTypeStore>>>>` because it manages multiple entity types, each with its own store. PerspectiveContext uses a simple `Option<Arc<StoreHandle<PerspectiveStore>>>` because there's only one perspective store.\n\nThis is a **justified** divergence -- perspectives are a single-type collection. Using a HashMap would be over-engineering. The pattern is: single type gets `Option<Arc<StoreHandle>>`, multi-type gets `RwLock<HashMap<...>>`. No action needed, but this should be the documented convention." #review-finding