---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m00xep4cz99657tsk0r2r6vd
  text: |-
    Measurement done. Corpus: six well-known Go repositories cloned at HEAD on 2026-08-14 — kubernetes/client-go 3fcdd4c72588c077802ae4c6a3fec8375665080b, spf13/cobra adbc8813901bba65827259daa8e22ff94ec1f30e, etcd-io/etcd 0836b69e9cf47d00b535f2bc331b4c47bb23cb80, gin-gonic/gin 34dac209ffb6ef85cc78c5d217bbb7ad001d68fd, grpc/grpc-go bf9e7cd3430df40d0732ba42eb88bd5f2cc63407, prometheus/prometheus 05f9eb8b3b8e10b48c8f4153b0714dbe9bc9a630. 5470 `.go` files, 1290 of them `_test.go`, 32 Go modules. Method: two golangci-lint runs for each module, one at `lines: 1, statements: 10000` and one at `lines: 10000, statements: 1`, so funlen prints each function's own line count and statement count in its message. 23216 functions measured. Every sweep is arithmetic on the tool's own numbers, and the winning setting was then run through the real tool for a check: predicted 12 findings and 8 with the test carve-out, measured 12 and 8.

    FIRST FACT, which turns the card around. funlen ORs its two dimensions. `funlen.go` runs the statement check first and `continue`s past the line check only so one function reports one time. A statement limit BESIDE the line gate can therefore only ADD findings, never carve one out: measured at `lines: 250`, statements 10000 -> 161 findings, 250 -> 161, 180 -> 162, 120 -> 180, 80 -> 253, 40 -> 982. The statement dimension carves out data only when it is the ONLY gate.

    SECOND FACT. The line gate's own finding set is nearly all carve-out material. Of the 161 functions over 250 funlen lines, 136 hold 40 statements or fewer. The ratio of statements to lines for functions of 250+ lines is median 0.017 (p10 0.004, p90 0.290) — a data literal. For functions of 100+ statements it is median 0.633 (p10 0.437, p90 0.780) — code. Lines select data; statements select code.

    DECISION 1: the statement dimension comes back AS THE GATE, and the line gate goes out of reach. `lines: 10000, statements: 160`. 160 = 250 code lines x 0.633, the measured median for the procedural population, the same derivation `function-length-python` used for 180. The sweep says 160 is the elbow: statements 120 -> 35 findings of which 19 stand under 250 lines; 140 -> 17 of which 5; 160 -> 12 of which 1; 190 -> 10 of which 0.

    DECISION 2: a test is exempt by its DEFINITION, not by its path. At the gate of 160 the corpus reports 12, and 4 of them are named test functions the prompt rule exempts word for word. `linters.exclusions.rules` with `path: _test\.go$` and `text: "^Function '(Test|Benchmark|Fuzz|Example)([^\\p{Ll}].*)?' "` drops those 4 and leaves 8. funlen writes the function NAME into its own message, so the exclusion reads the definition. A plain `path: _test\.go` exclusion would drop 142 of the 161 shipped findings, 11 of them helpers the prompt rule still lists, which is the trade `complexity-go` refuses.

    Measured on a probe module against the candidate config: a 302-line composite literal of 1 statement is silent; a 301-line builder chain of 1 statement is silent; a 307-line table-driven `TestTable` is silent; a `TestLoop` of 169 statements is silent; a `buildRequest` helper of 170 statements in the same `_test.go` file REPORTS; `Testify` in a `_test.go` file reports (the `[^\p{Ll}]` guard); `TestLooking` outside a `_test.go` file reports; `//nolint:funlen // reason` silences. The generated-code carve-out is untouched: `linters.exclusions.generated: lax` still drops the file carrying the header with `rules` present.

    The shipped fixture pair no longer straddles the gate — `ShortMixState` holds 202 statements and `LongMixState` 262 — so both fixtures must be retuned to the statement gate.
  timestamp: 2026-08-14T19:54:45.260294+00:00
- actor: claude-code
  id: 01m00ybxgk3adzram6mfrgp2r7
  text: |-
    ### implement — changed
    - evidence: 7 files — builtin/validators/code-hygiene/rules/function-length-go.md (gate `lines: 10000, statements: 160`, `linters.exclusions.rules` test carve-out, whole body rewritten with the corpus tables), builtin/validators/code-hygiene/fixtures/function-length-go.fail.go.tmpl (170 statements), builtin/validators/code-hygiene/fixtures/function-length-go.pass.go.tmpl (150 statements), builtin/validators/code-hygiene/VALIDATOR.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs (new, 3 acceptance tests), crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs (`verify_staged_rows_report` takes borrowed staging and rows), crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs (`GO_FUNCTION_LENGTH_RULE`).
    - RED first, each for the defect the card names: `..._measures_statements_and_not_lines` reported the 302-line composite literal and the 301-line builder chain and stayed silent on the 170-statement procedure; `..._reads_a_test_from_its_definition` reported the 307-line table-driven `TestTable` and stayed silent on the `buildRequest` helper; `..._skips_a_generated_file` reported nothing at all. GREEN after the rule change.
    - cargo nextest run -p swissarmyhammer-validators: 725 passed, 0 failed. cargo nextest run -p mirdan: 522 passed. cargo fmt --check clean, cargo clippy --workspace --all-targets -D warnings clean.
    - The three tests broke on a WARM golangci-lint cache before `go_uncached` was added: golangci-lint keys its cache on package content and replays the first run's absolute paths, which also makes the generated-code filter fail open. That is ^mms9g8d's defect, reproduced and recorded on that card. This card touched the heredoc configuration alone and left the pipeline region ^mms9g8d owns untouched.
    - next: /review
  timestamp: 2026-08-14T20:10:43.091086+00:00
