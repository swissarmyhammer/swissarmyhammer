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
- actor: claude-code
  id: 01m07ph4ger2jy7kw67qbnrfbn
  text: |
    ### review — findings
    - evidence: 2 findings — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:1256, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:1285
    - scope: review sha HEAD~1..HEAD (64fc37380)
    - next: Remove the duplicate code from the new Go decline tests. Make one shared probe builder and one shared test helper. Then run the review again.
  timestamp: 2026-08-17T11:08:26.510424+00:00
- actor: claude-code
  id: 01m07phr6fpy043bb2rctq7103
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 5 files. Each script filters its argument list under `[ -e ] && [ ! -r ]`, states one marked line for each such path, and exits 0. The absent path still breaks the run. The survey found 3 silent shapes and 5 loud ones. Both tests use the shared decline helper.
    - test: green — cargo nextest run --workspace, 14162 passed, 0 failed. fmt and clippy clean. The test step re-drove real revive and confirmed it truly writes 0 bytes and makes no record for a mode-000 file. Each test was proved to fail for its own rule alone.
    - commit: 64fc37380
    - review: findings — 2. missing_docs.rs:1256 and :1285, both `reuse/reuse`: the new `go_missing_docs_decline_probe()` and its test repeat the pair in stuttering_name_go.rs.
  timestamp: 2026-08-17T11:08:46.671840+00:00
- actor: claude-code
  id: 01m07q03b5stbdkbnsg3vd7srm
  text: |
    Both findings worked. This pair is NOT the `^8nbxwq5` conflict.

    **The two builders read side by side.** `ShippedDeclineProbe` holds five
    fields. The probe of `missing-docs-go` and the probe of `stuttering-name-go`
    answered:

    | the field | missing-docs-go | stuttering-name-go | same? |
    |---|---|---|---|
    | `project_types` | `GO_PROJECT_TYPES` | `GO_PROJECT_TYPES` | yes |
    | `rule` | `GO_MISSING_DOCS_RULE` | `GO_STUTTERING_NAME_RULE` | no |
    | `judged` | one pair: `judged.go` + `type Plain struct{}` | one pair: `judged.go` + `type StagedType struct{}` | the path yes, the source no |
    | `path` | `GO_FORBIDDEN_PATH` | `GO_FORBIDDEN_PATH` | yes |
    | `expected` | one row: `go_missing_docs_judged_row()` | one row: `go_stuttering_name_judged_row()` | no |

    The judged source must differ: `missing-docs-go` needs a name that does NOT
    repeat its package, so revive answers under `comments` alone; `stuttering-name-go`
    needs a name that DOES repeat it. Each source therefore holds exactly ONE
    finding of its own rule.

    **Why this is the opposite case of `^8nbxwq5`.** There, one rule carried TWO
    heads and the others carried one, so the probes did not share a SHAPE and no
    factory could hold them. Here both probes carry the same shape: one judged
    file, one expected row, one refusing path, one project-type list. Only the
    VALUES differ, and three of the five fields carry one value for both rules. A
    builder that fixes those three and takes the other two is a real reduction, not
    the same struct literal under a longer signature.

    **What was made.** ONE function in `shipped.rs`, beside `GO_FORBIDDEN_PATH` and
    `GO_FORBIDDEN_SOURCE`:

        #[cfg(unix)]
        fn verify_go_rule_declines_a_forbidden_path(
            rule: &'static str,
            judged: (&'static str, &'static str),
            judged_row: String,
        )

    It builds the probe and drives `verify_unreadable_file_is_declined` with
    `ShippedUnreadableFile::Forbidden(GO_FORBIDDEN_SOURCE)`. One function answers
    BOTH findings: the probe builder of each file is gone, and each test body is
    now one call of three arguments. Two functions — a builder and a wrapper of the
    verify call — would leave the builder with one caller and add a step that
    carries nothing, so one function holds the pair.

    `GO_FORBIDDEN_PATH` already carried the sentence "ONE path and ONE source serve
    the refusing-path probe of both rules", so the shared function stands where
    that pair stands.

    **Proof each test still holds its OWN rule.** The marked line of one rule was
    taken away, both tests were run, and the line was written back word for word:

    | the line taken away | missing-docs test | stuttering-name test |
    |---|---|---|
    | `missing-docs-go` `printf 'sah-diagnostic: ...'` | FAIL, "it stated []" | PASS |
    | `stuttering-name-go` `printf 'sah-diagnostic: ...'` | PASS | FAIL, "it stated []" |

    `git status` shows both rule bodies unchanged after the two rounds.

    `cargo nextest run -p swissarmyhammer-validators`: 820 tests run, 820 passed,
    0 skipped. `cargo fmt --check` clean, `cargo clippy --all-targets` 0 warnings.
  timestamp: 2026-08-17T11:16:36.837968+00:00
