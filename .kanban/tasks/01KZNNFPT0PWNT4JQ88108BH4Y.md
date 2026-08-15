---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m01av0nkn6k37aksgkg5m9r9
  text: |-
    ## Tool survey, done before the decision

    The whole JS/TS unused-export space was read again (versions, publish dates, archive state, inline suppression, entry-point awareness), and knip was RUN over the corpus.

    - `ts-prune` 0.10.3 was published 2021-12-12. Its GitHub repository is ARCHIVED, and its README carries a maintenance notice its author added 2025-09-19 naming `knip` as the successor. `depcheck` and `unimported` are archived too and each names knip.
    - `knip` 6.32.2 is active, reads entry files from `package.json` `main`/`bin`/`exports`, and ships a plugin per framework whose config file becomes an entry file. It answers BOTH carve-outs of this card natively.
    - `oxlint` and Biome cannot answer the question at all: `import/no-unused-modules` is unchecked in the oxc tracking issue, and Biome's `noUnusedExports` is an open discussion.
    - knip measured over the corpus: zod 13, zustand 0, redux 2 — against this rule's 78, 1, 6.

    knip was NOT taken, and the swap is a card of its own (^3r5bhpj). Two properties block it: knip has no line-comment suppression at all (its docs refuse one), so the staging contract would move to a JSDoc tag that states "public" rather than "a consumer lands next"; and knip exits 1 for findings and 2 for a broken run, so the script has to tell those apart.

    ## The decision the card asked for

    The card named two mechanisms. Measurement refutes the second one AS STATED and answers it another way.

    `package.json` `main`/`exports` names the paths a package PUBLISHES, and a library that builds publishes BUILD OUTPUT. Measured over the corpus, counting each entry path and asking whether it names a source file of the tree: zod 9 of 37, redux 0 of 4, zustand 0 of 5. `redux` names `dist/cjs/redux.cjs` for a source of `src/index.ts`; only `tsup.config.ts` states that mapping.

    So the run reads TWO facts, and both are the package stating its own surface:

    1. `package.json` — `main`, `module`, `browser`, `types`, `typings`, every string leaf of `exports`, every value of `bin`. Reading the whole `exports` map is what finds zod, which writes `"@zod/source": "./src/index.ts"` beside its build output.
    2. `tsconfig.json` `compilerOptions.paths` — every target of a mapping whose key is a workspace package's own NAME. That is the self-reference a repository writes so its own tests import the package the way an outside caller does. zustand writes `"zustand": ["./src/index.ts"]`, redux writes `"redux": ["./src/index.ts"]`. A key that names no package — redux's `"@internal/*"` — states nothing.

    The entry modules go to ts-prune's own `--ignore`, which is `--retain-public` for TypeScript. The marker stays ONLY for the framework-registered shape, which ts-prune has no mechanism to see.

    ## Discoveries

    - **ts-prune reads a project configuration through cosmiconfig, and merges it UNDER the command line.** A `package.json` holding `"ts-prune": {"ignore": "src"}` silenced the WHOLE gate. Measured: the old script reported 0 findings on a probe with one dead export; the shipped script now states `--ignore` and `--skip` on every call and reports 1. That was a live hole through which any project could turn the rule off.
    - **`--ignore` is matched against the whole REPORT LINE, not the path** (`presented.filter(file => !file.match(config.ignore))` in `lib/runner.js`). The pattern is therefore anchored `^(?:<path>|<path>):` and each path escaped.
    - **`tsc --showConfig` is needed, not `JSON.parse`.** `paths` usually stands in an EXTENDED file (redux `tsconfig.base.json`, zod `.configs/tsconfig.base.json`), and real tsconfigs hold comments (redux) and trailing commas (zod) that `JSON.parse` refuses.
    - **`--showConfig` `files` is the ROOT file list, not the program.** A first attempt matched entries against it and missed `packages/bench`, whose program reaches zod's `src/index.ts` by path. The script now resolves declared paths on the filesystem.
    - **ts-prune's presenter writes an absolute path minus its leading separator for a file outside the project.** The rule then prefixes the project path onto it, so 284 zod findings named a file that exists nowhere. Filed as ^yxky1aj.
    - **The pipe still answers zero for a crashed ts-prune.** Measured on a `tsconfig.json` of bytes that are not JSON: `@ts-morph/common` throws, and the shipped script reports 0 findings at exit 0. Pre-existing; filed as ^gxncs25.
    - A `*` in `exports` is DROPPED and a `*` in `paths` is EXPANDED. zustand's `"./*": "./*.js"` maps build output at the package root, so expanding it would have exempted every file of the repository, tests included.
  timestamp: 2026-08-14T23:48:40.755257+00:00
