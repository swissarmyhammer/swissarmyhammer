---
assignees:
- claude-code
position_column: todo
position_ordinal: 8a80
title: entity-field-changed silently drops patches for entities not yet in store
---
rust-engine-container.tsx: entity-field-changed handler\n\nThe handler uses `prev.map(e => e.id !== id ? e : patched)` which only patches entities already present in the store. If a field-changed event arrives before entity-created (race condition during rapid writes), the patch is silently lost.\n\nThe old code did a full `get_entity` re-fetch when `fields` was present, which would upsert. The new architecture intentionally avoids re-fetch, making this race more consequential.\n\nSuggestion: After the `.map()`, check if any entity was actually patched. If not (entity not in store), either log a warning or fetch-and-add as a recovery path. Example:\n```ts\nsetEntitiesFor(entity_type, (prev) => {\n  let found = false;\n  const next = prev.map((e) => {\n    if (e.id !== id) return e;\n    found = true;\n    // ... patch ...\n  });\n  if (!found) console.warn(`[entity-field-changed] entity ${entity_type}/${id} not in store, patch dropped`);\n  return next;\n});\n```\n\nVerification: Write a test that emits entity-field-changed before entity-created and verify the patch is not silently lost. #review-finding