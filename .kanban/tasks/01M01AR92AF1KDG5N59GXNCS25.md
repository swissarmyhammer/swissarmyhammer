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
- actor: claude-code
  id: 01m03p6fwgpnbq3y2hsav4qqnj
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` — 3 files reviewed, 9 validators attempted, 0 findings from the engine. By-hand prose check of commit 17f72ce80 answered 1 finding — builtin/validators/code-hygiene/rules/dead-code-typescript.md:1217
    - next: the body claims the acceptance test holds "all three facts" of rows 2 and 3; it holds `0 findings` and `the project named`, not `exit 1`. `expect_err` separates only success from failure, and `command_failure_detail` answers `output.status` only when stderr is empty while this run always writes to stderr. Either word the claim as the nonzero exit, the named project and the 0 findings, or read the status number with a path that keeps `output.status`. Task stays in `review`.
  timestamp: 2026-08-15T21:45:39.984486+00:00
- actor: claude-code
  id: 01m03p7ekqpt0y58frxcnv8kzh
  text: |
    ### finish iteration 2 — findings
    - implement: changed — 4 files (builtin/validators/code-hygiene/rules/dead-code-typescript.md, crates/swissarmyhammer-validators/src/review/tool_rules.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs)
    - test: green — cargo fmt --check exit 0; cargo clippy --workspace --all-targets -- -D warnings exit 0, 0 warnings; cargo nextest run --workspace 14240 passed, 0 failed, 0 skipped
    - commit: 17f72ce80 test(validators): hold a broken tool-rule run to placing no finding (^gxncs25)
    - review: findings — 1 finding, builtin/validators/code-hygiene/rules/dead-code-typescript.md:1217. The engine answered 0 findings over 3 files with 9 validators attempted. The finding is in the prose the engine reads no byte of.
    - detail: iteration 1's finding IS answered — `verify_shipped_tree_breaks` now reads the `0 findings` half off the same run through `drive_shipped_script_whole`. The new wording overshot: it claims the test holds "all three facts of each" of rows 2 and 3, and the `exit 1` fact reaches no assertion. `expect_err` does not separate `ScriptFailure::Exit` from `ScriptFailure::Start`, and `command_failure_detail` (crates/swissarmyhammer-common/src/command.rs:66-73) answers the status only when stderr is empty — this script always writes to stderr, so the status number never reaches the text the test reads. A script changed to exit 2 keeps the test green.
    - guardrail: this is the SECOND round of a finding on the same coverage claim, at a different line with a different missing fact. Not yet the third.
    - next: iteration 3 works the open finding
  timestamp: 2026-08-15T21:46:11.447952+00:00
- actor: claude-code
  id: 01m03q1mcd39mqq67e1tdekbtw
  text: |-
    Picked up for the open finding of the 2026-08-15 16:40 round. The fix chosen is the one that makes the CLAIM true — the test now reads the exit status NUMBER — rather than the one that weakens the prose. The finding named both options and the round before it established that weakening leaves a table row uncovered.

    WHY THE NUMBER REACHED NO ASSERTION, read before the edit:

    - `read_script_output` (`crates/swissarmyhammer-validators/src/review/tool_rules.rs`) answers one `ScriptFailure::Exit` for EVERY nonzero status, carrying `command_failure_detail`.
    - `command_failure_detail` (`crates/swissarmyhammer-common/src/command.rs`) answers `format!("exited with {}", output.status)` ONLY when stderr is empty, and otherwise answers the stderr alone. This script always writes its `dead-code-typescript: ts-prune exited 1 for ...` line to stderr, so the error string is the stderr and carries no status.
    - So no text the engine hands on can hold a script to exit 1 rather than exit 2. The number has to come off the `Output` itself.

    WHAT WAS DONE:

    1. `ShippedScriptRun` (`tests/shipped.rs`) gains `status: Option<i32>` — `output.status.code()` for a run that happened, `None` for a shell that would not start. `drive_shipped_script_whole` fills it off the SAME `Output` it already reads `outcome` and `placed` off, so no second run and no third interpretation of the contract.
    2. `verify_shipped_tree_breaks` holds a breaking run to `Some(BROKEN_RUN_EXIT_STATUS)`, a named constant of 1, beside the two assertions it already made. `assert_eq!` on `Option<i32>` also separates `ScriptFailure::Exit` from `ScriptFailure::Start`, which `expect_err` did not.
    3. No existing assertion was weakened. `verify_shipped_tree_breaks_without_run_of` was NOT changed: its probes break with a stubbed `PATH`, its doc claims the error alone, and no rule prose states a status for it.

    REQUIREMENT 4, THE SHARED HELPER, MEASURED: `verify_shipped_tree_breaks` has 10 call sites over 5 shipped rules — `unused-dependencies-rust` (1), `missing-docs-rust` (4), `dead-code-typescript` (2), `stuttering-name-go` (1), `dead-code-rust` (2). Every one of the 10 is now held to the same number and every one passes: `cargo nextest run -p swissarmyhammer-validators -E 'test(shipped::)'` ran 175 tests, 175 passed. So `exit 1` is the status EVERY shipped script answers for a broken run, and the rule body says the helper holds them all.

    `builtin/validators/README.md` asks for "exit nonzero", not for 1, so the constant is documented as what the shipped set measures rather than as a contract the README states.
  timestamp: 2026-08-15T22:00:29.325721+00:00
- actor: claude-code
  id: 01m03q27wp42jmy4bknd8xb48v
  text: |-
    RED WATCHED FIRST, induced the way the last round induced it.

    The new assertion is about behaviour that already holds, so the shipped script's break branch was temporarily changed from `exit 1` to `exit 2` and the test run against it. `the_shipped_typescript_dead_code_tool_rule_breaks_on_a_project_ts_prune_cannot_read` FAILED with:

        assertion `left == right` failed: a run that breaks must exit 1: ts-prune judged no
        export of a project whose tsconfig it cannot read, so the run breaks and names that
        project rather than answering a clean workspace
          left: Some(2)
         right: Some(1)

    That is exactly the failure the finding names. The two OLD assertions — the named project and the 0 findings — both PASSED on the sabotaged script, so the new one is the only thing between `exit 2` and a green test.

    The sabotage was reverted. `git diff builtin/` over the reverted state answered 0 lines, so the `run:` block is byte-identical to HEAD, and the only `.md` hunk of this change is the prose paragraph at 1215-1228.

    REQUIREMENT 2, THE WHOLE FILE: every other coverage claim in `dead-code-typescript.md` was checked BY HAND against what its named test asserts. Every one HOLDS, and none overshoots:

    - line 762 `..._names_every_export_of_a_module_nothing_imports` "holds the probe above" — `TYPESCRIPT_ORPHAN_MODULE_PROBE` expects `src/orphan.ts:2`, `:5` and `:10`, one row for each of the three exports the probe's orphan module holds. HOLDS.
    - line 846 `..._keeps_its_own_gate_beside_a_project_config` "holds row 2" — `TYPESCRIPT_PROJECT_CONFIG_PROBE` stages a `package.json` carrying `"ts-prune": {"ignore":"src","skip":"src"}` and expects `src/lib.ts:2`, one finding. Row 2 of that table is 1 finding. HOLDS.
    - line 878 `..._reads_the_program_the_project_states` "holds both rows of the table" — the test drives `TYPESCRIPT_TESTS_IN_PROGRAM_PROBE`, expecting `src/lib.ts:5` alone (row 1, "the export nothing imports"), and `TYPESCRIPT_TESTS_OUT_OF_PROGRAM_PROBE`, expecting `src/lib.ts:2` and `:5` (row 2, "both exports"). HOLDS.
    - line 1079 "Five acceptance tests hold this section" — five are named, and each exists and stages the tree the sentence names. The claim that `..._names_a_module_outside_the_project_directory` "drives it through the engine as well" holds: the test calls `verify_shipped_staged_positions_report`, which runs `plan_tool_rules` and reads the planned run.
    - line 1123 `..._says_the_manifest_it_could_not_read_out_loud` "holds both halves" — `TYPESCRIPT_BROKEN_MANIFEST_PROBE` expects `packages/app/src/lib.ts:2` and `packages/other/src/index.ts:2`, so the whole package's entry is spared and the broken package's entry reports, and the test also holds the run to exactly one diagnostic naming the manifest. HOLDS.
    - line 1251 "The thirteen acceptance tests" — 13 `#[test]` functions in `dead_code_typescript.rs`. HOLDS.
    - line 1265 "The five acceptance tests named in 'How the run is shaped'" — the same five as line 1079. HOLDS.

    The status tables at lines 1145-1156 and 1178-1182 are MEASUREMENTS rather than coverage claims, and the `exit 1` cell of rows 2 and 3 is now the one an acceptance test holds as well.
  timestamp: 2026-08-15T22:00:49.302070+00:00