- actor: claude-code
  id: 01m01avh0w3aj122wraqd7f7gh
  text: |-
    ## The corpus, and the measurement

    Cloned at HEAD 2026-08-14, each a PUBLISHED library with a real `src/index.ts`, beside this workspace as the private-application control:

    | repository | commit | `.ts`/`.tsx` files | tsconfig projects |
    |---|---|---|---|
    | colinhacks/zod | `4e1720c` | 424 | 9 |
    | pmndrs/zustand | `2115efb` | 34 | 2 |
    | reduxjs/redux | `3084fc3` | 53 | 3 |
    | this workspace | HEAD | — | 2 |

    Findings, before and after the entry carve-out, run through the SHIPPED script extracted from the rule frontmatter:

    | workspace | before | after | time |
    |---|---|---|---|
    | zod | 1946 | 78 | 7.6 s |
    | zustand | 9 | 1 | 0.7 s |
    | redux | 14 | 6 | 1.0 s |
    | this workspace | 58 | 58 | 6.2 s |

    This workspace does not move, and that is right: both of its TypeScript projects state `"private": true`, name no `main`, no `exports` and no self `paths` mapping, so neither declares a surface.

    Of the 143 findings left over the four workspaces, 69 are the framework-registered shape the marker answers: 24 Next.js app-router modules, 4 page-directory modules, 10 configuration `default` exports, 16 components only an `.mdx` page names, and the 15 in this workspace the card's own hand-check found (`src/test/stubs/`, `src/test/integration-commands.ts`).

    The test carve-out was measured over the 12 `tsconfig.json` projects of the four workspaces: every one holds each test file that stands beside the sources it names. The consequence when a project does exclude them is measured on a probe and stated in the rule body.

    ## What did NOT change, and why

    **The fixture pair is unchanged.** Neither carve-out can be gated by it. Doctor counts only the findings a run reports ABOUT the fixture under test, and both carve-outs take findings off a DIFFERENT file — the package's entry module — so a `package.json.tmpl` in `fixtures/` would move neither the fail count nor the pass count. A manifest there would also enter the doctor run of the five other rules that read that directory, for a gate it cannot hold. The rule body and the test module doc each state this.

    The five acceptance tests in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs` carry it instead, driving the shipped script over probe repositories. Watched RED first: 3 of the 5 failed against the old rule with exactly the expected messages (`left: ["src/index.ts:2", "src/lib.ts:2"] right: ["src/lib.ts:2"]`, and the project-config probe reporting nothing), and all 5 pass against the new one. The existing pair is still gated by `every_shipped_dead_code_tool_rule_passes_its_fixtures`, which passes.
  timestamp: 2026-08-14T23:48:57.500735+00:00
- actor: claude-code
  id: 01m01avs736d3yax21bqqvj9nc
  text: |-
    ### implement — changed
    - evidence: 6 files — builtin/validators/code-hygiene/rules/dead-code-typescript.md, builtin/validators/code-hygiene/VALIDATOR.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs (new), crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/temp_directory.rs
    - tests: `cargo nextest run -p swissarmyhammer-validators` 743 passed, 0 failed; `cargo nextest run -p mirdan` 522 passed; `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean
    - follow-up cards: ^3r5bhpj (knip swap), ^gxncs25 (silent zero on a ts-prune crash), ^yxky1aj (absolute path prefixed onto a project path)
    - next: /review
  timestamp: 2026-08-14T23:49:05.891716+00:00