- actor: claude-code
  id: 01m00yx2z1w7s91sdygfbmr4xf
  text: |-
    ### review — clean
    - evidence: `review sha 09d8c2d28~1..09d8c2d28` — 0 findings, 0 confirmed, 2 refuted, 9 attempted. 3 files reviewed, 2 fixture files out of scope. No prior Review Findings section on this task.
    - scope: the engine reviewed the diffs only — the lines this commit added or modified.
    - next: none. Task moves to done.
  timestamp: 2026-08-14T20:20:05.729010+00:00
- actor: claude-code
  id: 01m00yybne3f0h1bkjs89e4jc6
  text: |-
    ### finish iteration 1 — clean. Card done.
    - **The card's premise was corrected by measurement.** It assumed funlen's two dimensions AND. They OR — `funlen.go` runs the statement check first and only `continue`s so a function reports once. So a statement limit BESIDE the line gate can only add findings, and the fix is an inversion: `lines: 10000, statements: 160`. The statement count is now the gate and the line limit is out of reach.
    - Corpus: 6 Go repositories at HEAD — kubernetes/client-go 3fcdd4c, spf13/cobra adbc881, etcd-io/etcd 0836b69, gin-gonic/gin 34dac20, grpc/grpc-go bf9e7cd, prometheus/prometheus 05f9eb8. 5470 .go files, 1290 of them _test.go, 32 modules. Two runs per module with each dimension set to 1 in turn make funlen print every function's own line AND statement count, so 23216 functions came back with both numbers and every sweep is arithmetic on the tool's own counts rather than a re-run.
    - **What decided 160**: of the 161 functions over 250 funlen lines, 136 hold 40 statements or fewer. Statements-to-lines ratio is median 0.017 for the 250+-line population and 0.633 for the 100+-statement population — lines select DATA, statements select CODE. 250 × 0.633 = 160, the same derivation function-length-python used for its 180. The sweep confirms the elbow: 120 → 35 findings of which 19 stand under 250 lines, 140 → 17 of which 5, **160 → 12 of which 1**, 190 → 10 of which 0.
    - **Tests exempted by DEFINITION, not path.** funlen writes the function name into its own message, so the exclusion matches `Test|Benchmark|Fuzz|Example` names in _test.go: 12 → 8. A plain path exclusion would have dropped 142 of the old gate's 161, ELEVEN of them helpers the prompt rule still lists — the trade complexity-go refuses, and consistent with the no-path-based-test-exclusion rule.
    - The three trailing shapes are silent at the gate: composite literal 302 lines/1 statement, builder chain 301/1, table-driven test 307/4. The statement gate alone drops all three; the exclusion only covers the dense named tests that remain.
    - Non-regressions held: generated-code carve-out untouched (`exclusions.generated: lax`), `//nolint:funlen` still silences, both under acceptance tests.
    - Prediction re-run through the real tool: predicted 12 findings and 8 after the carve-out, measured 12 and 8.
    - test: green. commit: 09d8c2d28. review: clean — 0 findings, 2 refuted, 9 attempted; 3 files reviewed, 2 fixtures out of scope.

    **Two process notes.** The commit subject initially read "set lines to 250, statements to 10000" — the OLD broken state rather than the fix — and was amended to "gate function-length-go on statements at 160, not lines". And the rule body now contains `statements: 10000` in THREE legitimate roles: the quoted defect, the measurement harness, and nowhere in the shipped `run` block. The reviewer was warned and verified which role each played rather than assuming.
  timestamp: 2026-08-14T20:20:47.406949+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff8380
title: function-length-go sets statements 10000, which removes the data and builder carve-out
---
`builtin/validators/code-hygiene/rules/function-length-go.md` runs `funlen` through golangci-lint at `lines: 250`, `statements: 10000`, `ignore-comments: true`, and declares `supersedes: [function-length]`.

`function-length.md` exempts "Functions that are mostly configuration/data (e.g., builder patterns with many options)" and "Initialization functions that set many fields". `funlen` has two dimensions, lines and statements. A 400-line composite literal is one statement, so the statement dimension WOULD carve it out. The rule sets `statements: 10000` on purpose to turn that dimension off, so the line gate is the only gate and a data-heavy function reports. The exemption is made unreachable by configuration.

Compare `function-length-python`, which selects `PLR0915` — a statement count — and gets the same carve-out for free. The two rules make opposite choices for one prompt rule.

The test carve-out is also dropped: golangci-lint analyses `_test.go` files by default, the temp config sets no `linters.exclusions.rules`, and `funlen` has no test option. A 300-line table-driven `TestFoo` reports.

The generated-code carve-out IS reproduced, from the `linters.exclusions.generated: lax` default.

`//nolint:funlen // <reason>` works. Decide whether the statement dimension comes back, and how tests are exempted.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity