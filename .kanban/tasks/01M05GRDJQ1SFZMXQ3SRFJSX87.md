---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m0605mc3he384gak0vrhz09b
  text: |-
    Research, measured with golangci-lint 2.12.2, eslint 10.8.0 and jq 1.8.2 on this machine.

    **TypeScript is the sound shape the card expected.** eslint marks a file it could not read with `fatal: true` and judges every other file of the same run beside it. Measured over `judged.ts` (one function of 302 lines) and a file whose body never closes: the report carried the length message AND one `Parsing error: '}' expected.` message with `ruleId: null`, `fatal: true`, `severity: 2`, at exit 1 and 0 bytes of stderr. So the decline is one item of a sound run.

    `fatal` rather than a null rule id is the field to read. Measured on a file carrying two `eslint-disable` comments the temporary config cannot resolve: `Definition for rule '@typescript-eslint/no-explicit-any' was not found.` under that rule id at severity 2, and `Unused eslint-disable directive (...)` under a NULL rule id at severity 1 — and eslint judged that file. Read over the whole `lib/` tree of eslint 10.8.0, three places write `fatal: true`: `parser-service.js`, `processor-service.js`, and `linter.js` through `file-report.js` `addFatal` for an inline `/* eslint ... */` comment it cannot read. The third one leaves the file linted, measured: eslint marked the comment fatal AND reported the length of the function under it.

    **Go is NOT the sound shape.** golangci-lint's `invalid_issue` processor answers with the typecheck rows ALONE — `if len(tcIssues) > 0 { return tcIssues, nil }` — so ONE Go file that does not parse drops every `funlen` row of the whole run. Measured over a probe module of two packages: the report carried the `typecheck` row and no `funlen` row, at exit 1 and 0 bytes of stderr. Four more shapes answered the same way: a second sound package beside the broken one, the broken file in the SAME package, the broken file as a `_test.go`, and a file whose bytes are not UTF-8.

    So a `typecheck` row is a run that measured NOTHING, and the answer is a broken run rather than a `sah-diagnostic:` line. The card's "the finding of the good file survives" half is not reachable for that shape: golangci-lint itself throws the finding away.

    **A dead end, recorded so the next agent does not repeat it.** A first round read a file golangci-lint cannot READ as a declined item of a sound run, because a scratch probe over a WARM cache reported the other package's `funlen` row beside the `level=error` line. The acceptance test then failed under parallel load. Measured over one workspace run three times: the file unreadable on a cold cache gives `Issues: []` at exit 7; the file readable gives the `funlen` row at exit 1; the file unreadable again gives the `funlen` row at exit 1 — out of the cache the second run filled. `saveIssuesToCache` runs only for a run that met no error. So the cache decides, a probe over a fresh workspace always reads a cold one, and the two rows need two tests: a real-tool break, and a stub that answers with the bytes the warm run wrote.

    **The Go status roster**, read from `pkg/exitcodes`: 0 Success, 1 IssuesFound, 2 WarningInTest, 3 Failure, 4 Timeout, 5 NoGoFiles, 6 NoConfigFileDetected, 7 ErrorWasLogged. Measured: a workspace with no `go.mod` exits 7 with `Issues: []` (a report that reads clean), a module with no `.go` file exits 5, a `--config` it cannot read exits 3, and eight runs started together without `allow-serial-runners` stop three of the eight at exit 3. `setupExitCode` sets 7 only while the status is still Success, so the status gate at {0,1} can cost no finding.
  timestamp: 2026-08-16T19:18:26.435953+00:00
