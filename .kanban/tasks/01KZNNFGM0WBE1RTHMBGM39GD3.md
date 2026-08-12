---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzw6gavzddahx1g755wbwyaj
  text: |-
    Tool survey answered the card: periphery CAN separate test-support code from product code, so the escape hatch is not used.

    `periphery scan --help` (3.8.0, /opt/homebrew/bin/periphery) names four path options, and they are not the same thing:

    - `--exclude-tests` / `--exclude-targets` — take targets out of the INDEX.
    - `--index-exclude` — takes source globs out of the INDEX.
    - `--report-exclude` — "Source file globs to exclude from the results. Note that this option is purely cosmetic, these files will still be indexed."
    - `--report-include` — supersedes `--report-exclude`, also cosmetic.

    `--report-exclude` is exactly the split the card asked for: the test targets stay indexed, so a test still counts as a caller, and their own declarations never reach the report.

    Re-took the measurement over the corpus the rule body names, `Alamofire` (clone at `0455bfb`), built with `swift build --build-tests`, store at `.build/out`, each run minus `var.parameter`:

    | Run | Findings | Where |
    |---|---|---|
    | shipped flags, no report filter | 74 | 22 `Source/`, 52 `Tests/` |
    | `--report-exclude 'Tests/**'` | 22 | every one in `Source/` |
    | `--exclude-tests` | 25 | every one in `Source/` |

    The 103/74 split reproduces the rule's existing table exactly. 30 of the 52 stand in `Tests/AFError+AlamofireTests.swift`, which is the file the card names.

    `diff` of the 22 from the filtered run against the `Source/` subset of the unfiltered run: IDENTICAL. So the filter drops findings and changes no analysis.

    `--exclude-tests` is the trap. It adds three findings that a test does call:
    - `Source/Core/RequestTaskMap.swift:102 isEmpty`
    - `Source/Features/OfflineRetrier.swift:109 init(monitor:maximumWait:isOfflineError:)`
    - `Source/Features/RequestInterceptor.swift:79 retryRequired`

    Those three are the whole reason the run builds the tests, so the flag that reads like the shorter spelling of this carve-out is the one flag that breaks it.

    Target paths come from `swift package describe --type json`, `.targets[] | select(.type == "test") | .path`, not from a `Tests/` naming guess. Alamofire declares one test target at `Tests` (explicit `path:`), swift-nio declares fifteen at `Tests/<Name>` (SwiftPM convention), vapor three. Measured: repeated `--report-exclude` flags work; ONE flag holding two space-joined globs does NOT (74, nothing excluded), so the script writes one flag per target.

    Also measured: the shipped fixture package (`Package.swift.tmpl`) declares no test target, so the loop writes no flag and the fail fixture still reports its six findings.

    `supersedes: dead-code` stays, and it is now honest: the prompt rule's "test functions and test-only helpers" carve-out is reproduced by the run rather than left to the reader.
  timestamp: 2026-08-12T23:56:44.287003+00:00
- actor: claude-code
  id: 01kzw6gnvntg6ttjz4wx759sqk
  text: |-
    ### implement — changed
    - evidence: 4 files — `builtin/validators/code-hygiene/rules/dead-code-swift.md` (run block writes one `--report-exclude <test target path>/**` for each target `swift package describe` marks `test`, with two guards for a manifest the run cannot read; new "The test targets are indexed, and never reported" section holds the measurement table); `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_swift.rs` (new acceptance test); `.../tests/shipped.rs` (registers the module, module doc); `.../tests.rs` (`SWIFT_DEAD_CODE_RULE`).
    - RED verified twice against the shipped bytes, by editing the shipped script and re-running the test:
      - script without the report filter → `["Sources/Probe/Product.swift:7", "Tests/ProbeTests/ProbeTests.swift:5"]`, so the test-only helper reported.
      - script with `--exclude-tests` in place of the filter → `["Sources/Probe/Product.swift:2", "Sources/Probe/Product.swift:7"]`, so the product declaration only a test calls reported.
      - shipped shape → `["Sources/Probe/Product.swift:7"]` alone. The rule file was restored from a backup after each run; `git diff` confirms only the intended change stands.
    - green: `cargo nextest run -p swissarmyhammer-validators` 698 passed, 0 failed; `cargo nextest run -p mirdan` 503 passed; `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-12T23:56:55.541661+00:00
position_column: doing
position_ordinal: '8280'
title: dead-code-swift reports test-only helpers, which dead-code exempts by name
---
`builtin/validators/code-hygiene/rules/dead-code-swift.md` runs `swift build --build-tests` then `periphery scan --retain-public ...` and declares `supersedes: [dead-code]`.

`dead-code.md` exempts "**Tests**: test functions and test-only helpers ... and items gated by `#[cfg(test)]` / `mod tests`."

`--build-tests` makes the test harness a caller, which covers the test functions. It also brings the test-support code itself under the gate, and the rule's own measurement says where the findings sit: "**52 of the 74 findings** sit [there] — a long file of `AFError` convenience properties no test ever calls." Those 52 are the "test-only helpers" the prompt rule carves out. periphery has no flag that retains test-support declarations while still counting tests as callers.

The entry-point carve-out is partial. `--retain-objc-accessible`, `--retain-swift-ui-previews` and `--retain-codable-properties` cover the ObjC runtime, `#Preview` and reflection-driven encoding. They do not cover a declaration reached by string-keyed registration or a non-`@objc` plugin entry.

`// periphery:ignore` works but takes no trailing text, so a reason needs a second comment line. 52 suppressions is not a carve-out.

Decide how the rule separates test-support code from product code, or state on the rule why it cannot.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity