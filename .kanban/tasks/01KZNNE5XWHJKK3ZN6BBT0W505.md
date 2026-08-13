---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzs7f2v588sftmgzz8ws0s9h
  text: |-
    Measurement first. The card had one wrong premise, and the rule body carried it: "gocognit has no suppression comment of its own". `gocognit v1.2.1` reads `//gocognit:ignore`. The description now holds each spelling that works and each spelling that does not.

    Two other premises hold. `-test` does not reach a named path, and `-ignore` reads the path and never the `DO NOT EDIT` header.

    One fact the card did not state, and it decided the design: `-ignore` DOES apply to a named path argument. So `-ignore '_test\.go$'` would silence a `_test.go` file the script names. The rule refuses it. Over one `_test.go` file that holds a `TestTable` function and a `buildRequest` helper, each scoring 18: the run reports both, and `-ignore '_test\.go$'` silences both. The prompt rule keeps the helper, so that expression trades a true finding for the carve-out. This is the trade `magic-numbers-go` refuses for a shift operand.

    An approach that was rejected: a `jq select` on the function name, to drop `Test`/`Benchmark`/`Fuzz`/`Example`. `builtin/validators/README.md` states "Selection in the pipe is attribution, not exemption... To exempt one code item, use an inline suppression in the code — never the pipe." So the pipe cannot carry the test carve-out.
  timestamp: 2026-08-11T20:15:48.581222+00:00
- actor: claude-code
  id: 01kzs7fgesv2vtdkvc3sg7e7an
  text: |-
    ### implement — changed
    - evidence: 2 files.
      - `builtin/validators/code-hygiene/rules/complexity-go.md` — the `run` script now drops a file whose head carries `^// Code generated .* DO NOT EDIT\.$` above the `package` clause, and it counts its arguments a second time under that test. `check_command` names `sed` and `grep`, which the script runs. The body gained "The annotation an author writes" and "The two carve-outs the superseded prompt rule states", and the false statement about the suppression comment is gone.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity.rs` — two acceptance tests.
    - RED to GREEN, `the_shipped_go_complexity_tool_rule_skips_a_generated_file`: with the head test removed from the shipped `run`, the run reported `["marked/staged.go", "plain/staged.go"]` against the expected `["plain/staged.go"]` and the test FAILED. With the shipped script it PASSES. The two positions hold the same declarations, so the header is the one difference between them.
    - RED to GREEN, `the_shipped_go_complexity_tool_rule_reports_a_test_file`: with `-ignore '_test\.go$'` added to the shipped `run`, the run reported `[]` against the expected `["suite/staged_test.go"]` and the test FAILED. With the shipped script it PASSES. That test holds the rule to stating no path filter.
    - The same pair at the shell, over three files each holding one function of score 18: the old script reported `gen.pb.go` and `plain.go`; the new script reports `plain.go` alone; the new script over `gen.pb.go` alone reports nothing and exits 0; the new script with no argument reports nothing and exits 0.
    - `cargo nextest run -p swissarmyhammer-validators`: 646 tests run, 646 passed, 0 skipped. `cargo nextest run -p mirdan`: 503 passed. `cargo fmt --all` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean.
    - `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` ran before each test run, so every run read the shipped bytes.
    - next: `/review`.
  timestamp: 2026-08-11T20:16:02.521494+00:00
- actor: claude-code
  id: 01kzs8pysxzt8wvjskekw4p38k
  text: |-
    ### review — findings
    - scope: `review sha HEAD~1..HEAD` (commit aab7517e4), task-mode.
    - engine: 8 validators attempted, 0 findings, 0 confirmed, 1 refuted, 0 failed, 0 skipped.
    - Directed check of every measured number in the rule body, against the real `gocognit v1.2.1` binary. Two independent measurement passes agree.
    - 3 open findings, all on `builtin/validators/code-hygiene/rules/complexity-go.md`: line 184 (the usage-text count is 39 in the body and 52 in the tool), line 13 (the first argument count is dead), line 186 (two sentences give the first count work it does not do).
    - CONFIRMED by measurement, no change asked: each of the nine `//gocognit:ignore` spellings; `-ignore 'DO NOT EDIT'` drops none of three; `-ignore '\.pb\.go$'` drops the file whose name ends that way; the `-test` walk results and `-test=false` over a named path; `-ignore '_test\.go$'` silences both `TestTable` and `buildRequest`; `-json` prints `null\n`; exit 1 with a finding, exit 0 without, exit 0 through the `jq` pipe; the `for`/`if`/`if`-`else if`-`else` function scores 8; the flat 260-line function scores 0.
    - CONFIRMED, the Go standard library numbers reproduce exactly at GOROOT `/opt/homebrew/Cellar/go/1.26.5/libexec`: 4350 files, 2731 over the gate of 15, 29580 over 0, and 356 at 13, 282 at 14, 256 at 15, 232 at 16.
    - CONFIRMED, the generated-file carve-out agrees with the siblings. Over nine files, the default golangci-lint `linters.exclusions.generated` and the new `sed`/`grep` test drop the same set: both drop the strict header and the header under a `//go:build` line; both keep a header with no `DO NOT EDIT.`, a lower-case header, a block-comment header, a header with trailing space, and a plain file.
    - next: correct the three findings, then `/review ^bt0w505` again.
  timestamp: 2026-08-11T20:37:35.165013+00:00
