---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m0nf4kt0a5t808ph6w37cy35
  text: |-
    Research done. Every claim measured on this machine, not read from docs.

    ## Toolchain measured

    | binary | path | version |
    |---|---|---|
    | `swift` | `/usr/bin/swift` | Apple Swift 6.4 (swiftlang-6.4.0.30.4) |
    | `swift format` | toolchain subcommand | reports `main` |
    | `swift-format` | `/opt/homebrew/bin/swift-format` | separate Homebrew copy |
    | `swiftformat` | `/opt/homebrew/bin/swiftformat` | 0.62.1 |
    | `swiftlint` | `/opt/homebrew/bin/swiftlint` | 0.65.0 |

    `swift format` is a subcommand of Apple's swift-format with `dump-configuration`, `format`, `lint` and a `--configuration` option that takes JSON. `swiftformat` is a different binary with `--rules`, `--lint`, `--options`. Confirmed different tools.

    ## The card's command lines: two are measurably wrong

    Probe: a real SPM package declaring `.package(url: "https://github.com/airbnb/swift", from: "1.0.0")`. Resolved to AirbnbSwift 1.2.0. `swift package plugin --list` names `'format' (plugin 'FormatSwift' in package 'AirbnbSwift')`.

    1. **`swift package format --lint` alone FAILS in a non-interactive shell.**
       ```
       error: Plugin 'FormatSwift' wants permission to write to the package directory.
       Stated reason: "Format Swift source files".
       Use `--allow-writing-to-package-directory` to allow this.
       EXIT: 1
       ```
       The plugin declares `permissions: [.writeToPackageDirectory(...)]` unconditionally in its own `Package.swift`, so SwiftPM demands the flag even in lint mode. Every documented line therefore has to carry `--allow-writing-to-package-directory`. The lint run still writes nothing — it prints `(lint mode - no files will be changed.)`.

    2. **The switch is `--target`, not `--targets`.**
       | run | exit |
       |---|---|
       | `... format --lint --target Probe` | 1, reports the real findings |
       | `... format --lint --targets Probe` | 1, `error: unknownError(exitCode: 64)` |

       The plugin source reads `argumentExtractor.extractOption(named: "target")`. `--paths`, `--exclude` and `--swift-version` are all correct as the card states them.

    ## Exit codes: the card's claim CONFIRMED

    | run | exit |
    |---|---|
    | `--lint`, formatting defects | 1, `error: lintFailure` |
    | `--lint`, clean | 0 |
    | fix mode, formatting defects | 0, and the files were rewritten |
    | fix mode, clean | 0 |
    | fix mode, a `print(...)` call (SwiftLint `no_direct_standard_out_logs`) | 1, `error: lintFailure` |
    | `--lint`, the same `print(...)` | 1 |

    Non-zero on any failure in lint mode; in fix mode non-zero only for a SwiftLint rule, because SwiftLint cannot autocorrect it.

    ## The contradiction the parent flagged is REAL

    `swiftformat --options` on 0.62.1 gives the defaults:

    | option | default | what `idioms-swift` pins |
    |---|---|---|
    | `--pattern-let` | `hoist` | `inline` |
    | `--short-optionals` | `preserve-struct-inits` | `always` |
    | `--single-line-for-each` | `ignore` | `convert` |
    | `--guard-like-if-statements` | `preserve` | `convert` |

    Measured, over `if case .some(let inner) = value` — the shape `idioms-swift` asks for:

    - bare `swiftformat` rewrote it to `if case let .some(inner) = value`
    - `idioms-swift` then reports that exact line: `hoistPatternLet: Reposition let or var bindings within pattern.`

    So a developer following the old bare `swiftformat .` line produces code our own review reports, and the tool's own fix walks them straight into the finding. Re-running with `--pattern-let inline --short-optionals always --guard-like-if-statements convert --single-line-for-each convert` left the gate's shape standing untouched.

    ## A SECOND contradiction, in the Airbnb plugin itself

    `airbnb.swiftformat` carries `--pattern-let inline`, `--short-optionals always`, `--single-line-for-each convert` — the same options `idioms-swift` pins. Good.

    But it ALSO carries `--property-types inferred`, which `idioms-swift` deliberately refused and `builtin/validators/swift/rules/idioms.md` names a validator error. Measured, the plugin's fix mode over a struct:

    ```
    -  public var items: [Int] = []          +  public var items = [Int]()
    -  public var table: [String: Int] = [:] +  public var table = [String: Int]()
    ```

    That turns the DO of `idioms.md` into its DON'T, word for word. The partial has to warn about this one line, or the plugin path silently fights our own prompt rule.

    ## Other defects found in the partial

    `swift test --help` states the filter form is `<test-target>.<test-case>` or `<test-target>.<test-case>/<test>`, and it is a regular expression. The partial says `swift test --filter <Suite>/<test>`, which drops the test-target component.

    ## Card overlap

    ^7fgdenq covers the config-detection half of the SAME two lines. Full overlap. This card absorbs it — see the separate comment.
  timestamp: 2026-08-22T19:28:38.208360+00:00
