---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzh26fgrc3gwjas5b452tzb1
  text: |-
    Research done. Every number below is measured on this machine, not quoted.

    Tools installed for the work: `eslint-plugin-sonarjs@4.2.0` (npm, global, beside the eslint 10.8.0 and typescript-eslint 8.66.0 that `magic-numbers-typescript` already pins), `gocognit v1.2.1` and the standalone `funlen v0.2.0` (`go install`). `swiftlint 0.65.0` and `golangci-lint` were already present.

    ## TypeScript — both card rules are fit

    `eslint-plugin-sonarjs` 4.2.0 loads under eslint 10.8.0 through the same temporary flat config and `NODE_PATH` trick `magic-numbers-typescript` uses.

    Hand-checked two findings against the published Sonar algorithm and both are exact:
    - `format-date.ts` `formatRelativeMagnitude` scores 18 — 7 `if` at nesting 0, plus 5 ternaries at nesting 1 which score 2 each, plus 1 ternary at nesting 0.
    - `progress-ring-display.tsx` scores 16 from nested ternaries.

    JS and TS have no macro expansion, so there is no contamination of the kind that made `clippy::cognitive_complexity` unfit.

    Corpus: 444 `.ts`/`.tsx` files under `apps/`. `sonarjs/cognitive-complexity` at 15 reports 33 findings, 10 of them outside `*.test.*`/`*.spec.*`. `max-lines-per-function` at 250 with `skipBlankLines`/`skipComments` reports 37, of which 36 sit on `describe(...)` arrow callbacks in test files and 1 on a real component. The counting is exact: a probe of 260 code lines plus 52 comment lines plus 52 blank lines is counted as 264, which is the code lines plus the signature line and the closing brace.

    Enumerated the alternative before keeping the card's choice: `eslint-plugin-sonarjs` also ships `sonarjs/max-lines-per-function`. It agrees on the count (264 on the same probe) but takes `{maximum: N}`, and the bare-number form `["warn", 250]` silently does nothing — it reported 0 findings over the whole 444-file corpus. The core rule states its counting in options the prompt rule already names, so the card's choice stands.

    Also measured: with a temporary config, every project `eslint-disable` comment naming a plugin the temporary config does not load turns into a "Definition for rule ... was not found" message — 155 of them on this corpus, plus 10 unused-directive messages. The `jq` filter selecting the two owned rule ids drops all of them, which is the attribution the README describes.

    ## Swift — `ignores_case_statements: true` is a measured decision

    Enumerated the swiftlint metrics rules: `cyclomatic_complexity`, `function_body_length`, `closure_body_length`, `nesting`, `file_length`, `type_body_length`, `line_length`. Swiftlint has no cognitive-complexity rule.

    `function_body_length` is an exact match for the prompt rule: on a probe of 260 code lines with 52 comment lines and 52 blank lines it reports "should span 250 lines or less excluding comments and whitespace: currently spans 262 lines" — 262 is exactly the code lines. Specifying only `warning:` disables the default `error:` level: a 262-line body reports Warning against 250, not Error against the default 100.

    `cyclomatic_complexity` reports when the count is strictly over the warning level, and counts decision points with no `+1` base — a probe of one `for`, one `if` and 14 `else if` scores 16 and is silent at `warning: 16`.

    Corpus: Alamofire, swift-nio and vapor at HEAD — 893 `.swift` files. None of the three carries a `.swiftlint.yml`, so the numbers are not the residue of prior linting.

    | `warning` | plain | `ignores_case_statements: true` |
    |---|---|---|
    | 8 | 118 | 21 |
    | 10 | 66 | 12 |
    | 12 | 44 | 9 |
    | 15 | 23 | 2 |
    | 20 | 8 | 1 |

    At 15 the plain form reports 23 and 21 of them vanish when case statements stop counting — they are flat dispatch tables. `NIOHTTP1/HTTPEncoder.swift` `write(response:)` scores 121 from 120 one-line `case` arms writing a status line; `NIOPosix/HappyEyeballs.swift` `processInput` scores 26 from a state-machine `switch`. The `cognitive-complexity` prompt rule states the opposite: "A `match` or `switch` counts once for the whole construct. Its arms are branches of one decision", and carves out "a long flat list of simple cases". So 21 of 23 plain findings are the prompt rule's own carve-out reported as mandatory work.

    `ignores_case_statements: true` drops a switch to zero, not to one — a probe of a 21-arm switch scores 21 plain and reports nothing at `warning: 1` with the option on. That under-counts, and it hides the nesting in `DirectoryEntries.swift`, a `while` around a `switch` around a `switch`. The rule takes the under-count: a missed finding costs a review nothing, and a wrong finding is a requirement to change correct code.

    ## Go — gocognit fit, and the length tool chosen by measurement

    `gocognit v1.2.1` implements the published Sonar algorithm exactly. Hand-checked: a `for` holding an `if` holding an `if`/`else if`/`else` scores 8 — 1 + 2 + 3 + 1 + 1. A flat 260-line function of `total += N` scores 0, so length never leaks into the score. Over the Go 1.26.5 standard library (4350 files, 29580 functions) `-over 15` reports 2731, and the distribution across the gate is smooth — 356 at 13, 282 at 14, 256 at 15, 232 at 16 — with no mass piled just over it. `-json` on an empty result prints `null`, so the pipe has to read `(. // [])[]`.

    Length tool: wrote a Go program that replicates funlen's `parseStmts` and `getLines` from its source and computes, for every function, the true code-line count from `go/scanner` tokens. Verified it against the tools: it reproduces golangci-lint's 314/278 and standalone funlen's 262/232 on the same probe. Ran it over the Go standard library plus 411 MB of module cache — 94774 functions.

    | metric | median ratio to code lines, functions of 250+ code lines | gate at 250: findings / false positives / missed |
    |---|---|---|
    | funlen `lines`, `ignore-comments: true` | 1.002 | 412 / 39 / 6 |
    | revive `function-length` lines | 1.132 | 517 / 138 / 0 |
    | funlen `statements` | 0.763, but p10 0.012 and p90 0.997 | not usable |

    379 functions are truly over 250 code lines. funlen's line count with comments ignored is the closest metric there is — it counts code lines plus blank lines, and long Go functions carry almost none. revive counts raw physical lines including comments and blanks, which the prompt rule excludes, and gives 138 false positives. The statement count spans 80x across the range because a 400-line composite literal is one statement, so no single threshold can stand for 250 code lines.

    Which binary runs funlen: the standalone `funlen` command exposes NO threshold flags. `funlen -flags` lists only `V`, `all`, `c`, `flags`, `json`, `source`, `tags`, `test`, `v`; the limits are hardwired to 60 lines and 40 statements in `NewAnalyzer`. So it cannot be set to 250 and it is unfit standalone. `golangci-lint` carries the same funlen analyzer and configures it — the same verdict, for the same reason, that `magic-numbers-go` already records for `mnd`.

    funlen's source also shows two traps: `stmtLimit == 0` means "use the default of 40", not "off", and the statement check `continue`s past the line check. The statement dimension therefore has to be set past every real function. The largest statement count in the 94774-function corpus is 6400, `rewriteValueAMD64` in the Go compiler, so 10000 turns it off.

    ## Dart

    No tool rule. DCM is commercial. This goes in VALIDATOR.md so a reviewer does not file it as a gap.
  timestamp: 2026-08-08T16:09:48.312317+00:00
