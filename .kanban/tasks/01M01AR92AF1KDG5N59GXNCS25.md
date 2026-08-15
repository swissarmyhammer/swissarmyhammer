---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m02v5f95mz575d1r67bxh1ve
  text: |-
    NOT MOOTED. ^3r5bhpj decided the knip question and KEPT `ts-prune`, so this card stands as written and its fix is still owed.

    Worth recording before that work starts: the swap would NOT have answered this card either. Measured with knip 6.32.2 over `zod` at `4e1720c` with `node_modules` installed, replacing `packages/zod/tsconfig.json` with bytes that are not JSON — knip exits 1, the finding count holds at 17, and the JSON on stdout is BYTE-IDENTICAL to the healthy run. 174 bytes on stderr are the only difference. The shipped ts-prune rule has the same shape at exit 0 with its count held at 76.

    So a `tsconfig.json` a tool cannot read is silent under BOTH tools, and neither status nor stdout finds it. The fix this card asks for has to READ STDERR as well as the exit status. That is a firmer requirement than the card states today, and it is the same shape the three shipped swiftlint rules already carry, inverted: they read stderr to answer clean, this one must read stderr to break.

    The status table for ts-prune that this card asks to measure is still owed. `dead-code-typescript.md` now carries the knip status table beside it, which is a useful comparison when the ts-prune one is written.
  timestamp: 2026-08-15T13:53:15.045870+00:00