- actor: claude-code
  id: 01m0nf5p8pp1hwcvvyg29h5xy2
  text: |-
    ## Overlap with ^7fgdenq — this card absorbs it

    Both cards rewrite the SAME bullet, `builtin/_partials/project-types/swift.md` Format/Lint lines. They are not adjacent, they are the same edit.

    - ^m7ynz9c (this card) requires "split the two tools onto their own lines and **name the config file each reads**".
    - ^7fgdenq requires "**honor** an existing `.swift-format`/`.swiftformat`, tool defaults otherwise, never create a config as a side effect".

    Naming which config file a tool reads and then not saying to honor it is half a sentence. Doing them as two cards means writing the same two lines twice and risking the second pass contradicting the first.

    **Proposal: ^m7ynz9c absorbs ^7fgdenq.** This card delivers both halves in one edit, plus ^7fgdenq's second checkbox — the content regression test in `crates/swissarmyhammer-project-detection/src/types.rs` beside `spec_partial_matches_key`. ^7fgdenq's acceptance criteria are then all met here and it can be archived.

    One correction to carry over: ^7fgdenq's proposed text says "`swiftformat .` honors it likewise — pick the tool that matches the config file present". Measured, that is not enough. Honoring `.swiftformat` does NOT make bare `swiftformat` agree with our shipped `idioms-swift` gate — the four options that matter (`--pattern-let`, `--short-optionals`, `--single-line-for-each`, `--guard-like-if-statements`) all default the wrong way, and the project config only moves them if the project WROTE them. The absorbed text has to state the options, not only the config file.
  timestamp: 2026-08-22T19:29:13.494692+00:00
- actor: claude-code
  id: 01m0nfn18f2ekncrd1ggyamb7v
  text: |-
    ## What landed

    **`builtin/_partials/project-types/swift.md`** — the two loose lines are gone. In their place:

    - A three-row table naming each tool beside the config file it alone reads: Apple swift-format / `swift format` / `.swift-format` (JSON); SwiftFormat (Nick Lockwood) / `swiftformat` / `.swiftformat`; SwiftLint / `swiftlint` / `.swiftlint.yml`. The line above it states the first two are DIFFERENT programs.
    - Use the tool whose config file the repo already holds, and obey it. Never write a config file as a side effect. Never add style flags that disagree with a config already there. Tool defaults when no config exists.
    - The Airbnb plugin path, preferred when `Package.swift` declares the dependency, with both command lines, the switch list, and the exit codes.
    - Without the plugin: swiftformat 0.62.1+ with the four options spelled out, Apple swift-format with `-s` on the check run, swiftlint with `--fix`.
    - Write the project Swift version in `.swift-version`.

    **`crates/swissarmyhammer-templating/src/resolver.rs`** — `swift_partial_separates_the_two_formatters_and_documents_the_airbnb_plugin`, plus a `SWIFT_PARTIAL` name constant and a `builtin_partial` lookup helper, all inside the existing `#[cfg(test)] mod tests`.

    ## Where the test lives, and why not where ^7fgdenq said

    ^7fgdenq proposed `swissarmyhammer-project-detection`, reading the `.md` off disk through `CARGO_MANIFEST_DIR`. That crate holds only the partial's PATH string and never sees its content, so such a test measures a source file rather than the shipped text.

    `swissarmyhammer-templating` embeds every partial at build time (`build.rs` → `BuiltinGenerator` → `get_builtin_partials()`), and `PromptResolver::load_builtin_partials` registers this one as `_partials/project-types/swift` — the same string `partial!("swift")` builds in project-detection. The test reads that embedded table, so it measures the text an agent actually receives. `build.rs` emits `rerun-if-changed` on the partials directory, so editing the markdown rebuilds and re-checks. This follows the existing `test_load_all_prompts_registers_builtin_partials` precedent in the same module.

    ## Two corrections to the card's own command lines

    The card is the order and both requirements are delivered, but two literals on it do not run. Measured, not read:

    1. `swift package format --lint` alone exits 1 with `error: Plugin 'FormatSwift' wants permission to write to the package directory` in a non-interactive shell. The plugin declares `.writeToPackageDirectory` unconditionally in its own `Package.swift`, so lint mode needs the flag too. Both documented lines therefore carry `--allow-writing-to-package-directory`, and the partial says why. The check run still writes nothing.
    2. The switch is `--target` (singular). `--targets Probe` exits non-zero with `error: unknownError(exitCode: 64)`; the plugin source reads `extractOption(named: "target")`. `--paths`, `--exclude` and `--swift-version` are correct as the card states them.

    The card's exit-code claim is confirmed exactly as written.

    ## Consistency with the two shipped validators

    Both contradictions the parent warned about are real and are now written into the partial:

    - Bare `swiftformat .` fights `idioms-swift`. The four options default the wrong way; a bare run rewrote `if case .some(let inner)` into `if case let .some(inner)` and the gate reported that line.
    - The Airbnb plugin fights `builtin/validators/swift/rules/idioms.md`. Its config carries `--property-types inferred`, and fix mode rewrote `var items: [Int] = []` into `var items = [Int]()` — the DO of that rule turned into its DON'T. The partial carries a caution paragraph for it.

    Airbnb's config otherwise carries `--pattern-let inline`, `--short-optionals always` and `--single-line-for-each convert`, the same options `idioms-swift` pins, so the plugin path is the most consistent one available.

    ## Rest of the partial

    - ULID guidance verified by building a real package: `from: "1.2.0"` resolves (1.3.1), and `import ULID` + `ULID()` compiles. Left alone.
    - Test filter corrected. `swift test --help` states the form is `<test-target>.<test-case>` or `<test-target>.<test-case>/<test>`, and that it is a regular expression. The old line dropped the target component.
    - Build, run, deps and file-location lines checked and left alone.
  timestamp: 2026-08-22T19:37:36.271596+00:00
