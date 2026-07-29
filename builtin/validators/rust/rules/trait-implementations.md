---
name: trait-implementations
description: New public types must implement all applicable traits
---

# Rust Trait Implementations

New public types must implement all applicable traits. The orphan rule blocks
downstream crates from adding these traits later. If you do not implement
them now, downstream crates cannot add them.

Check for these traits: `Clone`, `Debug`, `Display`, `Default`, `PartialEq`,
`Eq`, `Hash`, `PartialOrd`, `Ord`, `From`/`TryFrom`, `AsRef`, `Send`/`Sync`.
For pointer types, add compile-time assertions for `Send`/`Sync`.

- For collections, implement `FromIterator` and `Extend`.
- For `serde`, implement `Serialize`/`Deserialize` behind an optional feature flag.
