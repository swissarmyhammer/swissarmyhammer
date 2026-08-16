---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m05spxdvt4j3ggyxx292fr1v
  text: |-
    Research and measurement, before any edit. ruff 0.14.5, jq 1.8.2, awk 20200816, macOS.

    Probe tree: `judged.py` (no module docstring, one undocumented function -> D100 + D103), `unparsable.py` (`def broken(`), `absent.py` (no file), `notutf8.py` (bytes `\xff\xfe` inside a literal), `noread.py` (mode 000).

    What ruff does against the shipped command line (`--isolated --no-cache --select D100,D101,D102,D103,D104,D106,D107 --output-format json`):

    | the run | report codes | stderr | exit |
    |---|---|---|---|
    | judged.py unparsable.py | D100, D103, invalid-syntax | nothing | 1 |
    | judged.py absent.py | D100, D103 | `warning: Failed to lint absent.py: No such file or directory (os error 2)` | 1 |
    | judged.py notutf8.py | D100, D103 | `warning: Failed to lint notutf8.py: stream did not contain valid UTF-8` | 1 |
    | judged.py noread.py | D100, D103 | `warning: Failed to lint noread.py: Permission denied (os error 13)` | 1 |
    | all five | D100, D103, invalid-syntax | the three `Failed to lint` lines | 1 |

    So ruff judged `judged.py` in EVERY row. The two findings are there to lose.

    What the SHIPPED script does over the same probe:

    | the run | stdout | stderr | exit |
    |---|---|---|---|
    | judged.py | 2 findings | nothing | 0 |
    | judged.py unparsable.py | NOTHING | `.../unparsable.py:2: invalid-syntax unexpected EOF while parsing` | 1 |
    | judged.py absent.py | NOTHING | `missing-docs-python cannot read absent.py` | 1 |
    | judged.py notutf8.py | 2 findings | ruff's raw `warning: Failed to lint notutf8.py: ...`, UNMARKED | 0 |
    | judged.py noread.py | NOTHING | `missing-docs-python cannot read noread.py` | 1 |

    Row 4 is the guard's blind spot the card names, measured: `[ ! -r "$file" ]` is FALSE for the non-UTF-8 file, ruff declines it, and the raw stderr line carries no marker, so the engine drops it as tool chatter at exit 0. `notutf8.py` reads as CLEAN.

    One shape hides the other, the way `^qvj3v4g` did. Measured over `absent.py unparsable.py`: exit 1, stderr `missing-docs-python cannot read absent.py` and NOTHING about the parse failure — the pre-flight guard runs before ruff, so the parse decline is never stated at all.

    The third `exit 1`, inside the awk scan, is REACHABLE through the shipped pipeline, deterministically. `jq @tsv` escapes a backslash to `\\`, so a Python file named `back\slash.py` reaches awk as `back\\slash.py`, which awk cannot open. Measured with the shipped script over `back\slash.py judged.py`: exit 1, nothing on stdout, `missing-docs-python cannot read /.../back\\slash.py` on stderr — the D100 and D103 of `judged.py` lost. The rule's own prose already claims the fail-open this code does not have.

    The broken-run gate still has work to do: `--select ZZ999` exits 2, writes 0 bytes to stdout, and `error: invalid value 'ZZ999' for '--select <RULE_CODE>'` to stderr. That gate stays.

    Scope of the pre-flight guard: `^z69hh5a` names `missing-docs-swift`, `magic-numbers-swift` and `missing-docs-dart`, and NOT `missing-docs-python`. This card names the guard as item 2 of its own three. So the guard is in scope HERE, and `^z69hh5a` keeps the other three rules.
  timestamp: 2026-08-16T17:25:32.731010+00:00
