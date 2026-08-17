---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m07gs9a6kpc2ws5asjfgyzcv
  text: |
    Picked up. Research done.

    `function-length-go` carries the shape this card asks for. I read its script and
    its acceptance tests, then measured every shape the card names with
    golangci-lint 2.12.2 over the shipped `mnd` command line, before I wrote a line.

    Measured, each over a probe module holding `return status == 404`:

    | the run | exit | the report | stderr |
    |---|---|---|---|
    | every literal in the ignored list | 0 | `Issues: []` | 0 bytes |
    | one literal outside it | 1 | one `mnd` row | 0 bytes |
    | a `.go` file that does not parse | 1 | one `typecheck` row ALONE | 0 bytes |
    | a workspace whose one package nobody may read | 7 | `Issues: []` | one `level=error` line |
    | the same file over a warm cache | 1 | the `mnd` row | one `level=error` line |
    | a workspace holding no `go.mod` | 7 | `Issues: []` | one `level=error` line |
    | a module holding no `.go` file | 5 | 0 bytes | one `level=error` line |
    | a `--config` golangci-lint cannot read | 3 | 0 bytes | `Error: can't load config: ...` |

    `Report.Linters` carries 121 entries, and `Enabled: true` stands on `mnd` and on
    `typecheck` and on no other. So a row of any other linter is the broken run.

    Eight runs of the script started together, three rounds, measured by me:
    without `allow-serial-runners` 3 of 8 stopped at exit 3 with
    `Error: parallel golangci-lint is running` and 0 bytes of report, in each round;
    with the key all 8 reported the row, in each round.
  timestamp: 2026-08-17T09:28:02.118366+00:00
- actor: claude-code
  id: 01m07gsqvgfn4d1ny3gw5jpqcm
  text: |
    Implementation landed.

    `builtin/validators/code-hygiene/rules/magic-numbers-go.md` now carries the same
    shape `function-length-go` holds:

    - `set -e` opens the script.
    - The tool writes its report to `$work/report.json` and its stderr to
      `$work/lint.err`. The pipe into `jq` is gone.
    - The status gate accepts 0 and 1 and breaks every other status. It forwards
      golangci-lint's own stderr and report first, then says
      `magic-numbers-go: golangci-lint exited <status> and measured no literal`.
    - A `jq` that cannot read the report breaks the run.
    - A report row of any linter that is not `mnd` breaks the run, because the
      `invalid_issue` processor answers with the typecheck rows ALONE.
    - Each line on the tool's stderr goes out whole under
      `sah-diagnostic: golangci-lint declined an item and said: <line>` at exit 0.
      No refusal head is matched. A sound run writes 0 bytes there, measured.
    - `doctor.check_command` now names `cat`, which the script really calls. The
      list is the same one `function-length-go` names.

    The rule body records each measurement, and the two paragraphs that described
    the old pipe now describe the run that ships.

    Tests: `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/magic_numbers_go.rs`,
    five acceptance tests over the REAL tool — a control that the probe module
    reports, and the four break/decline shapes. RED before, GREEN after.

    The two shared decline helpers did not fit. `ShippedDeclineProbe` and
    `verify_unreadable_file_is_declined` drive a `files`-scope run over one named
    path; `magic-numbers-go` is `workspace` scope and takes no file argument, so
    `function-length-go` does not use them either. Instead of a per-language copy I
    made `.../tests/shipped/go_probe.rs`: the Go module manifest, the package
    clause, the file golangci-lint cannot parse, the file nobody may read, the tool
    name, and the two shared error fragments now stand there one time.
    `function_length_go.rs` reads them from there. `ProbeResult<T>` moved up to
    `shipped.rs` for the same reason.
  timestamp: 2026-08-17T09:28:17.008429+00:00
