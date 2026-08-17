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
- actor: claude-code
  id: 01m06zzjv9mjc8mfkb9p9kdpn5
  text: |-
    ### review — findings
    - evidence: 3 findings, 3 confirmed, 2 refuted, 9 attempted. Scope `review sha HEAD~1..HEAD` (c759e69f0). crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:1396, :1459, :1498 — all `magic-numbers/no-magic-numbers`.
    - next: correct the cause in the whole file, then run /review again.
  timestamp: 2026-08-17T04:34:22.697207+00:00
- actor: claude-code
  id: 01m07007xxc38pm8hcpvma85t7
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 2 files (missing-docs-python.md, missing_docs.rs). The two jq filters and the awk scan became one embedded Python program. The doctor check is now `which ruff python3 mktemp`.
    - test: green — cargo nextest run --workspace, 14137 passed, 0 failed. fmt and clippy clean. RED/GREEN proof done on the backslash probe. 9 of the 12 tests still drive real ruff.
    - commit: c759e69f0
    - review: findings — missing_docs.rs:1396, missing_docs.rs:1459, missing_docs.rs:1498. One cause: a line number written as a bare literal in a string.
  timestamp: 2026-08-17T04:34:44.285103+00:00
- actor: claude-code
  id: 01m070j9b3zg5p58hf5sr0x6yk
  text: |-
    The three findings name one cause: a line number written as a bare literal in
    a string. I read every line of `missing_docs.rs` and found 21 such literals at
    9 positions, not 3. The three the review printed are three of them.

    The whole list, and the value each names:

    | position | what it was | where the number comes from |
    |---|---|---|
    | `DART_BROKEN_RUN_ROWS` | `":1"`, `":2"` | the class head opens `DART_BROKEN_RUN_SOURCE`, the method head stands under it |
    | `python_backslash_rows` | `":1"` twice | `D100` names the module; `D103` names the function that opens the file |
    | `PYTHON_VANISHED_REPORT_ANSWER` | `"row": 1`, `"row": 4` | the stub row of a path holding no file; the `def` line of `PYTHON_JUDGED_SOURCE` |
    | the vanished assertion | `":1"` | the same stub row, which must agree with the report |
    | `PYTHON_READ_FINDINGS` | `":1"` x4, `":2"` | module, class, method of the two unread Python files |
    | `SWIFT_READ_FINDINGS` | `":1"` x2, `":2"` x2 | the type head and the member under it |
    | `TYPESCRIPT_MISSING_DOCS_READ_FINDINGS` | `":1"` x2 | the exported function that opens each file |
    | `GO_MISSING_DOCS_READ_FINDINGS` | `":3"` x2 | a Go file opens with a `package` clause and a blank line |
    | `DART_MISSING_DOCS_READ_FINDINGS` | `":1"` x2, `":2"` x2 | the class head and the method under it |

    Each probe holds its rows in a `&'static [&'static str]` built with `concat!`,
    and `concat!` takes literals alone, so a row of a static probe cannot read a
    `const usize`. The file already answers that shape: `dart_broken_run_path!`
    stands as a macro beside the constant that holds it, for that stated reason. I
    named the rows the same way — 4 macros, each with a doc comment that says which
    bytes of which fixture put the declaration on that line:

    - `opening_declaration_row!` — the declaration that OPENS a probe source.
    - `nested_declaration_row!` — the member on the line directly under it.
    - `python_module_row!` — where ruff stands a `D100`, which names the module and
      carries no declaration line of its own. Same line, different reason.
    - `go_declaration_row!` — the third line, under the `package` clause.

    Two positions take a better answer than a name, because the number can be READ
    out of the fixture:

    - `python_backslash_rows` now calls `expected_row` for the `D103`, so the row
      is counted out of `PYTHON_BACKSLASH_SOURCE`.
    - `PYTHON_VANISHED_REPORT_ANSWER` was a `concat!` of raw strings and is now
      `python_vanished_report_answer()`. Its judged row calls `declaration_line` on
      `PYTHON_JUDGED_SOURCE`, so the stub report and the row the test reads back
      can never drift apart. It writes the same bytes as before.

    Three more constants came out of the same rewrite, because the stub spelled
    each of them twice: `PYTHON_UNDOCUMENTED_FUNCTION_CODE`,
    `PYTHON_UNDOCUMENTED_FUNCTION_MESSAGE`, and `python_stub_report_entry`, which
    builds one entry of the report.

    Every constant names the value the test measured before. `cargo nextest run -p
    swissarmyhammer-validators`: 795 passed, 0 failed, 0 skipped. `cargo fmt --all
    --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.
    No `.unwrap()`, `.expect(` or `panic!(` was added.
  timestamp: 2026-08-17T04:44:35.555798+00:00
- actor: claude-code
  id: 01m070jfmhtnejvj3886m41jmp
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs. 21 bare literal line numbers removed at 9 positions. New names: `opening_declaration_row!`, `nested_declaration_row!`, `python_module_row!`, `go_declaration_row!`, `PYTHON_BACKSLASH_DECLARATION`, `PYTHON_VANISHED_FINDING_ROW`, `PYTHON_UNDOCUMENTED_FUNCTION_CODE`, `PYTHON_UNDOCUMENTED_FUNCTION_MESSAGE`, `python_stub_report_entry`, `python_vanished_report_answer`. `cargo nextest run -p swissarmyhammer-validators`: 795 passed, 0 failed. fmt and clippy clean.
    - next: /review
  timestamp: 2026-08-17T04:44:42.001350+00:00
- actor: claude-code
  id: 01m0711pjyfv5pygt3fty7v6gk
  text: |-
    ### test — green (iteration 2)

    Verified all 5 claims by measurement, against `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs` diffed at `git diff HEAD` (working tree vs commit c759e69f0).

    1. **`PYTHON_VANISHED_REPORT_ANSWER` → `python_vanished_report_answer()` writes the same bytes.** Read the old constant with `git show HEAD:<path>`:
       `  printf '[{"filename": "vanished.py", "location": {"row": 1}, "code": "D103", "message": "Missing docstring in public function"}, {"filename": "judged.py", "location": {"row": 4}, "code": "D103", "message": "Missing docstring in public function"}]'\n  exit 1`
       Hand-expanded the new function: `python_stub_report_entry(PYTHON_VANISHED_PATH="vanished.py", PYTHON_VANISHED_FINDING_ROW="1")` and `python_stub_report_entry(PYTHON_JUDGED_PATH="judged.py", &declaration_line(PYTHON_JUDGED_SOURCE, PYTHON_JUDGED_DECLARATION).to_string())`. `PYTHON_JUDGED_SOURCE` is `"""A documented module."""\n\n\ndef undocumented_function() -> None:\n    return None\n` — 3 newlines before the `def` line, so `declaration_line` (pre-existing helper in `shipped.rs`) returns `4`. Confirmed the Rust line-continuation backslashes in the format string strip the newline and all leading whitespace of the next line (per the Rust reference), so no stray spaces enter the string. Byte-for-byte, the expanded new output is identical to the old constant.

    2. **All 9 positions name the same value as before.** Read each old literal via `git show HEAD:<path>` and diffed against the new macro/const expansion:
       - `DART_BROKEN_RUN_ROWS`: old `:1`,`:2` → `opening_declaration_row!()`="1", `nested_declaration_row!()`="2" ✓
       - `python_backslash_rows`: old `:1`,`:1` → `python_module_row!()`="1"; `expected_row(PYTHON_BACKSLASH_PATH, PYTHON_BACKSLASH_SOURCE, PYTHON_BACKSLASH_DECLARATION)` — declaration is at offset 0 in the source, `declaration_line` returns 1 → ":1" ✓
       - `PYTHON_VANISHED_REPORT_ANSWER`: row 1, row 4 → covered in (1) ✓
       - the vanished assertion: old `:1` → `PYTHON_VANISHED_FINDING_ROW` = `opening_declaration_row!()` = "1" ✓
       - `PYTHON_READ_FINDINGS`: old `1,1,2,1,1` → `python_module_row!(),opening_declaration_row!(),nested_declaration_row!(),python_module_row!(),opening_declaration_row!()` = `1,1,2,1,1` ✓
       - `SWIFT_READ_FINDINGS`: old `1,2,1,2` → `opening_declaration_row!(),nested_declaration_row!()` x2 = `1,2,1,2` ✓
       - `TYPESCRIPT_MISSING_DOCS_READ_FINDINGS`: old `1,1` → `opening_declaration_row!()` x2 = `1,1` ✓
       - `GO_MISSING_DOCS_READ_FINDINGS`: old `3,3` → `go_declaration_row!()` x2 = `3,3` ✓
       - `DART_MISSING_DOCS_READ_FINDINGS`: old `1,2,1,2` → `opening_declaration_row!(),nested_declaration_row!()` x2 = `1,2,1,2` ✓

    3. **Every macro/constant's doc comment says WHY, not just the value.** Read all 4 macros (`opening_declaration_row!`, `nested_declaration_row!`, `python_module_row!`, `go_declaration_row!`) and the new constants (`PYTHON_BACKSLASH_DECLARATION`, `PYTHON_VANISHED_FINDING_ROW`, `PYTHON_UNDOCUMENTED_FUNCTION_CODE`, `PYTHON_UNDOCUMENTED_FUNCTION_MESSAGE`) plus `python_stub_report_entry`/`python_vanished_report_answer`. Each names which fixture bytes put the declaration on that line (e.g. "A Go file opens with its `package` clause and carries a blank line under it, so the `func Exported` head... takes the third line"), not merely "this is 3". No shortfalls found.

    4. **No `.unwrap()`/`.expect(`/`panic!(` added.** `git diff HEAD -- <path> | grep '^+' | grep -E '\.unwrap\(|\.expect\(|panic!\('` — no matches.

    5. **No bare literal line number left anywhere in the file.** Swept with `grep -noE '"[a-zA-Z0-9_./]*:[0-9]+"'` and `grep -noE '"row":? ?\{?[0-9]+'` over the whole file — zero matches. The only remaining bare digit literals are the 4 macro bodies themselves (single source of truth, by design, matching the file's pre-existing `dart_broken_run_path!` pattern) and unrelated values (exit codes, fixture source bytes like `return 1`/`return 2`, `configVersion: 2`) that are not line numbers.

    Full verification:
    - `cargo fmt --check` — exit 0, clean
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, clean
    - `cargo nextest run --workspace` — 14137 tests run: 14137 passed, 0 failed, 0 skipped (36 slow, 1 leaky — not failures)
    - `cargo nextest run -p swissarmyhammer-validators shipped::missing_docs` — 55 passed, 0 failed

    evidence: cargo fmt --check (exit 0); cargo clippy --workspace --all-targets -- -D warnings (exit 0); cargo nextest run --workspace — 14137 passed, 0 failed, 0 skipped; cargo nextest run -p swissarmyhammer-validators shipped::missing_docs — 55 passed, 0 failed
    next: ready for review
  timestamp: 2026-08-17T04:53:00.638253+00:00
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

## Review Findings (2026-08-16 23:29)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 1 file(s) reviewed, 4 not reviewed.

> 4 file(s) not reviewed — excluded by an ignore rule:
> - `.kanban/ (from .reviewignore)` — 4 file(s)

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:1396` `magic-numbers/no-magic-numbers` — Hardcoded line number `1` in format string should be a named constant for clarity and reusability. Extract `1` to a named constant like `const EXPECTED_LINE_NUMBER: &str = "1"` and use it in the format calls.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:1459` `magic-numbers/no-magic-numbers` — Hardcoded line number `4` in JSON string should be a named constant to clarify what line number ruff reports findings for. Define a constant like `const JUDGED_FILE_FUNCTION_LINE: u32 = 4` and interpolate it into the JSON, or add a clearer comment explaining why line 4 is used.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:1498` `magic-numbers/no-magic-numbers` — Hardcoded line number `1` in format string should be a named constant for clarity and reusability. Extract `1` to a named constant like `const EXPECTED_LINE_NUMBER: &str = "1"` and use it in the format calls.