- actor: claude-code
  id: 01m05td284zgmzn21x3h5151zj
  text: |-
    Implementation landed. All three `exit 1` sites are now declined items at exit 0, and the broken-run gate stands.

    RED first, four tests, each failing for its own reason:
    - `..._declines_a_file_it_cannot_parse` — `a script handed an item it cannot judge must judge the rest and exit 0: Exit("/private/var/.../repo/broken.py:2: invalid-syntax unexpected EOF while parsing")`
    - `..._declines_a_path_that_holds_no_file` — `Exit("missing-docs-python cannot read unreadable.py")`
    - `..._declines_a_file_it_may_not_read` — the same pre-flight guard shape
    - `..._declines_a_file_it_cannot_decode` — a DIFFERENT failure: `assertion left == right failed: the run must state the one item it declined; it stated []`, left 0, right 1. That is the guard's blind spot: exit 0, the finding kept, and ruff's raw unmarked line dropped as tool chatter, so the file read as CLEAN.

    The three replacements:
    1. The `unread.txt` row of another code now goes straight from jq to stderr as `sah-diagnostic: ruff could not measure <path>: <code> <message>`, at exit 0.
    2. The pre-flight `[ ! -r "$file" ]` loop is GONE. The script captures ruff's stderr to `$work/ruff.err` and reads each line opening `warning: Failed to lint ` — the one channel that answers all three refusing shapes, the non-UTF-8 file included. The head is stripped as a quoted value, `${line#"$declined_head"}`, so a reason keeps every `: ` it holds and a glob character in the head stays literal. Nothing is split positionally; the whole `path: reason` remainder is forwarded.
    3. The awk scan now states the file under the marker and reads no line for it, so the finding stands. That is the fail-open the rule's prose always claimed.

    The broken-run gate stays and got stronger: a ruff status over 1 forwards ruff's stderr AND its report, then states `missing-docs-python: ruff exited <status> and judged no code`, and exits 1.

    Measured with the fixed script over the probe:

    | the run | stdout | stderr | exit |
    |---|---|---|---|
    | judged2.py | 1 finding | nothing | 0 |
    | judged2.py unparsable.py | 1 finding | one `could not measure` line | 0 |
    | judged2.py absent.py | 1 finding | one `could not read` line | 0 |
    | judged2.py notutf8.py | 1 finding | one `could not read` line | 0 |
    | judged2.py noread.py | 1 finding | one `could not read` line | 0 |
    | all five together | 1 finding | 4 marked lines | 0 |
    | `back\slash.py` judged2.py | 3 findings | one scan `could not read` line | 0 |
    | no argument | nothing | nothing | 0 |
    | a stub ruff exiting 2 | nothing | the stub's stderr + the named status | 1 |
    | a stub ruff exiting 101 | nothing | the stub's stderr + the named status | 1 |
    | a stub jq exiting 3 | nothing | nothing | 3 |
    | the test carve-out probe | `D100` + `helper_thing` | nothing | 0 |

    The trap was re-measured against the new shapes by counting directories directly under `TMPDIR`: 291445 before, 291445 after five clean runs, after a run that declines four items at exit 0, and after a run that exits 1 on a broken ruff.

    Scope decision, stated: `^z69hh5a` names `missing-docs-swift`, `magic-numbers-swift` and `missing-docs-dart` only. `missing-docs-python` is not on it, and THIS card names the guard as item 2 of its own three, so the guard was removed HERE. `^z69hh5a` keeps the other three rules untouched.

    New work found and carded, not silently absorbed: `^b2kq9hy` — `jq @tsv` escapes a backslash, so a file named `back\slash.py` is reported at a path with a DOUBLED backslash on the finding row as well as in the scan. The fail-open makes the finding survive; the wrong path is a separate defect. The rule body points at that card. No acceptance test stages a backslash path here, because such a test would lock the wrong path into an assertion.
  timestamp: 2026-08-16T17:37:38.564341+00:00
- actor: claude-code
  id: 01m05tdcatks5dev3e6j34w16m
  text: |-
    ### implement — changed
    - evidence: 2 files — `builtin/validators/code-hygiene/rules/missing-docs-python.md` (+245/-…), `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs` (+192/-…). RED first on 4 tests; the parse and the two guard shapes failed `a script handed an item it cannot judge must judge the rest and exit 0: Exit(...)`, and the non-UTF-8 shape failed `the run must state the one item it declined; it stated []`. Gates: `cargo fmt` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0, 0 warnings; `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` 3525 run, 3525 passed, 0 failed, 0 skipped; `cargo build -p mirdan` exit 0 with the embedded copy traced byte-identical to the source, 19583 bytes, and the run block holding exactly one `exit 1` — the broken-run gate.
    - next: `/review`. Card `^b2kq9hy` was raised for the `jq @tsv` backslash path corruption the work uncovered.
  timestamp: 2026-08-16T17:37:48.890336+00:00
