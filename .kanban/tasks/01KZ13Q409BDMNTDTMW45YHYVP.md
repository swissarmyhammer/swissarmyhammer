---
position_column: todo
position_ordinal: e680
title: swissarmyhammer-templating frontmatter split trims the closer and miscounts CRLF
---
`parse_frontmatter` and `find_closing_delimiter` in `crates/swissarmyhammer-templating/src/frontmatter.rs` are a private copy of the frontmatter split. They read every prompt and every partial: `PromptLoader::parse_front_matter` in `prompts.rs`, and the partial detect and strip in `resolver.rs`. The module is re-exported from `lib.rs`.

The opener is correctly anchored: three hyphens must be followed by `\n` or `\r`. The copy still holds three defects.

## 1. The closer trims, so an indented delimiter closes early

`find_closing_delimiter` compares `line.trim() == "---"`. A bare `---` indented inside a YAML block scalar therefore closes the frontmatter. The canonical `split_frontmatter_body` compares the whole line, so an indented run stays inside the scalar.

A markdown table separator such as `|---|---|` is safe here, because `trim` does not remove the pipes. A horizontal rule written inside a folded description is not safe.

## 2. The offset is wrong on CRLF

`find_closing_delimiter` accumulates `line.len() + 1` for each line, but `str::lines()` strips `\r\n` whole. On a CRLF file the returned offset is short by one byte for every preceding line, so the body starts inside the text.

## 3. The `strip_prefix("---")` silently does nothing

When the closing delimiter was indented, the slice at the offset starts with spaces, so `strip_prefix("---")` matches nothing and returns the input unchanged. The body then keeps a leading `  ---`.

## Required change

Call `swissarmyhammer_common::frontmatter::split_frontmatter_body`. It is line-anchored, it accepts CRLF, and it returns borrowed slices, so the body stays byte-exact. Do not write another copy.

`swissarmyhammer-templating/Cargo.toml` already depends on `swissarmyhammer-common`. `swissarmyhammer-common` does not depend on `swissarmyhammer-templating`, so there is no cycle.

## Acceptance

- No frontmatter split of the templating crate's own remains.
- RED first: a prompt whose frontmatter holds an indented `---` inside a block scalar keeps every key.
- RED first: a CRLF prompt gives a byte-exact body with no leading `\r` and no lost byte.
- No body ever starts with a leftover `---`.
- No behavior change for well-formed input. Existing tests pass unedited.

Found by the workspace audit on ^8ax9jrq. Related: ^tv3692e.

#bug