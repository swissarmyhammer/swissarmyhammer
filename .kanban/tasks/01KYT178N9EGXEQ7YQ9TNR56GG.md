---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kywrpksm7tx6akpgpaxa83x4
  text: |-
    Picked up. Took option 1 (scope the trim), not the "deliberate normalization" option.

    RED first, exactly as the card predicted. Made the bystander body `"No marker here   \nsecond line"`, `edit_rewrites_only_the_tasks_carrying_the_marker` failed with `left: "...No marker here\nsecond line"` / `right: "...No marker here   \nsecond line"`.

    Research: the whole-body normalization was two defects, not one. `remove_tag` trimmed every line, AND `edit_tag_markers` reassembled with `text.lines()` + `join("\n")`, which also drops a final newline and turns `\r\n` into `\n`. The second half hits bystanders too: `swissarmyhammer-entity/src/io.rs` round-trips a task body verbatim (`parse_frontmatter_body` takes everything after the opening delimiter line, `format_frontmatter_body` writes it back), so a body CAN carry a trailing newline or CRLF. So "byte-identical" was not honestly claimable without fixing the reassembly as well.

    Fix: `markdown_lines` now yields a `MarkdownLine { content, terminator, tag_bearing }` and keeps each line's terminator verbatim; `edit_line_markers` returns whether it rewrote anything; `edit_tag_markers` trims only the lines removal actually edited. Untouched text is now a true identity. The reader (`parse_tags`) is unchanged — `split_line_terminator` strips the terminator exactly as `str::lines` did.

    Confirmed the card's note about the write-skip guard: `StoreHandle::write` (swissarmyhammer-store/src/handle.rs) returns `Ok(None)` when the serialized text is identical, before the changelog append, the atomic write, and the pending event. The fix does not rest on the `new_body != body` guard; the guard's docstring now says what actually short-circuits.

    Blast radius: `tag_parser` is `pub mod` but has NO consumers outside swissarmyhammer-kanban (grepped the workspace). In-crate callers are `tag/delete.rs`, `tag/update.rs`, `tag/cut.rs`, `task/tags.rs`, `derive_handlers.rs` — all fold the writers over slugs the body carries, so scoping the trim only helps them.
  timestamp: 2026-07-31T18:59:02.580990+00:00
- actor: claude-code
  id: 01kywsnes12c1wxxa846nge9gt
  text: |-
    Adversarial double-check came back REVISE with 6 findings. All 6 fixed, all verified.

    1. `test_remove_tag_undoes_append_tag` claimed a universal round trip that does not hold: a body already ending in a newline loses it, because `append_tag("prose\n")` and the own-line append onto `"prose"` produce the identical string, so removal cannot tell them apart. The tie is forced, not chosen. Narrowed the docstring and added the 5 newline-terminated cases with their real expectations, so both halves are pinned.
    2. `drop_last_line_terminator`'s docstring claimed a body ending in a blank line keeps the blank line. False: `remove_tag("a\n\n#bug")` is `"a\n"`. Restated as "exactly one terminator, never a run" and pinned it plus the 3-newline case.
    3. The identity fixture only reached the "no `#` at all" path — never a real marker for a DIFFERENT tag, which is the everyday bystander and the load-bearing false path of the new `edited` flag. Added `"fix #other   \nsecond   \n"`, `"#other   \n"`, `"#other and #another   "`.
    4. `out.pop()` (space absorb) reached backwards into a shared buffer on an unwritten invariant, and a break would PANIC at `result[content_start..]` rather than misbehave. Made it structurally line-local: `edit_line_markers` captures `line_start = out.len()` and guards `out.len() > line_start`. No behavior change; the panic class is gone.
    5. `tag/shared.rs` claimed "nothing observable distinguishes the guarded walk from an unguarded one". Too strong — `EntityCache::write` invalidates memoized computes unconditionally, even hash-unchanged, so an unguarded walk would evict the compute cache board-wide. Narrowed to what holds: no changelog entry, no file write, no event, no mtime change.
    6. Two paraphrases understated the trim rule ("whitespace the hole left behind"). The rule trims the edited line's whole end. Reworded both.

    Proved every fixture live, not vacuous: temporarily reintroduced the pre-fix whole-body normalization and confirmed all four guards fail, including the new `#other` bystander (`left: "fix #othersecond"` vs `right: "fix #other   \nsecond   \n"`). Scaffolding removed; grep confirms none left.

    Discovery worth knowing: my earlier comment on this card quoted a bare triple-dash inside backticks, which corrupted the card's own read via the substring split in `parse_frontmatter_body`. Card 01KYWS1BRH1N3MFFHJYFPCBETH now tracks that separately. Avoid literal triple-dashes in kanban comments until it lands.

    Green: 1605/1605 in swissarmyhammer-kanban, 95/95 on the tools kanban+tag filter, clippy `-D warnings` clean.
  timestamp: 2026-07-31T19:15:53.249185+00:00
