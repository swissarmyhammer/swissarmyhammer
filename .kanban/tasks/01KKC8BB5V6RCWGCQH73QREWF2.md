---
depends_on:
- 01KKC8A9RVGEJP5XBVJH6H3G5V
position_column: done
position_ordinal: ffffffffffb580
title: 'STATUSLINE-3: Style system + format parser'
---
## What
Implement the ANSI styling system and the format string parser. These are independent utilities used by the module framework.

Key files:
- `swissarmyhammer-statusline/src/style.rs` (new) — parse style specs like \"green bold\", \"cyan\", \"dim\", \"bg:red\" into ANSI escape sequences
- `swissarmyhammer-statusline/src/format.rs` (new) — parse format strings like \"$directory $git_branch $model\" into ordered `Vec<FormatSegment>` (Literal or Module)

Style system supports:
- Colors: black, red, green, yellow, blue, purple/magenta, cyan, white
- Modifiers: bold, dim, italic, underline
- Background: bg:color syntax
- Produces `Style { ansi_open, ansi_close }` with `apply(text) -> styled_text`

Format parser:
- Scans for `$identifier` tokens (alphanumeric + underscore)
- Everything else is literal text (spaces, brackets, separators)
- Returns `Vec<FormatSegment>` where each is `Literal(String)` or `Module(String)`

## Acceptance Criteria
- [ ] `Style::parse(\"green bold\")` produces correct ANSI open/close codes
- [ ] `Style::apply(\"text\")` wraps text in ANSI codes
- [ ] All 8 base colors + 4 modifiers + bg: prefix supported
- [ ] `parse_format(\"$dir $model\")` returns `[Module(\"dir\"), Literal(\" \"), Module(\"model\")]`
- [ ] Literal-only and module-only format strings work
- [ ] Adjacent modules with no separator handled correctly

## Tests
- [ ] style.rs: test each color, modifier, and combination
- [ ] style.rs: test bg:color
- [ ] style.rs: test empty/unknown style spec
- [ ] format.rs: test basic format string with modules and literals
- [ ] format.rs: test format string with only literals
- [ ] format.rs: test format string with only modules
- [ ] format.rs: test dollar sign escaping edge cases
- [ ] `cargo test -p swissarmyhammer-statusline`