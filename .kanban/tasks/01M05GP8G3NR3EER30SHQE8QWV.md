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
- actor: claude-code
  id: 01m05tx41qja4gq7fa8e8bygr5
  text: |-
    ### commit — changed
    - evidence: fe5149ddc fix(validators): read ruff stderr for missing-docs-python refusals (^hqe8qwv); 4 files changed
    - next: none
  timestamp: 2026-08-16T17:46:24.695983+00:00
- actor: claude-code
  id: 01m05vn2r03dz11cykg2preeyx
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (fe5149ddc) — engine 9 validators attempted, 0 findings, 0 confirmed, 0 refuted. No validator matches `*.md`, so the shipped rule file went unreviewed by the engine; hand verification against ruff 0.14.5 / jq 1.8.2 / BSD awk 20200816 found 5 — builtin/validators/code-hygiene/rules/missing-docs-python.md:22, :28, :37, :46, :358.
    - verified true: both tables reproduce byte for byte (exit codes and stderr strings exact); the `sah-diagnostic:` marker opens all four decline lines; the awk scan fails open (once per file, `head=""` matches no carve-out, every finding kept); the no-argument guard stops the tree walk; the gate fires for ruff at status 2, 101, SIGKILL and absent-from-PATH.
    - open: a mode-000 directory argument is silent at exit 0 (`warning: Encountered error:` head is unmatched); the added broken-run-gate sentence is false; `jq` under `set -e` discards a judged run on a malformed report at status <= 1; the stderr loop drops a final unterminated line; the awk fail-open has no acceptance test.
    - scope calls: removing the guard here rather than under ^z69hh5a is RIGHT — this card names the guard as item 2 and the guard lives in this file, while ^z69hh5a names three other rules. Carding `jq @tsv` as ^b2kq9hy is RIGHT — the doubled path is a separate defect on the finding rows and pinning it now would lock one of two candidate paths; the test that was skipped with it is a separate matter, recorded above.
    - next: clear the five findings, then re-review.
  timestamp: 2026-08-16T17:59:29.792792+00:00
- actor: claude-code
  id: 01m05vqfkprk1578y3dkn3n11n
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 2 files. Measured every shape first, watched RED on four new probes, removed the pre-flight guard, and read ruff's stderr instead.
    - test: green — cargo fmt --check exit 0; cargo clippy --workspace --all-targets -- -D warnings exit 0, 0 warnings; cargo nextest run --workspace 14110 passed, 0 failed, 0 skipped; embed traced. Behaviour re-run against real ruff for all four refusing shapes.
    - commit: fe5149ddc fix(validators): read ruff stderr for missing-docs-python refusals (^hqe8qwv) — 4 files changed
    - review: findings — 5 findings, all in builtin/validators/code-hygiene/rules/missing-docs-python.md at :22, :28, :37, :46, :358.

    ### THE IMPORTANT ONE — the fix introduced the defect it exists to remove

    `:22` — removing the pre-flight guard left a **mode-000 DIRECTORY argument** wholly unprotected. ruff answers that with `warning: Encountered error: Permission denied (os error 13)` — a head that does NOT match `warning: Failed to lint ` and that carries no path at all.

    Measured: the new script over `judged.py` plus that directory gives exit 0, the `D103` reported, and **0 bytes on stderr**. The old script gave exit 1 naming the directory. So an unread directory now reads as a fully judged clean tree — which is worse than the exit-1 defect the card was written to fix, and it is exactly the shape the guard used to catch.

    `:358` follows from it: the added sentence "The broken-run gate stands beside all of it, so a ruff that refuses its command line still never reads as a clean tree" is false as measured.

    ### The other three

    - `:37` — the rewritten `jq` runs bare under `set -e`. A malformed report at status ≤ 1 passes the `-gt 1` gate, jq then fails, and the script exits 5 with empty stdout and only jq's parse error. The card's own defect class, relocated from the deleted `exit 1` onto the `jq` step. `missing-docs-rust.md:47` and `function-length-rust.md:48` already carry the gate that stops this.
    - `:28` — `while IFS= read -r line` drops a final `warning: Failed to lint ` line that carries no trailing newline. Measured: 2 diagnostics with the newline, 1 without.
    - `:46` — the awk fail-open ships with no acceptance test.

    ### The review engine reviewed NONE of the shipped content

    Worth recording as a structural fact. The commit touched 4 files; 2 are `.kanban/` and excluded. Of the other two the engine reviewed only the Rust test file. It did NOT review `missing-docs-python.md` — `list validators` shows no validator whose `match_globs` include `*.md`, so the shipped script and its whole prose body fall outside every validator's reach.

    All five findings live in that unreviewed file. The engine's zero is real and says nothing about the shipped content. This is the same structural gap recorded on ^epe1z67.

    ### What measured TRUE

    Both tables reproduce row for row, with the three quoted stderr strings matching character for character under `od -c`. The marker opens each decline line at byte 0. The awk scan genuinely fails open — `getline` distinguishes `-1` from `0`, `head=""` matches neither carve-out regex so every row keeps its finding, and `scanned[file]=1` is unconditional so the diagnostic fires once per file. The broken-run gate fires for ruff at status 2, 101, SIGKILL and absent from PATH. The no-argument guard still stops the tree walk.

    And the guard removal IS a net improvement for six shapes — broken symlink, symlink loop, absent path, non-UTF-8, unreadable file, unreadable parent directory — each now declining at exit 0 with the neighbouring finding kept. The regression is the directory case alone.

    ### Both scope calls were RIGHT

    Removing the guard here rather than under ^z69hh5a is the card's own stated item 2, and ^z69hh5a names three other rules. Carding the `jq @tsv` backslash defect as ^b2kq9hy without a test is right for the PATH, since an assertion would pin one hand-off shape before the correct one is chosen — but the reviewer notes it does not cover the BEHAVIOUR, which a probe can hold without asserting the path. That gap is finding `:46`, not a fault in the carding decision.
    - next: iteration 2 works all five findings, starting with the directory regression
  timestamp: 2026-08-16T18:00:48.502823+00:00
- actor: claude-code
  id: 01m05x0v3jaa2ex4ds99bad7g8
  text: |
    Iteration 2. All five findings reproduced first, then answered. ruff 0.14.5, jq 1.8.2, BSD awk 20200816.

    ## Finding `:22` — the directory regression

    Reproduced word for word. `ruff check --isolated --no-cache --select D100,... --output-format json judged.py noreaddir` over a mode-000 directory: exit 1, one `D103` row on the report, and `warning: Encountered error: Permission denied (os error 13)` on stderr — 44 bytes, read back under `od -c`. The shipped script over the same pair: **exit 0, the `D103` reported, 0 bytes on stderr.**

    **Declined item, not broken run, and `builtin/validators/README.md` decides it.** ruff judged `judged.py` and reported its finding; it refused ONE argument of the run. The README: "A script that judged the code and could not judge ONE item says so on stderr, on a line that opens `sah-diagnostic:`, and it still exits 0." A broken run is the other paragraph — a tool that "judged nothing" — and the `[ "$status" -gt 1 ]` gate owns that. So the answer is a marked line at exit 0, and the finding stands.

    **No new head was added.** I surveyed what ruff can write on that channel instead:

    | the run | stderr | exit |
    |---|---|---|
    | clean file, documented module, `# noqa` file, byte-order-mark file, JSON-holding `.py`, the same file named twice | nothing | 0 or 1 |
    | absent path / broken symlink | `warning: Failed to lint <path>: No such file or directory (os error 2)` | 1 |
    | symlink loop | `warning: Failed to lint <path>: Too many levels of symbolic links (os error 62)` | 1 |
    | non-UTF-8 file | `warning: Failed to lint <path>: stream did not contain valid UTF-8` | 1 |
    | unreadable file / file under an unreadable parent | `warning: Failed to lint <path>: Permission denied (os error 13)` | 1 |
    | mode-000 DIRECTORY | `warning: Encountered error: Permission denied (os error 13)` | 1 |
    | directory holding no Python file, alone | `warning: No Python files found under the given path(s)` | 0 |
    | removed selector `UP027` | `ruff failed` + `Cause: Rule ...` | 2 |

    THREE heads, and one of them names no path. A sound run writes 0 bytes there. So the script now reads EVERY line of `$work/ruff.err` and forwards it whole under the marker:

        sah-diagnostic: ruff declined an item and said: <ruff's own line>

    No head is stripped and none is enumerated, so a ruff release that writes a head this rule never met still says its piece. The `Failed to lint ` lines keep carrying their path, so `verify_declined_item_is_stated` still matches on the path for the three older probes.

    RED first: `..._declines_a_directory_it_may_not_read` failed with `the run must state the one item it declined; it stated []`, left 0, right 1.

    ## Finding `:37` — the jq step

    Reproduced: a stub ruff at exit 1 writing `[\n  {\n    "code": "D103"` gave **exit 5, 0 bytes on stdout, and `jq: parse error: Unfinished JSON term at EOF at line 3, column 18` alone**. Fixed with the shape `missing-docs-rust.md` and `function-length-rust.md` carry: `filtered=0`, each `jq` into a FILE with `|| filtered=$?`, then one gate that writes `missing-docs-python: jq could not read the ruff report` and exits 1. The first `jq` had to move off `>&2` into `$work/unmeasured.txt` so the gate stands ahead of any output; the file is `cat`-ed to stderr after the gate.

    Measured after: the same stub → exit 1, 0 bytes on stdout, jq's own message plus the named line. A stub writing `{ not json` at status 0 reads the same, and a `jq` that exits 127 reads the same.

    RED first: `..._breaks_on_a_report_the_filter_cannot_read` failed with `the run must break with 'missing-docs-python: jq could not read the ruff report'; got 'the run script exited nonzero: jq: parse error: Unfinished JSON term at EOF at line 3, column 18'`.

    ## Finding `:28` — the unterminated last line

    Reproduced exactly: a stub ruff writing two `warning: Failed to lint ` lines gave **2 diagnostics with the closing newline and 1 without it**. Fixed with `while IFS= read -r line || [ -n "$line" ]`. Measured after: 2 either way.

    RED first: `..._reads_a_decline_with_no_closing_newline` failed with left `[]`, right the one expected diagnostic.

    ## Finding `:46` — the fail-open now has a test

    `..._keeps_the_findings_its_scan_cannot_carve` stages `back\slash.py` beside `judged.py` and holds the run to exit 0, to the row of `judged.py` standing, to a count of THREE findings, and to one diagnostic containing the fragment `slash.py`. No assertion names the doubled path, so `^b2kq9hy` is still free to pick either spelling.

    The probe passed on first run, because the fail-open itself was already correct and the review verified it. So I mutation-tested it: putting the old `exit 1` back inside the awk scan made it fail with `a scan that cannot read one file must keep every finding and exit 0: Exit("missing-docs-python cannot read .../back\\slash.py")`. The fail-open was then restored.

    ## Finding `:358` — the false sentence

    Replaced. The section now states the two gates by what they do and what they do NOT reach: "Neither one reaches a ruff that REFUSED one argument and judged the rest, because that run keeps status 1 and writes a readable report. The stderr channel is what answers that shape, and it answers every head ruff writes there." The counts in that section moved from four marked lines to five, measured over the probe with the directory added.

    ## Re-probe of every refusing shape, with the fixed script

    | the run | findings | marked lines | exit |
    |---|---|---|---|
    | `judged.py` alone | 1 | 0 | 0 |
    | + unparsable | 1 | 1 | 0 |
    | + absent path | 1 | 1 | 0 |
    | + non-UTF-8 | 1 | 1 | 0 |
    | + unreadable file | 1 | 1 | 0 |
    | + broken symlink | 1 | 1 | 0 |
    | + symlink loop | 1 | 1 | 0 |
    | + file under an unreadable parent | 1 | 1 | 0 |
    | + mode-000 DIRECTORY | 1 | 1 | 0 |
    | + `back\slash.py` | 3 | 1 | 0 |
    | judged + unparsable + all 3 paths + the directory | 1 | 5 | 0 |
    | no argument | 0 | 0 | 0 |
    | stub ruff at 2 / 101 / 127 | 0 | — | 1, named |
    | malformed report at ruff 0 / 1, or jq at 127 | 0 | — | 1, named |

    The six shapes the review verified correct — broken symlink, symlink loop, absent path, non-UTF-8, unreadable file, unreadable parent directory — all still decline at exit 0 with the neighbouring finding kept. `TMPDIR` entry count 254250 before and 254250 after the whole probe run, so the trap still covers every exit.

    ## Harness work this needed

    `ShippedStaging` gained a `restore` hook, called after the run and before the temporary tree drops. A mode-000 DIRECTORY stops `remove_dir_all`, so without it `tempfile::TempDir` would leave the whole probe tree on disk on every run of that test. Confirmed after the suite: `find $TMPDIR -maxdepth 3 -name unread.py` returns nothing. `forbid_probe_directory` / `restore_probe_directory` stand beside `forbid_probe_read`, and `drive_shipped_script_with_stub` is the `files`-scope counterpart of `verify_shipped_tree_breaks_with_stub`.

    ## One thing outside the five, and why it is not churn

    `builtin/validators/README.md` states "`rules/missing-docs-python.md` ... its script is 51 lines". That was true before `fe5149ddc`, which moved it to 49 without updating the sentence; this change moves it to 52. Corrected to 52, because leaving a number my own edit made wronger is not an option. Nothing else in that file was touched.

    `function-length-python.md` reads the SAME single `Failed to lint ` head and would answer a mode-000 directory the same silent way. It is a different rule and a different card, so it was left alone rather than absorbed here.
  timestamp: 2026-08-16T18:23:23.762772+00:00
