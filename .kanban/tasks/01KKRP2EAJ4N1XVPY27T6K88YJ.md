---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffb980
title: 'heb/header.rs: EventHeader has public fields — permanent API commitment with no getters'
---
heb/src/header.rs:48-65

`EventHeader` has all public fields. Per the Rust review guidelines, public fields are a permanent commitment — adding, removing, or changing field types is a breaking change. The `seq` field is particularly concerning: it is set to `0` on construction and mutated externally after persistence, which is error-prone (easy to use a header whose seq was never updated).

Suggestion: make `seq` private with a `seq()` getter, and expose a `with_seq(u64) -> Self` builder that returns a new header. Other fields can remain public if this is explicitly intended as a data-transfer struct, but document that explicitly.