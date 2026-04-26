---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffd780
title: Document reserved `_`-prefixed pseudo-field dependencies in ARCHITECTURE.md
---
## What

`EntityContext::apply_compute_with_query` supports lazy-injected pseudo-field dependencies for computed fields. A field declares `depends_on: ["_foo"]` and the entity layer sources `_foo` from a non-field source (changelog, filesystem metadata, ...) before derivation, then strips it after.

Today there are two:

- `_changelog` — the entity's JSONL changelog as a JSON array.
- `_file_created` — RFC 3339 timestamp from `Metadata::created().or(modified())`; `Value::Null` on stat failure. (Added by `01KP2GT5C7RGK5BW4G0HSYFW5V`.)

The mechanism is documented only on `apply_compute_with_query`'s docstring. Discovery for someone writing a new computed field requires reading `swissarmyhammer-entity/src/context.rs`. As the list grows this will be a real friction point.

## Approach

Add a short "Computed Fields and Pseudo-Field Dependencies" section to `ARCHITECTURE.md` covering:

- What a pseudo-field is (reserved `_`-prefixed name that never persists).
- How a field opts in (`depends_on: ["_name"]` in YAML).
- The current list of supported names, their source, and their error/missing semantics.
- The rule for adding a new one: add a branch in `apply_compute_with_query`, add a corresponding `entity.fields.remove("_name")` in the strip block, and update the doc list.

Alternatively (or additionally) write a module-level `//!` doc on `swissarmyhammer-entity/src/context.rs` with the same content. `ARCHITECTURE.md` is the right place for cross-crate discoverability; the module doc is the right place for contributors already inside the file.

## Files

- `ARCHITECTURE.md` — add the new section near the "Rust Core" / schema discussion.
- Optional: `swissarmyhammer-entity/src/context.rs` — module-level `//!` block at the top.

## Acceptance Criteria

- [x] A contributor searching for "_changelog" or "_file_created" in `ARCHITECTURE.md` finds a section that explains the mechanism and lists both.
- [x] The section names the file and function (`apply_compute_with_query` in `swissarmyhammer-entity/src/context.rs`) that a new pseudo-field would need to extend.
- [x] No code changes — docs only.

## Non-goals

- Do NOT add new pseudo-fields as part of this card.
- Do NOT rename or reorganize existing ones.
- Do NOT touch user-facing UI docs.
#docs