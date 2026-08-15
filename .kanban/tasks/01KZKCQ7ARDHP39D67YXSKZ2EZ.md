---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m01nd6etzb8vj1f2g3penjyr
  text: |-
    Picked up. Survey in progress. First measurements, Dart SDK 3.11.0 / Flutter 3.41.2.

    `solid_lints` 0.3.3 (already the tool behind `magic-numbers-dart`) carries BOTH candidate rules: `cyclomatic_complexity` and `function_lines_of_code`. Each takes a gate parameter and an `exclude` list of declaration names.

    The `custom_lint` 0.8.1 config shape is FLAT, not nested. `configs.dart` reads `item.keys.first` as the rule name and `item.entries.skip(1)` as the parameters, so

        - cyclomatic_complexity:
          max_complexity: 15

    is correct and

        - cyclomatic_complexity:
            max_complexity: 15

    silently falls back to the DEFAULT gate. Measured: the nested form reported against 10 and 200; the flat form reported against 15 and 250. The threshold value is written into the problem message, so a script can verify the config was read.

    `cyclomatic_complexity` in 0.3.3 is BROKEN and cannot carry the gate. Its `run` registers `addDeclaration` INSIDE the `addBlockFunctionBody` callback, so a body's own listener is added part way through the AST walk and then fires once for every Declaration node visited AFTER it, each time re-measuring the captured body. Measured over one file holding five functions of 16 `if` statements each:

    | function | position | findings |
    |---|---|---|
    | alpha | first | 12 |
    | bravo | second | 10 |
    | charlie | third | 8 |
    | delta | fourth | 6 |
    | echo | fifth | 4 |
    | andor, 32 entities | last, holds `var t = 0` | 1 |
    | switchy, 31 switch arms | sixth | 0 |

    The count is the number of declarations that follow the function, so it is a fact about file layout and not about the code. The fatal row: a file whose ONLY declaration is one function of 20 `if` statements reports 0 findings; adding `int trailing(int x) => x;` after it makes the same function report 1. A dirty file reads as clean.

    Two more gaps in the same rule: it registers `addBlockFunctionBody` alone, so an expression-bodied (`=>`) function is never measured; and `visitSwitchCase` never fires on Dart 3 source, because `case 0:` parses as `SwitchPatternCase`, so `switchy` above scores 1.

    `function_lines_of_code` has NO such defect. It registers `addFunctionDeclaration`, `addMethodDeclaration` and `addFunctionExpression` at the top level of `run`, and its visitor counts DISTINCT line numbers holding a non-synthetic token inside the body — blank lines and comments excluded, which is the `function-length` prompt rule's definition word for word. Measured on a probe of 300 code lines interleaved with comment-only and blank lines: it reports the code lines. It reads a named function, a method, a getter and an anonymous closure, and it is silent on a constructor.

    Next: measure `dart_code_linter` 4.1.9 (the maintained fork of the discontinued `dart_code_metrics`) for the complexity gate.
  timestamp: 2026-08-15T02:53:22.266187+00:00