- actor: claude-code
  id: 01m0ng392wvh1yxh5agskcrw1y
  text: |-
    ## Rules pass, and what it changed

    Read the full `dump validators` output — 7 validators, 47 rules, for extensions `md` and `rs`. Two rules changed the code after the first draft:

    **`tdd` "One thing. 'and' in name? Split it."** The first draft was one test named `swift_partial_separates_the_two_formatters_and_documents_the_airbnb_plugin`, covering three separate requirements. Split into three:

    - `swift_partial_names_each_formatter_with_its_own_config_file`
    - `swift_partial_documents_the_airbnb_plugin_commands`
    - `swift_partial_agrees_with_the_shipped_swift_tool_validators`

    **`duplication/duplication`: "Two blocks that differ only by a value are one function with an argument."** Splitting one test into three would have triplicated the `for (needle, requirement)` loop and its assert body. Extracted `assert_swift_partial_states(&[(needle, requirement)])`; each test now passes only its own table. That is also the shape `code-hygiene/data-driven` asks for — a table interpreted once, never N parallel assert lines differing only by a constant.

    ## Rules checked and cleared, with the reason

    - **`reuse/reuse` "Reimplements a shared function/library".** Searched first. No helper anywhere fetches one builtin partial's embedded content by name. The nearest thing, `expand_partials` in `crates/swissarmyhammer-skills/tests/common/mod.rs`, expands `{% include %}` tags in a skill body and reads from disk — different operation, different crate, different contract. `reuse` carve-out: "same shape, different domain or contract is not a reuse miss", plus "Single-call-site helpers are not a reuse concern."
    - **`rust/error-handling` "Never panic on expected failure modes (bad input, missing files, network errors)."** This is why the test reads `get_builtin_partials()` and not the `.md` from disk. The embedded table is built by `build.rs`, so a missing entry is an internal invariant violation, which the same rule names as the one legitimate use of panic: "Panics are for bugs only — internal invariant violations." The disk-read approach ^7fgdenq proposed would have panicked on a missing FILE, which is one of the three forbidden modes.
    - **`test-integrity/no-test-cheating`.** No `#[ignore]`, real assertions, exact strings rather than "is not empty". The rule warns that an absence assertion on a string that was never present cannot fail, so that was proved separately — see the RED evidence below.
    - **`completeness/case-sensitivity-coverage`.** The needles are exact literals from a document this repo owns, and the domain is genuinely case-sensitive: `DIFFERENT programs` is deliberately capitalized and the command lines are case-sensitive. A case-sensitive match honors the format's real case contract, which is what the rule asks for.
    - **`completeness/inverse-operation-coverage` check 2.** No test name contains `roundtrip`, `symmetry`, `inverse`, `both directions`, or `read back` — the tests read only, so such a name would lie about scope.
    - **`code-hygiene/magic-numbers`.** Counted the sites, as the rule requires. Every needle appears exactly once, so the "genuinely one-off literals used exactly once" carve-out holds.
    - **`code-hygiene/function-length-rust`.** A `#[test]` is explicitly NOT exempt from the 250-code-line gate, and data lines count. The largest of the three tests is well under it.
    - **`code-hygiene/missing-docs-rust`.** Silent on `#[cfg(test)]` items, so doc comments are not required here — all four new items carry them anyway.
    - **`code-hygiene/dead-code-rust`.** Both helpers have callers; nothing is staged.
    - **`completeness/invariant-propagation` check 1** — "why only `swift.md` when sibling partials exist?" The token this guard protects is the Swift formatting guidance, which exists at exactly one site. No sibling partial documents two different formatters that share a name, so no sibling consumes the same token. The rule's own carve-out: "The token is genuinely handled at only one site (verify by searching, don't assume)." Verified by searching.

    ## RED → GREEN evidence

    | run | result |
    |---|---|
    | all three tests against the ORIGINAL partial | 3 failed, each on its own first requirement: missing `.swift-format`, missing `https://github.com/airbnb/swift`, missing `--pattern-let inline` |
    | new partial with `(or \`swiftformat .\`)` appended | `swift_partial_names_each_formatter_with_its_own_config_file` FAILED on the negative assertion; the other two passed |
    | new partial as shipped | 3 passed |

    The middle row is the one that matters for `no-test-cheating`: the absence assertion is provably able to fail, not a trivial assertion that always holds.
  timestamp: 2026-08-22T19:45:23.036320+00:00
