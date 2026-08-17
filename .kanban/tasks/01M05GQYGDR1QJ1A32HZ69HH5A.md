---
assignees:
- claude-code
position_column: todo
position_ordinal: ffee80
title: Three rules break the batch on a pre-flight readability guard that also misses the non-UTF-8 file
---
`builtin/validators/code-hygiene/rules/missing-docs-swift.md`,
`builtin/validators/code-hygiene/rules/magic-numbers-swift.md` and
`builtin/validators/code-hygiene/rules/missing-docs-dart.md` each open with a
pre-flight loop that walks the argument list and exits 1 for one path it cannot
read, before the tool runs at all. The two swift rules also exit 1 on
swiftlint's own `Could not read contents of` line.

Two defects in one guard:

1. It breaks the WHOLE run for ONE declined path, which
   `builtin/validators/README.md` refuses: "Do not exit nonzero for a declined
   item. A nonzero exit fails the WHOLE run, so one unjudged path throws away
   every finding the run did make."
2. `[ ! -r "$file" ]` cannot answer every refusing shape. Measured while
   implementing `^d3j6sbt` against three staged paths: the test is true for a
   path that holds no file and for a file with no read permission, and FALSE for
   a file whose bytes are not UTF-8 — the mode lets a reader open that one. A
   run gated on the test reads the third file as CLEAN.

`function-length-python` records both verdicts and holds the worked answer: read
what the TOOL itself said, and write it under `sah-diagnostic:` at exit 0.

The work:

- Measure, for each of the three rules, what its tool says for each of the three
  refusing paths — the report, stderr, and exit — and what it reported for the
  OTHER files of the same run.
- Replace the pre-flight guard with a read of the tool's own message, written
  under the marker at exit 0. The marker must OPEN the line.
- Rewrite the acceptance tests that lock the current break.
  `verify_unreadable_file_is_declined` in
  `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`
  holds the shape, and `ShippedUnreadableFile` already stages all three refusing
  paths.
- State each measurement in each rule body.

Related to `^8nbxwq5`, which covers the SILENT declines of the three swiftlint
rules. This card covers the guards that break the run instead.

Found while implementing `^s8d7fva`. #tool-validators #objectivity