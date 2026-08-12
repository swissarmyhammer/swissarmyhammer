---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzq24rvdzm60sje3qgenagey
  text: |-
    From ^xd5r1zh (`magic-numbers-swift`): that card was asked to decide whether to fix the discarded-`.swiftlint.yml` defect for its own rule or leave it with this card. It LEFT it here, so this card now owns the fix for all THREE Swift rules. Fix them together.

    The three sites, each writing its own config to a temporary path and passing `--config`, and each therefore dropping the project's `excluded:` list for generated Swift:

    - `builtin/validators/code-hygiene/rules/missing-docs-swift.md` — `only_rules: [missing_docs]`.
    - `builtin/validators/code-hygiene/rules/complexity-swift.md` — `only_rules: [cyclomatic_complexity, function_body_length]` plus thresholds.
    - `builtin/validators/code-hygiene/rules/magic-numbers-swift.md` — `only_rules: [no_magic_numbers]` plus `allowed_numbers`.

    The reason ^xd5r1zh gave for leaving it: the defect is not a defect of any one swiftlint rule, it is a defect of the shape all three share. One answer must serve all three, or the three rules end up with three different config-build shapes.

    Two things to measure before the fix, neither of which ^xd5r1zh did:

    1. Whether swiftlint merges a parent and a child config (`parent_config` / `child_config`), and what a merge does to `only_rules` — a merge that lets the project's rule set back in breaks the "the rule owns its whole invocation" contract each of the three rule bodies states.
    2. Whether an `excluded:` list can be read out of the project's `.swiftlint.yml` and copied into the temporary config alone, without the rule set beside it.

    Each of the three rule bodies states "never reads the project's own `.swiftlint.yml`" today. Whichever answer wins, all three sentences change together.
  timestamp: 2026-08-11T00:04:18.925957+00:00
- actor: claude-code
  id: 01kzqvyr41q3gcz631a6b65gv6
  text: |-
    Measured with swiftlint 0.65.0, before any edit. Probe tree: `.swiftlint.yml` holding `excluded: [Generated]` and `only_rules: [todo]`, `Generated/gen.swift` and `Sources/plain.swift` each holding one undocumented `public struct` with two undocumented public members, plus `deep/nested/other.swift`.

    ## The card premise holds

    The shipped script over both files reports 6 findings — 3 in `Generated/gen.swift` and 3 in `Sources/plain.swift`. The project's `excluded:` list is discarded.

    ## The card's first fix is REFUTED as written

    "give the temp config an `excluded:` list of its own" does not work. An `excluded:` list has NO effect on a file named as an explicit command-line argument.

    | config | invocation | Generated file |
    |---|---|---|
    | temp config, `excluded: [Generated]` | explicit file paths | reported |
    | config in the repo root, `excluded: [Generated]` | explicit file paths | reported |
    | temp config, `excluded: [<absolute>/Generated]` | explicit file paths | reported |
    | config in the repo root, `excluded: [Generated]` | no path argument | silent |
    | config in the repo root, `excluded: [Generated]` | the directory `.` | silent |

    `--force-exclude` is the flag that makes `excluded:` apply to an explicit path. Its help states it word for word: "Exclude files in config `excluded` even if their paths are explicitly specified."

    With `--force-exclude`, an `excluded:` entry resolves relative to the directory of the config file that states it. A relative `Generated` in a temp config resolves under `/var/folders/...` and matches nothing (6 findings). An absolute path in a temp config works (3 findings). A relative path in a config at the repo root works (3 findings).

    ## The card's second fix WORKS

    swiftlint takes `--config` repeatedly and evaluates the list as a parent-child hierarchy. Measured with the project's `.swiftlint.yml` as the parent and the rule's temp config as the child, with `--force-exclude`:

    - The child's `only_rules` wins. A parent stating `only_rules: [todo]` reports missing_docs findings and no todo finding.
    - The child's `only_rules` beats a parent's `disabled_rules: [missing_docs]`. The rule still runs, 3 findings. A project cannot switch the rule off.
    - The parent's `excluded:` applies. The generated file goes silent.
    - The parent's `included:` does NOT apply to an explicit path. A file outside `included: [Sources]` still reports.
    - A nested `Nested/.swiftlint.yml` is not read. With `--config` swiftlint reads the named configs and no other.

    ## The rule OPTIONS leak, and one child key stops it

    A parent stating `missing_docs: excludes_inherited_types: false` leaks into the run when the child states no `missing_docs:` block: an undocumented `public struct Wide: Equatable` reports 2 findings. A child that states the block wins: the same run reports 0.

    The child's `missing_docs:` block REPLACES the parent's block. Measured with a parent stating `warning: [open]` and a child stating `excludes_inherited_types: true` alone: the `public struct` still reports, so the parent's `warning: [open]` did not survive.

    Hostile-parent run, with the child stating all five `missing_docs` options: parent `disabled_rules: [missing_docs]` + `missing_docs: {warning: [open], excludes_inherited_types: false}` + `excluded: [Generated]`, over `Sources/plain.swift`, `Sources/wide.swift` and `Generated/gen.swift` — the run reports `Sources/plain.swift:1` and `:2` and no other. The same with the parent stating `only_rules: [todo]` instead reports the same two.

    ## Every file excluded exits 1

    `--force-exclude` where every given file is excluded exits 1 with `Error: No lintable files found at paths: 'Generated/gen.swift'` on stderr and `[]` on stdout. A file that is not there exits 1 with the same message. The two are told apart by testing each file for readability before the run.

    ## The other traps, measured

    - A `--config` path that holds no file exits 134 with `Could not read configuration`. The script must test for the project config before it names it.
    - A file that is not there exits 1 with `No lintable files found`. The current pipeline hands `jq` an empty stdin, and a pipeline takes its LAST command's status, so the run exits 0 and reports nothing.
    - No path argument at all: swiftlint walks the whole tree. Over the probe tree it reports 8 findings across three files and exits 0.
    - A Swift file that does not parse is NOT a trap. `public func oops( {` still reports 2 missing_docs findings and exits 0. swiftlint recovers from the parse error and never reports the file as unread.
    - A config that is not YAML exits 134.

    ## Card claims 4 and 5, measured

    Claim 5 holds. A `Tests/support.swift` holding `public final class TestSupport` with `public func makeThing` reports both, and the `final class ThingTests: XCTestCase` with `func testThing()` beside them reports nothing.

    Claim 4 holds for the shape it names. `extension Shown: CustomStringConvertible { public var description }` reports nothing. A member of a plain `public extension Shown` reports.
  timestamp: 2026-08-11T07:35:24.545709+00:00
- actor: claude-code
  id: 01kzqxbphvg1ypqhdebdp3gdyk
  text: |-
    Implementation landed. One answer serves all three Swift rules, as the card asked.

    ## The answer

    Each of the three `run` scripts now names TWO configuration files. swiftlint reads a list of `--config` paths as a parent-child hierarchy.

    - The PARENT is the project's own `.swiftlint.yml` at the repository root, named only when the file is there. It gives the run the project's `excluded:` list.
    - The CHILD is the file the script writes into a temporary directory. It states the rule set and every option of every rule the script measures with.

    `--force-exclude` makes the `excluded:` list reach a file named on the command line.

    The card's first fix — "give the temp config an `excluded:` list of its own" — is refuted for a relative entry and works for an absolute one, and the absolute form would need the script to read the project's YAML itself. The measurements stand in the comment above and in the rule bodies.

    ## What each script now holds

    - `if [ "$#" -eq 0 ]; then exit 0; fi` — a run given no file reads no tree.
    - A readability test on every file it is given, exiting 1 with `<rule> cannot read <path>`.
    - `work="$(mktemp -d)"` with `trap 'rm -rf "$work"' EXIT`.
    - The tool's report written to a file rather than into a pipe, with the exit status read.
    - `grep -qF 'No lintable files found'` on the tool's stderr, which exits 0: the readability test above makes that message mean one thing only, that the exclude list took every file.

    ## What changed

    - `builtin/validators/code-hygiene/rules/missing-docs-swift.md` — script and body.
    - `builtin/validators/code-hygiene/rules/magic-numbers-swift.md` — script and body.
    - `builtin/validators/code-hygiene/rules/complexity-swift.md` — script and body.
    - `builtin/validators/code-hygiene/rules/missing-docs.md` — the inherited-type note said "the swiftlint `missing_docs` default"; the shipped script now WRITES the value, so the note names the setting and states both facts. The four probe files were re-measured against the new script and every row of that note is unchanged.
    - `builtin/validators/README.md` — the contract said the project's own configuration is never read. A new bullet states when a script may read it, for the FILE LIST alone, and requires the child to state every option of every rule it measures with.
    - 14 acceptance tests, in `shipped/missing_docs.rs`, `shipped/magic_numbers.rs` and `shipped/complexity.rs`. The shared Swift staging moved into `tests/shipped.rs` so the three modules hold one definition.
    - `ShippedStagedPositions` and `ShippedBrokenRun` now take a `prompt_rule`. Both shapes hard-coded `missing-docs`, which was wrong once a magic-numbers and a complexity probe used them.

    ## RED to GREEN

    Eight tests were RED before the scripts changed and are GREEN after: the exclude-list test, the every-file-excluded test and the cannot-read test of each of the three rules, and the empty-run test of each. The three "keeps its own options" tests could not be RED before the change, because a script that never names the project configuration cannot read an option out of it. Each was proved RED by removing the option block from the script it guards, and GREEN with it: `missing-docs-swift` 0 against 2 expected, `magic-numbers-swift` 0 against 1, `complexity-swift` 0 against 1.

    ## Discoveries the next agent should keep

    - `excluded:` does NOTHING for a file named as a command-line argument without `--force-exclude`. It applies only when swiftlint discovers the files itself, or is given a directory.
    - An `excluded:` entry resolves against the directory of the configuration file that states it, so a relative entry in a temporary configuration matches nothing.
    - The child's block for a rule REPLACES the parent's block whole. One key in the child is enough to reset the rest to swiftlint's defaults, so the child must state every option it relies on.
    - `only_rules` in the child beats `disabled_rules` in the parent. A project cannot switch a rule off.
    - `included:` in the parent does not reach an explicit path.
    - A nested `.swiftlint.yml` is never read while `--config` is passed.
    - A Swift file that does not parse is not a trap: swiftlint recovers, reports and exits 0.
    - A `--config` path that holds no file aborts swiftlint with exit 134.
  timestamp: 2026-08-11T07:59:57.499370+00:00
- actor: claude-code
  id: 01kzqxby460d4eqzpxjrztz5ej
  text: |-
    ### implement — changed
    - evidence: 10 files — `builtin/validators/README.md`, `builtin/validators/code-hygiene/rules/missing-docs.md`, `missing-docs-swift.md`, `magic-numbers-swift.md`, `complexity-swift.md`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs`, `tests/shipped.rs`, `tests/shipped/missing_docs.rs`, `tests/shipped/magic_numbers.rs`, `tests/shipped/complexity.rs`. `cargo nextest run -p swissarmyhammer-validators` — 611 tests run, 611 passed, 0 skipped. `cargo fmt --all -- --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. 14 new acceptance tests, 8 of them proved RED before the scripts changed and GREEN after, and 3 more proved RED by removing the option block each guards.
    - next: `/review`
  timestamp: 2026-08-11T08:00:05.254677+00:00
- actor: claude-code
  id: 01kzqy746p8b3mkpa8kb7hsgvy
  text: |-
    ### review — findings
    - scope: `review sha HEAD~1..HEAD` (09cf60b11), task mode
    - engine: 16 checks attempted, 0 confirmed, 2 refuted
    - measurement: swiftlint 0.65.0, hostile project `.swiftlint.yml`, 14 levers, 3 shipped scripts
    - evidence: builtin/validators/README.md:221, builtin/validators/code-hygiene/rules/complexity-swift.md:28, builtin/validators/code-hygiene/rules/complexity-swift.md:32, builtin/validators/code-hygiene/rules/magic-numbers-swift.md:26, builtin/validators/code-hygiene/rules/missing-docs-swift.md:219
    - trust boundary: a project cannot make the gate more weak. Only `excluded:` changes the counts, and that is the behaviour the card asks for. The child block replaces the parent block whole; a control run proves the probe.
    - next: correct the five findings. The card stays in `review`.
  timestamp: 2026-08-11T08:14:56.214714+00:00
- actor: claude-code
  id: 01kzqy86d9fban938x7xmddc43
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 10 files; the three swiftlint rules name the project config as parent and their own as child, with --force-exclude; a zero-argument guard, a readability test, a trap and a report file in each
    - test: green — cargo nextest run -p swissarmyhammer-validators, 611 passed; 14 new acceptance tests, 8 proved RED to GREEN; clippy -D warnings clean
    - commit: 09cf60b11 — 10 files
    - review: findings — 5 items. The trust boundary HOLDS: 14 hostile levers measured, and only `excluded:` moves a count, which is the wanted behaviour. builtin/validators/README.md:221, complexity-swift.md:28 and :32, magic-numbers-swift.md:26, missing-docs-swift.md:219
  timestamp: 2026-08-11T08:15:31.241649+00:00
- actor: claude-code
  id: 01kzqz7rc4xseh3zh8hypy46nw
  text: |-
    All five findings corrected. Every number below was measured with swiftlint 0.65.0 on this machine.

    ## Finding 1 — README:221

    The sentence stood inside the bullet for a tool with NO configuration flag, which builds a temporary package. `missing-docs-dart` is that shape: the script writes its own `analysis_options.yaml` into the package and copies the changed files in. It copies no file of the project's own. The sentence now states that limit: "The script writes the configuration of that tree itself, and it copies no configuration of the project's own into the tree." It no longer contradicts the bullet under it, which covers a tool that merges two configurations.

    ## Finding 3 — the serious one

    Two shapes of a project `.swiftlint.yml` abort swiftlint when it stands beside the rule's own configuration:

    | the project file | swiftlint |
    |---|---|
    | `child_config: other.yml` | exit 134, `There's an ambiguity in the child / parent configuration tree` |
    | bytes that are not YAML | exit 134, `Cannot parse YAML file` |

    Each abort writes `Could not read configuration` to stderr and leaves stdout empty. That one string tells both shapes from every other status, so it is the trigger.

    Options measured before the decision:

    - `parent_config: other.yml` in the project file is NOT a trap. swiftlint reads both configurations and exits 0. So the trigger must not key on a YAML key name.
    - A temporary configuration that names the project file through its own `parent_config:` aborts the same way. That is not an escape.
    - Writing the project's `excluded:` list into the temporary configuration is refuted for a relative entry (it resolves under `/var/folders` and matches nothing) and would need the script to parse the project YAML, and its child files, itself.

    DECISION: the script runs a SECOND time with its own configuration alone, and writes one line to stderr naming what it dropped. The gate still measures. The project's `excluded:` list is dropped for that run — which is the state before commit 09cf60b11, so no project loses ground.

    Measured over one file under `Generated/` beside `child_config: other.yml` and `excluded: [Generated]`: missing docs 2 findings, magic numbers 1 finding, complexity 1 finding. Each run exits 0.

    ## Finding 2 — the `error` options, and why they forced a script change

    Stating `error` is not free. Measured over a 150-line body and a 300-line body:

    | what the child states | 150 lines | 300 lines |
    |---|---|---|
    | `warning: 250` alone | 0 | 1, warning severity |
    | `warning: 250` + `error: 100` (swiftlint's default) | 1, error | 1, error |
    | `warning: 250` + `error: 250` | 0 | 1, error |
    | `warning: 250` + `error:` with no value | swiftlint answers `Invalid configuration ... Falling back to default.` and measures against `warning: 50` |

    Row 2 moves the gate from 250 to 100, so swiftlint's own default is wrong here. Row 4 is refused. Row 3 keeps both counts, so the child states `error` at the same number as `warning` for each rule — one gate each.

    That has a consequence: swiftlint exits 2 when it reports a finding of error severity. Measured: the complexity-16 probe reports 1 finding and swiftlint exits 2. The script therefore reads status 2 as a measured run beside status 0. Without that, EVERY complexity finding read as a broken tool.

    ## Finding 4 — the magic-numbers roster

    The child now states `severity: warning` and `test_parent_classes: ["QuickSpec", "XCTestCase"]` beside `allowed_numbers`. Both are swiftlint's own defaults. Measured over one file holding `return status == 404` with no project configuration: the new child reports 1 finding, and a child stating `allowed_numbers` alone reports the same 1 finding.

    ## Finding 5 — the getter and setter sentence

    Measured over two files with the shipped configuration: `public struct Plain` holding `public var value: Int { 1 }` and `public func setValue(_ next: Int)` reports 3 findings at rows 1, 2 and 3; the same two items inside `public struct Wide: Equatable` report 0. The bullet now carries the inherited-type condition and names `missing-docs.md`.

    ## RED to GREEN

    Three new acceptance tests, one for each rule, named `the_shipped_swift_<rule>_tool_rule_measures_beside_a_project_child_config`. Each stages a project `.swiftlint.yml` holding `child_config: other.yml` and `excluded: [Generated]`, beside the `other.yml` it names, and one file under `Generated/`.

    - RED: with the retry block removed from all three scripts, all 3 tests fail with `ToolRunError ... There's an ambiguity in the child / parent configuration tree`.
    - GREEN: with the retry, all 3 pass.

    The `error` option and the status-2 acceptance are held by the tests that already existed. RED: with `&& [ "$status" -ne 2 ]` removed from the complexity script, 5 of the 6 Swift complexity tests fail, and the doctor fixture pair fails with `tool broke on complexity-swift.fail.swift.tmpl: exited with exit status: 1`. GREEN: with it, all 6 pass.

    ## For the next agent

    - The shell message `Abort trap: 6` for a swiftlint abort is written by the SHELL, not by swiftlint, so it never lands in the redirected `lint.err`. It reaches the script's own stderr. The trigger has to read swiftlint's own text, which is why the test is on `Could not read configuration`.
    - swiftlint's JSON report order over two files is not stable. Two runs over the same pair gave `Generated` first and then `Sources` first. A probe that asserts an ordered file list must stage ONE file.
  timestamp: 2026-08-11T08:32:45.444197+00:00
- actor: claude-code
  id: 01kzqz80f8fkvx7r9mgnjnf5tb
  text: |-
    ### implement — changed
    - evidence: 8 files — `builtin/validators/README.md`, `builtin/validators/code-hygiene/rules/complexity-swift.md`, `magic-numbers-swift.md`, `missing-docs-swift.md`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, `tests/shipped/complexity.rs`, `tests/shipped/magic_numbers.rs`, `tests/shipped/missing_docs.rs`. `cargo nextest run -p swissarmyhammer-validators` — 614 tests run, 614 passed, 0 skipped. `cargo fmt --all -- --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. 3 new acceptance tests, each proved RED (`ToolRunError ... ambiguity in the child / parent configuration tree`) and GREEN. The `error:` option and the status-2 acceptance proved RED over 5 of the 6 Swift complexity tests, doctor fixture pair included, and GREEN. All 5 review findings flipped to `- [x]`.
    - next: `/review`
  timestamp: 2026-08-11T08:32:53.736993+00:00
- actor: claude-code
  id: 01kzr028bx223rjmswyd1dnaax
  text: |-
    ### review — findings
    - evidence: 2 findings — builtin/validators/code-hygiene/rules/magic-numbers-swift.md:54, builtin/validators/code-hygiene/rules/missing-docs-swift.md:55. The engine reviewed HEAD~1..HEAD (1cc990488), attempted 16 checks and confirmed 0. `cargo nextest run -p swissarmyhammer-validators` gave 614 tests and 614 passed, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`.
    - The two findings have one cause. A project `.swiftlint.yml` that holds `warning_threshold:` makes swiftlint exit 2 with a full JSON report. These two scripts test `if [ "$status" -ne 0 ]`, so each exits 1 and reports no finding. This commit closed the same failure mode for complexity-swift.md:56 with the test `[ "$status" -ne 0 ] && [ "$status" -ne 2 ]`.
    - The five items of the round of 2026-08-11 03:02 are each closed. Measured again with swiftlint 0.65.0: every option of every measured rule is stated in each child configuration; the two abort shapes (`child_config:` and bytes that are not YAML) each keep the baseline count at script exit 0 and write one line to stderr that names the drop; no broken run gives status 2, so the complexity script does not read a broken run as a measured run; and missing-docs.md is not made false.
    - next: give the magic-numbers and missing-docs scripts the same status test as the complexity script, and write the `warning_threshold` measurement into each rule body.
  timestamp: 2026-08-11T08:47:13.789334+00:00
- actor: claude-code
  id: 01kzr03nhjetn72ey28ys856mq
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 8 files; all five findings closed, each with its own measurement; the abort shapes fall back to the rule's own config with a stderr line
    - test: green — cargo nextest run -p swissarmyhammer-validators, 614 passed; clippy -D warnings clean
    - commit: 1cc990488 — 8 files
    - review: findings — magic-numbers-swift.md:54 and missing-docs-swift.md:55. A project `warning_threshold: 1` makes swiftlint exit 2 with the full report on stdout, and both scripts read a nonzero status as broken, so each reports 0. The complexity script is immune because this commit gave it the `-ne 2` guard. The same guard closes the other two.
  timestamp: 2026-08-11T08:48:00.050362+00:00
- actor: claude-code
  id: 01kzr0q6rgvj7pxyc1g5734810
  text: |-
    Both `warning_threshold` findings closed. Every number below was measured with swiftlint 0.65.0 on this machine, first over the shipped scripts pulled out of the rule files, then again through the real pipeline.

    ## The hole, reproduced

    Probe: `Sources/Magic.swift` holding `return status == 404`, and `Sources/Docs.swift` holding `public struct Thing` with one undocumented stored property.

    | the project `.swiftlint.yml` | magic-numbers script | missing-docs script |
    |---|---|---|
    | no file | 1 finding, exit 0 | 2 findings, exit 0 |
    | `warning_threshold: 1` (before) | 0 findings, exit 1 | 0 findings, exit 1 |
    | `warning_threshold: 1` (after) | 1 finding, exit 0 | 2 findings, exit 0 |

    The complexity script kept its count of 1 for the same project file at every step, because commit 1cc990488 gave it the `-ne 2` guard.

    ## The fix

    Each of the two scripts now tests `[ "$status" -ne 0 ] && [ "$status" -ne 2 ]`, which is the test `complexity-swift.md` already carried.

    ## Status 2 is a measured run

    swiftlint writes the whole report to stdout at status 2. The threshold entry is a synthetic row of `rule_id: warning_threshold` and error severity, and each script's `jq` filter selects its own rule id, so that row never becomes a finding.

    Measured against the magic-numbers child configuration, then against the missing-docs child configuration:

    | what the run is | status | stdout, magic | stdout, docs |
    |---|---|---|---|
    | a clean file | 0 | 0 entries | 0 entries |
    | the probe file | 0 | 1 entry | 2 entries |
    | the probe file beside `warning_threshold: 5` | 0 | 1 entry | 2 entries |
    | the probe file beside `warning_threshold: 1` | 2 | 2 entries | 3 entries |
    | one file whose only line is `public func oops( {` | 0 | 0 entries | 1 entry |
    | a path that holds no file | 1 | empty | empty |
    | a `--config` path that holds no file | 134 | empty | empty |
    | a project configuration that holds `child_config:` | 134 | empty | empty |
    | a command-line option that does not exist | 64 | empty | empty |

    Each run that broke wrote an empty stdout at status 1, 134 or 64. Each run of status 2 wrote a report of 2 entries or of 3 entries. So status 2 stands beside status 0 as a measured run.

    ## The other route to status 2 is shut

    A finding of error severity also makes swiftlint exit 2, and a project cannot reach one, because a child block replaces the parent block whole.

    - magic-numbers: a project that states `no_magic_numbers:` with `severity: error` gives exit 0 and 1 entry of warning severity. A CHILD that states `severity: error` gives exit 2 and 1 entry of error severity.
    - missing-docs: a project that states `missing_docs:` with `error: [open, public]` gives exit 0 and 2 entries of warning severity. A CHILD that states the same list gives exit 2 and 2 entries of error severity.

    `strict: true` in the project file is not a route either: measured with the missing-docs child, exit 0 and 2 entries.

    ## RED to GREEN

    Two acceptance tests, one for each changed script, named `the_shipped_swift_<rule>_tool_rule_measures_beside_a_project_warning_threshold`. Each stages the shared ordinary position beside a project `.swiftlint.yml` holding `warning_threshold: 1`, and reuses `ShippedStagedPositions` and the shared Swift constants in `tests/shipped.rs`.

    - RED, before the script change: both fail with `ToolRunError { rule: "magic-numbers-swift", detail: "exited with exit status: 1" }` and the same for `missing-docs-swift`.
    - GREEN, after: both pass. The magic-numbers run reports 1 finding, the missing-docs run reports 2.

    ## For the next agent

    - swiftlint breaches the threshold AT the number, not over it: 1 warning beside `warning_threshold: 1` exits 2. The finding text reads `Number of warnings exceeded threshold of 1.`
    - The threshold row of the report carries `file: null` and `line: 0`. A filter that selected every row would make a finding with no file.
    - The parent's `strict:` key does not reach a run that names a child configuration.
  timestamp: 2026-08-11T08:58:40.272564+00:00
- actor: claude-code
  id: 01kzr0qdvmjd42xmzxf87abxr7
  text: |-
    ### implement — changed
    - evidence: 5 files — `builtin/validators/code-hygiene/rules/magic-numbers-swift.md`, `builtin/validators/code-hygiene/rules/missing-docs-swift.md`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, `tests/shipped/magic_numbers.rs`, `tests/shipped/missing_docs.rs`. `cargo nextest run -p swissarmyhammer-validators` — 616 tests run, 616 passed, 0 skipped, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`. `cargo fmt --all` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. 2 new acceptance tests, each proved RED (`exited with exit status: 1`) and GREEN. Both review findings flipped to `- [x]`; 7 of 7 findings on the card are closed.
    - next: `/review`
  timestamp: 2026-08-11T08:58:47.540493+00:00
- actor: claude-code
  id: 01kzr1gt50qnwxqarhkjs3c32k
  text: |-
    ### review — findings
    - evidence: 5 findings — `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:54`, `magic-numbers-swift.md:261`, `missing-docs-swift.md:55`, `missing-docs-swift.md:218`, `complexity-swift.md:56`. The engine reviewed `HEAD~1..HEAD` (c2beb6ec9), attempted 8 checks and confirmed 0. `cargo nextest run -p swissarmyhammer-validators` gave 616 tests and 616 passed, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`. Both new acceptance tests pass.
    - The commit closes the `warning_threshold:` hole. A control run with the old status test on a copy of each script gives 0 findings at exit 1, and the shipped scripts give 1 finding and 2 findings at exit 0.
    - The commit opens a new hole. A project `.swiftlint.yml` that holds `swiftlint_version:` with a version that is not installed makes swiftlint write 0 bytes to stdout and exit 2. Each of the three scripts now reads status 2 as a measured run, so `jq` reads an empty report and the script reports 0 findings at exit 0. The engine reads a dirty file as clean. Before the commit the same project file gave exit 1, which the engine reads as a broken tool.
    - 36 project levers were run against each of the three scripts with swiftlint 0.65.0. Two independent measurements agree. Only `excluded:`, which the card asks for, and `swiftlint_version:` changed the counts.
    - next: tell a measured run apart from a broken run by more than the status. A run at status 2 that writes an empty stdout is a broken run. Apply the fix to all three rule files, and write the version-mismatch shape into each status table.
  timestamp: 2026-08-11T09:12:39.328420+00:00
- actor: claude-code
  id: 01kzr1j32g7vsd4p3a3qsfbd09
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — 5 files; both scripts read status 2 as a measured run, which closes the warning_threshold hole
    - test: green — cargo nextest run -p swissarmyhammer-validators, 616 passed; 2 acceptance tests proved RED then GREEN; clippy -D warnings clean
    - commit: c2beb6ec9 — 5 files
    - review: findings — 5 items. The warning_threshold hole IS closed. Accepting status 2 opened a new one: a project `swiftlint_version:` naming a version that is not installed makes swiftlint write 0 bytes to stdout, run no lint, and exit 2. All three scripts then report 0 findings at exit 0, so the engine reads a dirty file as clean.

    **The fix for the next iteration.** Status 2 alone is not enough. Accept status 2 only when the report file holds a non-empty JSON array. A broken run at status 2 writes 0 bytes, so the two are told apart by the report and not by the status. Apply it to all three scripts, prove it RED to GREEN against a project config holding `swiftlint_version: 99.0.0`, and correct the two prose claims that say no broken run states status 2.
  timestamp: 2026-08-11T09:13:21.232202+00:00
- actor: claude-code
  id: 01kzr29erwxt54jmx7x2zrkhn2
  text: |-
    All five findings closed. Every number below was measured with swiftlint 0.65.0 on this machine.

    ## The defect, reproduced before any edit

    The three shipped scripts were copied out of the rule files and run beside a probe tree.

    | the project `.swiftlint.yml` | magic numbers | docs | complexity |
    |---|---|---|---|
    | no file | 1 finding, exit 0 | 2 findings, exit 0 | 1 finding, exit 0 |
    | `swiftlint_version: 99.0.0` | 0 findings, exit 0 | 0 findings, exit 0 | 0 findings, exit 0 |
    | `warning_threshold: 1` | 1 finding, exit 0 | 2 findings, exit 0 | 1 finding, exit 0 |

    Row 2 is the defect: the engine read a dirty file as clean for each of the three.

    ## The discriminator is the REPORT, not the status

    Measured with the raw tool, at the child configuration of each rule:

    | the run | status | stdout |
    |---|---|---|
    | a clean file | 0 | an empty array, 5 bytes |
    | the magic-numbers probe | 0 | 1 entry, 385 bytes |
    | the probe beside `warning_threshold: 1` | 2 | 2 entries, 608 bytes |
    | the probe beside `swiftlint_version: 99.0.0` | 2 | 0 bytes |
    | a path that holds no file | 1 | 0 bytes |
    | a `--config` path that holds no file | 134 | 0 bytes |
    | a project file that holds `child_config:` | 134 | 0 bytes |
    | a command-line option that does not exist | 64 | 0 bytes |

    Every run that broke wrote 0 bytes. Every run that measured wrote a JSON array. So a measured run at status 2 holds a JSON array of one entry or more, and the version-mismatch run holds 0 bytes.

    ## The fix

    Each of the three scripts replaced the status test with:

    ```
    measured=0
    if [ "$status" -eq 0 ]; then
      measured=1
    elif [ "$status" -eq 2 ] &&
      jq -e 'type == "array" and length > 0' "$work/report.json" >/dev/null 2>&1
    then
      measured=1
    fi
    if [ "$measured" -eq 0 ]; then
      ...
    ```

    A 0-byte report makes `jq` exit nonzero, so the version-mismatch run falls to the broken path and the script exits 1.

    ## Both holes are shut at once

    After the fix, over the shipped scripts:

    | the project `.swiftlint.yml` | magic numbers | docs | complexity |
    |---|---|---|---|
    | `swiftlint_version: 0.65.0` | 1 finding, exit 0 | 2 findings, exit 0 | 1 finding, exit 0 |
    | `swiftlint_version: 0.64.0` | 0 findings, exit 1 | 0 findings, exit 1 | 0 findings, exit 1 |
    | `swiftlint_version: 99.0.0` | 0 findings, exit 1 | 0 findings, exit 1 | 0 findings, exit 1 |
    | `swiftlint_version: 0.1.0` | 0 findings, exit 1 | 0 findings, exit 1 | 0 findings, exit 1 |
    | `warning_threshold: 1` | 1 finding, exit 0 | 2 findings, exit 0 | 1 finding, exit 0 |

    ## RED to GREEN

    Three acceptance tests, one for each rule, named `the_shipped_swift_<rule>_tool_rule_breaks_beside_a_project_version_mismatch`. Each reuses `ShippedBrokenRun` and the shared Swift shapes in `tests/shipped.rs`, stages the rule's own probe source at the shared ordinary position, and stages a project `.swiftlint.yml` that holds `swiftlint_version: 99.0.0`.

    - RED, before the script change: all three fail at `the run must report exactly one tool error; got []` — 0 errors and 0 findings, which is the defect exactly.
    - GREEN, after: all three pass. The error detail carries `configuration specified version 99.0.0`.

    The two `warning_threshold` acceptance tests stayed green through the change, so both holes are shut together.

    ## The prose

    `magic-numbers-swift.md` and `missing-docs-swift.md`: the sentence "Status 2 states a measured run, and no broken run states it." is gone. Each status table now carries the `swiftlint_version:` row, and each `stdout` cell states the bytes, so "an empty array, 5 bytes" no longer reads the same as "0 bytes". The closing sentences state what the script accepts: status 0, and status 2 only when the report holds a JSON array of one entry or more. The two shapes that make swiftlint exit 2 with a report are named as two.

    `complexity-swift.md`: the sentence "the script reads status 2 as a measured run beside status 0" was narrower than the script. A new section states the report condition and carries its own three-row table.

    `builtin/validators/README.md`: the contract said "run it into a file, test the status against the findings status". That contract PRODUCED this defect. A new paragraph states that one status can carry both a measured run and a broken run, and that the script must then test the report beside the status.

    ## For the next agent

    - swiftlint writes `[\n\n]` — 5 bytes — for a clean run, and 0 bytes for a run that broke. The two are not the same, and a filter that reads "empty" as one thing loses the difference.
    - The complexity rule reaches status 2 on EVERY finding, because its child states `error:` at the same number as `warning:`. Its report at status 2 holds 1 entry in 413 bytes, so the new test keeps it measuring.
    - `jq -e 'type == "array" and length > 0'` on a 0-byte file exits nonzero from the parse error, which is the behaviour the test relies on.
  timestamp: 2026-08-11T09:26:06.876883+00:00
- actor: claude-code
  id: 01kzr29pm5hv108bmyy9yc5bhw
  text: |-
    ### implement — changed
    - evidence: 8 files — `builtin/validators/README.md`, `builtin/validators/code-hygiene/rules/magic-numbers-swift.md`, `missing-docs-swift.md`, `complexity-swift.md`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, `tests/shipped/magic_numbers.rs`, `tests/shipped/missing_docs.rs`, `tests/shipped/complexity.rs`. `cargo nextest run -p swissarmyhammer-validators` — 619 tests run, 619 passed, 0 skipped, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`. `cargo fmt --all -- --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. 3 new acceptance tests, each proved RED (`the run must report exactly one tool error; got []`) and GREEN. The 2 `warning_threshold` tests stayed green, so both holes are shut at once. All 5 review findings flipped to `- [x]`; 12 of 12 findings on the card are closed.
    - next: `/review`
  timestamp: 2026-08-11T09:26:14.917125+00:00
- actor: claude-code
  id: 01kzr37dzkj1z5bfc1b7hjxjyt
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (92dd40b67) attempted 16 checks, confirmed 0 and refuted 0. `cargo nextest run -p swissarmyhammer-validators` gave 619 tests and 619 passed. A trust-boundary measurement of 82 project levers, in 246 script runs, with swiftlint 0.65.0, gave 3 findings — `builtin/validators/code-hygiene/rules/complexity-swift.md:190` (the same sentence at `magic-numbers-swift.md:294` and `missing-docs-swift.md:251`), `builtin/validators/README.md:177`, `builtin/validators/README.md:186`.
    - The gate holds. No lever makes a script report nothing at exit 0 while the fixture is dirty, other than `excluded:` of the source directory, which is the behaviour the card asks for. Each of the 10 `swiftlint_version:` values that is not the installed version gives 0 findings at exit 1, so no broken run is read as measured. No run of the 246 gave a count of findings above 0 at a nonzero exit, so no measured run is read as broken. The composite attack fails: no project file makes swiftlint write a status-2 array that holds no entry of the measured rule.
    - Each of the three new acceptance tests proves the gate. A control that puts the old gate back on a copy gives 0 findings at exit 0, where the shipped script gives 0 findings at exit 1, and the engine raises a tool error only on a nonzero exit.
    - Each finding is a false sentence, and none is a gate hole. The first is the most important: "It breaks at every other status" is false, because each script exits 0 at status 1 when stderr holds `No lintable files found`, and that branch is how the `excluded:` behaviour this card asks for reaches exit 0.
    - next: correct the three sentences, then run `/review 01KZNNJ0Q8BH2X59JGHXV57PF8 HEAD~1..HEAD` again.
  timestamp: 2026-08-11T09:42:29.107964+00:00
- actor: claude-code
  id: 01kzr3f6k4kgzmhegs31x9exxg
  text: |-
    ### finish iteration 4 — findings
    - implement: changed — 8 files; each script accepts status 2 only when the report holds a JSON array of one entry or more, because a version mismatch exits 2 with 0 bytes
    - test: green — cargo nextest run -p swissarmyhammer-validators, 619 passed; 3 acceptance tests proved RED then GREEN; clippy -D warnings clean
    - commit: 92dd40b67 — 8 files
    - review: findings — 3 items, each a false sentence, and NONE is a gate hole

    **The gate is proved.** 82 project levers in 246 script runs against the three scripts, with swiftlint 0.65.0. No lever makes a script report nothing at exit 0 while the file is dirty, other than `excluded:` of the source directory, which is the wanted behaviour. Each of 10 `swiftlint_version:` values that is not installed gives 0 findings at exit 1, so the hole is closed over the whole value space. No measured run is read as broken. The composite attack fails.

    The three open findings are prose:
    1. `complexity-swift.md:190`, `magic-numbers-swift.md:294`, `missing-docs-swift.md:251` — "It breaks at every other status" is false. Each script exits 0 at status 1 when stderr holds `No lintable files found`, and that branch is how the `excluded:` behaviour reaches exit 0.
    2. `README.md:177` — "A linter keeps one status for findings and a higher status for a failure" is false; swiftlint uses status 2 for both.
    3. `README.md:186` — the entry count is the count of findings plus one, not the constant 2.
  timestamp: 2026-08-11T09:46:43.684428+00:00
- actor: claude-code
  id: 01kzr7gr9jdb374vz9bfm8k600
  text: |-
    All three findings closed. Each is a false sentence, and each correction below was measured again with swiftlint 0.65.0 on this machine before it was written.

    ## How the measurement was made

    The three `run` scripts were extracted from the rule files at the current working tree and run without a change. Three fixtures, each under `src/`:

    - `src/Magic.swift` — `return status == 404`. Baseline 1 finding.
    - `src/Docs.swift` — `public struct Thing` with one undocumented stored property. Baseline 2 findings.
    - `src/Complex.swift` — one function of cyclomatic complexity 16. Baseline 1 finding.

    Each lever is one project `.swiftlint.yml` beside the fixture. The raw swiftlint run states the same argument list the script states, with the child configuration the script writes.

    ## Finding 1 — `No lintable files found` exits 0

    Measured, for each of the three scripts, with a project `.swiftlint.yml` that states `excluded: [src]`, and again with `excluded: ["."]`:

    | script | swiftlint status | swiftlint stdout | script exit | findings |
    |---|---|---|---|---|
    | magic numbers | 1 | 0 bytes | 0 | 0 |
    | missing docs | 1 | 0 bytes | 0 | 0 |
    | complexity | 1 | 0 bytes | 0 | 0 |

    swiftlint writes `Error: No lintable files found at paths: 'src/Magic.swift'` to stderr, and the same line with the other two file names. So the sentence "It breaks at every other status" was false: the script exits 0 at status 1 for that shape, through the branch `if grep -qF 'No lintable files found' "$work/lint.err"; then exit 0; fi`.

    Each of the three rule bodies now states the branch, names the measurement, and points at the section "A run whose every file the project excludes". Each of the three status tables carries a new row: `the same file beside a project excluded: that covers it | 1 | 0 bytes`.

    ## Finding 2 — the failure status is the same as the findings status

    Measured with the child configuration of each rule:

    | the run | status |
    |---|---|
    | complexity fixture, no project file, 1 finding of error severity | 2 |
    | magic-numbers fixture beside `warning_threshold: 1`, 2 entries | 2 |
    | missing-docs fixture beside `warning_threshold: 1`, 3 entries | 2 |
    | any fixture beside `swiftlint_version: 99.0.0`, 0 bytes, no lint | 2 |

    So swiftlint 0.65.0 answers 2 for a run that reports findings AND for a version-mismatch failure. The README sentence now reads "A linter can keep one status for findings and another status for a failure", and the paragraph under it states "The status of a failure is then the same as the status of a finding."

    ## Finding 3 — the entry count is the findings count plus one

    Measured with `warning_threshold: 0` beside each fixture, with the child configuration each script writes:

    | fixture | findings | entries | `rule_id` values |
    |---|---|---|---|
    | magic numbers | 1 | 2 | `no_magic_numbers`, `warning_threshold` |
    | complexity | 1 | 2 | `cyclomatic_complexity`, `warning_threshold` |
    | missing docs | 2 | 3 | `missing_docs`, `missing_docs`, `warning_threshold` |

    The README now states the count as one entry for each finding and one entry more for the threshold, and it names the three measured counts.

    ## Two results the next agent should keep

    - The complexity script at `warning_threshold: 1` keeps 1 entry at status 2, where `warning_threshold: 0` gives 2 entries. The threshold counts WARNINGS, and the one complexity finding is error severity, so the count of warnings is 0. A threshold of 0 is met at 0 warnings.
    - `swiftlint_version: 0.65.0`, the version that is installed, keeps the baseline count for each of the three scripts at script exit 0. The mismatch alone breaks the run.

    ## Tests

    `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then `cargo nextest run -p swissarmyhammer-validators` — 619 tests run, 619 passed, 0 skipped. `cargo fmt --all -- --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean. No test changed, because the scripts did not change: this round corrects prose alone, and the behaviour each sentence describes was already held by the acceptance tests the earlier rounds added.
  timestamp: 2026-08-11T10:57:28.882894+00:00
- actor: claude-code
  id: 01kzr7gzehn7sj0cwt87jczfaq
  text: |-
    ### implement — changed
    - evidence: 4 files — `builtin/validators/README.md`, `builtin/validators/code-hygiene/rules/complexity-swift.md`, `magic-numbers-swift.md`, `missing-docs-swift.md`. `cargo nextest run -p swissarmyhammer-validators` — 619 tests run, 619 passed, 0 skipped, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`. `cargo fmt --all -- --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean. The three false sentences are gone from all three rule files and from the README, and each replacement carries a measurement made with swiftlint 0.65.0 on this machine. All 3 review findings flipped to `- [x]`; 15 of 15 findings on the card are closed.
    - next: `/review`
  timestamp: 2026-08-11T10:57:36.209023+00:00
- actor: claude-code
  id: 01kzr8bdsymr06g4ypwf0tet63
  text: |-
    ### review — findings

    - evidence: 4 findings — `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:290`, `builtin/validators/README.md:192`, `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:309`, and the commit message of bcc6d6cc3. The engine attempted 0 checks on `HEAD~1..HEAD` (bcc6d6cc3), because no validator matches `*.md` and the commit changes prose only. `cargo nextest run -p swissarmyhammer-validators` gave 619 tests and 619 passed, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`. Measurement used swiftlint 0.65.0 against the three shipped scripts, which were copied out of the rule files without a change.
    - measured: a project `.swiftlint.yml` that holds `excluded: [src]` gives raw status 1, 0 bytes on stdout, script 0 findings and script exit 0, for each of the three scripts. `warning_threshold: 0` gives status 2 with 2 entries for magic numbers, 2 for complexity and 3 for missing docs. `swiftlint_version: 99.0.0` gives status 2, 0 bytes and 0 `Linting` lines. `git show bcc6d6cc3` gives 60 added lines and 28 removed lines, 0 of them script lines, and 1 added line of 117 characters.
    - next: correct the two false sentences, wrap the 117-character line, and write the stderr test into the README. The card stays in `review`.
  timestamp: 2026-08-11T11:12:02.878574+00:00
- actor: claude-code
  id: 01kzr8d1ywzce9wxvxzcqkz4ag
  text: |-
    ### finish iteration 5 — findings
    - implement: changed — 4 files; the three false sentences of the round of 04:45 are gone, each replacement measured with swiftlint 0.65.0
    - test: green — cargo nextest run -p swissarmyhammer-validators, 619 passed, 0 skipped, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`; fmt and clippy clean
    - commit: bcc6d6cc3 — 4 files, 60 insertions, 28 deletions
    - review: findings — 4 items. `magic-numbers-swift.md:290` and `missing-docs-swift.md:247` state "Each run that broke wrote 0 bytes, at status 1, 134, 64 or 2.", which classes the `excluded:` answer as a broken run; `README.md:192` omits the `No lintable files found` test the three scripts make; `magic-numbers-swift.md:309` is 117 characters where 59 of the 60 added lines are 79 or fewer; the commit message of bcc6d6cc3 says the scripts test stderr, and the commit changes 0 script lines.
    - The engine attempted 0 checks. No validator matches `*.md`, and this commit changes prose only. The verification is measurement.
    - next: correct the four items. Each round to now gives new findings, so the guardrail is not armed.
  timestamp: 2026-08-11T11:12:56.284366+00:00
- actor: claude-code
  id: 01kzr8kf35swcrqsz9c48ah65c
  text: |-
    Research for the round of 06:20. Every number below was measured on this machine with swiftlint 0.65.0, before an edit was made.

    ## How the measurement was made

    The three `run` scripts were extracted from the front matter of the rule files at HEAD (bcc6d6cc3) and run without a change. Three fixtures, each under `src/`:

    - `src/Magic.swift` — `return status == 404`.
    - `src/Docs.swift` — `public struct Thing` with one undocumented stored property.
    - `src/Complex.swift` — one function of cyclomatic complexity 16.

    ## Finding 1 — a run at status 1 with 0 bytes gives a clean answer

    Baseline, with no project `.swiftlint.yml`: magic numbers 1 finding at script exit 0; missing docs 2 findings at script exit 0; complexity 1 finding at script exit 0.

    Beside a project `.swiftlint.yml` that holds `excluded: [src]`:

    | script | raw status | raw stdout | script exit | findings |
    |---|---|---|---|---|
    | magic numbers | 1 | 0 bytes | 0 | 0 |
    | missing docs | 1 | 0 bytes | 0 | 0 |
    | complexity | 1 | 0 bytes | 0 | 0 |

    swiftlint writes `Error: No lintable files found at paths: 'src/Magic.swift'` to stderr, and the same line with the other two file names. So the sentence "Each run that broke wrote 0 bytes, at status 1, 134, 64 or 2." classes a clean answer as a broken run.

    `grep -rn "Each run that broke wrote 0 bytes" builtin/validators/` gives 2 hits: `magic-numbers-swift.md:290` and `missing-docs-swift.md:247`. `complexity-swift.md` does not hold the sentence. Its status table at lines 183 to 188 carries 4 rows and no such closing sentence, so the cause is in two files and not three.

    ## Finding 2 — the README omits the stderr test

    `grep -c 'No lintable' builtin/validators/README.md` gives 0.

    The gate block after the status test is byte-identical in the three scripts. The md5 of the block is `3a5027629666af84f5d410d00247e7a6` for each of the three:

    ```
    if [ "$measured" -eq 0 ]; then
      if grep -qF 'No lintable files found' "$work/lint.err"; then
        exit 0
      fi
      exit 1
    ```

    ## Finding 3 — the line width

    `awk 'NR==309 {print length($0)}' builtin/validators/code-hygiene/rules/magic-numbers-swift.md` gives 117. Of the 60 lines that bcc6d6cc3 adds, 59 are 79 characters or fewer and 1 is 117.

    ## Finding 4 — the commit message of bcc6d6cc3

    `git show --numstat bcc6d6cc3` gives 60 added lines and 28 removed lines over 4 files: README 14 and 11, complexity 11 and 2, magic numbers 16 and 6, missing docs 19 and 9.

    The `run` script was extracted from each of the three rule files at bcc6d6cc3~1 and at bcc6d6cc3, and `diff` reports the two are identical for each of the three. So the commit changes 0 script lines. The hunk headers are `@@ -174,17 +174,20 @@`, `@@ -185,10 +185,19 @@`, `@@ -276,6 +276,7 @@`, `@@ -291,12 +292,21 @@`, `@@ -233,6 +233,7 @@` and `@@ -248,15 +249,24 @@`, and no hunk reaches the front matter.

    ## The numbers of the line that is rewrapped hold

    The sentence at magic-numbers-swift.md:306 to 311 states a `swiftlint_version:` result. Measured again over the three fixtures:

    | the project `.swiftlint.yml` | magic numbers | missing docs | complexity |
    |---|---|---|---|
    | `swiftlint_version: 0.65.0` | 1 finding, exit 0 | 2 findings, exit 0 | 1 finding, exit 0 |
    | `swiftlint_version: 0.64.0` | 0 findings, exit 1 | 0 findings, exit 1 | 0 findings, exit 1 |
    | `swiftlint_version: 99.0.0` | 0 findings, exit 1 | 0 findings, exit 1 | 0 findings, exit 1 |
    | `swiftlint_version: 0.1.0` | 0 findings, exit 1 | 0 findings, exit 1 | 0 findings, exit 1 |

    The rewrap keeps the sentence and changes the line breaks alone.
  timestamp: 2026-08-11T11:16:26.341403+00:00
- actor: claude-code
  id: 01kzr8y1w3a2r5fk7ed5ybv8fz
  text: |-
    ## Correction to the commit message of bcc6d6cc3

    Finding 4 of the round of 06:20 cannot be closed by an edit, because a commit that is made cannot be rewritten. The correction stands here.

    The message of bcc6d6cc3 says:

    > The scripts now test stderr to tell the two cases apart.

    That sentence is false for bcc6d6cc3. The commit changes 0 script lines.

    Evidence, measured on this machine:

    - `git show --numstat bcc6d6cc3` gives 60 added lines and 28 removed lines over 4 files: README 14 added and 11 removed, complexity 11 and 2, magic numbers 16 and 6, missing docs 19 and 9. Each of the 88 lines is prose.
    - The `run` script was extracted from the front matter of each of the three rule files at bcc6d6cc3~1 and at bcc6d6cc3. `diff` reports the two are identical for each of the three.
    - The hunk headers are `@@ -174,17 +174,20 @@`, `@@ -185,10 +185,19 @@`, `@@ -276,6 +276,7 @@`, `@@ -291,12 +292,21 @@`, `@@ -233,6 +233,7 @@` and `@@ -248,15 +249,24 @@`. No hunk reaches the front matter, where each script stands.

    The stderr branch shipped in commit 92dd40b67. bcc6d6cc3 states that branch in prose alone.

    ## The message for the commit of this round

    This round also changes prose alone. The three `run` scripts were extracted again at the working tree and compared with the scripts at HEAD: `diff` reports the two are identical for each of the three. The commit message must state that, and it must not say a script changed. The message to use:

    ```
    docs(validators): state the stderr test and correct two false sentences (^xv57pf8)

    `magic-numbers-swift.md` and `missing-docs-swift.md` said "Each run that
    broke wrote 0 bytes, at status 1, 134, 64 or 2." A run at status 1 with 0
    bytes gives a clean answer when the project excludes each file, so the
    sentence classed a clean answer as a broken run. Each file now states the
    measurement.

    The README contract stated the report test and left out the stderr test the
    three shipped scripts make. It now states both.

    One line of `magic-numbers-swift.md` stood at 117 characters. It is wrapped.

    This commit changes 0 script lines. The three scripts are byte-identical
    with the scripts at bcc6d6cc3.
    ```
  timestamp: 2026-08-11T11:22:13.251959+00:00
- actor: claude-code
  id: 01kzr8yp24336rqtydmfcy5h8r
  text: |-
    All four findings closed. Each number below was measured with swiftlint 0.65.0 on this machine before the sentence was written, and each was measured again after the edit.

    ## Finding 1 — the false sentence, in two files

    `grep -rn "Each run that broke wrote 0 bytes" builtin/validators/` gave 2 hits before the edit and gives 0 after. The two files are `magic-numbers-swift.md` and `missing-docs-swift.md`. `complexity-swift.md` does not hold the sentence, so the cause was in two files and not three.

    The sentence now reads:

    > Each run that measured wrote a JSON array, at status 0 or 2. Each other run wrote 0 bytes, at status 1, 134, 64 or 2. A report of 0 bytes does not make a run broken. The run beside a project `excluded:` that covers the file writes 0 bytes at status 1, and it gives a clean answer. Stderr tells that run from a run that broke.

    That leads into the paragraph under it, which already stated the stderr branch.

    ## Finding 2 — the stderr test in the README contract

    `grep -c 'No lintable' builtin/validators/README.md` gave 0 before and gives 2 after. The README now carries one paragraph beside the report-test paragraph. It states the stderr test, quotes the swiftlint message, and names the measurement.

    ## Finding 3 — the line width

    `awk 'NR==309 {print length($0)}'` gave 117 before. The paragraph is wrapped over three lines. `git diff` of `builtin/validators/` gives 0 added lines over 79 characters.

    ## Finding 4 — the commit message correction

    The correction stands in the comment above, with the message for the commit of this round.

    ## Measurement, after the edit

    The three `run` scripts were extracted from the front matter of the rule files at the working tree. `diff` reports each is identical with the script at HEAD, so this round changes prose alone.

    Baseline, with no project `.swiftlint.yml`:

    | script | findings | script exit |
    |---|---|---|
    | magic numbers | 1 | 0 |
    | missing docs | 2 | 0 |
    | complexity | 1 | 0 |

    Beside a project `.swiftlint.yml` that holds `excluded: [src]`:

    | script | raw status | raw stdout | findings | script exit |
    |---|---|---|---|---|
    | magic numbers | 1 | 0 bytes | 0 | 0 |
    | missing docs | 1 | 0 bytes | 0 | 0 |
    | complexity | 1 | 0 bytes | 0 | 0 |

    The stderr line of each of the three, byte for byte: `Error: No lintable files found at paths: 'src/Magic.swift'`, `'src/Docs.swift'`, `'src/Complex.swift'`.

    The `swiftlint_version:` numbers of the rewrapped sentence hold: at `0.65.0` each script keeps its baseline count at exit 0; at `0.64.0`, at `99.0.0` and at `0.1.0` each script reports 0 findings at exit 1.

    ## Language

    The added lines hold 0 frequency words, from the set usually, often, always, generally, normally, typically, sometimes, rarely, mostly, may, might, should, probably, simply, just, basically.

    ## For the next agent

    - The `run` script of a rule file can be extracted from the `run: |` block of the front matter by indentation. That gives a script that can be run beside a probe tree without the engine, and it is how each round of this card measured the trust boundary.
    - `dump validators` over a `*.md` path returns 0 rules. No validator matches Markdown, so the review of a prose round is measurement and not an engine check.
    - A rule `.md` edit does not rebuild the compiled-in builtins. `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` before `cargo nextest run`.
  timestamp: 2026-08-11T11:22:33.924974+00:00
- actor: claude-code
  id: 01kzr8yyktxqs5zvm7ds9ja8wp
  text: |-
    ### implement — changed
    - evidence: 3 files — `builtin/validators/README.md` (11 added, 0 removed), `builtin/validators/code-hygiene/rules/magic-numbers-swift.md` (8 added, 6 removed), `builtin/validators/code-hygiene/rules/missing-docs-swift.md` (5 added, 3 removed). `cargo nextest run -p swissarmyhammer-validators` — 619 tests run, 619 passed, 0 skipped, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`. `cargo fmt --all -- --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean. `grep -rn "Each run that broke wrote 0 bytes" builtin/validators/` 0 hits. `grep -c 'No lintable' builtin/validators/README.md` 2. 0 added lines over 79 characters. 0 frequency words. The three `run` scripts are byte-identical with the scripts at HEAD, so this round changes prose alone. All 4 review findings flipped to `- [x]`; 19 of 19 findings on the card are closed.
    - next: `/review`. The card stays in `doing`. The commit message for this round stands in the comment above.
  timestamp: 2026-08-11T11:22:42.682432+00:00
- actor: claude-code
  id: 01kzr92cvwm8zcdz3ygfhxrjh7
  text: |-
    ### commit — changed
    - evidence: 6abc69a4e docs(validators): fix status-1 clean-run wording, add stderr test to README (^xv57pf8); 3 files changed, 24 insertions(+), 9 deletions(-)
    - next: review
  timestamp: 2026-08-11T11:24:35.580175+00:00
- actor: claude-code
  id: 01kzr9nkq1xbcnn0x2trq06qxn
  text: |
    ### review — findings
    - scope: `review sha HEAD~1..HEAD` (6abc69a4e). The engine attempted 0 checks, because no validator matches `*.md`.
    - tests: `cargo nextest run -p swissarmyhammer-validators` gave 619 tests, 619 passed, 0 failed, 0 skipped, 0 warnings, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`.
    - measurement: swiftlint 0.65.0 against the three shipped scripts, extracted from the front matter with a YAML parser.
    - evidence: 3 findings — `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:294`, `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:269`, `builtin/validators/code-hygiene/rules/missing-docs-swift.md:403`.
    - closed this round: each of the 4 findings of the round of 06:20. The README now names the stderr test, `grep -c 'No lintable'` gives 2 where it gave 0. 0 of the 24 added lines are more than 79 characters, where the last round measured 1 line at 117.
    - next: correct the 3 false sentences. Finding 1 and finding 2 each stand in more than one file, so the cause must leave every file the finding names.
  timestamp: 2026-08-11T11:35:05.185410+00:00
- actor: claude-code
  id: 01kzr9qeyhae8k4zbfrg907wra
  text: |-
    ### finish iteration 6 — findings
    - implement: changed — 3 files; the false broken-run sentence left both rule files, the README gained the stderr test, the 117-character line is wrapped
    - test: green — cargo nextest run -p swissarmyhammer-validators, 619 passed, 0 skipped, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`; fmt and clippy clean
    - commit: 6abc69a4e — 3 files
    - review: findings — 3 items, each a false sentence. `magic-numbers-swift.md:294` and `missing-docs-swift.md:251` say stderr tells the clean run from the broken run, and 4 different runs give the same status, the same 0 bytes and the same stderr substring. `magic-numbers-swift.md:269` and `missing-docs-swift.md:226` say the report tells a measured run from a broken run, and the file's own table gives status 2 / 0 bytes for a broken run beside status 1 / 0 bytes for a clean run. `missing-docs-swift.md:403` states 2 findings where the measurement gives 1.
    - The engine attempted 0 checks. No validator matches `*.md`.
    - guardrail: 6 rounds, and each round gives new findings. No finding repeats. The guardrail is not armed.
    - next: correct the three sentences in every file that holds them.
  timestamp: 2026-08-11T11:36:05.841475+00:00
- actor: claude-code
  id: 01kzra6s7r71f1jjpq76k8f8zx
  text: |
    ### Measurement, swiftlint 0.65.0, this machine

    The three scripts were read out of the front matter of the rule files at
    6abc69a4e with a YAML parser, and run without a change. Each raw run used
    the same child configuration the matching script writes.

    Set A, the 4 shapes that hold `No lintable files found` (magic numbers):

    | the run | raw status | stdout | script findings | script exit |
    |---|---|---|---|---|
    | `excluded: [src]` over `src/Magic.swift` | 1 | 0 bytes | 0 | 0 |
    | the directory `hollow` | 1 | 0 bytes | 0 | 0 |
    | the path `src/Absent.swift` | 1 | 0 bytes | 0 | 1 |
    | the file `src/Notes.txt` | 1 | 0 bytes | 0 | 0 |

    Each of the 4 wrote `Error: No lintable files found at paths: '<path>'` to
    stderr. The string names the path. The `[ ! -r "$file" ]` guard answers
    `magic-numbers-swift cannot read src/Absent.swift` at exit 1 for row 3,
    before swiftlint runs. So the guard makes that distinction, and stderr does
    not.

    Set B, the status and report pairs (magic numbers):

    | the project file | raw status | stdout | script findings | script exit |
    |---|---|---|---|---|
    | no file | 0 | 1 entry, 382 bytes | 1 | 0 |
    | `warning_threshold: 1` | 2 | 2 entries, 605 bytes | 1 | 0 |
    | `swiftlint_version: 99.0.0` | 2 | 0 bytes | 0 | 1 |
    | `excluded: [src]` | 1 | 0 bytes | 0 | 0 |

    The report separates the two runs of status 2. At status 1 the report is 0
    bytes for the clean run and for the broken run, so the report separates no
    other pair.

    Set C, the parse-error file (missing docs). One file whose only line is
    `public func oops( {`: raw status 0, 1 entry, 361 bytes; the script reports
    1 finding and exits 0. The count of 2 in the rule body was false.

    Set D and E, the docs fixture. No project file: raw status 0, 2 entries, 720
    bytes, script 2 findings at exit 0. `excluded: [src]`: raw status 1, 0
    bytes, script 0 findings at exit 0. `swiftlint_version: 99.0.0`: raw status
    2, 0 bytes, script 0 findings at exit 1. `warning_threshold: 1`: raw status
    2, 3 entries, 945 bytes, script 2 findings at exit 0. The docs child over
    the directory `hollow`, over `src/Absent.swift` and over `src/Notes.txt`:
    raw status 1 and 0 bytes for each. The script answers 0 findings at exit 0
    for `hollow` and for `src/Notes.txt`, and 0 findings at exit 1 for
    `src/Absent.swift`, with `missing-docs-swift cannot read src/Absent.swift`.

    Set F, the complexity fixture, one function of cyclomatic complexity 16. No
    project file: raw status 2, 1 entry, 411 bytes, script 1 finding at exit 0.
    `excluded: [src]`: raw status 1, 0 bytes, script 0 findings at exit 0.
    `swiftlint_version: 99.0.0`: raw status 2, 0 bytes, script 0 findings at
    exit 1. The complexity child over `hollow`, over `src/Absent.swift` and over
    `src/Notes.txt`: raw status 1 and 0 bytes for each; the script answers 0
    findings at exit 0 for the first and the third, and 0 findings at exit 1 for
    the second, with `complexity-swift cannot read src/Absent.swift`.

    Discovery. The byte counts of the status tables move with the length of the
    absolute path, because the JSON reporter writes the absolute path of each
    file into each entry. The counts I measured are 3 bytes less for each entry
    than the counts the tables hold. The entry counts agree. A separate card
    records this.
  timestamp: 2026-08-11T11:44:27.896413+00:00
- actor: claude-code
  id: 01kzrabat5zma9mx81ep07r6c0
  text: |
    ### implement — changed

    - what changed: the 3 open findings of the 2026-08-11 06:31 section, and the
      same cause in each file that holds it.
      - "Stderr tells that run from a run that broke." leaves
        `magic-numbers-swift.md` and `missing-docs-swift.md`. The new sentence
        names the guard on each file and the test on stderr, and points at the
        two paragraphs that state each test. Each of the 3 rule files gains a
        paragraph that states the limit: the stderr string names the path, the
        4 measured shapes hold that string, and the `[ ! -r "$file" ]` guard
        makes the one distinction that stderr does not. That paragraph also
        states the distinction no test makes.
      - "The status alone does not tell a measured run from a broken run. The
        REPORT does." now holds its limit in `magic-numbers-swift.md` and
        `missing-docs-swift.md`: the report tells the two apart at status 2,
        and at status 1 the report is 0 bytes for the clean run and for the
        broken run. `complexity-swift.md` held "The REPORT tells the two
        apart." and now holds the same limit.
      - "reports 2 `missing_docs` findings" becomes "reports 1 `missing_docs`
        finding" at `missing-docs-swift.md`, with the row of the status table
        named. Measured: 1 finding at exit 0.
    - evidence: 3 files, 61 added lines, 10 removed lines —
      `builtin/validators/code-hygiene/rules/magic-numbers-swift.md`,
      `builtin/validators/code-hygiene/rules/missing-docs-swift.md`,
      `builtin/validators/code-hygiene/rules/complexity-swift.md`.
      `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then
      `cargo nextest run -p swissarmyhammer-validators`: 619 tests run, 619
      passed, 0 skipped, 0 failed.
      `grep -rn "Stderr tells that run\|The REPORT does\|The REPORT tells the
      two apart\." builtin/` gives 0 hits.
      `grep -rn "reports 2 \`missing_docs\`" builtin/` gives 0 hits.
      0 of the 61 added lines is more than 79 characters.
      The 61 added lines hold 0 frequency words, from the set usually, often,
      always, generally, normally, typically, sometimes, rarely, mostly, may,
      might, should, probably, simply, just, basically.
    - new work: ^y81mb7n records the byte counts of the status tables, which
      move with the length of the absolute path.
    - next: /review. The card stays in `doing`. No commit, no push.
  timestamp: 2026-08-11T11:46:56.965726+00:00
- actor: claude-code
  id: 01kzray99ch57rbbz9def4g1v4
  text: |
    ### review — findings

    - scope: `review sha HEAD~1..HEAD` (da876389e). The engine attempted 0 checks, because no validator matches `*.md`. `review file` over the 3 rule files gave the same 0.
    - measurement: swiftlint 0.65.0 and jq 1.8.2. The 3 scripts were read out of the front matter with a YAML parser, and run without a change. 12 raw runs for the 4 shapes, 6 raw runs for the status pairs, 1 run for the file that does not parse.
    - tests: `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then `cargo nextest run -p swissarmyhammer-validators` gave 619 tests run, 619 passed, 0 failed, 0 skipped.
    - evidence: 2 findings — `builtin/validators/code-hygiene/rules/complexity-swift.md:183`; and the same sentence in 3 files at `complexity-swift.md:183`, `magic-numbers-swift.md:273`, `missing-docs-swift.md:230`.
    - closed this round: each of the 3 findings of the 06:31 round. `grep -rn "Stderr tells that run" builtin/` gives 0 hits. `grep -rn "The REPORT does\." builtin/` gives 0 hits. `grep -rn "reports 2 \`missing_docs\`" builtin/` gives 0 hits. The corrected count of 1 finding for the file that does not parse holds, and it agrees with the row at `missing-docs-swift.md:241`.
    - confirmed: the 4-shape paragraph holds in 3 of 3 files, 12 of 12 runs at status 1 with 0 stdout bytes and the string `No lintable files found`. The guard string of each file holds word for word. The status-2 half of the corrected sentence holds: the array run against the 0-byte run, in 3 of 3 files.
    - not a finding this round: the stdout byte counts of the status tables. They move with the length of the absolute path. Card ^y81mb7n owns that.
    - next: give the table of `complexity-swift.md` the row for a path that holds no file, and name a status-1 broken shape that the script reaches.
  timestamp: 2026-08-11T11:57:17.996294+00:00
- actor: claude-code
  id: 01kzraz8z1x0vbpypfs0tr7b0z
  text: |-
    ### finish iteration 7 — findings
    - implement: changed — 3 files, 61 insertions, 10 deletions; each false sentence names the test that makes its distinction, and each file states the distinction that no test makes
    - test: green — cargo nextest run -p swissarmyhammer-validators, 619 passed, 0 skipped, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`
    - commit: da876389e — 3 files
    - review: findings — 2 items. `complexity-swift.md:183` names two status-1 runs, and its table of 4 rows holds no row for a path that holds no file, where the other two files hold that row. The same sentence in all three files shows the status-1 limit with a run the script never makes: the `[ ! -r "$file" ]` guard runs before `mktemp -d`, so over `src/Absent.swift` each script wrote `<rule> cannot read src/Absent.swift`, ran swiftlint 0 times and exited 1. Two reachable shapes show the same limit: a directory that holds no Swift file, and a file whose name does not end in `.swift`.
    - The three sentences of the 06:31 round each give 0 grep hits.
    - guardrail: 7 rounds, each with new findings. No finding repeats. The guardrail is not armed.
    - next: give `complexity-swift.md` the missing table row, and write each of the three sentences to a run the script makes.
  timestamp: 2026-08-11T11:57:50.433415+00:00
- actor: claude-code
  id: 01kzrbc6e0j9pkgged60v1thd5
  text: |
    Re-measurement for the 2 findings of the 06:56 round. swiftlint 0.65.0,
    jq 1.8.2, on this machine. The 3 scripts were read out of the front matter
    of the rule files with a YAML parser and run without a change.

    Fixture: `src/Notes.txt` holding one line of plain text; an empty directory
    `hollow`; `src/Absent.swift`, which holds no file. Each child configuration
    was written out word for word from the `printf` block of its rule.

    Raw swiftlint, child configuration alone, 9 runs:

    | rule | shape | status | stdout |
    |---|---|---|---|
    | complexity | `src/Absent.swift` | 1 | 0 bytes |
    | complexity | `hollow` | 1 | 0 bytes |
    | complexity | `src/Notes.txt` | 1 | 0 bytes |
    | magic-numbers | `src/Absent.swift` | 1 | 0 bytes |
    | magic-numbers | `hollow` | 1 | 0 bytes |
    | magic-numbers | `src/Notes.txt` | 1 | 0 bytes |
    | missing-docs | `src/Absent.swift` | 1 | 0 bytes |
    | missing-docs | `hollow` | 1 | 0 bytes |
    | missing-docs | `src/Notes.txt` | 1 | 0 bytes |

    Each of the 9 wrote `Error: No lintable files found at paths: '<path>'` to
    stderr.

    The shipped scripts, over the same 3 shapes, with a shim that counts each
    swiftlint call, 9 runs:

    | rule | shape | exit | swiftlint calls | stdout |
    |---|---|---|---|---|
    | complexity-swift | `src/Absent.swift` | 1 | 0 | 0 bytes |
    | complexity-swift | `hollow` | 0 | 1 | 0 bytes |
    | complexity-swift | `src/Notes.txt` | 0 | 1 | 0 bytes |
    | magic-numbers-swift | `src/Absent.swift` | 1 | 0 | 0 bytes |
    | magic-numbers-swift | `hollow` | 0 | 1 | 0 bytes |
    | magic-numbers-swift | `src/Notes.txt` | 0 | 1 | 0 bytes |
    | missing-docs-swift | `src/Absent.swift` | 1 | 0 | 0 bytes |
    | missing-docs-swift | `hollow` | 0 | 1 | 0 bytes |
    | missing-docs-swift | `src/Notes.txt` | 0 | 1 | 0 bytes |

    Over `src/Absent.swift` each script wrote only
    `<rule-name> cannot read src/Absent.swift` to stderr and ran swiftlint 0
    times. That confirms finding 2: the `[ ! -r "$file" ]` guard stands before
    `mktemp -d` and before each swiftlint call, so the script never gives that
    path to swiftlint. `hollow` and `src/Notes.txt` pass the guard, so the
    script does give each of those to swiftlint.

    What the edit does.

    Finding 1. `complexity-swift.md` gets the row
    `| a path that holds no file | 1 | 0 bytes |`, which
    `magic-numbers-swift.md` and `missing-docs-swift.md` each hold.

    Finding 2. The sentence in all 3 files now names the directory `hollow`,
    which holds no Swift file. Each of the 3 tables gets the row
    `| the directory \`hollow\`, which holds no Swift file | 1 | 0 bytes |`, so
    the table of each file holds a row for the run its own sentence names.

    Byte counts. Each count this edit adds is 0 bytes, measured. The edit states
    no other byte count. Card ^y81mb7n owns the byte-count question and this
    round leaves it as it was.
  timestamp: 2026-08-11T12:04:53.824287+00:00
- actor: claude-code
  id: 01kzrbfn44k8y829jzskfgw2rc
  text: |
    ### implement — changed
    - evidence: 3 files —
      `builtin/validators/code-hygiene/rules/complexity-swift.md`,
      `builtin/validators/code-hygiene/rules/magic-numbers-swift.md`,
      `builtin/validators/code-hygiene/rules/missing-docs-swift.md`.
      14 added lines, 8 removed lines. 0 of the 14 added lines is more than 79
      characters. The 14 lines hold 0 frequency words, from the set usually,
      often, always, generally, normally, typically, sometimes, rarely, mostly,
      may, might, should, probably, simply, just, basically. The edit changes
      prose only: 0 script line and 0 front matter line moved.
    - finding 1: `complexity-swift.md` gets the row
      `| a path that holds no file | 1 | 0 bytes |`. Measured: status 1, 0
      stdout bytes.
    - finding 2: the sentence in each of the 3 files now names the directory
      `hollow`, which holds no Swift file. The script gives that directory to
      swiftlint: measured 1 swiftlint call, exit 0, 0 stdout bytes, in 3 of 3
      rules. Each of the 3 tables gets the row
      `| the directory \`hollow\`, which holds no Swift file | 1 | 0 bytes |`.
    - the cause is gone from the whole tree:
      `grep -rn "run over a path that holds no file" --include='*.md' .` gives
      0 hits outside `.kanban/`.
    - tests: `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then
      `cargo nextest run -p swissarmyhammer-validators`: 619 tests run, 619
      passed, 0 failed, 0 skipped.
    - card state: 0 open findings of 24, column `doing`.
    - next: `/review`.
  timestamp: 2026-08-11T12:06:47.172966+00:00
- actor: claude-code
  id: 01kzrc23xze48vtjm066vchaq7
  text: |-
    ### review — findings
    - scope: `review sha HEAD~1..HEAD`, commit 40a094908. 3 files, 14 added lines, 9 removed lines. 4 of the added lines are table rows, 10 are prose.
    - engine: the `review` engine attempted 0 checks and gave 0 findings. The 3 changed files are rule `.md` files, and the engine holds no validator for them. The findings below come from measurement.
    - measurement, swiftlint 0.65.0, 9 runs with the child configuration of each rule: `a path that holds no file` gives status 1 and 0 stdout bytes in 3 of 3 rules; the directory `hollow` gives status 1 and 0 stdout bytes in 3 of 3 rules. An empty `hollow` and a `hollow` that holds 1 non-Swift file give the same status and the same 0 bytes.
    - guard: the operator is `-r`, not `-f`. `[ ! -r "$file" ]` admits a readable directory and stops a path that holds no file. Each of the 3 scripts holds 1 guard and 2 swiftlint call sites, and 2 of 2 call sites stand after the guard. So the corrected sentence names a run the script makes.
    - tests: `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then `cargo nextest run -p swissarmyhammer-validators` gives 619 run, 619 passed, 0 failed, 0 skipped, 0 compiler warnings.
    - prose: 0 of the 14 added lines is more than 79 characters. The added lines hold 0 hedge words.
    - byte counts: 2 counts came 1 byte over the table. A control run with a path 1 character shorter gave the stated count in both. Path length explains both, so this round reports no finding for them. Card ^y81mb7n owns that question.
    - finding 1: `complexity-swift.md:184` closes the lead-in with `over one file holding one function of cyclomatic complexity 16:`. That clause is false for 3 of the 6 rows, and the commit adds 2 of the 3.
    - finding 2: `complexity-swift.md:195` and the same row in the other 2 files have 0 acceptance tests. `grep -rn 'hollow' crates/ --include='*.rs'` gives 0 hits, and the shape the commit takes out of the sentence holds 3 tests.
    - evidence: 2 findings — builtin/validators/code-hygiene/rules/complexity-swift.md:184, builtin/validators/code-hygiene/rules/complexity-swift.md:195
    - next: correct the lead-in of `complexity-swift.md`, and add 1 acceptance test per rule over a directory that holds no Swift file.
  timestamp: 2026-08-11T12:16:52.159221+00:00
- actor: claude-code
  id: 01kzrca01espkx7nve63dd12x1
  text: |-
    ### finish iteration 8 — findings
    - implement: changed — 3 files, 14 added lines, 8 removed; `complexity-swift.md` gained the row for a path that holds no file, and each of the three files now shows the status-1 limit with a directory that holds no Swift file, a run the script makes
    - test: green — cargo nextest run -p swissarmyhammer-validators, 619 passed, 0 skipped, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`
    - commit: 40a094908 — 3 files, 14 insertions, 9 deletions
    - review: findings — 2 items, both in `complexity-swift.md`. Line 184: the lead-in ends "over one file holding one function of cyclomatic complexity 16:", which gives the whole table one input, and the clause is false for 3 of the 6 rows. The two sibling files close the same sentence with wording that names no input. Line 195: the hollow-directory run has 0 acceptance tests, where the shape it replaced holds 3, one for each rule, at `complexity.rs:656`, `magic_numbers.rs:635` and `missing_docs.rs:1452`.
    - guardrail: 8 rounds, each with new findings. No finding repeats. The guardrail is not armed.
    - next: give `complexity-swift.md` a lead-in that names no single input, and add one acceptance test for each rule over a directory that holds no Swift file.
  timestamp: 2026-08-11T12:21:10.318718+00:00
- actor: claude-code
  id: 01kzrd5mrdv757952yv88c8dvq
  text: |-
    Measurement on this machine, before any number was written. `swiftlint version` gives 0.65.0.

    The child configuration of each of the 3 rules was written out word for word, and swiftlint was run against 3 targets: an empty directory `hollow_empty`, a directory `hollow_notes` that holds 1 file named `Notes.txt`, and a directory named `Sources/Hollow.swift` that holds 1 file named `Notes.txt`. 9 runs in all.

    Each of the 9 runs gives status 1, writes 0 bytes to stdout, and writes `Error: No lintable files found at paths: '<target>'` to stderr. The name of the directory moves no number. So the table row `| the directory hollow, which holds no Swift file | 1 | 0 bytes |` holds for a directory whose name ends in `.swift` as well.

    That last target is the shape the acceptance tests use. The rule matches a path with `**/*.swift`, so a directory named `hollow` reaches no run at all. A directory named `Sources/Hollow.swift` reaches the run, passes the `[ ! -r "$file" ]` guard, and goes to swiftlint.

    No number in the 3 rule bodies was changed. The commit adds 12 prose lines and changes 3. 0 script lines and 0 front matter lines changed, which `git diff` shows.

    Card ^y81mb7n owns the stdout byte-count question. This round writes no byte count and changes none.
  timestamp: 2026-08-11T12:36:16.269198+00:00
- actor: claude-code
  id: 01kzrd66cz434ebqzw3cwqwhxk
  text: |-
    Both findings are closed.

    Finding 1, the table lead-in of `complexity-swift.md`. The clause `over one file holding one function of cyclomatic complexity 16:` gave the whole table 1 input, and it was false for 3 of the 6 rows. The lead-in now reads `The probe file holds one function of cyclomatic complexity 16. Each status swiftlint 0.65.0 answers with was measured against the child configuration this script writes:`. The second sentence is the wording `magic-numbers-swift.md` and `missing-docs-swift.md` close the same sentence with, and it names no input. The first sentence defines the probe file, which 3 rows of the table name. The cause is off the whole table, and no row was edited.

    Finding 2, the 3 acceptance tests. One new shape stands in `shipped.rs`: `ShippedHollowDirectory` and `verify_shipped_hollow_directory_answers_clean`. It stages a directory, asserts the staged path IS a directory, plans a run over it, and holds the run to 0 errors and 0 findings. Each rule then holds one probe and one test, beside the test for the path that holds no file:
    - `complexity.rs` — `the_shipped_swift_complexity_tool_rule_stays_clean_over_a_hollow_directory`
    - `magic_numbers.rs` — `the_shipped_swift_magic_numbers_tool_rule_stays_clean_over_a_hollow_directory`
    - `missing_docs.rs` — `the_shipped_swift_missing_docs_tool_rule_stays_clean_over_a_hollow_directory`

    The directory is `Sources/Hollow.swift`, and it holds 1 file, `Notes.txt`. The name ends in `.swift` because the rule matches a path by that suffix. A directory named `hollow` reaches no run, so a test over that name would measure nothing the engine does.

    RED to GREEN, proved with the real tool. The mutation is the guard operator: `[ ! -r "$file" ]` became `[ ! -f "$file" ]` in each of the 3 rule bodies. `-f` refuses a directory, so the guard stops the run. `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then `cargo nextest run -p swissarmyhammer-validators -E 'test(stays_clean_over_a_hollow_directory)'` gave 3 run, 0 passed, 3 failed. Each failure reads `a directory the tool reads must break no run; errors: [ToolRunError { ... detail: "<rule> cannot read Sources/Hollow.swift" }]`. The 3 guards were then restored, and the same command gave 3 run, 3 passed. So each test holds the `-r` guard and the stderr branch, and each one can fail.

    The 3 rule bodies each name the new test in the paragraph that states the 4 stderr shapes, which is the pattern each other behaviour of those files keeps. The test names carry `stays_clean` rather than `answers_clean` to hold each name line under the 79-character limit the card states.

    The rules the review engine holds for a `.rs` file were read whole before the code was written, through `dump validators`.
  timestamp: 2026-08-11T12:36:34.335512+00:00
- actor: claude-code
  id: 01kzrd6eb2dpe89n6wqwebw7qe
  text: |-
    ### implement — changed
    - evidence: 7 files — builtin/validators/code-hygiene/rules/complexity-swift.md, builtin/validators/code-hygiene/rules/magic-numbers-swift.md, builtin/validators/code-hygiene/rules/missing-docs-swift.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/magic_numbers.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs. `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then `cargo nextest run -p swissarmyhammer-validators` gives 622 run, 622 passed, 0 failed, 0 skipped. `cargo fmt --all` and `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` give 0 warnings. 2 of 2 findings checked. 12 added rule-body lines, the longest 79 characters, 0 frequency words.
    - next: /review
  timestamp: 2026-08-11T12:36:42.466621+00:00
- actor: claude-code
  id: 01kzrebhttzrbhf8dnpyxpcd9n
  text: |
    ### review — findings

    - evidence: The engine ran `review sha HEAD~1..HEAD` over commit 2bb04d62f. It attempted 16 checks. It confirmed 2, and it refuted 3. It failed 0, and it skipped 0. The 2 open items are `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:482` and `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity.rs:664`. The 8 earlier passes hold 26 items, and all 26 are marked done. The card holds 9 Review Findings sections and 2 open items.
    - next: Do the 2 open items. Both items are about code this commit adds, so the test-refactor exception does not release them. The diff adds `struct ShippedHollowDirectory`, and `ShippedFailFixture` was in the file before. The cause of item 2 is in 3 files, and the finding names 2: `complexity.rs:664`, `magic_numbers.rs:666`, and `missing_docs.rs:1485` each hold the value `"Sources/Hollow.swift"`. 3 more constants hold the same staged-file pair at `complexity.rs:668`, `magic_numbers.rs:670`, and `missing_docs.rs:1489`. The card stays in review.
  timestamp: 2026-08-11T12:56:58.458362+00:00
- actor: claude-code
  id: 01kzree5p3aqrcaynvv74bfh1p
  text: |-
    ### finish iteration 9 — findings
    - implement: changed — 7 files; the `complexity-swift.md` table lead-in names no single input, and three acceptance tests hold the run over a directory that holds no Swift file, one for each rule, on a shared `ShippedHollowDirectory` shape. RED to GREEN: the guard mutated to `[ ! -f "$file" ]` gave 3 failures with `<rule> cannot read Sources/Hollow.swift`, and the shipped guard gives 3 passes.
    - test: green — cargo nextest run -p swissarmyhammer-validators, 622 passed, 0 skipped; fmt and clippy 0 warnings
    - commit: 2bb04d62f — 7 files, 213 insertions, 5 deletions
    - review: findings — 2 items, both on code this commit adds. The engine attempted 16 checks, confirmed 2 and refuted 3. `tests/shipped.rs:482`: `ShippedHollowDirectory` is a near-duplicate of `ShippedFailFixture` at line 125, 48 tokens and 93 percent alike. `tests/shipped/complexity.rs:664`: `SWIFT_COMPLEXITY_HOLLOW_PATH` holds `"Sources/Hollow.swift"`, and `magic_numbers.rs` and `missing_docs.rs` hold the same value under their own names — the constant belongs once in `shipped.rs` beside the other shared Swift constants.
    - guardrail: 9 rounds, each with new findings. No finding repeats. The guardrail is not armed.
    - next: fold the new shape into the shape it duplicates, and move the path constant into `shipped.rs`.
  timestamp: 2026-08-11T12:58:24.323129+00:00
- actor: claude-code
  id: 01kzrf40jf3fp00r511pjy866h
  text: |-
    Both findings of the 2026-08-11 07:38 pass are closed.

    Finding 1, the near-duplicate struct. `ShippedHollowDirectory` is gone. Its 6 fields were the 6 fields `ShippedBrokenRun` already held, under other names: `directory` is a `path`, `staged` is `support`, and a probe that writes no file at the path states `source: None`. `ShippedBrokenRun` is now `ShippedNamedPath` — one path the work-list names, what the probe stages around it, and what the run over that path must answer — and the two verifier functions state which answer each holds the run to. The name had to change, because the shape now carries a run that must NOT break. The 9 probes in `complexity.rs`, `magic_numbers.rs` and `missing_docs.rs` carry the new name.

    The two verifiers held the same 14 lines of setup, and after the merge those 14 lines were the same text. They now call one function, `drive_shipped_named_path`, which stages the repository, plans the run, and executes it. It answers the probe repository beside the outcome, because `TempDir` removes the tree as it drops and the hollow test reads the staged tree. The tail the hollow verifier shared with `verify_shipped_staged_positions_report` — no error, and the reported files equal `expected` — is now `assert_shipped_run_reports`, which both call.

    Finding 2, the constant in three files. `shipped.rs` now holds `SWIFT_HOLLOW_PATH`, `SWIFT_HOLLOW_FILES` and `SWIFT_HOLLOW_PURPOSE`, beside `SWIFT_PROJECT_TYPES` and `SWIFT_EXCLUDE_POSITIONS`. The finding named 2 files and the cause stood in 3, so all 3 rule files lost their own copies. `grep -rn '"Sources/Hollow.swift' crates/` gives 2 hits, both in `shipped.rs`: the path, and the file inside it. The `reason` string, which the 3 probes also held word for word, is off the probes: `ShippedNamedPath` carries no `reason` field, and the hollow verifier states the one message itself.

    RED to GREEN, proved with the real tool after the refactor. The mutation is the guard operator: `[ ! -r "$file" ]` became `[ ! -f "$file" ]` in each of the 3 rule bodies. `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then `cargo nextest run -p swissarmyhammer-validators -E 'test(stays_clean_over_a_hollow_directory)'` gave 3 run, 0 passed, 3 failed, each with `the shipped pipeline must not break; errors: [ToolRunError { ... detail: "<rule> cannot read Sources/Hollow.swift" }]`. The 3 guards were then restored with `git checkout --`, and the same command gave 3 run, 3 passed. So the merged shape still holds the guard and the stderr branch, and each test can fail.

    No rule body and no front matter changed. `git status --porcelain -- builtin` is empty.
  timestamp: 2026-08-11T13:10:19.983778+00:00
- actor: claude-code
  id: 01kzrf47acw4xdgxksww6ga4jw
  text: |-
    ### implement — changed
    - evidence: 4 files — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, .../tests/shipped/complexity.rs, .../tests/shipped/magic_numbers.rs, .../tests/shipped/missing_docs.rs. `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then `cargo nextest run -p swissarmyhammer-validators` gives 622 run, 622 passed, 0 failed, 0 skipped. The 3 named tests, run with `-E 'test(stays_clean_over_a_hollow_directory)'`, give 3 run, 3 passed. `cargo fmt --all` and `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` give 0 warnings. 2 of 2 findings checked. `"Sources/Hollow.swift"` stands in 1 file, and `ShippedHollowDirectory` in 0.
    - next: /review
  timestamp: 2026-08-11T13:10:26.892398+00:00
- actor: claude-code
  id: 01kzrfxq42t4ge6x5xk057fpwk
  text: |-
    ### review — clean
    - evidence: the engine read `HEAD~1..HEAD` (70698eeca). It attempted 16 checks, confirmed 1 and refuted 10. Measurement closes the 1 confirmed check, so 0 findings stay open. The description holds 28 checked findings and 0 unchecked findings over 9 dated sections. `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then `cargo nextest run -p swissarmyhammer-validators` gives 622 run, 622 passed, 0 failed, 0 skipped.
    - next: none. The card moves to done.

    The engine gave 1 confirmed finding, on `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:1491`. It states that `SWIFT_HOLLOW_PROBE` reads `SWIFT_HOLLOW_PURPOSE` and `SWIFT_HOLLOW_PATH`, that the file does not define the two constants, and that no file defines them. It gives two answers: (1) define the 3 constants in `shipped.rs` and let `use super::*` carry them to the probes, or (2) write the values into each probe.

    Answer (1) is the state of the code now. The measurements:

    - `shipped.rs:765` holds `const SWIFT_HOLLOW_PATH`, `shipped.rs:770` holds `const SWIFT_HOLLOW_FILES`, and `shipped.rs:773` holds `const SWIFT_HOLLOW_PURPOSE`. `grep -c "const SWIFT_HOLLOW_" shipped.rs` gives 3.
    - `shipped.rs` declares the 3 child modules at lines 13, 15 and 16. Each child file holds `use super::*;` at line 7: `shipped/complexity.rs:7`, `shipped/magic_numbers.rs:7` and `shipped/missing_docs.rs:7`.
    - The 3 probes read the constants at `shipped/complexity.rs:670-673`, `shipped/magic_numbers.rs:672-675` and `shipped/missing_docs.rs:1491-1494`. A search of the crate source for the 3 names gives 12 hits in 4 files: 3 definitions and 9 uses.
    - Rust stops a build on a constant that no scope defines. The build completed and 622 of 622 tests passed. So all 3 constants resolve.

    Answer (2) writes the 3 values into 3 files again. The finding of 2026-08-11 07:38 asked for the opposite, and this card closed it. So answer (1) is the only answer that agrees with the closed findings.

    The merge of the two shapes holds. `ShippedHollowDirectory` stands in 0 files. `ShippedNamedPath` stands at `shipped.rs:416`, `drive_shipped_named_path` at `shipped.rs:459` and `assert_shipped_run_reports` at `shipped.rs:386`. Each of the 3 stands one time.

    This pass reports no byte-count difference. Card ^y81mb7n owns the byte counts in the Swift status tables.
  timestamp: 2026-08-11T13:24:22.274793+00:00
- actor: claude-code
  id: 01kzrfzcp4emgnk8qvk0bhpgh0
  text: |-
    ### finish iteration 10 — clean
    - implement: changed — 4 test files; `ShippedHollowDirectory` folded into the shape it duplicated, which is now `ShippedNamedPath`; the two verifiers share `drive_shipped_named_path` and `assert_shipped_run_reports`; the Swift hollow-path constants stand once in `shipped.rs`, and all three rule test files lost their copies
    - test: green — cargo nextest run -p swissarmyhammer-validators, 622 passed, 0 failed, 0 skipped; the three hollow-directory tests pass; fmt and clippy 0 warnings
    - commit: 70698eeca — 4 files, 119 insertions, 144 deletions
    - review: clean — 16 checks attempted, 1 confirmed, 10 refuted. The one confirmed check asked for the constants to stand once in `shipped.rs` and reach the children through `use super::*`, which is the state of the code: `shipped.rs:765`, `:770` and `:773` hold the three, each child holds `use super::*` at line 7, and the crate compiles and runs 622 tests.
    - The card moved to `done`. 28 findings of 28 are closed over 10 rounds.
  timestamp: 2026-08-11T13:25:17.124885+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffe480
title: missing-docs-swift discards the project .swiftlint.yml, so generated Swift is reported
---
`builtin/validators/code-hygiene/rules/missing-docs-swift.md` runs `swiftlint` with `only_rules: [missing_docs]` from a temporary config, and declares `supersedes: [missing-docs]`.

`missing-docs.md` exempts "Generated code". The temp config exists so the rule "never reads the project's own `.swiftlint.yml`", which is where a project holds the `excluded:` list for generated Swift. A changed generated public API file is reported in full.

Two carve-outs ARE reproduced. Private and internal items, because swiftlint's `missing_docs` defaults to `[open, public]`. Obvious implementations, mostly, because `excludes_extensions: true` and `excludes_inherited_types: true` cover protocol conformances written in extensions and inherited members — and the prompt rule's closing note withdraws that carve-out for Swift anyway.

The test carve-out holds only incidentally: XCTest classes and `func test...` methods are conventionally internal. A `public final class TestSupport` in a changed test file still reports.

The same discarded-`excluded:` defect stands on `complexity-swift` and `magic-numbers-swift`. One answer may serve all three: give the temp config an `excluded:` list of its own, or merge the project's.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity

## Review Findings (2026-08-11 03:02)

The engine reviewed `HEAD~1..HEAD` (09cf60b11). It attempted 16 checks, confirmed 0 and refuted 2. Measurement with swiftlint 0.65.0 gave the findings below.

- [x] `builtin/validators/README.md:221` — The sentence "The project's own configuration is still never read." is still in the file. The new bullet that starts at line 222 says a script MAY read the project configuration. The two sentences do not agree. The commit message says this sentence is now false. The README diff adds 16 lines and removes no line. Remove the false sentence, or write the limit of the sentence into the sentence.
- [x] `builtin/validators/code-hygiene/rules/complexity-swift.md:28` — The child configuration does not state the `error` option of `cyclomatic_complexity`, and it does not state the `error` option of `function_body_length`. `swiftlint rules cyclomatic_complexity` names `warning`, `error` and `ignores_case_statements`. `swiftlint rules function_body_length` names `warning` and `error`. The new README bullet at line 231 tells a script to state EVERY option of EVERY rule it measures with.
- [x] `builtin/validators/code-hygiene/rules/complexity-swift.md:32` — A project that uses the swiftlint `child_config:` key breaks all three shipped rules. Measured with `child_config: other.yml` in the project file: swiftlint stops with "There's an ambiguity in the child / parent configuration tree: More than one parent is declared" and aborts. Each of the three scripts then exits 1, and the engine reads exit 1 as a broken tool. The same invocation is at `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:29` and at `builtin/validators/code-hygiene/rules/missing-docs-swift.md:32`.
- [x] `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:26` — The child configuration states `allowed_numbers` only. `swiftlint rules no_magic_numbers` names `severity`, `test_parent_classes` and `allowed_numbers`. The new README bullet at line 231 tells a script to state EVERY option of EVERY rule it measures with.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-swift.md:219` — The sentence "So a public getter and a public setter each need a doc comment." is more wide than the measurement. Measured with the shipped script over one file: `public var value: Int { 1 }` and `public func setValue(_ next: Int)` in `public struct Plain` each report. The same two items in `public struct Wide: Equatable` report nothing. `builtin/validators/code-hygiene/rules/missing-docs.md` states "Swift stays silent for every getter in a type that declares an inherited type." Write the inherited-type condition into the sentence.

### What the measurement confirms

A project cannot make the gate more weak. Measured with swiftlint 0.65.0 against each of the three shipped scripts, with a hostile project `.swiftlint.yml`. Baseline counts with no project file: docs 3, magic numbers 1, complexity 1. Each lever below gave the same counts as the baseline:

- `disabled_rules:` that names each measured rule.
- `only_rules:` and `whitelist_rules:` that name an unrelated rule.
- `opt_in_rules: []` with `disabled_rules:`.
- A permissive option block for each measured rule: `missing_docs: {warning: []}`, `no_magic_numbers: {allowed_numbers: [42.0]}`, `no_magic_numbers: {test_parent_classes: ["Base"]}`, `cyclomatic_complexity: {warning: 500, error: 900}`, `function_body_length: {warning: 500, error: 900}`.
- A severity change, in both directions.
- `included:` that names another directory.

Only `excluded:` changed the counts, to 0. That is the behaviour the card asks for.

The child block of a rule replaces the parent block whole. A control run proves the probe: when the CHILD states `test_parent_classes: ["Base"]`, the count goes to 0. When the PARENT states the same value and the child leaves it unstated, the count stays 1. So an unstated option takes swiftlint's own default, and never the project's value. The two `error` options were unstated at the time of this measurement, and were safe for the same reason: `warning:` alone turns the error level off. Measured over a 150-line body against `warning: 250`: the run reports nothing. Over a 300-line body: the run reports one finding. Finding 2 asked for both options to be stated, and the fix states each one at the same number as its `warning:`, which keeps both counts.

## Review Findings (2026-08-11 03:45)

The engine reviewed `HEAD~1..HEAD` (1cc990488). It attempted 16 checks and confirmed 0. `cargo nextest run -p swissarmyhammer-validators` gave 614 tests and 614 passed, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`. Measurement with swiftlint 0.65.0 against the three shipped scripts gave the finding below.

- [x] `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:54` — A project switches this gate off with the `warning_threshold:` key. swiftlint exits 2 when the count of warnings is more than `warning_threshold`, and it writes the full JSON report to stdout. This script tests `if [ "$status" -ne 0 ]`, so it exits 1 and reports no finding. Measured over one file that holds `return status == 404`: with no project file the script reports 1 finding and exits 0. With a project file that holds `warning_threshold: 1` the script reports 0 findings and exits 1. The engine reads exit 1 as a broken tool, which is the same failure mode as the `child_config:` shape. This commit closed that failure mode for `complexity-swift.md:56`, which now tests `[ "$status" -ne 0 ] && [ "$status" -ne 2 ]` and keeps its count of 1 beside the same project file. Give this script the same test, and write the measurement into the rule body.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-swift.md:55` — A project switches this gate off with the `warning_threshold:` key. The cause is the same as the item above: this script tests `if [ "$status" -ne 0 ]`, and swiftlint exits 2 with a full JSON report when the count of warnings is more than `warning_threshold`. Measured over one file that holds `public struct Thing` and one undocumented stored property: with no project file the script reports 2 findings and exits 0. With a project file that holds `warning_threshold: 1` the script reports 0 findings and exits 1. Give this script the same test as `complexity-swift.md:56`, and write the measurement into the rule body.

### What the measurement confirms

Measured with swiftlint 0.65.0. The scripts were copied out of the three rule files and run without a change.

Baseline counts with no project `.swiftlint.yml`: complexity 1, magic numbers 1, docs 2. Each lever below gave the baseline count for each of the three scripts, at script exit 0:

- `disabled_rules:` that names each measured rule.
- `only_rules:` and `whitelist_rules:` that name an unrelated rule.
- `opt_in_rules: []` with `disabled_rules:`.
- A permissive option block for each measured rule.
- A severity change, in both directions.
- `included:` that names another directory.
- `parent_config: other.yml`, where `other.yml` names `disabled_rules:`.
- `child_config: other.yml` — the first abort shape. The script writes one line to stderr that names the drop, runs a second time with its own configuration alone, and keeps the baseline count.
- `child_config: other.yml` with `excluded:` that names the source directory. The count stays at the baseline, so the exclude list is dropped for the second run.
- Bytes that are not YAML — the second abort shape. Same result as the shape above.
- A YAML list where swiftlint expects a map.
- An empty file.
- `strict: true`.
- `analyzer_rules:`.
- `reporter: xcode`.

`excluded:` that names the source directory, and `excluded:` that names `.`, each gave 0 findings at script exit 0. That is the behaviour the card asks for.

`warning_threshold: 1` gave 0 findings at script exit 1 for the magic-numbers script and for the missing-docs script. It gave the baseline count of 1 at script exit 0 for the complexity script. The two findings above state that result.

### Status 2 is told apart from a broken run

swiftlint 0.65.0 returns these statuses. Each shape was run with the child configuration of the complexity rule:

| what the run is | status |
|---|---|
| a clean run over a real file | 0 |
| a run with a finding of error severity | 2 |
| a configuration that names a rule that does not exist | 0, with a warning on stderr |
| a configuration option of the wrong type | 0, with a warning on stderr |
| a Swift source file that does not parse | 0, with an empty JSON list |
| a path that does not exist | 1 |
| a command-line option that does not exist | 64 |
| a configuration path that does not exist | 134 |
| a project configuration that holds `child_config:` | 134 |

No broken run gives status 2. Status 2 comes only from a measured run, and that run writes the full JSON report to stdout. The test `[ "$status" -ne 0 ] && [ "$status" -ne 2 ]` at `complexity-swift.md:56` therefore does not read a broken run as a measured run.

### Every option of every measured rule is stated

`swiftlint rules <name>` names these options. Each child configuration states each one:

| rule | options swiftlint names | options the child states |
|---|---|---|
| `missing_docs` | `warning`, `excludes_extensions`, `excludes_inherited_types`, `excludes_trivial_init`, `evaluate_effective_access_control_level` | all five |
| `no_magic_numbers` | `severity`, `test_parent_classes`, `allowed_numbers` | all three |
| `cyclomatic_complexity` | `warning`, `error`, `ignores_case_statements` | all three |
| `function_body_length` | `warning`, `error` | both |

### The claims in the changed files

Each claim below was measured again and holds:

- The README sentence "The script writes the configuration of that tree itself, and it copies no configuration of the project's own into the tree." The two temporary-package scripts write `pubspec.yaml` and `analysis_options.yaml` into a temporary directory, and they copy Swift and Dart sources only.
- The three rows of the gate table in `complexity-swift.md`. Measured over bodies of 120, 150, 200, 260 and 300 code lines: `error: 100` reports at 120 lines where `error: 250` and `warning: 250` alone each stay silent, so `error` at swiftlint's own default moves the gate from 250 to 100. `error: 250` keeps the count of `warning: 250` alone at each body, and makes the finding error severity.
- `error:` with no value. swiftlint answers `Invalid configuration for 'function_body_length' rule. Falling back to default.` and measures against `warning: 50`.
- The getter and setter bullet in `missing-docs-swift.md`. Measured: `public struct Plain` reports 3 findings; `public struct Wide: Equatable` reports 0; `public class Sub: Base` reports 0; `public struct P: SomeProtocol` reports 0. A computed property with an explicit `get` and `set` block gives the same result.
- The abort table rows and the stderr line each script writes on the retry path.
- `builtin/validators/code-hygiene/rules/missing-docs.md` is not made false. Each statement it makes about the Swift rule matches the measurements above.

## Review Findings (2026-08-11 04:09)

The engine reviewed `HEAD~1..HEAD` (c2beb6ec9). It attempted 8 checks, confirmed 0 and refuted 0. `cargo nextest run -p swissarmyhammer-validators` gave 616 tests and 616 passed, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`. Both new acceptance tests pass. A trust-boundary measurement of 36 project levers against the three shipped scripts, with swiftlint 0.65.0, gave the findings below.

- [x] `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:54` — A project switches this gate off with the `swiftlint_version:` key. The new status test reads a broken run as a measured run. swiftlint compares `swiftlint_version:` with the version that is installed. At a difference it writes `warning: Currently running SwiftLint 0.65.0 but configuration specified version <v>.` to stderr, writes 0 bytes to stdout, and exits 2. The script now accepts status 2, so `jq` reads an empty report, and the script reports 0 findings and exits 0. The engine reads that as a clean file. Measured over one file that holds `return status == 404`: no project file gives 1 finding; `swiftlint_version: 0.65.0` gives 1 finding; `swiftlint_version: 0.64.0`, `swiftlint_version: 99.0.0` and `swiftlint_version: 0.1.0` each give 0 findings at exit 0. Any value that is not the installed version does this. The status test before this commit gave exit 1 for the same project file, so the engine read a broken tool and did not read a clean file. Tell a run that measures apart from a run that breaks by more than the status: a run at status 2 that writes an empty stdout is a broken run.
- [x] `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:261` — The sentence "Status 2 states a measured run, and no broken run states it." is false. A project `.swiftlint.yml` that holds `swiftlint_version:` with a version that is not installed makes swiftlint exit 2 with an empty stdout, and no lint runs. The table that follows does not hold this shape, and its closing sentence at line 278, "So the script measures at status 0 and at status 2, and it breaks at every other status.", is false for the same reason. The sentence at line 280, "A finding of error severity is the other shape that makes swiftlint exit 2", names two shapes where the measurement gives three. Write the version-mismatch shape into the table, and write each sentence to the measurement.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-swift.md:55` — A project switches this gate off with the `swiftlint_version:` key. The cause is the same as the magic-numbers item above. Measured over one file that holds `public struct Thing` and one undocumented stored property: no project file gives 2 findings; `swiftlint_version: 0.65.0` gives 2 findings; `swiftlint_version: 0.64.0`, `swiftlint_version: 99.0.0` and `swiftlint_version: 0.1.0` each give 0 findings at exit 0.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-swift.md:218` — The sentence "Status 2 states a measured run, and no broken run states it." is false. The cause is the same as the magic-numbers item above. The closing sentence at line 235 and the sentence at line 237 are false for the same reason.
- [x] `builtin/validators/code-hygiene/rules/complexity-swift.md:56` — The same cause stands in this file, which the commit 1cc990488 gave the status-2 test. Measured over one file that holds a function body of more than 250 code lines: no project file gives 1 finding; `swiftlint_version: 99.0.0` gives 0 findings at exit 0. A finding shows one example of a cause, and the cause must leave all three files.

### What the measurement confirms

Measured with swiftlint 0.65.0. The three scripts were copied out of the rule files and run without a change. Fixtures: one file that holds `return status == 404` (magic numbers, baseline 1); one file that holds `public struct Thing` with one undocumented stored property (docs, baseline 2); one file that holds a function body of more than 250 code lines (complexity, baseline 1).

36 project levers were run against each of the three scripts. Each lever below gave the baseline count at script exit 0:

- `warning_threshold:` at 0, 1, 2, 3, 5, -1, a value that is not a number, and 1 together with `strict: true`.
- `strict: true`. `lenient: true`.
- `baseline: base.json`, where `base.json` was written by `swiftlint --write-baseline` and holds every violation of the three fixtures. `write_baseline: out.json`.
- `allow_zero_lintable_files: true`. `check_for_updates: true`. `cache_path:`. `reporter: xcode`.
- `disabled_rules:` that names each measured rule.
- `only_rules:` that names an unrelated rule. `opt_in_rules: []` with `disabled_rules:`.
- A permissive option block for each measured rule. A severity change to `error`.
- `included:` that names another directory. `excluded:` that names an unrelated directory.
- `parent_config:` and `child_config:` that each name a file which disables the measured rules.
- Bytes that are not YAML. A YAML list where a map belongs. An empty file. A key that swiftlint does not know.
- A nested `.swiftlint.yml` inside the source directory that holds `excluded: ["."]` and `disabled_rules:`. swiftlint does not read a nested file when the run states `--config`.

`excluded:` that names the source directory gave 0 findings at script exit 0 for each script. That is the behaviour the card asks for.

`swiftlint_version:` with a version that is not installed gave 0 findings at script exit 0 for each of the three scripts. The five findings above state that result.

### The warning-threshold change holds

A control run proves the change of this commit. The status test of each script was put back to `if [ "$status" -ne 0 ]` on a copy, and each copy was run beside the same fixtures:

| project file | script | old test | new test |
|---|---|---|---|
| none | magic numbers | 1 finding, exit 0 | 1 finding, exit 0 |
| none | docs | 2 findings, exit 0 | 2 findings, exit 0 |
| `warning_threshold: 1` | magic numbers | 0 findings, exit 1 | 1 finding, exit 0 |
| `warning_threshold: 1` | docs | 0 findings, exit 1 | 2 findings, exit 0 |

The first table of each new rule section holds. `warning_threshold:` at every value from 0 to 5, and at -1, keeps the baseline count for each of the three scripts. Neither new rule section holds a frequency word.

Each other claim of the two new sections was measured and holds. The second table of each section matches row for row, and each "empty" row writes 0 bytes and not `[]`. The boundary sentence "At that number, and over it" is true: at a count of 1 the magic-numbers run exits 2 at `warning_threshold: 0` and at `warning_threshold: 1`, and exits 0 at `warning_threshold: 2`; at a count of 2 the docs run exits 2 at 1 and at 2, and exits 0 at 3. The added entry carries `rule_id` of exactly `warning_threshold` and severity `Error`, so the `jq` filter keeps it out. The error-severity claim of each section holds in both directions.

Both acceptance tests run and pass: `the_shipped_swift_magic_numbers_tool_rule_measures_beside_a_project_warning_threshold` and `the_shipped_swift_missing_docs_tool_rule_measures_beside_a_project_warning_threshold`. Each asserts the count its rule body states — one entry for magic numbers, two for docs.

## Review Findings (2026-08-11 04:45)

The engine reviewed `HEAD~1..HEAD` (92dd40b67). It attempted 16 checks, confirmed 0 and refuted 0. `cargo nextest run -p swissarmyhammer-validators` gave 619 tests and 619 passed, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`. All three new acceptance tests pass. A trust-boundary measurement of 82 project levers, in 246 script runs, against the three shipped scripts, with swiftlint 0.65.0, gave the findings below. The gate itself holds. Each finding is a false sentence.

- [x] `builtin/validators/code-hygiene/rules/complexity-swift.md:190` — The sentence "It breaks at every other status, and it breaks at status 2 with a report of 0 bytes." is false. The script does not break at status 1. Each of the three scripts holds this branch after the status gate: `if grep -qF 'No lintable files found' "$work/lint.err"; then exit 0; fi`. Measured with a project `.swiftlint.yml` that holds `excluded: [src]`, over the dirty magic-numbers fixture: swiftlint writes `Error: No lintable files found at paths: 'src/Magic.swift'` to stderr, writes 0 bytes to stdout, and exits 1; the script reports 0 findings and exits 0. That branch is how the `excluded:` behaviour this card asks for reaches exit 0, so the branch must stay and the sentence must state it. Every one of the 9 `excluded:` levers measured reaches exit 0 by this branch. The tables of `magic-numbers-swift.md` and of `missing-docs-swift.md` hold the row `a path that holds no file | 1 | 0 bytes` above the same sentence. The same sentence stands at `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:294` and at `builtin/validators/code-hygiene/rules/missing-docs-swift.md:251`. A finding shows one example of a cause, and the cause must leave all three files.
- [x] `builtin/validators/README.md:177` — The sentence "A linter keeps one status for findings and a higher status for a failure" is false. This commit removed the word "usually" from that sentence, which makes the sentence a statement about every linter and about every failure. The paragraph this same commit adds six lines below measures the counter-example. swiftlint 0.65.0 exits 2 for a run that reports findings, and it exits 2 for a version-mismatch failure. Its failure status is thus the SAME as its findings status, and not a higher status. Measured again with the child configuration the magic-numbers script writes, over one file that holds `return status == 404`: `warning_threshold: 1` gives status 2 with a JSON array of 2 entries; `swiftlint_version: 99.0.0` gives status 2 with 0 bytes. The next paragraph states this result in its own words: "One status can carry both a measured run and a broken run." Write the limit into the sentence, or name the shape the sentence holds for.
- [x] `builtin/validators/README.md:186` — The sentence "a run that breaches `warning_threshold:` exits 2 and writes a JSON array of 2 entries" is false. The count is the count of findings plus one, and not the constant 2. This same commit writes the counter-example at `builtin/validators/code-hygiene/rules/missing-docs-swift.md:234`: the docs fixture holds 2 findings, and its threshold breach writes 3 entries in 949 bytes. Measured again over the three fixtures with `warning_threshold: 0`, with the child configuration each script writes: magic numbers 2 entries, complexity 2 entries, docs 3 entries. The `rule_id` values of each array are the measured rule and `warning_threshold`. State the count as the count of findings plus one, or name the run the count belongs to.

### The gate holds — no lever weakens it

Measured with swiftlint 0.65.0. The three scripts were extracted from the rule files at 92dd40b67 and run without a change. Fixtures: one file that holds `return status == 404` (magic numbers, baseline 1 finding, exit 0); one file that holds `public struct Thing` with one undocumented stored property (docs, baseline 2 findings, exit 0); one file that holds a function body of 302 code lines (complexity, baseline 1 finding, exit 0).

82 project levers were run against each of the three scripts, in 246 script runs. 63 levers gave the baseline count at script exit 0:

- `swiftlint_version: 0.65.0`, the version that is installed.
- `warning_threshold:` at 0, 1, 2, 3, 5, -1, a value that is not a number, and 1 together with `strict: true`.
- `strict: true`. `lenient: true`. `strict: true` together with `lenient: true`.
- `baseline: base.json`, where `base.json` was written by `swiftlint --write-baseline` and holds the fixture violation. `write_baseline: out.json`. `baseline:` together with `warning_threshold: 0`.
- `allow_zero_lintable_files: true`. `check_for_updates: true`. `cache_path:`. `reporter: xcode`. `reporter:` with a name that does not exist. `reporter: json`. `use_nested_configs:`.
- `disabled_rules:` that names each measured rule, one at a time and all four together.
- `only_rules:` that names an unrelated rule. `whitelist_rules:` that names an unrelated rule. `opt_in_rules: []` together with `disabled_rules:`. `only_rules:` together with `disabled_rules:` in one file.
- A permissive option block for each measured rule, one at a time and all four together. A severity change to `error`, and to `warning`.
- `included:` that names another directory. `included: []`. `excluded:` that names an unrelated directory.
- `parent_config:` and `child_config:` that each name a file which disables the measured rules.
- Bytes that are not YAML. A YAML list where a map belongs. An empty file. A key that swiftlint does not know, `deployment_target:`, `analyzer_rules:`, `unused_import:`.
- A UTF-8 BOM. CRLF line endings.
- A `.swiftlint.yml` that is a directory. A `.swiftlint.yml` at mode 000.
- A nested `src/.swiftlint.yml`, with and without a file at the root.

9 levers gave 0 findings at script exit 0. Each one is an `excluded:` that covers the source directory: `excluded: [src]`, `excluded: ["."]`, `excluded: ["**"]`, `excluded: ["src/*.swift"]`, `included: [src]` together with `excluded: [src]`, and each of those together with `allow_zero_lintable_files: true`, `warning_threshold: 0` or `strict: true`. That is the behaviour the card asks for. Each of the 9 reaches exit 0 through the `No lintable files found` branch, which the first finding above names.

No lever makes a script report nothing at exit 0 while the fixture is dirty, other than `excluded:` of the source directory.

Every top-level key swiftlint accepts was tested: `disabled_rules`, `opt_in_rules`, `only_rules`, `whitelist_rules`, `analyzer_rules`, `included`, `excluded`, `warning_threshold`, `reporter`, `allow_zero_lintable_files`, `strict`, `lenient`, `baseline`, `write_baseline`, `check_for_updates`, `swiftlint_version`, `cache_path`, `parent_config`, `child_config`, `use_nested_configs`, `deployment_target`, `unused_import`, each per-rule option block, each per-rule `severity`, `warning` and `error`, and keys swiftlint does not know. The command-line flags were not tested, because a project `.swiftlint.yml` cannot reach the command line: each script states the whole argv.

### No broken run is read as measured

10 levers gave 0 findings at script exit 1, for each of the three scripts. The engine reads exit 1 as a broken tool. Each one names a swiftlint version that is not installed: `0.64.0`, `99.0.0`, `0.1.0`, `0.65` (a prefix), `"0.65.0.0"` (four parts), `notaversion`, `swiftlint_version:` as a YAML list, `swiftlint_version:` as a map, `warning_threshold: 0` together with `swiftlint_version: 99.0.0`, and a `parent_config:` that names a file which holds `swiftlint_version: 99.0.0`. swiftlint requires an exact match. So the hole the round of c2beb6ec9 found is closed for the whole value space, and not for `99.0.0` alone.

### No measured run is read as broken

No run of the 246 gave a count of findings above 0 at a nonzero script exit. No script gave a status outside 0 and 1. The complexity fixture makes swiftlint exit 2 with no project file at all, because the child states `function_body_length` `error: 250` and the body spans 302 lines, so the status-2 gate carries that script's own baseline.

### The composite attack fails

A status-2 run with a JSON array that holds no entry of the measured rule would report 0 findings at exit 0. No project file reaches that shape. Measured with `warning_threshold: 0` together with `baseline:`, with `excluded:` of an unrelated directory, with `disabled_rules:` of all four rules, and with `swiftlint_version: 99.0.0`: each array holds the measured rule beside the `warning_threshold` entry, or the run writes 0 bytes and the script breaks. The structural reason: the rule configuration carries `only_rules` and is passed as the LAST `--config`, so the project cannot add another rule to the array, and the one synthetic entry swiftlint adds appears only when a real warning is already in the same array.

### The status and byte claims of the rule docs hold

Each status and each byte count of each rule-doc table was reproduced exactly at the path length each doc was measured at — magic numbers at 152 characters, missing docs at 151, complexity at 154. The byte count scales at 1 byte per path character per entry that carries a file, which a control at two root lengths confirms. The two counts that no path changes — 5 bytes for an empty array, and 0 bytes for a broken run — match at every path length.

Reproduced: status 0 with 5 bytes for a clean run; magic numbers 1 entry in 385 bytes; magic numbers 2 entries in 608 bytes at `warning_threshold: 1`; docs 2 entries in 726 bytes; docs 3 entries in 949 bytes at `warning_threshold: 1`; docs 1 entry in 364 bytes for a file that does not parse; complexity 1 entry of error severity in 413 bytes with the reason `currently complexity is 16`; status 2 with 0 bytes for each version mismatch; status 1 with 0 bytes for a path that holds no file; status 134 with 0 bytes for a `--config` path that holds no file and for `child_config:`; status 64 with 0 bytes for a command-line option that does not exist.

The stderr line each rule doc quotes was reproduced byte for byte: `warning: Currently running SwiftLint 0.65.0 but configuration specified version 99.0.0.`

The claim that swiftlint lints no file at a version mismatch holds. The mismatch run writes no `Linting` line, where the control run at `0.65.0` writes `Linting Swift files at paths`, `Linting 'MD.swift' (1/1)` and `Done linting! Found 2 violations, 0 serious in 1 file.`

### The error-severity claims hold

`missing-docs-swift.md` states that a project cannot reach an error-severity finding. Measured: a project `.swiftlint.yml` that states `missing_docs:` with `error: [open, public]` gives status 0 and 2 entries of Warning severity; a child that states the same `error:` list gives status 2 and 2 entries of Error severity. `magic-numbers-swift.md` states the same for its rule. Measured: a project that states `no_magic_numbers:` with `severity: error` gives status 0 and 1 entry of Warning severity.

### The three new acceptance tests prove the gate

Each of the three is one call to `verify_shipped_run_breaks`. Each stages its dirty file at `Sources/Staged.swift` beside a support file `.swiftlint.yml` that holds `swiftlint_version: 99.0.0`, drives the real script through `execute_tool_runs`, and asserts three things: the findings are empty, the errors have a length of exactly 1, and the first error holds `configuration specified version 99.0.0`.

Two lines of evidence show each assertion is load-bearing. First, a control: the gate of each script was put back to `if [ "$status" -ne 0 ] && [ "$status" -ne 2 ]` on a copy, and each copy reported 0 findings at exit 0, where the shipped script reports 0 findings at exit 1. Second, the engine raises a tool error only on a nonzero exit, at `crates/swissarmyhammer-validators/src/review/tool_rules.rs:771`, and it discards stderr on a zero exit. So under the old gate the run gives 0 findings AND 0 errors, and the length assertion fails.

### Two protections that are load-bearing

`baseline:` is fully effective in a single-configuration run, and only the two-`--config` layering neutralises it. Measured: one file that holds the rule configuration AND `baseline: base.json` gives status 0 with 5 bytes and an empty array, which is total suppression of a dirty fixture; the same `baseline:` in the project file with the rule configuration layered after it gives status 0 with the finding present. A swiftlint child configuration does not inherit the parent's `baseline`, and the rule configuration never states it. Attempts to carry it across failed: `child_config:`, `parent_config:`, `baseline:` with each of those, an absolute baseline path, and `baseline:` with `warning_threshold: 0`. If the rule configuration is ever merged into one file, or the `--config` arguments are ever reordered, `baseline:` becomes a hole.

`child_config:` in a project file makes swiftlint 0.65.0 abort with status 134 and `Abort trap: 6`. The scripts survive only because the retry path greps for `Could not read configuration`, which that abort message holds, and re-lints without the project configuration. The recovery depends on that exact substring.

### Language

The lines this commit adds hold no frequency word.

## Review Findings (2026-08-11 06:20)

The engine reviewed `HEAD~1..HEAD` (bcc6d6cc3). It attempted 0 checks, because no validator matches `*.md`, and this commit changes Markdown prose only. `cargo nextest run -p swissarmyhammer-validators` gave 619 tests and 619 passed, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`. Measurement with swiftlint 0.65.0 against the three shipped scripts gave the findings below.

- [x] `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:290` — The sentence "Each run that broke wrote 0 bytes, at status 1, 134, 64 or 2." is false. A run at status 1 with 0 bytes does not break. Measured with swiftlint 0.65.0, over one file under `src/` that holds `return status == 404`, beside a project `.swiftlint.yml` that holds `excluded: [src]`: swiftlint exits 1, writes 0 bytes to stdout, and writes `Error: No lintable files found at paths: 'src/Magic.swift'` to stderr; the shipped script reports 0 findings and exits 0. This same commit added the row for that run at line 279. The sentence classes a run that gives a clean answer as a run that broke. The same sentence stands at `builtin/validators/code-hygiene/rules/missing-docs-swift.md:247`, above the row this commit added at line 236. A finding shows one example of a cause, and the cause must leave both files.
- [x] `builtin/validators/README.md:192` — The README names one test that the three shipped scripts make, and it leaves out a second test. The sentence "The three shipped swiftlint rules accept status 2 only when the report holds a JSON array of one entry or more." is the whole account the README gives. Each of the three scripts holds a second test after the status gate, and that gate is byte-identical in the three files: `if [ "$measured" -eq 0 ]; then if grep -qF 'No lintable files found' "$work/lint.err"; then exit 0; fi; exit 1; fi`. `grep -c 'No lintable' builtin/validators/README.md` gives 0. Measured with a project `.swiftlint.yml` that holds `excluded: [src]`: raw status 1, 0 bytes on stdout, script 0 findings, script exit 0. A reader who writes a script from this README paragraph alone answers a tool error for each project `excluded:` list. Write the stderr test into the README beside the report test.
- [x] `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:309` — The line is 117 characters. The other 59 lines this commit adds are 79 characters or fewer. `git show bcc6d6cc3` gives 60 added lines: 59 at 79 characters or fewer, and 1 at 117. Two sentences stand on that one line: "script reports 0 findings and exits 1, which the engine reads as a broken tool. A script that accepted every status 2". The same paragraph in the other two files is wrapped, at `builtin/validators/code-hygiene/rules/complexity-swift.md:204-205` and at `builtin/validators/code-hygiene/rules/missing-docs-swift.md:266-267`. Wrap the line to the width of the file.
- [x] The commit message of bcc6d6cc3 says "The scripts now test stderr to tell the two cases apart." That sentence is false for this commit. This commit changes 0 script lines. `git show bcc6d6cc3` gives 60 added lines and 28 removed lines, and each one of the 88 is prose. The hunks are README `@@ -174,17 +174,20 @@`, complexity `@@ -185,10 +185,19 @@`, magic numbers `@@ -276,6 +276,7 @@` and `@@ -291,12 +292,21 @@`, missing docs `@@ -233,6 +233,7 @@` and `@@ -248,15 +249,24 @@`. Each script stands in the front matter, for example `complexity-swift.md:27-52`, and no hunk touches that region. The stderr branch shipped in commit 92dd40b67. State what the commit does in the message of the next commit.

### What the measurement confirms

Measured with swiftlint 0.65.0. The scripts were copied out of the three rule files and run without a change. Fixtures: one file that holds `return status == 404` (magic numbers, baseline 1 finding, raw status 0, 1 entry, 378 bytes, script exit 0); one file that holds `public struct Thing` with one undocumented stored property (missing docs, baseline raw status 0 and 2 entries); one file that holds one function of cyclomatic complexity 16 (complexity, baseline raw status 2 and 1 entry of Error severity).

- The stderr branch of the new prose holds. The gate is byte-identical in the three files. `measured` stays 0 at each status other than 0, and at status 2 with a report that is not a JSON array of one entry or more. Control then goes into the block whose first statement is the stderr test, and no `exit 1` stands before that test. So the sentence "At each other status, and at status 2 with a report of 0 bytes, the script makes one more test, on stderr" states the code path.
- The three new table rows "beside a project `excluded:` that covers it | 1 | 0 bytes" hold. Measured raw against the child configuration each script writes: complexity status 1 and 0 bytes; magic numbers status 1 and 0 bytes; missing docs status 1 and 0 bytes.
- The three stderr strings the new prose quotes hold, byte for byte: `Error: No lintable files found at paths: 'src/Complex.swift'` at 61 bytes with the newline, `'src/Magic.swift'` at 59 bytes, `'src/Docs.swift'` at 58 bytes. No other byte, and no ANSI code.
- The three new measurement sentences hold: raw status 1, 0 bytes on stdout, script 0 findings, script exit 0, for each of the three scripts.
- The README entry counts hold. Measured with `warning_threshold: 0` against the child configuration each script writes: magic numbers status 2 and 2 entries, with `rule_id` `no_magic_numbers` and `warning_threshold`; complexity status 2 and 2 entries, with `rule_id` `cyclomatic_complexity` and `warning_threshold`; missing docs status 2 and 3 entries, with `rule_id` `missing_docs`, `missing_docs` and `warning_threshold`. The count is the count of findings plus one, which is what the new sentence states.
- The README version-mismatch sentence holds. Measured: status 2, 0 bytes on stdout, and 0 `Linting` lines on stderr.
- The `swiftlint_version:` sentence of the magic-numbers rule holds. Measured over the magic-numbers fixture: at `0.65.0` the script reports 1 finding and exits 0; at `99.0.0` the script reports 0 findings and exits 1, with `warning: Currently running SwiftLint 0.65.0 but configuration specified version 99.0.0.` on stderr.
- The cross-reference direction words hold. The section title `## A run whose every file the project excludes` stands in each of the three files. `complexity-swift.md` says "below": the sentence is at line 195 and the section at line 255. `magic-numbers-swift.md` says "above": the sentence is at line 299 and the section at line 198. `missing-docs-swift.md` says "above": the sentence is at line 256 and the section at line 155.
- The false sentences of the last round are gone. `grep -rn "breaks at every other status" builtin/validators/` gives 0 hits. `grep -rn "Status 2 states a measured run" builtin/validators/` gives 0 hits.
- The sentence `builtin/validators/README.md:177` holds. It reads "A linter can keep one status for findings and another status for a failure". The word "higher" is gone, and the paragraph that follows states the shape where the two statuses are the same number. Measured for swiftlint 0.65.0: the findings status is 2, and the failure statuses are 1, 2, 64 and 134.
- Language. The 60 added lines hold 0 frequency words, from the set usually, often, always, generally, normally, typically, sometimes, rarely, mostly, may, might, should, probably, simply, just, basically.

## Review Findings (2026-08-11 06:31)

The engine reviewed `HEAD~1..HEAD` (6abc69a4e). It attempted 0 checks, because no validator matches `*.md`, and this commit changes Markdown prose only. `cargo nextest run -p swissarmyhammer-validators` gave 619 tests, 619 passed, 0 failed and 0 skipped, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`. The build wrote 0 warnings. `git show 6abc69a4e` gives 24 added lines and 9 removed lines in 3 files. Measurement with swiftlint 0.65.0 against the three shipped scripts gave the findings below.

- [x] `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:294` — The sentence "Stderr tells that run from a run that broke." is false. Stderr writes the same string for a clean run and for a run that broke. Measured with swiftlint 0.65.0, against the child configuration the magic-numbers script writes, in 4 runs. A project `.swiftlint.yml` that holds `excluded: [src]`, over the dirty fixture: status 1, 0 bytes on stdout, stderr `Error: No lintable files found at paths: 'src/Magic.swift'`, script 0 findings, script exit 0. A directory that holds no Swift file: status 1, 0 bytes, stderr `Error: No lintable files found at paths: 'hollow'`, script 0 findings, script exit 0. A path that holds no file: status 1, 0 bytes, stderr `Error: No lintable files found at paths: 'src/Absent.swift'`. A readable file whose name does not end in `.swift`: status 1, 0 bytes, stderr `Error: No lintable files found at paths: 'src/Notes.txt'`, script 0 findings, script exit 0. Each of the 4 holds the substring `No lintable files found`, which is the substring the script greps for. The string names the path, and it does not name the reason. The script tells the path that holds no file apart with the `[ ! -r "$file" ]` test that runs before swiftlint, and that test gives 0 findings at exit 1 with `magic-numbers-swift cannot read src/Absent.swift` on stderr. So the pre-guard makes that distinction, and stderr does not. Name the test that makes the distinction, or write the limit of the stderr test into the sentence. The same sentence stands at `builtin/validators/code-hygiene/rules/missing-docs-swift.md:251`. A finding shows one example of a cause, and the cause must leave both files.
- [x] `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:269` — The sentence "The status alone does not tell a measured run from a broken run. The REPORT does." is false, and this commit makes the file disagree with itself. The paragraph this commit adds 23 lines below states that a report of 0 bytes carries a clean answer and a run that broke, and that stderr tells the two apart. The table between the two sentences states the same result: the row at line 278 gives status 2 and 0 bytes for a run that broke, and the row at line 279 gives status 1 and 0 bytes for a clean run. The two rows hold the same report. Measured with swiftlint 0.65.0 against the child configuration this script writes: a project `.swiftlint.yml` that holds `swiftlint_version: 99.0.0` gives status 2 and 0 bytes; a project `.swiftlint.yml` that holds `excluded: [src]` gives status 1 and 0 bytes. The report separates the two runs of status 2, and it separates no other pair. The sentence states no limit. Write the limit into the sentence. The same sentence stands at `builtin/validators/code-hygiene/rules/missing-docs-swift.md:226`, and `builtin/validators/code-hygiene/rules/complexity-swift.md:180` holds "The REPORT tells the two apart." A finding shows one example of a cause, and the cause must leave all three files.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-swift.md:403` — The sentence "swiftlint recovers from the parse error, reports 2 `missing_docs` findings and exits 0" is false. The count is 1. Measured with the shipped missing-docs script over one file whose only line is `public func oops( {`: the script reports 1 finding and exits 0, and the one entry names line 1 with the message `public declarations should be documented`. The table of the same file at line 237 states the correct count: `| one file whose only line is `public func oops( {` | 0 | 1 entry, 364 bytes |`. The file disagrees with itself. Write the count of the measurement into the sentence.

### What the measurement confirms

Measured with swiftlint 0.65.0. The three scripts were extracted from the front matter of the rule files at 6abc69a4e with a YAML parser, and run without a change. Fixtures: one file that holds `return status == 404` (magic numbers, baseline raw status 0, 1 entry, script 1 finding, script exit 0); one file that holds `public struct Thing` with one undocumented stored property (missing docs, baseline raw status 0, 2 entries, script 2 findings, script exit 0); one file that holds one function of cyclomatic complexity 16 (complexity, baseline raw status 2, 1 entry, script 1 finding, script exit 0).

Each claim of the new README paragraph holds:

- "a project `.swiftlint.yml` that holds `excluded: [src]` makes swiftlint write `Error: No lintable files found at paths: 'src/Magic.swift'` to stderr, write 0 bytes to stdout, and exit 1". Measured: status 1, 0 bytes, and that string byte for byte.
- "Each of the three shipped swiftlint rules tests stderr for `No lintable files found` after the status gate". Measured: the 6-line gate block is byte-identical in the three scripts.
- "Measured over three dirty fixtures beside that project file: each of the three reported 0 findings at exit 0". Measured: complexity 0 findings at exit 0, raw status 1 and 0 bytes; magic numbers 0 findings at exit 0, raw status 1 and 0 bytes; missing docs 0 findings at exit 0, raw status 1 and 0 bytes. Each stderr string names its own fixture: `'src/Complex.swift'`, `'src/Magic.swift'`, `'src/Docs.swift'`.
- "A script without the stderr test answers a tool error for each project `excluded:` list". A control run proves it: the stderr branch was deleted from a copy of each of the three scripts, and each copy gave 0 findings at exit 1 beside the same project file, where the shipped script gives 0 findings at exit 0.

The two corrected sentences hold, except for the last sentence of each, which finding 1 names:

- "Each run that measured wrote a JSON array, at status 0 or 2." Measured over 6 measured runs: each one wrote a JSON array, at status 0 or 2.
- "Each other run wrote 0 bytes, at status 1, 134, 64 or 2." Measured over 9 shapes: a path that holds no file 1; an empty source directory 1; a `--config` path that holds no file 134; a project `child_config:` 134; a command-line option that does not exist 64; a version mismatch 2; bytes that are not YAML 134; a `.swiftlint.yml` that is a directory 134; a `.swiftlint.yml` at mode 000 134. Each of the 9 wrote 0 bytes, and each status is in the set the sentence names.
- "A report of 0 bytes does not make a run broken. The run beside a project `excluded:` that covers the file writes 0 bytes at status 1, and it gives a clean answer." Measured: raw status 1, 0 bytes, script 0 findings, script exit 0.

The false sentence of the last round is gone. `grep -rn "Each run that broke wrote 0 bytes" builtin/validators/` gives 0 hits. `grep -rn "breaks at every other status" builtin/validators/` gives 0 hits. `complexity-swift.md` never held that sentence, and its section at lines 190 to 200 already states the stderr branch.

The README now names the stderr test. `grep -c 'No lintable' builtin/validators/README.md` gives 2, where the last round measured 0. The same count is 4 for each of the three rule files.

The line-width finding of the last round is closed. `git show 6abc69a4e` gives 24 added lines. 0 of the 24 are more than 79 characters, and the longest is 79 at `builtin/validators/README.md:202`. The 117-character line is now 3 lines at `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:311-313`, at 73, 76 and 74 characters.

The commit message states what the commit does. It says "This commit changes prose only. It changes 0 script lines." Measured: the 3 hunks stand at README `@@ -193,6 +193,17 @@`, magic numbers `@@ -287,9 +287,11 @@` and `@@ -306,9 +308,9 @@`, and missing docs `@@ -244,9 +244,11 @@`. Each script stands in the front matter, and no hunk touches that region.

Language. The 24 added lines hold 0 frequency words, from the set usually, often, always, generally, normally, typically, sometimes, rarely, mostly, may, might, should, probably, simply, just, basically.

Line width outside the added lines. 3 lines of the changed files are more than 100 characters, and this commit added none of them: `builtin/validators/README.md:62` at 123 characters, inside an indented code block that quotes a rule script; `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:221` at 113 characters and `builtin/validators/code-hygiene/rules/missing-docs-swift.md:178` at 113 characters, each a Markdown table row. `builtin/validators/code-hygiene/rules/complexity-swift.md:278` holds the same 113-character row.

The cross-reference direction words hold. `magic-numbers-swift.md` says "above": the sentence is at line 301 and the section at line 198. `missing-docs-swift.md` says "above": the sentence is at line 258 and the section at line 155. `complexity-swift.md` says "below": the sentence is at line 195 and the section at line 255.

Each row of each status table holds. The 10 rows of `magic-numbers-swift.md:275-284` and the 10 rows of `missing-docs-swift.md:232-241` each agree with the prose above and below them.

## Review Findings (2026-08-11 06:56)

The engine reviewed `HEAD~1..HEAD` (da876389e). It attempted 0 checks, because
no validator matches `*.md`. Measurement with swiftlint 0.65.0 and jq 1.8.2
gave the 2 findings below. The 3 scripts were read out of the front matter of
the rule files with a YAML parser, and run without a change. Each raw run used
the same child configuration the matching script writes.

- [x] `builtin/validators/code-hygiene/rules/complexity-swift.md:183` — The new sentence names 2 runs at status 1: "the clean run beside a project `excluded:` list" and "the broken run over a path that holds no file". The status table that the same sentence introduces stands at lines 187 to 192 and holds 4 rows. 0 of the 4 rows names a path that holds no file. `magic-numbers-swift.md:285` and `missing-docs-swift.md:242` each hold the row `| a path that holds no file | 1 | 0 bytes |`. Measured with the complexity child configuration over `src/Absent.swift`: status 1, 0 stdout bytes. The number in the sentence is correct, and the table of this file gives no row for it. Give the table the row.
- [x] `builtin/validators/code-hygiene/rules/complexity-swift.md:183`, `builtin/validators/code-hygiene/rules/magic-numbers-swift.md:273`, `builtin/validators/code-hygiene/rules/missing-docs-swift.md:230` — The sentence gives "the broken run over a path that holds no file" as the status-1 run that the report cannot separate from the clean run. The script never gives that path to swiftlint. Measured: the `[ ! -r "$file" ]` guard stands before `mktemp -d` and before each `swiftlint` call. Over `src/Absent.swift` each of the 3 scripts wrote `<rule-name> cannot read src/Absent.swift` to stderr, ran swiftlint 0 times, reported 0 findings and exited 1. The limit is real, and 2 shapes that the script does give to swiftlint show it: the directory `hollow` and the file `src/Notes.txt` each gave status 1 and 0 stdout bytes in 3 of 3 rules. Name a shape the script reaches.

### What the measurement confirms

Set A, the new paragraph of the 4 shapes, at `complexity-swift.md:206-216`,
`magic-numbers-swift.md:313-323` and `missing-docs-swift.md:270-280`. 12 raw
runs, 4 shapes for each of the 3 rules: each run gave status 1, 0 stdout bytes,
and the string `Error: No lintable files found at paths: '<the path>'` on
stderr. The string names the path. It names no reason. The script gave 0
findings and exit 0 for 3 of the 4 shapes in each rule, and 0 findings and exit
1 for `src/Absent.swift`. The guard string of each file holds word for word:
`complexity-swift cannot read src/Absent.swift`,
`magic-numbers-swift cannot read src/Absent.swift`,
`missing-docs-swift cannot read src/Absent.swift`. The guard passes on the
directory `hollow` and on `src/Notes.txt`, because each one is readable, so
swiftlint runs for those 2 shapes.

Set B, the status-2 half of the corrected sentence. The probe run of
`complexity-swift` gave status 2 with 1 entry. The `warning_threshold: 1` run
gave status 2 with 2 entries for `magic-numbers-swift` and 3 entries for
`missing-docs-swift`. Each `swiftlint_version: 99.0.0` run gave status 2 with 0
bytes. `jq 'type == "array" and length > 0'` answered true for each array run
and false for each 0-byte run. So the report tells the 2 runs apart at status 2,
in 3 of 3 files. At status 1 the report is 0 bytes for the clean run beside a
project `excluded:` list and 0 bytes for a broken run, so the report tells no
pair apart at status 1.

Set C, the corrected count at `missing-docs-swift.md:420`. One file whose only
line is `public func oops( {`: swiftlint exits 0 and writes 1 entry with
`rule_id` `missing_docs`; the script reports 1 finding and exits 0. The count of
1 holds. The row it names, at `missing-docs-swift.md:241`, states status 0 and 1
entry, so the count agrees with the row.

Set D, the corrected sentence at `magic-numbers-swift.md:298` and
`missing-docs-swift.md:255`. The script does test stderr: it holds
`grep -qF 'Could not read configuration'` and
`grep -qF 'No lintable files found'`, each on `"$work/lint.err"`. 2 paragraphs
stand below the sentence in each file, at `magic-numbers-swift.md:301-311` and
`313-323`, and at `missing-docs-swift.md:258-268` and `270-280`. The first
states the stderr test. The second states the guard test and the limit of both.

Set E, `complexity-swift.md:180`. The probe run is the row
`| the probe file | 2 | 1 entry, 413 bytes |` at `complexity-swift.md:190`. It
gave status 2 with 1 entry of `rule_id` `cyclomatic_complexity`. It writes a
JSON array at status 2.

Tests. `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then
`cargo nextest run -p swissarmyhammer-validators`: 619 tests run, 619 passed, 0
failed, 0 skipped.

The 3 sentences of the 06:31 round are gone.
`grep -rn "Stderr tells that run" builtin/` gives 0 hits.
`grep -rn "The REPORT does\." builtin/` gives 0 hits.
`grep -rn "reports 2 \`missing_docs\`" builtin/` gives 0 hits.

Language and width. The commit adds 61 lines. 0 of the 61 lines is more than 79
characters. The 61 lines hold 0 frequency words, from the set usually, often,
always, generally, normally, typically, sometimes, rarely, mostly, may, might,
should, probably, simply, just, basically.

The commit changes prose only. The front matter of the 3 files ends at line 77,
74 and 75. The first changed line of each file is 180, 269 and 226. So the
commit changes 0 script lines.

Byte counts. The stdout byte counts I measured are larger than the counts in the
status tables, because the JSON reporter writes the absolute path of each file
into each entry. The entry counts agree. Card ^y81mb7n owns that, and this round
reports no finding for it.

## Review Findings (2026-08-11 07:13)

The `review` engine ran `review sha HEAD~1..HEAD` over commit 40a094908. It attempted 0 checks and gave 0 findings, because the 3 changed files are rule `.md` files and the engine holds no validator for them. The 2 findings below come from measurement on this machine with swiftlint 0.65.0.

The commit is correct in what it measures. Each new row was reproduced over 9 runs: the row for a path that holds no file, and the row for the directory `hollow`, each give status 1 and 0 stdout bytes, in 3 of 3 rules. An empty `hollow` and a `hollow` that holds 1 file that is not Swift give the same status and the same 0 bytes. The guard operator is `-r`, not `-f`, so `[ ! -r "$file" ]` admits a readable directory and stops a path that holds no file. Each script holds 1 guard and 2 swiftlint call sites, and 2 of 2 call sites stand after the guard. So the corrected sentence names a run the script makes.

`touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then `cargo nextest run -p swissarmyhammer-validators` gives 619 run, 619 passed, 0 failed, 0 skipped, 0 compiler warnings. 0 of the 14 added lines is more than 79 characters, and the added lines hold 0 hedge words. The commit changes 0 script lines and 0 front matter lines.

2 byte counts came 1 byte over the table. A control run with an absolute path 1 character shorter gave the stated count in both. Path length explains both, so this round reports no finding for them. Card ^y81mb7n owns that question.

- [x] `builtin/validators/code-hygiene/rules/complexity-swift.md:184` — The sentence that introduces the table ends with `over one file holding one function of cyclomatic complexity 16:`. That clause gives the whole table 1 input. The commit adds 2 rows, at lines 194 and 195, that were measured over other inputs: the row for a path that holds no file was measured over `src/Absent.swift`, and the hollow row was measured over a directory. Line 190, the row for a file that holds no function over a gate, was already outside the clause. So the clause is false for 3 of the 6 rows, and the commit adds 2 of the 3. `magic-numbers-swift.md:274` and `missing-docs-swift.md:231` close the same sentence with `Each status swiftlint 0.65.0 answers with was measured against the child configuration this script writes:`, which names no input and is true for every row. Give `complexity-swift.md` a lead-in that names no single input. Remove the cause from the whole table, not from the 2 new rows alone.

- [x] `builtin/validators/code-hygiene/rules/complexity-swift.md:195` — The hollow-directory run has 0 acceptance tests. `grep -rn 'hollow' crates/ --include='*.rs'` gives 0 hits. The run that the commit takes out of the 3 sentences holds 3 tests, 1 for each rule: `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity.rs:656`, the test `the_shipped_swift_complexity_tool_rule_breaks_on_a_file_it_cannot_read`, and the same shape at `magic_numbers.rs:635` and `missing_docs.rs:1452`. The commit puts the hollow run into 3 sentences and 3 tables, and states that the script gives that directory to swiftlint and answers clean. 0 tests hold that behavior. Add 1 acceptance test for each rule, 3 in total, over a directory that holds no Swift file, beside the tests for the path that holds no file.

## Review Findings (2026-08-11 07:38)

The engine reviewed `HEAD~1..HEAD` (2bb04d62f). It attempted 16 checks. It confirmed 2, and it refuted 3. It failed 0, and it skipped 0. The commit changes 7 files, adds 213 lines, and removes 5 lines. swiftlint 0.65.0 is on this machine. The commit changes 0 script lines and 0 front matter lines.

The 8 earlier Review Findings sections hold 26 items. All 26 items are marked done. 0 item from an earlier pass stays open.

This pass reports no byte-count difference. Card ^y81mb7n owns the byte counts in the status tables.

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:482` — struct `ShippedHollowDirectory` is a near-duplicate of `ShippedFailFixture` at crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:125 (48 tokens, 93% alike).
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity.rs:664` — `SWIFT_COMPLEXITY_HOLLOW_PATH` defines a constant with value `"Sources/Hollow.swift"` that is duplicated in `magic_numbers.rs` as `SWIFT_MAGIC_NUMBERS_HOLLOW_PATH`. Since the value is identical and the constant is used across multiple rule files, it should be defined once in `shipped.rs` as a shared constant (following the pattern of `SWIFT_PROJECT_TYPES`, `SWIFT_EXCLUDE_POSITIONS`, etc. at lines 654–776), not redefined per rule. Define `SWIFT_HOLLOW_PATH` once in `shipped.rs` (around line 776, with other shared Swift constants) with value `"Sources/Hollow.swift"`. Remove `SWIFT_COMPLEXITY_HOLLOW_PATH` from complexity.rs and `SWIFT_MAGIC_NUMBERS_HOLLOW_PATH` from magic_numbers.rs. Update the probe references to use the shared constant.

### Measurements for the 2 findings

The test-refactor exception does not release finding 1. The diff of `HEAD~1..HEAD` adds the line `struct ShippedHollowDirectory {`. `ShippedFailFixture` was in the file before this commit. The subject of the finding is new code, not code that was there before.

`ShippedFailFixture` holds 5 fields: `run`, `fixture`, `path`, `support`, `noun`. `ShippedHollowDirectory` holds 6 fields: `run`, `prompt_rule`, `change_purpose`, `directory`, `staged`, `reason`. 1 of the 6 field names is the same in both: `run`.

Finding 2 names 2 files. The cause is in 3 files. `grep -rn '"Sources/Hollow.swift"' crates/swissarmyhammer-validators/src/review/tool_rules/tests/` gives 3 hits, 1 in each of these files:

- `complexity.rs:664` — `SWIFT_COMPLEXITY_HOLLOW_PATH`
- `magic_numbers.rs:666` — `SWIFT_MAGIC_NUMBERS_HOLLOW_PATH`
- `missing_docs.rs:1485` — `SWIFT_HOLLOW_PATH`

The staged-file list has the same shape. 3 constants hold the same 1 pair, `("Sources/Hollow.swift/Notes.txt", "notes\n")`: `complexity.rs:668`, `magic_numbers.rs:670`, and `missing_docs.rs:1489`. Remove the cause from all 3 files, not from the 2 files the finding names. Move 1 path constant and 1 staged-file constant to `shipped.rs`, and let all 3 rule files use them.