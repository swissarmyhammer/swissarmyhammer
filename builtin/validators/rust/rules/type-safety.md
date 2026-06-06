---
name: type-safety
description: Newtypes, no adjacent bools, builders, and sealed traits
severity: warn
---

# Rust Type Safety

- **Newtypes for semantic distinctions.** Two parameters of the same primitive type with different meanings (e.g., `user_id: u64, order_id: u64`) must use newtypes. Zero runtime cost, compile-time safety.
- **No adjacent `bool` parameters.** `Widget::new(true, false)` is unreadable. Use enums: `Widget::new(Small, Round)`.
- **Builder pattern** for structs with 3+ optional fields. Method chaining should feel natural.
- **Sealed traits** for public traits not meant to be implemented downstream. Prevents semver hazards when adding methods.