- actor: claude-code
  id: 01kzs8r1fe6fe1v4gw6758pz71
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 2 files; the run drops a Go generated-code header, and the rule states why a test function still reports
    - test: green for this change — cargo nextest -E 'rdeps(swissarmyhammer-validators)' 3409 run, 3405 passed; the 4 failures reproduce with this change stashed out, so they came before it
    - commit: aab7517e4
    - review: findings — builtin/validators/code-hygiene/rules/complexity-go.md:13, :184, :186
  timestamp: 2026-08-11T20:38:10.670779+00:00
- actor: claude-code
  id: 01kzs9cae2100wdh4n2yv7x9fg
  text: |-
    Measurement decided the shape of the fix, and the set-wide guard decided against deletion.

    1. The usage-text count. `gocognit v1.2.1`, given no path: 52 lines on stderr, 0 bytes on stdout, exit 2. Three runs each gave 52 lines and 1421 bytes. The body said 39 lines and "exits nonzero".

    2. Deletion of the first count. I made the deletion, ran `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`, and ran the guard. RED: `each_shipped_files_scope_script_answers_a_run_that_gives_it_no_file` FAILED with `["complexity-go"]` and the message "must stand together in each `files`-scope script, above every line that runs". So the first count cannot go away while the script keeps a second count under the generated-file test. The guard is right.

    3. The reshape. The script now writes one guard at its head, then `for file in "$@"`, and it hands `gocognit` one file at a time inside that loop. `done |` carries the whole loop into the same `jq`. The `while`/`shift`/`set --` list rebuild and the second count are gone. Because the tool takes exactly one path on each call, no run can give it an empty argument list, so one count is enough and it stands where the contract puts it.

    4. The four runs, over one ordinary Go file and one generated Go file, each holding one function of cognitive complexity 21. The script under test is byte-identical to the `run` block the rule now ships (`diff` clean).

    | run | stdout | stderr | exit |
    |---|---|---|---|
    | no argument | 0 bytes | 0 lines | 0 |
    | plain.go | 1 finding, 103 bytes | 0 lines | 0 |
    | gen.pb.go alone | 0 bytes | 0 lines | 0 |
    | plain.go gen.pb.go | 1 finding, the plain file | 0 lines | 0 |

    5. The two shapes that prove which count did the work, over the same four runs. The old script with the FIRST count removed gave the same stdout, the same stderr and the same exit status on each of the four. The old script with the SECOND count removed wrote 52 lines of usage text to stderr on the run over the generated file alone.

    6. One measured difference the reshape makes: the loop reports in the order of its arguments, and the one call over every file reported in sorted order. Over two ordinary files both shapes report the same 2 findings. The acceptance tests sort before they compare, and they pass.

    7. The same wrong count and the same mis-credit stood in the doc comment of `the_shipped_go_complexity_tool_rule_reads_only_the_files_it_is_given`. Both are corrected there.
  timestamp: 2026-08-11T20:49:15.202306+00:00