- actor: claude-code
  id: 01m06063jcygh2ejvh2sw0ncsk
  text: |-
    ### implement — changed
    - evidence: 5 files.
      - `builtin/validators/code-hygiene/rules/function-length-go.md` — the pipe becomes a script: `set -e`, golangci-lint into `report.json` and `lint.err`, a status gate that accepts 0 and 1 alone, two `jq` steps whose own status is read, a gate that breaks the run for a report row of another linter, and a loop that states each stderr line under `sah-diagnostic:` at exit 0. `check_command` gains `cat`. The body gains "A run that measured no function" with the status table, the `invalid_issue` measurement, and the three-run cache table, and the `allow-serial-runners` and selection paragraphs are rewritten to the new shape.
      - `builtin/validators/code-hygiene/rules/function-length-typescript.md` — the same shape: a status gate at 0 and 1, `select(.fatal)` written under the marker naming the file and eslint's own message, the length messages kept as findings, every other message dropped, and each stderr line stated under the marker. `check_command` gains `cat`. The body gains "A file eslint could not judge" with the status table and the three writers of `fatal: true`, and the temporary-directory paragraph is measured again.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` — `verify_staged_tree_breaks` and `verify_staging_breaks`, the borrowed counterparts of `verify_shipped_tree_breaks`.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs` — four acceptance tests: `..._breaks_on_a_file_it_cannot_parse`, `..._breaks_on_a_workspace_holding_no_module`, `..._breaks_on_a_file_it_may_not_read`, `..._declines_a_file_it_may_not_read`.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_typescript.rs` — two acceptance tests: `..._declines_a_file_it_cannot_parse` (the finding of `judged.ts` survives AND the unparsable file is stated) and `..._breaks_on_a_status_it_cannot_read`.
    - tests: `cargo nextest run -p swissarmyhammer-validators` — 778 passed, 0 failed. `cargo nextest run -p mirdan` — 503 passed. `cargo fmt --check` and `cargo clippy --all-targets` clean.
    - next: `/review`. `magic-numbers-go` carries the same bare pipe and is now `^am9b40g`.
  timestamp: 2026-08-16T19:18:41.996230+00:00
- actor: claude-code
  id: 01m061sq47tqcas2vhj4kh59dy
  text: |-
    ### test — green

    - `cargo fmt --check` — clean, no output.
    - `cargo nextest run --workspace` — 14120 tests run, 14120 passed, 0 failed, 0 skipped. Ran twice; first run 0 leaks, second run 1 leaky mark on `swissarmyhammer-embedding embedder::tests::embedder_error_config_parse_display` — an unrelated package, not the changed rules, and no process stayed alive after the run (`ps` clean). Confirmed the same nextest "LEAK" mark under narrow forced concurrency on the unrelated `function_length_python` tests too, so it is a pre-existing nextest process-reap race under heavy parallel load, not a defect from this card.
    - `cargo nextest run -p swissarmyhammer-validators -E 'test(shipped::function_length_go::) or test(shipped::function_length_typescript::)'` — 14 tests run, 14 passed, run by name to confirm none were filtered out. Repeated 6 more rounds (some at `--test-threads 16`), all 14/14 pass every round.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean, exit 0, no warnings.
    - Mirdan embed check: `cargo build -p mirdan`, found `OUT_DIR` at `target/debug/build/mirdan-66497afbb02f504c/out/builtin_validators.rs`, extracted the `code-hygiene/rules/function-length-go.md` and `code-hygiene/rules/function-length-typescript.md` entries and diffed against the files on disk — `diff` exit 0 for both, byte-identical.
    - Sound-run stderr check: ran each shipped script directly against a probe workspace/file over the gate. Go rule: exit 0, one finding, stderr 0 bytes. TypeScript rule: exit 0, one finding, stderr 0 bytes.
    - TMPDIR leak check: counted entries under `$TMPDIR` immediately before and after a sound run of each script — Go rule 255847 → 255847, TypeScript rule 255847 → 255847. No net leak from the `mktemp -d` work directory (the golangci-lint cache directory is documented, deliberate, persistent state, not a leak).
    - Manually reproduced the cold-cache and warm-cache halves of the "file it may not read" shape against a real `$TMPDIR` workspace with the shipped Go script: cold (unreadable) → breaks, exit 1, no finding, matching `the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read`. Warm (readable) → exit 0 with the `funlen` finding. A third round (unreadable again) surfaced a live `typecheck` Issue row rather than the stub's log-only diagnostic — a different, less favorable cache state than the doc's own measurement describes, and the shipped script's status gate still handled it correctly by breaking the run (exit 1, no finding) rather than reading it as clean. This confirms the rule doc's own statement that the "declines" row's exact cache state cannot be reliably staged live, which is why that acceptance test uses a stub; the real tool's status-gate behavior stayed sound under every live cache state I drove it through.
    - No `#[ignore]` or test-skip in the changed/added test files (only an unrelated `Iterator::skip` call).
    - `git status` on the touched paths shows only the same 5 files from the start of the run; no accidental edits from probing.

    task: none other; recording on ^rfjsx87 per the step-record instruction.
  timestamp: 2026-08-16T19:46:53.191900+00:00