- actor: claude-code
  id: 01m0ng3x4w354qt13vfwz77c3h
  text: |-
    ### implement — changed
    - evidence: 2 files — `builtin/_partials/project-types/swift.md`, `crates/swissarmyhammer-templating/src/resolver.rs`. `cargo nextest run` 14191 tests run, 14191 passed, 0 skipped. `cargo fmt --all -- --check` exit 0. `cargo clippy --workspace --all-targets -- -D warnings` exit 0. Three new tests verified RED against the original partial and GREEN against the new one; the negative assertion verified failing on its own by re-appending the old interchangeable phrasing.
    - acceptance: each tool named once beside its own config file (partial lines 48-50); plugin path documented with real command lines, switches and exit codes (lines 57-74); the DIFFERENT-programs statement stands and the `(or \`swiftformat .\`)` phrasing is gone and guarded by an assertion.
    - next: `/review`. Two literals on the card were corrected against measurement — `swift package format --lint` needs `--allow-writing-to-package-directory` in a non-interactive shell, and the switch is `--target`, not `--targets`. ^7fgdenq is fully satisfied by this work and can be archived.
  timestamp: 2026-08-22T19:45:43.580751+00:00
- actor: claude-code
  id: 01m0ngbeg1w4emfkvwsa38ew4x
  text: |
    ### test — green
    - evidence: `cargo nextest run --workspace` — 14191 tests run, 14191 passed, 0 failed, 0 skipped (baseline 14188 + 3 new). `cargo fmt --all -- --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - confirmation 1 (served partial, not disk file): confirmed from source. `crates/swissarmyhammer-templating/src/resolver.rs` test helper `builtin_partial()` calls `get_builtin_partials()`, the same function `PromptResolver::load_builtin_partials()` calls in production. `get_builtin_partials()` is codegen'd by `crates/swissarmyhammer-templating/build.rs` (`BuiltinGenerator::new("partials").source_dir("../../builtin/_partials")`), embedding the content at compile time. The tests measure the artifact an agent actually receives, not a raw file read.
    - confirmation 2 (tests fail on old partial): spot-checked all three. Reverted `builtin/_partials/project-types/swift.md` to `HEAD` content, ran `cargo nextest run -p swissarmyhammer-templating swift_partial` — all 3 tests FAILED (`swift_partial_names_each_formatter_with_its_own_config_file`, `swift_partial_documents_the_airbnb_plugin_commands`, `swift_partial_agrees_with_the_shipped_swift_tool_validators`), each panic naming the missing requirement (e.g. missing ".swift-format", missing "--pattern-let inline"). Restored the file byte-for-byte (verified via `git diff --stat` matching pre-check state) and re-ran — all 3 PASS. Full workspace suite confirmed green after restore.
    - next: none, ready for review
  timestamp: 2026-08-22T19:49:50.721552+00:00
- actor: claude-code
  id: 01m0ngx89mdmqe2wa4nqjpd6zt
  text: |-
    ### review — findings
    - evidence: 1 finding — crates/swissarmyhammer-templating/src/resolver.rs:572. Engine `review sha HEAD~1..HEAD` returned findings 0, confirmed 0, refuted 0, attempted 7 — but it opened only 1 of the 12 changed files and never read `builtin/_partials/project-types/swift.md`. No validator of the 13 matches `*.md`. The hand review RAN every command line in the partial against Swift 6.4, SwiftFormat 0.62.1, SwiftLint 0.65.0 and airbnb/swift 1.2.0. Every documented claim measured TRUE, the pre-existing ULID and test-command claims included. Both documented contradictions reproduce exactly: a bare `swiftformat .` rewrote `if case .some(let inner)` into `if case let .some(inner)` and the shipped `idioms-swift` gate then reported `hoistPatternLet` on that line; the plugin config carries `--property-types inferred` and its fix mode rewrote `var items: [Int] = []` into `var items = [Int]()`. The one finding is on the added Rust test, not the document.
    - next: couple `swift_partial_agrees_with_the_shipped_swift_tool_validators` to `builtin/validators/code-hygiene/rules/idioms-swift.md` — read the four swiftformat options and the `--min-version` floor out of the shipped rule instead of repeating the literals. A further engine reproduction is recorded on ^dyad426.
  timestamp: 2026-08-22T19:59:34.196556+00:00
- actor: claude-code
  id: 01m0ngz19wnjmbm50hqj1rr8v3
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 2 files; the partial now names each tool with its own config file, and documents the Airbnb plugin path
    - test: green — cargo nextest run --workspace, 14191 passed, 0 failed, 0 skipped; fmt clean; clippy clean; all 3 regression tests verified RED against the old partial
    - commit: d4a6da772
    - review: findings — 1 open
    - engine coverage: 1 of 12 files. **Root cause of ^dyad426 found here** — no validator names `*.md` in its `match_globs`, so markdown cannot enter the pipeline at all.

    **The document itself measured correct in every claim.** The reviewer RAN every command line against Swift 6.4, swift-format `main`, SwiftFormat 0.62.1, SwiftLint 0.65.0 and a real SPM package resolving airbnb/swift 1.2.0:
    - Bare `swift package format --lint` with stdin closed → exit 1, "Plugin 'FormatSwift' wants permission to write to the package directory". With the flag it runs, and `shasum` of all three sources was byte-identical before and after — the check run genuinely writes nothing.
    - `--targets Demo` → "Unknown option", exit 64. `--target Demo` scopes correctly.
    - Exit codes exactly as documented: lint mode exit 1 on 7 SwiftFormat errors; fix mode exit 0 on the same file, then exit 1 once a SwiftLint-only defect was present.
    - Config mappings confirmed, including that a PARENT `.swift-format` is honored (`dump-configuration --effective` showed the parent's `"spaces": 7`).

    **Both documented contradictions reproduce exactly.** Bare `swiftformat .` rewrote `if case .some(let inner)` into `if case let .some(inner)`, and the shipped `idioms-swift` script then reported `hoistPatternLet`. The plugin's built config carries `--property-types inferred`, and its fix mode rewrote `var items: [Int] = []` into `var items = [Int]()`.

    The pre-existing content was checked too, not assumed: the ULID dependency builds, `let id = ULID()` compiles, and the documented `swift test --filter` selected exactly one test.

    **The finding is a coupling defect, and it is the right kind to catch.** `swift_partial_agrees_with_the_shipped_swift_tool_validators` asserts only that six literals appear in the partial. It never reads `idioms-swift.md`, the file its own doc comment names as the thing it agrees with. So the four swiftformat options and the version floor now live in three places with nothing holding them equal — change the gate's `run:` options and the agreement silently breaks while the test stays green.

    That is the same defect class as the probe that passed for the wrong reason on ^qs32yvp: a test whose name claims more than its assertions prove.
  timestamp: 2026-08-22T20:00:32.572163+00:00
- actor: claude-code
  id: 01m0nh8429x811gkq3by784vf6
  text: |-
    ## Research for the open finding

    **The dependency constraint is real. `swissarmyhammer-templating` CANNOT read the shipped rule.**

    `crates/swissarmyhammer-validators/Cargo.toml` line 40 carries `swissarmyhammer-templating = { workspace = true }` as a NORMAL dependency ("Templating / partials for the rules-as-data loader"). `crates/swissarmyhammer-templating/Cargo.toml` names `validators` zero times. So `templating -> validators` is a cycle, and `builtin_validators_dir()` is out of reach from `resolver.rs`. The card says not to force it, so it is not forced.

    **Where the test goes instead: `swissarmyhammer-validators`.** Both sides are already reachable there:

    - the SHIPPED rule, through the loader the engine plans with — `required_shipped_tool_rule(&loader, "idioms-swift").script` in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, which is a better reader than a raw disk read of the `.md`;
    - the SERVED partial, through `swissarmyhammer_templating::resolver::get_builtin_partials()` — `pub fn` inside `pub mod resolver`, generated by `crates/swissarmyhammer-templating/build.rs`. Same table `PromptResolver::load_builtin_partials` serves from, so the "served, not on disk" pattern the two sound tests use is kept.

    All three tests move together. Splitting them would duplicate `SWIFT_PARTIAL`, `builtin_partial` and `assert_swift_partial_states` across two crates for one test.

    **The second half of the same defect.** The finding names the four options and the version floor. The SAME test also holds `--property-types inferred` as a bare literal, and its own doc comment names `builtin/validators/swift/rules/idioms.md` as the thing that is agreed with. That file states the option, the DO (`var items: [Int] = []`) and the DON'T (`var items = [Int]()`) word for word in one bullet, and it is reachable through the existing `swift_prompt_rule_body(&loader, "idioms")` helper. Same cause, same file — so it is read out of the shipped rule too, not hardcoded.

    **The other two tests are sound.** For a document, the presence of the exact command line IS the behaviour. `swift_partial_names_each_formatter_with_its_own_config_file` also carries a negative assertion pinning the removed `(or `swiftformat .`)` regression by name, and that assertion was already proved able to fail.
  timestamp: 2026-08-22T20:05:30.313692+00:00
- actor: claude-code
  id: 01m0nj20f67y0psfkpnqjtbd4m
  text: |-
    ## What landed for the open finding

    **The test now READS the shipped rule. No literal stands in for the real thing.**

    ### The dependency constraint, and where the tests went

    `swissarmyhammer-templating` cannot read a validator. `crates/swissarmyhammer-validators/Cargo.toml` carries `swissarmyhammer-templating` as a NORMAL dependency, so the edge the other way is a cycle. It was not forced.

    All three tests moved to `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs`, where both sides are already reachable. They moved together because splitting them would copy `SWIFT_PARTIAL`, `builtin_partial` and `assert_swift_partial_states` into two crates for one test.

    The SERVED-partial pattern is kept: the module reads `swissarmyhammer_templating::resolver::get_builtin_partials()`, the same table `PromptResolver::load_builtin_partials` serves from. The shipped rule is read the way the ENGINE reads it — `required_shipped_tool_rule(&loader, "idioms-swift").script` and `swift_prompt_rule_body(&loader, "idioms")` — which is a better reader than a raw `.md` read through `builtin_validators_dir()`, because it is the loader that plans the real run.

    ### What the coupling test measures now

    `swift_idioms_agreed_options` reads the `swiftformat --lint` command off the shipped script, joins its continuation lines, and takes every `--option value` pair. Three options are named as the ENGINE's own — `--reporter`, `--cache`, `--rules` — because an author running swiftformat by hand needs none of them. **Every OTHER option is one the guidelines must state.** So a fifth style option added to the gate reaches the test with no edit here; it does not need a list of four names to keep in step.

    The version floor is read from `--min-version` and the partial is held to its VALUE, because the partial advises a minimum in prose rather than writing the flag.

    ### The same defect in the same test, fixed too

    The finding names the four options and the floor. The SAME test held `--property-types inferred` as a bare literal while its own doc comment named `builtin/validators/swift/rules/idioms.md` as the thing agreed with. Same cause. That rule states the option, the DO (`var items: [Int] = []`) and the DON'T (`var items = [Int]()`) in one bullet, so all three are now read out of it. The bullet is found by the option it names, so the DO, the DON'T and the option come off the SAME sentence rather than out of whichever bullet wrote `DO:` first.

    ### RED proof — four ways to break the gate, four failures

    Each mutation was applied, the test run, then the file restored; `git status --porcelain -- builtin/` was empty after each restore.

    | the divergence | the test's answer |
    |---|---|
    | `--pattern-let inline` → `hoist` in the gate `run:` block | FAIL: `must pin every style option the idioms-swift gate's lint command carries (missing "--pattern-let hoist")` |
    | `--min-version 0.62.1` → `0.63.0` | FAIL |
    | a fifth style option `--self remove` added to the lint command | FAIL: `must carry 5 options ... it carries [("--min-version","0.62.1"),("--short-optionals","always"),("--pattern-let","inline"),("--self","remove"),...] left: 6 right: 5` |
    | the DO form of the shipped `idioms` prompt rule renamed | FAIL |

    Restored, all three PASS.

    ## The other two tests — what I found

    **`swift_partial_documents_the_airbnb_plugin_commands` is sound.** Every literal is an external tool's own command line, which this repo owns no second copy of, and the reviewer RAN each one. Nothing here can drift out of step with a repo source of truth.

    **`swift_partial_names_each_formatter_with_its_own_config_file` carried the SAME fault, and it is fixed.** Its name claims the partial names each formatter WITH ITS OWN config file. Its assertions proved only that `.swift-format`, `.swiftformat` and `.swiftlint.yml` each appear SOMEWHERE. A partial whose table swapped `.swift-format` and `.swiftformat` — sending an agent to the wrong config file, which is the exact defect this card exists to stop — held all three names and passed.

    Each needle now carries the command cell and the config cell of ONE table row. Proved: the two config files were swapped in the table with all three names still present; the test FAILED with `must pair Apple's swift-format with the config file it alone reads (missing "`swift format` (in the toolchain) | `.swift-format`")`. Restored, PASS. The test is not weakened and not renamed — the assertions now reach the claim the name already made.

    ## One shared constant hoisted

    `SWIFT_IDIOMS_LINE_JOIN` moved from `shipped/idioms_swift.rs` up to `shipped.rs`, so the roster reader and the lint-command reader share one definition instead of holding two copies.
  timestamp: 2026-08-22T20:19:38.598103+00:00