- actor: claude-code
  id: 01m03jb4j4401jx7ts8bg4ygdn
  text: |-
    Picked up. Research done, and the measurement the card asks for is taken.

    MEASURED with ts-prune 0.10.3, tsc 5.9.3 and node v25.2.1, each run as the shipped script runs it — `ts-prune -p tsconfig.json --ignore '$^' --skip '$^'`, the project's own directory the working directory:

    | the run | status | stdout | stderr |
    |---|---|---|---|
    | a project with no dead export | 0 | 0 bytes | 0 bytes |
    | a project holding one dead export beside an entry module | 0 | 2 rows | 0 bytes |
    | a `tsconfig.json` of bytes that are not JSON | 1 | 0 bytes | 1847 bytes, a node stack |
    | no `tsconfig.json` at all | 1 | 0 bytes | 2350 bytes, a node stack |
    | a `tsconfig.json` that cannot be read, mode 000 | 1 | 0 bytes | 2263 bytes, `EACCES` |
    | a `tsconfig.json` whose root value is `[]` | 1 | 0 bytes | 1893 bytes |
    | a `package.json` on the way UP that does not parse | 1 | 0 bytes | 2110 bytes, cosmiconfig |
    | a `tsconfig.json` naming a `compilerOptions.target` that is not a target | 0 | 2 rows | 0 bytes |
    | a `tsconfig.json` whose `extends` names no file | 0 | 2 rows | 0 bytes |
    | a `tsconfig.json` whose `include` reaches no file | 0 | 0 bytes | 0 bytes |

    So the STATUS separates a broken run from a measured one for every shape measured here: ts-prune answers 0 for a clean project AND for a project holding findings, and 1 for each shape that judged nothing. No stderr test is needed for ts-prune.

    That CORRECTS the earlier comment on this card. The comment read "a `tsconfig.json` a tool cannot read is silent under BOTH tools, and neither status nor stdout finds it", which was measured for knip. ts-prune's own status IS 1 for that shape. What answers 0 is the SHIPPED SCRIPT, because the per-project pipe ends in the node placement and the loop ends in `sort -u`. The status is thrown away by the script, not withheld by the tool.

    Two more facts read before the edit:

    - The rule is `scope: workspace`. `builtin/validators/README.md` states that such a script "writes no `"$@"` and no zero-argument guard", and `tests/shipped/scope_roster.rs` holds every workspace-scope rule to both. So the card's zero-argument requirement — which it states "applies to a `files`-scope script" — is answered by NOT writing the guard here. Writing it would break `no_shipped_workspace_scope_script_holds_the_zero_argument_guard` and would exit the run before ts-prune starts.
    - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` already carries the shape this needs: `verify_shipped_tree_breaks` holds a run to breaking with an error that names every fragment the probe states, and `command_failure_detail` puts the whole stderr in that error.
    - The existing acceptance test `..._says_the_manifest_it_could_not_read_out_loud` stages its broken `package.json` BESIDE the project rather than above it, so ts-prune never walks up onto it. That test stays green under this change.
  timestamp: 2026-08-15T20:38:17.924754+00:00
- actor: claude-code
  id: 01m03jxsfjs0ghr73xhkywwakz
  text: |-
    Implementation landed, TDD, RED watched first.

    RED: the new test `the_shipped_typescript_dead_code_tool_rule_breaks_on_a_project_ts_prune_cannot_read` failed against the shipped script at HEAD with `... : []` — the run answered `Ok([])`, 0 findings at exit 0, over a project ts-prune never opened. That is the defect this card names, reproduced through the real engine path.

    GREEN: the script now runs ts-prune into a file, reads its status, and exits nonzero.

    - `find` writes the project list to a file, and the loop reads it with `done < "$work/projects.txt"` rather than standing in a pipeline, so `continue` and the shell variables belong to the script rather than to a subshell.
    - Each project's ts-prune run writes `"$work/prune.out"` and `"$work/prune.err"`, and `status=$?` reads its status.
    - `cat "$work/prune.err" >&2` forwards ts-prune's own words for every project, whole or broken.
    - A nonzero status writes `dead-code-typescript: ts-prune exited <status> for <project> and judged no export of it` on stderr, adds the project to an unread list, and takes the loop to the next project.
    - The placement appends to `"$work/rows.txt"`. After the loop the script exits 1 when the unread list holds anything, and otherwise runs `sort -u "$work/rows.txt"` as its last command, so `sort` stands with no command above it and the script takes its own status.
    - `doctor.check_command` gains `cat`, because the script runs it. `dead-code-python` and `unused-dependencies-rust` name `cat` the same way.

    MEASURED, the earlier pipe beside the shipped script, over three probe trees each holding one export nothing imports:

    | the tree | the earlier pipe | the shipped script |
    |---|---|---|
    | one project ts-prune read | 1 finding, exit 0 | 1 finding, exit 0 |
    | one project whose `tsconfig.json` is not JSON | 0 findings, exit 0 | 0 findings, exit 1, the project named |
    | two projects, one ts-prune read and one it did not | 1 finding, exit 0 | 0 findings, exit 1, the broken project named |

    MEASURED over this workspace, the shipped script, three runs one after another: 58 findings, exit 0, 0 items declined and 0 bytes on stderr each time, in 10.21 s, 7.54 s and 7.47 s. 58 is the count the rule body already states for this workspace, so the status gate takes no finding away.

    Two notes on the card's wording, neither of them a change of scope:

    - The card names `complexity-swift`. No rule of that name ships. The shape it points at is the one the three shipped swiftlint rules carry — `function-length-swift`, `magic-numbers-swift` and `missing-docs-swift` — beside `missing-docs-python`: run the tool into a file, read the status, write the tool's stderr and a line of the rule's own, exit nonzero. That is the shape adopted.
    - The card's zero-argument requirement states it "applies to a `files`-scope script". This rule is `scope: workspace`. `builtin/validators/README.md` states that such a script "writes no `"$@"` and no zero-argument guard", and `tests/shipped/scope_roster.rs` holds every workspace-scope rule to both statements. So no guard was written, and the shipped script still carries none. `no_shipped_workspace_scope_script_holds_the_zero_argument_guard` and `no_shipped_workspace_scope_script_reads_the_argument_list` both pass.
  timestamp: 2026-08-15T20:48:29.170076+00:00
- actor: claude-code
  id: 01m03jyd7550f543x7g7wbz6z4
  text: |-
    The card asks which shape a per-project break is when OTHER projects were judged fine, and asks for the justification here. It is the FIRST shape — a broken run — and the shipped script exits nonzero for it. The rule body carries the same argument under "A run cannot answer zero for a project ts-prune never read".

    Why it is not a declined item on the `sah-diagnostic:` channel:

    1. **A project is not one item.** `builtin/validators/README.md` gives that channel to "a script that judged the code and could not judge ONE item", and it names what an item is: "a path no compile database covers, a manifest the tool could not resolve, a configuration the tool read differently than the rule asked for". ts-prune judged NO export of a broken project — not one export it could not place. The placement's own declines are the item shape here, and they keep the diagnostic channel and exit 0 exactly as before.

    2. **The ordinary Node repository holds ONE `tsconfig.json`.** There the broken project and the whole tree are the same thing, and a diagnostic at exit 0 there is the README's own trap word for word: "a tool that refused to start reports as a clean file". Letting the answer depend on how many `tsconfig.json` files a repository happens to hold would give one defect two different answers.

    3. **The finding list of the healthy project makes the run LOOK measured.** Measured, row 3 of the table in the comment above: the earlier pipe over two projects — one read, one not — wrote the healthy project's 1 finding and exited 0. The engine then kept that finding and read every file of the broken package as clean. A diagnostic beside those rows still leaves the report stating findings for a tree half of which was never judged.

    What the run does NOT do, so the fix trades no announced decline for a silent drop:

    - It does not stop at the first broken project. Every project is read, so the stderr names each one ts-prune could not read.
    - It does not go quiet about the projects it did judge for any other reason. Every branch that stops short says something: a broken project takes a line of the run's own beside ts-prune's whole stderr, and every item the placement cannot place keeps its `sah-diagnostic:` line at exit 0.
    - The findings the run did place are not written to a stdout the engine throws away. The row file is read after the loop and only when no project stands in the unread list, so a broken run writes 0 bytes to stdout and the whole of its stderr reaches the diagnosing agent.

    The cost is stated rather than hidden: one project ts-prune cannot read costs this rule the findings of every other project of that run, until the author fixes the tsconfig. That is the cost the card's first shape carries by definition, and the run names the project so the fix is one line away.
  timestamp: 2026-08-15T20:48:49.381916+00:00
- actor: claude-code
  id: 01m03jym3aejnns5q75j4z8myp
  text: |-
    ### implement — changed
    - evidence: 2 files — builtin/validators/code-hygiene/rules/dead-code-typescript.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs. `cargo nextest run -p swissarmyhammer-validators` 763 tests run, 763 passed, 0 skipped. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-15T20:48:56.426924+00:00
- actor: claude-code
  id: 01m03k6dt7ar9j3z63msh55m6z
  text: |
    ### test — green
    - evidence: `cargo fmt --check` — exit 0, no diff. `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, 0 warnings. `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3520 tests run, 3520 passed, 0 failed, 0 skipped (26 slow, 1 leaky) in 130.2s.
    - detail: all 14 `review::tool_rules::tests::shipped::dead_code_typescript::*` tests passed. The slowest is `the_shipped_typescript_dead_code_tool_rule_names_a_module_outside_the_project_directory` at 9.1s (the ts-prune fixture).
    - note: the 1 leaky test is `code-context-cli::bin/code-context cli::tests::help_displays_all_lifecycle_commands`. It passes. It is not in the changed crate, and this change does not touch it.
    - next: review.
  timestamp: 2026-08-15T20:53:12.135742+00:00
- actor: claude-code
  id: 01m03kszkbcxk63jgt54fv2kgg
  text: |-
    ### review — findings
    - evidence: 1 finding — builtin/validators/code-hygiene/rules/dead-code-typescript.md:1216. `review sha HEAD~1..HEAD` answered 0 findings over 1 file, and the rule body is prose the engine reads no byte of, so the body was checked by hand: the shipped script was extracted from the `run:` block and run over four probe trees with ts-prune 0.10.3, tsc 5.9.3 and node v25.2.1. Every other body claim HOLDS — the three-tree table, the ten-run status table, "nothing the run did place reaches stdout", "the run reads every project before it stops" (two broken projects take two lines), the forwarded ts-prune stderr, the 58-finding workspace run, the thirteen acceptance tests, the mktemp/trap pair on lines 18 and 19, and the absence of `"$@"` and of the zero-argument guard at `workspace` scope. No false exhaustive claim stands: line 1143 states the table is "not a roster of every status ts-prune can answer with". No branch stops short in silence.
    - detail: the open finding is a coverage claim, not the script. `verify_shipped_tree_breaks` calls `expect_err`, and `run_script` answers `Err(ScriptFailure::Exit(..))` before it parses stdout, so the test cannot read the `0 findings` half of rows 2 and 3.
    - verdict: the two card wordings the implementer could not follow were both right. No rule named `complexity-swift` ships, and `builtin/validators/README.md:221` forbids the zero-argument guard at `workspace` scope.
    - next: /implement to answer the finding, then /review again.
  timestamp: 2026-08-15T21:03:52.939948+00:00
- actor: claude-code
  id: 01m03ktvrtexnr5tnx1npbmbxj
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 2 files (builtin/validators/code-hygiene/rules/dead-code-typescript.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs)
    - test: green — cargo fmt --check exit 0; cargo clippy --workspace --all-targets -- -D warnings exit 0, 0 warnings; cargo nextest run -E 'rdeps(swissarmyhammer-validators)' 3520 passed, 0 failed, 0 skipped
    - commit: 643e5ca4b fix(validators): break dead-code-typescript on a project ts-prune cannot read (^gxncs25)
    - review: findings — 1 finding, builtin/validators/code-hygiene/rules/dead-code-typescript.md, the coverage claim that the new acceptance test "holds rows 2 and 3 of the table above"
    - next: iteration 2 works the open finding
  timestamp: 2026-08-15T21:04:21.786060+00:00
- actor: claude-code
  id: 01m03mv28p7sy21db79twybwhs
  text: |-
    Picked up for the open finding. The fix chosen is the one that makes the CLAIM true, not the one that weakens the prose — the review named both options and said the second leaves row 3 with no coverage at all.

    WHY THE TEST COULD NOT SEE THE ROW COUNT, read before the edit:

    - `run_script` (`crates/swissarmyhammer-validators/src/review/tool_rules.rs`) answers `Err(ScriptFailure::Exit(..))` the moment the status is nonzero, and `parse_tool_stdout` stands AFTER that return. So no caller of `run_script` can read the stdout of a broken run. That is the engine's own contract and it is right for the engine — a broken tool judged the code nowhere, and its rows are not a partial answer.
    - `verify_shipped_tree_breaks` calls `expect_err` and `assert_shipped_failure_names` reads the error text alone.

    WHAT WAS DONE:

    1. `run_script` was split. The reading of a COMPLETED run moved to `read_script_output(&Output)`, and `run_script` is now `run_shell(..)` + `read_script_output(..)`. Pure decomposition — the engine calls the same bytes in the same order, and every branch and message is unchanged.
    2. `drive_shipped_script_whole` in `tests/shipped.rs` drives ONE run through `run_shell` with the same `script_args` the engine builds, keeps the `Output`, and answers a `ShippedScriptRun`: the answer `read_script_output` gives (what the engine would answer), and the `path:line` rows `parse_tool_stdout` + `finding_rows` read off that same run's stdout. Both halves off one run, both read with the engine's own code — no second interpretation of the contract.
    3. `drive_shipped_script` and `drive_shipped_staged_tree_with` were rewritten on top of it, so the staging stands in ONE place and nothing is copied.
    4. `verify_shipped_tree_breaks` now holds a breaking run to BOTH: the error names every fragment the probe states, AND the run placed no row. Its doc comment already claimed "holds a run to no finding and to an error that names what broke" — that claim was false for every caller and is now what the code does.

    The row assertion was given to `verify_shipped_tree_breaks` rather than to the two TypeScript probes alone, because the helper's own doc states the no-finding half for every caller. Measured: all 11 breaking probes of 6 shipped rules (dead-code-rust, dead-code-typescript, missing-docs-rust, function-length-rust, unused-dependencies-rust, stuttering-name-go, missing-docs-dart) pass it. No shipped script places a row on the path it breaks on.

    `verify_shipped_tree_breaks_without_run_of` was NOT changed. Its doc claims the error alone, no rule prose states a row count for it, and its probes break with a stubbed `PATH` rather than with a tool that read part of the tree.
  timestamp: 2026-08-15T21:21:57.014979+00:00
- actor: claude-code
  id: 01m03mvr2s2e3xjnvejt32az3a
  text: |-
    RED WATCHED FIRST, and it failed for exactly the right reason.

    The new assertion is about behaviour that already holds, so the RED had to be induced. The shipped script's break branch was temporarily changed from

        if [ -s "$work/unread.txt" ]; then
          exit 1
        fi

    to write the rows it had placed before exiting:

        if [ -s "$work/unread.txt" ]; then
          sort -u "$work/rows.txt"
          exit 1
        fi

    `the_shipped_typescript_dead_code_tool_rule_breaks_on_a_project_ts_prune_cannot_read` then FAILED with:

        a run that breaks must place no finding; it placed ["packages/app/src/lib.ts:2"]: one project
        ts-prune could not read leaves every export of that project unjudged, so the run breaks and
        names it rather than answering the findings of the project it did judge

    That is row 3 of the table, exactly: the run still exited nonzero and still named the broken project — the OLD assertion passed on that sabotaged script — and the only thing that caught it was the new half. The readable project's row reaching stdout is the whole difference the review named.

    The sabotage was reverted. `git diff` over `dead-code-typescript.md` now touches the prose paragraph alone; the `run:` block is byte-identical to HEAD. The test passes GREEN.

    ALSO CORRECTED, the same cause elsewhere in the file: every other coverage claim in `dead-code-typescript.md` was checked by hand against what its named test asserts.

    - line 762 `..._names_every_export_of_a_module_nothing_imports` "holds the probe above" — `TYPESCRIPT_ORPHAN_MODULE_PROBE` expects `src/orphan.ts:2`, `:5` and `:10`, which is "rows 2, 5 and 10". HOLDS.
    - line 846 `..._keeps_its_own_gate_beside_a_project_config` "holds row 2" — the probe stages a `package.json` carrying `"ts-prune": {"ignore":"src","skip":"src"}` and expects 1 finding. Row 2 is 1 finding. HOLDS.
    - line 878 `..._reads_the_program_the_project_states` "holds both rows of the table" — `TESTS_IN_PROGRAM` expects `src/lib.ts:5` (the export nothing imports) and `TESTS_OUT_OF_PROGRAM` expects `:2` and `:5` (both). Rows 1 and 2. HOLDS.
    - lines 1079-1096, the five tests of "How the run is shaped" — each named test stages the tree the sentence names. HOLDS.
    - line 1123 `..._says_the_manifest_it_could_not_read_out_loud` "holds both halves" — the probe expects `packages/app/src/lib.ts:2` and `packages/other/src/index.ts:2`, and NOT `packages/app/src/index.ts`, so the whole package's entry is spared and the broken package's entry reports; the test also holds the one diagnostic to naming the manifest. HOLDS.
    - line 1245 "The thirteen acceptance tests" — 13 `#[test]` functions, and `cargo nextest run -E 'test(shipped::dead_code_typescript)'` runs 13. HOLDS.

    ONE FALSE MEASUREMENT FOUND AND FIXED, in the test file rather than the rule: the doc comment on `..._reads_the_program_the_project_states` read "Measured over 12 `tsconfig.json` projects of the four corpus workspaces". The rule body's corpus table counts 9, 2, 3 and 2 — sixteen — and line 876 of the rule states 16. The 12 was written when the file was created (7f4ef93a3) and the 16 was corrected later (b0d05f7a8, "correct dead-code-typescript vs knip arithmetic"), so 12 is the stale one. It now reads 16 and names the table it comes from.
  timestamp: 2026-08-15T21:22:19.353788+00:00