- actor: claude-code
  id: 01kzs9cq9f7tgr675q03c033jq
  text: |-
    ### implement — changed
    - evidence: 2 files.
      - `builtin/validators/code-hygiene/rules/complexity-go.md` — the `run` script keeps one zero-argument guard at its head and loops over `"$@"`, giving `gocognit` one file at a time; the `while`/`shift`/`set --` rebuild and the second argument count are gone. The section "The run answers for its own arguments" is rewritten to the measured numbers: 52 lines of usage text, exit 2, and the four runs with their stdout, stderr and exit code.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity.rs` — the doc comment of `the_shipped_go_complexity_tool_rule_reads_only_the_files_it_is_given` carried the same 39-line count and the same mis-credit; both are corrected.
    - All 3 findings of the newest `## Review Findings` section are `- [x]`.
    - RED that decided the design: with the head guard deleted, `each_shipped_files_scope_script_answers_a_run_that_gives_it_no_file` FAILED and named `complexity-go`.
    - `cargo nextest run -p swissarmyhammer-validators`: 646 tests run, 646 passed, 0 skipped.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'`: 3409 run, 3405 passed, 4 failed. The 4 are the known failures of this tree — 3 `review_e2e` tests and `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport`.
    - `cargo fmt --all` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean.
    - `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` ran before each test run, so every run read the shipped bytes.
    - next: `/review`.
  timestamp: 2026-08-11T20:49:28.367577+00:00
- actor: claude-code
  id: 01kzs9zbgz8a5f29ctaagrwwde
  text: |-
    ### review — findings
    - scope: `review sha HEAD~1..HEAD` (commit 6c2e88566), task-mode. HEAD is 6c2e88566.
    - engine: 8 validators attempted, 0 findings, 0 confirmed, 0 refuted, 0 failed, 0 skipped.
    - Directed check of the reshaped loop against the real `gocognit v1.2.1` at `/Users/wballard/.local/bin/gocognit`. Both scripts under test were cut from the YAML `run` blocks of this commit and of its parent, and `diff` shows each is byte-identical to its rule.
    - 3 open findings, all on `builtin/validators/code-hygiene/rules/complexity-go.md`: line 212 (the sort key is complexity and not the path), line 174 (the loop changed the result of a run that holds a bad file, and the section states nothing), line 195 (a file the tool cannot read reaches the engine as a clean file).
    - CONFIRMED, the head claim. `gocognit` with no path: 52 lines on stderr, 1421 bytes, 0 bytes on stdout, exit 2. Three runs gave the same md5 `91c1f395e847335a1c2402e46db0b1a1`.
    - CONFIRMED, the four runs of the section reproduce exactly, over one ordinary file and one generated file each holding one function of complexity 21: no argument gave nothing and exit 0; the ordinary file gave 1 finding, 0 stderr lines, exit 0; the generated file alone gave nothing, 0 stderr lines, exit 0; the two files together gave the one finding of the ordinary file, 0 stderr lines, exit 0.
    - CONFIRMED, "both shapes report the same 2 findings" over two ordinary files of complexity 21 and 18. The set and the order agree.
    - CONFIRMED, the head guard. `builtin/validators/README.md` states the three lines and states "Write them above every line that runs." The shipped script writes them on its first three lines. `each_shipped_files_scope_script_answers_a_run_that_gives_it_no_file` reads the text and the place, and it passes.
    - CONFIRMED, no count changed. `-top` has no capping default: over 12 files, each holding one function of complexity 16 to 27, the one call reported 12 and the loop reported 12. An explicit `-top 3` reported 3, so the flag works and its default does not cap.
    - CONFIRMED, a path that holds a space. Over `with space.go`, and over that file beside a second file, both shapes reported the correct finding, 0 stderr lines, exit 0. The quoting is correct.
    - CONFIRMED, a file with no `package` clause. `sed` prints the whole file, so a file that carries the generated header and no `package` clause is still dropped: nothing on stdout, nothing on stderr, exit 0.
    - CONFIRMED, `-json` prints `null\n`, 5 bytes, when nothing is over the gate.
    - CONFIRMED, the shipped bytes are green. `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`, then `cargo nextest run -p swissarmyhammer-validators shipped::complexity`: 18 tests run, 18 passed, 0 skipped.
    - next: correct the three findings, then `/review ^bt0w505` again.
  timestamp: 2026-08-11T20:59:38.911931+00:00
- actor: claude-code
  id: 01kzsa05fahpqvss0z9yjcaawy
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 2 files; the three prior findings are closed, the run script is reshaped to one head guard plus a per-file loop
    - test: green for this change — cargo nextest -E 'rdeps(swissarmyhammer-validators)' 3409 run, 3405 passed; only the 4 known failures that came before
    - commit: 6c2e88566
    - review: findings — builtin/validators/code-hygiene/rules/complexity-go.md:174, :195, :212 (three new findings; the three of iteration 1 are confirmed closed)
  timestamp: 2026-08-11T21:00:05.482895+00:00
