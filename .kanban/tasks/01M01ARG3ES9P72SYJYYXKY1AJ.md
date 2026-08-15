---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m02v5p48w751hy30pxbwmf7b
  text: |-
    NOT MOOTED, and it stays ts-prune-specific. ^3r5bhpj decided the knip question and KEPT `ts-prune`, so this defect stands and its fix is still owed.

    The defect reproduces exactly as this card describes. Measured over `zod` at `4e1720c` with the shipped script, `node_modules` installed: 1 line of 76 names a path that does not exist on disk, and it is the `packages/bench` project reaching `packages/zod/src/v4/core/standard-schema.ts`. The same 1 of 78 reproduces on a bare clone, so the install state does not move it. The leaked path also embeds the absolute location of the checkout, so the line is not portable between machines.

    Recorded for the record: a swap to knip WOULD have eliminated this defect structurally, because knip runs ONCE at the workspace root and writes each path relative to that root, so there is no per-project prefix arithmetic at all. That is not a reason to swap — ^3r5bhpj shows the swap costs 27 of 29 genuinely dead symbols — but it does mean the fix this card asks for is work the shipped tool needs and the successor would not.

    The fix this card names is still the right one: prefix only a RELATIVE path and leave an absolute one alone, the same reading `reportedAs` in the rule's own node script already makes.
  timestamp: 2026-08-15T13:53:22.056059+00:00
- actor: claude-code
  id: 01m0305tm1wr40gejvv2357p16
  text: |-
    Implemented. What was measured, and what the numbers actually are.

    MEASURED BEFORE, over `zod` at `4e1720c` with `node_modules` installed, driving the shipped script byte for byte: 76 findings, 11.5 s, and 1 of the 76 named a path that stands nowhere — `packages/bench/<the absolute path of the checkout>/packages/zod/src/v4/core/standard-schema.ts:157` `StandardSchemaWithJSON`. That matches the earlier comment, not the 284 the description predicts.

    WHERE THE 284 IS. It is the no-entry-carve-out state. Re-measured with the entry pattern forced empty: 1944 rows, 285 of them carrying the absolute spelling — 284 at `packages/zod/src/index.ts` and 1 at `standard-schema.ts`. Every one of the 284 stands in a module `zod` publishes, so the carve-out already silences them. The shipped run therefore leaks exactly ONE, not 284. Reported as measured.

    MEASURED AFTER: 76 findings, 11.1 s, 0 naming a file that stands nowhere. `zustand` 1 and `redux` 6 do not move. The count is unchanged and one more finding of the 76 now reaches the author.

    THE ENGINE DROP IS CONFIRMED, not guessed. `normalize_tool_path` strips the repository root from an absolute path and a leading `./`, and `run_tool_script` keeps a workspace-scope finding only when the normalized path meets a file of the run. Driving the new probe through `plan_tool_rules` + `execute_tool_runs` with the OLD script, the engine reported `[]` where it must report `packages/shared/src/lib.ts`. That is the silent miss, watched RED.

    THE FIX mirrors `reportedAs` rather than inventing a second spelling. The `sed` no longer carries `${prefix}`; a `while read` after it tests `[ -f "$dir/<path>" ]`, prefixes a RELATIVE path and gives an ABSOLUTE one back the separator ts-prune's presenter cut. That is the exact inverse of `reportedAs`, and it is a filesystem test rather than a lexical one on purpose: a lexical test against the workspace root would still corrupt a path OUTSIDE the workspace, which is the same defect class.

    TWO THINGS THAT DID NOT WORK, so the next agent does not repeat them:
    1. The engine-level probe first staged BOTH modules as work-list positions. `assert_shipped_run_reports` compares order-sensitively, and after the fix the two findings reach the engine in the order `sort -u` puts `/private/var/…` and `packages/consumer/…` in. That order is LOCALE-dependent — the C locale puts the absolute row first, a UTF-8 collation that ignores punctuation puts it second — so the probe now names ONE position and stands the inside module beside it as support.
    2. The path could not be decided inside `sed`. sed cannot stat, and no lexical rule tells an absolute-minus-separator path from a relative one in general.

    The rule body states the before/after table in its existing style, and the knip decision record is untouched apart from two sentences that would otherwise have gone false: the `StandardSchemaWithJSON` line now reads past tense and names why the hand-check counted it wrong, and the "what the swap would have fixed" line now points at the fix.
  timestamp: 2026-08-15T15:20:49.537274+00:00