- actor: claude-code
  id: 01m07gt0fvx8cre8b0nybxecp6
  text: |
    ### implement — changed
    - evidence: 5 files — `builtin/validators/code-hygiene/rules/magic-numbers-go.md`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs`, and two new files `.../tests/shipped/go_probe.rs` and `.../tests/shipped/magic_numbers_go.rs`. RED: `cargo nextest run -p swissarmyhammer-validators -E 'test(magic_numbers_go) or test(the_shipped_go_magic_numbers) or test(the_shipped_go_function_length)'` gave 17 run, 13 passed, 4 failed. GREEN after the rule change: 17 of 17 passed. Whole crate: `cargo nextest run -p swissarmyhammer-validators` gave 818 run, 818 passed, 0 skipped. `cargo clippy -p swissarmyhammer-validators --all-targets` and `cargo fmt -p swissarmyhammer-validators -- --check` are both clean. The shipped script was also driven by hand over five probe workspaces: exit 0 with the row and one `sah-diagnostic:` line at column 0 over a warm cache, and exit 1 with 0 bytes of stdout over the three broken shapes.
    - next: `/review`
  timestamp: 2026-08-17T09:28:25.851013+00:00
- actor: claude-code
  id: 01m07hjw33h2gpcyhe2f6r3z5d
  text: |-
    ### test — green

    Verified each claim in the test directive by measurement, fixed two failures found, then ran the full suite.

    **Claim 1 (LOAD-BEARING) — probe-byte drift found and fixed.** `go_probe.rs`'s shared `GO_MODULE_MANIFEST` read `"module golangci-probe\n\ngo 1.21\n"`, but the bytes `function_length_go.rs` held at HEAD were `"module function-length-probe\n\ngo 1.21\n"` (confirmed via `git show HEAD:.../function_length_go.rs`). Fixed `go_probe.rs` to restore the exact original bytes. Every other moved constant (`GO_MODULE_MANIFEST_PATH`, `GO_PACKAGE_CLAUSE`, `GO_UNPARSABLE_PATH`, `GO_UNPARSABLE_SOURCE`, `GO_UNREADABLE_PATH`, `GO_UNREADABLE_SOURCE`, `GO_TOOL_BINARY_NAME`, `GO_ANOTHER_LINTER_ERROR`→"golangci-lint reported a row of another linter", `GO_BROKEN_STATUS_ERROR`→"golangci-lint exited") matched byte-for-byte against the old `function_length_go.rs` content.

    **Claim 2 — rule specificity, confirmed both directions.** Broke `magic-numbers-go`'s `select(.FromLinter == "mnd")` line: only `magic_numbers_go::*` tests failed (2), all `function_length_go::*` tests (14) passed. Reverted, reran clean (16/16). Broke `function-length-go`'s equivalent line: only `function_length_go::*` tests failed (8), all `magic_numbers_go::*` tests (8) passed. Reverted; `git diff HEAD -- builtin/` returned to the pre-perturbation state (confirmed via md5 match before/after).

    **Claim 3 — whole-stderr forwarding, no closed enumeration.** Read the script: the final diagnostic loop (`while IFS= read -r line ...; printf 'sah-diagnostic: ... %s\n' "$line"`) has no grep/substring match on content. Drove real golangci-lint over an unmeasured-for-this-rule shape (added `wsl` to `enable:`, which the rule's own doc never measures — only `function-length-go.md` does) against a probe with one real `mnd` finding: the script reported the finding AND forwarded three `level=warning` deprecation lines verbatim as `sah-diagnostic:` at exit 0, unmodified. Reverted the `wsl` addition; diff restored (md5 match).

    **Claim 4 — byte-0 confirmed by hand + `od -c`.** Drove the shipped script directly (extracted verbatim) over (a) a workspace whose one package golangci-lint could not read (`chmod 000`) — stderr began at offset `0000000` with `level=error msg="[linters_context]...`; and (b) the two-package "declines" case with a warm cache — stderr began at offset `0000000` with `sah-diagnostic: golangci-lint declined an item and said: level=error msg=...`, real finding still reported on stdout, exit 0.

    **Claim 5 — `doctor.check_command` gap found and fixed.** The script's `trap 'rm -rf "$work"' EXIT` and `find ... -exec rm -rf {} +` both call `rm`, which was absent from `check_command`. Fixed: `"which golangci-lint go jq cat mktemp shasum mkdir touch find"` → `"...find rm"`. Every other listed tool (`golangci-lint`, `go` — transitively via golangci-lint's package loading, `jq`, `cat`, `mktemp`, `shasum`, `mkdir`, `touch`, `find`) is genuinely called; none listed are unused.

    **Claim 6 — clean.** No `.unwrap(`, `.expect(`, or `panic!(` in the diff. No unnamed numeric literals in code logic (only doc-comment prose, Go-source string literals, and one already-conventional `stated[0]` index matching the pattern already used in `function_length_go.rs`).

    evidence:
    - `cargo nextest run -p swissarmyhammer-validators shipped::magic_numbers_go shipped::function_length_go shipped::go_probe` — 16 passed, 0 failed, 0 skipped
    - `cargo fmt --check` — clean
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean
    - `cargo nextest run --workspace` — 14160 tests run: 14160 passed, 0 failed, 0 skipped
    - `git status --porcelain` — only the task's own changed/new files, plus the two fixes above

    task: ^am9b40g
  timestamp: 2026-08-17T09:42:00.547725+00:00
- actor: claude-code
  id: 01m07j20f3bnsa7vt5tr2r72zt
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (commit 3c9df4953). 9 rules ran. 0 findings, 0 confirmed, 1 refuted, 0 failed. 4 files reviewed. 4 `.kanban/` files not reviewed, because an ignore rule holds them out.
    - next: card moves to done. No open finding stays on this card.
  timestamp: 2026-08-17T09:50:16.547976+00:00
- actor: claude-code
  id: 01m07j2ehqpes07g066r9gk7fp
  text: |
    ### finish iteration 1 — clean
    - implement: changed — 5 files. The rule writes the report to a file, holds the status to 0 and 1, breaks on a report row of any linter that is not `mnd`, and states every stderr line whole under the marker at exit 0. 5 acceptance tests over the real tool. The Go probe bytes both golangci-lint rules share moved to a new go_probe.rs.
    - test: green — cargo nextest run --workspace, 14160 passed, 0 failed. fmt and clippy clean. The test step found the shared probe's module manifest had DRIFTED during the extraction (`function-length-probe` became `golangci-probe`), which would have changed what the other rule's tests measure, and restored the bytes. It also found `rm` missing from the doctor check.
    - commit: 3c9df4953
    - review: clean — 9 rules, 0 findings.
  timestamp: 2026-08-17T09:50:30.967874+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffa580
title: 'magic-numbers-go is a bare pipe: one Go file that does not parse reads as a clean tree'
---
`builtin/validators/code-hygiene/rules/magic-numbers-go.md` drives golangci-lint
through the same shape `function-length-go` carried before `^rfjsx87`:

    golangci-lint run --config "$config" --path-mode abs --show-stats=false \
      --output.json.path stdout ./... 2>/dev/null |
      jq -c '(.Issues // [])[] | select(.FromLinter == "mnd") | ...'

The pipe ends in `jq`, so every golangci-lint status reads as 0, and
`2>/dev/null` drops the one channel golangci-lint states a declined item on.

Measured with golangci-lint 2.12.2 while implementing `^rfjsx87`, over the
shipped command line:

- One Go file that does not parse gives a `typecheck` row, and the
  `invalid_issue` processor then answers with the typecheck rows ALONE:
  `if len(tcIssues) > 0 { return tcIssues, nil }`. So the run reports no `mnd`
  row for any file, at exit 1, with 0 bytes on stderr.
- A workspace holding no `go.mod` exits 7 with a report of `Issues: []`.
- A module holding no `.go` file exits 5 with 0 bytes of stdout.
- A `--config` golangci-lint cannot read exits 3 with 0 bytes of stdout.
- Eight runs started together without `allow-serial-runners` stop three of the
  eight at exit 3 with `Error: parallel golangci-lint is running`.
- A `.go` file nobody may read writes
  `level=error msg="[linters_context] typechecking error: open <path>: permission denied"`
  to stderr. A run whose cache holds no answer for the other packages then
  reports nothing at exit 7; a run whose cache does hold one reports those
  findings at exit 1.

The work: give `magic-numbers-go` the shape `function-length-go` now carries —
run the tool into a file, gate the status against 0 and 1, break the run for a
report row of another linter, and state each stderr line under
`sah-diagnostic:` at exit 0. Add the acceptance tests beside the ones
`function_length_go.rs` holds.

Found while implementing `^rfjsx87`. #tool-validators #objectivity