- actor: claude-code
  id: 01kzsazyw433whgpnxs9wmj1na
  text: |-
    Measurement decided the shape of the fix for the third finding, and it is a defect in the script and not in the prose alone.

    1. The shared status. `gocognit v1.2.1`, one file at each run: a file with a function over the gate gave exit 1, a JSON array of one entry on stdout and 0 lines on stderr; a file with none gave exit 0 and `null`; a path that holds no file gave exit 1, 0 bytes on stdout and `gocognit: open ...: no such file or directory`; a file with a syntax error gave exit 1, 0 bytes and `gocognit: ...:5:12: expected '}', found 'EOF'`; an empty `.go` file gave exit 1, 0 bytes and `gocognit: ...:1:1: expected 'package', found 'EOF'`. So exit 1 alone cannot tell a finding from a failure.

    2. The test the README states for a shared status. `builtin/validators/README.md` says to test the REPORT beside the status and to accept the shared status only for the report shape a measured run writes. The script calls a run measured for exit 0 with `null`, and for exit 1 with a JSON array of one entry or more. Every other answer names the file on stderr and exits 1.

    3. The two causes are separate, so the script answers each one. A path the script cannot read never reaches `gocognit`: `[ ! -r "$file" ]` names it and exits 1, which also keeps the `sed` error of that path off stderr. A file that is readable and does not parse reaches `gocognit` and fails the report test.

    4. The findings are held in a variable and written at the end. Measured with the streaming shape first: over one ordinary file of complexity 21 beside a missing path, the script wrote 1 finding to stdout and then exited 1. The engine drops a finding at a nonzero exit, so the result was the same, but the accumulating shape writes NO finding for a broken run whatever the order of the arguments. That is the shape that shipped.

    5. The delta table, measured over three kinds of bad file and in both argument orders (bad file first, bad file last). Each run holds one ordinary Go file of complexity 21 beside one bad file.

    | the second file | one call | loop | this script |
    |---|---|---|---|
    | a path that holds no file | 0 findings, exit 0 | 1 finding, exit 0 | 0 findings, exit 1 |
    | a syntax error | 0 findings, exit 0 | 1 finding, exit 0 | 0 findings, exit 1 |
    | an empty `.go` file | 0 findings, exit 0 | 1 finding, exit 0 | 0 findings, exit 1 |

    The cause of the one-call column: `gocognit` writes one report for the whole run and writes 0 bytes when one file breaks. Measured raw, in each of the six orders: 0 bytes on stdout.

    6. The sort key. Over three files of complexity 28 (`zzz.go`), 36 (`mmm.go`) and 21 (`aaa.go`), given in that argument order, the one call reported `mmm.go` 36, `zzz.go` 28, `aaa.go` 21. That is the complexity, highest first. The loop reported `zzz.go`, `mmm.go`, `aaa.go`, which is the argument order.

    7. `set -e` is new at the head of the script. It stands above the zero-argument guard, which the contract allows, and the coverage guard passes. The doc comment of `each_shipped_files_scope_script_answers_a_run_that_gives_it_no_file` counted 11 rules with the guard on the first line and 5 under `set -e`; the count is now 10 and 6, and the comment is corrected.

    8. No temporary directory. The accumulating variable takes the place of a report file, so `TEMP_DIRECTORY_RULE_COUNT` stays at 16 and `check_command` names no `mktemp`.
  timestamp: 2026-08-11T21:17:27.300461+00:00
