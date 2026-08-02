---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyyvj9tea019kx4cy2p84wh8
  text: |-
    This card's blast-radius note is wrong, and the correction makes the task much cheaper.

    Found while fixing a docstring on ^a2ef9wh, which required verifying the real call sites before writing the sentence.

    **`parse_frontmatter` and `parse_frontmatter_with_expansion` have zero callers anywhere in the workspace.** Nothing imports either one. `split_frontmatter_body` is the only item any other crate takes from `swissarmyhammer-common::frontmatter`.

    This card says the callers "reach across `swissarmyhammer-templating` (resolver, prompts, lib), `mirdan::list`, and every prompt and workflow load", and that the blast radius could not be covered by ^a2ef9wh's verification scope. That was inferred from the stale module docstring, which claimed the module was "used by both workflow and prompt parsers". Prompt parsing actually goes through `swissarmyhammer-templating`'s own separate copy.

    So the two edge cases this card lists — a closing delimiter at EOF with no terminator, and a CRLF file — change the behavior of functions nothing calls. Verify the zero-caller claim yourself first (`grep -rn 'parse_frontmatter\b' crates/ apps/` and check the imports, not just the name, since several crates define their own function of the same name). If it holds, then either:

    1. Route `parse_frontmatter_internal` through `split_frontmatter_body` as this card asks — now a near-risk-free change, since the only tests that can break are the module's own; or
    2. Delete both public functions and `parse_frontmatter_internal` outright as dead code, which removes the fourth splitter rather than fixing it.

    Option 2 is worth serious consideration. A duplicate parser with a known defect and no callers is a liability: it will be found and reused. If it is deleted, the accompanying docstring text on `split_frontmatter_body` and the module doc (added under ^a2ef9wh, both of which currently point at this card) must be updated to match.

    Either way the work is confined to one module, not the cross-crate sweep this card describes.

    Related, and separate: the count is five splitters, not four. `swissarmyhammer-templating::frontmatter::parse_frontmatter`, `swissarmyhammer-merge::frontmatter::split_frontmatter` (line-anchored, already correct), and `mirdan::list::parse_frontmatter` — that last one does `strip_prefix("---")` then `find("---")`, the same defect ^a2ef9wh just fixed, unguarded, feeding `mirdan list`'s name/description/version reads of `SKILL.md`. Tracked on ^0zer2xf.
  timestamp: 2026-08-01T14:27:35.886151+00:00
