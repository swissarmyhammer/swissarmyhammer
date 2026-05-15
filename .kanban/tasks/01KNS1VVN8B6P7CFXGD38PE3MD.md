---
assignees:
- claude-code
depends_on:
- 01KNS1TQR2C3TYG1G8STEYZPA5
position_column: done
position_ordinal: ffffffffffffffffffffffffff9380
project: code-context-cli
title: Implement magnifying glass banner for code-context CLI
---
## What
Create `code-context-cli/src/banner.rs` with a magnifying glass ASCII art splash and "CODE-CONTEXT" in ANSI Shadow block font, using a blue-to-cyan gradient (search/lens theme).

Mirror `shelltool-cli/src/banner.rs` exactly in structure:
- `COLORS: [&str; 7]` — blue-to-cyan gradient: `\x1b[38;5;45m` (bright cyan) → `\x1b[38;5;39m` → `\x1b[38;5;33m` → `\x1b[38;5;27m` → `\x1b[38;5;21m` → `\x1b[38;5;20m` → `\x1b[38;5;19m` (deep blue)
- `DIM`, `RESET` constants
- `CODE_LINES: [&str; 7]` — magnifying glass ASCII art left side + "CODE" block font right side (7 lines)
- `CONTEXT_LINES: [&str; 5]` — "CONTEXT" block font continuation (5 lines)
- `render_banner(out: &mut dyn Write, use_color: bool)` — render to writer
- `print_banner()` — print to stdout respecting NO_COLOR and TTY
- `should_show_banner(args: &[String]) -> bool` — same logic as shelltool

Magnifying glass ASCII art (7 lines tall):
```
  .---.
 /  O  \
|  ( )  |
 \  O  /
  '---'
    |
    |___
```
Adjust to 7 lines. The word "CODE" follows on the right in ANSI Shadow block font.
"CONTEXT" continues in the second block (5 lines).

Tagline: `"Code intelligence for AI agents — symbols, search, and call graphs"`

## Acceptance Criteria
- [ ] `banner::render_banner(&mut buf, false)` produces valid UTF-8 containing the tagline
- [ ] No ANSI codes in plain mode; ANSI codes present in color mode
- [ ] `banner::should_show_banner` returns `true` for `--help`/`-h`, `false` for subcommands

## Tests
- [ ] `banner_plain_contains_tagline` — render plain, assert tagline present
- [ ] `banner_colored_contains_ansi_codes` — render colored, assert `\x1b[38;5;` present
- [ ] `banner_has_magnifying_glass` — render plain, assert lens art fragment present
- [ ] `should_show_banner_help_flags` — assert true for `--help` and `-h`
- [ ] `should_show_banner_subcommand_returns_false` — assert false for `["code-context", "serve"]`
- [ ] Run `cargo test -p code-context-cli banner` and confirm all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.