---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyyhhygmg49fsqzhps5zryg3
  text: |-
    Picked up. Research done before touching code.

    Blast radius: the substring split exists in TWO live read paths in `swissarmyhammer-entity`, not one.

    1. `io.rs` `parse_frontmatter_body` -- reached by `read_entity`, `read_entity_dir`, `parse_entity_text`. `EntityContext` uses these for every get and every list, so every entity type with a `body_field` goes through it. Tags, columns, boards, projects, perspectives and actors are plain YAML (`body_field: None`) and never reach this function; tasks are the exposed type.
    2. `store.rs` `EntityTypeStore::deserialize` -- the same `splitn` on a bare three-hyphen substring, plus the same `strip_prefix` of a lone LF. Reached by `StoreHandle::get` and by every changelog replay arm in `swissarmyhammer-store/src/handle.rs` (undo, redo, restore). `EntityTypeStore` is registered in production by `swissarmyhammer-kanban/src/context.rs`, so this is live, not dormant. Fixing only io.rs would leave undo/redo still corrupting cards.

    Recovered the real evidence. The poisoned text is the 18:59:02 comment on tnr56gg, preserved in `.kanban/tasks/01KYT178N9EGXEQ7YQ9TNR56GG.jsonl` entry 3. Its "Research:" paragraph quotes a backticked three-hyphen run. Confirmed the loss is already persisted: the current `.md` on disk no longer carries that text, because the paragraph was reworded to "takes everything after the opening delimiter line". So the read-modify-write really did eat it, exactly as the card predicted.

    Second defect found while reading, not in the card: CRLF. The body is taken with a `strip_prefix` of a lone LF, so a file whose delimiter line ends CRLF yields a body with a leading CR LF pair. That is a byte-level body corruption independent of the split bug, and it would regress the byte-identity contract the tag_parser change (f1ad4bc61, 4a7bbf7e0) depends on. Covering it in the same fix.

    NOTE TO THE NEXT AGENT: keep literal three-hyphen runs out of this card until the fix ships in the running `sah` binary. The MCP server you talk to is an already-built process; it still has the old parser.
  timestamp: 2026-08-01T11:32:38.548964+00:00
- actor: claude-code
  id: 01kyyj5f6y156g4n9s70a73a8y
  text: |-
    Implementation landed. RED first on every acceptance item that could be RED.

    RED (before the fix), `cargo nextest run -p swissarmyhammer-entity --lib`:

    - `title_containing_a_delimiter_round_trips` -- the title's tail leaked into the body. Left was the title text from the embedded quote onward, followed by a delimiter line and the card description; right was the description alone.
    - `comment_with_a_delimiter_on_its_own_line_round_trips` -- `left: None`, `right: Some(String("doing"))` for `position_column`. The card fell off its column.
    - `crlf_delimiter_lines_leave_the_body_byte_exact` -- left carried a leading CR LF pair that right did not.
    - Also RED: `comment_quoting_a_delimiter_inline_round_trips`, `comment_with_an_indented_delimiter_round_trips`, and the two store.rs equivalents. 7 RED in total.

    The two "keeps passing" items were guards, not RED: `parse_frontmatter_body_round_trip` and the no-closing-delimiter case both already held.

    Fix: new `crates/swissarmyhammer-entity/src/frontmatter.rs` with `split_frontmatter_body(content) -> Option<(&str, &str)>`. The opening delimiter is the first line and must be exactly three hyphens; the frontmatter ends at the next line that is exactly three hyphens; a trailing CR still counts as a delimiter line. The body is a borrowed slice starting after the closing line's terminator, so it keeps its bytes exactly. That is what fixes CRLF as a side effect and what preserves the tag_parser byte-identity contract.

    Both readers now call it, so the delimiter rule cannot drift:

    1. `io.rs` `parse_frontmatter_body`
    2. `store.rs` `EntityTypeStore::deserialize`

    Write side inspected, no mirror-image defect. `format_frontmatter_body` and `EntityTypeStore::serialize` are safe because a YAML emitter never starts a line at column 0 inside a scalar: it indents block scalars and quotes what it cannot indent. Pinned that with `format_writes_exactly_two_delimiter_lines`, which serializes a card whose comment carries a bare three-hyphen line and asserts the output holds exactly two delimiter lines.

    Extra win found while checking: a comment that quotes a unified diff header also used to truncate the frontmatter under the substring split. It is now safe, because a diff header is not a bare delimiter line.

    Behavior change worth knowing. The old split discarded anything before the first three-hyphen substring, so a file that did NOT open with a delimiter still parsed. The new reader rejects that with `InvalidFrontmatter`. Verified this hurts nothing: all 2846 `.md` files under `.kanban/` open with exactly a delimiter line, and the 260 plain-YAML entities (tags, columns, actors, boards, projects, perspectives) never reach this function at all. They have `body_field: None` and go through `parse_plain_yaml`.

    GREEN: 3498 passing, 0 failing across `-p swissarmyhammer-entity -p swissarmyhammer-kanban -p swissarmyhammer-store -p swissarmyhammer-tools`. Baseline at 4a7bbf7e0 was 3480; the 18 new tests account for the whole delta. `cargo fmt` clean, `cargo clippy -p swissarmyhammer-entity --all-targets -- -D warnings` clean.
  timestamp: 2026-08-01T11:43:18.238019+00:00
- actor: claude-code
  id: 01kyyjrepair0000000000000
  text: |-
    Repaired this card by hand after it corrupted itself AGAIN, live, at 11:43.

    What happened: the fix is in the working tree, but the `sah` MCP server I talk to is an already-built process carrying the OLD parser. My previous comment quoted the literal characters, so the server's read truncated the frontmatter inside it, and the write that followed persisted the loss. On disk the card came back with `title: Untitled`, no `position_column`, and the real title, ordinal and description all sitting in the markdown body.

    Repair: rebuilt the file directly with the true field values and the original description byte-for-byte, and reworded both comments so no literal three-hyphen run remains. Nothing was lost.

    This is the reported bug reproducing itself on its own card for the third time. Treat it as live confirmation, not as a new defect.
  timestamp: 2026-08-01T11:52:00.000000+00:00
- actor: claude-code
  id: 01kyyk02rz73np2serxhgptdwk
  text: |-
    Adversarial double-check came back REVISE with 2 findings. Both real, both fixed, both proved.

    FINDING 1 (high) -- the frontmatter slice was still being trimmed, which ate a trailing newline.

    `parse_frontmatter_body` and `EntityTypeStore::deserialize` both did `frontmatter.trim()` before handing the slice to serde. The new splitter returns exactly the frontmatter bytes, which always end in the last field line's LF. Trimming that LF re-chomps a block scalar that ends the mapping, so a multi-line field value ending in a newline came back one newline short. Same silent-field-loss class the card is about, just on frontmatter scalars instead of the body.

    Reachable for real. `Entity::fields` is a std `HashMap`, so `format_frontmatter_body` emits keys in unordered order and ANY field can land last, making the loss intermittent. `store.rs` sorts with a BTreeMap so it is deterministic there (alphabetically last field).

    My own round-trip helpers advertised "every field survives" but no fixture value ended in a newline, so the assertion never reached the failing class. That is the defect the just-closed card existed to remove, and I reproduced it. Fixed.

    RED, `store::tests::test_a_frontmatter_value_keeps_its_trailing_newline`:
    `left: Some(Array [Object {"actor": String("claude-code"), "text": String("first line\nsecond line")}])`
    `right: Some(Array [Object {"actor": String("claude-code"), "text": String("first line\nsecond line\n")}])`

    Fix: dropped both `trim()` calls and passed the slice straight through, with a comment saying why. Added the io.rs twin of the test, plus a guard on each side that empty frontmatter still yields an entity with only the body (that path used to lean on the trim).

    FINDING 2 (low) -- a docstring stated a mechanism that is not what the emitter does.

    I had written that the YAML emitter "never starts a line at column 0 inside a scalar". False: a blank line inside a literal block scalar is emitted with zero indent, at column 0, because libyaml suppresses indent on empty lines. The CONSEQUENCE I depend on still holds, but the stated reason was wrong, in three places.

    Verified the consequence instead of asserting it. Built a throwaway probe outside the repo against serde_yaml_ng 0.10.0 and checked 38416 generated value shapes plus 21 hand-built ones (value at top level, nested in a sequence of maps, as a map key, 3 levels deep; trailing spaces, leading spaces, CR, tabs, NEL, LS, BOM, 3000-char lines, runs of delimiters). Zero emitted a bare three-hyphen line at column 0. Mechanism: serde_yaml_ng sets the emitter width to -1, which libyaml treats as unbounded, so scalar line-wrapping never fires.

    Reworded all three docstrings to state only the operative fact. Broadened `format_writes_exactly_two_delimiter_lines` from one fixture to a 10-shape table chosen to push the emitter between block-scalar and quoted style (trailing space, CR, tab, blank lines around the run, a bare run, an indented run, a 3000-char line), and made each case also parse back and compare the value, so the guard catches value loss and not only a stray delimiter line.

    Verifier's other checks came back clean and I am recording them so nobody re-derives them: no panic or wrong slice for multibyte, lone CR, CRLF pairs, 200 consecutive delimiter lines; no caller relied on the old leniency; no remaining substring split in entity, store or kanban. Out of scope but real, worth carding separately: `swissarmyhammer-tools/src/health_registry.rs:101` and `swissarmyhammer-tools/src/mcp/tools/ralph/state.rs:183` still carry the same bare substring split.

    Also worth knowing: `read_entity_dir` WARNS and SKIPS on invalid frontmatter rather than failing, so a file the stricter reader rejects would silently vanish from the board. Nothing on disk triggers it today (all 2846 task files open with a clean delimiter line), but that is the failure shape if a hand-edited file ever does.

    Final: 3502 passing, 0 failing across the four-package scope. Baseline at 4a7bbf7e0 was 3480; 22 new tests account for the delta. One flake seen once under parallel load, `review_working_emits_progress_notifications_per_pair_when_token_supplied`, a slow review test; it passes in isolation and the very next full run was 3502 for 3502. `cargo fmt --check` clean, `cargo clippy -p swissarmyhammer-entity --all-targets -- -D warnings` clean.
  timestamp: 2026-08-01T11:57:50.239427+00:00