- actor: claude-code
  id: 01m03q2ft2gtg1jm1rrsy72m00
  text: |-
    ### implement — changed
    - evidence: 2 files — builtin/validators/code-hygiene/rules/dead-code-typescript.md (prose alone, the `run:` block byte-identical to HEAD), crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs. `cargo fmt` clean, no diff after it. `cargo clippy --workspace --all-targets -- -D warnings` exit 0, 0 warnings. `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` 3520 tests run, 3520 passed, 0 failed, 0 skipped. `cargo nextest run -p swissarmyhammer-validators -E 'test(shipped::)'` 175 run, 175 passed — all 10 breaking probes of the 5 shipped rules held to the new status assertion.
    - finding: the one open finding is answered and checked. The claim is TRUE rather than weakened: the test reads the exit status NUMBER off the run itself, and a script changed to exit 2 goes RED (watched).
    - next: /review
  timestamp: 2026-08-15T22:00:57.410531+00:00
- actor: claude-code
  id: 01m03qe77z9zppgbef2n1t9kmh
  text: |-
    ### test — green
    - `cargo fmt --check` — exit 0, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
    - `cargo nextest run --workspace` — 14240 tests run, 14240 passed (28 slow), 0 failed, 0 skipped. Exit 0. Duration 231.105s.
    - Targeted re-check of the changed area: `cargo nextest run --package swissarmyhammer-validators -E 'test(the_shipped)'` — 158 tests run, 158 passed, 0 failed. Confirmed `the_shipped_typescript_dead_code_tool_rule_breaks_on_a_project_ts_prune_cannot_read` (uses the new `status: Option<i32>` field and `BROKEN_RUN_EXIT_STATUS` assertion in `verify_shipped_tree_breaks`) passed, along with the other 4 shipped rules that call the shared helper.
    - No files changed by this step. Task left in `doing`. No commit made.
    - next: hand off to review.
  timestamp: 2026-08-15T22:07:21.855293+00:00
