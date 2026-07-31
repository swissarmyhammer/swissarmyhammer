---
assignees:
- claude-code
position_column: todo
position_ordinal: d480
title: Frontmatter split on a bare triple-dash substring corrupts any card whose frontmatter contains one
---
`parse_frontmatter_body` splits the entity file on the **substring** of three hyphens, not on a line-anchored delimiter.

`crates/swissarmyhammer-entity/src/io.rs:345`:

```rust
// Split on --- delimiters: ["", frontmatter, body]
let parts: Vec<&str> = content.splitn(3, "---").collect();
```

A triple dash anywhere inside the frontmatter block ends the frontmatter early. Every key after that point falls into the body and is lost from the entity.

## How it was found

Hit live twice on 2026-07-31 while working ^tnr56gg.

**First hit — a comment.** The implementer wrote a progress comment that quoted a triple dash inside backticks (it was describing this very parser). Comments live in the frontmatter, so the split fired inside the comment. `get task` returned:

- `title` came back empty
- `position.column` came back empty — the card fell off its column
- `description` came back as the tail of the comment plus the leaked `position_column` / `position_ordinal` / `title` lines

**Second hit — this card's own title.** This card was first created with the triple dash quoted in its title. The title is a frontmatter key, so the card corrupted itself on creation: `title` was truncated mid-string and the remainder was prepended to the description. The title had to be reworded to avoid the literal characters.

In both cases the on-disk bytes were still intact; only the read was wrong. That is the dangerous part — the next read-modify-write persists the loss.

## Blast radius

Every frontmatter-carried string is exposed: `title`, and `comments[].text` which is free-form agent and human prose. A triple dash is ordinary in that prose — a markdown horizontal rule, a YAML document marker, a diff hunk, a Rust doc separator, an ASCII table rule. Any of them silently destroys the card.

Descriptions are safe: `splitn(3, ..)` leaves everything after the second delimiter in the body, so a triple dash in the body stays in the body. The exposure is frontmatter-only.

## Required change

Anchor the delimiter to a line. The opening delimiter is a line that is exactly three hyphens at the start of the file; the frontmatter ends at the next line that is exactly three hyphens. A triple dash inside a YAML block scalar (indented, so not a bare delimiter line) must not terminate the frontmatter, and neither must one inside a quoted scalar on a key line.

Check `format_frontmatter_body` on the write side for the mirror-image assumption.

## Acceptance

- A task whose comment text contains a bare triple dash on its own line inside a block scalar round-trips with `title`, `position_column`, and every other frontmatter key intact. Prove it RED first, using the real ^tnr56gg comment text as the fixture.
- A task whose **title** contains a triple dash round-trips with the full title intact. Use this card's original title as the fixture.
- A task whose comment contains an indented triple dash round-trips.
- The existing `parse_frontmatter_body_round_trip` test keeps passing.
- A malformed file with no closing delimiter still returns `EntityError::InvalidFrontmatter` rather than silently treating the whole file as frontmatter.