- actor: claude-code
  id: 01kzsb0d3c1j2stp99zpscha5a
  text: |-
    ### implement — changed
    - evidence: 3 files.
      - `builtin/validators/code-hygiene/rules/complexity-go.md` — the `run` script gains `set -e`, a `[ ! -r "$file" ]` test, a status-and-report test for each `gocognit` call, and an accumulating `findings` variable that writes at the end. The body gains the section "A file the tool cannot read", it states the measured sort key of the one call, and it states the delta of a run that holds a bad file.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity.rs` — two acceptance tests, `the_shipped_go_complexity_tool_rule_breaks_on_a_file_it_cannot_read` and `..._breaks_on_a_file_it_cannot_parse`. The doc comment of `the_shipped_go_complexity_tool_rule_reads_only_the_files_it_is_given` no longer states a pipe that the script does not write.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/zero_argument.rs` — the measured split of the 16 files-scope scripts is now 10 with the guard first and 6 under `set -e`.
    - All 3 findings of the newest `## Review Findings` section are `- [x]`.
    - RED to GREEN: `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`, then the rule file put back to its committed bytes. Both new tests FAILED with "the run must report exactly one tool error; got []" — the broken file passed as clean. With the new script both PASS.
    - The `run` block of the rule is byte-identical to the script each measurement drove (`diff` clean, twice).
    - `cargo nextest run -p swissarmyhammer-validators`: 648 tests run, 648 passed, 0 skipped. The count was 646 before the two new tests.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'`: 3411 run, 3407 passed, 4 failed. The 4 are the known failures of this tree — 3 `review_e2e` tests and `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport`.
    - `cargo fmt --all` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean.
    - `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` ran before each test run, so every run read the shipped bytes.
    - next: `/review`.
  timestamp: 2026-08-11T21:17:41.868565+00:00
- actor: claude-code
  id: 01kzsbrkg6h8pvszx1fk3eg5ds
  text: |-
    ### review — clean
    - scope: `review sha HEAD~1..HEAD` (commit f8f0fcbe5), task-mode.
    - engine: 8 validators attempted, 0 findings, 0 confirmed, 1 refuted, 0 failed, 0 skipped.
    - All 6 prior findings, of round 1 and round 2, are `- [x]`. No new finding. The task moves to `done`.
    - The script under test was cut from the `run` block of this commit. The two earlier shapes were cut from `HEAD~1` and `HEAD~2`. The engine runs each `run` block with **bash**, and it gives every matched file as one argument list (`run_tool_script` in `crates/swissarmyhammer-validators/src/review/tool_rules.rs`). Each measurement below ran under bash.

    CONFIRMED, the shared exit status. `gocognit v1.2.1`, one file at each run: a function over the gate gave exit 1 with a JSON array; a clean file gave exit 0 with `null` (4 bytes); a missing path gave exit 1 with 0 bytes; a syntax error gave exit 1 with 0 bytes; an empty `.go` file gave exit 1 with 0 bytes. A file with no read permission gave exit 1 with `permission denied`. A broken symbolic link gave exit 1 with `no such file or directory`.

    CONFIRMED, the status-and-report test is complete for `-over 15`. `gocognit -over 15 -json` gave exit 1 for a report that is an array and exit 0 for `null`. No shape gave exit 0 with an array. Measured over five clean shapes — a file with the `package` clause alone, a file with a doc comment and the `package` clause, a file under `//go:build ignore`, a file of declarations with no function, and a file with one empty function: each gave exit 0 with `null`, and the script gave exit 0 with 0 bytes and nothing on stderr. So the test breaks no clean file.

    CONFIRMED, the position of the bad file changes nothing. Over one ordinary file of complexity 21 beside one bad file, with the bad file FIRST and with the bad file LAST, and over three kinds of bad file — a missing path, a syntax error, an empty `.go` file — the script gave exit 1 with 0 findings in each of the six runs. The accumulate-then-write shape holds.

    CONFIRMED, the delta table of the rule body, all three rows and both orders:

    | the second file | one call (HEAD~2) | loop (HEAD~1) | this script |
    |---|---|---|---|
    | a path that holds no file | 0 findings, exit 0 | 1 finding, exit 0 | 0 findings, exit 1 |
    | a syntax error | 0 findings, exit 0 | 1 finding, exit 0 | 0 findings, exit 1 |
    | an empty `.go` file | 0 findings, exit 0 | 1 finding, exit 0 | 0 findings, exit 1 |

    CONFIRMED, the cause of the one-call column. `gocognit -over 15 -json` over the ordinary file beside the bad file wrote 0 bytes to stdout in each of the six orders.

    CONFIRMED, `[ ! -r "$file" ]` stops the path before the tool. Over a missing path, a broken symbolic link and a file with mode 000, the script wrote one line to stderr — `complexity-go: gocognit could not read <path>` — and exited 1. No `sed` error and no `gocognit` error reached stderr on those three runs.

    CONFIRMED, a path that holds a space and a path that holds a newline. Each gave exit 0 and one correct finding. The newline path came out as ONE line of stdout, because `jq -c` writes the newline as `\n` inside the JSON string.

    CONFIRMED, a large finding set. One file with 400 functions over the gate gave 400 findings, 41673 bytes, exit 0, nothing on stderr, and 400 lines of stdout with the last line whole. `printf` is a bash builtin, so the accumulator meets no argument-length limit. The same file beside an ordinary file gave 401. The same file beside a missing path gave exit 1 and 0 findings.

    CONFIRMED, a directory as an argument. `gocognit` walks it and reports the file inside it, so the script gave exit 0 and one finding named `dir.go/inner.go`. `sed` over a directory exited 0 and wrote nothing on this platform, so no error reached stderr. The engine gives this rule the changed FILES alone, so no run reaches this shape.

    CONFIRMED, `set -e` exits no run early. The four runs of the rule body gave exit 0: no argument, the ordinary file, the generated file alone, and the two files together. Each answer that must break is written with an explicit `exit 1`. The `|| status=$?` list and the two `if` conditions each suppress `errexit`, so the status the script means to test reaches the test.

    CONFIRMED, the guard stands where the contract puts it. `builtin/validators/README.md` states "Write them above every line that runs. Only a comment, a blank line and [a `set` line] may stand above them." `set -e` is such a line, so the sentence of the rule body holds.

    CONFIRMED, the sort key of the prose. Over `zzz.go` (28), `mmm.go` (36), `aaa.go` (21), given in that argument order, the one call reported `mmm.go`, `zzz.go`, `aaa.go` — the complexity, highest first. The loop reported `zzz.go`, `mmm.go`, `aaa.go` — the argument order. Over two ordinary files, both shapes reported the same 2 findings.

    CONFIRMED, the head claim. `gocognit` with no path: 52 lines on stderr, 1421 bytes, 0 bytes on stdout, exit 2. Three runs gave the same md5 `91c1f395e847335a1c2402e46db0b1a1`.

    CONFIRMED, the `zero_argument.rs` doc-comment count. Counted over the 16 shipped `files`-scope rules: 10 write the guard on the first line, and 6 write it under `set -e`. The 6 are `complexity-go`, `complexity-swift`, `magic-numbers-swift`, `missing-docs-go`, `missing-docs-python` and `missing-docs-swift`. The count 10/6 is exact. The guard logic did not change.

    CONFIRMED, the two new acceptance tests drive the real pipeline. `verify_shipped_run_breaks` holds the run to no finding and to exactly one tool error, and it holds the error text to the path.

    CONFIRMED, the shipped bytes are green. `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`, then `cargo nextest run -p swissarmyhammer-validators`: 648 tests run, 648 passed, 0 skipped.

    - next: `/commit`, then the card is done.
  timestamp: 2026-08-11T21:30:54.854912+00:00