- actor: claude-code
  id: 01m03qtjzhfmt2c5qszanasatz
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (961943cae) — engine 0 findings over 1 file, 9 validators attempted, 2 `.kanban/` files excluded. Hand check of the changed prose: 1 finding — `builtin/validators/code-hygiene/rules/dead-code-typescript.md:1225`, same cause at `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1228-1230`.
    - next: the exit-1 claim of the round before now HOLDS; the new claim "every breaking probe of every shipped rule" overshoots — the helper carries 10 probes of 5 rules, while `verify_shipped_tree_breaks_without_run_of` (6 sites) and `function_length_rust.rs:449,515,589,650` hold no status. Narrow the words, or hold those probes to `BROKEN_RUN_EXIT_STATUS` too. Task stays in `review`.
  timestamp: 2026-08-15T22:14:07.089115+00:00
- actor: claude-code
  id: 01m03qw9jmz1k8g3znc17rs6jf
  text: |
    ### finish iteration 3 — findings
    - implement: changed — 2 files (builtin/validators/code-hygiene/rules/dead-code-typescript.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs). RED watched first with a sabotaged `exit 2`; the two older assertions PASSED on the sabotaged script, so the new status assertion is the only gate.
    - test: green — cargo fmt --check exit 0; cargo clippy --workspace --all-targets -- -D warnings exit 0, 0 warnings; cargo nextest run --workspace 14240 passed, 0 failed, 0 skipped
    - commit: 961943cae test(validators): tie the exit-1 fact to an assertion in shipped tests (^gxncs25)
    - review: findings — 1 finding, builtin/validators/code-hygiene/rules/dead-code-typescript.md:1225 and the same claim at crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1228-1230. The engine answered 0 findings over 1 file with 9 validators attempted.
    - detail: iteration 2's finding IS answered and holds. `ShippedScriptRun` carries `status: Option<i32>` off the same `Output`, and `verify_shipped_tree_breaks` asserts `Some(BROKEN_RUN_EXIT_STATUS)`. The new sentence overshoots: it states the helper holds "every breaking probe of every shipped rule" to the one number. The helper has 10 call sites over 5 rules. TEN OTHER breaking probes reach no status assertion — `verify_shipped_tree_breaks_without_run_of` carries six sites and asserts the name alone, and `function_length_rust.rs:449,515,589,650` call `shipped_script_findings(..).expect_err(..)` straight. `function-length-rust` changed to exit 2 keeps those four tests green.

    ### Guardrail state — read before iteration 4

    Three rounds, three findings, and they are NOT the same finding:

    1. iteration 1 — `dead-code-typescript.md:1216`, the claim "holds rows 2 and 3" while the test never read the `0 findings` half. ANSWERED, holds.
    2. iteration 2 — `dead-code-typescript.md:1217`, the claim "all three facts of each" while the `exit 1` fact reached no assertion. ANSWERED, holds.
    3. iteration 3 — `dead-code-typescript.md:1225`, the claim "every breaking probe of every shipped rule" while ten breaking probes reach no status assertion.

    Each round answers the round before it, and each round writes a NEW prose claim that reaches further than the assertions. The guardrail asks for the same file:line and the same message three times. That did not occur, so the loop continues.

    The CAUSE is one thing: prose written wider than the code measured. Iteration 4 must measure the reach FIRST and then write the sentence to that measurement, rather than write the sentence and then look.
    - next: iteration 4 works the open finding
  timestamp: 2026-08-15T22:15:02.996154+00:00