- actor: claude-code
  id: 01kz1bvhmhg98cs4nfshc636j8
  text: |-
    Confirmed the zero-caller claim in the prior comment: `grep -rn 'parse_frontmatter\b'` across `crates/` shows `swissarmyhammer-templating` and `mirdan::list` each define their own separate `parse_frontmatter` function; neither imports `swissarmyhammer_common::frontmatter::parse_frontmatter` or `parse_frontmatter_with_expansion`. Only `split_frontmatter_body` has external callers (entity io.rs/store.rs, ralph state.rs, health_registry.rs).

    Went with the card's stated "Required change" (option 1: route through the shared splitter) rather than deleting the functions, since that is the documented acceptance criteria for this card and the comment left both options open.

    TDD: added two new tests first and watched them fail (RED) against the unmodified `splitn(3, "---\n")` implementation:
    - `closing_delimiter_without_trailing_newline_parses_with_an_empty_body` — content `"---\nkey: value\n---"` (no trailing newline after closing `---`). Old code: `splitn` never finds a second `"---\n"` occurrence, so it fell through to "no frontmatter". New code: `split_frontmatter_body` accepts the unterminated `---` as a valid closing line, so it now parses with metadata `{key: value}` and an empty body.
    - `crlf_frontmatter_parses` — content `"---\r\nkey: value\r\n---\r\nbody\r\n"`. Old code: `content.starts_with("---\n")` never matches a `"---\r\n"` opening line, so it fell through to "no frontmatter". New code parses it correctly.

    Implementation: `parse_frontmatter_internal` now calls `split_frontmatter_body(content)` and drops the `starts_with("---\n")` gate + `splitn(3, "---\n")` entirely.

    Refreshed the comment on `test_parse_frontmatter_opening_delimiter_no_closing` (assertions unchanged, per the card's instruction) since it named the old `splitn` behavior. Updated the module doc, the `split_frontmatter_body` doc, and the `parse_frontmatter` doc — all previously pointed at this card (^tv3692e) as the pending fix; now say five readers agree on the line-anchored rule.

    Verification: `cargo nextest run -p swissarmyhammer-common -p swissarmyhammer-templating -p mirdan` — 1259 passed, 0 failed, 0 skipped. `cargo test -p swissarmyhammer-common --doc` — 23 passed. `cargo clippy -p swissarmyhammer-common --all-targets -- -D warnings` — clean.

    Only code file changed: `crates/swissarmyhammer-common/src/frontmatter.rs`.

    ### implement — changed
    - evidence: crates/swissarmyhammer-common/src/frontmatter.rs — `parse_frontmatter_internal` now delegates to `split_frontmatter_body`; added tests `closing_delimiter_without_trailing_newline_parses_with_an_empty_body` and `crlf_frontmatter_parses` (RED before the fix, GREEN after); refreshed docs pointing at ^tv3692e. `cargo nextest run -p swissarmyhammer-common -p swissarmyhammer-templating -p mirdan`: 1259 passed, 0 failed. `cargo test -p swissarmyhammer-common --doc`: 23 passed. `cargo clippy -p swissarmyhammer-common --all-targets -- -D warnings`: clean.
    - next: ready for /review
  timestamp: 2026-08-02T13:50:44.881298+00:00
- actor: claude-code
  id: 01kz1by3dr490ba45g4zy2xmey
  text: |-
    ### test — red
    - evidence: `cargo nextest run -p swissarmyhammer-common -p swissarmyhammer-templating -p mirdan` → 1259 tests run: 1259 passed (2 slow), 0 skipped (matches prior report). `cargo test -p swissarmyhammer-common --doc` → 23 passed; 0 failed; 4 ignored (pre-existing ignores, unrelated to this change; count matches prior report). `cargo clippy -p swissarmyhammer-common --all-targets -- -D warnings` → clean, no warnings. `cargo fmt --all -- --check` → FAILS with 2 diffs, both in the two new tests added by this change:
      - `crates/swissarmyhammer-common/src/frontmatter.rs:468-471` in `closing_delimiter_without_trailing_newline_parses_with_an_empty_body`: `assert_eq!(metadata.get("key").and_then(|v| v.as_str()), Some("value"));` is wrapped across 4 lines; rustfmt wants it on one line.
      - `crates/swissarmyhammer-common/src/frontmatter.rs:488-491` in `crlf_frontmatter_parses`: same `assert_eq!` wrapped across 4 lines; rustfmt wants it on one line.
    - Spot check: both new tests genuinely call `parse_frontmatter`, which calls `parse_frontmatter_internal` (the function under test). Both assert real regressions against the old `splitn(3, "---\n")` substring split: an unterminated closing `---` with no trailing newline, and CRLF-terminated delimiter lines. Both assertions check actual metadata values and body content, not trivially-true conditions. These are meaningful regression tests.
    - next: run `cargo fmt --all` to collapse the two `assert_eq!` calls, then re-run `cargo fmt --all -- --check` to confirm clean, then re-run this test step.
  timestamp: 2026-08-02T13:52:08.632754+00:00
- actor: claude-code
  id: 01kz1bzdm22ejfz4ah154at8ns
  text: |-
    ### finish iteration 1 — green
    - implement: changed — crates/swissarmyhammer-common/src/frontmatter.rs, parse_frontmatter_internal now delegates to split_frontmatter_body
    - test: red (fmt only) — cargo fmt --all applied directly (2 diffs, both assert_eq! collapse in new tests), then re-verified green: cargo nextest run -p swissarmyhammer-common -p swissarmyhammer-templating -p mirdan → 1259 passed, 0 failed, 0 skipped; cargo fmt --all -- --check clean
    - commit: pending
    - review: pending
  timestamp: 2026-08-02T13:52:51.842249+00:00
position_column: doing
position_ordinal: '8380'
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