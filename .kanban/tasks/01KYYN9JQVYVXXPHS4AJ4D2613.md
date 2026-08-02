---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyyv4he3nd16r0pnmencrg7h
  text: |-
    Second occurrence, 2026-08-01, reviewing `3523b4594` for ^a2ef9wh. Stronger evidence this time: `git blame` on every cited line.

    All 10 engine findings cited lines that blame to commits OTHER than the one under review. Not one was in the delta:

    | Cited | Blames to |
    |---|---|
    | `swissarmyhammer-common/src/frontmatter.rs:118`, `:127` | `ddb3c8da1` |
    | `swissarmyhammer-entity/src/io.rs:104` / `:250` / `:278` | `4b8a48703` / `a3db85e01` / `4227e1331` |
    | `swissarmyhammer-tools/src/health_registry.rs:22`, `:46`, `:68`, `:88` | `569279fb5` |
    | `swissarmyhammer-tools/src/mcp/tools/ralph/state.rs:141` | `355650537` |

    Blame is a better test than the line-mismatch check used on the first occurrence: it proves the cited line is not part of the reviewed commit, rather than only showing the description does not match what sits there.

    The line numbers were also wrong in the same way as before. `frontmatter.rs:118` is the doc line `/// use ...::parse_frontmatter;`, while the `metadata: None` construction the finding describes is at 181 and 223. `health_registry.rs:88` is `for dir in dirs_to_check {`, while the `Arc::new(RwLock::new(..))` it describes is at 194-206.

    One finding was also factually wrong on its own terms, independent of the location problem: it claimed `write_entity` panics on a parentless path. It does not — `swissarmyhammer-entity/src/io.rs:100` is `if let Some(parent) = path.parent() {`, a no-op when there is no parent.

    So the failure has two layers worth separating when fixing this:

    1. **Scope** — `review sha <range>` reports on code the range does not touch. Blame makes this cheap to detect, and cheap to assert in a regression test: every finding's cited line must blame to a commit inside the reviewed range.
    2. **Location** — within a reported file, the line number does not point at the described code.

    A useful acceptance test falls straight out of the blame check: review a known commit, then assert that `git blame` for every reported `file:line` resolves to a commit in the reviewed range. That catches both layers without needing to judge whether a finding is substantively correct.

    Cost so far: two consecutive tasks have each needed a manual cross-check to separate real findings from noise. On ^a2ef9wh that was 10 spurious findings against 1 real one.
  timestamp: 2026-08-01T14:20:04.931960+00:00
- actor: claude-code
  id: 01kyyw91btmxr1sjw2w7y9c942
  text: |-
    Third consecutive occurrence, 2026-08-01, reviewing `60a173bf2` for ^a2ef9wh iteration 2. This one carries a proof that needs no blame check.

    **The reviewed commit changed comment lines only — zero executable lines.** Verified by filtering the diff for any added or removed line that is not `//`, `//!`, or `///`; the result is empty. The engine nonetheless reported 5 findings, including:

    - a missing `PartialEq`/`Eq` derive on `Frontmatter`
    - a CRLF handling gap in `parse_frontmatter_internal`
    - a magic literal `3`
    - a hardcoded `"---\n"`

    A comment-only delta cannot introduce a missing derive, a CRLF gap, or a magic number. These are structurally impossible as findings *on this delta*, independent of where their line numbers point. That makes this occurrence stronger evidence than the previous two: no blame comparison is needed to rule them out, only the observation that no code changed.

    Blame agrees anyway — the 5 cited lines blame to `3523b4594`, `d6dd0ada4`, and `ddb3c8da1`, none of them the reviewed commit. And the line numbers are misplaced in the usual way: `:90` is a closing brace while the derive it names is at 106; `:187` is a closing brace while the `starts_with("---\n")` it names is at 203; `:354`/`:355` sit inside a test fixture string with no `"---\n"` literal at either line.

    Two of the five also target `parse_frontmatter_internal`, which ^tv3692e owns and this commit did not touch.

    ## Running cost

    | Task | Engine findings | Actually in scope |
    |---|---|---|
    | ^fpcbeth | 13 | 0 |
    | ^a2ef9wh iter 1 | 10 | 1 |
    | ^a2ef9wh iter 2 | 5 | 0 |

    28 findings, 1 real. Every one of the three reviews needed a manual cross-check to separate signal from noise, and without that check an implementer would have been dispatched to edit code the commits never touched.

    This suggests a cheap, high-value guard independent of the root cause: **when the reviewed delta contains no executable lines, no finding about code structure can be in scope.** Asserting that alone would have caught this occurrence outright.
  timestamp: 2026-08-01T14:40:00.890638+00:00
position_column: todo
position_ordinal: d880
title: Review engine reports findings against a stale revision — cited line numbers do not resolve
---
`review sha HEAD~1..HEAD` returned 13 confirmed findings whose cited line numbers point at unrelated code in the current tree. Observed 2026-08-01 reviewing commit `42e32c3a3` for ^fpcbeth.

## Evidence

Every citation was off by a large, non-uniform offset. Verified by hand:

| Finding cites | What is actually there | Where the described code really is |
|---|---|---|
| `io.rs:309` `<serialization>` | a bare `///` line | io.rs:443 and 460 |
| `io.rs:140` / `334` `.tmp_{Ulid}` | — | io.rs:115 and 543 |
| `io.rs:1321` magic number | — | io.rs:1709 and 1736 |
| `store.rs:109` `EntityTypeStore::deserialize` | a bare `///` line | store.rs:173 |

Cross-checking the commit's own hunk headers confirms the functions the engine named — `write_entity`, `copy_attachment`, `restore_entity_files`, `read_entity_dir`, `reconcile_read_results` — are untouched by `42e32c3a3`. `copy_attachment` appears zero times in the diff.

So the engine was reasoning over a different revision of the file than the one the range names.

## Why it matters

1. **Findings become unactionable.** An agent told to fix `io.rs:309` finds a doc-comment line. The honest responses are to guess or to dismiss, and both are bad.
2. **It invites wrong edits.** An agent that trusts the line number and "fixes" what it finds there damages unrelated code.
3. **It breaks scoping.** `review sha` exists to review one delta. Reporting on untouched functions defeats the purpose and forces a manual hunk-header cross-check on every run to tell in-scope from out-of-scope.
4. **It interacts badly with the finish loop.** The loop treats any open finding as blocking. Findings that cannot be located cannot be closed honestly.

Related but distinct: ^k5wsxh0 (same validator returns different finding sets across runs on an unchanged file). That one is nondeterminism; this one is a stale or mismatched revision.

## Investigate

- Whether the file content handed to validators comes from the working tree, the index, or the named revision — and whether it matches the line numbers the finding reports.
- Whether the batching that inlines file bytes (`batch_size`) offsets line numbers when a file is split or truncated.
- Whether `review sha` resolves the range to the pre-image or post-image, and whether findings are numbered against the other one.

## Acceptance

- Every finding's `file:line` resolves to code that matches the finding's own description. Demonstrate on a commit that touches a file with many edits above the changed region, since that is where drift shows.
- `review sha <range>` reports only on code the range actually changed. A finding on an untouched function is a bug in scoping, not a pre-existing finding to be split off.
- Add a regression test that reviews a known commit and asserts the reported lines resolve to the expected symbols. #bug #review