- actor: claude-code
  id: 01m03rrxf1r2fxsf7mvrppr3qh
  text: |-
    Picked up for the open finding of the 2026-08-15 17:08 round. The guardrail entry says the cause is one thing: prose written wider than the code measured. So this round MEASURED FIRST and wrote the sentence last.

    THE MEASUREMENT, taken before a single word was written.

    `read_script_output` was temporarily instrumented with one `eprintln!` of `output.status.code()` on the nonzero branch — the one function EVERY breaking probe's run passes through, whatever helper drives it. Then `cargo nextest run -p swissarmyhammer-validators -E 'test(/shipped/) and test(/break/)' --no-capture`.

    RESULT: 36 breaking tests, 37 broken script runs, and **all 37 answered exit status 1**. `grep -o 'SAHMEASURE status=...' | sort | uniq -c` answers one line: `37 SAHMEASURE status=Some(1)`. The instrumentation was reverted.

    That includes the stubbed-`PATH` probes, which the round before guessed might answer another status. They do not. The stub exits 127, and the SHIPPED script reads the status of its own step and states the broken run in its own words at exit 1. So the 127 never reaches the script's own status.

    THE ROSTER, counted mechanically rather than by eye. The finding named 20 sites. There are 37, because it did not know about a THIRD breaking family:

    | the path | sites | rules | status held before | now |
    |---|---|---|---|---|
    | `verify_shipped_tree_breaks` | 10 | 5 | YES | YES |
    | `verify_shipped_tree_breaks_without_run_of` | 6 | 5 | no | YES |
    | `verify_rust_function_length_breaks` (was 4 straight `shipped_script_findings(..).expect_err(..)`) | 4 | 1 | no | YES |
    | `verify_shipped_run_breaks` | 17 | 10 | no | no — see below |

    10 + 6 + 4 + 17 = 37 sites across 36 tests, which is exactly the 37 runs measured. 13 shipped rules carry a breaking probe.

    WHAT WAS DONE. The card's preferred branch — every probe exits 1, so extend the assertion — was taken for the two families the finding named:

    1. `assert_shipped_break` is new, and holds a broken run to the named fragments AND `Some(BROKEN_RUN_EXIT_STATUS)`. `verify_shipped_tree_breaks` now calls it beside its own `placed.is_empty()`, so NO existing assertion was weakened.
    2. `verify_shipped_tree_breaks_without_run_of` now drives `drive_shipped_staged_tree_whole` rather than `drive_shipped_staged_tree_with`, and calls `assert_shipped_break`. Six sites gain the number.
    3. `verify_rust_function_length_breaks` is new in `function_length_rust.rs`. The four sites each carried the SAME copied 8-line block — `shipped_script_findings(..).expect_err(..)`, `failure.to_string()`, `assert!(detail.contains(..))` — and each now calls the one helper, which reads the status off the run's own `Output`. That removes the copy as well as the gap.

    WHY THE 17 ARE NOT HELD, stated rather than papered over. `verify_shipped_run_breaks` drives `execute_tool_runs` — the ENGINE — and not the script. The engine answers a `ToolRunError` that carries the detail string alone and keeps no status at all; that is its documented contract and the whole reason `BROKEN_RUN_EXIT_STATUS` exists. Reading the number there needs either a second run of the same probe (which the iteration-2 round rejected: both halves must come off ONE output, and one of these probes takes 22 s) or a new status field on the engine's public `ToolOutcome`, which no card authorizes and which contradicts the contract the engine states. So the 17 keep the error text, and BOTH the rule prose and the `BROKEN_RUN_EXIT_STATUS` doc now say so in those words, with the counts.
  timestamp: 2026-08-15T22:30:40.865162+00:00
