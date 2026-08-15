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
position_column: doing
position_ordinal: '8280'
title: dead-code-typescript answers zero findings when ts-prune crashes
---
`builtin/validators/code-hygiene/rules/dead-code-typescript.md` ends its per-project pipe in `sed` and the loop in `sort`, so the exit status of `ts-prune` is thrown away.

`ts-prune` 0.10.3 crashes with an unhandled error when it cannot read a `tsconfig.json`. Measured on a probe holding one dead export beside a `tsconfig.json` of bytes that are not JSON: `@ts-morph/common` throws, the stack goes to stderr, and the shipped script reports 0 findings and exits 0. The engine reads exit 0 as "the tool judged the code", so a project with a broken tsconfig reads as a clean workspace.

`builtin/validators/README.md` names this trap word for word: "Write a pipe only where the tool cannot exit nonzero. Otherwise write a script: run the tool into a file, test the status, and exit nonzero yourself."

The fix is the shape `complexity-swift` and `missing-docs-python` already carry: run `ts-prune` into a file, read its status, and exit nonzero with a line on stderr for the statuses that mean a broken run. Measure which statuses `ts-prune` answers with for a clean project, for a project holding findings, and for each broken shape, and state the table in the rule body. Ship an acceptance test beside the five in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs`.

Found while implementing ^108bh4y. #tool-validators #objectivity