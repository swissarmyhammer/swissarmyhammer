---
assignees:
- claude-code
position_column: todo
position_ordinal: ffd280
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