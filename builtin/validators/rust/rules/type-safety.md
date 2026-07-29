---
name: type-safety
description: Use newtypes, avoid adjacent bools, use builders, and seal traits
---

# Rust Type Safety

- **Use newtypes for semantic distinctions.** Two parameters of the same primitive type can have different meanings, for example `user_id: u64, order_id: u64`. You must use newtypes for these parameters. Newtypes add no runtime cost. Newtypes add compile-time safety.
- **Do not use adjacent `bool` parameters.** `Widget::new(true, false)` is hard to read. Use enums instead, for example `Widget::new(Small, Round)`.
- **Use the builder pattern** for structs with three or more optional fields. The method chain must feel natural.
- **Seal public traits** that you do not want downstream crates to implement. Sealing prevents semver hazards when you add methods.