- actor: claude-code
  id: 01m03n098jdscw24zffbvb9hx5
  text: |-
    ### implement — changed
    - evidence: 4 files — builtin/validators/code-hygiene/rules/dead-code-typescript.md, crates/swissarmyhammer-validators/src/review/tool_rules.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs. `cargo fmt --check` exit 0. `cargo clippy --workspace --all-targets -- -D warnings` exit 0, 0 warnings. `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` 3520 tests run, 3520 passed, 0 failed, 0 skipped. `cargo nextest run -E 'test(shipped::dead_code_typescript)'` 13 tests, 13 passed.
    - finding: the one open finding is answered and checked. The claim is now TRUE rather than weakened — the test reads the finding count on the error path.
    - next: /review
  timestamp: 2026-08-15T21:24:48.018735+00:00
- actor: claude-code
  id: 01m03nbaxrm9kmhhzqd33q3r38
  text: |
    ### test — green
    - evidence: `cargo fmt --check` exit 0, no diff. `cargo clippy --workspace --all-targets -- -D warnings` exit 0, 0 warnings. `cargo nextest run --workspace` — 14240 tests run, 14240 passed, 0 failed, 0 skipped (25 slow, 2 leaky) in 261.489s. The 13 shipped `dead_code_typescript` tests are in the run; `cargo nextest list -p swissarmyhammer-validators` confirms them by name.
    - next: review
  timestamp: 2026-08-15T21:30:50.168663+00:00
