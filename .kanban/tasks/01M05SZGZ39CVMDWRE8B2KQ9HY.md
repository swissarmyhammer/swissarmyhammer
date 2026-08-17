---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m06z1npm8v6brxxdjdrcyf0j
  text: |-
    Measured the hand-off with ruff 0.14.5, jq 1.8.2 and python 3.14.6.

    `@tsv` changes each of the three characters the same way, in the `filename` and
    in the `message` alike. The FIELDS survive and the TEXT does not:

    | the character | what `@tsv` writes | what `awk -F'\t'` reads |
    |---|---|---|
    | a backslash | two backslashes | one field, the backslash doubled |
    | a tab | a backslash and `t` | one field, the tab gone |
    | a newline | a backslash and `n` | one row, the newline gone |

    `join("\t")` is worse: over one report of seven rows that holds all three
    names, jq wrote NINE lines. The newline path split its row in two, and the tab
    path pushed each field one place right, so awk read the file name as two fields
    and the code as the row number.

    So the fix takes the text hand-off away. The two `jq` filters and the `awk`
    scan are now ONE embedded Python program, the shape `function-length-python`
    already takes. It reads ruff's JSON report itself and writes each finding as
    one JSON object — the second stdout shape `builtin/validators/README.md`
    states. The JSON object is needed, not chosen: `json.dumps` escapes a newline
    INSIDE the object, and the engine reads stdout one line at a time, so a
    `path:line: message` row cannot carry a path that holds a newline.

    RED, through the real ruff pipeline, over `judged.py` beside `back\slash.py`:
    `["back\\\\slash.py:1", "back\\\\slash.py:1", "judged.py:4"]`.
    GREEN over the same probe: `["back\\slash.py:1", "back\\slash.py:1",
    "judged.py:4"]`, exit 0, and no diagnostic. A file whose name holds a tab and a
    file whose name holds a newline each read the same way.

    Every other measured behaviour holds, checked against the shipped bytes:
    - the four staged positions report the same 6 findings;
    - an unparsable file gives 1 finding and 1 marked line at exit 0;
    - `judged.py` beside the unparsable file, three refusing paths and a mode-000
      directory gives 1 finding, 5 marked lines, exit 0;
    - ruff status 2 and 101 each exit 1 with ruff's own stderr;
    - no argument gives no finding at exit 0;
    - the `TMPDIR` count stays at 0 after a clean run, a declining run and a broken
      run.

    Two changes beside the filter:
    - the doctor check is now `which ruff python3 mktemp`; `jq` and `awk` are gone.
    - the broken-report gate now says `missing-docs-python: the filter could not
      read the ruff report: <the reason Python gave>`. Measured against a filter
      with no guard: exit 1, 0 bytes on stdout, a Python traceback naming the
      temporary copy of the filter, and no word about which rule broke.

    The fail-open of the definition-line scan is no longer reachable through the
    shipped pipeline, because the filter opens each path ruff wrote and ruff states
    every path it could not open itself. Its acceptance test therefore drives a
    stub ruff that reports a path holding no file, and holds the run to two
    findings, one marked line and exit 0.
  timestamp: 2026-08-17T04:18:02.580575+00:00
- actor: claude-code
  id: 01m06z1w55s6w1n55sj7hg9cfg
  text: |-
    ### implement — changed
    - evidence: 2 files — builtin/validators/code-hygiene/rules/missing-docs-python.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs. RED then GREEN on `the_shipped_python_missing_docs_tool_rule_reports_a_path_holding_a_backslash`. `cargo nextest run -p swissarmyhammer-validators`: 795 passed, 0 failed. `cargo nextest run -p mirdan`: 503 passed. `cargo clippy -p swissarmyhammer-validators --all-targets`: no warning.
    - next: /review
  timestamp: 2026-08-17T04:18:09.189689+00:00