- actor: claude-code
  id: 01m03rsmaavb19ea4eckzp4ht5
  text: |-
    RED WATCHED FIRST, and this round the gap was reproduced BEFORE the assertion was written rather than after.

    STEP 1 — the defect, shown. The five `exit 1` statements in the `run:` block of `function-length-rust.md` were changed to `exit 2` (the file was copied aside first). Against the tests AS THEY STOOD:

        Summary [1.480s] 6 tests run: 6 passed, 757 skipped

    All six breaking tests of `function-length-rust` stayed GREEN on a script that exits 2. That is the finding's own sentence, reproduced: "`function-length-rust` changed to exit 2 on a broken build script keeps all four of those tests green."

    STEP 2 — the assertions added, sabotage still in place. Same command:

        Summary [1.488s] 6 tests run: 1 passed, 5 failed, 757 skipped

    with each failure reading:

        thread '...breaks_on_a_member_build_script_that_breaks' panicked at
        crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1235:5:
        assertion `left == right` failed: a run that breaks must exit 1: a member build
        script that breaks must break the run
          left: Some(2)
         right: Some(1)

    Five went RED for exactly the right reason. The five are the 4 straight sites plus `..._breaks_when_the_filter_cannot_read_the_report`, which is the `verify_shipped_tree_breaks_without` family.

    The SIXTH, `..._breaks_on_a_workspace_it_cannot_compile`, still PASSED on the sabotaged script. It is a `verify_shipped_run_breaks` probe, and it is the boundary of the reach — so the RED run itself DEMONSTRATES the limit the new prose states, rather than the prose asserting it on trust.

    STEP 3 — reverted. `git diff builtin/` answers 0 lines over the reverted state, so the `run:` block is byte-identical to HEAD. Re-run: 6 tests, 6 passed.

    The only `.md` hunk of this change is `@@ -1222,8 +1222,20 @@` in `dead-code-typescript.md` — prose alone, no `run:` byte.

    REQUIREMENT 4, THE WHOLE FILE. Every OTHER coverage claim in `dead-code-typescript.md` was re-checked against what its named test asserts, and each HOLDS. Nothing in `dead_code_typescript.rs` changed this round, and the one shared helper it uses, `verify_shipped_tree_breaks`, kept all three of its assertions.

    - "thirteen acceptance tests" (twice: the fixture-pair section and the corpus section) — `grep -c '^#\[test\]'` answers 13.
    - Every test the body names by its elided `..._` suffix EXISTS, checked one by one: `names_every_export_of_a_module_nothing_imports`, `keeps_its_own_gate_beside_a_project_config`, `reads_the_program_the_project_states`, `names_a_module_outside_the_project_directory`, `names_no_file_that_is_not_the_file_of_the_finding`, `says_the_finding_it_declines_out_loud`, `places_a_file_the_two_readings_spell_differently`, `says_the_file_outside_the_workspace_out_loud`, `says_the_manifest_it_could_not_read_out_loud`, `breaks_on_a_project_ts_prune_cannot_read` — 10 named, each one match.
    - "Five acceptance tests hold this section" and "the five acceptance tests named in 'How the run is shaped'" name the same five, and all five exist.
    - "holds rows 2 and 3 of the table above, all three facts of each" still HOLDS: `verify_shipped_tree_breaks` still asserts the named project, `Some(BROKEN_RUN_EXIT_STATUS)` and `placed.is_empty()`.
    - The status tables and the three-tree table are MEASUREMENTS of the shipped script, and the script did not change, so they stand from the first round.

    THE NEW SENTENCE, and every number in it verified mechanically with `rg -c` rather than by reading: 10 + 6 + 4 = 20 held over 7 rules, 17 not held over 10 rules, 37 total over 13 rules and 36 tests. The 37 matches the 37 measured runs exactly, so the roster is closed — no breaking probe is unaccounted for.
  timestamp: 2026-08-15T22:31:04.266297+00:00
