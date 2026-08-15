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
- actor: claude-code
  id: 01m032wgvqvx6w0ye1f3g4ezdt
  text: |-
    ### commit — changed
    - evidence: a4a4160fe fix(validators): rebuild dead-code-typescript paths from ts-prune's cwd, not an anchored guess (^yxky1aj)
    - next: none
  timestamp: 2026-08-15T16:08:10.359001+00:00
- actor: claude-code
  id: 01m0347zsa0bhgemvyrb3nw40a
  text: |
    ### review — findings

    - evidence: engine `review sha a4a4160fe~1..a4a4160fe` reported 0 findings over 1 file (`crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs`, 9 runs attempted, 0 failed, 0 skipped). 2 findings by hand on the rule body the engine cannot read — `builtin/validators/code-hygiene/rules/dead-code-typescript.md:220`, `builtin/validators/code-hygiene/rules/dead-code-typescript.md:239`. Both land on lines this commit adds.
    - checked by hand: the classifier enumeration against ts-prune 0.10.3 `lib/presenter.js`; the residue claim and its 141 (76 zod + 1 zustand + 6 redux + 58 this workspace, the table at rule body line 769); whether the fix can drop a finding that reported correctly before.
    - round 2 of this card. Round 1's fix made round 2's defect. Item 220 and item 239 are both new causes this commit makes: the fourth spelling and the refusal path did not exist before it.
    - next: another round. Close item 239 first — a silent refusal is the failure mode this work removes.
  timestamp: 2026-08-15T16:31:54.666442+00:00
- actor: claude-code
  id: 01m034yadwz44ca0p7hz458cxw
  text: |-
    Round 3. Both findings worked, and the scope stayed on the two files the findings name plus the test helper each needed.

    FINDING 239 FIRST, because a silent drop is the failure this work removes.

    THE CHANNEL. The drop now travels STDOUT, the same channel a finding travels, at `packages/<project>/tsconfig.json:1`. Measured, driving the shipped bytes over the sibling-prefix probe:

        packages/consumer/tsconfig.json:1: dead-code-typescript dropped one finding. 2 files of this program stand at the spelling `src/lib.ts`. The run cannot tell which file the finding is about. The dropped line is `src/lib.ts:2: unused export 'trulyDead'; nothing in the project imports it`

    WHY THE TSCONFIG, and not a source file. The tsconfig always stands as a file, so the line names a path the run can confirm for BOTH drop shapes — the 2-candidate collision and the 0-candidate fourth spelling, which has no candidate to name at all. It is also the file that made the program: the `include` list is what put the colliding sibling in the graph, so it is where the author acts. Reporting at the standing candidates was rejected: for the collision those are real source files whose row does not hold the export, and `..._names_no_file_that_is_not_the_file_of_the_finding` is the test that forbids exactly that.

    `run_script_findings` and `tool_rules.rs` were NOT touched, as the card states. The engine's workspace-scope path filter still applies, so the drop reaches the report when the project's own tsconfig is one of the changed files. The rule body says that in those words rather than claiming more. The carrier on `ToolOutcome` stays with ^m6ba1bf.

    WATCHED RED, both halves. The sibling-prefix probe's `expected` gained `packages/consumer/tsconfig.json:1`, and a new test `the_shipped_typescript_dead_code_tool_rule_says_the_finding_it_drops_out_loud` reads the CLAIM rather than the row. Before the script change: rows `["packages/consumer-bench/src/lib.ts:2", "packages/consumer/src/other.ts:4"]` against the three the probe names, and 0 claims starting with the drop sentence against 1. Both then GREEN. The row alone would pass a run that wrote some other sentence there, which is why the claim is read.

    THE TEST HELPER. `shipped_script_findings` collapsed each finding to `path:line` and threw the claim away, and it has 11 callers, so its signature was left alone. The staging and run body moved into `drive_shipped_script(loader, rule, staged, files, read)`; `shipped_script_findings` passes `finding_rows` and the new `drive_shipped_staged_tree_claims` passes `finding_claims`. No duplicated staging body.

    FINDING 220. Three exhaustive claims were false and all three now state what the code does. The loop comment no longer says "Three spellings therefore reach this loop" — it says the working directory rebuilds THREE of the spellings the cut makes and names the fourth, the one a nested copy makes, as the shape no rebuild reaches. The table gained a fourth row: the old "anywhere else" row is now "outside the project, and its path holds the project path nowhere", beside "…at a LATER position", with the worked example `/mnt/backup/w/packages/a/src/x.ts` under `/w/packages/a` coming out as `mnt/backup/src/x.ts`. The residue paragraph opens "Two shapes the rebuild cannot close" and states both.

    THE FOURTH SPELLING IS NOT RECONSTRUCTED, as the card orders. It is dropped, and now the drop is announced.

    THE SWEEP, beyond the two lines the findings name. Four more places carried the same two causes: the knip-section line "the one shape that rebuild leaves"; the table column header `refused`, now `dropped`, to match the word the script and the prose use; "The eight acceptance tests" (nine now); and "The two acceptance tests named in 'How the run is shaped'" (three now). The knip decision record and the corpus tables are otherwise untouched.

    THE CORPUS NUMBERS WERE NOT RE-MEASURED, and they do not need to be. The corpus checkouts are not on this machine. Nothing changes for a finding the run CAN place — the `found -eq 1` path is byte for byte what it was — and the table already records 0 dropped over all four workspaces, so no drop line is emitted for any of them and every count stands. Reported as reasoning, not as a fresh measurement.
  timestamp: 2026-08-15T16:44:06.460301+00:00
