---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzw71cm0r7m7dqahv9gwn0mf
  text: |-
    Research done. Re-measured with swiftlint 0.65.0 at two fixture roots whose absolute paths differ by exactly 3 characters (length 132 and 135).

    Result — the entry count holds at every row, and only the byte count moves:

    | row | entries | bytes @132 | bytes @135 | table states |
    |---|---|---|---|---|
    | magic-numbers: `return status == 404` | 1 | 381 | 384 | 385 |
    | magic-numbers: + `warning_threshold: 1` | 2 | 604 | 607 | 608 |
    | missing-docs: 2 undocumented public items | 2 | 718 | 724 | 726 |
    | missing-docs: + `warning_threshold: 1` | 3 | 941 | 947 | 949 |
    | missing-docs: `public func oops( {` | 1 | 360 | 363 | 364 |
    | complexity: the probe file | 1 | 409 | 412 | 413 |
    | an empty array | 0 | 5 | 5 | 5 |

    Each byte delta is exactly 3 for each entry that carries a path. The 3-entry threshold row moved 6, not 9, because the `warning_threshold` entry carries an empty `file` field and so no path. That confirms the cause precisely.

    `an empty array, 5 bytes` and `0 bytes` are path-INDEPENDENT — measured 5 bytes (`[\n\n]\n`) at both roots. They reproduce, and they carry the distinction the script's status gate depends on. They stay.

    Survey of every byte count standing in the Swift rule bodies today (the tree moved under this card, so the card's quoted numbers are stale):
    - magic-numbers-swift.md: `1 entry, 385 bytes`; `2 entries, 608 bytes`; `1 entry, 392 bytes` (Latin-1 table)
    - missing-docs-swift.md: `2 entries, 726 bytes`; `3 entries, 949 bytes`; `1 entry, 364 bytes`; `2 entries, 740 bytes` (Latin-1 table)
    - complexity-swift.md: `1 entry in 413 bytes` (prose); `1 entry, 413 bytes` (table)
    - dead-code-swift.md: NO byte count and no entry count at all. Clean, nothing to do.

    Acceptance tests with the same defect:
    - tests/shipped/magic_numbers.rs: `writes 1 entry in 392 bytes`
    - tests/shipped/missing_docs.rs: `writes 2 entries in 740 bytes`

    Answer picked: DROP the byte count and keep the entry count.
    - The entry count is what the script actually gates on (`jq -e 'type == "array" and length > 0'`). The byte count gates nothing.
    - Stating the path cannot work: each measurement runs inside a `mktemp -d` directory that differs on every run and every machine, so a recorded path makes the number checkable only in a directory that no longer exists.
    - Marking the count path-dependent keeps a number a later reader still cannot check — it documents the defect instead of removing it.
    - complexity-swift.md already carries a row reading `1 entry` with no byte count (the Latin-1 row), so dropping makes the files self-consistent.
  timestamp: 2026-08-13T00:06:03.136422+00:00
- actor: claude-code
  id: 01kzw7a6sfh0pqdqfq96f9ehhj
  text: |-
    Implementation landed. Answer taken: DROP the byte count and keep the entry count.

    Every path-dependent byte count is gone from the Swift rule bodies and the shipped Swift tests. Survey after the change: the only byte counts that stand are 45 counts of `0 bytes` and 10 counts of `5 bytes`, and each was measured identical at both fixture roots, so each reproduces.

    Rows changed (all of them, not only the rows the card quoted):
    - magic-numbers-swift.md: `1 entry, 385 bytes` -> `1 entry`; `2 entries, 608 bytes` -> `2 entries`; `1 entry, 392 bytes` -> `1 entry` (Latin-1 table)
    - missing-docs-swift.md: `2 entries, 726 bytes` -> `2 entries`; `3 entries, 949 bytes` -> `3 entries`; `1 entry, 364 bytes` -> `1 entry`; `2 entries, 740 bytes` -> `2 entries` (Latin-1 table)
    - complexity-swift.md: `stdout carries 1 entry in 413 bytes` -> `stdout carries 1 entry` (prose); `1 entry, 413 bytes` -> `1 entry` (table)
    - magic_numbers.rs: `writes 1 entry in 392 bytes` -> `writes 1 entry`
    - missing_docs.rs: `writes 2 entries in 740 bytes` -> `writes 2 entries`

    To remove the CAUSE and not only the rows, each of the three status tables now carries a paragraph that states why the row holds an entry count and no byte count. Four earlier commits re-measured these tables today; without that paragraph the next re-measurement puts the byte counts back. Each Latin-1 decode table carries a one-sentence cross-reference to it.

    `0 bytes` and `an empty array, 5 bytes` stay. Both were measured identical at both roots, and both carry the distinction the script's status gate depends on — a JSON array of one entry or more against no output at all.

    dead-code-swift.md needed no change: it carries no byte count and no entry count.

    Out of scope and left alone: complexity-rust.md (1133/7489 bytes) and missing_docs_rust.rs (1124 bytes). Those are cargo reports under a different rule, and three other cards moved complexity-rust today.

    Verification:
    - cargo nextest run -p swissarmyhammer-validators: 698 tests run, 698 passed, 0 skipped
    - cargo fmt --check: exit 0
    - cargo clippy --workspace --all-targets -- -D warnings: exit 0
    - Nothing staged.
  timestamp: 2026-08-13T00:10:52.079191+00:00
- actor: claude-code
  id: 01kzw7acrmwz69jwqrsnmdkrxc
  text: |-
    ### implement — changed
    - evidence: 5 files — builtin/validators/code-hygiene/rules/magic-numbers-swift.md, builtin/validators/code-hygiene/rules/missing-docs-swift.md, builtin/validators/code-hygiene/rules/complexity-swift.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/magic_numbers.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs. 698/698 tests pass, fmt clean, clippy clean.
    - next: /review
  timestamp: 2026-08-13T00:10:58.196543+00:00
- actor: claude-code
  id: 01kzw7hp5t6438qe0v0hj8szq7
  text: |-
    ### review — clean
    - evidence: `review sha 7896580b9~1..7896580b9` — 0 findings, 0 confirmed, 0 refuted; 8 validators attempted, 0 failed, 0 skipped. No prior Review Findings sections, no unchecked items.
    - next: task moved to done.
  timestamp: 2026-08-13T00:14:57.210998+00:00
- actor: claude-code
  id: 01kzw7jefnjkq6gz0qdb44315h
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 5 files. Answer (2) chosen: drop the byte count, keep the entry count. The defect was reproduced first rather than trusted, because four commits moved these tables today — two fixture roots whose absolute paths differ by exactly 3 characters. Every entry count holds; no byte count reproduces. Each delta is exactly 3 per entry that carries a path, and the 3-entry row moved 6 rather than 9 because the `warning_threshold` entry carries an empty `file` field, which confirms the diagnosed cause precisely.
    - Why not (3): each measurement runs inside a `mktemp -d` directory that differs on every run and every machine, so recording the path makes the number checkable only in a directory that no longer exists. Why not (1): it keeps a number a later reader still cannot check, documenting the defect instead of removing it. The entry count is also what the script gates on (`jq -e 'type == "array" and length > 0'`); the byte count gates nothing.
    - `0 bytes` and `an empty array, 5 bytes` STAY — measured byte-identical at both roots, because they carry no path, and they carry the distinction the status gate depends on.
    - Applied to every affected row, not only the rows the card quoted: 3 rows in magic-numbers-swift, 4 in missing-docs-swift, 1 row plus 1 prose sentence in complexity-swift, and the two acceptance-test doc comments. Each status table now states WHY it holds an entry count and no byte count, so the next re-measurement does not put them back. dead-code-swift.md needed no change; it carries no such count.
    - Post-change survey: the only byte counts left anywhere in the Swift rules are 45 × `0 bytes` and 10 × `5 bytes`, all path-independent.
    - test: green — cargo nextest run -p swissarmyhammer-validators, 698 passed, 0 skipped. fmt and clippy clean.
    - commit: 7896580b9
    - review: clean — 0 findings over 7896580b9~1..7896580b9, 8 validators attempted, 0 failed. Task moved to done.
  timestamp: 2026-08-13T00:15:22.101870+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffffff780
title: Swift rule status tables state stdout byte counts that move with the path length
---
The status tables of the three shipped swiftlint rules state a byte count
for each measured run, for example `1 entry, 385 bytes` at
`builtin/validators/code-hygiene/rules/magic-numbers-swift.md`, and
`2 entries, 726 bytes` and `1 entry, 364 bytes` at
`builtin/validators/code-hygiene/rules/missing-docs-swift.md`, and
`1 entry, 413 bytes` at
`builtin/validators/code-hygiene/rules/complexity-swift.md`.

The swiftlint JSON reporter writes the ABSOLUTE path of the file into each
entry, so the byte count of a report moves with the length of that path. A
later measurement from a different directory gives a different count for the
same run, and a reviewer reads the row as false.

Measured with swiftlint 0.65.0 from a fixture tree under
`/private/tmp/.../work`: the magic-numbers row gave 382 bytes where the
table states 385; the docs 2-entry row gave 720 where the table states 726;
the docs threshold row gave 945 where the table states 949; the docs
parse-error row gave 361 where the table states 364; the complexity row gave
411 where the table states 413. Each difference is 3 bytes for each entry,
which is the difference of the two path lengths.

The entry counts hold at each row. The byte counts are the part that does
not reproduce.

Answer one of these: state the byte count as a count that depends on the
path, drop the byte count and keep the entry count, or state the path the
count belongs to.

Found while working ^xv57pf8. #tool-validators #objectivity