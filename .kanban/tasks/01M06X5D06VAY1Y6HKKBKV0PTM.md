---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m07jb52x8s3qf9gjar0xhyj6
  text: |-
    Picked up. Read commit `8fb77fd98` (card `^jttzhnd`) and both rule bodies first.

    Survey of the revive read path, measured on revive 1.15.0, each shape staged
    BESIDE one sound Go file that reports, under `[rule.exported]`:

    | the path | `[ -e ]` | `[ -r ]` | revive | stderr bytes | the sound file |
    |---|---|---|---|---|---|
    | a file at mode 000 | yes | no | exit 0, no record of it | 0 | reported |
    | a file at mode 200 | yes | no | exit 0, no record of it | 0 | reported |
    | a symlink to a file at mode 000 | yes | no | exit 0, no record of it | 0 | reported |
    | a directory nobody may read | yes | no | exit 1 | 106 | nothing |
    | a file under a directory nobody may read | no | no | exit 1 | 198 | nothing |
    | a dangling symlink | no | no | exit 1 | 190 | nothing |
    | a symlink loop | no | no | exit 1 | 182 | nothing |
    | a path that holds no file | no | no | exit 1 | yes | nothing |
    | a readable empty directory | yes | yes | exit 0, no record of it | 0 | reported |

    The line the survey draws: revive STATS each path before it lints. A path it
    can stat and cannot open is dropped in SILENCE at exit 0. A path it cannot
    stat costs the WHOLE run at exit 1, and the rule bodies already state that
    row. `[ -e ]` is the stat and `[ -r ]` is the open, so the two shell tests
    together name the silent shape exactly.

    The readable empty directory is no defect: revive reads a directory as a
    package and judges the Go files it holds, so an empty one holds nothing to
    judge.

    revive writes 0 bytes on stderr for every silent shape, so there is no line to
    forward under the marker. The script must test each path BEFORE revive starts,
    which is the shape `builtin/validators/README.md` names.

    A config that names a rule revive does not carry exits 1 with `cannot find
    rule: nosuchrule`, so no run met writes a byte on stderr at exit 0.
  timestamp: 2026-08-17T09:55:16.189690+00:00
- actor: claude-code
  id: 01m07kaccj2mt9b9et66ypkw1s
  text: |-
    Implementation landed.

    **Both scripts** now test each path they are given BEFORE revive starts:

        set -e
        if [ "$#" -eq 0 ]; then
          exit 0
        fi
        pending="$#"
        while [ "$pending" -gt 0 ]; do
          file="$1"
          shift
          pending=$((pending - 1))
          if [ -e "$file" ] && [ ! -r "$file" ]; then
            printf 'sah-diagnostic: <rule> cannot read %s, so its <items> are unread\n' "$file" >&2
            continue
          fi
          set -- "$@" "$file"
        done
        if [ "$#" -eq 0 ]; then
          exit 0
        fi

    `[ -e ]` is the stat and `[ ! -r ]` is the open, which is exactly the line the
    survey draws. The loop rotates the argument list, so a path with a space or a
    newline in its name stays whole.

    **The zero-argument guard stays FIRST.** The first shape put the filter above
    that guard, and `each_shipped_files_scope_script_answers_a_run_that_gives_it_no_file`
    broke for both rules: the three lines of the guard must stand together above
    every line that runs. The guard is now first, and a SECOND count guard stands
    under the filter for the run whose every path refuses the reader. Do not move
    the guard down again.

    The script calls no tool it did not call before — the filter uses `[`,
    `printf`, `set` and `shift` alone — so `check_command` is unchanged.

    **The absent path is NOT touched.** `[ -e ]` is false for it, so it still
    reaches revive and still costs the whole run at exit 1, which is the row both
    rule bodies already state and measure. This card names the SILENT shape.

    Measured over the SHIPPED scripts, final shape:

    | the run | stdout | stderr | exit |
    |---|---|---|---|
    | judged file + file at mode 000 | its finding | 1 marked line | 0 |
    | the file at mode 000 alone | nothing | 1 marked line | 0 |
    | no argument | nothing | nothing | 0 |
    | judged file + readable empty directory | its finding | nothing | 0 |
    | judged file + a path that holds no file | nothing | revive's own line | 1 |

    **Tests.** `verify_unreadable_file_is_declined` + `ShippedDeclineProbe` fit, so
    both new tests use them with `ShippedUnreadableFile::Forbidden`. `GO_FORBIDDEN_PATH`
    and `GO_FORBIDDEN_SOURCE` stand in `shipped.rs` beside `GO_PROJECT_TYPES`, so
    the two Go rules share ONE source rather than each holding a near-copy. The
    source holds an unexported type, which both halves of revive's `exported` rule
    carve out, so a run that DID read the file reports nothing of it and the
    diagnostic is the whole difference.

    RED before: both new tests failed with "the run must state the one item it
    declined; it stated []". GREEN after: both pass.

    Each test holds ITS OWN rule. With the `missing-docs-go` test broken alone:
    missing-docs FAIL, stuttering-name PASS. With `stuttering-name-go` broken
    alone: the reverse. Measured two times, on the first shape and again on the
    final shape.

    Rule bodies: both carry the new subsection "A path revive drops before it
    lints" with the read-path table of 8 rows, and both correct the sentences about
    the argument count and the temporary directory.
  timestamp: 2026-08-17T10:12:19.474646+00:00
- actor: claude-code
  id: 01m07kam1zj5ebxtz7t4h1bz9a
  text: |-
    ### implement — changed
    - evidence: 5 files — builtin/validators/code-hygiene/rules/missing-docs-go.md, builtin/validators/code-hygiene/rules/stuttering-name-go.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, .../tests/shipped/missing_docs.rs, .../tests/shipped/stuttering_name_go.rs. `cargo nextest run -p swissarmyhammer-validators`: 820 tests run, 820 passed, 0 skipped. `cargo nextest run -p mirdan`: 503 passed. `cargo clippy -p swissarmyhammer-validators --all-targets`: 0 warnings. RED before both new tests ("it stated []"), GREEN after, and each test proved to fail for its own rule alone.
    - next: /review
  timestamp: 2026-08-17T10:12:27.327665+00:00
position_column: doing
position_ordinal: '8280'
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