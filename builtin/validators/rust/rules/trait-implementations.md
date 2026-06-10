---
name: trait-implementations
description: New public types must implement all applicable traits
severity: warn
---

# Rust Trait Implementations

New public types must implement all applicable traits. Due to orphan rules, if you don't, downstream crates can't add them.

Check for: `Clone`, `Debug`, `Display`, `Default`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`, `From`/`TryFrom`, `AsRef`, `Send`/`Sync` (add compile-time assertions for pointer types).

- Collections: implement `FromIterator` and `Extend`.
- `serde`: `Serialize`/`Deserialize` behind an optional feature flag.
- A new public type missing obvious trait impls is a silent semver hazard.
