---
assignees:
- claude-code
position_column: todo
position_ordinal: b980
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