- actor: claude-code
  id: 01kzh3dwpdxwrskfem8vv3cprp
  text: |-
    Implementation landed and green. Four rules, eight fixtures, three rosters, VALIDATOR.md.

    ## What shipped

    - `rules/complexity-typescript.md` — files scope, one eslint run, temporary flat config through `--config` and `--no-config-lookup`, `sonarjs/cognitive-complexity` at 15 plus core `max-lines-per-function` at 250 with `skipBlankLines`/`skipComments`. `supersedes: [cognitive-complexity, function-length]`. `install.commands` pins `eslint@10.8.0 typescript-eslint@8.66.0 typescript@5.9.3 eslint-plugin-sonarjs@4.2.0`.
    - `rules/complexity-swift.md` — files scope, one swiftlint run, temporary `.swiftlint.yml` through `--config`, `--reporter json` through jq. `cyclomatic_complexity` at `warning: 15` with `ignores_case_statements: true`, and `function_body_length` at `warning: 250`. `supersedes: [cognitive-complexity, function-length]`.
    - `rules/complexity-go.md` — files scope, `gocognit -over 15 -json` through jq. `supersedes: cognitive-complexity`. Pins `gocognit@v1.2.1`.
    - `rules/function-length-go.md` — workspace scope, `funlen` through golangci-lint at `lines: 250`, `ignore-comments: true`, `statements: 10000`. `supersedes: function-length`. Pins the same `golangci-lint@v2.12.2` `magic-numbers-go` already pins.
    - Eight `.tmpl` fixtures. Each fail fixture trips every gate its rule decides, and each pass fixture holds the same shapes under them.
    - `builtin/mod.rs`, `review/tool_rules.rs` and `crates/mirdan/src/builtin_validators.rs` carry the four new rules and the eight new fixture names.
    - `VALIDATOR.md` — the complexity section is rewritten for seven tool rules, split into the three languages that settle both gates in one run and the two that take one rule for each gate, and it states which of the five complexity gates are the published Sonar metric. A new `### DCM — rejected` entry records the Dart verdict.

    ## The two decisions the card left open

    **Swift `ignores_case_statements: true`.** Measured, not assumed. Over 893 `.swift` files, at the gate of 15 the plain form reports 23 findings and 21 of them are flat `switch` dispatch tables — the exact shape the `cognitive-complexity` prompt rule says counts once and carves out. The option costs an under-count (a `switch` drops to zero, not one), which the rule body states plainly.

    **Go function length: `funlen` through golangci-lint.** Three candidates measured against 94774 real Go functions. funlen's line count with comments ignored tracks true code lines at a median ratio of 1.002 and gives 39 false positives at a gate of 250; revive's `function-length` counts raw physical lines and gives 138; the statement count spans 80x and cannot stand for a line count at all. The standalone `funlen` binary has no threshold flags — `funlen -flags` lists nine flags and none of them is `lines` — so golangci-lint runs it, the same verdict `magic-numbers-go` records for `mnd`.

    ## Doctor rows

    `sah doctor` in a scratch project carrying `package.json`, `go.mod` and `Package.swift` — project types `go, nodejs, swift`:

    ```
    ✓ code-hygiene/complexity-go        tool present (github.com/uudashr/gocognit v1.2.1); fixtures pass
    ✓ code-hygiene/complexity-swift     tool present (0.65.0); fixtures pass
    ✓ code-hygiene/complexity-typescript tool present (v10.8.0); fixtures pass
    ✓ code-hygiene/function-length-go   tool present (golangci-lint 2.12.2 ...); fixtures pass
    ```

    The scratch project was needed because `~/.validators/code-hygiene/` holds an older `sah init` snapshot that shadows the builtin set by the documented precedence, so `sah doctor` in this repository reports the old roster. That is not a defect and nothing was changed in the user's home; the scratch project carries a `./.validators` copy of `builtin/validators`, which wins over both.

    ## RED verified fifteen ways, then GREEN restored

    The roster edits landed first and failed before the rules existed ("code-hygiene should carry exactly its prompt and tool rules, left: 22, right: 26"). Then each gate was broken on purpose and the fixture test watched to fail. Every rule file was restored byte for byte after each break.

    | # | break | failure |
    |---|---|---|
    | 1 | TS both gates raised out of reach | fail fixture `complexity-typescript.fail.ts.tmpl` produced no findings |
    | 2 | TS complexity gate 15 -> 2 | pass fixture produced 2 findings |
    | 3 | TS length gate 250 -> 200 | pass fixture produced 1 finding |
    | 4 | Swift both gates raised out of reach | fail fixture produced no findings |
    | 5 | Swift complexity gate 15 -> 3 | pass fixture produced 1 finding |
    | 6 | Swift length gate 250 -> 200 | pass fixture produced 1 finding |
    | 7 | Go `-over 15` -> `-over 1000` | fail fixture produced no findings |
    | 8 | Go `-over 15` -> `-over 2` | pass fixture produced 2 findings |
    | 9 | Go `lines: 250` -> `10000` | fail fixture produced no findings |
    | 10 | Go `lines: 250` -> `200` | pass fixture produced 1 finding |
    | 11 | Swift `--config` dropped from the run | pass fixture produced 1 finding |
    | 12 | Go length `--config` dropped from the run | fail fixture produced no findings |
    | 13 | TS `--config` dropped from the run | fail fixture produced no findings |
    | 14 | `- function-length` deleted from `complexity-swift` | supersedes assertion: left `[cognitive-complexity]`, right `[cognitive-complexity, function-length]` |
    | 15 | `complexity-go` supersedes changed to `missing-docs` | roster assertion: `complexity-go must supersede ["cognitive-complexity"]` |

    Breaks 11 to 13 are the proof that each temporary config reaches its tool: drop the `--config` flag and the tool falls back to its own defaults or to none, and the pair fails either way.

    Each gate was also isolated at script level before the tests ran, by materializing the whole `fixtures/` directory the way doctor does and running each rule's script over it by hand.

    ## Notes for the next agent

    - `gocognit -json` prints `null`, not `[]`, for an empty result. The pipe reads `(. // [])[]` for that reason. `missing-docs-go` already had the same problem with revive.
    - `funlen`'s `stmtLimit == 0` means "use the default of 40", not "off", and the statement check `continue`s past the line check. Setting `statements: 10000` is what makes the line gate the gate; the largest statement count in a 94774-function corpus is 6400.
    - A swiftlint threshold rule given only `warning:` also disables its default `error:` level. Without that, `function_body_length` would report against its default error level of 100.
    - Two pre-existing, gitignored directories were found and left alone: `./crates/swissarmyhammer-mcp-proxy/.kanban` (the ^3dfhnxg card already noted it) and `./.validators`, which holds only a `.gitignore` dated 5 July and therefore shadows nothing.

    ## GREEN

    - `swissarmyhammer-validators` lib: 526 passed, 0 failed.
    - `mirdan`: 501 + 2 passed, 0 failed, 1 ignored.
    - `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean on both crates.
  timestamp: 2026-08-08T16:31:19.757518+00:00
- actor: claude-code
  id: 01kzh3e9b9hcvr6a1arpxh7awz
  text: |-
    ### implement — changed
    - evidence: 16 files — 4 new rules (`complexity-typescript.md`, `complexity-swift.md`, `complexity-go.md`, `function-length-go.md`), 8 new fixtures (`complexity-typescript.{fail,pass}.ts.tmpl`, `complexity-swift.{fail,pass}.swift.tmpl`, `complexity-go.{fail,pass}.go.tmpl`, `function-length-go.{fail,pass}.go.tmpl`), `builtin/validators/code-hygiene/VALIDATOR.md`, `crates/swissarmyhammer-validators/src/builtin/mod.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules.rs`, `crates/mirdan/src/builtin_validators.rs`. Tests: 526 passed in the `swissarmyhammer-validators` lib, 503 passed in `mirdan`, 0 failed. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean on both crates. RED verified 15 ways before GREEN; `sah doctor` shows all four rules "tool present; fixtures pass" in a go/nodejs/swift project.
    - next: `/review`. Tools installed for the work: `eslint-plugin-sonarjs@4.2.0` (npm -g), `gocognit@v1.2.1` and `funlen@latest` (`go install`).
  timestamp: 2026-08-08T16:31:32.713823+00:00
- actor: claude-code
  id: 01kzh3q430d8waa6j5r5gcvvrg
  text: |-
    ### test — green
    - evidence: cargo fmt --check clean; cargo clippy --workspace --all-targets -- -D warnings clean; cargo nextest run --workspace — 13862 tests run: 13862 passed (7 slow), 0 failed, 0 skipped
    - next: none
  timestamp: 2026-08-08T16:36:22.240429+00:00
- actor: claude-code
  id: 01kzh3zfp6dsfdkvfj0mn265v8
  text: |-
    ### review — clean
    - evidence: `review sha 23dc610f7..5b026d3b5` (18 files) — findings 0, confirmed 0, refuted 0; 9 validator/file pairs attempted, 0 failed, 0 skipped. No prior Review Findings section, no unchecked items.
    - next: none. Task moved to done.
  timestamp: 2026-08-08T16:40:56.262285+00:00
- actor: claude-code
  id: 01kzh403yrs0jkrgzywdkexmcf
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 16 files; 4 rules (complexity-typescript, complexity-swift, complexity-go, function-length-go), 8 fixtures, VALIDATOR.md, 3 rosters. RED verified 15 ways: 10 threshold breaks, 3 dropped-config breaks, 2 supersedes breaks
    - test: green — cargo nextest run --workspace, 13862 passed, 0 failed, 0 skipped; cargo fmt --check clean; cargo clippy --workspace --all-targets -- -D warnings clean
    - commit: 5b026d3b5 — 18 files; staged by explicit path because a parallel session shares this tree
    - review: clean — 0 findings, 9 pairs attempted, 0 failed, 0 skipped on `review sha 23dc610f7..5b026d3b5`
    - result: task moved to done

    Tools installed during the work: eslint-plugin-sonarjs@4.2.0 (npm -g), gocognit v1.2.1 and funlen v0.2.0 (go install). swiftlint 0.65.0 and golangci-lint 2.12.2 were already present.

    Note for later cards: `~/.validators/code-hygiene/` holds an older `sah init` snapshot that shadows the builtins by the documented precedence, so doctor rows for new builtin rules must be checked in a scratch project until that snapshot is refreshed.
  timestamp: 2026-08-08T16:41:17.016722+00:00
depends_on:
- 01KZEBACVE127AV1BTD3DFHNXG
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffcc80
title: 'complexity tool rules: TypeScript + Swift + Go'
---
Extend the complexity and function-length tool rules to the other languages. Follow the pattern from ^3dfhnxg.

TypeScript/JavaScript — eslint, files scope:
- eslint-plugin-sonarjs `cognitive-complexity` at 15 (the same Sonar metric the tree-sitter probe computes) plus core `max-lines-per-function` at 250 with skipBlankLines and skipComments.
- The run script writes a temporary flat config and passes `--config <tmp> --no-config-lookup`. Pin the eslint and plugin versions in `install.commands`.
- One run, `supersedes: [cognitive-complexity, function-length]`.

Swift — swiftlint, files scope:
- `cyclomatic_complexity` and `function_body_length` in one run.
- The run script writes a temporary `.swiftlint.yml` and passes `--config`. Use `--reporter json` piped through jq.
- `supersedes: [cognitive-complexity, function-length]`.

Go — files scope:
- `gocognit -over 15` for complexity (Sonar cognitive metric). Supersedes `cognitive-complexity`.
- Pick a function-length tool during the work. If no standalone tool is healthy, Go keeps the `function-length` prompt rule. Record the decision in the rule body.

Dart — no tool rule. DCM (dart_code_metrics) is commercial. Dart keeps the probe + prompt path. State this in the code-hygiene VALIDATOR.md so a reviewer does not file it as a gap.

Every new rule ships a fail/pass fixture pair and shows doctor rows.

#tool-validators