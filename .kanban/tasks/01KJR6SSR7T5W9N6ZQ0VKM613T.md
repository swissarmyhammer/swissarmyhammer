---
title: Deduplicate open() and init_entity_context() in context.rs
position:
  column: done
  ordinal: d4
---
**context.rs lines 50-94 and 1012-1044**

`open()` and `init_entity_context()` contain nearly identical code that loads builtin definitions, merges with local overrides, and constructs a `FieldsContext` + `EntityContext`. The two code paths can diverge silently if one is updated but not the other.

**Suggestion:** Extract the shared logic into a private helper like `build_entity_context(root)` that both `open()` and `init_entity_context()` delegate to.

- [ ] Extract shared fields/entity initialization into a private helper
- [ ] Have `open()` call the shared helper
- [ ] Have `init_entity_context()` call the shared helper
- [ ] Verify tests pass after refactor #warning