- actor: claude-code
  id: 01m05x164rjfy457wwksfbcd8h
  text: |
    ### implement — changed
    - evidence: 4 files — `builtin/validators/code-hygiene/rules/missing-docs-python.md`, `builtin/validators/README.md`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs`. All five findings reproduced first; RED watched on 3 of the 4 new probes (`the run must state the one item it declined; it stated []` 0 vs 1 · `the run must break with 'missing-docs-python: jq could not read the ruff report'; got 'the run script exited nonzero: jq: parse error: Unfinished JSON term at EOF at line 3, column 18'` · left `[]` right the one expected decline), and the fourth (`..._keeps_the_findings_its_scan_cannot_carve`) mutation-tested RED by restoring the old awk `exit 1`. Gates: `cargo fmt` and `cargo fmt --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0, 0 warnings; `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` 3529 run, 3529 passed, 0 failed, 0 skipped (was 3525, +4 new probes, all PASS); `cargo build -p mirdan` exit 0 with `target/debug/build/mirdan-2dd4355981eab855/out/builtin_validators.rs` holding the rule file byte for byte (24693 bytes), the run block at 52 lines with exactly two `exit 1` — the ruff-status gate and the jq gate — and no `[ ! -r "$file" ]` inside it.
    - next: `/review`.
  timestamp: 2026-08-16T18:23:35.064663+00:00