- actor: claude-code
  id: 01kzsbshr6fwa8t2wpga5hv3ab
  text: |-
    ### finish iteration 3 — clean
    - implement: changed — 3 files; the silent zero is closed, the script tests the report beside the status and breaks the run for a file the tool cannot read
    - test: green — cargo nextest -p swissarmyhammer-validators 648 run, 648 passed; rdeps 3411 run, 3407 passed with only the 4 failures that came before
    - commit: f8f0fcbe5
    - review: clean — 8 validators attempted, 0 findings, 1 refuted; all 6 prior findings checked; the card moves to done
  timestamp: 2026-08-11T21:31:25.830339+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffe680
title: complexity-go supersedes cognitive-complexity but drops the test and generated-code carve-outs
---
`builtin/validators/code-hygiene/rules/complexity-go.md` runs `gocognit -over 15` and declares `supersedes: cognitive-complexity`.

`cognitive-complexity.md` exempts "**Tests.** A function the probe marks as a test is already exempt and will not be listed." and "**Generated code and macro expansions.**"

Decide how the rule states each carve-out, or state on the rule why it cannot.

## What the tool does, measured

Measured with `gocognit v1.2.1` on 2026-08-11, over synthetic Go files that each hold one function of cognitive complexity 18 against the gate of 15.