- actor: claude-code
  id: 01m07q09r8mttqv2f8ansdq46w
  text: |
    ### implement — changed
    - evidence: 3 files — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, .../tests/shipped/missing_docs.rs, .../tests/shipped/stuttering_name_go.rs. Both `reuse/reuse` findings are closed by ONE shared function `verify_go_rule_declines_a_forbidden_path`. `cargo nextest run -p swissarmyhammer-validators`: 820 run, 820 passed, 0 skipped. fmt and clippy clean. Each test proved to fail for its own rule alone and to stay green for the other.
    - next: /review
  timestamp: 2026-08-17T11:16:43.400713+00:00
- actor: claude-code
  id: 01m07qvz332bs2ynrw4r3y3axw
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` over commit 30f1885c0. 18 rules ran, 0 findings, 0 failed. The 2 items from the 2026-08-17 06:00 section are checked.
    - next: none. The card moved to `done`.
  timestamp: 2026-08-17T11:31:49.987158+00:00
- actor: claude-code
  id: 01m07qwjtf2vsahqmsarry6r61
  text: |
    ### finish iteration 2 — clean
    - implement: changed — 3 files. One shared function, `verify_go_rule_declines_a_forbidden_path`, holds both Go decline probes. Each test file calls it with one line. This is NOT the ^8nbxwq5 conflict: the two probes carry the same SHAPE and differ only in the rule, the judged source and the row that follows from it.
    - test: green — cargo nextest run --workspace, 14162 passed, 0 failed. fmt and clippy clean. The shared function was proved to hold every condition the two old builders held, and each test still fails for its own rule alone.
    - commit: 30f1885c0
    - review: clean — 18 rules, 0 findings. Both prior findings checked.
  timestamp: 2026-08-17T11:32:10.191312+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffa680
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

## Review Findings (2026-08-17 06:00)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 3 file(s) reviewed, 4 not reviewed.

> 4 file(s) not reviewed — excluded by an ignore rule:
> - `.kanban/ (from .reviewignore)` — 4 file(s)

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:1256` `reuse/reuse` — Function duplicates `go_stuttering_name_decline_probe()` which already exists in the parallel test file `stuttering_name_go.rs`. The two functions are 95% identical, differing only in `rule` constant and the `expected` row function; they should be consolidated into a single parameterized builder. Extract a generic `go_decline_probe(rule: &str, judged_row_fn: fn() -> Row)` helper function in `shipped.rs` or a shared location, then have both `go_missing_docs_decline_probe()` and `go_stuttering_name_decline_probe()` call it with their rule-specific parameters.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:1285` `reuse/reuse` — Test function is 96% identical to `the_shipped_go_stuttering_name_tool_rule_declines_a_file_it_may_not_read()` in `stuttering_name_go.rs`. Both call the same shared helper `verify_unreadable_file_is_declined()` with only the probe builder differing; test logic should be extracted to avoid duplication. Extract a shared `go_rule_declines_unreadable_file(probe: &ShippedDeclineProbe)` helper function that wraps the call to `verify_unreadable_file_is_declined()`, then have both test functions call this helper with their respective probes.