- actor: claude-code
  id: 01m0nj2d0e2pawehsqggpzd5yd
  text: |-
    ### implement — changed
    - evidence: 4 files — NEW `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs`; `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` (mod line, shared `SWIFT_IDIOMS_LINE_JOIN`, module doc); `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs` (constant hoisted out); `crates/swissarmyhammer-templating/src/resolver.rs` (three tests and their helpers removed). `cargo nextest run` 14191 tests run, 14191 passed, 0 skipped — the same count as before, because the three tests moved rather than being added. `cargo fmt --all -- --check` exit 0. `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
    - RED proof: FIVE divergences each made a test FAIL, and each file was restored byte-for-byte (`git status --porcelain -- builtin/` empty after every restore). Gate `run:` block — `--pattern-let inline` to `hoist`; `--min-version 0.62.1` to `0.63.0`; a fifth style option `--self remove` added. Shipped `idioms` prompt rule — the DO form renamed. Guidelines partial — the two config files swapped in the table with all three names still present, which the OLD assertions passed and the new one fails.
    - dependency constraint: `templating -> validators` IS a cycle. `crates/swissarmyhammer-validators/Cargo.toml` carries `swissarmyhammer-templating` as a normal dependency; `crates/swissarmyhammer-templating/Cargo.toml` names `validators` zero times. Not forced. The tests moved to the validators crate, which already reaches the served partial through `swissarmyhammer_templating::resolver::get_builtin_partials()` and the shipped rules through the engine's own loader.
    - next: `/review`. Both sibling tests were checked for the same fault, as the card asked: the Airbnb-plugin test is sound; the config-file test carried the fault and was strengthened to pair each tool with its own config file, without weakening or renaming it.
  timestamp: 2026-08-22T20:19:51.438882+00:00
- actor: claude-code
  id: 01m0njc1sjf024qssfm98smyes
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 14191 tests run: 14191 passed, 0 skipped (count unchanged as expected, since tests moved not multiplied). `cargo fmt --all -- --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - (1) Confirmed `test_load_all_prompts_registers_builtin_partials`, `test_prompt_resolver_loads_user_prompts`, `test_prompt_resolver_loads_local_prompts` remain in `crates/swissarmyhammer-templating/src/resolver.rs` (unrelated tests) and no `swift`-named test exists there anymore. The three moved Swift tests — `swift_partial_names_each_formatter_with_its_own_config_file`, `swift_partial_documents_the_airbnb_plugin_commands`, `swift_partial_agrees_with_the_shipped_swift_tool_validators` — exist in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs` and all 3 PASS: `cargo nextest run --package swissarmyhammer-validators shipped::swift_guidelines_partial` — 3 passed.
    - (2) `mod swift_guidelines_partial;` is registered in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, and the run above proves it actually executes (not just compiles).
    - (3) `idioms_swift.rs` — all 12 of its own tests pass in isolation: `cargo nextest run --package swissarmyhammer-validators shipped::idioms_swift` — 12 passed, including `every_shipped_idioms_tool_rule_passes_its_fixtures` (passing fixture reports 0) and `the_shipped_swift_idioms_tool_rule_reports_a_file_beside_one_it_declined` (asserts `SWIFT_IDIOMS_FAIL_FIXTURE_FINDINGS = 42`). The "21 distinct rules" figure is documented in `builtin/validators/code-hygiene/rules/idioms-swift.md` ("42 findings ... carrying 21 of the 27 enabled rules").
    - (4) RED proof reproduced myself: swapped `.swift-format`/`.swiftformat` in the table of `builtin/_partials/project-types/swift.md` (Apple swift-format ↔ SwiftFormat rows) — `swift_partial_names_each_formatter_with_its_own_config_file` FAILED with `the swift guidelines partial must pair Apple's swift-format with the config file it alone reads`. Restored the file; `git status --porcelain` on that file returned empty (byte-for-byte restore confirmed), and the test went back to PASS (3 passed, 845 skipped).
    - next: caller (/finish or user) records this outcome on the card.
  timestamp: 2026-08-22T20:25:07.634645+00:00
- actor: claude-code
  id: 01m6bszy52nkybteq5kgehxrfh
  text: |-
    ### implement — stuck
    - evidence: no file changed. The card work is already in HEAD: commit d4a6da772 put the Airbnb plugin path and the separate swift-format / swiftformat config files into builtin/_partials/project-types/swift.md. Commit 9e8efe569 added the coupling test swift_guidelines_partial.rs, and 414cf4209 later updated it. Nothing is left to implement from the old card text.
    - conflict: the new one-formatter direction removes what this card ordered. Open card ^ywgq61n ("Rewrite the Swift project partial to state one formatter") orders the partial to name `swiftformat` nowhere and the Airbnb plugin nowhere, with `swift format` as the only formatter. This card ordered the partial to name each tool with its config file and to prefer `swift package format --lint` (the Airbnb plugin). The idioms-swift rule now runs on the toolchain `swift format` (commits 414cf4209, b773be49b), so the current partial text about the swiftformat options and the idioms-swift gate is stale. The two cards cannot both be correct.
    - human decision needed: close ^m7ynz9c as superseded by ^ywgq61n (and ^7fgdenq, the config-detection half, likewise), or keep the Airbnb plugin path and change ^ywgq61n. I did not guess and did not edit the partial or any validator.
    - next: person decides.
  timestamp: 2026-11-01T15:00:05.922631+00:00