- CORRECT, with one correction of spelling. `-test` is a boolean whose default is TRUE, so the bare flag changes nothing. `-test=false` filters a DIRECTORY WALK alone. Over a directory of one ordinary file and one `_test.go` file: the walk reported both files with `-test` and without it, and `-test=false` reported the ordinary file alone. The same `-test=false` over the NAMED `_test.go` path reported the test function again. This rule states `scope: files` and names each changed file, so the flag reaches nothing.
- CORRECT. `-ignore <regexp>` reads the PATH and never the content. `-ignore 'DO NOT EDIT'` dropped none of three files; `-ignore '\.pb\.go$'` dropped the file whose NAME ends that way. `-ignore` DOES apply to a named path argument, which the card did not state.
- CORRECT. `function-length-go` and `magic-numbers-go` get the generated-code carve-out from the golangci-lint default. Measured over one generated file and one plain file: the default `linters.exclusions.generated` dropped the generated file, and `generated: disable` reported it.
- WRONG. The rule stated that `gocognit` has no suppression comment. `gocognit v1.2.1` reads `//gocognit:ignore`. It must stand alone on a comment line in the doc comment of the function, with no blank line above the `func` line. Measured silent: the directive alone; a doc line above it; a doc line below it. Measured reporting: `// gocognit:ignore` with a space; the directive with text after it; `/*gocognit:ignore*/`; `//Gocognit:ignore`; the directive with a blank line under it; and `//nolint:gocognit`.

## The decision each carve-out took

- [x] **Generated code — reproduced in the run.** The script reads the lines above the `package` clause of each file it is given and drops the file that carries `^// Code generated .* DO NOT EDIT\.$`. An author cannot answer this one with the directive, because the generator writes the file again and the directive goes away.
- [x] **A test function — not reproduced, and named on the rule with the measured reason.** `gocognit` reads no function name, so it cannot make the mark at the DEFINITION that the prompt rule states. `-ignore '_test\.go$'` reads the FILE NAME, which the prompt rule forbids, and it silences a complex helper the prompt rule keeps. The author writes `//gocognit:ignore` above the test function instead.
- [x] **The directive — stated on the rule.** The false statement is gone, and the rule now states each spelling that works and each spelling that does not.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity

## Review Findings (2026-08-11 15:52)

- [x] `builtin/validators/code-hygiene/rules/complexity-go.md:184` — The rule body says "Given no path it writes 39 lines of usage text to stderr and exits nonzero." The count is wrong. Measured with gocognit v1.2.1: `gocognit 2>&1 >/dev/null | wc -l` gives 52 lines, and the exit code is 2. Three runs each gave 52. Correct the number to 52, or remove the count. — DONE. Re-measured with the real `gocognit v1.2.1`: three runs each gave 52 lines on stderr, 0 bytes on stdout, exit 2. The rule body now states 52 lines and exit 2. The same wrong count stood in the doc comment of `the_shipped_go_complexity_tool_rule_reads_only_the_files_it_is_given`, and it is corrected there as well.
- [x] `builtin/validators/code-hygiene/rules/complexity-go.md:13` — The first argument count does nothing. Delete it. With no argument, `remaining` is 0, the `while` loop does not run, and the second count below the generated-file test exits 0. Measured: the script without the first count gives exit 0, no stdout, and no stderr — the same result as the shipped script, byte for byte. Measured: the script without the SECOND count, over one generated file alone, writes 52 lines of usage text to stderr. The second count does all the work. — DONE by reshaping, because deletion breaks the set-wide guard. Measured RED: with the first count deleted, `each_shipped_files_scope_script_answers_a_run_that_gives_it_no_file` FAILED and named `complexity-go`. The script now loops over `"$@"` and hands `gocognit` one file at a time, so no run can give the tool an empty argument list and the second count is gone. One count is left, at the head of the script, where the `run` key of `builtin/validators/README.md` puts it.
- [x] `builtin/validators/code-hygiene/rules/complexity-go.md:186` — Two sentences give the first argument count work that it does not do: "The script counts its arguments first, and a count of zero exits 0 with no finding" and "So the guard makes the 0 an answer of the script's own, and it keeps the usage text off stderr." The first count keeps nothing off stderr. Write both sentences about the count that stands below the generated-file test. — DONE. The section "The run answers for its own arguments" is rewritten. It states 52 lines and exit 2; it states that the loop, and not the count, keeps the usage text off stderr; it names the count as the guard the README states; and it records the four measured runs with their stdout, their stderr and their exit code.

## Review Findings (2026-08-11 15:58)

