---
name: api-surface-minimality
description: Public API should expose the minimum necessary surface area
---

# API Surface Minimality

dtolnay follows the principle that every public item is a commitment. Once
something is `pub`, removing it is a breaking change. Start private, expose
only what's needed, and think carefully before making anything public.

## What to Check

Examine visibility modifiers and public API decisions in the changed code:

1. **Overly public items**: Structs, functions, or modules marked `pub` that
   are only used within the same crate. Should be `pub(crate)` or private.

2. **Public struct fields**: Struct fields marked `pub` when accessor methods
   would preserve the ability to change the internal representation later.

3. **Public helper functions**: Utility functions made `pub` when they're
   implementation details. Should be private or `pub(crate)`.

4. **Public constructor with all-public fields**: If all fields are `pub`
   and there's also a `new()` constructor, one of these is redundant.
   Either use public fields (for simple data) or private fields with a
   constructor (for invariant-maintaining types).

5. **Leaking internal types**: Public functions that return or accept types
   from internal modules that weren't meant to be part of the API.

## What Passes

- `pub(crate)` for items used across modules within the crate
- Private fields with public accessor methods on types that maintain invariants
- `pub` on items that are genuinely part of the documented API
- `pub` fields on simple data-transfer structs (DTOs) that have no invariants
- `#[non_exhaustive]` on public enums and structs to preserve extensibility
- Re-exporting specific items from internal modules in `lib.rs`

## What Fails

- `pub fn` on a function only called from within the same crate
- `pub` struct fields on a type that also has a `new()` constructor enforcing invariants
- `pub mod internal` or `pub mod util` (implementation detail modules exposed publicly)
- A public function returning a type from a private module
- Making everything `pub` "just in case someone needs it"

## Why This Matters

serde has maintained backwards compatibility for years while evolving rapidly.
This is only possible because the public API surface is carefully controlled.
Every `pub` item dtolnay adds to serde is deliberate and permanent. The smaller
the API surface, the more freedom you have to improve internals.