- actor: claude-code
  id: 01m01bbqcr3wymsvvr8tqf1bje
  text: |-
    ### review — clean
    - evidence: `review sha 7f4ef93a3~1..7f4ef93a3`, diffs only, 4 files reviewed, 0 not reviewed. 9 validator runs attempted, 0 failed, 0 skipped. counts: 0 findings, 0 confirmed, 0 refuted. No prior Review Findings section on the card, so no unchecked item stays open.
    - findings dropped for a false premise: 0. The engine raised no finding, so no claim needed a read to confirm.
    - judged the one deviation the implementer stated: no fixture pair was added.
      - The premise is TRUE, and it was read, not assumed. `crates/swissarmyhammer-validators/src/doctor.rs` `run_fixture` counts only the findings whose file name equals the fixture under test. Both carve-outs take findings off `src/index.ts`, a different file.
      - One qualification, which does not change the decision. The pass half of a pair CAN hold the entry carve-out if the pass fixture is made the published entry, so "a pair cannot distinguish" is stronger than what holds. The cost makes the decision right anyway: the pass fixture would stop holding the marker contract it now holds for five kinds of export, one pair holds one bit and there are two entry paths, and it can hold neither negative control. `builtin/validators/code-hygiene/fixtures/` also carries no `package.json` and is shared by 4 TypeScript rules and 8 TypeScript fixtures, which is the second stated reason and it is correct.
      - The 5 acceptance tests DO hold the carve-outs, and they run green: `cargo test -p swissarmyhammer-validators --lib shipped::dead_code_typescript` 5 passed, 0 failed. `verify_shipped_tree_reports` asserts the EXACT reported set, so each probe holds absence and presence together, which a fixture pair cannot do.
        - Carve-out 1 (published surface) is held twice, each with a control: the `exports` `source` probe and the `paths` self-reference probe each expect `["src/lib.ts:2"]` alone, so the entry is silent AND the ordinary module still reports. The `@internal/*` alias in the second probe maps every source file and must exempt nothing, which catches an over-broad carve-out.
        - Carve-out 2 (framework-registered) is held as the MARKER contract only, not as detection. The rule body states outright that ts-prune has no plugin, no configuration reader and no framework roster, and 69 of the 143 remaining findings are this shape and each needs a hand-written marker. The card granted this by naming the annotation contract and saying "Decide."
        - The test carve-out is held two-sided by the in-program and out-of-program probes, and the project-config probe closes the live hole where a project's own ts-prune settings silenced the gate.
      - "3 of 5 watched RED" is consistent with the change: probes 1 and 2 would report the extra `src/index.ts:2` under the old rule and the project-config probe would report nothing. History was not replayed.
    - next: none, the card is done.
  timestamp: 2026-08-14T23:57:48.312013+00:00