- actor: claude-code
  id: 01kywxfyrff65kk8s4tce2z0bt
  text: |-
    Worked the 2026-07-31 15:08 review finding: control-flow nesting depth 4 in `edit_tag_markers`.

    Pure structural extraction, no behavior change. Two helpers, because the cause had two instances in the file, not one:

    1. `tidy_removed_line(result, content_start, terminator) -> bool` — the trim plus the emptied-final-line drop, lifted out of `edit_tag_markers`. The loop now returns early for a non-tag-bearing line, so `edit_tag_markers` is `for > if` (depth 2). The old `for > if > if > if` chain is gone. `&&` short-circuit keeps the helper off any line the removal did not edit, which is the byte-identity contract.
    2. `rewrite_marker(bytes, after_marker, replacement, out, line_start) -> usize` — the replace-or-absorb-a-space policy, lifted out of the `Some(found)` arm of `edit_line_markers`. That arm was the sibling instance: `while > match > arm > if/else if/else if`. Counting a match arm as a level made it 4. The helper is three sequential guard `if`s at depth 1, and the arm is now one assignment.

    Audited every other function in the file for depth 4. None left. Deepest remaining is 3 (`edit_line_markers`: `while > match > arm`, and the arm bodies hold no control flow). `split_line_terminator`'s nested match has expression arms and no nested blocks, so it stays at 2. The test module is `for` loops with asserts, depth 1.

    Re-ran the `complexity` validator on the file: zero nesting findings. It did raise ONE unrelated pre-existing item — `append_tag` builds its inline candidate and its own-line candidate with near-duplicate blocks. Untouched code, different cause, so it went to its own card instead of into this change.

    No test needed editing, which is the signal that behavior held. Every diff hunk is in the production section; none reach `mod tests`. Green: 1605/1605 in swissarmyhammer-kanban, `cargo clippy -p swissarmyhammer-kanban --all-targets -- -D warnings` clean, `cargo fmt --check` clean.
  timestamp: 2026-07-31T20:22:47.311321+00:00
- actor: claude-code
  id: 01kywy67v73kpzk1yfw7kekxvr
  text: |-
    Proved the "no behavior change" claim instead of asserting it.

    Built a differential harness in a scratch directory outside the repo: `git show HEAD:...tag_parser.rs` as module `old`, the working-tree file as module `new`, then compared them over a generated corpus. Tokens `#bug`, `#other`, `#bug-x`, one space, two spaces, `\n`, `\r\n`, `a`, a backticked marker, a fence, a heading, comma, period, tab, and `x #bug`, in every combination up to length 4 — 69,905 bodies. Each body ran through `remove_tag`, `rename_tag`, `append_tag`, `parse_tags`, and `normalize_slug` for four slugs, including one no body carries.

    Result: 0 mismatches. Byte for byte identical to HEAD on every case, so the extraction moved code and nothing else. The corpus is ASCII; the multibyte path is the `None` arm, which the change does not touch, and `test_remove_tag_multibyte_chars` and `test_rename_tag_multibyte_chars` still cover it.

    Also green: 95/95 on the `swissarmyhammer-tools` kanban+tag filter, same as the previous iteration.
  timestamp: 2026-07-31T20:34:57.511487+00:00
- actor: claude-code
  id: 01kywyx2xmnckerm58218y9d00
  text: |-
    Round 2, prompted by the adversarial check. It returned PASS on everything — equivalence, the invariants, the docstrings, byte-exact `mod tests` (md5 of the old test range equals md5 of the new one) — but it reported that the `complexity` validator is NONDETERMINISTIC on this file. One of its runs flagged `collect_line_tags` and `edit_line_markers` with "match arms contain code at depth 4".

    Those two are false positives against the documented rule. `builtin/validators/complexity/rules/cognitive-complexity.md` counts nested **conditions**, and both functions sat at 2. But a finding that a future review round can re-raise is a sibling left behind, so I removed the ambiguity instead of arguing with it: both functions were two-arm `Option` matches whose `Some` arm held statements, so both became `if let ... else`. A `match` with statement-bearing arms is now gone from the file. Nothing left for an arm-counting scanner to see, and under the documented condition-counting rule the file is unchanged at 2.

    Re-verified after round 2:
    - Differential harness re-run against HEAD: 69,905 bodies, 0 mismatches. Still byte-for-byte identical.
    - `cargo nextest run -p swissarmyhammer-kanban`: 1605/1605.
    - clippy `--all-targets -D warnings` clean, `cargo fmt --check` clean.
    - Every diff hunk still lands above `mod tests`; no test touched in either round.
    - Validator re-run twice more: zero nesting findings both times.

    Also seen, NOT carded, because the source is unstable: the validator's later runs raised two stylistic items on untouched lines — the module doc has no `# Examples` section, and the `b'` backtick literal appears in three functions and could be a named constant. Different run, different set. Leaving these for the real `/review` gate rather than guessing which ones are stable enough to be cards.
  timestamp: 2026-07-31T20:47:26.132075+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8680
title: remove_tag rewrites every task body, not just the ones carrying the marker
---
`tag_parser::remove_tag` ends with a trailing-whitespace normalization applied to **every** task body the walker visits, whether or not that body carries the marker:

```rust
edit_tag_markers(text, slug, None)
    .lines()
    .map(str::trim_end)
    .collect::<Vec<_>>()
    .join("\n")
```

So `delete tag` silently rewrites bystander cards. Any task whose description happens to carry trailing whitespace gets edited by an operation that names a different tag.

Pre-existing — the old per-file walkers called the same `remove_tag` on every body. Found during verification of ^1t92gnj, not introduced by it.

## Reproduction

`crates/swissarmyhammer-kanban/src/tag/shared.rs` has a test, `edit_rewrites_only_the_tasks_carrying_the_marker`, whose bystander body is `"No marker here"` — too clean to expose this. Change only that body to `"No marker here   \nsecond line"` and the test goes RED:

```
left:  "...No marker here\nsecond line"
right: "...No marker here   \nsecond line"
```

## Also: the docstring over-claims

That same test's docstring says it catches "an `edit_fn` **or a boundary rule** that over-matches". It does not — the boundary-rule half is exactly the case above, and the test's own fixture is too clean to reach it. A docstring that claims coverage the test does not have is worse than no docstring: it tells the next agent this ground is already held.

## Required change

Pick one, do not do both halves partially:

1. **Preferred** — scope the trim to lines the edit actually touched, so a body with no marker comes back byte-identical. Then give the test's bystander trailing whitespace so the fixture proves it.
2. If the board-wide normalization is deliberate and wanted, say so in the doc and narrow the test's docstring to what it really holds.

Whichever is chosen, `apply_tag_edit_to_all_tasks` must end with a stated, tested contract about what it does to untouched bodies.

## Acceptance

- A bystander task carrying trailing whitespace survives `delete tag` byte-identical (or the deliberate-normalization decision is documented and the docstring narrowed).
- The test fixture is dirty enough to fail if the rule regresses — prove it RED first.
- No docstring in `tag/shared.rs` claims coverage the test does not have.

Note: the write-skip guard `if new_body != body` in `apply_tag_edit_to_all_tasks` is unobservable — `StoreHandle::write` already short-circuits on identical text before the changelog append, the atomic write, and the pending ChangeEvent. Do not build the fix on that guard. #bug

## Review Findings (2026-07-31 15:08)

- [x] `crates/swissarmyhammer-kanban/src/tag_parser.rs:303` — edit_tag_markers function has control flow nesting depth of 4 (for > if > if > if), exceeding the threshold of 3 levels deep. The nested conditions make the logic hard to reason about and increases maintenance burden. Extract the innermost conditional and the logic it guards into a separate helper function (e.g., `fn should_drop_final_line_terminator(...) -> bool` or `fn handle_emptied_final_line(...)`), reducing edit_tag_markers nesting depth to 3 levels maximum.