- actor: claude-code
  id: 01m03061k8peh2tmh1hv4xx189
  text: |-
    ### implement — changed
    - evidence: 2 files — builtin/validators/code-hygiene/rules/dead-code-typescript.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs. `cargo nextest run -p swissarmyhammer-validators`: 746 run, 746 passed, 0 skipped. `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean. zod at 4e1720c, installed: 76 findings before and after; paths naming no file 1 → 0.
    - next: /review
  timestamp: 2026-08-15T15:20:56.680645+00:00
- actor: claude-code
  id: 01m031kaer0nyjagyy10gz0y5h
  text: |
    ### review — findings
    - scope: `review sha 02fca87ea~1..02fca87ea`. The report scope line reads "reviewed the diffs only", 1 file reviewed, 0 not reviewed. The engine read only the `.rs` acceptance test and reported no finding. No validator declares a `*.md` glob, thus the rule body was not read by the engine.
    - evidence: 4 findings by hand — builtin/validators/code-hygiene/rules/dead-code-typescript.md:217, :222, :722, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs:345
    - mirror check: the new shell code is NOT a lexical inverse of `reportedAs`, and it does not try to be. It uses a filesystem test, `[ -f "$dir/${finding%%:*}" ]`, which the commit states is on purpose. The true authority both sides must copy is the ts-prune presenter, `result.file.replace(process.cwd(), "").replace(/^\//, "")`. That replace is not anchored, so a sibling directory whose name extends the project name breaks both sides. The new code is never worse than the old code for these shapes, because the old code wrote the same corrupt string.
    - numbers check: coherent. 284 exports of `packages/zod/src/index.ts` plus 1 export of `packages/zod/src/v4/core/standard-schema.ts` give the 285 rows that carry the absolute spelling, out of 1944 rows in the run with the entry pattern forced empty. 1944 to 285 is a stated subset, not a gap. `index.ts` is a published entry module, so the entry carve-out takes all 284 off; `standard-schema.ts` is not, so 1 leaks. "76 before and after" is correct, because the fix respells the finding, it does not remove it. The test doc comment states this exactly: "1 of the 76 findings named ... a file that stands nowhere."
    - dropped: 4 candidate findings dropped for a false premise — the premise that the new `sed` makes a lexical leading-separator test, and the three premises that the counts contradict each other. 2 more dropped because they land on lines this commit does not touch: the anchored `startsWith` in `reportedAs`, and the `path.sep` behaviour on Windows.
    - next: correct the comment, the classifier and the prose in the rule body, and extend the probe to a package name that is a prefix of a sibling.
  timestamp: 2026-08-15T15:45:40.312277+00:00
- actor: claude-code
  id: 01m032rk3jzfwmvwzzp4r05j12
  text: |-
    All four findings worked. What the run does now, and what it measures.

    FINDING 4 FIRST, WATCHED RED. A new probe stages `packages/consumer` beside TWO packages whose directory names begin with it: `packages/consumer-bench` and `packages/consumersrc`. Driven through the shipped bytes, the OLD script answered `["/-bench/src/lib.ts:2", "packages/consumer/src/lib.ts:2", "packages/consumer/src/other.ts:4"]` where it must answer `["packages/consumer-bench/src/lib.ts:2", "packages/consumer/src/other.ts:4"]`. Both halves of finding 2 stand in the shipped bytes, not only in the presenter source: one path that stands nowhere, and one that names `packages/consumer/src/lib.ts` — a REAL file whose row 2 holds a LIVE export the run must never report.

    THE PRESENTER, MEASURED RATHER THAN READ. `ts-prune -p tsconfig.json` in `packages/consumer` over that tree writes `src/lib.ts:2`, `src/other.ts:4` and `-bench/src/lib.ts:2`. The third row is the `-bench` sibling; the first is the `consumersrc` sibling wearing the spelling of a real file of the project.

    FINDING 2, HOW IT IS CLOSED. The loop no longer completes one path and tests it. It takes the working directory ts-prune used — `cwd="$(cd "$dir" && pwd -P)"`, physical because `analyzer.js` reports `fs.realpathSync(result.file)` and `process.cwd()` is physical too — and rebuilds all THREE spellings the cut can have made: `$cwd/$cut`, `$cwd$cut` and `/$cut`. Exactly one standing as a file is the answer. Zero, or two, is a path the run cannot confirm, so it writes the line on stderr and reports nothing for it.

    THE RESIDUE, STATED HONESTLY RATHER THAN PAPERED OVER. Two files of one program can wear the SAME spelling: `packages/consumersrc/lib.ts` and `packages/consumer/src/lib.ts` both come out as `src/lib.ts`. The cut destroyed what told them apart and nothing on the line brings it back, so both are refused. The rule body says so, with the measurement: 0 refused of the 141 findings the four workspaces answer with dependencies installed. A dropped finding, never a wrong one.

    TWO ALTERNATIVES REJECTED, so the next agent does not re-walk them:
    1. Run ts-prune with cwd=`/` (or at the workspace root). That makes every path absolute-minus-separator and kills the collision structurally — but `runner.js` builds tsconfig `files` entrypoints with `path.join(process.cwd(), file)`, so every project that lists `files` would lose its DEFINITELY_USED carve-out and gain false findings. Rejected on that regression.
    2. Break a tie by reading the symbol name at the reported row. `export default` reports the name `default`, which never stands on the line, so the tie-break would reject the TRUE file for that shape. A wrong drop for a common export kind. Rejected.

    FINDING 1 and FINDING 3. The loop comment now states the presenter's actual operation, unanchored cut included. `reportedAs` no longer models the presenter with `startsWith(projectDir + path.sep)` + `path.relative` — it COPIES it: `absolute.replace(projectDir, "").replace(/^sep/, "")`. The prose now names the presenter as the one authority both sides copy, and states that `--ignore` is applied inside ts-prune by `runner.js` before stdout, so the carve-out and the pipe never meet.

    THE SWEEP. Beyond the four findings, 7 more places carried the same cause and were corrected: the `reportedAs` doc comment; the `reportedAs` implementation itself; the "The path each finding is rebuilt at" opening paragraph; the knip-section line "reading ts-prune's two spellings apart"; the closing line claiming ONE acceptance test holds the path arithmetic; and in the test file the `TYPESCRIPT_OUTSIDE_MODULE_PROBE` reason and the outside-module test doc. One stale count was corrected beside them (seven acceptance tests → eight), and one stale reason: the positions probe named one position because the two spellings sorted locale-dependently, which no longer holds now that both are workspace-relative.

    MEASURED, corpus at the same commits, dependencies installed, shipped bytes extracted from the rule front matter before and after: zod 76 → 76, zustand 1 → 1, redux 6 → 6, this workspace 58 → 58. 0 findings name a file that stands nowhere, 0 refused, stderr empty on all four. The ONE line that changed over the whole corpus is zod's `packages/zod/src/v4/core/standard-schema.ts:157` `StandardSchemaWithJSON`, which the old script spelled with the absolute path of the checkout and the new script spells `packages/zod/src/...`. That kills the machine-specific path the first comment on this card recorded as a defect.
  timestamp: 2026-08-15T16:06:01.586105+00:00
- actor: claude-code
  id: 01m032rvbwnwf02fw4r76trgcs
  text: |-
    ### implement — changed
    - evidence: 2 files — builtin/validators/code-hygiene/rules/dead-code-typescript.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs. New acceptance test `the_shipped_typescript_dead_code_tool_rule_names_no_file_that_is_not_the_file_of_the_finding`, watched RED on the shipped bytes then GREEN. `cargo nextest run -p swissarmyhammer-validators`: 747 run, 747 passed, 0 skipped. `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean. Corpus with dependencies installed, before → after: zod 76 → 76, zustand 1 → 1, redux 6 → 6, this workspace 58 → 58; 0 paths naming no file, 0 refused.
    - next: /review
  timestamp: 2026-08-15T16:06:10.044110+00:00
position_column: doing
position_ordinal: '8280'
title: dead-code-typescript prefixes a project path onto an absolute path, so the finding names a file that is nowhere
---
`builtin/validators/code-hygiene/rules/dead-code-typescript.md` runs `ts-prune` inside each project directory and puts the project's path back on the front of every finding.

`ts-prune`'s presenter writes `result.file.replace(process.cwd(), "").replace(/^\//, "")`. For a file INSIDE the project that gives a relative path, which the prefix completes. For a file the program reaches from OUTSIDE the project directory, the working directory is no prefix of the path, so the presenter writes the whole absolute path minus its leading separator — and the rule then writes the project path in front of it.

Measured over `colinhacks/zod` at `4e1720c`: `packages/bench/tsconfig.json` builds a program that reaches `packages/zod/src/index.ts`, and the run wrote 284 findings whose path was `packages/bench/private/tmp/.../corpus/zod/packages/zod/src/index.ts`. No such file exists, so the engine drops every one of them. That is a silent miss, not a wrong finding.

The entry carve-out already reads this shape — `reportedAs` in the rule's node script writes the same spelling ts-prune writes — so the fix is to make the `sed` that rebuilds each finding path do the same: prefix only a RELATIVE path, and leave an absolute one alone. Measure the count before and after over `zod`, and ship an acceptance test.

Found while implementing ^108bh4y. #tool-validators

## Review Findings (2026-08-15 10:23)

> Scope: `review sha 02fca87ea~1..02fca87ea` — reviewed the diffs only — lines this change added or modified. 1 file(s) reviewed, 0 not reviewed.

The engine read one file, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs`, and reported no finding. No validator declares a `*.md` glob, thus the engine did not read the rule body. The items below come from a check by hand of the rule body and the test, against the installed `ts-prune` 0.10.3 source. Each item is on a line that this commit adds.

- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:217` `hand-check/path-arithmetic` — The new comment says that ts-prune writes a path relative to its working directory for a file in the project, and the absolute path less its leading separator for a file outside it. The presenter does `result.file.replace(process.cwd(), "").replace(/^\//, "")`. `String.replace` with a string argument is not anchored, and it does not need a separator after the match. For the project `packages/zod` and the file `packages/zod-bench/src/x.ts`, the presenter writes `-bench/src/x.ts`. That is neither spelling the comment names. Write the operation the presenter does, and classify on that operation.
- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:222` `hand-check/path-arithmetic` — The test `[ -f "$dir/${finding%%:*}" ]` gives the wrong class for a path the presenter cut at the wrong position. For the project `packages/a` and the outside file `packages/ab/src/lib.ts`, the presenter writes `b/src/lib.ts`. If `packages/a/b/src/lib.ts` is present, the test is true, and the run puts the finding on a real file that is not the file of the finding. If it is absent, the run writes `/b/src/lib.ts`, which stands nowhere. Build the path from the working directory ts-prune used, or refuse a path the run cannot confirm.
- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:722` `hand-check/accuracy` — The new prose says that `reportedAs` and the pipe "have to agree or the `--ignore` pattern would name a spelling the pipe never writes". `runner.js` applies `--ignore` to the presented line inside ts-prune, before stdout: `presented.filter(function (file) { return !file.match(config.ignore); })`. The pipe never sees that decision, and the pipe rewrites every line it passes. `reportedAs` must agree with the presenter, not with the pipe. Name the presenter as the one authority that both sides copy.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs:345` `hand-check/coverage` — The probe stages `packages/consumer` and `packages/shared`. Neither name is a prefix of the other, thus the probe does not hold the shape the two items above name. Stage a second package whose name starts with the name of the project package, for example `packages/consumer-bench`, and hold the path the run reports.