- [x] `builtin/validators/code-hygiene/rules/complexity-go.md:212` — The sentence "The one call over every file reported in sorted order." names no sort key, and the measured key is not the path. Measured with gocognit v1.2.1 over three ordinary Go files, given in the argument order `zzz.go` (complexity 25), `mmm.go` (40), `aaa.go` (16): the one call reported `mmm.go` 40, then `zzz.go` 25, then `aaa.go` 16. That order is the complexity, highest first. The name order is `aaa.go`, `mmm.go`, `zzz.go`, so the one call did not sort by name. The loop reported `zzz.go`, then `mmm.go`, then `aaa.go`, which is the argument order and agrees with the sentence before it. Write the measured key into the sentence: the one call sorts by complexity, highest first. — DONE. Reproduced with gocognit v1.2.1 over three files of complexity 28 (`zzz.go`), 36 (`mmm.go`) and 21 (`aaa.go`), given in that argument order: the one call reported `mmm.go` 36, `zzz.go` 28, `aaa.go` 21, and the loop reported `zzz.go`, `mmm.go`, `aaa.go`. The sentence now states the key: the one call sorted by COMPLEXITY, highest first, and it states the name order to show the key is not the name.
- [x] `builtin/validators/code-hygiene/rules/complexity-go.md:174` — The section "The run answers for its own arguments" records four runs, and each of the four holds files that exist and that parse. The change from one call to a loop also changed the result of a run that holds a file the tool cannot read, and the section states nothing about that change. Measured over one ordinary file of complexity 21 and one path that does not exist: the parent script reported 0 findings, and the shipped script reported 1 finding, which is the file that parses. Measured over one ordinary file and one file with a syntax error: the parent script reported 0 findings, and the shipped script reported 1. The position of the bad argument, first or last, changed nothing. The one call lost the findings of every file of the run. The loop loses the findings of the bad file alone. State this change with the measured numbers. — DONE. Reproduced each number, over three kinds of bad file and in both argument orders. The new section "A file the tool cannot read" carries a table of the three shapes: the one call gave 0 findings at exit 0, the loop gave 1 finding at exit 0, and this script gives 0 findings at exit 1. It also states the cause: `gocognit` writes one report for the whole run and writes 0 bytes when one file breaks, measured in each order.
- [x] `builtin/validators/code-hygiene/rules/complexity-go.md:195` — The four bullets each state "nothing on stderr", and the section states no run in which `gocognit` fails. Measured with the shipped script: over a path that does not exist, stderr held two lines — `sed: ...: No such file or directory` and `gocognit: open ...: no such file or directory` — and the script exited 0 with no finding. Over a `.go` file with a syntax error, stderr held `gocognit: ...:5:9: expected '}', found 'EOF'` and the script exited 0 with no finding. Over an empty `.go` file, stderr held `gocognit: ...:1:1: expected 'package', found 'EOF'` and the script exited 0 with no finding. A file the tool cannot read therefore reaches the engine as a clean file. `builtin/validators/README.md` states "A tool can exit 0 for a file it could not open, and print an empty report. Test each file the script is given before the tool starts." Add a measured failure run to the section, or make the test the README states. — DONE in the script, not in the prose alone. `gocognit` keeps ONE status for a finding and for a failure: measured, a finding gives exit 1 with a JSON array, a clean file gives exit 0 with `null`, and a missing path, a syntax error and an empty `.go` file each give exit 1 with 0 bytes. The script now tests the REPORT beside the status, the shape `builtin/validators/README.md` states for a shared status. It calls a run measured only for exit 0 with `null` or exit 1 with a JSON array of one entry or more; every other answer writes `complexity-go: gocognit could not read <path>` to stderr and exits 1. A `[ ! -r "$file" ]` test names an unreadable path before `gocognit` runs, which also keeps the `sed` error off stderr. The script holds its findings in a variable and writes them at the end, so a broken run writes NO finding whatever the position of the bad file. Two RED-to-GREEN acceptance tests drive the shipped bytes: `the_shipped_go_complexity_tool_rule_breaks_on_a_file_it_cannot_read` and `the_shipped_go_complexity_tool_rule_breaks_on_a_file_it_cannot_parse`. Measured RED with the old shipped script: each reported 0 findings and 0 errors, and each FAILED with "the run must report exactly one tool error; got []". Both PASS with the new script.