- actor: claude-code
  id: 01m01bd5r7nykf3qwfy93tkcap
  text: |-
    ### finish iteration 1 — clean. Card done.
    - **The card's proposed mechanism was REFUTED as stated.** It suggested reading `package.json` `main`/`exports`. Those name BUILD OUTPUT: only 9 of 37 zod entry paths, 0 of 4 redux and 0 of 5 zustand resolve to a source file. So the run reads TWO facts, both the package stating its own surface — every string leaf of `exports` plus `main`/`module`/`browser`/`types`/`typings`/`bin`, AND every `compilerOptions.paths` target whose key is a workspace package's OWN name, since a self-name mapping states the surface while a generic `@internal/*` alias does not. Both go to ts-prune's `--ignore`.
    - The mandatory marker survives only for the framework-registered shape, which ts-prune has no mechanism to see — no plugin, no config reader, no framework roster. 69 of the 143 remaining findings are that shape. Held as a MARKER CONTRACT, not as detection, and stated plainly in the rule.
    - **A live hole was closed on the way**: a project's own `package.json#ts-prune {"ignore": "src"}` silenced the ENTIRE gate — 0 findings before, 1 after the run began stating its own `--ignore`/`--skip`.
    - Corpus at HEAD 2026-08-14: zod 4e1720c 424 files 1946→78; zustand 2115efb 34 files 9→1; redux 3084fc3 53 files 14→6; this workspace 58→58, correct because both its TS projects are private and publish nothing.
    - **Tool survey found ts-prune ARCHIVED** — published 2021-12-12, repo archived, maintenance notice its own author added 2025-09-19 naming `knip` as successor. knip answers BOTH carve-outs natively (entry files from package.json, plus a plugin per framework): zod 13 against this rule's 78. knip was not taken because the card names ts-prune; the swap is card ^3r5bhpj. Also filed: ^gxncs25 (the pipe answers zero when ts-prune crashes) and ^yxky1aj (project path prefixed onto an absolute path).
    - **Deliberate deviation, and the reviewer judged it rather than accepting it**: no fixture pair was added, because doctor's `run_fixture` counts only findings whose file name equals the fixture under test, while both carve-outs remove findings from a DIFFERENT file — the entry module. The reviewer read `doctor.rs` and confirmed the premise, then qualified it honestly: the PASS half alone could hold the entry carve-out if the pass fixture were made the published entry, but it would lose the marker contract for five export kinds it currently carries, one pair carries one bit against two entry paths, and a pair can carry no negative control. Five acceptance tests over probe repositories carry the facts instead, 3 of 5 watched RED first, and `verify_shipped_tree_reports` asserts the EXACT reported set so each probe holds absence and presence together — which a fixture pair cannot do.
    - Carve-out 1 is held twice, each with a control: the `exports` probe and the `paths` self-reference probe each expect `["src/lib.ts:2"]` alone, so the entry module is silent while the ordinary module still reports, and the second adds `"@internal/*"` mapping every source file which must exempt nothing — that control catches an over-broad carve-out, the dangerous failure.
    - test: green — 743 validators, 522 mirdan. fmt and clippy clean.
    - commit: 7f4ef93a3
    - review: clean — 0 findings, 9 runs attempted, 0 failed, 0 skipped, 4 files reviewed.
  timestamp: 2026-08-14T23:58:35.783786+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff8880
title: dead-code-typescript reports library entry points and framework-registered exports as dead
---
`builtin/validators/code-hygiene/rules/dead-code-typescript.md` runs `ts-prune` per `tsconfig.json` and declares `supersedes: [dead-code]`.

Two carve-outs of `dead-code.md` are dropped.

- "**Exported public API**: ... Its callers live outside this repo, so an empty inbound callgraph is expected, not dead." The rule states its position outright: "TypeScript has no `pub` and no `__all__` — every `export` is the module's surface." So for a published library package, every entry of `src/index.ts` that no in-repo module imports is reported as dead. ts-prune has no concept of a package entry point.
- "**Entry points**: ... framework-invoked handlers, CLI command callbacks, registered hooks/callbacks". The rule names the two shapes and turns the exemption into a mandatory marker: "a module a bundler aliases by path, and a function a framework registers by name." Its own hand-check found 3 of 10 sampled findings were exactly this — `resolve.alias` targets in `vite.config.ts`, and vitest browser commands.

The test carve-out depends on a file the rule reads but does not control: if a project's `tsconfig.json` excludes `*.test.ts`, every helper exported only for tests looks unimported and reports.

`// ts-prune-ignore-next` works, so an annotation contract is available. `package.json` `main`/`exports` names the entry points and could be read. Decide.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity