---
title: Add markdown tag parser and auto-color to kernel
position:
  column: done
  ordinal: a5
---
Add a `tag_parser` module and `auto_color` module to the kanban kernel crate. These are pure library functions with no behavioral changes to existing operations.

**tag_parser.rs:**
- `parse_tags(text: &str) -> Vec<TagMatch>` — extract `#tag` patterns from markdown text using regex: `(?m)(?:^|(?<=\s))#([\w\p{Emoji_Presentation}\p{Extended_Pictographic}][\w\p{Emoji_Presentation}\p{Extended_Pictographic}/\-]*)`
- `TagMatch` struct: `{ name: String, start: usize, end: usize }`
- Normalize: lowercase, trim
- `append_tag(text: &str, tag_name: &str) -> String` — append `#tag` at end, accumulating on last line
- `remove_tag(text: &str, tag_name: &str) -> String` — remove first `#tagname` occurrence, clean whitespace
- `rename_tag(text: &str, old: &str, new: &str) -> String` — replace `#old` with `#new` preserving position

**auto_color.rs:**
- `TAG_PALETTE: [&str; 12]` — the 12 hex colors from the spec
- `auto_color(tag_name: &str) -> String` — deterministic hash-to-palette-index

**Tests:**
- Tag extraction (single, multiple, in sentences)
- Heading disambiguation (`# Heading` ≠ tag)
- URL fragment immunity (`https://x.com#frag` ≠ tag)
- Emoji and hierarchy tags (`#🐛bug`, `#frontend/css`)
- append/remove/rename correctness
- auto_color determinism

**Files:** `swissarmyhammer-kanban/src/tag_parser.rs` (new), `swissarmyhammer-kanban/src/auto_color.rs` (new), `swissarmyhammer-kanban/src/lib.rs`, `swissarmyhammer-kanban/Cargo.toml` (add `regex` dep)

- [ ] Create tag_parser.rs with parse_tags, append_tag, remove_tag, rename_tag
- [ ] Create auto_color.rs with TAG_PALETTE and auto_color()
- [ ] Register modules in lib.rs
- [ ] Add regex dependency to Cargo.toml
- [ ] Write unit tests for all functions
- [ ] cargo test passes"