- actor: claude-code
  id: 01m034yhr82h79m3k15w51qnsv
  text: |-
    ### implement — changed
    - evidence: 3 files — builtin/validators/code-hygiene/rules/dead-code-typescript.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs. New acceptance test `the_shipped_typescript_dead_code_tool_rule_says_the_finding_it_drops_out_loud`, watched RED on the shipped bytes then GREEN, beside the drop row added to the sibling-prefix probe. `cargo nextest run -p swissarmyhammer-validators`: 748 run, 748 passed, 0 skipped. `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean. Both review findings flipped to `- [x]`.
    - next: /review
  timestamp: 2026-08-15T16:44:13.960661+00:00
- actor: claude-code
  id: 01m035gcb6bhn2xycmm9an77bh
  text: |
    ### review — findings

    - evidence: 4 findings — builtin/validators/code-hygiene/rules/dead-code-typescript.md:227, :246, :824, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs:693. Engine `review sha ebed84085~1..ebed84085` read 2 files over 9 validator runs and reported 0 findings; all 4 items come from a check by hand of the rule body, which no validator glob reaches.
    - convergence: NOT converging. Round 1 fix made the round 2 defect, round 2 fix made the two round 3 defects, and round 3 fix makes these. Every round moves the same lines and makes a new cause on them.
    - stop condition: the round 3 review set it — a further round of new causes on these lines means hand-off, not a fourth iteration. That condition is met. A person must take this rule body, not another agent round.
    - next: hand off. The two causes to settle by hand are (1) the drop row is a path this rule's own `match.files` globs never select, so the workspace retain discards it always, and (2) `found` counts candidates that exist, not candidates that are correct.
  timestamp: 2026-08-15T16:53:58.246437+00:00
- actor: claude-code
  id: 01m035nvb7e9mrbayxkfr9yx4p
  text: |-
    ### finish iteration 4 — stuck

    - implement: changed — 3 files (round 3)
    - test: green — cargo nextest, 748 passed
    - commit: ebed84085
    - review: findings — builtin/validators/code-hygiene/rules/dead-code-typescript.md:227, :246, :824; crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs:693

    **Guardrail hit. Task is STUCK — it stays in `review`. Do not iterate a fifth time.**

    Four rounds, and each fix put a new cause on the same lines. The reviewer set the stop condition in round 2 and it is met.

    Round 4 named two root causes for the first time. Both are structural, not wording:

    1. **The drop row is a path this rule's own globs never select.** `run_tool_script` keeps a `scope: workspace` finding only when its path stands in `run.files` (`crates/swissarmyhammer-validators/src/review/tool_rules.rs:95-100`). `run.files` is the changed-file list filtered through the rule's globs — `**/*.ts`, `**/*.tsx`, `**/*.js`, `**/*.jsx`, `**/*.mjs`, `**/*.cjs`. A `tsconfig.json` matches none of them. So the drop cannot reach the report on ANY run, not merely on runs where that file is unchanged. The rule body's concession reads as a satisfiable condition; it is unsatisfiable. Moving the channel from stderr to stdout changed nothing.

    2. **`found` measures existence, not correctness.** A candidate that exists but is the wrong file gives `found` = 1, so the `-ne 1` branch never runs and the tool reports at a file the finding is not about. That is a wrong finding, not a drop.

    **Blocked on [[^m6ba1bf]].** Root cause 1 is this rule working around the engine defect that `^m6ba1bf` fixes: a tool rule has no supported channel for "the tool could not answer". Once the engine carries that signal, the whole drop-reporting apparatus in this rule is very likely deleted rather than repaired. Fix `^m6ba1bf` first, then re-scope this card.

    The corpus counts are NOT unverified — the reviewer confirmed the `found -eq 1` path is byte-for-byte unchanged, so the numbers stand. One finding was dropped for a false premise.
  timestamp: 2026-08-15T16:56:57.447914+00:00
- actor: claude-code
  id: 01m0374rr6nrv0cep5v9m4ndhb
  text: |-
    ### Unblocked — ^m6ba1bf landed as 9fcdd8387. RE-SCOPE this card.

    The engine now carries a diagnostic that no glob filter can drop. A tool-rule script states a declined item by writing a stderr line opening `sah-diagnostic:` while exiting 0; `ToolOutcome`/`ToolReport` carry `diagnostics: Vec<ToolDiagnostic>`, the workspace retain reaches `outcome.findings` alone, and synthesis renders a block per entry naming the validator, the rule and the message. `ToolReport::is_inert` reads the vector, so a run whose only fact is a diagnostic no longer prints "Nothing in scope to review." The contract is in `builtin/validators/README.md`.

    **Do NOT repair the four open findings as written.** Three of them describe the stdout drop-reporting apparatus, which is now the wrong shape. The new scope is:

    1. **DELETE the stdout drop-reporting apparatus** from `builtin/validators/code-hygiene/rules/dead-code-typescript.md`, and with it the three prose claims the round-4 review flagged at :227, :246 and :824. Replace it with one `sah-diagnostic:` line. This removes the unsatisfiable concession — the diagnostic no longer needs a path its globs select, because it is about the RUN.

    2. **Fix the surviving root cause: `found` measures existence, not correctness.** A candidate that exists but is the wrong file gives `found` = 1, so the `-ne 1` branch never runs and the rule reports at a file the finding is not about — a WRONG finding, not a drop. This one is real and independent of the carrier. The fix is not another rebuild spelling; four rounds show widening the rebuild is what produces the next round's defect. Decline the item and state it, unless the candidate can be confirmed as the right file rather than merely an existing one.

    3. Drop the now-dead assertion at `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs:693`, and hold the new behaviour instead.

    The corpus counts stand — the `found -eq 1` path was byte-for-byte unchanged and the reviewer confirmed it. Do not re-measure; the clones are gone from this machine.
  timestamp: 2026-08-15T17:22:34.886352+00:00
- actor: claude-code
  id: 01m0388az87ps244bg8gsma5bs
  text: |-
    Round 5, on the re-scope. The rebuild is GONE. Nothing widens it, because nothing inverts the cut any more.

    THE DESIGN. The presenter's cut destroys text, so no arithmetic reads the path back — every round of this card tried and made the next round's defect. The run now goes the other way. `tsc -p tsconfig.json --listFilesOnly` prints the absolute path of every file the project's own program holds, and the run spells each of those files with `reportedAs`, the presenter's own operation the script already carried. The file whose spelling meets the reported one IS the file of the finding, because ts-prune reports a file of the program. Exactly one such file places the finding. Zero, or two, is an item the run declines.

    WHY THIS ANSWERS FINDING :227 ROOT AND NOT AROUND IT. `found` counted candidates that EXIST. The new test counts files of the PROGRAM that carry the spelling. Existence of an unrelated file can no longer place a finding: `/w/a/src/x.ts` standing on disk means nothing unless the program of `/w/ab` holds it, and if the program holds both it and the nested original, both carry the same spelling and the item declines. The reviewer's wrong-finding shape cannot reach a report from this code.

    THE CARRIER. The stdout apparatus at the tsconfig is deleted whole. A declined item is one stderr line opening `sah-diagnostic:` at exit 0, per `builtin/validators/README.md` and ^m6ba1bf. The engine's workspace retain reaches `outcome.findings` alone, so no glob can drop it. The message does NOT repeat the rule name — synthesis already names the validator and the rule.

    ONE MORE SILENT CHANNEL CLOSED beside the card's list: the entries job used to write its fail-open failure as a bare stderr line, which the engine drops as tool chatter. Both jobs now use the marker, so `tsc --showConfig` writing nothing, and a manifest that does not parse, reach the report instead of nobody.

    WATCHED RED, then GREEN. `..._says_the_finding_it_declines_out_loud` reads the DIAGNOSTIC rather than a stdout row; on the shipped bytes before the change it read `[]` against 1. `..._names_no_file_that_is_not_the_file_of_the_finding` lost the tsconfig row from `expected` and failed on the old script. Both green after.

    THE HARNESS. `drive_shipped_script` now hands its reader the whole `ScriptOutcome` rather than the findings alone, so a probe can read diagnostics. `finding_claims` and `drive_shipped_staged_tree_claims` had one caller — the deleted drop test — so they are deleted rather than left dead; `script_diagnostics` and `drive_shipped_staged_tree_diagnostics` take their place. The two direct `finding_rows` callers (shipped.rs, dead_code_swift.rs) pass the outcome.

    MEASURED, this workspace, shipped bytes extracted from the front matter before and after: 58 findings and 58 findings, byte-identical stdout, 0 items declined, 0 bytes on stderr, 6.2 s against 7.3 s — the second `tsc` call costs less than the per-finding shell loop it replaces. Checked separately over both projects: all 58 reported spellings meet exactly one file of the program, 0 meet none, 0 meet two.

    THE CORPUS ROWS ARE NOT RE-MEASURED, and the rule body now says so in those words. The card ruled that out and the clones are gone. The earlier argument — the `found -eq 1` path is unchanged — does NOT carry over, because that path no longer exists, so the table marks zod, zustand and redux as measured under the earlier placement and marks this workspace as the row measured under the shipped one. A count that moves now moves for one reason and the run states each such item, so a re-measurement reads the difference off the report.

    WHAT A HUMAN MUST CHECK BY HAND. No validator declares a `*.md` glob, so the engine reads none of the rule body. By hand: (1) the node program in the front matter, above all `placeFindings` and `reportedPath`; (2) the rewritten "The path each finding is reported at" section, which drops the four-shape table and makes no exhaustive claim about path shapes — it names one shape, `packages/consumersrc`, as an example; (3) the corpus table caption stating which rows were measured under which placement.
  timestamp: 2026-08-15T17:42:00.424639+00:00
- actor: claude-code
  id: 01m0394z7x6tp1wbzh29e8v623
  text: |-
    ### implement — changed
    - evidence: 4 files — builtin/validators/code-hygiene/rules/dead-code-typescript.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_swift.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs. New acceptance test `the_shipped_typescript_dead_code_tool_rule_says_the_finding_it_declines_out_loud` reads the DIAGNOSTIC; watched RED (`[]` against 1) on the shipped bytes, then GREEN. `..._names_no_file_that_is_not_the_file_of_the_finding` lost the tsconfig drop row from `expected`, RED then GREEN. All four review findings flipped to `- [x]` under the re-scope.
    - next: /test

    ### test — green
    - evidence: `cargo nextest run --workspace`: 14233 run, 14233 passed, 0 skipped. `cargo nextest run -p swissarmyhammer-validators`: 756 run, 756 passed. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /commit

    ### commit — changed
    - evidence: ba36dc365 fix(validators): place dead-code-typescript findings from the program file list (^yxky1aj); d7c34df97 docs(validators): correct the dead-code-typescript timing claim and three unconditional ones (^yxky1aj)
    - next: /review

    ### review — clean
    - evidence: engine `review sha ba36dc365~1..d7c34df97` reported 0 findings over 3 files, 9 validator runs attempted, 0 failed, 0 skipped. No validator declares a `*.md` glob, so the engine read none of the rule body.
    - checked by hand, and the two it caught are FIXED in d7c34df97 rather than carried: (1) `6.2 s against 7.3 s` compared a COLD run of the old script with a warm run of the new one and read as the new placement being faster; re-measured, lowest of three runs each, 5.6 s before and 6.3 s after, and the body now states the 0.7 s the second `tsc` call costs. (2) Three claims were wider than the code — the placement premise (ts-prune builds its program from the same `tsconfig.json` `tsc` read), the ways a count can fall (`sort -u` collapsing a pair the earlier spelling held apart, beside a declined item), and "three things reach that line" (three reasons the PLACEMENT states an item; the entry job writes on the same channel).
    - checked by hand and confirmed: the example diagnostic in the rule body is byte-for-byte the line the shipped script writes over the sibling-prefix tree; the `sah-diagnostic:` contract against `builtin/validators/README.md`; the workspace retain in `tool_rules.rs` reaching `outcome.findings` alone; the nine acceptance tests still nine; `doctor.check_command` still names every command the script runs.
    - NOT self-certified: the rule body is 1000 lines the engine cannot read, and every round of this card found something there by hand. The card stays in `review` for that reading rather than going to done.
    - next: a human reads the rule body — the node program in the front matter, "The path each finding is reported at", and the corpus table caption.
  timestamp: 2026-08-15T17:57:38.685865+00:00
- actor: claude-code
  id: 01m039x5m3xk1b3nkmk224vdch
  text: |
    ### review — findings
    - evidence: engine `review sha 9fcdd8387..HEAD` clean (0 findings, 9 validator runs, 3 files). Hand-check of the rule body, which no validator glob reads, raised 3: `builtin/validators/code-hygiene/rules/dead-code-typescript.md:836`, `:895`, `:955`. All three land on lines this range adds. 6 findings dropped for a false premise.
    - convergence: converging. The round 1-4 root causes are structural and are REMOVED, not restated — the path-rebuild apparatus is deleted, and the diagnostic channel is verified end to end from `marked_diagnostics` through `render_tool_diagnostics`. All 3 new findings are on genuinely new code and prose. None of the 4 prior findings recurs.
    - next: correct the three claims. `:836` and `:955` each name a code change beside the prose: say what `standing.length` measures, and decline a manifest the run could not read.
  timestamp: 2026-08-15T18:10:51.651948+00:00
- actor: claude-code
  id: 01m03bj8br6h79tyy9sbtrb76n
  text: |-
    Round 6, on the three open findings. All three worked, each with a code change beside the prose.

    FINDING :836 — THE DEFECT, AND WHAT THE TOOLS ACTUALLY DO. The reviewer's mechanism is real and I reproduced it end to end, but NOT by the route the finding names, and that difference matters for the next agent.

    Measured with ts-prune 0.10.3 + tsc 5.9.3:
    1. An `include` GLOB over a symbolic link: `tsc --listFilesOnly` lists the link, and ts-morph's program does NOT hold it — its directory walk answers `isFile()` false for a link. So ts-prune reports nothing for it and there is no divergence to observe. Both a file link (`src/link.ts`) and a directory link (`src/vendor/`) behave this way.
    2. A pnpm-style store layout: `tsc` and ts-prune BOTH report the store path. TypeScript realpaths a resolved module, so the two agree. No divergence.
    3. A `files` ENTRY that is a symbolic link: ts-morph adds it by path, and `analyzer.js` reports `fs.realpathSync` of it. `tsc` lists `src/link.ts`; ts-prune reports `outside/util.ts:2 - trulyDead`. THAT is the divergence, and it is what this probe stages.

    So the finding's claim is right — `standing.length` counted what EXISTS in the list, not what IS the reported file — and the shape the reviewer guessed (symlink, pnpm) does not produce it, while a `files` entry does.

    THE FIX, on both sides as the finding orders. `realFiles` groups the listed files by `fs.realpathSync`, and registers each real file under BOTH spellings: `reportedAs` of the path `tsc` printed, and `reportedAs` of the real path. `filesBySpelling` then keys on those, and `standing` counts REAL files rather than list entries. Three consequences: the reported file stands among its own candidates; two entries that resolve to one real file are ONE candidate, so a link listed beside its own target is not a collision (that would have been a regression); and a second, different real file carrying the spelling declines as before. The placement now reports the REAL path, which is where the export text stands.

    WATCHED RED at the shipped-script level on identical trees, then in Rust. On the script before this change the `files`-link tree answered `sah-diagnostic: 0 files of the program ... carry the spelling \`outside/util.ts\`` and reported nothing; after it, `outside/util.ts:2`. The Rust test `..._places_a_file_the_two_readings_spell_differently` was then run against the OLD rule body byte for byte (rule file swapped out and back, md5 checked) and FAILED, then passed.

    THE RESIDUE IS STATED, NOT HIDDEN. One candidate names the reported file when `tsc` listed that file. A file ts-prune reported that `tsc` listed under NEITHER spelling is outside what the count can see. The prose says exactly that rather than claiming a guarantee the code does not implement.

    FINDING :895 — RE-MEASURED THREE WAYS. Lowest of three runs each, this workspace, warm: the shell placement 5.5 s, the file-list placement 6.2 s, the file-list placement with the real-path reading 6.2 s. 58 findings on all three, byte-identical stdout, 0 declined, 0 bytes stderr. The body now says the 0.7 s is what the WHOLE placement costs — two `tsc --listFilesOnly` runs, two `node ... place` starts, minus the shell loop — and says plainly that the measurement does not divide it between the three. The real-path reading moved the number by 0.03 s, under the spread of the placement's own three runs, so the body says that too rather than claiming it is free.

    FINDING :955 — THE CODE, NOT THE SENTENCE. `manifests()` now `decline`s the manifest it could not parse, naming its workspace-relative path. Verified: on the probe tree the run reports `packages/app/src/lib.ts:2` and `packages/other/src/index.ts:2` — the broken package's ENTRY module reported as dead, which is the whole cost — and writes `sah-diagnostic: the manifest packages/other/package.json does not parse, so the entry modules it publishes stay under the gate: SyntaxError: ...`. Before the change: same two rows, zero diagnostics. The prose now separates the two halves honestly: the `tsc` half takes the whole job (through `main`'s catch, which writes `the entries job broke in <dir>: <failure>` — measured), the manifest half is narrow.

    ONE THING THAT DOES NOT WORK, so the next agent does not try it: the broken manifest cannot stand AT or ABOVE the project directory. ts-prune reads its own configuration through cosmiconfig from the cwd upward and DIES on a `package.json` it cannot parse. The probe puts it in a sibling package.

    THE SWEEP, beyond the three lines. Five more places carried the same causes: the ":883" decline bullet blamed symbolic links for the zero case, which the measurement above shows is not the mechanism; "the list below states each way they can" was a false exhaustive claim; "The placement states an item on that channel for three reasons" missed the fourth (`main`'s catch), so it now names four; "A row goes missing two ways" now states the ways without a count and names the new one the real-path spelling can make; and `decline`'s own doc comment said "one item this run judged", which the manifest line is not.

    HARNESS. `drive_shipped_script` gained a `links` argument, `stage_probe_links` makes them (unix; the non-unix arm asserts the list is empty), `drive_shipped_staged_tree_diagnostics` became `drive_shipped_staged_tree_read(probe, links, read)` — one function for rows and for diagnostics — and `assert_shipped_tree_rows` is extracted so the two row checks share one assertion. Eleven acceptance tests now, from nine.

    WHAT A HUMAN MUST CHECK BY HAND. No validator declares a `*.md` glob, so the engine reads none of the rule body. By hand: (1) `realPath`/`realFiles`/`filesBySpelling` and the `manifests` catch in the front matter; (2) "The path each finding is reported at", above all the three paragraphs stating what the count measures and what it rests on; (3) the timing paragraph and the corpus caption; (4) "Entry resolution fails OPEN".
  timestamp: 2026-08-15T18:39:51.160630+00:00
- actor: claude-code
  id: 01m03bst27ptgrvhndkds607r3
  text: |-
    ### implement — changed
    - evidence: 3 files — builtin/validators/code-hygiene/rules/dead-code-typescript.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs. Two new acceptance tests, each watched RED against the OLD rule body byte for byte and then GREEN: `..._places_a_file_the_two_readings_spell_differently` (was `[]` against `outside/util.ts:2`) and `..._says_the_manifest_it_could_not_read_out_loud` (was 0 diagnostics against 1). All three review findings flipped to `- [x]`.
    - next: /test

    ### test — green
    - evidence: `cargo nextest run --workspace`: 14235 run, 14235 passed, 0 skipped. `cargo nextest run -p swissarmyhammer-validators`: 758 run, 758 passed. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /commit

    ### commit — changed
    - evidence: bc820561f fix(validators): match dead-code-typescript spellings by real path, and state the manifest it cannot read (^yxky1aj)
    - next: /review

    ### review — clean
    - evidence: engine `review sha bc820561f~1..bc820561f` reported 0 findings over 2 files, 9 validator runs attempted, 0 failed, 0 skipped. No validator declares a `*.md` glob, so the engine read none of the rule body.
    - checked by hand, and each one measured rather than reasoned: the example diagnostic in the rule body is byte for byte what the shipped script writes over the sibling-prefix tree; every one of this workspace's 58 reported paths stands on disk; the `tsc`-writes-no-configuration half of "Entry resolution fails OPEN" reaches the report through `main`'s catch, message text confirmed; the four decline reasons against the four `decline` call sites; the eleven acceptance tests still eleven.
    - corrected by hand before the commit, both my own false claims: a doc comment said an `include` glob reaches NEITHER reading of a linked file — `tsc` does list it, only ts-morph drops it; and a probe doc said the link target stands outside every `include` glob, when that probe states its program with `files` and has no glob at all.
    - NOT self-certified. The rule body is a thousand lines the engine cannot read, and every round of this card has found something there by hand. The card stays in `review` for that reading rather than going to done.
    - next: a human reads the rule body — the node program in the front matter (`realPath`, `realFiles`, `filesBySpelling`, the `manifests` catch), "The path each finding is reported at" above all its three paragraphs on what the count measures, the timing paragraph, the corpus caption, and "Entry resolution fails OPEN".
  timestamp: 2026-08-15T18:43:58.663539+00:00
- actor: claude-code
  id: 01m03bt12pagnqarnht13jxe83
  text: |-
    ### finish iteration 6 — clean
    - implement: changed — 3 files (round 6, the three findings of the 12:58 section)
    - test: green — cargo nextest --workspace, 14235 passed, 0 skipped
    - commit: bc820561f
    - review: clean — engine 0 findings over 2 files, 9 validator runs; hand-check of the rule body found and fixed 2 claims of my own before the commit

    The card stays in `review` on purpose. It is NOT stuck and it is NOT done: the engine cannot read the rule body, so a person reads it once and moves the card. The prior round left it here for the same reason and that reading is what raised the three findings this round closed.
  timestamp: 2026-08-15T18:44:05.846597+00:00
- actor: claude-code
  id: 01m03cqawvd9pyhrtqcyaeehr6
  text: |
    ### review — findings
    - evidence: 6 findings — dead-code-typescript.md:269, :882, :944, :951, :962, dead_code_typescript.rs:647. Scope `review sha 1335ae9ba..HEAD`; engine counts findings 1, confirmed 1, refuted 0 over 9 validator runs; 5 items from a hand check of the rule body no validator globs.
    - next: round 6 closes the three findings of round 5 with code. The implementer's correction is verified sound both ways: a `files` entry that is a symbolic link reproduces the divergence, an `include` glob does not. One finding is a recurrence of the round 2 and round 3 root cause on the new mechanism — a real path outside the workspace root is placed and then dropped silently, where the earlier placement declined it out loud. Task stays in review.
  timestamp: 2026-08-15T19:00:06.171803+00:00
- actor: claude-code
  id: 01m03e91j3hr684p04p22acfjf
  text: |-
    Round 7, on the six findings of 13:45. The behavior regression is the point of the round, and it is closed with code and a test that stages the reviewer's own tree.

    FINDING :269, THE REGRESSION. Reproduced exactly as the reviewer measured it, then fixed, then measured again. The probe stands at `<work>/repo` with `<work>/outside/util.ts` beside it, `files: ["src/index.ts", "src/link.ts"]`, and `src/link.ts` a link two segments up onto that file. Before the fix the shipped script wrote `/private/.../outside/util.ts:2: unused export 'trulyDead'; ...` on stdout and nothing on stderr — the engine drops that row, because `normalize_tool_path` cannot strip a root the path does not begin with and the workspace retain keeps only a path that meets a file of the run. After the fix stdout is empty and stderr carries one line:

        sah-diagnostic: the file this finding is about stands outside the workspace, at `/private/.../probe/outside/util.ts`, and the report carries a file of the workspace alone: `private/.../probe/outside/util.ts:2: unused export 'trulyDead'; nothing in the project imports it`

    THE FIX. `reportedPath` split into `workspacePrefix`, `insideWorkspace` and `reportedPath`, and `placeFindings` declines an item whose one standing candidate fails `insideWorkspace`. Nothing else changed in the placement, so a file inside the workspace places exactly as before: the corpus row for this workspace is 58 findings, stdout byte-identical to both earlier placements.

    WATCHED RED. The new acceptance test `..._says_the_file_outside_the_workspace_out_loud` failed on the shipped bytes with `left: ["/private/var/.../outside/util.ts:2"] right: []`, then passed. The harness gained what the shape needs: a probe repository now stands at `repo/` inside its work directory, and `drive_shipped_script` takes an `outside` list staged beside it. `drive_shipped_staged_tree_read_with` carries it; `drive_shipped_staged_tree_read` delegates with `NO_PROBE_OUTSIDE`, so the three existing callers are untouched.

    FINDING :882. The guarantee is retracted and replaced by one the code implements: every path the run writes is a path `tsc` listed, OR the real path of such a path — one is what `tsc` printed, the other is what the filesystem answers for it. The paragraph on what the run WRITES stands where the count is stated, and says the row carries the real path, and that a real path outside the workspace has no row at all.

    FINDING :944. Both places now name the `try` around the `main()` call, and the bullet adds `main` itself holds no `try`, so a reader who opens `main` is not sent looking.

    FINDING :951. The caption names the SHELL placement and points at the first row of the timing table. The acceptance-test paragraph was swept for the same cause: "before this change" and "the placement before this one" each name the placement they mean now.

    FINDING :962, RE-MEASURED RATHER THAN RESTATED. Three shipped scripts run one after another, that cycle repeated three times, this workspace, warm: the shell placement 6.31 / 6.72 / 6.76 s, the file-list placement 6.86 / 6.96 / 8.18 s, and the same reading the real path 7.17 / 7.37 / 7.85 s. All three answered 58 findings, byte-identical stdout (one md5 for all three), 0 declined, 0 bytes on stderr. The whole placement costs 0.55 s over the shell loop at the lowest readings; the real-path reading adds a further 0.31 s, under each of the three spreads (0.45 s, 1.32 s, 0.68 s). The paragraph states every number the claim rests on, and states that a reading moves between sessions — the corpus table reads 6.2 s for this workspace, taken in an earlier one.

    FINDING :647. The compiler-options block stood in SEVEN tsconfig constants, not the two the finding names, so the cause is removed from the whole file: `typescript_probe_tsconfig_head!()` holds the object opening through the last option, and each constant states its own `include`, `files`, `exclude` or `paths` after it. A macro rather than a constant because `concat!` takes literals — the same shape `dart_broken_run_path!` already uses in `missing_docs.rs`.

    TWO CLAIMS OF MY OWN CAUGHT BY HAND BEFORE THE COMMIT, both over-claims I wrote this round: "a file `tsc` listed under neither spelling places no finding at all" is false where another listed file wears the same cut spelling, which is the residue the count paragraph already states; and "A row leaves the list two ways" put back the closed-set count an earlier round had removed. Both rewritten. A third, pre-existing, was corrected beside them: "Where a row leaves the list, the report says so" claimed the `sort -u` collapse is stated, and it is not.

    WHAT A HUMAN MUST CHECK BY HAND. No validator declares a `*.md` glob, so the engine reads none of the rule body. By hand: (1) `workspacePrefix`, `insideWorkspace`, `reportedPath` and the new decline in `placeFindings`; (2) "The path each finding is reported at", above all the paragraph on what the row carries and the five decline reasons; (3) the timing table and the paragraphs under it; (4) the five acceptance tests named at the end of that section.
  timestamp: 2026-08-15T19:27:15.011548+00:00
- actor: claude-code
  id: 01m03e9yfg08sxqd76qhzmyp8d
  text: |-
    ### implement — changed
    - evidence: 5 files — builtin/validators/code-hygiene/rules/dead-code-typescript.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, .../tests/shipped/dead_code_typescript.rs, .../tests/shipped/dead_code_swift.rs, .../tests/shipped/stuttering_name_go.rs. New acceptance test `..._says_the_file_outside_the_workspace_out_loud`, watched RED on the shipped bytes (`["/private/var/.../outside/util.ts:2"]` against `[]`) then GREEN. All six review findings flipped to `- [x]`.
    - next: /test

    ### test — green
    - evidence: `cargo nextest run --workspace`: 14236 run, 14236 passed, 0 skipped. `cargo nextest run -p swissarmyhammer-validators`: 759 run, 759 passed. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. Shipped script over this workspace: 58 findings, stdout md5 identical for the shell placement, the file-list placement and the shipped one, 0 declined, 0 bytes on stderr.
    - next: /commit

    ### commit — changed
    - evidence: f16120809 fix(validators): decline a dead-code-typescript finding whose file stands outside the workspace (^yxky1aj)
    - next: /review
  timestamp: 2026-08-15T19:27:44.624749+00:00
- actor: claude-code
  id: 01m03ergf2379zjhvmhft8tf4g
  text: |-
    ### review — clean
    - evidence: engine `review sha 5d5f81d59~1..5d5f81d59` reported 0 findings over 4 files, 9 validator runs attempted, 0 failed, 0 skipped. No validator declares a `*.md` glob, so the engine read none of the rule body.
    - checked by hand, and the one it caught is FIXED in the commit rather than carried: the new decline bullet said "a symbolic link onto a directory beside the repository", and the probe stages a link onto a FILE standing outside the workspace. It now says what the probe stages.
    - checked by hand and measured rather than reasoned: the decline the shipped script writes over the reviewer's own tree — workspace root `<work>/repo`, link target `<work>/outside/util.ts` — is one `sah-diagnostic:` line on stderr and nothing on stdout; the five decline bullets against the five `decline` call sites the script holds; the timing table against three interleaved runs of the three scripts; the stdout of all three placements against one md5.
    - NOT self-certified. The rule body is a thousand lines the engine cannot read, and every round of this card has found something there by hand, including this one. The card stays in `review` for that reading.
    - next: a human reads the rule body — `workspacePrefix`, `insideWorkspace`, `reportedPath` and the new decline in `placeFindings`; "The path each finding is reported at", above all the paragraph on what the row carries and the five decline reasons; the timing table and the paragraphs under it; the five acceptance tests named at the end of that section.
  timestamp: 2026-08-15T19:35:41.794998+00:00
- actor: claude-code
  id: 01m03fg84s32qgnpwg2g26a1kw
  text: |
    ### review — clean

    - evidence: `review sha cff0fdf22~2..HEAD` — 0 findings over 4 files, 9 validator runs. All 19 prior findings checked. Hand-check of `builtin/validators/code-hygiene/rules/dead-code-typescript.md` found no behavior defect.
    - verified the RED/GREEN claim myself. Staged the round-6 tree by hand — workspace root `<work>/repo`, `files: ["src/index.ts", "src/link.ts"]`, `src/link.ts` a link on `../../outside/util.ts` — and ran the shipped bytes of both revisions. `5d5f81d59~1` wrote `/private/tmp/.../probe/outside/util.ts:2: unused export 'trulyDead'` on stdout and nothing on stderr, which the engine drops without a word. `HEAD` wrote nothing on stdout and one `sah-diagnostic: the file this finding is about stands outside the workspace, at ...` on stderr.
    - verified the placement is unchanged for a file INSIDE the workspace. The same tree with the link one segment up answered `outside/util.ts:2: ...` on both revisions, md5 `c087fab8f1944416848ab6d2e8858f5f` both. Over this workspace both revisions answered 58 findings, stdout md5 `63c367c26288b1296146971cf705d7db` both, 0 bytes on stderr both.
    - verified the decline is exact. `insideWorkspace` is the complement of what `normalize_tool_path` (`tool_rules.rs:977`) can strip, so an item it declines is one the workspace retain (`tool_rules.rs:955-960`) could never keep, and an item it passes carries a repo-relative path the retain can match. The decline can only trade a silent drop for an announced one.
    - verified the five decline reasons against the four `decline` sites of the placement, the timing table arithmetic (0.45, 1.32, 0.68, 0.55, 0.31 all correct), and the five acceptance test names. 12 tests in the module, 171 in `shipped`, all green.
    - 6 findings dropped for a false premise.
    - next: one prose point filed apart as ^jcgp8bc — the 0.55 s is stated as a cost without saying it stands under the 1.32 s spread of the second row, while the 0.31 s is called noise for exactly that reason.
  timestamp: 2026-08-15T19:48:39.705215+00:00
depends_on:
- 01M034AGX0RXH2RCCPPM6BA1BF
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff9080
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

## Review Findings (2026-08-15 11:08)

> Scope: `review sha a4a4160fe~1..a4a4160fe` — reviewed the diffs only — lines this change added or modified. 1 file(s) reviewed, 0 not reviewed.

The engine read one file, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs`, and reported no finding from 9 validator runs. No validator declares a `*.md` glob, thus the engine did not read the rule body `builtin/validators/code-hygiene/rules/dead-code-typescript.md`, which is the other file this commit changes. The items below come from a check by hand of that rule body against the installed `ts-prune` 0.10.3 presenter and against the tool-rule runner. Each item is on a line that this commit adds.

- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:220` `hand-check/path-arithmetic` — The comment says three spellings reach the loop, the table at line 747 gives "anywhere else" the whole absolute path less its leading separator, and the paragraph at line 791 says one shape only stays open. A fourth spelling is reachable. `String.replace` given a string cuts the FIRST occurrence of that text wherever it stands, and the working directory can stand at a position after the first character of the path. For the working directory `/w/packages/a` and the file `/mnt/backup/w/packages/a/src/x.ts`, the presenter writes `mnt/backup/src/x.ts`. None of `$cwd/$cut`, `$cwd$cut` and `/$cut` builds that path again, so `found` is 0 and the run refuses the finding. A nested copy of an absolute tree — a backup mount, a bind mount, a staged copy — makes that shape. State the fourth shape in the comment and in the table, and put it with the residue at line 791, which says one shape only.
- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:239` `hand-check/observability` — The refusal is silent. The comment at line 237 says the finding is "said out loud", and the prose at line 795 says the run "writes each such line on stderr, naming the project and the count". Nothing reads that stderr on a run that exits 0. `crates/swissarmyhammer-common/src/command.rs:56` sets `.stderr(Stdio::piped())`, so the bytes do not reach the terminal. `crates/swissarmyhammer-validators/src/review/tool_rules.rs:770-776` reads `output.stderr` only inside the `!output.status.success()` branch; on the success path it reads `output.stdout` alone and drops the buffer. A normal run of this rule exits 0. So a refused finding reaches no author, no log and no report — the same silent drop this rule was written to remove, and the shape `shipped.rs:931` names: a run that reports no finding and exits 0 reads exactly like a clean tree. `verify_shipped_tree_reports` holds the refusal only by the absence of the row from `expected`; no test holds the announcement. Carry the refusal out of the script on a path the author reads, and hold it with a test.

## Review Findings (2026-08-15 11:46)

> Scope: `review sha ebed84085~1..ebed84085` — reviewed the diffs only — lines this change added or modified. 2 file(s) reviewed, 0 not reviewed.

The engine read the two Rust files this commit changes — `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` and `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs` — over 9 validator runs, and reported no finding. No validator declares a `*.md` glob, thus the engine did not read `builtin/validators/code-hygiene/rules/dead-code-typescript.md`, which is the third file this commit changes. The items below come from a check by hand of that rule body and of the two Rust files, against the tool-rule runner. Each item is on a line that this commit adds.

The counts are verified. The implementer did not measure the corpus again, and argued the numbers cannot move because the `found` -eq 1 path is unchanged. The argument holds. The diff of the script touches comment lines only, plus the two `printf` lines INSIDE the `if [ "$found" -ne 1 ]` branch. `cut=`, `stands=""`, `found=0`, the candidate loop and the `case "$stands"` block are all context lines in the diff. A workspace whose drop count is 0 therefore runs the same bytes and writes the same rows. One finding was dropped for a false premise, this one.

- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:246` `hand-check/observability` — The drop still reaches nobody. The `printf` writes the drop at `${config#./}`, which is a `tsconfig.json`. The runner keeps a `scope: workspace` finding only when its path is one of the files of the RUN: `crates/swissarmyhammer-validators/src/review/tool_rules.rs:95-100` retains a finding only when `normalize_tool_path` of its path is in `run.files`. `run.files` is the changed-file work-list filtered through the rule's OWN `match.files` globs — `matched_rule_files` at `tool_rules.rs:421-441` calls `rule.matches(ruleset, &ctx)` per path. This rule declares six globs at lines 6-11: `**/*.ts`, `**/*.tsx`, `**/*.js`, `**/*.jsx`, `**/*.mjs`, `**/*.cjs`. A `tsconfig.json` matches none of them, so it can never stand in `run.files`, for ANY changed-file set. The retain therefore discards every drop announcement on every engine run. The channel moved from stderr to stdout, and the drop is still lost before the report. Write the drop at a path the rule's own globs select, or take the announcement off the finding channel altogether.
- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:824` `hand-check/accuracy` — The concession is not honest. It says the engine "keeps a workspace-scope finding only when its path meets a file of the run, so the drop reaches the report when the project's own `tsconfig.json` is one of the changed files". That names a condition a reader takes as satisfiable, and it is not: the rule's globs never select a `.json` path, so the tsconfig is never a file of the run. The text describes a narrow hole while the hole is total. State the condition that actually governs — the rule's own `match.files` list — and state that the current row satisfies it never.
- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:227` `hand-check/exhaustive-claim` — A new unconditional claim came in with the fourth shape. The comment says the shape "stands at no candidate below, so the run drops it"; line 748 says the working directory "rebuilds none of the fourth"; line 810 says "no rebuild reaches it"; line 800 says the run "drops rather than guesses" for both shapes. The first half is true — no candidate rebuilds the ORIGINAL file. The second half is not: `found` counts candidates that EXIST, not candidates that are the right file. For the working directory `/w/ab` and the file `/w/a/w/ab/src/x.ts` the cut is `w/a/src/x.ts`, and the candidate `/w/a/src/x.ts` stands whenever that file is present. Then `found` is 1, the `-ne 1` branch does not run, and the run reports the finding at a file it is not about — a wrong finding, not a drop. Say what `found` measures, and hold the fourth shape to the same statement the first three get.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs:693` `hand-check/coverage` — The new test cannot catch the item above at line 246. `drive_shipped_staged_tree_claims` at `shipped.rs:961` reaches `run_script_findings` through `drive_shipped_script`, and `run_script_findings` parses stdout and stops. The workspace retain of `run_tool_script` at `tool_rules.rs:95-100` is on neither path, so the test holds the sentence the SCRIPT writes and says nothing about the sentence the REPORT carries. `TYPESCRIPT_CONSUMER_DROP_ROW` in `expected` has the same gap. The rule body at line 858 states that the outside module is held "through the engine as well"; the drop is not. Drive the drop through the engine path that applies the retain, and hold the row there.

## Review Findings (2026-08-15 12:58)

> Scope: `review sha 9fcdd8387..HEAD` — reviewed the diffs only — lines this change added or modified. 3 file(s) reviewed, 0 not reviewed.

The engine read the three code files this range changes — `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, `.../tests/shipped/dead_code_typescript.rs` and `.../tests/shipped/dead_code_swift.rs` — over 9 validator runs, and reported no finding. No validator declares a `*.md` glob, thus the engine did not read `builtin/validators/code-hygiene/rules/dead-code-typescript.md`, which is the fourth file this range changes. The items below come from a check by hand of that rule body and of its node program, against the installed `ts-prune` 0.10.3 (`lib/presenter.js`, `lib/analyzer.js`) and against the engine's diagnostic carrier. Each item is on a line this range adds.

**The prior root causes are gone, and none of the four prior findings recurs.** The path-rebuild apparatus was deleted, not widened: `placeFindings` writes `reportedPath(standing[0], workspaceRoot)` and `standing[0]` is always an entry of `tsc -p tsconfig.json --listFilesOnly`, so no arithmetic can name a path the program does not hold, and a file that is not of the program cannot place a finding. Two program files carrying one spelling reach `standing.length !== 1` alongside zero, and the decline names the count, the spelling and the finding. The observability root cause is gone as well, verified end to end: `run_script` builds `diagnostics: marked_diagnostics(&stderr)` on the exit-0 path, `run_tool_script`'s workspace retain touches `outcome.findings` alone, and `render_tool_diagnostics` in `crates/swissarmyhammer-validators/src/review/synthesize.rs:647` writes every diagnostic into the report markdown. The `sah-diagnostic:` usage meets the contract in `builtin/validators/README.md` — the marked form, on stderr, at exit 0, on a channel no file filter can drop.

Six findings were dropped for a false premise: a file outside the program placing a finding; a spelling collision going unhandled; the corpus table reading as current, when its caption bolds that the zod, zustand and redux rows were measured under the EARLIER placement and marks the `declined` cell `—` for exactly those three; the diagnostic lost on the success path, which was round 2's finding; the diagnostic discarded by the workspace retain, which was round 3's finding; and `tsc` writing a config error into `files.txt`, which makes every finding decline and announces each one.

- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:836` `hand-check/exhaustive-claim` — The section states a guarantee the placement does not implement: "ts-prune builds its program from the same `tsconfig.json` `tsc` read, so a spelling ONE file of that list carries names the file ts-prune reported and no other. Where the two readings of the program disagree, the run declines rather than guesses, and the list below states each way they can." `placeFindings` tests `standing.length !== 1`, and `standing` holds the files of the program that CARRY the spelling, not the file that IS the reported one. The same section retracts the premise three paragraphs later: the bullet at line 883 states the two readings disagree because ts-prune reports `fs.realpathSync(result.file)` — confirmed at `ts-prune/lib/analyzer.js:213` — while `tsc --listFilesOnly` prints the path it globbed. A disagreement puts the reported file OUT of the list, so its spelling no longer collides with its own entry, and one other file of the program carrying that spelling leaves `standing.length` at 1. For the project `/w/app`, a `src/vendor` symbolic link onto `/w/w/app/pkg` and a directory `w/pkg` inside the project, `/w/w/app/pkg/util.ts` and `/w/app/w/pkg/util.ts` both spell `w/pkg/util.ts`; the run then reports the finding at a file it is not about — a wrong finding, not a decline. This is the shape of the checked item at line 227 of the 11:46 section, on the new mechanism: a count of what EXISTS standing in for a count of what is RIGHT. Say what `standing.length` measures, and hold a disagreement to the same statement the collision gets.
- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:895` `hand-check/attribution` — "The second `tsc` call is what those 0.7 s buy." The measurement supports the delta and not the attribution. The comparison itself is sound — the shipped script before this change against the shipped script after it, the lowest of three runs each, 5.6 s against 6.3 s — and it is a warm reading on both sides, so the cold-against-warm defect is corrected. But the same diff makes three changes to the loop, not one: it adds `tsc -p tsconfig.json --listFilesOnly` per project, it adds a `node "$work/prune.js" place` process per project, and it removes the shell placement loop. Over this workspace's two projects that is two added `tsc` runs and two added node process starts. Neither added cost was measured on its own, so the 0.7 s is the net of the three. Every other number in this file states the conditions it was taken under; this one names a cause it did not measure apart. Measure the two added costs separately, or name the whole placement rather than one call of it as what the 0.7 s buys.
- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:955` `hand-check/observability` — "Entry resolution fails OPEN. A `tsc` run that writes no configuration, and a manifest that does not parse, leave the pattern empty, the run then states `--ignore '$^'` ... The node script states that failure with the `sah-diagnostic:` marker, so the report carries it." That holds for the `tsc` half: `writeEntryPattern` reaches `JSON.parse` on the configuration, and `main`'s `try`/`catch` declines. It is false for the manifest half. `manifests()` wraps its own `JSON.parse` in `catch { continue; }` — a line this range adds — and writes nothing: no decline, no marker, no report line. Nor does an unparseable manifest leave the pattern empty; every other manifest still contributes, so the pattern is built and only that one package's entry modules fall out of it. Every export of that package's entry module then reports as dead, with nothing on any channel saying why — the silent shape this card exists to remove, arriving through the one read the script still swallows. Decline the manifest the run could not read, and hold the announcement with a test.

## Review Findings (2026-08-15 13:45)

> Scope: `review sha 1335ae9ba..HEAD` — reviewed the diffs only — lines this change added or modified. 2 file(s) reviewed, 0 not reviewed.

The engine read the two Rust files this range changes — `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` and `.../tests/shipped/dead_code_typescript.rs` — over 9 validator runs, and reported one finding. No validator declares a `*.md` glob, thus the engine did not read `builtin/validators/code-hygiene/rules/dead-code-typescript.md`, which is the third file this range changes. The items after the first come from a check by hand of that rule body and of its node program, run against the installed `ts-prune` 0.10.3 and `tsc` 5.9.3 over probe trees built for this review. Each item is on a line this range adds.

**The implementer's correction of the previous round's premise is sound, measured both ways.** Over a tree stating `files: ["src/index.ts", "src/link.ts"]` with `src/link.ts` a symbolic link onto `../outside/util.ts`, `tsc -p tsconfig.json --listFilesOnly` printed `src/index.ts` and `src/link.ts`, and `ts-prune` reported `.../probe/outside/util.ts:2 - trulyDead`. Over the SAME tree stated with `include: ["src"]`, `tsc` listed the link either way and ts-prune's program held `src/index.ts` alone — it reported `src/index.ts:2 - surface` and nothing at all for the link. So a `files` entry that is a symbolic link is the route that reproduces the divergence, an `include` glob is not, and the fix covers the route that reproduces it. Both new acceptance tests pass.

**The three findings of 12:58 are each closed with code.** `realFiles` puts the reported file among its own candidates, so the round-5 wrong-finding shape now reaches `standing.length !== 1` and declines. The timing paragraph attributes the 0.7 s to the whole placement and states plainly that the measurement does not divide it. `manifests()` declines by name on the `sah-diagnostic:` channel instead of `catch { continue; }`, and an acceptance test holds both halves.

**Six findings were dropped for a false premise:** `fs.realpathSync` on a path that does not exist — it throws, `realPath` catches it and answers the path itself, and the residue paragraph at line 890 covers the consequence; `decline` and `reportedPath` being called from `manifests` above their own declarations — a function declaration hoists; the `placeFindings` doc claiming without condition that the reported file stands among its own candidates — the residue paragraph states the exception; the four decline reasons reading as a closed set — the text claims no closure and the fourth reason is the catch-all; ts-prune's `emitTsConfigEntrypoints` not reading a real path — that is ts-prune's own code and not this rule's; and `manifests()` writing one diagnostic per project rather than one per workspace — no prose claims otherwise.

- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:269` `hand-check/observability` — `standing` now holds the REAL path, so the row the run writes is the path behind a symbolic link. When that real path stands OUTSIDE the workspace root, the row leaves the report without a word. Measured over a probe tree — workspace root `.../probe/repo`, `files: ["src/index.ts", "src/link.ts"]`, `src/link.ts` a link onto `.../probe/outside/util.ts` — the shipped script wrote `/private/tmp/.../probe/outside/util.ts:2: unused export 'trulyDead'; nothing in the project imports it` on stdout and NOTHING on stderr. `normalize_tool_path` at `crates/swissarmyhammer-validators/src/review/tool_rules.rs:977` cannot strip a root that is no prefix, so the path stays absolute, and the workspace retain at `tool_rules.rs:955-959` keeps a finding only when its normalized path is one of `run.files` — repo-relative every one. The finding is discarded silently. The placement before this change DECLINED the same item and said so: run against `1335ae9ba`'s script, the same tree answered `sah-diagnostic: 0 files of the program of tsconfig.json carry the spelling ...`. So this range trades an announced decline for a silent drop — the shape this card exists to remove, and the root cause of the checked items of 11:08 and 11:46 arriving on the new mechanism. Decline a placement whose real path stands outside the workspace root, or write it at the listed path, and hold it with a test.
- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:882` `hand-check/accuracy` — The added paragraph makes the run write a path `tsc` did not list, and the standing guarantee at line 911 is left unretracted: "every path the run writes is a path `tsc` listed for the program, so no line of the report is a path the run made up." `placeFindings` writes `reportedPath(standing[0], workspaceRoot)`, and `standing[0]` is now the REAL path, which for a `files` entry that is a symbolic link is a path `tsc` never printed. This range's own acceptance test states it: `TYPESCRIPT_LINKED_FILE_TSCONFIG` lists `src/index.ts` and `src/link.ts`, and `expected` is `outside/util.ts:2`. Measured by hand over the same shape, `tsc --listFilesOnly` printed the two `src/` paths and the run wrote `.../probe/outside/util.ts`. State what the run writes — a path `tsc` listed, OR the real path behind one — and state what still holds in place of the retracted guarantee.
- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:944` `hand-check/accuracy` — The prose names `main` as what catches, and `main` contains no `try`. The program reads `try { main(); } catch (failure) { decline(...) }` — the catch stands on the CALLER of `main`, so a reader who opens `main` to find the handling finds none. Line 1027 repeats the same attribution for the entries job: "the parse throws, `main` catches it and states `the entries job broke in <directory>: <failure>`". Name the top-level `try` around the `main()` call as what catches, in both places.
- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:951` `hand-check/accuracy` — The added paragraph names THREE placements — the shell placement, the file-list placement, and the file-list placement reading the real path of each listed file — and the caption fifteen lines below still reads "The three library rows below were measured under the EARLIER placement". With three placements named directly above it, "the EARLIER placement" names two of them, and the reading a reader takes first — the placement immediately before this change, the file-list one — is the wrong one: zod's 76, zustand's 1 and redux's 6 were measured under the SHELL placement. Name the placement each of those three rows was measured under.
- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:962` `hand-check/accuracy` — "moved the reading by 0.03 s, under the spread of the three runs the file-list placement itself answered". Neither number the claim rests on is written down. The paragraph above gives the two readings as 6.2 s and 6.2 s, one decimal each, which carries no 0.03 s difference; and the spread of the three runs is stated nowhere in this file. So a reader cannot check "under the spread" against anything the file holds. Every other number in this file states the conditions it was taken under and the value it was read at. State the two readings at the precision the 0.03 s needs, and state the spread.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs:647` `code-hygiene/magic-numbers` — TypeScript compiler options block is repeated identically in both TYPESCRIPT_LINKED_FILE_TSCONFIG (lines 562–569) and TYPESCRIPT_TWO_PACKAGE_TSCONFIG (lines 649–656). Repeated configuration should be extracted to a single named constant to avoid maintenance drift. Extract the compiler options block into a shared constant (e.g., `const TYPESCRIPT_COMPILER_OPTIONS: &str = concat!(…)`) and compose both TYPESCRIPT_LINKED_FILE_TSCONFIG and TYPESCRIPT_TWO_PACKAGE_TSCONFIG by referencing it.