- actor: claude-code
  id: 01m05xrdvtwg3xex27qxea5529
  text: |-
    ### test — green

    Gates:
    - `cargo fmt --check` — exit 0, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
    - `cargo nextest run --workspace` — 14114 tests run: 14114 passed, 0 failed, 0 skipped. All 11 `missing_docs::the_shipped_python_missing_docs_tool_rule_*` probes present and PASS (verified by name in the stored run output, not by count alone).
    - `cargo build -p mirdan` — succeeds. Traced OUT_DIR to `target/debug/build/mirdan-2dd4355981eab855/out` via `cargo build -p mirdan -vv` after touching the source file to force a rebuild. Extracted the embedded `("code-hygiene/rules/missing-docs-python.md", r#"..."#)` entry from `out/builtin_validators.rs` and diffed it byte-for-byte against the on-disk rule file: identical, 24657 bytes both sides.

    Behaviour checks, run against real ruff 0.14.5 / jq 1.8.2 / BSD awk 20200816 (all match the versions the rule cites):

    - Sound-run stderr probe (the risky change): extracted the shipped 52-line `run:` block to a standalone script and ran it directly. Every sound shape wrote 0 bytes to stderr at exit 0: a clean documented file alone, a file with one `D103` finding, a `# noqa: D103` suppression, a file opening with a UTF-8 BOM, a file holding valid-Python JSON content (`D100` only), and the same file named twice.
    - Refusing shapes, each run beside a judged file with one live `D103` finding, confirmed the finding survived and each decline landed under `sah-diagnostic:` at exit 0: unparseable file (`invalid-syntax` row forwarded), absent path (`Failed to lint ... No such file or directory`), non-UTF-8 file (`stream did not contain valid UTF-8`), unreadable file mode 000 (`Permission denied`), broken symlink (`No such file or directory`), and a mode-000 directory (`Encountered error: Permission denied`, no path — the `Encountered error` head the rule calls out separately from `Failed to lint`).
    - Broken-run gate: stubbed `ruff` to exit 2 and to exit 101 — both exited the script 1 with the stub's own stderr plus `missing-docs-python: ruff exited <status> and judged no code`.
    - Both `jq` gates: stubbed `ruff` to exit 1 with a truncated JSON report (`printf '[\n  {\n    "code": "D103"'; exit 1`) — script exited 1, 0 bytes on stdout, and stderr carried the rule's own `missing-docs-python: jq could not read the ruff report` (not jq's bare parse-error status alone).
    - No-argument guard: reproduced the exact fixture the acceptance test uses (`top.py` = `def top():\n    return 1\n`, `deep/nested/other.py` = `class Other:\n    def method(self):\n        return 2\n`). No-argument run: exit 0, 0 findings. Same two files named explicitly: exit 0, exactly 5 findings — matches the rule prose's own "5 findings" claim word for word.
    - TMPDIR leak check: ran `cargo nextest run -p swissarmyhammer-validators the_shipped_python_missing_docs_tool_rule_declines_a_directory_it_may_not_read` alone (exercises the `ShippedStaging.restore` hook against the mode-000 directory). Counted entries in `$TMPDIR` before and after: 255006 both times — no leak.
    - README line-count claim: `awk` over the `run: |` block counts 52 lines — matches the "its script is 52 lines" sentence exactly.

    `git status --porcelain` on `builtin/validators` and `crates/swissarmyhammer-validators` still shows only the four intended files, unchanged by probing.

    No commit made. Task left in `doing`.
  timestamp: 2026-08-16T18:36:16.634920+00:00
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