- actor: claude-code
  id: 01m05tv29fa4ys8ja6ks6rcw18
  text: |
    ### test — green

    Gates:
    - `cargo fmt --check`: exit 0, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings`: exit 0, zero warnings, zero errors.
    - `cargo nextest run --workspace`: 14110 tests run, 14110 passed, 0 failed, 0 skipped (194.678s). Zero `FAIL` lines in the log. All `missing_docs::the_shipped_python_missing_docs_tool_rule_*` tests pass, including the four new probes (`_declines_a_file_it_cannot_parse`, `_declines_a_path_that_holds_no_file`, `_declines_a_file_it_cannot_decode`, `_declines_a_file_it_may_not_read`).
    - `cargo build -p mirdan`: exit 0. Traced `OUT_DIR=target/debug/build/mirdan-2dd4355981eab855/out` via `cargo build -p mirdan -vv`. Confirmed the `("code-hygiene/rules/missing-docs-python.md", ...)` entry in `$OUT_DIR/builtin_validators.rs` embeds the corrected script: the `run:` script body has zero occurrences of `[ ! -r "$file" ]` (the sole remaining occurrence in the entry is in the doc PROSE explaining the removal, not in the script), and the script carries `declined_head`/`sah-diagnostic:` handling.

    Behaviour checks — ran the script extracted verbatim from `builtin/validators/code-hygiene/rules/missing-docs-python.md`, against real ruff 0.14.5, in a scratch probe directory:

    - `judged.py` (module docstring, one undocumented function) alone with each refusing shape:
      - + `broken.py` (unparseable): stdout = 1 `D103` line, stderr = 1 line `sah-diagnostic: ruff could not measure <path>/broken.py: invalid-syntax unexpected EOF while parsing`, exit 0.
      - + `absent.py` (does not exist): stdout = 1 `D103` line, stderr = 1 line `sah-diagnostic: ruff could not read absent.py: No such file or directory (os error 2)`, exit 0.
      - + `notutf8.py` (non-UTF-8 bytes): stdout = 1 `D103` line, stderr = 1 line `sah-diagnostic: ruff could not read notutf8.py: stream did not contain valid UTF-8`, exit 0. **This is the case the old `[ ! -r "$file" ]` guard silently passed as clean — now explicitly declined with a marked line.**
      - + `noread.py` (mode 000): stdout = 1 `D103` line, stderr = 1 line `sah-diagnostic: ruff could not read noread.py: Permission denied (os error 13)`, exit 0.
    - All five files together (`judged.py broken.py absent.py notutf8.py noread.py`): stdout = 1 `D103` line (finding survives), stderr = 4 marked `sah-diagnostic:` lines (one per refused path), exit 0.
    - Broken-run gate: stub `ruff` exiting status 2 → exit 1, stdout empty, stderr carries the stub's own message plus `missing-docs-python: ruff exited 2 and judged no code`. Stub `ruff` exiting status 101 → exit 1, stdout empty, stderr carries the stub's own message plus `missing-docs-python: ruff exited 101 and judged no code`. Neither run placed a finding.
    - No-argument guard: script run with no file arguments over a tree holding `top.py` and `deep/nested/other.py` → stdout empty, stderr empty, exit 0 (does not walk the tree). Sanity check: the same script given those two paths explicitly reports 4 findings, proving the no-arg run is the guard and not an accidental empty tree.

    Both files remain modified and uncommitted (`git status --short` confirms). No commit made. Task left in `doing`.
  timestamp: 2026-08-16T17:45:17.359031+00:00
position_column: doing
position_ordinal: '8280'
title: missing-docs-python fails the whole run for one file ruff cannot parse, and for one path it cannot read
---
`builtin/validators/code-hygiene/rules/missing-docs-python.md` exits 1 three
different ways for ONE declined item, and each one throws away every finding the
run did make.

`builtin/validators/README.md` states the answer: a script that judged the code
and could not judge ONE item writes a line opening `sah-diagnostic:` and still
exits 0. "Do not exit nonzero for a declined item. A nonzero exit fails the
WHOLE run, so one unjudged path throws away every finding the run did make."

The three:

1. A row of another code on the report — a file ruff could not PARSE — lands in
   `$work/unread.txt`, and `if [ -s "$work/unread.txt" ]; then cat ...; exit 1; fi`.
   Measured with ruff 0.14.5 while implementing `^s8d7fva`: ruff reports the
   OTHER files of the same run beside the `invalid-syntax` row, so those
   findings are there to lose.
2. A pre-flight guard, `for file in "$@"; do if [ ! -r "$file" ]; then ...; exit 1; fi; done`,
   breaks the batch before ruff runs at all. The rule body of `function-length-python`
   already records that this test cannot answer all three refusing shapes:
   `[ ! -r "$file" ]` is FALSE for a file whose bytes are not UTF-8, because the
   mode lets a reader open it. So the guard both breaks the run AND misses a
   shape.
3. A third `exit 1` sits inside the awk filter, for a source line it could not
   re-read. That one fires AFTER ruff judged every file, so it is the purest
   case of one declined item discarding a judged run. The rule's own prose says
   "A row the scan does not hold keeps its finding, so a short read can add a
   finding and can drop none" — that describes a fail-open the code does not
   have.

The work:

- Measure each of the three shapes against the shipped script: what ruff
  reported for the OTHER files of the same run, and what the script did with it.
- Replace each `exit 1` with a `sah-diagnostic:` line at exit 0. Note the trap
  `^s8d7fva` hit: the marker must OPEN the line, or the engine drops it as tool
  chatter.
- Two acceptance tests lock the current break —
  `..._breaks_on_a_file_it_cannot_read` and `..._breaks_on_a_file_it_cannot_parse`.
  Rewrite both to hold the declined-item answer, staging a file with a missing
  docstring beside the declined item so the test proves the findings survive.
  `verify_unjudged_file_is_declined` and `verify_unreadable_file_is_declined` in
  `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`
  already hold the shape.
- Restate the rule body against what was measured.

Found while implementing `^s8d7fva`. #tool-validators #objectivity