- actor: claude-code
  id: 01m03rt1b994k384170nabsdaf
  text: |-
    ### implement — changed
    - evidence: 3 files — `builtin/validators/code-hygiene/rules/dead-code-typescript.md` (prose alone; `git diff builtin/` carries one hunk, `@@ -1222,8 +1222,20 @@`, and the `run:` block is byte-identical to HEAD), `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_rust.rs`.
    - measurement (taken FIRST, before any sentence): `read_script_output` instrumented with one `eprintln!` of `output.status.code()`, then `cargo nextest run -p swissarmyhammer-validators -E 'test(/shipped/) and test(/break/)' --no-capture` — 36 tests, 37 broken runs, `37 SAHMEASURE status=Some(1)`, every one exit 1. Instrumentation reverted. Roster counted with `rg -c`: 10 `verify_shipped_tree_breaks` + 6 `verify_shipped_tree_breaks_without_run_of` + 4 straight `expect_err` + 17 `verify_shipped_run_breaks` = 37 sites, closing exactly on the 37 measured runs.
    - RED: sabotaged the five `exit 1` of `function-length-rust.md` to `exit 2`. Before the change 6 tests passed; after it 1 passed and 5 FAILED on `assertion left == right ... left: Some(2) right: Some(1)`. Sabotage reverted, `git diff builtin/` 0 lines, 6 tests pass.
    - finding: the one open finding is answered. The claim is now written TO the measurement: 20 breaking probes over 7 rules are held to the number, and the 17 that drive `execute_tool_runs` are held to the error text alone, because the engine keeps no status. Both the rule prose and the `BROKEN_RUN_EXIT_STATUS` doc state the same counts and the same reason. No existing assertion was weakened.
    - tests: `cargo fmt --check` exit 0. `cargo clippy --workspace --all-targets -- -D warnings` exit 0, 0 warnings. `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3520 tests run, 3520 passed, 0 failed, 0 skipped, in 115.3s.
    - next: /review
  timestamp: 2026-08-15T22:31:17.609005+00:00
- actor: claude-code
  id: 01m03tb10kfz3b9qm2w94tr0fg
  text: |-
    ### test — green

    - Command: `cargo fmt --check`. Result: exit 0. No diff.
    - Command: `cargo clippy --workspace --all-targets -- -D warnings`. Result: exit 0. No warnings.
    - Command: `cargo nextest run --workspace`. Result: 14240 tests run. 14240 passed. 0 failed. 0 skipped.
    - Command: `cargo nextest run -p swissarmyhammer-validators -E 'test(/function_length_rust/)'`. Result: 14 tests run. 14 passed. This confirms all four sites in `tests/shipped/function_length_rust.rs` pass through the shared helper.
    - Command: `cargo nextest run -p swissarmyhammer-validators -E 'test(/shipped/)'`. Result: 177 tests run. 177 passed.
    - Check: `crates/swissarmyhammer-validators/src/review/tool_rules.rs` has no `eprintln!`. The old debug print is gone.
    - Check: no `SAHMEASURE` string in any source file. It appears only in `.shell/log` and in the two kanban task record files. These are log and history files, not code.
    - Check: `git diff HEAD -- builtin/validators/code-hygiene/rules/dead-code-typescript.md` shows the change starts after the file's last code fence (line 1205). The `run:` block is untouched.

    next: hand off for review. Task stays in `doing`. No commit made.
  timestamp: 2026-08-15T22:58:02.899234+00:00
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

## Review Findings (2026-08-15 16:40)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. The engine reviewed 3 files, attempted 9 validators and answered 0 findings (2 `.kanban/` files excluded by `.reviewignore`). The rule body is prose the engine reads no byte of, so every claim the commit added or changed was checked BY HAND against what the named tests actually assert.

- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:1217` `hand/tool-rule-contract` — the body now states the test "holds rows 2 and 3 of the table above, all three facts of each", and the test holds two of the three facts, not three. Each row's "the shipped script" cell states `0 findings`, `exit 1`, and `the project named`. `0 findings` is now held — `verify_shipped_tree_breaks` (`crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1213`) asserts `placed.is_empty()` off the stdout of the same run. `the project named` is held by `assert_shipped_failure_names`. `exit 1` is NOT held: the test reads `outcome.expect_err(probe.reason)`, which separates only success from failure and does not even separate `ScriptFailure::Exit` from `ScriptFailure::Start`, and the status number reaches no assertion, because `command_failure_detail` (`crates/swissarmyhammer-common/src/command.rs:66-73`) answers `output.status` ONLY when stderr is empty, while this run always writes its `dead-code-typescript: ts-prune exited 1 for ...` line to stderr, so the error string is the stderr alone and carries no status. `read_script_output` (`crates/swissarmyhammer-validators/src/review/tool_rules.rs:918-921`) collapses every nonzero status into one `ScriptFailure::Exit`, so no probe driven through it can hold the script to exit 1 rather than exit 2 — a script changed to exit 2 keeps this test green and falsifies both rows. Either word the second fact the way the very next sentence already words it — the nonzero exit, the named project and the 0 findings — and state that the exit-status NUMBER is a measurement the status table records rather than something an acceptance test holds, or read the number with a path that keeps `output.status`.

