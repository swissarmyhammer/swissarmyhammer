---
assignees:
- claude-code
position_column: todo
position_ordinal: fff280
title: missing-docs-go and stuttering-name-go read a Go file nobody may read as a clean file
---
Found while implementing `^jttzhnd`.

`builtin/validators/code-hygiene/rules/missing-docs-go.md` and
`builtin/validators/code-hygiene/rules/stuttering-name-go.md` both run revive.
revive DROPS a `.go` file it cannot open, in silence.

Measured with revive 1.15.0 on this machine, over `noread.go` at mode 000
beside one sound file that reports:

- exit 0
- NO record about `noread.go` at all, of any category
- the sound file still reports its finding

The same file alone, at mode 000: exit 0, and `-formatter json` writes `null`.

So both shipped scripts answer "clean" for a file no reader may open. That is
the shape `builtin/validators/README.md` forbids: "Do not stay silent either: a
run that reports no finding and exits 0 over an item it never judged reads
exactly like a clean pass over that item."

The unparsable shape does NOT have this defect — revive writes an unnamed
record for it, and `^jttzhnd` made the script state that record under
`sah-diagnostic:` at exit 0.

The work:

- Measure WHICH shapes revive drops in silence. Mode 000 is one. Survey the
  read path rather than stopping at the one shape this card names: revive
  resolves its paths first, and a path it drops at resolution may answer the
  same way.
- Decide by evidence whether the script can see the difference from outside
  revive. A test of each argument BEFORE revive starts is the shape
  `builtin/validators/README.md` names: "A tool can exit 0 for a file it could
  not open, and print an empty report. Test each file the script is given
  before the tool starts."
- State each such item under `sah-diagnostic:` at exit 0, in BOTH rules — one
  cause, two files.
- Add an acceptance test for each rule that stages a file that REPORTS beside
  the unreadable one, so the test proves the findings survive.
  `verify_unreadable_file_is_declined` is the helper the sibling rules use, and
  it takes `ShippedUnreadableFile::Forbidden`.
- State the measurement in both rule bodies.

#tool-validators #objectivity