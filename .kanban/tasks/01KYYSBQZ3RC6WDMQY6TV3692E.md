---
assignees:
- claude-code
position_column: todo
position_ordinal: da80
title: parse_frontmatter in swissarmyhammer-common still splits frontmatter on a substring
---
A fourth copy of the frontmatter substring-split defect, found while implementing ^a2ef9wh. It sits in the same module as the shared splitter, which makes it the last one.

`crates/swissarmyhammer-common/src/frontmatter.rs`, in `parse_frontmatter_internal`:

```rust
if content.starts_with("---\n") {
    let parts: Vec<&str> = content.splitn(3, "---\n").collect();
```

`"---\n"` is stricter than the bare `"---"` that ^fpcbeth and ^a2ef9wh fixed, but it is still a substring: any line whose text ends in three hyphens -- `title: foo---` -- closes the frontmatter early and drops every key after it.

## Why it was not folded into ^a2ef9wh

^a2ef9wh moved `split_frontmatter_body` into this module and routed the entity readers, ralph state, and the prompt health check through it. `parse_frontmatter` was left alone on purpose: it is a different function with a different contract (it parses the YAML, handles the `{% partial %}` marker, and falls through to "no frontmatter" instead of failing), and its callers reach across `swissarmyhammer-templating` (resolver, prompts, lib), `mirdan::list`, and every prompt and workflow load. That blast radius could not be tested inside the verification scope that card was given.

## Required change

Route `parse_frontmatter_internal` through `split_frontmatter_body`, which now lives a few functions above it in the same file. Drop the `starts_with("---\n")` gate once the splitter owns validity.

Two edge cases move, so prove each with a test before changing the code:

- A closing delimiter at end of file with no terminator (`---\nkey: v\n---`) currently yields "no frontmatter" and comes back as whole content; the splitter reads it as a frontmatter block with an empty body.
- A CRLF file (`---\r\n...`) currently yields "no frontmatter"; the splitter parses it.

## Verification

Run `cargo nextest run` for `swissarmyhammer-common`, `swissarmyhammer-templating`, and `mirdan` -- those are the crates that call `parse_frontmatter`. Existing tests must pass unedited; `test_parse_frontmatter_opening_delimiter_no_closing` in that module names the old `splitn` behaviour in a comment and will need its comment refreshed, not its assertions. #bug