## Review Findings (2026-08-16 12:58)

> Scope: `review sha HEAD~1..HEAD` (fe5149ddc) — the diffs only. The engine fleet
> attempted 9 validators and returned 0 findings, but NO validator matches `*.md`,
> so `builtin/validators/code-hygiene/rules/missing-docs-python.md` — the shipped
> script and its whole prose body — went unreviewed by the engine. Every item
> below was measured by hand against ruff 0.14.5, jq 1.8.2 and BSD awk 20200816.
> Both tables of the rule body reproduce byte for byte, the `sah-diagnostic:`
> marker opens each of the four decline lines, the awk scan does fail open, and
> the no-argument guard does stop the tree walk.

- [x] `builtin/validators/code-hygiene/rules/missing-docs-python.md:22` `code-hygiene/missing-docs-python` — removing the pre-flight guard leaves a directory argument with no read permission wholly unprotected. ruff answers that shape with `warning: Encountered error: Permission denied (os error 13)`, which does not open with `warning: Failed to lint ` and carries no path, so the new stderr scan matches nothing. Measured over `judged.py` beside a mode-000 directory: the shipped script exits 0, reports the `D103`, and writes 0 bytes to stderr — the unread directory reads as a fully judged clean tree; the script before this change exited 1 and named the path. Read the `warning: Encountered error` head as well, so no refusal ruff states goes unsaid.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-python.md:358` `code-hygiene/missing-docs-python` — the added sentence "The broken-run gate stands beside all of it, so a ruff that refuses its command line still never reads as a clean tree." is false as measured. ruff refused a mode-000 directory at exit 0, the gate `[ "$status" -gt 1 ]` never fired, and the tree read as clean with nothing on stderr. Restate the sentence against what the run does, and state the `Encountered error` head the body never mentions.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-python.md:37` `code-hygiene/missing-docs-python` — the rewritten `jq` runs bare under `set -e`, so a report ruff writes malformed at exit 0 or 1 passes the broken-run gate and then kills the script, discarding every finding the run did make. Measured with a stub ruff at exit 1 writing truncated JSON: the script exits 5, stdout is empty, and stderr carries only `jq: parse error: Unfinished JSON term at EOF at line 1, column 82` — no `sah-diagnostic:`, no named framing. This is the defect class this card exists to remove, moved off the deleted `exit 1` and onto the `jq` step. `builtin/validators/code-hygiene/rules/missing-docs-rust.md:47` and `builtin/validators/code-hygiene/rules/function-length-rust.md:48` hold the worked answer — gate the filter and write `missing-docs-python: jq could not read the ruff report`.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-python.md:28` `code-hygiene/missing-docs-python` — the stderr read loop drops a final `warning: Failed to lint ` line that carries no trailing newline, because `while IFS= read -r line` returns nonzero on a partial last line and the loop body never runs for it. Measured over two decline lines: with the trailing newline the loop states 2 diagnostics, without it 1 — the last decline lost, and that path then reads as clean. Read the trailing partial line as well.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-python.md:46` `code-hygiene/missing-docs-python` — the awk scan's fail-open ships with no acceptance test. The commit states the shape was left untested so as not to lock a wrong path into an assertion, but `verify_declined_item_is_stated` holds the diagnostic with `stated[0].contains(declined)`, so a probe can hold exit 0, the surviving findings, and one diagnostic naming a FRAGMENT of the file name without asserting the doubled path `^b2kq9hy` will change. Add that probe to `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs`.