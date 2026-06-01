---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb580
title: Reject auto-tags starting with punctuation in kanban tag parser
---
## What
`crates/swissarmyhammer-kanban/src/tag_parser.rs::parse_tags` extracts a `#tag` as `#` followed by ANY run of non-whitespace, non-`#` characters (inner loop ~lines 51-67: `while end < len && bytes[end] != b'#' && !bytes[end].is_ascii_whitespace() { end += 1 }`). This contradicts the module doc (lines 3-4: "Tags are `#word` where `word` is one or more lowercase alphanumeric characters or hyphens"). So a literal like `#[serial(cwd)]` in a description gets auto-tagged as `[serial(cwd)]):` — exactly what polluted card 01KSR24VH91GS5SN5J3573J6TG (its bogus `[serial(cwd)]:` tag).

Fix `parse_tags` to honor the documented contract:
- The character immediately after `#` MUST be ASCII alphanumeric; otherwise it is NOT a tag (rejects `#<punctuation>`: `#[...]`, `#(foo)`, `#!x`, and leading-hyphen `#-x`).
- Stop the slug at the first character outside the slug charset `[A-Za-z0-9-]` instead of swallowing arbitrary punctuation up to whitespace. This naturally trims trailing punctuation: `#bug,` → `bug`, `#bug.` → `bug`. Document this trailing-trim behavior in the doc comment.
- Change ONLY the slug-extraction loop. Keep the existing guards: preceding-char check (lines 54-55), heading skip (line 29), fenced-block + inline-code skipping.
- Keep `append_tag`/`remove_tag` consistent with the corrected definition (they format `#{slug}` from already-valid slugs, so no behavior change expected — confirm).

Callers that consume `parse_tags` output and need NO change once it's corrected (verify): `crates/swissarmyhammer-kanban/src/task/shared.rs::auto_create_body_tags` and `crates/swissarmyhammer-kanban/src/entity/update_field.rs::auto_create_tags`.

## Acceptance Criteria
- [ ] `parse_tags("#[serial(cwd)]")` returns no tag (regression for the exact polluting case)
- [ ] `parse_tags("#(foo)")`, `parse_tags("#!x")`, `parse_tags("#-x")` return no tag (leading punctuation/hyphen rejected)
- [ ] `parse_tags("#bug")` → `["bug"]`; `parse_tags("#multi-word-tag")` → `["multi-word-tag"]`
- [ ] `parse_tags("#bug,")` and `parse_tags("#bug.")` → `["bug"]` (trailing punctuation trimmed); doc comment updated to state this
- [ ] All existing `tag_parser` tests still pass (headings skipped, code spans skipped, preceded-by-alphanumeric rejected)
- [ ] `cargo clippy -p swissarmyhammer-kanban --all-targets` clean

## Tests
- [ ] Add unit tests to the existing `#[cfg(test)] mod` in `crates/swissarmyhammer-kanban/src/tag_parser.rs` covering every Acceptance Criteria case above, including the `#[serial(cwd)]` regression
- [ ] `cargo nextest run -p swissarmyhammer-kanban tag_parser` passes

## Workflow
- Use `/tdd` — write the failing `#[serial(cwd)]` regression test (and the leading-punctuation rejection tests) first, watch them fail, then correct the slug-extraction loop to make them pass.