- actor: claude-code
  id: 01m01r06mk56r372kvwp82z72n
  text: |-
    ## The survey, tool by tool, with the version tested

    Dart SDK 3.11.0, Flutter 3.41.2, macos_arm64.

    | Tool | Version | Complexity | Function length | Verdict |
    |---|---|---|---|---|
    | `dart analyze` built-in linter | SDK 3.11.0, ~360 rules | no | no | no metric layer at all |
    | `lints` | 6.1.0 (2026-01-30) | no | no | a selection from the SDK rules |
    | `flutter_lints` | 6.0.0 (2025-05-27) | no | no | a selection from the SDK rules |
    | `very_good_analysis` | 10.3.0 | no | no | zero dependencies, config bundle only |
    | `lint` | 2.8.0 | no | no | config bundle only |
    | `altive_lints` | 4.1.0 | no | no | config bundle only |
    | `pedantic_mono` | 1.38.1 | no | no | depends on `flutter_lints` alone |
    | `flutter_lint_rules` | — | — | — | does not exist on pub.dev |
    | `dart_code_metrics` | 5.7.6 (2023-07-16) | yes | yes | DISCONTINUED; pins `analyzer >=5.1.0 <5.14.0`; superseded by the fork below |
    | `dcm` (dcm.dev) | 1.38.3 | yes | yes | commercial; CI use documents `--ci-key`/`DCM_CI_KEY`, so it cannot run unattended on a fresh machine. The VALIDATOR.md rejection stands. |
    | `solid_lints` | 0.3.3 (2025-12-05) | `cyclomatic_complexity` | `function_lines_of_code` | complexity rule is BROKEN, see the comment above |
    | `dart_code_linter` | **4.2.0 (2026-08-11)** | `cyclomatic-complexity`, `maximum-nesting-level` | `source-lines-of-code` | the maintained MIT fork of `dart_code_metrics`, by Bancolombia; `analyzer >=10.0.0 <15.0.0`, resolves analyzer 14.1.0 |
    | SonarQube Dart analyzer | current | yes, incl. cognitive | no per-function length rule | needs a server plus `sonar-scanner` |
    | `lizard` | current | no Dart parser | no | 26 languages, Dart is not one |
    | PMD | current | Dart is CPD-only | no | no Dart rule engine |
    | `scc` / `tokei` / `cloc` | current | no | file-level lines only | no function boundaries |
    | `semgrep` | Dart support beta | no | no | pattern matcher, no metric aggregation |

    `dart_code_linter` 4.2.0 also ships 88 lint RULES. `avoid_nested_conditional_expressions` is the only one in the neighbourhood, and it reads nested ternaries rather than depth. Complexity and length are METRICS, not rules.

    ## `dart_code_linter` 4.2.0 is correct where `solid_lints` is not

    `dart run dart_code_linter:metrics analyze` takes every threshold as a CLI flag, writes JSON, and reports each function ONCE. Measured over the file that defeated `solid_lints` — one function of 20 `if` statements as the only declaration in the file — it reports it at 21. It reads a named function, a method, a getter, a CONSTRUCTOR and an expression-bodied (`=>`) function, all of which `solid_lints` misses or mis-attributes. Warm run: 0.79 s over a probe, 44 s over the 3931-file corpus below.

    Exit codes, measured: 0 ran clean, 2 ran with a finding under `--set-exit-on-violation-level=warning`, 64 usage error, 1 could not write the report. Findings are `value > threshold`; the level bands are `> t*2` alarm, `> t` warning, `> t*0.8` noted, so `noted` is a near-miss and not a finding.

    ## The corpus

    `dart-lang/http` at `a9176ac`, `dart-lang/shelf` at `fb3f931` and `flutter/packages` at `a3e763e` — 3931 `.dart` files copied into one probe package, 3630 files carrying 63241 functions. Every sweep below is arithmetic on the tool's own per-function numbers.

    ## `source-lines-of-code` IS the length gate

    It counts distinct lines holding a token inside the body — blank lines and comment-only lines excluded, which is the `function-length` prompt rule's definition word for word. Measured on a probe of 302 code lines interleaved with 60 blank and 60 comment-only lines: it reports 302.

    | gate | findings | in test files | on `main` in a test file |
    |---|---|---|---|
    | 100 | 966 | 589 | 561 |
    | 150 | 648 | 485 | 465 |
    | 200 | 468 | 421 | 409 |
    | 250 | 400 | 376 | 369 |
    | 300 | 352 | 336 | 330 |
    | 400 | 296 | 287 | 282 |

    At 250 — the number the prompt rule states — the corpus reports 24 findings outside test files, and each one is a true long function: `GoogleFonts.asMap` 1895 lines, `_tokenize` 1219, `ThemeData.debugFillProperties` 633, `_InputDecoratorState.build` 340.

    ## `cyclomatic-complexity` is REJECTED, and this is the measurement

    `cognitive-complexity` exempts "Configuration parsing with many options, where the score comes from a long flat list of simple cases rather than from nesting". Dart's dominant idioms are exactly that list: a `copyWith` of N optional parameters writes N `??` operators, an `==` writes N `&&`, and a `lerp` writes N ternaries. Cyclomatic complexity charges one for each.

    At the gate of 15 the corpus reports 356 findings outside test files, and **188 of them stand at nesting level 2 or less** — flat. `InputDecoration.copyWith` scores 59 at nesting 1 and is 59 named parameters each defaulted with `??`. `ThemeData.==` scores 85, `DatePickerThemeData.lerp` 87, `ButtonStyle.lerp` 61. The published Sonar cognitive metric the `complexity` probe computes scores a sequence of `&&` once, not once per operator, so it would rate these near zero.

    No threshold separates them: the flat shapes run to 149. Under this set's contract a tool finding is a requirement, so shipping this gate would make 188 suppressions mandatory on code the prompt rule calls correct. That is the trade `complexity-swift` refused for `switch` arms and `function-length-go` refused for test paths.

    ## `maximum-nesting-level` is REJECTED, and this is the measurement

    The metric names every construct that raised the depth, so the chain can be read. Measured: a widget tree 3 constructors deep scores 1, a collection literal 4 deep scores 1, and a flat `if`/`else if` chain scores 2 — all correct. But **every closure body raises the depth by one**, and Dart is closure-heavy.

    The prompt rule's gate is CONDITION-nesting depth 4 or more, which maps to a metric value over 4. At that gate, outside test files:

    | tool nesting > 4 | of those, condition-depth >= 4 |
    |---|---|
    | 229 | 98 |

    **131 of 229 findings (57%) come from closure bodies rather than conditions**, and 34 of them have condition-depth 0 or 1 — `_TabScaffoldExampleState.build` reaches level 6 through nested builder callbacks with no condition at all. Same verdict, same reason.

    ## Verdict

    - `function-length` CAN be superseded for Dart, by `dart_code_linter` 4.2.0 `--source-lines-of-code=250`.
    - `cognitive-complexity` CANNOT be superseded for Dart. Neither metric reproduces either of its two gates without a large population of findings the rule itself carves out.

    So the card's shape — one `complexity-dart.md` naming both prompt rules — is refuted by measurement. Shipping `function-length-dart.md` instead, superseding `function-length` alone, which is the shape `function-length-go` and `function-length-python` already take. Dart keeps the `complexity` probe and the `cognitive-complexity` prompt rule.
  timestamp: 2026-08-15T03:38:42.195177+00:00
- actor: claude-code
  id: 01m01snyc31evyc0fk0r11t3fa
  text: |-
    ## What shipped, and how it deviates from the card

    The card orders one `complexity-dart.md` declaring `supersedes: [cognitive-complexity, function-length]`. **That file was NOT written**, and the two comments above are the measurement that refutes it: neither `cyclomatic-complexity` nor `maximum-nesting-level` reproduces either gate of `cognitive-complexity` without a large population of findings that rule itself carves out — 188 of 356, and 131 of 229.

    What shipped instead is `function-length-dart.md`, superseding `function-length` alone, which is the shape `function-length-go` and `function-length-python` already take. The card's "Done when" is met on both branches: Dart gets the length gate from a tool, and the card and `VALIDATOR.md` record why no tool can give the branching gate.

    ## The rule

    `dart_code_linter` 4.2.0, `metrics analyze --source-lines-of-code=250`, `scope: files`. The script builds a probe package in a temporary directory, so the project's own `analysis_options.yaml` is never read — measured, a project stating `dart_code_linter: metrics-exclude: ["lib/**"]` takes the run to 0 findings at exit 0, and one stating `analyzer: exclude: ["lib/**"]` leaves a report of 0 bytes.

    Five silent-zero shapes were found and each one is tested for. A file that is not valid UTF-8, and a file that does not parse at all, each give ONE record with `functions: {}` at exit 0 — the answer of a clean file. A file under a dot directory gets NO record, because `dart_code_linter` skips every such path, and `flutter/packages` really does carry Dart under `camera_android_camerax/.agents/skills/`. A directory holding no Dart file writes a report of 0 bytes at exit 0.

    The syntax test is an INTERSECTION rather than a search: it breaks only on a file that both measured no function AND carries a `SYNTACTIC_ERROR`. An earlier shape that broke on any `SYNTACTIC_ERROR` reported 0 findings and exited 1 over the whole of `flutter/packages`, because that repository turns on the experimental `private-named-parameters` feature — and `dart_code_linter` measures all 72 functions of the file either way.

    ## Two defects found and fixed on the way

    The probe's `environment: sdk:` LOWER bound is the package's LANGUAGE VERSION. A fixed floor of `>=3.5.0` made `dart analyze` write `This requires the 'dot-shorthands' language feature to be enabled` over `flutter/packages`, and the whole run reported 0 findings. The script now reads the version out of `dart --version` and writes `sdk: '^3.11.0'`. **The two shipped sibling Dart rules have the same defect**, and it is now card `^hc2pcyp` with the measurement on it.

    The probe copied each file to a path mirroring its source path, and the two files under `.agents/skills/` then reached the "no record" test and broke a run that was otherwise clean. Each file now goes to a flat `lib/probe_N.dart` with a table the findings map back through.

    ## The measurement on a real repository

    The shipped script over every `.dart` file of `flutter/packages` at `a3e763e` — 3508 files: **22 findings in 55 s**, exit 0. Every one is true as a measurement. 9 of the 22 are shapes the PROMPT rule carves out and the tool does not — one test registered under `lib/`, two `test_fixes/` data files, two generated localization tables, two generated font maps, and two widget-definition data tables. The other 13 are long procedural declarations the prompt rule lists too. The rule file names all 22 groups and the annotation an author writes.

    ## Files

    - `builtin/validators/code-hygiene/rules/function-length-dart.md`
    - `builtin/validators/code-hygiene/fixtures/function-length-dart.{fail,pass}.dart.tmpl` — four declaration kinds each, at 252/253 over the gate and 247/248 under it
    - `builtin/validators/code-hygiene/VALIDATOR.md` — the survey, and the DCM verdict corrected: it claimed DCM is the only Dart tool that computes these metrics, which is wrong
    - `crates/swissarmyhammer-validators/src/builtin/mod.rs`, `.../tool_rules/tests.rs`, `.../tests/shipped.rs`, `.../shipped/function_length_dart.rs`, and the counts in `scope_roster.rs` (26→27), `zero_argument.rs` (15→16) and `temp_directory.rs` (21→22)
    - `crates/mirdan/src/builtin_validators.rs` — the fixture install roster

    `crates/mirdan/retired-validators/` is untouched.

    ### implement — changed
    - evidence: 12 files — 4 new, 8 modified. `cargo nextest run -p swissarmyhammer-validators -p mirdan`: 1284 passed, 0 failed. `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /review. The card's `complexity-dart.md` was not written; the reason is measured above and needs a human decision on whether the card is satisfied.
  timestamp: 2026-08-15T04:08:03.203169+00:00
- actor: claude-code
  id: 01m01vvz3d4gcrfzvra24b7ymy
  text: |-
    ### review — clean

    - scope: `review sha 5df34d385~1..5df34d385` (new engine, card ^apb04az)
    - evidence: 0 findings, 0 confirmed, 0 refuted, 9 validators attempted, 0 failed, 0 skipped. 8 files reviewed, 2 excluded.
    - fixtures: the engine excluded `function-length-dart.fail.dart.tmpl` and `function-length-dart.pass.dart.tmpl` by category ("validator fixture"). The deliberate defect in them was never a candidate.
    - coverage limit (^j169agt): no validator declares a `*.md` glob. The 9 validators that ran are exactly those matching `*.rs`, and the 8 files reviewed are exactly the 8 `.rs` files. The 538-line rule body `builtin/validators/code-hygiene/rules/function-length-dart.md` and the 101 changed lines of `builtin/validators/code-hygiene/VALIDATOR.md` matched zero validators and were NOT read. The clean verdict covers the Rust wiring only — not the rule script, thresholds, `supersedes` list, or prose.

    ### roster audit — one stale comment, OFF-DIFF

    Audited every roster and count that should carry the new rule. All carry it, and every stated total is consistent:

    - `builtin/mod.rs` `CODE_HYGIENE_COMPLEXITY_TOOL_RULES` — entry present; by-language order, new entry last per convention.
    - `tool_rules/tests.rs` `SHIPPED_COMPLEXITY_RULES` — present, `project_types: [flutter]` matches the rule.
    - `tests/shipped.rs` — `mod function_length_dart;` in correct alphabetical position.
    - `scope_roster.rs` — `SHIPPED_TOOL_RULE_COUNT = 27` verified; `WORKSPACE_SCOPE_RULE_COUNT = 11` correctly unchanged (the Dart rule is `files` scope).
    - `zero_argument.rs` — `FILES_SCOPE_RULE_COUNT = 16` verified; the 6 + 10 prose split verified against all 16 scripts. This commit CORRECTED a pre-existing wrong 7/8 split.
    - `temp_directory.rs` — `TEMP_DIRECTORY_RULE_COUNT = 22` verified.
    - `mirdan/src/builtin_validators.rs` — both fixtures listed; the rule file correctly absent from the prompt-rules-only list.
    - `VALIDATOR.md` — "eight tool rules" verified as 8; Dart length gate recorded; complexity rejection recorded with measurements.

    NOT a finding for this scope, but real and worth a card: `crates/swissarmyhammer-validators/src/builtin/mod.rs:306-308` doc comment still reads "Dart keeps the `complexity` probe and both prompt rules, because its only metrics tool is commercial." Both clauses are now false — Dart supersedes `function-length` (line 326), and `dart_code_linter` 4.2.0 is MIT. The identical sibling comment WAS corrected at `tool_rules/tests.rs:330-336`; this second copy was missed. The commit's only edit to `builtin/mod.rs` is the single added line 326, so the stale comment is on an UNCHANGED line and is out of scope for a diff op — the engine correctly did not report it.

    - next: file a card to fix the `builtin/mod.rs` doc comment to match the corrected wording in `tests.rs`. Separately, ^j169agt leaves every shipped `.md` rule body unreviewed.
  timestamp: 2026-08-15T04:46:17.709512+00:00
- actor: claude-code
  id: 01m01vy0xmcw3sacasyr2qfwc4
  text: |-
    ### finish iteration 1 — clean. Card done, with one deviation for a person to judge.
    - **The survey found a tool the rule set had written off**: `dart_code_linter` 4.2.0, published 2026-08-11, Bancolombia, MIT — a maintained free fork of the discontinued `dart_code_metrics` on a current analyzer, computing all three metrics with every threshold as a CLI flag. That directly corrects VALIDATOR.md's claim that commercial DCM was the only Dart tool computing them.
    - Measured all three metrics over 3931 .dart files and 63241 functions (dart-lang/http, dart-lang/shelf, flutter/packages).
    - **`source-lines-of-code` IS the length gate** — it counts exactly what `function-length` defines, blanks and comments excluded. Shipped at 250.
    - **Cyclomatic complexity REJECTED on measurement**: at a gate of 15, 188 of 356 non-test findings sit at nesting level ≤ 2 — flat `??`/`&&` chains in `copyWith`, `==` and `lerp`, with `InputDecoration.copyWith` scoring 59 at nesting 1. Those are precisely the "long flat list of simple cases" `cognitive-complexity` carves out, and NO threshold separates them, since the flat shapes run to 149.
    - **Maximum nesting level REJECTED too**: every closure body raises the depth by one and Dart is closure-heavy, so at the prompt rule's own gate 131 of 229 non-test findings come from closures rather than conditions.
    - **DEVIATION FROM THE CARD, for a person to judge.** The card specifies `complexity-dart.md` superseding BOTH prompt rules. This ships `function-length-dart.md` superseding `function-length` alone — the shape function-length-go and function-length-python already take. Shipping either complexity gate would make hundreds of suppressions mandatory on code the prompt rule calls correct, the trade complexity-swift and function-length-go each explicitly refused. The card's done-when allows "records why no tool can give them"; this is the in-between case — one gate from a tool, and a measured reason the other cannot be given.
    - **Two defects surfaced while building it.** The probe package's `environment: sdk:` LOWER BOUND is the language version, and a fixed floor made a whole 3508-file run report 0 findings; the script now derives it from `dart --version`. The two shipped sibling Dart rules carry the same defect — filed as ^hc2pcyp. And `dart_code_linter` silently skips any file under a dot directory, so probe copies use flat names with a mapping table.
    - Real-repository measurement: 3508 files of flutter/packages, 22 findings in 55s at exit 0. All 22 are true measurements; 9 fall under a prompt-rule carve-out the tool cannot express, and the rule names each one.
    - test: green — 1284 validators + mirdan tests. fmt and clippy clean.
    - commit: 5df34d385
    - review: clean — 0 findings, 9 attempted, 0 failed, 8 files reviewed, 2 fixtures excluded by category.

    **Rosters verified against actual entries rather than taken on faith**: `SHIPPED_TOOL_RULE_COUNT` 27, `FILES_SCOPE_RULE_COUNT` 16 with its 6+10 prose split — which this commit CORRECTED from a pre-existing wrong 7/8 — `TEMP_DIRECTORY_RULE_COUNT` 22, `WORKSPACE_SCOPE_RULE_COUNT` 11 correctly left alone since the Dart rule is files-scoped, and VALIDATOR.md's "eight tool rules" which is genuinely 8. mirdan lists both fixtures and correctly omits the rule from its prompt-rules-only list.

    **One staleness found and deliberately NOT forced into this card**: `builtin/mod.rs:306-308` still says Dart "keeps both prompt rules, because its only metrics tool is commercial" — both clauses now false. It sits on an unchanged line, so it is off-diff, and the engine refutes off-diff candidates before they reach the report; its silence was correct rather than a miss. Manufacturing an out-of-scope finding would corrupt the contract ^apb04az established. Carded separately instead.

    **Scope caveat, and it dominates here**: this commit's entire payload is a 538-line `.md` rule body plus 101 changed lines of VALIDATOR.md, and NONE of it was read — the 9 validators attempted are precisely those matching `*.rs`, and the 8 files reviewed are precisely the 8 `.rs` files. The clean verdict covers the Rust wiring only. It says nothing about whether the shell script is correct, the thresholds right, or `supersedes` naming the right rules. See ^j169agt.
  timestamp: 2026-08-15T04:47:25.108566+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff8c80
title: 'dart goes objective: complexity and function-length tool rules'
---
Dart is the only language with no complexity gate and no function-length gate. The prompt rules `cognitive-complexity` and `function-length` still apply to Dart files, so an LLM measures and decides.

Rust, Swift and TypeScript each get both gates from one `complexity-<lang>` rule that declares:

```yaml
supersedes:
  - cognitive-complexity
  - function-length
```

Dart needs the same shape, if a tool can do it.

## Survey first

`dart analyze` has no length lint and no complexity lint. Do not stop there. Enumerate the full Dart lint and metric tool space before you report a gap:

- every lint in the `lints` and `flutter_lints` packages
- `dart_code_metrics` — the open source releases, and what the move to DCM changed
- any analyzer plugin that reports cyclomatic complexity or lines per function

Record what you found in the card, tool by tool, with the version you tested.

## Then

If a tool exists, write `builtin/validators/code-hygiene/rules/complexity-dart.md` to the contract in `builtin/validators/README.md`:

- `match.files: ["**/*.dart"]`, `match.project_types: [flutter]`
- `supersedes: [cognitive-complexity, function-length]`
- a `tool.run` shell script that writes its own config to a temp path, never the project's `analysis_options.yaml`
- `doctor` and `install` blocks, with the tool version pinned
- a pass fixture and a fail fixture
- a measurement on a real Dart repository: finding count, run time, and whether every finding is true

If no tool exists, say so with the survey as evidence, and leave Dart on the prompt rules.

## Done when

Dart has both gates from a tool, or the card records why no tool can give them. #tool-validators #objectivity