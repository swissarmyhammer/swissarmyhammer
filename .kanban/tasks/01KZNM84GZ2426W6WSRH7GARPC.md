---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kznnaywvsp9g3ka8hxav129r
  text: |
    ### Decision — the mark at the definition, not the file name

    I took option A's SHAPE — one fact, stated one time, in the flat config the rule
    already writes — but I state the fact as the test-framework CALL, not as the
    file path. Evidence made the file-glob form wrong.

    **The measurement that rejected the file glob.** I ran the rule's own script over
    the 444 `.ts`/`.tsx` files under `apps/`. Of the 33 complexity findings, 23 stand
    in a `*.test.*` file. I read the source line of each one:

    - 19 of the 23 are NAMED HELPERS: `defaultInvoke`, `defaultInvokeImpl`,
      `bootstrapInvokeImpl`.
    - 4 of the 23 are `it(...)` or `it.each(...)` callbacks.

    `cognitive-complexity.md` states its exemption this way: "Identify a test from
    its attribute or framework naming convention at the **definition**, never from
    the file name. A complex helper named `build_request` in a file called
    `foo_test.rs` is still a complex function and is still listed."

    So a `**/*.test.*` glob that turns `sonarjs/cognitive-complexity` off drops 19
    findings the superseded prompt rule MAKES. That is the same false `supersedes`
    claim in the other direction — the losing direction the card names as the lesser
    defect, but a defect all the same, and it contradicts written rule text.

    The card's premise that "eslint cannot state the same fact" does not hold. The
    config is JavaScript. It wraps each of the two eslint rules and drops a report
    whose innermost function is the argument of a test-framework call (`describe`,
    `it`, `test`, `suite`, `context`, `beforeAll`, `beforeEach`, `afterAll`,
    `afterEach`), read through a member and a chained call so `it.each(rows)(...)`
    and `describe.only(...)` carry the mark. It reads the core rule through
    `require("eslint/use-at-your-own-risk")` and the sonarjs rule through the
    plugin's `rules` map. NO NEW DEPENDENCY, and no path glob.

    This is the `duplication` `TestSpec` precedent the card names: read the parse,
    read the call target.

    **B and C, and why not.** B (drop `function-length` from `supersedes`) pays the
    LLM calls the tool rule exists to remove. C (an inline suppression on each
    block) states the exemption 36 times and needs a new one for every new test
    file. Both are recorded in the rule doc.

    **The cost, stated plainly.** The mark reads the call, not the length or the
    score. A test-framework callback is exempt however long it runs: a `describe`
    callback of 900 lines is not a finding. The prompt rule makes the same trade, so
    the reach is the prompt rule's reach. It is NARROWER than the card's option A,
    which would also have exempted a named helper in a test file.
  timestamp: 2026-08-10T11:01:15.803148+00:00
- actor: claude-code
  id: 01kznnbkyxsd36kg6yez7d0dk1
  text: |
    ### Counts on this workspace, measured with the rule's own script

    I extracted the `tool.run` block from the rule file and ran it over all 444
    `.ts`/`.tsx` files under `apps/`. Not predicted — run.

    | | complexity | length | total |
    |---|---|---|---|
    | before | 33 | 37 | **70** |
    | after | 29 | 1 | **30** |

    The 40 dropped findings, read at their source lines:

    - 36 `describe(...)` callbacks over the line gate
    - 3 `it(...)` callbacks and 1 `it.each(...)` callback over the complexity gate

    A key-by-key diff of the two runs shows ZERO findings only in the AFTER set, so
    the change adds nothing. The 19 named helpers in test files
    (`defaultInvoke` / `defaultInvokeImpl` / `bootstrapInvokeImpl`) all stay, which
    is the behaviour `cognitive-complexity.md` requires.

    The one remaining length finding is `CommandPalette` in
    `apps/kanban-app/ui/src/components/command-palette.tsx`, which is production
    code.

    ### The fixture pair still proves the rule

    Both fixtures gained a guard, and I checked each one RED before GREEN.

    - **pass fixture** now holds a `describe(...)` callback of 265 counted lines
      holding an `it(...)` callback that scores 21. RED (the config as it was before
      this change) reports both. GREEN (the rule as it stands) reports none. So a
      change that drops the carve-out makes the pair fail.
    - **fail fixture** now holds its two functions INSIDE a `describe(...)` block.
      Both are still reported — `foldGrid` at 21, `mixState` at 264 lines — so a
      carve-out that grew broad enough to exempt everything in a test block would
      take the fail fixture to zero findings and make the doctor mark the rule
      unusable.

    Neither fixture file name matches any test-file convention, and the carve-out
    does not read file names at all, so `find_fixture` is untouched.

    Proved through the real doctor path, not by hand:

        cargo nextest run -p swissarmyhammer-validators \
          every_shipped_complexity_tool_rule_passes_its_fixtures
        1 passed

    That test drives `check_review_engine_with` over every shipped complexity rule
    and asserts `row.usable()`, which is the fixture pair.

    ### Notes for the next agent

    - Editing the `tool` block drops the stored `ToolHealthCache` verdict by design
      (the digest covers the whole `tool` block plus every file in `fixtures/`).
      Both fixture files changed too, so the verdict is proved again on the next
      review. That is correct.
    - The two rule ids CHANGED, because a wrapped rule takes the plugin namespace:
      `code-hygiene/cognitive-complexity` and `code-hygiene/max-lines-per-function`.
      The `jq` filter and the doc's suppression example both follow. I checked the
      repository first: no source file carries an `eslint-disable` for either old
      id, so nothing broke.
    - `sonarjs/cognitive-complexity` reports `{messageId, data, loc}` with NO `node`
      field. A wrapper that keys on `descriptor.node` therefore exempts nothing on
      the complexity gate — I measured that dead end before moving to the
      loc-to-innermost-function lookup that both rules share.
  timestamp: 2026-08-10T11:01:37.373591+00:00
- actor: claude-code
  id: 01kznnkqc6yp0302h6j4h7np2m
  text: |
    ### The `supersedes` survey — 24 rules read, 22 gaps, 22 cards filed

    I read every rule under `builtin/validators/**/rules/*.md` that declares a
    non-empty `supersedes` list, quoted the exemption text of each prompt rule it
    replaces, and answered YES / NO / PARTIAL for each exemption against what the
    `run` script's tool actually knows. Several answers were checked by running the
    tool, not by reading its documentation: ruff `D1` flags `def test_foo`,
    `__str__` and a `@property` getter; ruff `C901` flags a flat 21-branch dispatch
    chain and a test function; ruff `PLR2004` ignores `0`/`1`/`-1` and reports `100`.

    Two rules reproduce every carve-out and need no card:

    - `dead-code-dart` — Dart's `_` privacy makes the public-API and entry-point
      carve-outs for free, and the project's `analysis_options.yaml` is still read.
    - `unused-code-go` — `U1000` reports unexported identifiers only, and counts the
      test harness as a caller.

    One card is filed for each of the other 22:

    | card | rule | the carve-out the tool drops |
    |---|---|---|
    | ^bt0w505 | complexity-go | tests, generated code, and no suppression comment exists |
    | ^1xhws0j | complexity-python | flat configuration chains (C901 is McCabe), tests |
    | ^w5v73k1 | complexity-rust | data, builder and init functions; test targets |
    | ^h2ezbs7 | complexity-swift | tests, generated code, long init |
    | ^bdh09pb | dead-code-python | the whole public API with no `__all__`; FastAPI/Django decorators |
    | ^nkyb681 | dead-code-rust | an `include!` file is called an orphan module |
    | ^gm39gd3 | dead-code-swift | test-only helpers — 52 of its own 74 findings |
    | ^108bh4y | dead-code-typescript | library entry points, framework-registered exports |
    | ^1h52223 | function-length-go | `statements: 10000` removes the data carve-out |
    | ^kmxvk6r | function-length-python | tests, wide field-setting `__init__` |
    | ^s2ftjys | magic-numbers-go | `100` for percent, `<< 8` |
    | ^2syfvyt | magic-numbers-python | `100` reports, and ruff has no value allow-list |
    | ^xd5r1zh | magic-numbers-swift | a shift constant |
    | ^eedma7g | magic-numbers-typescript | `100` omitted although `ignore` supports it |
    | ^j0g7yk1 | missing-docs-dart | the probe copies tests and generated files into `lib/` |
    | ^s2056e1 | missing-docs-go | `ignoreGeneratedHeader` left false; `Error()`/`String()` |
    | ^kc0gez9 | missing-docs-python | `D1` flags tests, `__str__`, trivial getters |
    | ^302hw8c | missing-docs-rust | no `OUT_DIR` filter; also drops `--workspace` |
    | ^xv57pf8 | missing-docs-swift | the temp config discards the project `excluded:` |
    | ^739encr | missing-docs-typescript | trivial getters, obvious methods, no `.d.ts` filter |
    | ^bn9ws72 | no-commented-code-parsed | a "do not do this" example, and no marker exists |
    | ^m2zp1qg | duplication-parsed | semantic intent, generic dispatch, generated/vendored |

    ### Four patterns worth acting on above the card level

    1. **The test carve-out is the one most often dropped.** It survives only where
       the language toolchain makes it — rustc, staticcheck, revive's `isTest`
       guard, periphery's `--build-tests`. It is dropped everywhere a threshold
       linter is pointed at a changed-file list: gocognit, ruff `C901`/`PLR0915`/`D1`,
       funlen, and both swiftlint metrics. This card fixes the TypeScript instance.
       The mechanism it uses — read the definition's own mark from the parse — is
       the shape the others need.
    2. **Owning the config removes exemptions as a side effect.** Every rule that
       writes its own config does so to own its thresholds. The same act discards
       `per-file-ignores` for tests (ruff `--isolated`), `excluded:` paths for
       generated code (swiftlint, three rules), `analyzer: exclude:` (dart), and
       `ignoreGeneratedHeader` (revive, by omission). The generated-code exemption
       survives ONLY in the two golangci-lint rules, where it is a default the temp
       config happens not to override.
    3. **A difference between sibling rules is a reliable defect.**
       `complexity-rust` and `dead-code-rust` drop `OUT_DIR` paths;
       `missing-docs-rust` does not. `dead-code-typescript` drops `.d.ts`;
       `missing-docs-typescript` does not. `magic-numbers-swift` allows `100`; the
       other three do not, and two of the three could.
    4. **Two rules refuse a suppression marker entirely** — `complexity-go` and
       `no-commented-code-parsed`. Every false positive there is a mandatory code
       change.
  timestamp: 2026-08-10T11:06:03.014178+00:00
- actor: claude-code
  id: 01kznp5bj9b27nzggp47xnjyk7
  text: |
    ### implement — changed
    - evidence: 4 files — `builtin/validators/code-hygiene/rules/complexity-typescript.md` (the `tool.run` config plus the doc), `builtin/validators/code-hygiene/fixtures/complexity-typescript.fail.ts.tmpl`, `builtin/validators/code-hygiene/fixtures/complexity-typescript.pass.ts.tmpl`, `builtin/validators/code-hygiene/VALIDATOR.md`.
    - option: A's shape, with the carve-out stated as the test-framework CALL and not the file path. The file-glob form was rejected on measurement.
    - counts over the 444 `.ts`/`.tsx` files under `apps/`: 70 before (33 complexity, 37 length) → 30 after (29 complexity, 1 length). 40 dropped, all test-framework callbacks; 0 added.
    - fixtures: `cargo nextest run -p swissarmyhammer-validators every_shipped_complexity_tool_rule_passes_its_fixtures` — 1 passed. Both fixtures gained a guard, each checked RED before GREEN.
    - gates: `cargo nextest run --workspace` 14022 tests, 14021 passed, 1 failed — `mirdan git_source::tests::test_clone_anthropics_skills_select_nonexistent`, which clones from the network, took 77 s, and passes in 2 s on re-run. Not related to this change. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - 22 cards filed for the `supersedes` survey gaps.
    - next: `/review`.
  timestamp: 2026-08-10T11:15:40.745023+00:00
- actor: claude-code
  id: 01kznregjxrkhp7xxj91q02y86
  text: |-
    ### review — findings
    - scope: `review sha 65eea58d0^..65eea58d0` (4 files, all `.md` and `.ts.tmpl`)
    - engine: 0 findings, `attempted: 0`, `skipped: 0`, `skipped_files: []`. No validator `match_globs` covers `.md` or `.tmpl`, so the engine had nothing in scope. The verification below was done in this turn.
    - evidence: 3 findings — `builtin/validators/code-hygiene/fixtures/complexity-typescript.fail.ts.tmpl:35`, `builtin/validators/code-hygiene/rules/complexity-typescript.md:46`, `builtin/validators/code-hygiene/rules/complexity-typescript.md:68`

    #### What was proved correct
    - Fixture proof: `cargo nextest run -p swissarmyhammer-validators every_shipped_complexity_tool_rule_passes_its_fixtures` — 1 passed.
    - Counts reproduce EXACTLY with the shipped config against the pre-commit config over the 444 `.ts`/`.tsx` files under `apps/`: 70 before (33 complexity, 37 length), 30 after (29 complexity, 1 length), 40 dropped, **0 added**. The 40 dropped are 36 `describe`, 3 `it` and 1 `it.each` callbacks.
    - Nothing can ADD a finding: the wrapper only early-returns, and the two thresholds (15; 250 with `skipBlankLines` and `skipComments`) are unchanged from the pre-commit config.
    - Rule ids: the `jq` filter selects the NEW ids, and the doc suppression example names the new ids. No file in the repository suppresses `sonarjs/cognitive-complexity` or the bare `max-lines-per-function`; the 4 matches found are prose that names the upstream rules.
    - No new dependency: the commit holds 4 files only, with no `package.json`, `Cargo.toml` or `Cargo.lock`. The `install`, `check_command` and `check_version_command` lines have an EMPTY diff.
    - The refusal of the path-glob form is sound: `cognitive-complexity.md:71-73` states word for word that a complex helper in a test file is still listed, and `function-length.md:20` exempts "Functions explicitly marked as tests".
    - `it.each(rows)(...)` and `describe.only(...)` carry the mark, as the doc states.
    - A report anchored outside any function is handled: `innermostAt` gives `null`, and the report passes through.
    - All 4 changed files are far below the 262144-byte cap; the largest is 25202 bytes.

    #### The hole
    `innermostAt` uses the report location, and both gates report at the function head, which for a method or an accessor starts at the NAME — outside the function node's range. On this workspace the defect bites zero times today, because all 40 drops are `describe`/`it` callbacks. It is latent, not absent.

    - next: correct the two anchoring defects in the wrapper, and widen the fail fixture to a method or an accessor.
  timestamp: 2026-08-10T11:55:37.950002+00:00
- actor: claude-code
  id: 01kznrfs5ms6zdzpng3ks1nty7
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 4 files; the carve-out is stated as the test-framework CALL, not the file path. Option A as written was refused on measurement: 19 of the 23 complexity findings in test files are named helpers, and `cognitive-complexity.md:71-73` says a complex helper in a test file is still listed.
    - test: green with one caveat — `cargo nextest run --workspace` 14021 passed, 1 failed (`mirdan test_clone_anthropics_skills_select_nonexistent`, clones from the network, timed out at 77 s, passed in 2 s on re-run, unrelated); fmt clean; clippy clean
    - commit: 65eea58d0
    - review: findings — `fixtures/complexity-typescript.fail.ts.tmpl:35`, `rules/complexity-typescript.md:46`, `rules/complexity-typescript.md:68`
    - verified: the counts reproduce exactly on an independent sweep — 70 before, 30 after, 40 dropped, 0 added. The fixture proof passes. No manifest is in the commit. The `jq` filter and the doc name the new rule ids, and nothing suppresses the old ids.
    - The three findings are all in the wrapper, and all latent — they bite zero times on `apps/` today. `innermostAt` anchors on the REPORT location, but both gates report at the function head, and a method or accessor head begins at the NAME, which sits outside the `FunctionExpression` range. The lookup then climbs to the enclosing function, so a complex method inside a `describe` is exempted in silence. That is the same false-negative class the path glob was refused for. `rootCalleeName` also walks to the root identifier with no limit, so `context.run(() => {})` reads as a test call.
    - guardrail: first iteration, no repeat. The loop continues.
  timestamp: 2026-08-10T11:56:19.508334+00:00
- actor: claude-code
  id: 01kznsvp7sd0zrp48cndjstft1
  text: |
    ### The three review findings — corrected, each proved by a run

    Every claim below is a run with the SHIPPED `tool.run` block. The block was
    extracted out of the rule file by a script and compared byte for byte with the
    probe script, so "shipped" here is the file and not a copy of it.

    #### Finding 3 — the anchor climbed past a method and an accessor

    `innermostAt` read the report position against the function RANGES alone. eslint
    `getFunctionHeadLoc` starts the head at the parent's start for a `Property`, a
    `MethodDefinition` and a `PropertyDefinition` — that is, at the member's NAME,
    which stands before the `FunctionExpression` the rule measures. I read that in
    `<node_modules>/eslint/lib/rules/utils/ast-utils.js`, so the premise is read and
    not reasoned.

    The wrapper now stores each such member's own start offset in a `head` map, and
    `measuredAt` measures each candidate's span from that offset. The method's span
    is then the smallest one that holds the report.

    BEFORE, on `probe.ts` (each body scores 21 against the gate of 15):

        line 5   topNamed            (declaration, top level)   REPORTED
        line 25  topObjMethod        (object method, top level) REPORTED
        line 46  topClassMethod      (class method, top level)  REPORTED
        line 68  innerNamed          (declaration, in describe) REPORTED
        line 87  innerArrow          (arrow, in describe)       REPORTED
        line 107 innerObjMethod      (object method, describe)  SILENT
        line 128 innerClassMethod    (class method, describe)   SILENT

    AFTER: every one of the seven is reported. On `probe2.ts` the getter at line 4
    was SILENT before and is reported after.

    The length gate behaves the same. `probe3.ts` holds a 266-line class method, a
    266-line getter and a 266-line declaration, all inside one `describe`:

        BEFORE: 1 finding  — line 542 `longDeclared`
        AFTER:  3 findings — line 5 `longClassMethod`, line 274 `longGetter`,
                             line 542 `longDeclared`

    #### Finding 2 — the mark read the root identifier, not the call

    `rootCalleeName` is gone. `isTestCall` now requires BOTH the root identifier to
    be a framework name AND every property between it and the call to be a test
    modifier: `only`, `skip`, `todo`, `failing`, `fails`, `concurrent`,
    `sequential`, `shuffle`, `each`, `for`, `runIf`, `skipIf`. A computed property
    is refused. The walk also reads a tagged template, so `` it.each`table`(...) ``
    carries the mark.

    BEFORE, on `probe4.ts` (every callback scores 21):

        describe.only(...)        exempt   (correct)
        it.each(rows)(...)        exempt   (correct)
        it.each`table`(...)       REPORTED (the tagged form lost the mark)
        context.run(() => {...})  exempt   (WRONG — not a test call)
        describe.each(rows)(...)  exempt   (correct)

    AFTER: the only finding is `context.run`. The four framework forms stay exempt,
    and the tagged-template form gained the mark.

    #### Finding 1 — the fail fixture could not catch either defect

    The fail fixture now holds FIVE guards, all inside the `describe` block:
    `foldGrid` (declaration, complexity), `mixState` (declaration, length),
    `GridFolder.foldRows` (class METHOD), `readings.band` (ACCESSOR), and
    `context.run(() => {...})` (a call whose root identifier is a test name and
    which is not a test call).

    Run against the fixture with the pre-change wrapper and with the shipped one:

        BEFORE: 2 findings — line 47 foldGrid, line 75 mixState
        AFTER:  5 findings — 47 foldGrid, 75 mixState, 354 foldRows,
                             383 get band, 425 context.run

    A wider fixture alone still cannot catch the defect, because the doctor asks
    the fail fixture for AT LEAST one finding, and a carve-out that exempted four
    of the five would still pass. A new acceptance test therefore names all five:
    `the_shipped_typescript_complexity_tool_rule_measures_every_fail_fixture_guard`
    copies the shipped fail fixture into a probe repository, plans the rule, runs
    it through `execute_tool_runs`, maps each finding back to its source line, and
    asserts that every guard is present and that no sixth finding exists.

    I checked that test RED against the EXACT pre-change wrapper before GREEN:

        the carve-out must leave the fail fixture guard `foldRows(` measured inside
        its `describe` block; the run reported ["function foldGrid(...)",
        "function mixState(...)"]

    The pass fixture gained a `describe.only(...)` block and an `it.each(rows)(...)`
    block, each holding a body that scores 21. Both must be exempt, so a modifier
    list that stopped reading `only` or `each` takes the pass fixture off zero and
    the doctor marks the rule unusable. I proved that guard too: a wrapper that
    reads only a bare identifier callee makes doctor report "the pass fixture
    complexity-typescript.pass.ts.tmpl produced 2 finding(s); none are allowed".

    ### The sweep over `apps/`, re-run

    | | complexity | length | total |
    |---|---|---|---|
    | no carve-out | 33 | 37 | **70** |
    | the carve-out as committed at 65eea58d0 | 29 | 1 | **30** |
    | the carve-out with the three corrections | 29 | 1 | **30** |

    A key-by-key comparison of the last two rows: **0 findings back, 0 findings
    gone**. The count did not rise, and that is correct. Both defects are LATENT on
    this workspace: each of the 40 drops is a `describe` or an `it` callback written
    as a plain arrow, and no file under `apps/` holds a complex method inside a test
    callback or a `context.run` call. The fixture, not this workspace, is what
    proves the corrections.

    ### A path detail the new test met

    eslint prints the RESOLVED path of each file it reads. On macOS a temporary
    directory stands behind a symbolic link (`/var` → `/private/var`), so the
    engine's prefix strip matched nothing and every finding kept an absolute path.
    The test therefore hands `execute_tool_runs` the canonical repository root. A
    real repository root is already resolved, so production never meets this.

    ### Notes for the next agent

    - `dump validators` reads the compiled-in copy of the builtin rules, so its
      `complexity-typescript` body was the version from before 65eea58d0. Read the
      file on disk when the rule text matters.
    - `complexity_work` now takes the tool-rule name as its first argument, because
      two languages drive that shape end to end.
    - `shipped_fixture_template` is new and `swift_package_root` now calls it; the
      two had the same fixture-template lookup written out twice.
  timestamp: 2026-08-10T12:20:18.297159+00:00
- actor: claude-code
  id: 01kznsw2ndnmgvb4cxr57bj48r
  text: |
    ### implement — changed
    - evidence: 5 files — `builtin/validators/code-hygiene/rules/complexity-typescript.md` (the wrapper plus the doc), `builtin/validators/code-hygiene/fixtures/complexity-typescript.fail.ts.tmpl` (3 guards added), `builtin/validators/code-hygiene/fixtures/complexity-typescript.pass.ts.tmpl` (2 modifier guards added), `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` (a new acceptance test).
    - findings: all 3 checked. Each proved by a run of the shipped `tool.run` block, before and after — see the finding comment for the output of each.
    - sweep over the 444 `.ts`/`.tsx` files under `apps/`: 70 with no carve-out, 30 with the carve-out as committed, 30 with the corrections. 0 back, 0 gone. The count did not rise, because both defects bite zero times here.
    - fixtures: `cargo nextest run -p swissarmyhammer-validators every_shipped_complexity_tool_rule_passes_its_fixtures the_shipped_typescript_complexity_tool_rule_measures_every_fail_fixture_guard` — 2 passed. The new test was checked RED against the exact pre-change wrapper first.
    - gates: `cargo nextest run --workspace` — 14023 tests run, 14023 passed, 0 failed, 0 skipped (the `mirdan test_clone_anthropics_skills_select_nonexistent` network flake did not appear). `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - no new dependency: `install`, `check_command` and `check_version_command` are unchanged, and no manifest is in the diff.
    - next: `/review`.
  timestamp: 2026-08-10T12:20:31.021458+00:00
- actor: claude-code
  id: 01kznty7f4zvvwst7av8j6806m
  text: |
    ### review — findings
    - scope: `review sha da41103e6^..da41103e6`; the engine returned 0 findings over 9 attempted validators, and the two findings below come from re-probing the shipped config.
    - evidence: 2 findings — builtin/validators/code-hygiene/rules/complexity-typescript.md:41 (twice; the `TEST_MODIFIER` list)
    - verified by probe: the head anchor now measures 13 member shapes correctly inside a `describe` block; the previous config reports 2 of the 5 fail-fixture guards and this one reports 5; the sweep over `apps/` is 70 with no carve-out and 30 both before and after, with 0 keys back and 0 keys gone; the `doctor` and `install` blocks and the three `require()` targets are unchanged; the largest changed file is 30179 bytes.
    - unrelated to this commit: `review::tool_rules::tests::shipped::the_swift_package_root_restores_the_directory_before_it_removes_it` fails under the module run. It passes alone and it fails the same way when the new TypeScript test is skipped, so the cause is the two parallel callers of `swift_package_root` racing on the process working directory. Both callers stand at `65eea58d0` as well.
    - next: accept the framework root name as a property in `TEST_MODIFIER`, read the whole callee shape rather than a name pair, and put a Playwright-shaped guard in the pass fixture.
  timestamp: 2026-08-10T12:39:10.052962+00:00
- actor: claude-code
  id: 01kzntzsj1h2t8kvzfha70z8j8
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 5 files; all 3 findings fixed, each proved by a run of the shipped `tool.run` block, not by reasoning
    - test: green — `cargo nextest run --workspace` 14023 passed, 0 failed; fmt clean; clippy clean
    - commit: da41103e6
    - review: findings — 2, both at `builtin/validators/code-hygiene/rules/complexity-typescript.md:41`, the `TEST_MODIFIER` list
    - verified by probe, not by report: the anchor fix is robust across 13 member shapes (static, decorated, class expression, arrow in a property, class field, setter, nested member, computed key, generator, private `#`, `accessor` field, string key, decorator) — all 13 report, and the span math is monotonic so nesting cannot pick the wrong candidate. The fixtures genuinely guard: the previous config reports 2 of the 5 guards, the new one reports all 5, so the new Rust test is truly RED against the old wrapper. Sweep 70/30/30, 0 back, 0 gone. No dependency added; `doctor` and `install` are byte-identical.
    - THE REGRESSION: the modifier list refuses Playwright's namespace API. Playwright spells everything on the `test` root — `test.describe`, `test.beforeEach`, `test.afterEach`, `test.step`, `test.describe.serial`, `test.describe.parallel`, `test.fixme`, `test.slow`. None of those names is a modifier, so `isTestCall` returns at the first property and never reaches the root. Measured on one probe: `65eea58d0` reports 6, `da41103e6` reports 14, and the 8 added are exactly those 8 openers. This commit therefore makes 8 findings the superseded rule would never make — the same false claim this card exists to remove, running the other way.
    - The second finding: the mark still reads a NAME PAIR and not a call shape, so `context.each(rows)(fn)` and `context.for(rows)(fn)` stay exempt.
    - outside the delta: `the_swift_package_root_restores_the_directory_before_it_removes_it` fails under a module run and passes alone — two parallel callers race on the process working directory. It fails the same way at `65eea58d0`, so it is not from this work.
    - guardrail: no finding repeats across the two ledger entries. The loop continues.
  timestamp: 2026-08-10T12:40:01.345442+00:00
- actor: claude-code
  id: 01kznvqwgnynp8zb9jn7rjk6bw
  text: |-
    Pulled back to `doing` for the two open findings of the 2026-08-10 07:28 section.

    ## The rule the two findings point at

    The two findings pull in opposite directions, so neither list can move on its own. The mark now reads the callee as a CHAIN OF SEGMENTS with a role for each segment, and not two flat lists crossed with each other.

    - One segment must be a FRAMEWORK FUNCTION: `describe`, `it`, `test`, `suite`, `context`, `step`, `beforeAll`, `beforeEach`, `afterAll`, `afterEach`.
    - Each segment BEFORE it must be a FRAMEWORK ROOT. `test` is the one root, because Playwright puts its whole surface on `test`.
    - Each segment AFTER it must be a modifier THAT framework function accepts. `describe`, `it`, `test` and `suite` accept the full Vitest, Jest and Playwright set. `context` accepts `only` and `skip`, the two Mocha gives it. `step` accepts `only` and `skip`. The four hooks accept none.
    - The chain is read from the last segment to the first, so `test.describe` takes `describe` as the framework function and `test` as the root, while `test.each(rows)` takes `test` as the framework function and `each` as a modifier.

    Finding 1 falls out of the root: Playwright's 8 openers are root + framework function. Finding 2 falls out of the per-function modifier: Mocha has no `context.each`, so `each` is not in `context`'s set.

    ## Measured, on a 44-spelling probe, every callback at score 21 against the gate of 15

    Probe and runners: `/private/tmp/claude-501/-Users-wballard-github-swissarmyhammer-swissarmyhammer/bc2c1635-c6b9-4bba-9a81-0fce95d1ff03/scratchpad/h7/`.

    | config | findings | wrong cases |
    | --- | --- | --- |
    | `65eea58d0` (root name only) | 5 | 7 — the 6 `context.*` negatives exempt, and ``it.each`table` `` reported |
    | `da41103e6` (root name + one flat modifier list) | 20 | 18 — 14 Playwright spellings reported, and 4 `context.each`/`context.for` exempt |
    | this working tree | 10 | 0 |

    The 14 Playwright spellings `da41103e6` reports: `test.describe`, `test.describe` with no title, `test.beforeEach`, `test.afterEach`, `test.beforeAll`, `test.afterAll`, `test.step`, `test.step.skip`, `test.describe.serial`, `test.describe.parallel`, `test.describe.serial.only`, `test.fixme`, `test.slow`, `test.fail`. Each is a finding the superseded `function-length` would never make.

    The 10 findings the working tree gives are exactly the 10 negatives: `context.run`, `context.each(rows)(fn)`, `context.each(rows)("s", fn)`, `context.for(rows)(fn)`, `context.for(rows)("s", fn)`, `context.map`, `harness.describe`, `runner.test`, a plain call, and a named helper inside a `describe` block.

    ## What `context` costs and buys

    Measured, not argued. A config that drops `context` from the framework functions reports the SAME 30 findings over `apps/`, key for key — it buys nothing here. It costs the Mocha suite carve-out: `context`, `context.only` and `context.skip` each then give a finding. `apps/` holds no call that spells a suite `context(`. `context` therefore stays, with the two Mocha modifiers.

    ## The sweep does not move

    444 `.ts` and `.tsx` files under `apps/`: 70 with no carve-out (33 complexity, 37 length), 30 at `65eea58d0`, 30 at `da41103e6`, 30 here (29 complexity, 1 length). Key by key against `da41103e6`: 0 back, 0 gone. Against the run with no carve-out: 0 added, 40 dropped. `apps/` holds no Playwright spelling and no `context.each`, so both defects were silent here — the fixtures, and not this workspace, are what prove them.

    ## The fixtures hold both directions now

    - Fail fixture: 5 guards to 7. It adds `context.each(rows)(...)` and `context.for(rows)(...)`. Run key by key: `da41103e6` reports 5, this tree reports 7.
    - Pass fixture: adds `test.describe(...)`, `test.describe.serial(...)`, `test.step(...)` and `test.beforeEach(...)`. `da41103e6` reports all four; this tree reports none.

    The guard test now names all 7 guards.
  timestamp: 2026-08-10T12:53:10.805823+00:00
