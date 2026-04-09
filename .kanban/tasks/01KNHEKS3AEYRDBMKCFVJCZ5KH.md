---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffe480
title: Redundant useMemo identity wrapper for entityStore
---
rust-engine-container.tsx: near end of RustEngineContainer\n\n`const entityStore = useMemo(() => entitiesByType, [entitiesByType])` is an identity memo — it returns the same reference it was given and re-runs whenever entitiesByType changes. This provides no memoization benefit; it's equivalent to `const entityStore = entitiesByType`.\n\nSuggestion: Remove the useMemo wrapper and pass entitiesByType directly to EntityStoreProvider.\n\nVerification: Remove the useMemo, run tests, confirm no behavior change. #review-finding