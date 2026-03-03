---
title: 'Add cross-validation test: all builtin computed fields have registered derivations'
position:
  column: done
  ordinal: a2
---
**File:** `swissarmyhammer-kanban/src/defaults.rs` (tests section)

**What:** The builtin YAML definitions and the `kanban_compute_engine()` registrations are maintained independently. There is no test that verifies every `Computed { derive: "X" }` field in the builtins has a matching registration in the engine. The derive name mismatch bug (blocker card) proves this gap exists.

**Why:** Without a cross-validation test, future YAML additions with computed fields will silently fail at runtime. This is a defense-in-depth test that catches the class of bug, not just the instance.

- [x] Add a test `builtin_computed_fields_have_registered_derivations` that:
  1. Builds a `kanban_compute_engine()`
  2. Iterates all `builtin_field_definitions()`, parses each, checks if `type_.kind == computed`
  3. For each computed field, asserts `engine.has(derive_name)`
- [x] Run `cargo nextest run --workspace` to verify #Warning