- actor: claude-code
  id: 01kznw2bppaek7f2qy1mxpjz2s
  text: |-
    ### implement — changed
    - evidence: 4 files — `builtin/validators/code-hygiene/rules/complexity-typescript.md`, `builtin/validators/code-hygiene/fixtures/complexity-typescript.fail.ts.tmpl`, `builtin/validators/code-hygiene/fixtures/complexity-typescript.pass.ts.tmpl`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`.
    - probe, 44 spellings over both frameworks, every callback at score 21 against the gate of 15: `65eea58d0` 5 findings / 7 wrong; `da41103e6` 20 findings / 18 wrong (14 Playwright spellings reported, `context.each` and `context.for` exempt); this tree 10 findings / 0 wrong. The 10 are exactly the 10 negatives.
    - fixtures, run key by key: fail fixture 5 guards at `da41103e6` and 7 here (`context.each(rows)(`, `context.for(rows)(` added); pass fixture 4 findings at `da41103e6` (the four Playwright blocks) and 0 here. Both parse clean — eslint reports 0 fatal messages on each.
    - sweep over the 444 `.ts` and `.tsx` files under `apps/`: 70 with no carve-out (33 complexity, 37 length), 30 at `65eea58d0`, 30 at `da41103e6`, 30 here (29 complexity, 1 length). 0 back and 0 gone against `da41103e6`; 0 added and 40 dropped against the run with no carve-out. The count does not move because `apps/` holds no Playwright spelling and no `context.each` call.
    - `context` measured against its own removal: same 30 findings key for key, and 3 Mocha spellings lost. It stays.
    - gates: `cargo nextest run --workspace` 14023 run, 14023 passed, 0 skipped, 0 failed. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. `the_swift_package_root_restores_the_directory_before_it_removes_it` did not fail on this run.
    - doctor fixture pair: `every_shipped_complexity_tool_rule_passes_its_fixtures` PASS. Guard test: `the_shipped_typescript_complexity_tool_rule_measures_every_fail_fixture_guard` PASS with all 7 guards.
    - no new dependency; the `doctor` and `install` blocks are unchanged. HEAD stays at `da41103e6`; nothing committed.
    - next: `/review`.
  timestamp: 2026-08-10T12:58:54.038516+00:00
- actor: claude-code
  id: 01kznwvxxdzznk4j9pwtczkpy2
  text: |
    ### review — findings
    - scope: `c184cf1d8^..c184cf1d8`
    - engine: `review sha` reports 0 findings, 9 validators attempted, 0 failed.
    - probe: 2 findings, both measured against the shipped config.
    - evidence: `builtin/validators/code-hygiene/rules/complexity-typescript.md:51`, `builtin/validators/code-hygiene/rules/complexity-typescript.md:45`

    #### What the commit claims, verified
    - 44-spelling probe on the shipped config: 44 cases, 10 findings, 0 wrong. Claim holds.
    - Sweep over the 444 `.ts` and `.tsx` files under `apps/`: 30 findings (29 complexity, 1 length), key for key identical to the recorded `sweep-after.keys`. Claim holds.
    - `TYPESCRIPT_COMPLEXITY_FAIL_GUARDS` holds seven names, and `the_shipped_typescript_complexity_tool_rule_measures_every_fail_fixture_guard` passes. Claim holds.
    - Doctor fixture pair: `every_shipped_complexity_tool_rule_passes_its_fixtures` passes, with 23 fixture tests green. Claim holds.
    - No new dependency: the diff changes no `npm install` line and no `require(` line.
    - Size: the four changed files are 21687, 29516, 21063 and 30368 bytes, all under the 262144-byte cap.

    #### What the probe broke
    - Playwright API that is not a test opener: `test.use`, `test.extend`, `test.describe.configure`, `test.info` and `test.setTimeout` each give a finding. Correct — none carries the mark.
    - A framework function twice, and a root after the framework function: `describe.describe`, `describe.test` and `step.test` each give a finding. Correct.
    - `describe.each(rows)(name, fn)`, `it.each` template, `it.skipIf(true)(...)` and `test.only.each(rows)(...)` each keep the mark over the second call. Correct.
    - A computed member, `it[key](...)` and `describe[key].only(...)`, stops the read and gives a finding. Correct.
    - `test.test`, `test.context`, `test.suite` and `test.it` are exempt although no framework spells them. Each is rooted at `test`, which is itself the mark, so no finding is dropped on code a person writes.
    - Bare `step` and thirteen Mocha and Jest globals are wrong. Both are recorded as findings.

    #### Probe harness
    `/private/tmp/claude-501/-Users-wballard-github-swissarmyhammer-swissarmyhammer/bc2c1635-c6b9-4bba-9a81-0fce95d1ff03/scratchpad/h7/`
    - `cfg-ship.sh` — the `tool.run` block read out of the shipped rule; byte-identical to `cfg-after.sh`.
    - `probe2-gen.py` / `probe2.ts` — 40 adversarial spellings, 20 wrong.
    - `probe3-gen.py` / `probe3.ts` — 15 spellings taken from `globals.mocha` and `globals.jest`, 15 wrong.

    - next: give the chain grammar a root-required role or drop bare `step`, add the thirteen missing Mocha and Jest names, and guard both in the fixtures.
  timestamp: 2026-08-10T13:12:51.885413+00:00
- actor: claude-code
  id: 01kznwxnd6tf7sr6yqvqwd7edv
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — 4 files; the mark now reads the callee as a chain of segments with a role for each segment
    - test: green — `cargo nextest run --workspace` 14023 passed, 0 skipped; fmt clean; clippy clean
    - commit: c184cf1d8
    - review: findings — 2, `builtin/validators/code-hygiene/rules/complexity-typescript.md:51` (a bare `step(...)` now carries the mark, but no framework spells it — Playwright spells only `test.step`) and `:45` (`FRAMEWORK_CALL` names ten functions; thirteen more Mocha and Jest openers and hooks stand outside it)
    - every claim of the commit was verified by measurement: the 44-spelling probe gives 0 wrong, the sweep is 30 key for key, the guard test holds 7 names, 23 fixture tests pass, no dependency changed. The probe could not break the chain reader on `test.use`, `test.extend`, `test.describe.configure`, `describe.each(rows)(name, fn)`, the `it.each` template, or a computed member.

    ### The pattern across three rounds, and what ends it

    This is the THIRD review round to find a gap in the test-call recognition list. The literal guardrail has not fired, because each finding names a different line and a different cause:
    - iteration 1: the anchor, `rootCalleeName`, and a fixture that could not catch either
    - iteration 2: the Playwright namespace API, and a pairing that accepted `context.each`
    - iteration 3: a bare `step`, and thirteen absent Mocha and Jest globals

    The class is the same every time: a HAND-CURATED list of framework spellings is wrong in one direction or the other. Widening it makes a false negative; narrowing it makes a false positive.

    The fix this round names itself. `globals.mocha` and `globals.jest` are already inside the `globals` package in the `eslint-plugin-sonarjs` tree the config resolves through. The list can be READ rather than written. That is the same move the rest of this work makes — an exemption becomes a structural fact rather than a judgment.

    Iteration 4 takes that direction. If a fourth review round still finds absent spellings, the hand-curated approach has failed four times and the card goes to a person rather than to a fifth round.
  timestamp: 2026-08-10T13:13:48.710273+00:00
- actor: claude-code
  id: 01kznyczz96x44bb0dc4zt8b5p
  text: |
    ### Iteration 4 — the list is read, not written

    Every number below is a run of the SHIPPED `tool.run` block. The block is read
    out of the rule file by a script, so "shipped" here is the file itself.

    ## Where the names come from, and how it was verified

    Two files inside the tree eslint already resolves through:

    - `eslint-plugin-sonarjs/cjs/helpers/test-frameworks.js` exports
      `TEST_FRAMEWORK_STRUCTURE_FUNCTIONS` — 20 names: `describe`, `context`,
      `suite`, `it`, `test`, `specify`, `before`, `after`, `beforeEach`,
      `afterEach`, `beforeAll`, `afterAll`, `xdescribe`, `xcontext`, `xit`,
      `xtest`, `fdescribe`, `fcontext`, `fit`, `ftest`.
    - `globals` — `globals.mocha` (20 names) and `globals.jest` (13 names), union
      26. Mocha's TDD interface (`setup`, `teardown`, `suiteSetup`,
      `suiteTeardown`) and `xspecify` stand only here.

    Three facts inside `globals` drop the 4 names that open no test: an
    environment name (`mocha`, `jest`), a `globals.chai` name (`expect`), and
    `run`, Mocha's delayed-start runner, which takes no callback. `run` is the one
    name no file in the tree separates, so the config names it and the doc says
    why.

    Result: 25 framework functions, 12 of them on the Jest/Vitest/Playwright
    modifier tier. `context` lands on the Mocha tier (`only`, `skip`) because
    neither `globals.jest` nor `globals.vitest` declares it — that is the round-2
    fix, now derived rather than written.

    **NO NEW DEPENDENCY, verified two ways.** `eslint-plugin-sonarjs@4.2.0`
    declares `"globals": "^17.7.0"` in its own `package.json`, so the rule's own
    `npm install -g eslint@10.8.0 typescript-eslint@8.66.0 typescript@5.9.3
    eslint-plugin-sonarjs@4.2.0` brings it. On this machine it resolves to
    `<node_modules>/eslint-plugin-sonarjs/node_modules/globals/index.js`. A bare
    `require("globals")` under the rule's own `NODE_PATH` would NOT find it,
    because it sits under the plugin rather than beside it. The config therefore
    resolves it from the plugin's own directory:

        require.resolve("globals", {
          paths: [path.dirname(require.resolve("eslint-plugin-sonarjs"))],
        })

    `eslint-plugin-sonarjs` ships no `exports` field, so the helper subpath
    resolves as a plain file. Both reads were run under the rule's own resolution
    line and both answered; standard error was EMPTY, which is what proves the
    read happened rather than the fallback.

    ## Finding 1 — a bare `step` carried the mark

    The chain grammar gained the role the finding asked for. A framework function
    may be ROOTED, which means a framework root must stand before it. `globals`
    ships no Playwright environment, so `step` is the one written name, and it is
    rooted.

    ## Finding 2 — thirteen Mocha and Jest globals stood outside the map

    They now come from the read. So do `fdescribe`, `fcontext` and `ftest`, which
    sonarjs names and `globals` does not.

    ## Measured — three probes, before and after

    `before` is the config at `c184cf1d8`; `after` is this tree. Same probe files,
    same runner.

    | probe | cases | before | after |
    | --- | --- | --- | --- |
    | the 44-spelling probe | 44 | 10 findings, 0 wrong | 10 findings, **0 wrong** |
    | the globals probe (13 globals + bare `step` ×2) | 15 | 13 findings, **15 wrong** | 2 findings, **0 wrong** |
    | the adversarial probe | 40 | 26 findings, **20 wrong** | 14 findings, 4 wrong |

    The globals probe before: all 13 globals gave a finding and both bare `step`
    forms gave none — 15 of 15 wrong, exactly the two findings. After: all 13
    give no finding and both `step` forms give one.

    The 4 remaining adversarial cases are `test.test`, `test.context`,
    `test.suite` and `test.it`. The 2026-08-10 08:12 review read those four and
    accepted them: each is rooted at `test`, which is itself the mark, so no
    finding is dropped on code a person writes.

    The adversarial probe also holds `fdescribe`, which the config at `c184cf1d8`
    reported and this tree does not.

    ## The sweep over `apps/` does not move

    444 `.ts` and `.tsx` files: 30 findings (29 complexity, 1 length), KEY FOR KEY
    identical to the run at `c184cf1d8`. Against the run with no carve-out: 0
    added, 40 dropped. `apps/` holds no bare `step` call and none of the 13
    globals, so both defects were silent here — the fixtures and the probes, not
    this workspace, are what prove them.

    ## The fixtures hold both directions

    - Fail fixture: 7 guards to 8. It adds a bare `step("build the grid", ...)`
      inside the `describe` block, beside a local `const step` that documents why
      the name is ordinary. Run key by key: `c184cf1d8` reports 7, this tree
      reports 8.
    - Pass fixture: adds 13 blocks, one for each Mocha and Jest global, each
      callback scoring 21 against the gate of 15. `c184cf1d8` reports all 13;
      this tree reports 0.
    - Both fixtures parse clean: eslint reports 0 fatal messages on each.

    ## The guard test, and a new one

    - `the_shipped_typescript_complexity_tool_rule_measures_every_fail_fixture_guard`
      now names all 8 guards.
    - `the_shipped_typescript_complexity_config_reads_its_framework_names` is new.
      It reads the rule file, takes the rule's OWN resolution line and the eslint
      config out of the here-document, appends one `console.log`, and runs the
      shipped config under node. It then holds four facts: standard error is empty
      (the read answered, the mirror did not stand in); the written mirror equals
      the read, both for the functions and for the modifier tier; the read holds
      all 13 globals the finding named; and the read drops `mocha`, `jest`,
      `expect`, `run` and `step`, with `step` the one rooted call.

    Checked RED before GREEN, two ways:

    - Drop one name from the mirror →
      `assertion left == right failed: the mirror in the rule must say what the
      read says`.
    - Take the root requirement off `step` → the new test fails with
      `only a framework function no framework spells bare needs a root`, AND the
      guard test fails with `the carve-out must leave the fail fixture guard
      step("build the grid", ( measured inside its describe block; the run
      reported [... 7 guards ...]`.

    ## Notes for the next agent

    - The mirror is a fallback, not the source. If a future `eslint-plugin-sonarjs`
      moves `cjs/helpers/test-frameworks.js`, or drops `globals`, the config writes
      the resolution error to standard error and keeps working on the mirror — and
      `the_shipped_typescript_complexity_config_reads_its_framework_names` goes red
      on the empty-stderr assertion, so the drift is loud.
    - `globals.jasmine` was NOT read. It adds `fail`, `pending`, `runs`, `waits`,
      `waitsFor` and the `spyOn` family, which are ordinary names, so reading it
      would trade this finding for a new false negative. sonarjs already gives the
      Jasmine focus and skip aliases that matter (`fdescribe`, `fit`, `xdescribe`,
      `xit`).
    - The probe harness stands at
      `/private/tmp/claude-501/-Users-wballard-github-swissarmyhammer-swissarmyhammer/bc2c1635-c6b9-4bba-9a81-0fce95d1ff03/scratchpad/h7/`.
      `cfg-now.sh` is the block extracted from the rule as it stands; `cfg-ship.sh`
      is the block at `c184cf1d8`.
  timestamp: 2026-08-10T13:39:39.625403+00:00
- actor: claude-code
  id: 01kznydhj6xkattgw2atv6ah14
  text: |
    ### implement — changed
    - evidence: 4 files — `builtin/validators/code-hygiene/rules/complexity-typescript.md`, `builtin/validators/code-hygiene/fixtures/complexity-typescript.fail.ts.tmpl`, `builtin/validators/code-hygiene/fixtures/complexity-typescript.pass.ts.tmpl`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`. 737 insertions, 58 deletions.
    - how the list is derived: READ from `eslint-plugin-sonarjs/cjs/helpers/test-frameworks.js` (`TEST_FRAMEWORK_STRUCTURE_FUNCTIONS`, 20 names) and from `globals.mocha` and `globals.jest` in the `globals` package the plugin declares as a dependency. Three facts inside `globals` drop the 4 names that open no test. 25 framework functions, 12 on the modern modifier tier. `step` is the one written name, and it is ROOTED, so a bare `step(...)` is measured.
    - probes, before (`c184cf1d8`) → after: 44-spelling probe 10 findings / 0 wrong → 10 / 0; globals probe (13 globals + bare `step` ×2) 13 findings / 15 wrong → 2 / 0; adversarial probe (40 cases) 26 findings / 20 wrong → 14 / 4, the 4 being `test.test`, `test.context`, `test.suite` and `test.it`, which the 08:12 review read and accepted.
    - sweep over the 444 `.ts` and `.tsx` files under `apps/`: 30 findings (29 complexity, 1 length), key for key identical to `c184cf1d8`. 0 added and 40 dropped against the run with no carve-out.
    - fixtures: fail fixture 7 guards → 8 (`c184cf1d8` reports 7, this tree 8); pass fixture 0 findings here and 13 at `c184cf1d8`. Both parse clean, 0 fatal messages.
    - doctor fixture pair: `every_shipped_complexity_tool_rule_passes_its_fixtures` PASS. Guard test: `the_shipped_typescript_complexity_tool_rule_measures_every_fail_fixture_guard` PASS with all 8 guards. New: `the_shipped_typescript_complexity_config_reads_its_framework_names` PASS, checked RED two ways first.
    - gates: `cargo nextest run --workspace` — 14024 tests run, 14024 passed, 0 failed, 0 skipped. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. `the_swift_package_root_restores_the_directory_before_it_removes_it` did not fail on this run.
    - no new dependency: `globals@^17.7.0` is a declared dependency of `eslint-plugin-sonarjs@4.2.0`, which the rule's own install command brings. The `doctor` and `install` blocks are unchanged.
    - doc: every sentence the two findings falsify is corrected. The modifier table is gone, because the tiers are derived; the "It drops exactly the findings the two prompt rules exempt" claim now names the three probes; the measurement section names the 13 globals and the bare `step`.
    - HEAD stays at `c184cf1d8`; nothing committed.
    - next: `/review`.
  timestamp: 2026-08-10T13:39:57.638586+00:00
- actor: claude-code
  id: 01kznze06rzmxe5k7yt0wc140t
  text: |
    ### review — findings

    - evidence: 1 finding — `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:424`. Scope `3a5fe83dd^..3a5fe83dd`, 9 validators attempted, 0 failed, 0 skipped. The duplicated helper is NEW in this commit (`shipped_rule_source` added at +97 of the diff; `shipped_fixture_template` pre-existed), so the exception for existing test code does not release it.

    #### What the probes proved about READING instead of writing

    The read was reproduced against the real tree: `eslint-plugin-sonarjs@4.2.0`, `globals@17.9.0`, resolved at `eslint-plugin-sonarjs/node_modules/globals/index.js`. `TEST_FRAMEWORK_STRUCTURE_FUNCTIONS` is a Set of 20. `globals.mocha` + `globals.jest` declare 26 names; the three `globals` facts drop `jest` and `mocha` (environments) and `expect` (chai), and the hand-written `run` drops one more. Union = 25 framework functions, 12 modern. Both equal the mirror.

    A failed read cannot take the eslint run down. Four scenarios were run end to end against a shadow module tree:

    - sonarjs RENAMES the export — `[...structure]` throws inside the try. eslint exits 0, the mirror stands in, `complexity-typescript: the framework function names did not resolve (structure is not iterable); the mirror in the rule stands in` goes to STDERR, the JSON on STDOUT stays intact, and the globals probe still scores 0 wrong.
    - `globals.vitest` removed — same path, exits 0, mirror stands in, 0 wrong.
    - helper file moved, and `globals` deleted — eslint exits 2, but the stack shows the failure inside sonarjs's OWN rule files (`cjs/S2004/rule.js`, `cjs/S2137/rule.js`), which require the same two modules. The plugin is broken in those trees regardless of this commit, and the rule doc already states that exit 2 marks the rule unusable and falls back to the prompt rules.

    One degradation is silent: `globals.mocha` present but emptied does not throw, writes nothing to stderr, and loses the five Mocha TDD openers (globals probe 5 wrong). The shipped test catches it — `the_shipped_typescript_complexity_config_reads_its_framework_names` asserts the read equals the mirror and holds every name in `TYPESCRIPT_FRAMEWORK_GLOBALS`, so the drift fails the suite rather than passing in silence.

    #### The mirror is not a hidden fourth list

    `MIRROR_FRAMEWORK_FUNCTION` and `MIRROR_MODERN_FUNCTION` are hand-written, and the rule says so where they stand. They are never the operative source unless the read throws, and the shipped test holds them equal to the read, so a `globals` release that moves a name fails the suite. That is the opposite of rounds 1 to 3, where the hand list WAS the operative source and no test held it to anything.

    `run` is hand-named and the justification is true: `run` stands in `globals.mocha`, and in neither `globals.chai`, nor the environment keys, nor the sonarjs structure set, so no file in the tree separates it. It is not the only hand-named item — `step` (rooted), the `test` root, and both modifier sets are written too — but each is stated in the doc and `step` and the root are asserted by the test through `TYPESCRIPT_ROOTED_CALL`.

    #### Measurements

    - 44-spelling probe: 10 findings, 0 wrong — same as round 3. Rounds 2 and 3 fixes preserved.
    - globals probe (15 cases): round 3 gave 15 wrong, this tree gives 0. The 13 Mocha and Jest openers are exempt, and bare `step` and `step.skip` are measured.
    - adversarial probe (40 cases): round 3 gave 20 wrong, this tree gives 4 — `test.test`, `test.context`, `test.suite`, `test.it`, the four accepted in an earlier round.
    - sweep over the 444 `.ts` and `.tsx` files under `apps/`: 30 findings, IDENTICAL key for key to both stored baselines.
    - `cargo test -p swissarmyhammer-validators --lib`: `the_shipped_typescript_complexity_config_reads_its_framework_names`, `the_shipped_typescript_complexity_tool_rule_measures_every_fail_fixture_guard`, `every_shipped_complexity_tool_rule_passes_its_fixtures` — 3 passed, 0 failed.
    - fail fixture guards: 8, and the test asserts the reported count equals 8 exactly. Pass fixture takes 13 Mocha and Jest blocks.
    - no new dependency: `eslint-plugin-sonarjs@4.2.0` declares `globals: ^17.7.0`. The commit touches no `package.json`, `Cargo.toml`, or `Cargo.lock` — 4 files, largest 39029 bytes, all under the 262144 rendered cap.

    - next: remove the duplicated path-resolution helper, then re-review.
  timestamp: 2026-08-10T13:57:41.208932+00:00
- actor: claude-code
  id: 01kznzf6y94xx86q7rtqhfzxn4
  text: |-
    ### finish iteration 4 — findings
    - implement: changed — 4 files; the framework-name list is now READ from `TEST_FRAMEWORK_STRUCTURE_FUNCTIONS` in `eslint-plugin-sonarjs` plus `globals.mocha`/`globals.jest`, not written by hand
    - test: green — `cargo nextest run --workspace` 14024 passed, 0 skipped; fmt clean; clippy clean
    - commit: 3a5fe83dd
    - review: findings — 1, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:424`, the new `shipped_rule_source` repeats the path-resolution body of `shipped_fixture_template`
    - THE CLASS IS CLOSED. Three rounds each found a gap in a hand-written list. This round found none. The one finding is a mechanical duplication in a test helper, not a framework spelling.
    - The reviewer broke the read four ways in a shadow module tree. A renamed export and a removed `globals.vitest` both throw inside the `try`: eslint exits 0, the written mirror stands in, the error goes to stderr, and the JSON on stdout stays parseable. The two cases that do exit 2 fail inside sonarjs's own rule files, which require the same two modules, so the plugin is unusable in those trees whatever this rule does.
    - One degradation is silent — `globals.mocha` present but emptied loses the five Mocha TDD openers without an error. The shipped test catches it, because it holds the read equal to the mirror.
    - The mirror is NOT a hidden fourth list: it is never operative unless the read throws, and a test pins it to the read. That inverts rounds 1-3, where the hand list was the operative source and nothing held it to anything.
    - guardrail: no finding repeats across the four ledger entries.
  timestamp: 2026-08-10T13:58:20.873722+00:00
- actor: claude-code
  id: 01kzp03dxy9samcz0xsq0n8b60
  text: |
    ### implement — changed

    Round 5 finding: `shipped_rule_source` repeated the path-resolution body of
    `shipped_fixture_template`.

    **What was extracted.** One lookup stands in the file now:

        fn shipped_asset(loader: &ValidatorLoader, kind: &ShippedAssetKind, name: &str) -> PathBuf

    `ShippedAssetKind` carries the three parts that differ between the two callers —
    `dir`, `suffix`, and `label` for the failure message. Two constants state the two
    kinds: `FIXTURE_TEMPLATE_ASSET` (`fixtures/`, `.tmpl`, "fixture template") and
    `RULE_SOURCE_ASSET` (`rules/`, `.md`, "rule source"). Both old helpers are gone,
    and so are the `RULES_DIR_NAME` and `RULE_SOURCE_SUFFIX` constants that only
    that helper read.

    **Why a value and not four parameters.** The finding names the shape
    `shipped_asset(loader, name, dir, suffix, asset_type)`. Four `&str` parameters of
    different meanings is the shape the Rust `type-safety` rule refuses ("Two
    parameters of the same primitive type with different meanings must use
    newtypes"), so the three parts travel in one typed value. The requirement the
    finding states — one canonical path-resolution implementation — is met.

    **Whole-file sweep.** The finding shows one example of a cause, so I searched for
    a third copy. `grep base_path` over `crates/swissarmyhammer-validators/src`
    returns 14 hits; only the two the finding names held the resolution body. There
    is no third copy in this file or elsewhere in the crate.

    **Behavior did not change.** The panic text reads word for word as before
    ("a builtin validator set must ship a {name} fixture template" /
    "... {name} rule source"), because `name` and `suffix` stay separate arguments.
    The three call sites resolve the same file.

    **Gates.**
    - `cargo fmt --all --check` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean, 0 warnings.
    - The three named tests plus the known-flaky neighbour, run first on their own:
      4 tests run, 4 passed.
    - `cargo nextest run --workspace` — 14024 tests run, 14024 passed, 0 skipped.
      `the_swift_package_root_restores_the_directory_before_it_removes_it` passed;
      the known working-directory race did not fire.

    Files: `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`
    (one file, +33 / −44 lines).
  timestamp: 2026-08-10T14:09:23.390892+00:00
- actor: claude-code
  id: 01kzp0nv0r15ndse77ytcg2e5q
  text: |-
    ### review — clean

    - scope: `review sha e79853df9^..e79853df9` (one commit, one file: `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`)
    - engine counts: 3 candidates confirmed, 1 refuted, 9 attempted, 0 failed
    - recorded findings: 0. All 3 candidates fall under the review skill's blanket exception, which forbids raising, recording, or relaying a finding whose subject is changing test code that already existed:
      - `shipped.rs:110` `complexity_work(rule, path, content)` — three `&str` parameters. The commit does not touch this function. Diff hunks stand at 281, 298, 320, 324, 375, 446, 566 and 676.
      - `shipped.rs:873` `manifests_work(path, content)` — two `&str` parameters. The commit does not touch this function either.
      - `shipped.rs:324` — panic on a missing asset. The line stands in the delta, but the panic is carried word for word from both earlier helpers. The cause the finding names is the error handling of the whole existing test file, which uses `.expect` and `.unwrap` at every setup site.
    - the two type-safety candidates confirm the reasoning this commit states. The rule flags a signature of several `&str` parameters. `shipped_asset(loader, &ShippedAssetKind, name)` carries one `&str` and one typed value, and the rule does not flag it. The typed value answers the rule the four-parameter shape would have broken.
    - verification of the claims:
      - one path resolution stands. `base_path` appears one time in the file, at line 319. `shipped_fixture_template` and `shipped_rule_source` are both gone.
      - the failure message is unchanged. `panic!("a builtin validator set must ship a {name} {}", kind.label)` with `label: "rule source"` and `label: "fixture template"` gives the same two strings as before.
      - `RULES_DIR_NAME` and `RULE_SOURCE_SUFFIX` are removed, and nothing names them.
      - the three tests pass: `every_shipped_complexity_tool_rule_passes_its_fixtures`, `the_shipped_typescript_complexity_tool_rule_measures_every_fail_fixture_guard`, `the_shipped_typescript_complexity_config_reads_its_framework_names`. `cargo test -p swissarmyhammer-validators --lib` gives 3 passed, 0 failed, and no warning.
    - every prior `## Review Findings` item is checked.
    - next: none. The card moves to done.
  timestamp: 2026-08-10T14:19:26.616078+00:00
- actor: claude-code
  id: 01kzp0q4p35vfx84kqvdb3a8kq
  text: |-
    ### finish iteration 5 — clean
    - implement: changed — 1 file; `shipped_fixture_template` and `shipped_rule_source` merged into one `shipped_asset` lookup with a typed `ShippedAssetKind`
    - test: green — `cargo nextest run --workspace` 14024 passed, 0 skipped; fmt clean; clippy clean
    - commit: e79853df9
    - review: clean — the engine gave 3 candidates and all 3 were dropped under the written test-refactoring exception, because they name code the commit does not introduce (`shipped.rs:110` and `:873` are untouched; the panic at `:324` is carried word for word from both old helpers). `base_path` now appears one time in the file. The failure message is unchanged. The 3 named tests pass.
    - The typed-value choice was checked rather than accepted: the Rust `type-safety` rule fired twice in this same file on signatures of several bare `&str` parameters, and it did NOT fire on `shipped_asset`. The four-parameter shape the finding sketched is the shape that rule refuses, so the fix answered the duplication finding without making a type-safety one.
    - result: the card is in `done` after five iterations.
    - carried forward, not a finding: `FIXTURE_TEMPLATE_ASSET` names its parts through constants while `RULE_SOURCE_ASSET` inlines `"rules"` and `".md"`. The two kind constants are asymmetric in that one respect.
  timestamp: 2026-08-10T14:20:09.283208+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffd580
title: complexity-typescript supersedes function-length but drops its test carve-out
---
`complexity-typescript` declares `supersedes: [cognitive-complexity, function-length]`. It therefore does the whole job of the `function-length` prompt rule. But `function-length.md:20` exempts "Functions explicitly marked as tests", and eslint does not know about that exemption. The tool rule takes the job and drops the carve-out.

The result is findings the superseded rule would never have made. Over the 444 `.ts` and `.tsx` files under `apps/`, 36 of the 37 length findings are `describe(...)` arrow callbacks in a `*.test.tsx` file, and 23 of the 33 complexity findings are also in test files. That is about 59 of the 70 TypeScript findings.

The rule's own doc already states this at `builtin/validators/code-hygiene/rules/complexity-typescript.md:99-105`.

## This is not a duplication problem

`duplication` never sees a `describe` block. The test `a_typescript_describe_block_contributes_no_definition` at `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs:1337` puts a `function helper()` inside `describe('rows', ...)` and asserts that the file gives only the top-level `live`. The structural `TestSpec` reads the call target and excludes the block. That half is correct.

The tool is not wrong about the measurement. A `describe` callback is a real arrow function of more than 250 lines. The tool measures a unit that the rule it replaced had already decided not to count.

## The constraint

Do NOT exclude test code by path or by glob as a substitute for judgment. See the `duplication` precedent: the project chose a structural `TestSpec` that reads the parse — attribute text, definition name, call target, base list.

eslint cannot state the same fact. `max-lines-per-function` takes only `max`, `skipBlankLines`, `skipComments` and `IIFEs`. It has no filter on the name of the call that holds the function.

## The options

- [x] Decide how the rule reproduces the carve-out, and write the decision on this card with the evidence for it.
  - **A. Scope the two eslint rules off for test files.** The rule writes its own flat config, so a second config block can name `**/*.test.{ts,tsx}` and `**/*.spec.{ts,tsx}`. One structural fact, stated one time. It is BROADER than the prompt rule: it also exempts a helper function in a test file that is genuinely too long. State that trade in the rule doc if you choose it.
  - **B. Drop `function-length` from `supersedes` for TypeScript.** The prompt rule then keeps the carve-out, and eslint keeps only `sonarjs/cognitive-complexity`. This costs LLM calls that the tool rule exists to remove.
  - **C. Write the inline suppression on each block.** This is what the doc says today. It states the exemption 36 times, and a person must add it again for every new test file. Recommend against it, but say so on the card rather than in silence.
- [x] Make the finding count on this workspace match the decision, and record the new count.

Recommendation: A. It is one fact in the file that owns the whole eslint invocation, and it matches how the rule already owns its config. The extra reach — a long helper in a test file — is small and can be stated.

### What was done

A's shape, with the fact stated as the test-framework CALL rather than the file path. The premise "eslint cannot state the same fact" does not hold: the config is JavaScript, so it wraps each of the two eslint rules and drops a report whose measured function is the argument of a test-framework call. No new dependency, and no path glob. The file-glob form was rejected on measurement — it would drop 19 complexity findings on NAMED HELPERS in test files, which `cognitive-complexity.md` names word for word as still listed. Counts: 70 before (33 complexity, 37 length), 30 after (29 complexity, 1 length). See the decision comment for the evidence.

## The wider rule this is one instance of

A `supersedes` claim that the tool does not honor is a false claim. This rule already documents ONE dropped gate: the prompt rule's condition-nesting gate has no eslint rule, so superseding drops it (`complexity-typescript.md:78-80`). That trade loses findings. Dropping a carve-out is the opposite trade: it MAKES findings the superseded rule would not make. The second is worse, because every one of those findings is then a binary requirement that nobody should act on.

- [x] Read every rule that declares `supersedes` and say whether the prompt rule it replaces has an exemption the tool does not reproduce. Record the answer for each. File a card for each gap found; do not fix them here.

24 rules read, 22 gaps, 22 cards filed. `dead-code-dart` and `unused-code-go` reproduce every carve-out. See the survey comment for the answer for each rule and the card for each gap.

## Acceptance

- The carve-out decision is implemented, and the rule doc states which option was taken and why.
- A `review working` over `apps/` reports the new count, and the count matches the decision.
- The doctor fixture pair still proves the rule.
- No new dependency, and no path-glob exclusion used as a substitute for judgment outside the eslint config itself. #tool-validators #objectivity

## Review Findings (2026-08-10 06:54)

- [x] `builtin/validators/code-hygiene/fixtures/complexity-typescript.fail.ts.tmpl:35` — The fail fixture holds only `function` declarations in the `describe` block: `foldGrid` at line 35 and `mixState` at line 63. A declaration anchors correctly, so the fixture cannot find the method and accessor hole. Put a method or an accessor that goes over a gate in the `describe` block, so the fixture proves that a named helper of EVERY shape stays measured.
- [x] `builtin/validators/code-hygiene/rules/complexity-typescript.md:46` — `rootCalleeName` walks a member chain and a call chain to the root identifier, and it puts no limit on the property names between them. The mark thus reads "the root identifier has a test name", not "the call is a test-framework call". Any call whose root identifier is `describe`, `it`, `test`, `suite`, `context`, `beforeAll`, `beforeEach`, `afterAll` or `afterEach` exempts its function arguments. Measured with the shipped config: `context.run(() => { ... })`, with a body that scores 21 against the gate of 15, gives NO finding. `context` is a usual identifier for a React context, a request context and `AsyncLocalStorage`. The statement at `complexity-typescript.md:235-236`, "it drops nothing outside a test-framework callback", is thus not true. Limit the accepted properties to the test modifiers, such as `only`, `skip` and `each`.
- [x] `builtin/validators/code-hygiene/rules/complexity-typescript.md:68` — `innermostAt` anchors the exemption on the REPORT location. But both gates report at the function head, and for a method or an accessor the head starts at the NAME, which is outside the function node's own range. The lookup thus climbs to the enclosing function. A method or an accessor in a test-framework callback is therefore exempt from both gates. Measured with the shipped config: an object shorthand method, a class method and a getter, each scoring 21, give a finding at the top level and give NO finding in a `describe` block; a 264-line method gives the length finding at the top level and NO finding in a `describe` block. This is the same class of false negative that the file-glob form was refused for. The doc premise at `complexity-typescript.md:283-285` — "Both rules report at the head of the function they measure ... so the innermost function around that point is the function under measurement" — is not true for a method or an accessor. Anchor the exemption on the function node the rule measures, not on the report location.

## Review Findings (2026-08-10 07:28)

Scope: `da41103e6^..da41103e6`.

- [x] `builtin/validators/code-hygiene/rules/complexity-typescript.md:41` — `TEST_MODIFIER` refuses every property Playwright spells on the `test` root, so the carve-out now MAKES findings the superseded prompt rules exempt. Playwright writes its whole surface as `test.describe`, `test.beforeEach`, `test.afterEach`, `test.step`, `test.describe.serial`, `test.describe.parallel`, `test.fixme` and `test.slow`. None of `describe`, `beforeEach`, `afterEach`, `step`, `serial`, `parallel`, `fixme` or `slow` stands in the modifier list, so `isTestCall` returns at the first property and never reaches the root identifier `test`. Measured on a probe whose every callback scores 21 against the gate of 15, run twice with the same probe: the config at `65eea58d0` reports 6 findings, and the config in this commit reports 14. The 8 added findings are exactly those 8 Playwright openers. `function-length` exempts "Functions explicitly marked as tests", and a `test.describe` callback carries that mark, so each of the 8 is a finding the superseded rule would never make. That is the same false claim this card was opened to remove, in the opposite direction. The statement at `complexity-typescript.md:261-262` — "It drops exactly the findings the two prompt rules exempt" — is therefore not true. This workspace does not hold the shape, so the count over `apps/` does not move; the two defects this commit fixed were latent here too, and the commit answers that with the fixture and not with the workspace. Do the same here: accept the framework root name as a property as well, and put a Playwright-shaped guard in the pass fixture.
- [x] `builtin/validators/code-hygiene/rules/complexity-typescript.md:41` — The mark still reads a NAME PAIR and not the shape of the call, so a call whose object is not a test framework carries it. The condition is "the root identifier has a test name AND every property stands in the modifier list", and `each` and `for` are ordinary collection method names, so `context.each(rows)(fn)` and `context.for(rows)(fn)` both carry the mark. Measured with the shipped config on a probe whose every callback scores 21 against the gate of 15: both give NO finding. The doc names `context` itself as "a usual name for a React context, a request context and an `AsyncLocalStorage`". The statement added at `complexity-typescript.md:235-236` — the second condition "makes the mark read 'this is a test-framework call' and not 'the root identifier has a test name'" — is thus not true for `context.each` and for `context.for`.

### How both were closed

The two findings pull in opposite directions, so neither flat list could move on its own. The mark now reads the callee as a CHAIN OF SEGMENTS with a role for each segment.

- One segment must be a FRAMEWORK FUNCTION: `describe`, `it`, `test`, `suite`, `context`, `step`, `beforeAll`, `beforeEach`, `afterAll`, `afterEach`.
- Each segment BEFORE it must be a FRAMEWORK ROOT. `test` is the one root, because Playwright puts its whole surface on `test`.
- Each segment AFTER it must be a modifier THAT framework function accepts. `describe`, `it`, `test` and `suite` accept the full Vitest, Jest and Playwright set; `context` and `step` accept `only` and `skip`; the four hooks accept none.
- The chain is read from the last segment to the first, so `test.describe` takes `describe` as the framework function and `test` as the root, and `test.each(rows)` takes `test` as the framework function and `each` as a modifier.

Finding 1 falls out of the root. Finding 2 falls out of the per-function modifier: Mocha has no `context.each`, so `each` is not in `context`'s set.

Measured on a 44-spelling probe covering both frameworks, every callback at score 21 against the gate of 15: `65eea58d0` gives 5 findings and 7 wrong cases, `da41103e6` gives 20 findings and 18 wrong cases, this tree gives 10 findings and 0 wrong cases. The sweep over `apps/` stays at 30 (29 complexity, 1 length), key for key against both earlier configs. The fail fixture goes from 5 guards to 7, and the pass fixture takes four Playwright blocks that `da41103e6` reports and this tree does not. See the comment thread for the full table.

## Review Findings (2026-08-10 08:12)

Scope: `c184cf1d8^..c184cf1d8`.

- [x] `builtin/validators/code-hygiene/rules/complexity-typescript.md:51` — This commit adds `["step", MOCHA_MODIFIER]` to `FRAMEWORK_CALL`, so a BARE `step(...)` call carries the mark. No framework in scope spells `step` bare. The `globals` package inside the installed `eslint-plugin-sonarjs` tree — the same `node_modules` tree the config resolves through — lists `step` in neither `globals.mocha` nor `globals.jest`, and Playwright spells it only as `test.step`. The mark therefore exempts ordinary code. Measured with the shipped config on a probe whose every callback scores 21 against the gate of 15: `step("build", () => { ... })` and `step.skip("build", () => { ... })` give NO finding, while the config at `65eea58d0` and the config at `da41103e6` each give one finding for each of the two. That is a new false negative this commit adds, and each dropped finding is one `function-length` and `cognitive-complexity` would make on a build step, a wizard step or a saga step. The chain grammar cannot say "this framework function needs the `test` root", which is the one thing `step` needs: it holds a role for a segment BEFORE the framework function but no rule that such a segment must be present. Give the grammar that role, or take `step` out of `FRAMEWORK_CALL` and carry `test.step` another way, and put a guard for a bare `step` in the fail fixture.
- [x] `builtin/validators/code-hygiene/rules/complexity-typescript.md:45` — `FRAMEWORK_CALL` names ten framework functions, and thirteen further Mocha and Jest test openers and hooks stand outside it, so each one MAKES a finding the two superseded rules exempt. Read from the `globals` package inside the installed `eslint-plugin-sonarjs` tree: `globals.mocha` holds `before`, `after`, `setup`, `teardown`, `suiteSetup`, `suiteTeardown`, `specify`, `xdescribe`, `xcontext`, `xit` and `xspecify`; `globals.jest` adds `fit` and `xtest`. Mocha's own suite hooks are `before` and `after`, not the `beforeAll` and `afterAll` the map holds. Measured with the shipped config on a probe whose every callback scores 21 against the gate of 15: all thirteen give one finding each, thirteen of thirteen. `function-length` exempts "Functions explicitly marked as tests", and a Mocha `before(() => { ... })` carries that mark, so each of the thirteen is a finding the superseded rule would never make — the same class this card was opened to remove. The statement at `complexity-typescript.md:339-340`, "It drops exactly the findings the two prompt rules exempt", is therefore not true, and the measurement written at `complexity-typescript.md:299-312` names Mocha while holding no Mocha hook. The 44-spelling probe and both fixtures reproduce clean on this tree, so the gap is silent here as well: add the missing names and put a Mocha-shaped and a Jest-shaped guard in the pass fixture.

### How both were closed — the list is READ, not written

Three rounds each hand-curated a list of framework spellings, and each was wrong in one direction or the other. This round stops writing the list and reads it out of the same `node_modules` tree the config already resolves through.

- `eslint-plugin-sonarjs/cjs/helpers/test-frameworks.js` exports `TEST_FRAMEWORK_STRUCTURE_FUNCTIONS`, the 20 functions the plugin itself calls the ones "whose callbacks define test structure rather than business logic".
- `globals`, a declared dependency of `eslint-plugin-sonarjs`, holds `globals.mocha` and `globals.jest`. Mocha's TDD interface — `setup`, `teardown`, `suiteSetup`, `suiteTeardown`, `xspecify` — stands only there.
- Three facts inside `globals` take the names that open no test: a name that is itself a `globals` environment is the framework namespace object (`mocha`, `jest`); a name in `globals.chai` is an assertion entry (`expect`); and `run` is Mocha's delayed-start runner, which takes no callback. `run` is the one name no other file in the tree separates, so the config names it and states why.
- The two reads give 25 framework functions. The modifier tier is read too: a name `globals.jest` or `globals.vitest` declares takes the full Jest, Vitest and Playwright set, and a name only `globals.mocha` declares takes `only` and `skip`. `context` therefore keeps the round-2 fix without a written rule.

Finding 1 falls out of a new ROOTED role in the chain grammar. `globals` ships no Playwright environment, so `step` is written, and it is rooted: a rooted framework function needs a framework root before it. `test.step` and `test.step.skip` are exempt; a bare `step(...)` and `step.skip(...)` are measured.

No new dependency. `globals@^17.7.0` is a declared dependency of `eslint-plugin-sonarjs@4.2.0`, so the rule's own `npm install -g` command brings it, and the config resolves it from the plugin's own directory.

## Review Findings (2026-08-10 08:42)

Scope: `3a5fe83dd^..3a5fe83dd`.

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:424` — Function `shipped_rule_source` reimplements the exact same pattern as `shipped_fixture_template` (lines 286–298). Both iterate through rulesets, join a base path with a directory and suffix, search for an existing file, and panic with a similar message. The only differences are the directory name and suffix constant. This should be a single parameterized helper to avoid maintaining two copies of the same logic. Extract a single generic helper function `shipped_asset(loader: &ValidatorLoader, name: &str, dir: &str, suffix: &str, asset_type: &str) -> PathBuf` that takes the directory and suffix as parameters, or refactor `shipped_rule_source` to call `shipped_fixture_template` by extending it with a generic parameter. This keeps one canonical path-resolution implementation that will be fixed once if the logic changes.

### How it was closed

One path resolution stands in the file now. `shipped_asset(loader, kind, name)` does the whole lookup, and a `ShippedAssetKind` value carries the three parts that differ: the directory, the suffix, and the word for the failure message. Two constants state the two kinds — `FIXTURE_TEMPLATE_ASSET` (`fixtures/`, `.tmpl`) and `RULE_SOURCE_ASSET` (`rules/`, `.md`). `shipped_fixture_template` and `shipped_rule_source` are both gone, and the three call sites name a kind constant.

The parts travel in one value rather than as four separate `&str` parameters, because four string parameters of different meanings is the shape the Rust type-safety rule refuses. Behavior does not change: the failure messages read word for word as before, and the same file is found.

The file holds no third copy of the body. A search over the crate for `base_path` finds only the two the finding names.