### What was checked BY HAND and HOLDS

- `run_script` (`tool_rules.rs:903-910`) truly delegates to `read_script_output` (`:918-933`); no reading logic is left duplicated. The only edit inside the moved body is `&output` → `output`, so the branches and the messages are the ones HEAD carried.
- "The engine answers a nonzero exit before it reads stdout at all" holds: `tool_rules.rs:919-921` returns before the stdout read at `:926`.
- "the helper the test calls drives the run itself and reads both halves off one output" holds: `drive_shipped_staged_tree_whole` (`shipped.rs:1146-1164`) calls `drive_shipped_script_whole`, which calls `run_shell` ONCE and derives `placed` (`placed_outcome`/`finding_rows`) and `outcome` (`read_script_output`) off that one `Output`. No second run and no re-derived value.
- "holds rows 2 and 3" holds: `dead_code_typescript.rs:1081-1084` drives `TYPESCRIPT_BROKEN_TSCONFIG_PROBE` (row 2) and `TYPESCRIPT_BROKEN_PROJECT_BESIDE_A_WHOLE_ONE_PROBE` (row 3).
- "Row 3 is where the second half carries the weight" holds: that probe stages `packages/app` with a dead export beside the broken `packages/other` (`dead_code_typescript.rs:769-774`), so a written app row makes `placed` non-empty and fires the new assertion.
- `finding_rows` (`shipped.rs:790-802`) filters nothing, so `placed.is_empty()` is genuinely zero rows on stdout.
- The `shipped.rs:1195-1205` doc — "and to placing no finding", and "[`drive_shipped_staged_tree_whole`] keeps the stdout a broken run wrote, which [`run_script`] answers `Err` before reading" — states what the code does.
- `placed_outcome` doc "stated as a panic rather than counted as no row" matches its `unwrap_or_else(|error| panic!(..))`.
- `drive_shipped_script_whole` doc "arguments come from [`script_args`] and the scope from the SHIPPED rule" matches `script_args(shipped.scope, files)`, and the rewrite of `drive_shipped_staged_tree_with` is equivalent to the `shipped_script_findings` call it replaced.
- `drive_shipped_staged_tree_whole` doc "The work-list names the probe's own files and never `extra`" holds: `paths` is built from `probe.staged` alone.
- `dead_code_typescript.rs:840-841` "the 16 `tsconfig.json` projects of the four corpus workspaces — 9, 2, 3 and 2, which is the count the rule body's corpus table states" holds: the corpus table at `dead-code-typescript.md:443-448` states 9, 2, 3 and 2, which sum to 16, and the body already reads 16 at `:876-877`. The `12` this commit replaced was false.
- "thirteen acceptance tests" still holds: 13 `#[test]` functions in `dead_code_typescript.rs`.
- The `run:` block is untouched. The only `.md` hunk is `@@ -1214,7 +1214,13 @@`, prose alone, so the probe-tree measurements and the 58-findings workspace reading of the round before stand unchanged and were not re-measured.

## Review Findings (2026-08-15 17:08)

> Scope: `review sha HEAD~1..HEAD` — commit 961943cae alone — reviewed the diffs only — lines this change added or modified. The engine reviewed 1 file, attempted 9 validators and answered 0 findings (2 `.kanban/` files excluded by `.reviewignore`). The rule body is prose the engine reads no byte of, so every claim the commit added or changed was checked BY HAND against what the named tests actually assert.