- actor: claude-code
  id: 01m06zmgtx3z3mmk0dgb3pbr21
  text: |-
    ### test — green

    Verified all 5 implementer claims by measurement:

    1. **Stub-vs-real split is legitimate, not a defect.** Only 3 of the ~15 tests in `missing_docs.rs` drive a stubbed ruff (`..._keeps_the_findings_its_scan_cannot_carve`, `..._breaks_on_a_report_the_filter_cannot_read`, `..._reads_a_decline_with_no_closing_newline`) — all pre-existing defensive-code paths that real ruff cannot trigger (ruff only reports a finding for a file it successfully opened, so the definition-line scan reopening that same file cannot legitimately fail outside a race). The rest — `..._reports_every_fail_fixture_item`, `..._reads_the_item_name_and_not_the_path`, `..._declines_a_file_it_cannot_parse`, `..._declines_a_path_that_holds_no_file`, `..._declines_a_file_it_cannot_decode`, `..._declines_a_file_it_may_not_read`, `..._declines_a_directory_it_may_not_read`, the new `..._reports_a_path_holding_a_backslash`, and `..._reads_only_the_files_it_is_given` — all call `require_tool_installed` + `drive_shipped_script`, driving real ruff over real staged fixtures. The file is not stub-only.
    2. **RED→GREEN proved by hand.** Overwrote `builtin/validators/code-hygiene/rules/missing-docs-python.md` with the HEAD (pre-change, `jq @tsv`/`awk`) content, ran `cargo nextest run -p swissarmyhammer-validators the_shipped_python_missing_docs_tool_rule_reports_a_path_holding_a_backslash` → FAILED (`left: ["back\\\\slash.py:1", ...] right: ["back\\slash.py:1", ...]`). Restored the working-tree file byte-identical (md5 `ca5be4a0cb074b5032788b9a01f32fd6` before and after), re-ran → PASSED.
    3. **Hand-driven real-ruff run over a backslash path.** Extracted the shipped `tool.run` script verbatim, ran it directly with real ruff over `judged.py` + `back\slash.py`: exit 0, 0 bytes stderr, each reported `file` field decodes (verified via `json.loads`) to a name with `backslash_count == 1`.
    4. **Doctor check matches script tool calls.** New script calls only `ruff`, `mktemp`, `python3` (plus POSIX builtins `cat`/`printf`/`trap`/`while read`, which no sibling rule's doctor check lists either — e.g. `function-length-python.md` uses the identical `which ruff python3 mktemp` while also calling `cat`). Old doctor check was `which ruff jq awk mktemp`; new is `which ruff python3 mktemp` — `jq`/`awk` correctly dropped since the script no longer calls them, `python3` correctly added.
    5. **No new unwrap/expect/panic/unnamed-literal in the diff, beyond established idiom.** One `.expect(` textually increased (the old single test's closure-`.expect` split across two tests, reusing the same message text on the new stub test) — this is redistribution of an existing idiom already used pervasively throughout this exact file (`.expect(...)`/`.expect_err(...)` on every `drive_shipped_script*` call), consistent with this codebase's own convention of treating unwrap/panic differently in test code (see `function-length-rust.md`'s note on clippy's `allow-unwrap-in-tests`/`allow-panic-in-tests`). No `.unwrap()` or `panic!(` added. No unnamed numeric literals in Rust code — the only numeric-looking matches in the diff are inside `///` doc comments or inside the raw-string JSON stub payload (`"row": 1`), which is self-documented by its JSON key, not a bare magic number.

    Full verification:
    - `cargo fmt --check` — exit 0, clean
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, clean
    - `cargo nextest run --workspace` — 14137 tests run: 14137 passed (37 slow), 0 skipped, 0 failed

    evidence: cargo fmt --check (exit 0); cargo clippy --workspace --all-targets -- -D warnings (exit 0); cargo nextest run --workspace — 14137 passed, 0 failed, 0 skipped
    next: ready for review
  timestamp: 2026-08-17T04:28:20.189484+00:00
position_column: doing
position_ordinal: '8280'
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