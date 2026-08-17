---
assignees:
- claude-code
position_column: todo
position_ordinal: fff080
title: missing-docs-python reports a path with a doubled backslash, because jq @tsv escapes it
---
`builtin/validators/code-hygiene/rules/missing-docs-python.md` hands the report
rows to its awk filter through `jq ... | @tsv`. `@tsv` ESCAPES a backslash to
`\\`, so a Python file whose name holds a backslash reaches awk under a name
that names no file.

Measured with ruff 0.14.5 and jq 1.8.2 over a probe holding `back\slash.py` and
`judged.py`:

- The finding row the script writes carries `back\\slash.py`, not
  `back\slash.py`. The engine then reads a path that is not on disk.
- The awk scan cannot open that name either, so the definition line the test
  carve-out reads is never read. `^hqe8qwv` made that a declined item at exit 0,
  stated as `sah-diagnostic: missing-docs-python could not read
  <repo>/back\\slash.py, so every finding of that file stands`, so the finding
  survives and the carve-out is simply unanswerable for the file.

The finding PATH is the part still wrong. A finding the engine cannot attribute
to a file is a finding nobody reads.

`@tsv` escapes a tab and a newline for the same reason it escapes a backslash,
so a naive `join("\t")` trades one defect for another: a path or a message
holding a real tab would then split into the wrong fields.

The work:

- Measure what `@tsv` does to each of `\`, a tab and a newline inside a
  `filename` and inside a `message`, and what the awk filter does with each.
- Pick a hand-off shape that survives every one of them — a NUL-separated
  record, one JSON object for each row read by a filter that parses it, or the
  Python filter shape `function-length-python` already takes.
- State the measurement in the rule body, and replace the note under
  "The scan of the definition line, which fails open" that points at this card.
- Hold the fix with an acceptance test that stages a Python file whose name
  holds a backslash beside a judged file, and holds the run to reporting the
  finding at the REAL path.

Found while implementing `^hqe8qwv`. #tool-validators #objectivity