- [x] `builtin/validators/code-hygiene/rules/dead-code-typescript.md:1225` `hand/tool-rule-contract` — the body states "That helper holds every breaking probe of every shipped rule to the same number", and the helper holds ten breaking probes of five shipped rules, not every one. `verify_shipped_tree_breaks` (`crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1237`) is called from `dead_code_typescript.rs:1082-1083`, `dead_code_rust.rs:164,231`, `missing_docs_rust.rs:402,435,511,621`, `unused_dependencies.rs:171` and `stuttering_name_go.rs:251` — 10 sites, 5 rules. Ten other breaking probes reach no status assertion at all. `verify_shipped_tree_breaks_without_run_of` (`shipped.rs:1385-1412`) reads `drive_shipped_staged_tree_with(..).expect_err(probe.reason)` and asserts `assert_shipped_failure_names` alone — no status and no `placed` — and carries `function_length_rust.rs:849`, `unused_dependencies.rs:223`, `dead_code_rust.rs:356`, `missing_docs_rust.rs:712` and `missing_docs.rs:542,568`. `function_length_rust.rs:449,515,589,650` call `shipped_script_findings(..).expect_err(..)` straight and assert only that the detail carries `RUST_BUILD_SCRIPT_BROKEN_LINE`. The stated consequence is therefore false where it is checkable: `function-length-rust` changed to exit 2 on a broken build script keeps all four of those tests green, because `run_script` answers the same `ScriptFailure::Exit` for every nonzero status and the detail — the unchanged stderr — carries no number. The same words carry the same claim at `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1228-1230`: "Every breaking probe of every shipped rule is held to the one number, so a script changed to exit 2 breaks its own rule's test." Either state the reach the helper has — every breaking probe that CALLS `verify_shipped_tree_breaks`, ten of them across five rules — or hold the remaining breaking probes to `BROKEN_RUN_EXIT_STATUS` as well, at which point the words stand as written.

### What was checked BY HAND and HOLDS

- The `run:` block is byte-identical to HEAD~1. The `.md` file carries exactly one hunk, `@@ -1214,13 +1214,18 @@`, prose alone (12 insertions, 7 deletions), so every measurement of the two rounds before stands and none was re-measured.
- "all three facts of each" now holds. Rows 2 and 3 of the table at `dead-code-typescript.md:1181-1182` each state `0 findings`, `exit 1` and the project named. `verify_shipped_tree_breaks` (`shipped.rs:1237-1256`) now asserts all three: `assert_shipped_failure_names(&failure, probe.run.expected)`, `assert_eq!(status, Some(BROKEN_RUN_EXIT_STATUS))` and `assert!(placed.is_empty())`.
- "It reads the named project off the error the engine answers for the run, and the exit status and the 0 findings off that same run itself" holds. `drive_shipped_script_whole` (`shipped.rs:967-983`) calls `run_shell` ONCE and derives all three off that one `Output`: `outcome` from `read_script_output(&output)`, `placed` from `finding_rows(&placed_outcome(&output), ..)`, and `status` from `output.status.code()`.
- "The status is held as the NUMBER 1 rather than as \"nonzero\", so a script changed to exit 2 fails the test" holds for THIS test: `BROKEN_RUN_EXIT_STATUS` is `1` and the assertion is `assert_eq!`, not a nonzero test.
- "The error carries no number to read: the engine answers the same failure for every nonzero status, and the text it hands on is the script's own stderr for a run that wrote any" holds. `read_script_output` (`tool_rules.rs:940-943`) answers `Err(ScriptFailure::Exit(command_failure_detail(output)))` for every `!status.success()`, and `command_failure_detail` (`swissarmyhammer-common/src/command.rs:90-97`) answers the trimmed stderr whenever it is non-empty and `exited with {status}` only when it is empty. This run always writes its `dead-code-typescript: ts-prune exited 1 for ...` line, so the detail is the stderr alone.
- "It answers a nonzero exit before it reads stdout at all as well" holds: `tool_rules.rs:941-943` returns before the stdout read at `:948`.
- "Row 3 is where the 0 findings carries the weight" holds unchanged from the round before: the row-3 probe stages a readable `packages/app` holding a dead export beside the broken `packages/other`.
- The `status` field doc (`shipped.rs:936-941`) states what the code does: `output.status.code()` answers `None` for a signal, and the `Err` arm of the `run_shell` match (`shipped.rs:971`) writes `None` for a shell that never started the script.
- "every shipped script writes `exit 1` for that" (`shipped.rs:1209`) holds: every `exit <digit>` statement in every `run:` block under `builtin/validators` is `exit 0` or `exit 1`. The four `exit 2` strings in those files stand in prose and in status tables that record what a TOOL answered, never in a script.
- "it is what no caller of [`run_script`] can read" (`shipped.rs:1211-1213`) holds: `run_script` answers `Result<ScriptOutcome, ScriptFailure>`, and `ScriptFailure::Exit` carries a `String` alone.
- The `.kanban/` half of the diff is the review record of the round before, excluded by `.reviewignore`.