position_column: doing
position_ordinal: '8280'
title: dead-code-typescript answers zero findings when ts-prune crashes
---
`builtin/validators/code-hygiene/rules/dead-code-typescript.md` ends its per-project pipe in `sed` and the loop in `sort`, so the exit status of `ts-prune` is thrown away.

`ts-prune` 0.10.3 crashes with an unhandled error when it cannot read a `tsconfig.json`. Measured on a probe holding one dead export beside a `tsconfig.json` of bytes that are not JSON: `@ts-morph/common` throws, the stack goes to stderr, and the shipped script reports 0 findings and exits 0. The engine reads exit 0 as "the tool judged the code", so a project with a broken tsconfig reads as a clean workspace.

`builtin/validators/README.md` names this trap word for word: "Write a pipe only where the tool cannot exit nonzero. Otherwise write a script: run the tool into a file, test the status, and exit nonzero yourself."

The fix is the shape `complexity-swift` and `missing-docs-python` already carry: run `ts-prune` into a file, read its status, and exit nonzero with a line on stderr for the statuses that mean a broken run. Measure which statuses `ts-prune` answers with for a clean project, for a project holding findings, and for each broken shape, and state the table in the rule body. Ship an acceptance test beside the five in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs`.

Found while implementing ^108bh4y. #tool-validators #objectivity

## Review Findings (2026-08-15 15:56)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. The engine reviewed 1 file and answered 0 findings. The rule body is prose the engine reads no byte of, so every claim in it was checked BY HAND against the shipped script: the script was extracted from the `run:` block, run over four probe trees, and each body claim was measured.

- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:1216` `hand/tool-rule-contract` — the body states "The acceptance test `..._breaks_on_a_project_ts_prune_cannot_read` holds rows 2 and 3 of the table above", and the test holds two of the three things each of those rows states. Each row's "the shipped script" cell states three facts: `0 findings`, `exit 1`, and `the project named`. `verify_shipped_tree_breaks` (`crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1101`) calls `expect_err`, which throws the `Ok(Vec<String>)` payload away, and `assert_shipped_failure_names` reads the error text alone. The helper CANNOT read the row count: `run_script` (`crates/swissarmyhammer-validators/src/review/tool_rules.rs:907`) answers `Err(ScriptFailure::Exit(..))` before `parse_tool_stdout` at line 914 reads stdout at all. Row 3 is where the missing half carries the weight — `0 findings` is the whole difference between a run that breaks and a run that states the readable project's row as a measured tree. Either state what the test holds, which is the nonzero exit and the named project, or hold the row count with a helper that reads the stdout of a broken run.