position_column: doing
position_ordinal: '8280'
project: swift-validator
title: 'swift partial: prefer `swift package format --lint` when the Airbnb plugin is a dependency'
---
`builtin/_partials/project-types/swift.md:40-41` names three tools loosely and commits to none:

```
- Format: `swift format -i -r Sources Tests` (or `swiftformat .`) — run before committing
- Lint: `swiftlint` (if present)
```

`swift format` and `swiftformat` are DIFFERENT TOOLS. The first is Apple's `swift-format`, shipped with the toolchain. The second is Nick Lockwood's SwiftFormat, installed separately, with a different config format and a different rule set. Offering them as interchangeable alternatives in one line is wrong and will send an agent to the wrong config file.

## Two fixes

1. Split the two tools onto their own lines. Say which config file each reads: `swift format` reads `.swift-format`; `swiftformat` reads `.swiftformat`.

2. Detect the Airbnb Swift Package Manager plugin and prefer it when present. A package that declares

   ```swift
   .package(url: "https://github.com/airbnb/swift", from: "1.0.0")
   ```

   gets one command that runs SwiftFormat AND SwiftLint with a shared config and one exit code:

   ```
   swift package format --lint          # check, do not write
   swift package --allow-writing-to-package-directory format   # non-interactive fix
   ```

   Useful switches: `--paths Sources Tests Package.swift`, `--targets <Target>`, `--exclude Tests`, `--swift-version 6.2`.

   Exit codes: non-zero on any failure in `--lint` mode; in fix mode, non-zero only for SwiftLint rules, because those cannot autocorrect.

   For a project with this dependency, one install and one command beats wiring two tools ourselves.