- actor: claude-code
  id: 01kyyk4nzqx6wtnc1vmmxghn6v
  text: |-
    Second repair, and a lesson for anyone who ever hand-edits a card file.

    After I rebuilt this card's `.md` by hand, `get task` came back with `created` set to the file's birth time and `started` and `updated` null. Cause: the JSONL changelog is a patch CHAIN, replayed from the empty string. My out-of-band write meant the next entry's forward patch no longer applied to the result of the entry before it, `replay_store_log` bailed, and `_changelog` came back empty. `derive-created` then fell through to its filesystem-birthtime fallback, and `derive-updated` and `derive-started` had nothing to read.

    Repaired by replaying entries 0 through 5 to recover the text the chain expected, then regenerating the last entry's forward and reverse patches from that text to the current file with `diffy`, the same crate `swissarmyhammer-store/src/diff.rs` uses. Verified after: the chain replays cleanly through all 7 entries and the replayed text equals the on-disk bytes. `get task` now returns `created` 2026-07-31T19:04:54.801721Z, `started` 2026-08-01T11:27:22.871038Z, and the real `updated`.

    Rule to carry forward: editing an entity `.md` directly is not enough. Either repair the trailing changelog entry to match, or expect every changelog-derived field on that card to go quiet.
  timestamp: 2026-08-01T12:00:20.983356+00:00
position_column: doing
position_ordinal: '8480'
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
- A malformed file with no closing delimiter still returns `EntityError::InvalidFrontmatter` rather than silently treating the whole file as frontmatter. #bug #kanban