### What was measured and HOLDS

The shipped script was extracted from lines 18..418 of the rule (4-space dedent, 401 lines, opening `work="$(mktemp -d)"` and closing `sort -u "$work/rows.txt"`) and run with ts-prune 0.10.3, tsc 5.9.3 and node v25.2.1 — the versions the body names.

| the probe | exit | stdout rows | the script's own stderr |
|---|---|---|---|
| one project ts-prune read | 0 | 1 | 0 bytes |
| one project ts-prune could not read | 1 | 0 | the project named |
| one it read beside one it did not | 1 | 0 | the broken project named |
| one it read beside TWO it did not | 1 | 0 | BOTH broken projects named |

- The three-tree comparison table at lines 1178-1182 reproduces row for row.
- Line 1213 "Nothing the run did place reaches stdout for such a run" holds: the readable project's row stands in `rows.txt` and stdout is 0 lines.
- Lines 1199-1201 "The run reads every project before it stops" holds: two broken projects take two lines.
- Line 1207 "forwards ts-prune's own stderr for every project it reads" holds: `cat "$work/prune.err" >&2` runs over the status test.
- The ten-run status table at lines 1145-1156 reproduces every row, including the five that answer 0.
- No false exhaustive claim: line 1143 reads "not a roster of every status ts-prune can answer with", and line 1167 hedges "for every shape measured here".
- The workspace measurement at line 1220 holds: 58 findings, exit 0, 0 items declined, 0 bytes on stderr.
- Line 1245 "thirteen acceptance tests" holds: 13 `#[test]` functions.
- `scope: workspace` carries no `"$@"`, no `$#` and no zero-argument guard, which `builtin/validators/README.md:217-227` asks of it and `tests/shipped/scope_roster.rs` guards.
- `work="$(mktemp -d)"` at line 18 and `trap 'rm -rf "$work"' EXIT` at line 19 stand on lines directly under each other.
- No branch stops short in silence. The two `tsc ... 2>/dev/null` lines discard a status, but tsc writes its diagnostics to stdout, so the text reaches `config.json`, the node `entries` job throws on it, and `decline()` states it with the `sah-diagnostic:` marker at exit 0 — the README's own channel.

### The two card wordings the implementer could not follow

Both calls were right.

- No rule named `complexity-swift` ships. The swiftlint rules are `dead-code-swift`, `function-length-swift`, `magic-numbers-swift` and `missing-docs-swift`, and each carries the shape the card points at.
- The zero-argument guard is forbidden at `workspace` scope. `builtin/validators/README.md:221` states such a script "writes no `"$@"` and no zero-argument guard", and two coverage guards hold every workspace-scope rule to both statements.