## Related

Card ^7fgdenq — "guidelines: swift format instruction — honor an existing .swift-format/.swiftformat config, defaults otherwise" — covers the config-detection half. Read it before starting. These two cards may merge; if they do, close one and say so.

## Acceptance

- The partial names each tool once, with its own config file.
- The plugin path is documented with the real command lines above.
- No line implies `swift format` and `swiftformat` are the same tool. #tool-validators

## Review Findings (2026-08-22 15:00)

> Scope: `review sha HEAD~1..HEAD` — commit `d4a6da772`. Task-mode.
>
> ENGINE COVERAGE: the engine reported `findings: 0` over 1 of the 12 changed files. It never opened `builtin/_partials/project-types/swift.md`, which holds the substance of this commit. No validator in the roster of 13 matches `*.md`. That `findings: 0` is not a clean review, and a further reproduction is on ^dyad426.
>
> The finding below comes from a hand review. Every command line in the partial was RUN.

- [ ] `crates/swissarmyhammer-templating/src/resolver.rs:572` `tests/coupling` — `swift_partial_agrees_with_the_shipped_swift_tool_validators` does not measure agreement. It asserts only that six literal strings are in the partial. It never reads `builtin/validators/code-hygiene/rules/idioms-swift.md`, the file its own doc comment names as the thing agreed with. Change the `run:` option set of that rule and the partial disagrees with the gate while this test stays green — the four options and the version floor are now written in THREE places with nothing holding them equal. Read the four swiftformat options and the `--min-version` floor out of the shipped rule text, then hold the partial to those values, so one source of truth decides both. The other two tests are sound: for a document, the presence of the exact command line IS the behaviour, and `swift_partial_names_each_formatter_with_its_own_config_file` also pins the removed regression by name.

### What was measured, so it is not measured again

Every claim the commit adds to the partial was RUN on this machine. Toolchain: Swift 6.4, swift-format `main`, SwiftFormat 0.62.1, SwiftLint 0.65.0, airbnb/swift 1.2.0. Each row PASSED.

| the claim | how it was run | result |
|---|---|---|
| a bare `swift package format --lint` stops with a permission error | run with stdin closed | `error: Plugin 'FormatSwift' wants permission to write to the package directory.` exit 1 |
| the flag makes the check run | `swift package --allow-writing-to-package-directory format --lint` | runs, exit 1 on findings |
| the check run writes no file | `shasum` of all three sources before and after | identical |
| `--targets` is not a switch and exits non-zero | `--targets Demo` | `Error: Unknown option '--targets'`, tool exit 64, command exit 1 |
| `--target <Target>` is the switch | `--target Demo` | scoped the run to `Sources/Demo` |
| `--paths Sources Tests Package.swift` | the exact documented form | `Linting Swift files at paths Sources, Tests, Package.swift` |
| `--exclude Tests` | run with it | `Tests` dropped, `Sources` and `Package.swift` kept |
| `--swift-version 6.2` | run with it | accepted, and it turned on a version-gated `trailingCommas` finding |
| `--lint` exits non-zero on any failure | 7 SwiftFormat errors | exit 1 |
| fix mode exits non-zero ONLY for a SwiftLint rule | SwiftFormat-only defects, then a SwiftLint-only defect | exit 0, then exit 1 |
| the plugin config sets `--property-types inferred` | read `airbnb.swiftformat` from the built bundle | line 40: `--property-types inferred # redundantType, propertyTypes` |
| plugin fix mode rewrites `var items: [Int] = []` into `var items = [Int]()` | ran fix mode | rewrote it, and `var ids: Set<String> = []` into `var ids = Set<String>()` |
| bare `swiftformat .` rewrites `if case .some(let inner)` into `if case let .some(inner)` | ran it with no config | rewrote it |
| the `idioms-swift` gate then REPORTS that line | ran the shipped `run:` script on the rewritten file | `hoistPatternLet: Reposition let or var bindings within pattern.` on that line |
| the four defaults are `hoist`, `preserve-struct-inits`, `ignore` and `preserve` | `swiftformat --options` | all four exact |
| a parent `.swiftformat` stating the four options makes a bare run correct | staged one two directories up | inline form preserved, 0 files changed |
| `.swift-format` is JSON, and a parent one is obeyed | `swift format dump-configuration --effective` from a nested directory | the parent's `"spaces": 7` was effective |
| `swift format lint -s` exits non-zero, and without `-s` it exits 0 | both runs | exit 1 with `error:`, exit 0 with `warning:` |
| `swift format -i -r` writes in place | ran it | wrote |
| `swiftlint` reads `.swiftlint.yml` | 1 violation by default, then `only_rules: [todo]` | 0 violations |

The parts of the partial that PREDATE this work were measured too, because they were never under a gate either. All PASSED: the ULID dependency, product and target lines resolve and build, and `let id = ULID()` compiles; `swift test --filter 'MyTargetTests.ThingTests/testMakesAnId'` selects exactly that one test, so the target name IS part of the expression as documented; `.swift-version` gates `preferCountWhere` and `Package.swift` is not read for it.

Both documented contradictions are REAL and stated correctly. The partial is not misleading on either one.