- actor: claude-code
  id: 01m062b9377wt2cq1z98c54gb0
  text: |
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (commit e7a22e4a4) — 0 findings, 0 confirmed, 5 refuted of 9 attempted. 3 files reviewed. 2 `.kanban/` files not reviewed, because `.reviewignore` holds them out.
    - next: none. The card moves to done.
  timestamp: 2026-08-16T19:56:28.647475+00:00
- actor: claude-code
  id: 01m062c1t4kpfb5mvw70wvby0x
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 5 files; both bare pipes become scripts. TypeScript states each eslint message that carries `fatal: true` under the marker at exit 0. Go BREAKS instead, because golangci-lint answers with the typecheck rows alone, so one file that does not parse costs the run every funlen row.
    - test: green — cargo nextest run --workspace, 14120 passed, 0 failed, 0 skipped (run two times); the 14 new tests pass by name over 7 rounds; a sound run writes 0 bytes to the stderr of each tool; the mirdan build embeds both rule files byte-identically; no TMPDIR entry leaks; fmt and clippy clean
    - commit: e7a22e4a4
    - review: clean — 0 findings, 5 refuted of 9 attempted; card moved to done
  timestamp: 2026-08-16T19:56:53.956548+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff9a80
title: function-length-go and function-length-typescript read a file the tool cannot parse as clean
---
Two `function-length-*` rules drop the tool's own parse failure in the `jq`
filter and answer exit 0 with no finding, so a file the tool never measured
reads exactly like a clean file.

`builtin/validators/README.md`: "Do not stay silent either: a run that reports
no finding and exits 0 over an item it never judged reads exactly like a clean
pass over that item."

`builtin/validators/code-hygiene/rules/function-length-go.md` — no status test at
all, and stderr is thrown away:

    golangci-lint run ... --output.json.path stdout ./... 2>/dev/null |
      jq -c '(.Issues // [])[] | select(.FromLinter == "funlen") | ...'

The rule body already records that the `jq` filter drops golangci-lint's
`typecheck` diagnostics. The `2>/dev/null` removes the only other trace, so a Go
file that does not compile is silently clean. The pipe also ends in `jq`, so
every golangci-lint failure exits 0.

`builtin/validators/code-hygiene/rules/function-length-typescript.md` — the same
shape:

    eslint --no-config-lookup --config "$config" --format json "$@" |
      jq -c '.[] | ... | select(.ruleId == "code-hygiene/max-lines-per-function") | ...'

An eslint fatal parse message carries `ruleId: null`, so the `select` drops it
and the file reads as clean at exit 0. The pipe ends in `jq`, so every eslint
failure also exits 0.

The work:

- Measure, for each rule, what the tool writes for a file it cannot parse: the
  report row, its stderr, and its exit — and whether it measured the OTHER files
  of the same run.
- Rewrite each pipe as a script: run the tool into a file, test the status
  against the findings status, select this rule's rows as findings, and write
  each unmeasured row under `sah-diagnostic:` at exit 0.
  `function-length-python` holds the worked shape.
- Add the acceptance tests. Neither rule has one for this shape today, so the
  test must stage a file over the length gate beside the unparsable one and hold
  the run to both halves — `verify_unjudged_file_is_declined` in
  `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`.
- State each measurement in the rule body.

`function-length-swift` has the same silence, and the rule body already records
that swiftlint states no parse failure at all: it parses with recovery and lints
what it recovered. That one needs a tool answer before a rule answer, so it is
NOT part of this card.

Found while implementing `^s8d7fva`. #tool-validators #objectivity

## Review Findings (2026-08-16 14:48)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 3 file(s) reviewed, 2 not reviewed.

> 2 file(s) not reviewed — excluded by an ignore rule:
> - `.kanban/ (from .reviewignore)` — 2 file(s)
