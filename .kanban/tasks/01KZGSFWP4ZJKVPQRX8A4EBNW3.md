---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzk5f0j2q6wsfdbd2ecwk1zq
  text: |
    ### Research — detector branch decided

    **Branch 1 taken. The jscpd Rust engine IS consumable as a crate.**

    The engine at `github.com/kucherenko/jscpd/tree/master/rust` publishes four crates to crates.io under MIT. The one that holds the algorithm is `cpd-core` 0.1.7:

    - `cpd_core::detect::detect_prepared(format_groups: Vec<Vec<PreparedSource>>, min_tokens, skip_local, min_lines, scan_roots) -> Vec<CpdClone>` is the Rabin-Karp detector.
    - `PreparedSource { id, format, hashes: Vec<u64>, spans: Vec<(Location, Location)> }` has public fields, so the caller supplies its own token stream. We do NOT need `cpd-tokenizer` (which pulls the whole `oxc` parser chain). Our tree-sitter roster supplies the tokens, which we need anyway for structural test exclusion.
    - `windows_match` refuses only the identical position in the identical source, so two copies in ONE file are matched. The intra-file case the fail fixture needs works with no change.
    - Dependencies are `rayon`, `rustc-hash`, `serde`, `xxhash-rust` — all four already in `Cargo.lock`.

    So the plan is: our tree-sitter tokens into their proven detector.

    **Point 3 — the cosine probe.** `^adf0d7h` is already done and the `duplicates` probe stays declared on the `duplication` set, because the languages with no grammar keep the prompt path and that path reads the probe. Nothing to fold in; `^adf0d7h` stands.

    **Shape of the work** (the `no-commented-code-parsed` rule from `^n0680p8` is the template — same tool, same contract, same acceptance-test placement):

    - `swissarmyhammer-sem` gets the grammar knowledge: the token stream, the test-definition ranges, the suppression-marker ranges.
    - `swissarmyhammer-code-context` gets the op: read the files, run `detect_prepared`, drop a clone whose fragment sits in an excluded range, print the contract line.
    - The acceptance test goes in `apps/swissarmyhammer-cli/tests/` because the tool IS `sah` and only that package defines `CARGO_BIN_EXE_sah`.
  timestamp: 2026-08-09T11:45:22.498602+00:00
- actor: claude-code
  id: 01kzk7nhcdeqmqr79bhfzskdrh
  text: |
    ### Implementation landed

    **Detector: branch 1.** The jscpd Rust engine IS consumable. `cpd-core` 0.1.7 (MIT) publishes `detect::detect_prepared`, the Rabin-Karp detector, and `PreparedSource { id, format, hashes, spans }` with public fields — so the caller supplies the token stream. We supply it from `swissarmyhammer-sem`'s tree-sitter roster, which is what makes the structural test exclusion possible. `cpd-tokenizer` is NOT used; it would pull the whole `oxc` chain and give up the parse. All four of `cpd-core`'s dependencies (`rayon`, `rustc-hash`, `serde`, `xxhash-rust`) were already in `Cargo.lock`.

    **What landed.**

    - `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs` — the grammar half: the code-token stream (comments left out), and the exempted byte ranges. Test detection is a per-language `TestSpec` table read off the parse — an attribute's text, a definition's name, a call target, a base list. NEVER a path. Bash and Fortran answer the empty spec, because neither writes its tests beside the code and a whole-file rule would have to read the path.
    - `crates/swissarmyhammer-code-context/src/ops/find_duplication.rs` — the op. Reads the files, hashes the tokens, runs `detect_prepared` at a 50-token window, drops a clone whose fragment STARTS inside an exempted range, prints the contract line.
    - `builtin/validators/duplication/rules/duplication-parsed.md` plus its one fail/pass fixture pair.
    - `apps/swissarmyhammer-cli/tests/duplication_tool_rule.rs` — the acceptance test. It lives there for the same reason `commented_code_tool_rule.rs` does: the rule's tool IS sah, and only that package defines `CARGO_BIN_EXE_sah`.

    **The gate is structural on both sides.** The exemption reads the START of a block, so a copy that begins in a test definition is exempt however far it runs and a copy that begins in production code is reported however far it runs into one. `definition_range` walks back over the attributes, so a clone that starts at `#[test]` is still inside the definition the attribute marks.

    **A defect the tests caught.** `attribute_item` ends the same way `function_item` and `mod_item` do, so `#[cfg(test)]` first read as a test definition of its own and reported a second, nested range. `is_definition_kind` now refuses every attribute kind.

    **Measured on this workspace**, all 1155 tracked `.rs` files, 6.7 s: 945 findings. With the Rust test markers taken out of the table: 5077. The structural exclusion removes 4132, 81.4% of the raw total — the same order as the 60.6% jscpd figure this set's VALIDATOR.md already records. A path glob reaches none of them. The 945 that remain have a median of 67 tokens; 389 are intra-file; the largest read as real copies (five CLI `build.rs` files are the same file). The numbers are in the rule body.

    **A duplicated test HELPER is a finding, on purpose.** A helper is not a test definition, so no structural marker exempts it. This is the same standing rule the `cognitive-complexity` rule states.

    **Point 3 — the cosine probe.** It stays declared on the set and the `duplication` prompt rule still reads it for every language the roster does not parse. `^adf0d7h` therefore stands, and it is already done.

    **RED verified 17 ways.** Ten mutations of the extractor and the op (test markers removed, marker text changed, comments counted as tokens, definition range not walking back over attributes, attribute guard removed, call roster broken, window dropped to five, exemption check removed, language grouping collapsed, message reworded) and six of the shipped rule and its fixtures (`supersedes` drops swift, `match` drops `.rs`, `match` gains an extension the roster has no grammar for, `"$SAH_BIN"` replaced by a bare `sah`, the pass fixture losing its marker, the fail fixture keeping one copy). Every one went RED and every one was restored.

    **One test was too weak and was rewritten.** `two_languages_are_never_paired` first paired a long Rust block against a two-line Python file, which could never have matched, so collapsing the language grouping stayed GREEN. It now uses one block that is valid JavaScript and valid TypeScript — token for token the same stream, two different grammars — and the mutation goes RED.

    **End to end, through the production path** (`plan_tool_rules` runs the doctor over the shipped fixture pair, `execute_tool_runs` runs bash, the op, and the stdout parser):

        Finding { file: "src/lib.rs", line: 24, validator: "duplication",
                  rule: Some("duplication-parsed"),
                  claim: "verbatim duplicate of src/lib.rs:4 (18 lines / 78 tokens)",
                  evidence: "src/lib.rs:24: verbatim duplicate of src/lib.rs:4 (18 lines / 78 tokens)" }

    The acceptance bar is met: for that file `prompt_rules_for` returns an EMPTY reading list for the whole duplication set, so no task can carry a rule of it and no agent can read one. `ToolReport::attempted() == 1` and there are no fallbacks.

    **The live MCP `review` tool cannot show this yet**, and that is expected rather than a gap: the running server is the installed `sah` on PATH, which has no `find duplication` op, so `SAH_BIN` resolves to a binary without it. `sah tool review file review` refuses outright — that CLI route wires no agent factory. The acceptance test is the only place the real chain can run against the binary this branch builds, which is the same conclusion `^n0680p8` recorded.
  timestamp: 2026-08-09T12:23:53.485261+00:00
- actor: claude-code
  id: 01kzk7nt638v3xs821es1pb1t0
  text: |
    ### implement — changed
    - evidence: 5 new files (`crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs`, `crates/swissarmyhammer-code-context/src/ops/find_duplication.rs`, `builtin/validators/duplication/rules/duplication-parsed.md`, the rule's two `.tmpl` fixtures, `apps/swissarmyhammer-cli/tests/duplication_tool_rule.rs`), 14 modified. Detector branch 1: `cpd-core` 0.1.7 embedded, tokens from our tree-sitter roster. `cargo nextest run --workspace` 13960 passed / 0 failed / 0 skipped; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. RED verified 17 ways. End-to-end finding: `src/lib.rs:24: verbatim duplicate of src/lib.rs:4 (18 lines / 78 tokens)`, confirmed, with an EMPTY prompt-rule reading list for the whole duplication set.
    - next: `/review`
  timestamp: 2026-08-09T12:24:02.499512+00:00
- actor: claude-code
  id: 01kzk9g0zkd6y7jdssbq3nec0x
  text: |
    ### Claim verification — iteration 1

    Four load-bearing claims, checked against the source. All four hold.

    **1. Detector choice — CONFIRMED.** `cpd-core = "0.1.7"` sits in `[workspace.dependencies]` of the root `Cargo.toml` with a comment that states why `cpd-tokenizer` is left out; `crates/swissarmyhammer-code-context/Cargo.toml` takes it with `{ workspace = true }`. The registry manifest reads `license = "MIT"`, `homepage = "https://jscpd.dev"`, `description = "Core data models and hashing for cpd"`, and `.cargo_vcs_info.json` records `path_in_vcs: "rust/crates/cpd-core"`. `Cargo.lock` holds ONE `cpd` package, `cpd-core`, over `rayon`, `rustc-hash`, `serde`, `xxhash-rust` — all four already in the lock before this change, so the net new package count is 1 and the net new transitive count is 0. `grep 'cpd-tokenizer' Cargo.lock` returns nothing. `find_duplication.rs` calls `detect_prepared` once and builds `PreparedSource { id, format, hashes, spans }` field by field from `DuplicationSource` tokens. Six cpd symbols are imported and all six are used.

    The pin is caret `"0.1.7"`, so `>=0.1.7, <0.2.0`; the exact build is held by the lock checksum. The crate declares NO `repository` field, so the URL the rule body cites rests on the homepage and the VCS path rather than on declared metadata.

    **2. Structural test exclusion — CONFIRMED.** `TestSpec` carries four fields and every one is a parse fact: `attributes` (attribute node text), `name_prefixes` (the name a definition declares), `calls` (the callee name of a call node), `bases` (the declaration header text). `test_spec` matches on the LANGUAGE ID, never a path. Every string test in the module is applied to node text or a node KIND. The only `path` use is `parse_code(path, source)`, which picks a grammar by extension — routing, not test detection. The rule frontmatter has `match.files` and no `exclude`; `ValidatorMatch` has no exclusion field at all, so one cannot be added without a type change.

    Bash and Fortran do answer the empty spec, and the reason IS stated in the source — the doc comment on `test_spec` names both languages and says `bats` and pFUnit keep whole files of their own, so neither has a marker at a definition and a whole-file rule would have to read the path. The routing reaches `NO_TEST_SPEC` through the catch-all `_` arm rather than named arms; the doc comment sits on that function. Bash has a test, `a_language_with_no_definition_marker_exempts_nothing`; Fortran has none.

    **3. The acceptance bar — CONFIRMED.** The `duplication` set holds exactly four rules: `duplication.md`, `rust.md`, `swift.md`, `duplication-parsed.md`. So `supersedes: [duplication, rust, swift]` names every prompt rule in the set, and the tool rule is the only thing left. `plan_rule_by_health` writes one suppression entry per matched file for each superseded name, keyed `(validator, file)`. The acceptance test reads that key, asserts all three names are suppressed, then asserts `prompt_rules_for(ruleset, &suppressed)` comes back EMPTY — and `prompt_rules_for` is the fan-out planner's own filter, the single production call site at `fleet.rs:607`. So the prompt rules are not deprioritized; they never reach an agent. `report.attempted() == 1` and `report.fallbacks().is_empty()` are both asserted. A fallback path does exist and is correct: an unusable tool becomes a `ToolFallback` that suppresses nothing, so the prompt rules run. That branch cannot be taken on a matched file with a healthy tool, and the test proves the healthy branch through `plan_tool_rules` + `execute_tool_runs`.

    **4. The fixture constraint — CONFIRMED.** One pass fixture carries all three cases and each one is above or below the window as its case needs:
    - The marker-suppressed copy is `mirrored_band` repeating `folded_band` — the SAME block pair the fail fixture uses, which the pipeline reports at 18 lines / 78 tokens. It is well over the fifty-token window, so the marker is what exempts it.
    - The `#[cfg(test)] mod tests` pair is `first_grid`/`second_grid`, thirteen lines each and about seventy body tokens — over the window, so the test marker is what exempts it.
    - The below-minimum pair is `near_edge`/`near_floor`, each a one-line `limit + 1` of roughly fourteen tokens — under the window.

    None of the three passes for the wrong reason. Each is load-bearing: the doctor runs the whole pass fixture and a single missed exemption turns the plan into a fallback, which fails `the_shipped_duplication_tool_rule_reports_a_pasted_block_with_no_llm_call` at its first assertion.

    **For the record, not a finding.** The rule produces 945 findings over 1155 tracked `.rs` files. That is a backlog the rule creates, not a defect in it.
  timestamp: 2026-08-09T12:55:50.003015+00:00
- actor: claude-code
  id: 01kzk9gjhbh3ygzhx3c1rp7mg5
  text: |
    ### Claim 4 — measured, not estimated

    The token counts in the previous comment were a hand count. The op was then run for real (`target/debug/sah tool code_context duplication find --files <file>`) against the fixture and against mutated copies of it. `MINIMUM_WINDOW_TOKENS = 50` in `find_duplication.rs`; `MINIMUM_CLONE_LINES = 0`, so the window is the only gate.

    | Input | Result |
    |---|---|
    | pass fixture, unchanged | 0 findings |
    | fail fixture, unchanged | 1 — `fail.rs:28: verbatim duplicate of fail.rs:8 (18 lines / 78 tokens)` |
    | pass fixture, marker line deleted | 1 — `21 lines / 80 tokens` |
    | pass fixture, `#[cfg(test)]` line deleted | 1 — `13 lines / 83 tokens` |
    | pass fixture, both deleted | 2 |

    So the marker-suppressed copy is 80 tokens and the `#[cfg(test)]` pair is 83 tokens — 30 and 33 tokens above the gate. Neither passes for the wrong reason, and the `#[cfg(test)]` run still carried the marker, so the test-module exemption was the only thing suppressing it.

    The below-minimum pair was measured by growing both bodies with identical 7-token statements until a finding appeared: n=5 gave nothing, n=6 gave 54 tokens. 54 − 42 = 12 tokens for the base pair, against a 50-token gate. It is genuinely the minimum-window case. The margin is 38 tokens, so the pair does not exercise the 49/50 boundary — only that a very short repeat is ignored.

    The fixture pair runs through the doctor inside plan health: `plan_tool_rules` → `check_tool_rule` → `check_fixtures` → `verify_fixture_contract`, which requires the fail fixture to give at least one finding and the pass fixture to give exactly zero. `load_builtins` reads the on-disk `builtin/validators` directory, so the shipped fixtures are the ones checked. `find_fixture` is `read_dir().find(...)` on the `<rule>.<kind>.` prefix with no sort, so a second fixture of either kind would be silently ignored and which one wins would depend on directory order.
  timestamp: 2026-08-09T12:56:07.979772+00:00
- actor: claude-code
  id: 01kzk9gt7y9r86pkhjpxhxvbnn
  text: |
    ### review — findings
    - evidence: `review sha f1876f0f2..68092a2ff` — 16 findings, 16 confirmed, 13 refuted, 27 attempted, 0 failed, 0 skipped. `apps/swissarmyhammer-cli/Cargo.toml:38,58,59,66,85,90,91,96,98,99,105`; `apps/swissarmyhammer-cli/tests/duplication_tool_rule.rs:149,166`; `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs:26,41`; `crates/swissarmyhammer-tools/src/mcp/tools/code_context/execute.rs:319`. No finding was dropped: `git log --diff-filter=A` puts `duplication_tool_rule.rs` first at 68092a2ff, this commit, so the pre-existing-test exception reaches none of them. All four load-bearing claims CONFIRMED — see the two comments above.
    - next: `/implement` the 16 items, then re-review
  timestamp: 2026-08-09T12:56:15.870767+00:00
- actor: claude-code
  id: 01kzk9j1nxra098tdtd7my1zzh
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 19 files, 5 new. Detector branch 1: `cpd-core` 0.1.7 embedded, tokens supplied from this workspace's tree-sitter roster. RED verified 17 ways
    - test: green — cargo nextest run --workspace 13960 passed / 0 failed / 0 skipped, fmt clean, clippy -D warnings clean
    - commit: 68092a2ff — 22 files, 2285 insertions, 45 deletions
    - review: findings — 16 open, on `review sha f1876f0f2..68092a2ff`. Engine: 16 confirmed, 13 refuted, 27 attempted, 0 failed, 0 skipped

    No finding was dropped. `git log --diff-filter=A` puts the acceptance test first at `68092a2ff` — this commit — so the pre-existing-test exception reaches none of the 16.

    **All four load-bearing claims were confirmed by independent verification, not by assertion.**

    1. **Detector.** `cpd-core = "0.1.7"` sits in `[workspace.dependencies]`. The registry manifest reads `license = "MIT"`, `homepage = "https://jscpd.dev"`, and `.cargo_vcs_info.json` records `path_in_vcs: "rust/crates/cpd-core"`. One new package, ZERO new transitive crates — `rayon`, `rustc-hash`, `serde` and `xxhash-rust` were already present. `cpd-tokenizer` appears nowhere. `PreparedSource` is built field by field from our own tokens. Noted: the crate declares no `repository` field, so the GitHub URL the rule body cites rests on the homepage and the VCS path.
    2. **Structural test exclusion.** Every `TestSpec` field is a parse fact. Every string test is applied to node text or node kind; the only `path` use is `parse_code(path, source)`, which picks a grammar. The rule frontmatter has no `exclude`, and `ValidatorMatch` has no exclusion field to add one to. The Bash and Fortran reason is stated in the doc comment on `test_spec`.
    3. **Acceptance bar.** The `duplication` set holds exactly four rules, so `supersedes: [duplication, rust, swift]` names every prompt rule in it. The prompt rules are not deprioritized — they never reach an agent. The fallback branch exists only for an unusable tool.
    4. **Fixture constraint — MEASURED, not asserted.** The verifier ran the op against the fixture and against mutated copies. The marker-suppressed copy is 80 tokens and the `#[cfg(test)]` pair is 83, against a 50-token gate. Deleting the marker or the attribute each produces a finding; deleting both produces two. The below-minimum pair is 12 tokens. No case passes for the wrong reason. Noted: the below-minimum margin is 38 tokens, so it does not exercise the 49/50 boundary.

    **For the record, not a finding:** the rule produces 945 findings over 1155 tracked `.rs` files. That is a backlog the rule creates, not a defect in it. The structural exclusion removes 4132 of the raw 5077 — 81.4% — and a path glob reaches none of them.

    Guardrail: NOT hit — first iteration.
  timestamp: 2026-08-09T12:56:56.253324+00:00
- actor: claude-code
  id: 01kzk9ttndvypnx6mpyvmbcgf5
  text: |
    ### Research — the 16 findings reduce to four causes

    **Cause 1 — eleven findings, one manifest.** `cargo machete 0.9.2` over `apps/swissarmyhammer-cli` reports EXACTLY eleven unused dependencies, and they are the same eleven the review cites: `chrono`, `dirs`, `futures-util`, `glob`, `ignore`, `is-terminal`, `libc`, `reqwest`, `scopeguard`, `sha2`, `swissarmyhammer-code-context`. So the cited lines already enumerate the whole cause; no twelfth hides behind them. A textual check confirms each: no `use <crate>` and no `<crate>::` path in `src`, `tests` or `build.rs`. `reqwest` sits in BOTH `[dependencies]` and `[dev-dependencies]` and neither `src` nor `tests` names it, so removing the cause from the whole file takes both entries. The `unused-dependencies-rust` rule states the two exits word for word: "to keep one dependency, name it in `ignored` with a comment; to keep it out of the report any other way, delete it." Nothing names any of the eleven, so deletion is the fix.

    **Cause 2 — two findings, one duplicated test-helper shape.** `builtin_loader` exists THREE times, not twice: `apps/swissarmyhammer-cli/tests/duplication_tool_rule.rs`, `apps/swissarmyhammer-cli/tests/commented_code_tool_rule.rs`, and the in-crate test module at `crates/swissarmyhammer-validators/src/review/tool_rules.rs`. The one-validator work-list builder repeats the same three ways (`duplication_work`, `commented_code_work`, `code_hygiene_work`), plus a fourth inline `WorkList::new` in the second commented-code test. `crates/swissarmyhammer-validators/src/review/test_support.rs` is the module the finding names, it is already exported behind the `test-support` feature, and `swissarmyhammer-tools` already dev-depends on it that way — so the shared seam exists and the pattern is established.

    The finding at `:149` cites `ruleset_with_body` at `fleet/tests.rs:49`, which builds a `RuleSet` rather than a `WorkList`. Its second stated remedy is the one that fits: "extract common test work-list construction patterns to a shared helpers module."

    **Cause 3 — two findings, `Eq` without `Hash`.** `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs` derives `Eq` on THREE types, not two: `TokenPoint`, `DuplicationToken`, and `DuplicationSource`. The third is the same cause on the same file, so it takes the same fix. `Range<usize>` derives `Hash`, so `DuplicationSource` can carry it.

    **Cause 4 — one finding, a copied handler.** `execute_find_duplication` and `execute_find_commented_code` differ only in the function they call. Both ops have the same signature, `fn(&Path, &[&str]) -> Vec<T>` where `T: Display`, so one handler parameterized by the operation covers both.
  timestamp: 2026-08-09T13:01:43.981454+00:00
- actor: claude-code
  id: 01kzkany0296wkk4xhm99zkgfa
  text: |
    ### The 16 findings, worked by cause

    **Cause 1 — eleven unused dependencies, one manifest.** All eleven deleted from `apps/swissarmyhammer-cli/Cargo.toml`. The `unused-dependencies-rust` rule gives two exits — "name it in `ignored` with a comment", or "delete it" — and nothing in `src`, `tests` or `build.rs` names any of the eleven, so deletion is the fix rather than an ignore entry.

    Removing the cause from the WHOLE file took a twelfth entry the finding list does not cite: `reqwest` sat in `[dependencies]` AND in `[dev-dependencies]`, and no test names it either, so both went. `cargo machete apps/swissarmyhammer-cli` now answers "didn't find any unused dependencies". The `Cargo.lock` diff is exactly the eleven names leaving the `swissarmyhammer-cli` package entry — no package left the lock and no version moved.

    One dev-dependency was ADDED, and it is used: `swissarmyhammer-validators` with `features = ["test-support"]`, which is what carries cause 2's shared seam. `swissarmyhammer-tools` already dev-depends on it exactly that way.

    **Cause 2 — one duplicated test-helper shape, three copies.** `builtin_loader` and the one-validator work-list builder now live once, in `crates/swissarmyhammer-validators/src/review/test_support.rs` — the module the finding names — as `builtin_loader()` and `tool_rule_work(change_purpose, validator, rules, files)`.

    Four call sites route through them, not the two the findings cite: `duplication_tool_rule.rs`, `commented_code_tool_rule.rs` (both its work-list helper AND the inline `WorkList::new` in its second test), and the in-crate test module at `review/tool_rules.rs`, which held the third `builtin_loader` and a third work-list builder.

    The `:149` finding cites `ruleset_with_body` at `fleet/tests.rs:49`, which builds a `RuleSet` rather than a `WorkList` — the two are not the same structure. Its second stated remedy is the one taken: "extract common test work-list construction patterns to a shared helpers module."

    **Cause 3 — `Eq` without `Hash`.** Three types carried it in `duplication.rs`, not the two cited: `TokenPoint`, `DuplicationToken`, and `DuplicationSource`. All three now derive `Hash`. `Range<usize>` derives `Hash`, so `DuplicationSource` takes it with no field change.

    RED first, and it was a real RED: the new test `the_read_and_its_parts_key_a_hash_set` keys each of the three into a `HashSet`, and before the derives the compiler refused it three times over — `the trait bound duplication::TokenPoint: Hash is not satisfied` at :622, `DuplicationToken` at :623, `DuplicationSource` at :624. GREEN after: 23 passed in that module.

    **Cause 4 — a copied handler.** `execute_find_duplication` and `execute_find_commented_code` are now one line each, over a new `execute_file_report(args, context, operation)` parameterized by the op. Both ops have the same signature, `fn(&Path, &[&str]) -> Vec<T>` where `T: Display`, so no adapter is needed. The shared reasoning that both doc comments repeated — why the report is plain text, why no workspace is opened — now sits once on the shared handler.

    This one is BEHAVIOUR-PRESERVING and I did not stage a fake RED for it. The existing acceptance tests in both `duplication_tool_rule.rs` and `commented_code_tool_rule.rs` already drive both ops end to end through the real `sah` binary, and they still pass.

    The whole file was then checked with the card's own detector: `sah tool code_context duplication find` over `execute.rs` reports ZERO pairs.

    **Two duplicate pairs the detector still reports in the touched set, neither a finding on this card.** `duplication_tool_rule.rs` reports four pairs among `PASTED_BLOCK_RS` and `MARKED_BLOCK_RS` — those constants ARE the pasted block the acceptance test exists to catch, so the repetition is the fixture. `test_support.rs:311` repeats `:298` (3 lines / 51 tokens) — two pre-existing git-branch helpers, untouched by this change.
  timestamp: 2026-08-09T13:16:32.130885+00:00
- actor: claude-code
  id: 01kzkap7hjmy2gr7v2q0dy75rr
  text: |
    ### implement — changed
    - evidence: 8 files — `apps/swissarmyhammer-cli/Cargo.toml`, `apps/swissarmyhammer-cli/tests/duplication_tool_rule.rs`, `apps/swissarmyhammer-cli/tests/commented_code_tool_rule.rs`, `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs`, `crates/swissarmyhammer-tools/src/mcp/tools/code_context/execute.rs`, `crates/swissarmyhammer-validators/src/review/test_support.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules.rs`, `Cargo.lock`. 16 of 16 findings checked. `cargo nextest run --workspace` 13961 passed / 0 failed / 0 skipped (13960 before; the one new test is the `Eq`/`Hash` RED). `cargo fmt --all --check` exit 0. `cargo clippy --workspace --all-targets -- -D warnings` exit 0. `cargo machete apps/swissarmyhammer-cli` reports none. `sah tool code_context duplication find` reports zero pairs in `execute.rs`. HEAD unmoved at 68092a2ff.
    - next: `/review`
  timestamp: 2026-08-09T13:16:41.906997+00:00
- actor: claude-code
  id: 01kzkavjbw0t4746e1002pp8cp
  text: |
    ### Claim 3 — addendum, traced through the fan-out

    Two facts beyond the earlier comment, both from reading the production path rather than the test.

    **What holds the `supersedes` list complete.** `crates/swissarmyhammer-validators/src/builtin/mod.rs` carries `assert_eq!(duplication.rules.len(), 4)`. That guard is what stops a fifth prompt rule from being added to the set and silently escaping `supersedes: [duplication, rust, swift]`. There is NO generic validation that a `supersedes` name resolves to a real rule in its set, so a typo would suppress nothing; for this rule the acceptance test is the only thing that catches it.

    **The prompt path is dead, not reordered.** `plan_fan_out` in `crates/swissarmyhammer-validators/src/review/fleet.rs` calls `prompt_rules_for`, and when the result is empty for a non-empty ruleset it logs `every rule for these files is a tool rule or superseded by a healthy one; no LLM task` and `continue`s. No task is built, so there is no agent and no prompt. The acceptance test stops one function short of `plan_fan_out`, but `prompt_rules_for` is the sole filter that `plan_fan_out` applies, so the empty list reaches that `continue`.

    `prompt_rules_for` drops two kinds of rule: a tool rule, which never reaches an agent at all, and any name in the suppressed set.

    **Scope of the claim, stated precisely.** Suppression is keyed `(validator, file)`, so it reaches the `duplication` set and nothing else. The `reuse` set's `reuse` rule reads near-duplicates by cosine similarity and still costs an LLM turn — it is a different validator, outside "the duplication set", so the acceptance bar is untouched by it.

    **One narrowing on `attempted() == 1`.** The test builds its own `ToolReport` from `plan.runs()` filtered to `rule() == "duplication-parsed"`, rather than reading production's `tool_attempted` in `synthesize.rs`. So it proves "the shipped rule planned exactly one run", which is the right claim for this rule, rather than "the whole review attempted one tool run".

    **Observation, not a finding — pre-existing shared infrastructure, unchanged by this range.** Suppression is committed at plan time, before the tool runs: `synthesize.rs` calls `plan_tool_rules`, then `execute_tool_runs`, then takes the suppression out of the plan. The `ToolFallback` branch covers plan-time health only. So a script that passes the doctor and THEN exits nonzero at run time becomes a `ToolRunError` with no prompt re-run, and those files carry zero duplication coverage for that run. This ordering came in with `^n0680p8`, not with this card, and the review engine did not raise it.

    **A drift guard worth naming.** `the_shipped_duplication_tool_rule_matches_the_whole_grammar_roster` holds the rule's `match` equal to `get_all_code_extensions()`, so the glob list cannot grow wider than the roster and silence the prompt rule for files no tool reads.

    Verified live: `cargo nextest run -p swissarmyhammer-cli --test duplication_tool_rule` — 3 passed, 0 failed.
  timestamp: 2026-08-09T13:19:36.828119+00:00
- actor: claude-code
  id: 01kzkbqmpvwzax1f0t8neqq185
  text: |-
    ### review — findings
    - evidence: `review sha 68092a2ff..bc83a4dd5` — counts {findings: 3, confirmed: 3, refuted: 6, attempted: 27, failed: 0}. 2 recorded: crates/swissarmyhammer-validators/src/review/tool_rules.rs:1707, crates/swissarmyhammer-validators/src/review/tool_rules.rs:1961. 1 released by the skill's blanket exception for test code that already existed: crates/swissarmyhammer-validators/src/review/test_support.rs:370 (asks to extract a literal in the pre-existing `seed_chunk` helper into a named constant; the line arrives in 83223a9b0, an ancestor of 68092a2ff).
    - Both directed judgments clear. Twelve deleted CLI dependencies: 0 unsafe of 12 — no removed line carried `features`, `default-features`, `package =`, or `optional`, so feature unification cannot regress; no `[features]` table exists in the manifest; `is_terminal()` is `std::io::IsTerminal`. `test-support`: declared at crates/swissarmyhammer-validators/Cargo.toml:107, absent from any `default` list, and the module is gated at crates/swissarmyhammer-validators/src/review/mod.rs:21 with `#[cfg(any(test, feature = "test-support"))]`. The four routed call sites show no behavioural difference.
    - next: add the missing assertion to each of the two tests so neither can pass with zero assertions, then re-review.
  timestamp: 2026-08-09T13:34:56.731461+00:00
- actor: claude-code
  id: 01kzkbs48rc3dzx20gyyske1mb
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 8 files, 16/16 findings checked. Four causes removed
    - test: green — cargo nextest run --workspace 13961 passed / 0 failed / 0 skipped, fmt exit 0, clippy -D warnings exit 0
    - commit: bc83a4dd5 — 10 files, 275 insertions, 130 deletions
    - review: findings — 2 open, on `review sha 68092a2ff..bc83a4dd5`. Engine: 3 findings, 3 confirmed, 6 refuted, 27 attempted, 0 failed. Two recorded, one released

    **Each cause was wider than the findings cited it, and the whole-file rule caught the rest:** a twelfth unused dependency (`reqwest`, in both dependency tables), three copies of `builtin_loader` rather than two, three types missing `Hash` rather than two.

    **The two open findings are a real hole, not a style point.** `tool_rules.rs:1707` and `:1961` each do `let Some(run) = plan.runs().iter().find(...) else { return; };`, so when the tool is absent the test passes with ZERO assertions. Both lines predate this range (`6e5342c26` and `ae9dc0e94`), but the pre-existing-test exception covers refactoring, deduplicating, renaming and restyling. Adding an assertion so a test can fail is none of those, so no written rule releases them.

    **One finding WAS released, correctly:** `test_support.rs:370` asked that a hardcoded end line become a named constant. That line arrives in `83223a9b0`, an ancestor, and extracting a literal from existing test code is restyling. The blanket exception releases it.

    **Judgment A — the twelve deleted dependencies are safe, 0 unsafe of 12.** This rests on a STATIC fact, not on a green build: no removed line carried `features`, `default-features`, `package =`, or `optional`. Every one was a bare `workspace = true` requesting exactly the workspace-root baseline, so none could be the sole source of a feature another crate unified in — removal cannot subtract from a resolved feature set. `apps/swissarmyhammer-cli/Cargo.toml` has no `[features]` table at all, so no `dep:` reference can break. The `is_terminal()` calls resolve to the std trait (`banner.rs:6` imports `std::io::IsTerminal`), and the `dirs`, `glob` and `ignore` text hits are prose in doc comments.

    Noted, not a finding: `scopeguard` now has no workspace consumer, which leaves a dead `[workspace.dependencies]` entry. It breaks nothing.

    **Judgment B — the `test-support` gate is sound.** The feature is declared, the manifest has no `default` key, and the module is gated `#[cfg(any(test, feature = "test-support"))]`. Nothing test-only reaches a release build. The four routed call sites carry no behavioural difference — same purpose strings, same rule order and constants, same empty `ProbeNames` and `FileWork` arguments. No test lost an assertion, was deleted, or gained `#[ignore]`.

    Finding counts by iteration: 16, 2. No finding has repeated.

    Guardrail: NOT hit.
  timestamp: 2026-08-09T13:35:45.432369+00:00
- actor: claude-code
  id: 01kzkcf2tc984q5p7asyrg0n9d
  text: |
    ### The two findings, worked — and the whole-file sweep

    **The defect.** Both tests did `let Some(run) = plan.runs().iter().find(...) else { return; };`. `plan_tool_rules` plans a run ONLY when `check_tool_rule(...).usable()` — otherwise the rule becomes a `ToolFallback`. So on a machine without vulture or without `cargo machete`, each test returned having asserted NOTHING and reported PASS. A test that cannot fail is not a gate.

    **The fix — the established pattern, not a new one.** The same file already holds the shape twice, at `the_shipped_rust_tool_rule_reports_an_undocumented_public_item` and `the_shipped_rust_complexity_tool_rule_reports_an_over_complex_function`:

        .unwrap_or_else(|| {
            panic!(
                "the shipped Rust tool rule must plan a run; fallbacks: {:?}",
                plan.fallbacks()
            )
        });

    Both tests now use it. `plan.fallbacks()` is what NAMES the reason: each `ToolFallback` carries the doctor's `detail`.

    Remedy (3) of each finding is also taken, because remedy (1) alone leaves the test at the mercy of process order. Under nextest every test is its own process, so the install that `every_shipped_dead_code_tool_rule_passes_its_fixtures` performs does not reach a test that runs first. Both tests now call `install_project_tool_rules(&loader, &project_types)` themselves, the way `verify_shipped_tool_rules_pass_fixtures` does. The call is cheap when the tool is present: `install_tool_commands` answers `AlreadyPresent` and runs nothing.

    The `project_types` literal is now one binding for each test, shared by the install and by `plan_tool_rules`, so the two can never disagree.

    **RED, verified, not claimed.** Each `find` was pointed at `"no-such-rule"` and the two tests were run:

        thread '...the_shipped_python_dead_code_tool_rule_reports_and_suppresses_dead_code' panicked at
        crates/swissarmyhammer-validators/src/review/tool_rules.rs:1711:17:
        the shipped Python dead-code tool rule must plan a run; fallbacks: [ToolFallback {
          validator: "code-hygiene", rule: "no-commented-code-parsed",
          supersedes: Supersedes(["no-commented-code"]),
          detail: "tool missing: error: unrecognized subcommand 'commented_code'\n\nUsage: sah tool code_context [OPTIONS] [COMMAND]..." }]

        thread '...the_shipped_rust_unused_dependency_tool_rule_reports_an_unused_dependency' panicked at
        crates/swissarmyhammer-validators/src/review/tool_rules.rs:1972:17:
        the shipped Rust unused-dependency tool rule must plan a run; fallbacks: []

    `Summary [0.592s] 2 tests run: 0 passed, 2 failed`. Before the change the same mutation stayed GREEN. Restored, both PASS.

    The Python message also shows the fallback list doing its job on an UNRELATED rule — the installed `sah` on PATH has no `commented_code` subcommand — which is the same PATH condition this card already recorded.

    **Whole-file sweep — no other site.** Every `return` in `tool_rules.rs` outside the two: three in production code before the `mod tests` boundary at line 729, and three inside string or doc content. No `return Ok(())`, no `#[ignore]`, no `continue`. The one conditional in the test module, `if row.presence == ToolPresence::Present` in `verify_shipped_tool_rules_pass_fixtures`, is NOT this shape — the row's `supersedes` is asserted unconditionally, and `assert!(exercised > 0, ...)` fails the test when every rule takes that branch.

    **Sibling acceptance-test files — clean.** `apps/swissarmyhammer-cli/tests/commented_code_tool_rule.rs` and `apps/swissarmyhammer-cli/tests/duplication_tool_rule.rs` are the only other files that call `plan_tool_rules`, and all four of their `find` sites already carry `unwrap_or_else(|| panic!(...))` with `plan.fallbacks()`. Neither file holds a bare `return`.

    **Two early returns nearby that are NOT this defect, and were not touched.** `review/fleet/tests.rs` `wait_for` returns when its condition holds and `panic!`s on timeout. `review/test_support.rs` `rename_current_branch_to` returns when the branch already carries the name. Both are correct.

    **Same shape found OUTSIDE this card's scope, reported not changed.** `apps/swissarmyhammer-cli/tests/integration/doc_examples.rs` holds FOUR tests that do `if !dir.exists() { eprintln!("Skipping ..."); return; }` and then assert nothing — `test_doc_examples_*` at lines 18, 162, 184 and 251. Same defect, different file, no finding on this card, so it is left alone and reported here.
  timestamp: 2026-08-09T13:47:44.844292+00:00
- actor: claude-code
  id: 01kzkcfa9fvm5vxvab1g36vtpk
  text: |
    ### implement — changed
    - evidence: 1 file — `crates/swissarmyhammer-validators/src/review/tool_rules.rs`. 2 of 2 findings checked (`:1707`, `:1961`). RED verified: with each `find` pointed at `"no-such-rule"`, `2 tests run: 0 passed, 2 failed` — `the shipped Python dead-code tool rule must plan a run; fallbacks: [ToolFallback { ... detail: "tool missing: ..." }]` and `the shipped Rust unused-dependency tool rule must plan a run; fallbacks: []`. Restored, both PASS. `cargo nextest run --workspace` 13961 passed / 0 failed / 0 skipped. `cargo fmt --all --check` exit 0. `cargo clippy --workspace --all-targets -- -D warnings` exit 0. HEAD `3773be9a3` (the card named `bc83a4dd5`, now two commits back — a merge of the `review` branch landed `29272dcf3` on top).
    - next: `/review`
  timestamp: 2026-08-09T13:47:52.495046+00:00
- actor: claude-code
  id: 01kzkd03faq67j5ntkbhwwqtr9
  text: |-
    ### review — findings
    - scope: `review sha 3773be9a3..4ac042a8b` (the checkpoint commit `4ac042a8b` only; `3773be9a3` is a merge from a parallel session and was not widened into the review)
    - evidence: 4 findings — 2 from the engine (attempted 9, confirmed 2, refuted 0, failed 0, skipped 0) at crates/swissarmyhammer-validators/src/review/tool_rules.rs:1481 and :1595; 2 from the directed judgment on the added `install_project_tool_rules` call at crates/swissarmyhammer-validators/src/review/tool_rules.rs:1701
    - directed judgment: the added call writes outside the test temp dir (`~/.local/bin` through `uv tool install` / `pipx install`, `~/.cargo/bin` through `cargo install`) and has no lock under nextest. It IS idempotent — `tool_install.rs:271` short-circuits on a `which` check.
    - prior findings: both items of the 2026-08-09 08:18 section are checked and closed.
    - next: settle findings 1-4 with one decision on how a tool-rule test guarantees its tool, then apply that decision to every tool-rule test in the file. Card stays in `review`.
  timestamp: 2026-08-09T13:57:02.570190+00:00
- actor: claude-code
  id: 01kzkd1t1d0mv18vs5f6e68bbf
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — 1 file, 2/2 findings checked. Two tests that passed with ZERO assertions when the tool was absent now panic and name `plan.fallbacks()`
    - test: green — cargo nextest run --workspace 13961 passed / 0 failed / 0 skipped, fmt exit 0, clippy -D warnings exit 0
    - commit: 4ac042a8b, parent 3773be9a3 — 3 files, 169 insertions, 24 deletions
    - review: findings — 4 open, on `review sha 3773be9a3..4ac042a8b`. Engine: 9 attempted, 2 confirmed, 0 refuted, 0 failed, 0 skipped, plus 2 from the directed judgment

    **RED was proved the right way.** Pointing each `find` at `"no-such-rule"` gave `2 tests run: 0 passed, 2 failed`. The same mutation stayed GREEN before the change. That is the correct proof for a "this test cannot fail" finding — the old code passed the mutation.

    **The tree moved under this work.** A parallel session committed `29272dcf3` and merged the `review` branch, so `3773be9a3` is a merge commit and `4ac042a8b` sits on top of it. The commit step re-verified HEAD before staging and scoped the review to `4ac042a8b` alone.

    **The directed judgment found a real defect, and iteration 3 introduced part of it.** The added `install_project_tool_rules` call is idempotent (`tool_install.rs:271` short-circuits on `which`) and the nextest per-process claim is correct, but the call is NOT sandboxed. It takes `(loader, project_types)` with no path argument, so it does not reach the test's `tempfile::tempdir()`. It runs the rule's declared install commands through `run_shell(command, None, &[])` at `tool_install.rs:295` — cwd `None`:
    - `&["python"]` → `uv tool install vulture==2.14`, then `pipx install vulture==2.14`, landing in `~/.local/bin`
    - `&["rust"]` → `cargo install cargo-machete@0.9.2 --locked`, landing in `~/.cargo/bin`

    Software installed into the developer's HOME as a test side effect. Nothing locks the path — no `Mutex`, `OnceLock`, file lock, or `serial_test` — so parallel nextest processes can drive `uv`/`pipx`/`npm -g`/`go install` at one destination at once. `cargo install` escapes only because cargo holds its own `~/.cargo/.package-cache` lock.

    **All four findings are ONE decision.** How does a tool-rule test guarantee its tool? If the answer is "assert the precondition and fail loudly", then findings 1 and 2 are satisfied by an assertion rather than by a new install call, and findings 3 and 4 disappear with the call. The two engine findings at `:1481` and `:1595` say the same invariant is missing from two sibling tests.

    Bounding the blame fairly: the call is a pre-existing repo pattern (line 1771, reached by five tests, present before this commit), and `install_project_tool_rules` is the same function `sah init` runs in production. This commit widened an existing cause rather than inventing one.

    Finding counts by iteration: 16, 2, 4. No finding has repeated.

    Guardrail: NOT hit.
  timestamp: 2026-08-09T13:57:58.445208+00:00
- actor: claude-code
  id: 01kzkdsgh2b3gsfx8mecjq8p0b
  text: |-
    ### Review pass 3 — the four findings answered as ONE decision: assert the precondition

    The reviewer said the four findings share one subject. The decision taken is "a tool-rule test asserts its tool, it never installs one". Every `install_project_tool_rules` call is now out of `tool_rules.rs` tests.

    **Why "install it" was the wrong answer.** `install_project_tool_rules(&loader, &project_types)` takes no path. It reaches `install_tool_commands` (`tool_install.rs`), which runs the rule's install commands through `run_shell(command, None, &[])` with cwd `None`. For `python` that is `uv tool install vulture==2.14` then `pipx install vulture==2.14` into `~/.local/bin`; for `rust` it is `cargo install cargo-machete@0.9.2 --locked` into `~/.cargo/bin`. That is software written into the developer's HOME, outside the test's `tempfile::tempdir()`, with no lock between nextest processes.

    **What replaced it.**
    - New test helper `require_tool_installed(loader, project_types, rule_name)`: finds the rule through `project_tool_rules`, runs `check_presence` (the rule's own `which`), and panics naming the rule, what the doctor check reported, and the install commands plus `fix_hint`.
    - New test helper `remedy_label(install_commands, fix_hint)`: one formatter for the remedy line, shared by both failure paths, so the message is written once.
    - Four sites now call it: the Rust missing-docs test, the Rust complexity test, the Python dead-code test, the Rust unused-dependency test. The last two lost their install call; the first two gained the precondition findings 1 and 2 asked for, in the form the reviewer's note allows.

    **The pre-existing site.** `verify_shipped_tool_rules_pass_fixtures` also lost its install call. That site predates this card, but it is the SAME cause the finding names, and the finding says to remove the cause from the whole file rather than only from the two new sites, so it belongs here. Five tests reach it. Its documented tolerance of a missing tool stays — the rosters span six languages and a missing tool is the documented fallback — but the `exercised > 0` assertion now names each absent rule and the command that installs its tool, through the new `absent_rule_label` helper.

    **RED verified twice, by making the precondition false** (PATH stripped of `~/.local/bin`, nothing installed):
    - `the_shipped_python_dead_code_tool_rule_reports_and_suppresses_dead_code` FAILED with: ``dead-code-python` needs a tool this machine does not have, so this test cannot run. The doctor check reported: exited with exit status: 1. Install the tool and run the test again: uv tool install vulture==2.14  OR  pipx install vulture==2.14`.
    - `every_shipped_magic_numbers_tool_rule_passes_its_fixtures` FAILED with the roster message naming all four absent rules and their install commands.

    Both runs left the machine as they found it.

    **Gates:** `cargo nextest run --workspace` 13961 passed, 0 failed, 0 skipped. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
  timestamp: 2026-08-09T14:10:55.138399+00:00
- actor: claude-code
  id: 01kzkdsx93rdssx8mr66wrc0kf
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-validators/src/review/tool_rules.rs. All 4 open findings checked (tool_rules.rs:1481, :1595, :1701 x2). RED proved by making the precondition false: 2 tests failed naming the missing tool and its install command, with PATH stripped of ~/.local/bin. Gates: nextest workspace 13961 passed / 0 failed / 0 skipped; fmt clean; clippy -D warnings clean. HEAD 4ac042a8b.
    - next: /review
  timestamp: 2026-08-09T14:11:08.195080+00:00
- actor: claude-code
  id: 01kzkekbqk4wsghvq8bjbsqtbv
  text: |-
    ### review — findings
    - scope: `review sha 4ac042a8b..62a73bae5` (the checkpoint commit `62a73bae5` only)
    - evidence: 7 findings. The engine fleet was clean — 0 findings, 0 confirmed, 0 refuted, 9 attempted, 0 failed, 0 skipped. All 7 come from the directed judgment on the cost of the decision: `.github/workflows/ci.yml:40` (vulture, against `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1759`), `.github/workflows/ci.yml:40` (cargo-machete, against `tool_rules.rs:2034`), `.github/workflows/ci.yml:27` (clippy component, against `tool_rules.rs:1546` and `:1659`), `.github/workflows/ci.yml:40` (jq, same two tests), `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1521` (the printed remedy names the headline tool, not the binary that failed the check), `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1479` (`remedy_label` prints prose and macOS-only hints as commands), `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1848` (the roster tolerance contradicts the per-rule preconditions).
    - prior findings: all four items of the 2026-08-09 08:50 section are checked and closed. The decision is applied whole — no `install_project_tool_rules` call remains in `tool_rules.rs`.
    - correction: a first judgment this pass reported `remedy_label` and the roster split as clear. Both were wrong and are withdrawn in the dated section. A verifier found the `jq` case, which the per-rule reading hid, and the single-row `SHIPPED_UNUSED_DEPENDENCY_RULES` roster, which makes the tolerance dead where it most looks needed.
    - next: give the CI Test job the four tools the asserting tests now require; make the printed remedy name the binary that actually failed the check and keep prose out of the command position; apply `require_tool_installed` per row in `verify_shipped_tool_rules_pass_fixtures`. Card stays in `review`.
  timestamp: 2026-08-09T14:25:02.195602+00:00
- actor: claude-code
  id: 01kzkezdany4g24djm6tmrcz7h
  text: |
    ### CI provisioning landed — the four `.github/workflows/ci.yml` findings are closed

    One file changed: `.github/workflows/ci.yml`, +30 lines, all inside the `test` job, all before `Run tests`. The fix stays in CI provisioning. No test code changed, so iteration 4's decision — a test states its tool as a precondition and never installs one — stands.

    **What the Test job now provisions**

    - `components: clippy` on `dtolnay/rust-toolchain@stable`, the same idiom the Clippy job (`components: clippy`) and the Rustfmt job (`components: rustfmt`) use. This gives `cargo-clippy` to `missing-docs-rust` and `complexity-rust`.
    - `Ensure jq is available` — guarded `command -v` then `brew install jq`. The second binary of the `which cargo-clippy jq` check.
    - `Install vulture` — guarded `command -v uv` then `brew install uv`, then `uv tool install vulture==2.14`, then `uv tool dir --bin >> "$GITHUB_PATH"`.
    - `Install cargo-machete` — `cargo install cargo-machete@0.9.2 --locked`.

    **Verified by reading and by running**

    - The two install commands are the exact strings the rules declare. Parsed both rule frontmatters with `yaml.safe_load` and asserted string containment against the workflow's `run` bodies: `uv tool install vulture==2.14` is `dead-code-python` install command 1 of 2 (`pipx` is its fallback, not needed once `uv` succeeds); `cargo install cargo-machete@0.9.2 --locked` is `unused-dependencies-rust`'s only install command.
    - Every binary each `doctor.check_command` names is now accounted for. `vulture sed`; `cargo cargo-machete find grep awk mktemp head cut sort tr`; `cargo-clippy jq`. `sed find grep awk mktemp head cut sort tr` are macOS base utilities and `cargo` comes from the toolchain action.
    - The four `require_tool_installed` call sites map to exactly those four rules: `:1546` `missing-docs-rust`, `:1659` `complexity-rust`, `:1759` `dead-code-python`, `:2034` `unused-dependencies-rust`.
    - The whole workflow parses under `yaml.safe_load`. The `test` job is the job that runs `cargo nextest run --no-fail-fast`, and the new steps are indices 6, 7 and 8 of 10, before `Run tests` at index 9.
    - `bash -n` on every `run` body of the `test` job: 6 of 6 exit 0.
    - Both installs are idempotent, measured on this machine: `cargo install cargo-machete@0.9.2 --locked` prints `Ignored package ... is already installed` and exits 0; `uv tool install vulture==2.14` prints `` `vulture==2.14` is already installed `` and exits 0.

    **The self-hosted runner**

    The Test job declares `runs-on: self-hosted`, a Mac. Neither `uv` nor `pipx` is guaranteed there, which is why the vulture step installs `uv` first when `command -v uv` fails. That step depends only on `brew`, which the job already depends on — the pre-existing `Ensure Chromium is available` step calls `brew install --cask chromium` on the same runner. The `jq` step depends on `brew` for the same reason. `uv tool install` links executables into `uv tool dir --bin` (`~/.local/bin` by default), which is not on the runner PATH, so the step states it in `$GITHUB_PATH`. Tool rules run through `run_shell` → `shell_command(Shell::Bash, ...)`, a non-login bash that inherits the step process env, so a `$GITHUB_PATH` entry reaches the doctor's `which`.

    **Not verified**

    The workflow itself is unverified until it runs. GitHub Actions cannot be executed from here, and the self-hosted runner's contents cannot be read from here. What is asserted about the runner is only what the workflow declares plus what its existing steps already assume.

    **Gates, all on this machine, which has all four tools**

    - `cargo nextest run --workspace` — 13961 tests run, 13961 passed, 0 skipped, 0 failed. All four asserting tool-rule tests PASS.
    - `cargo fmt --all --check` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
  timestamp: 2026-08-09T14:31:37.045816+00:00
- actor: claude-code
  id: 01kzkf03g9v0py4fn519mcz7n0
  text: |
    ### Blocker — the card grew three findings mid-run that this pass did not implement

    The 09:13 review block was rewritten while this pass was in flight. When the work started, `get task` returned that block with three findings, all in `.github/workflows/ci.yml`. When the work finished, the same block held seven: the three CI ones with shifted line anchors, a fourth CI one for `jq`, and three new ones in `crates/swissarmyhammer-validators/src/review/tool_rules.rs`. The block's own text records the change — it now carries a section titled "Correction to this pass's own earlier judgment" that withdraws the two judgments the earlier version had recorded as clear.

    All four CI findings are implemented and checked. The three below are open and are NOT in this pass's diff:

    1. `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1521` — the panic prints the rule's headline remedy, but `check_presence` fails all-or-nothing over the whole `doctor.check_command`, so a machine with clippy and without `jq` is told `rustup component add clippy`. Name the binary that actually failed, or state a `jq` remedy on the rules that need it.
    2. `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1479` — `remedy_label` joins `install.commands` with `doctor.fix_hint`, and both call sites present the result as a command to run, but several `fix_hint` values are prose. Separate the runnable commands from the advisory hint.
    3. `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1848` — `verify_shipped_tool_rules_pass_fixtures` keeps a tolerance that contradicts the required preconditions at `:1759` and `:2034`. Call `require_tool_installed` per row and delete the `exercised`/`absent` accounting.

    **Why this pass stopped instead of continuing.** Finding 3 decides what CI must provision, and it decides it differently from the CI work this pass just landed. Requiring every roster row turns `dead-code-swift` (periphery), `complexity-swift`, `missing-docs-swift`, `magic-numbers-swift` (swiftlint), and the Go, Dart and TypeScript rows into hard preconditions of `cargo nextest run` on every machine, developer laptops included. The Test job would then have to provision all of them, and the finding text itself names the Mac-only fleet as the reason. Findings 1 and 2 change the text that failure prints, so their correct wording depends on which way finding 3 goes. Choosing that direction is a decision for a person, not something to settle inside an implement pass. Reporting `stuck` with the gap named rather than `changed` presented as complete.

    **State of the tree at the stop.** HEAD `62a73bae5`, unchanged from the start of the pass. Working tree carries one source change, `.github/workflows/ci.yml`. `.kanban/tasks/01KZGVEFEJTBJXV3P93GSM2FQ8.*` and the untracked `.kanban/` entries belong to a parallel session and were not touched.

    ### implement — stuck
    - evidence: 1 file — `.github/workflows/ci.yml` (+30). 4 of 7 findings closed, 3 open and named above. `cargo nextest run --workspace` 13961 passed / 0 failed / 0 skipped; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` exit 0. Workflow YAML parses; workflow behaviour on the runner unverified until it runs. HEAD `62a73bae5`.
    - next: a person decides finding 3's direction, then findings 1 and 2 follow from it.
  timestamp: 2026-08-09T14:31:59.753556+00:00
- actor: claude-code
  id: 01kzkf4s6rxpwqzs0m1hmg293w
  text: |-
    ### finish iteration 5 — stuck
    - implement: **stuck** — 4 of 7 findings closed. One file, `.github/workflows/ci.yml`, +30 lines, all in the `test` job before `Run tests`. No test code changed, so iteration 4's decision stands
    - test: green — cargo nextest run --workspace 13961 passed / 0 failed / 0 skipped, fmt clean, clippy -D warnings clean
    - commit: 68c12d51c, parent 62a73bae5 — 3 files, 129 insertions, 1 deletion

    **What CI now provisions:** `components: clippy` on the toolchain action (the idiom the Clippy and Rustfmt jobs already use), a guarded `brew install jq`, a guarded `brew install uv` then `uv tool install vulture==2.14` then `uv tool dir --bin >> "$GITHUB_PATH"`, and `cargo install cargo-machete@0.9.2 --locked`.

    Both install strings are verbatim what the rule frontmatter declares — checked by parsing the frontmatter with `yaml.safe_load` and asserting containment against the workflow `run` bodies. Every binary each `doctor.check_command` names is accounted for. The workflow parses, `bash -n` is clean on all six `run` bodies, and both installs are idempotent (measured: exit 0 when already present). The runner is `runs-on: self-hosted`, a Mac, which is why the vulture step installs `uv` first when `command -v uv` fails — it depends only on `brew`, which the pre-existing Chromium step already assumes.

    **Not verified, and stated as such:** the workflow's behaviour until it actually runs. GitHub Actions cannot be executed from here.

    ## STUCK — three findings need a human decision

    The review block was rewritten mid-run. It held 3 findings when the implementer read it and 7 when it finished. The reviewer withdrew two of its own earlier "clear" judgments and turned them into three findings in `crates/swissarmyhammer-validators/src/review/tool_rules.rs`:

    - `:1521` — the panic prints the remedy for the rule's headline tool, but `check_presence` is all-or-nothing over the whole `doctor.check_command` (`doctor.rs:310`–`:317`). Both clippy rules check `which cargo-clippy jq`, so a machine with clippy and no `jq` is told `rustup component add clippy` — already satisfied, and unable to fix the failure it prints for.
    - `:1479` — `remedy_label` joins `install.commands` with `doctor.fix_hint` and both call sites present the result as a command. `fix_hint` is not always a command: `dead-code-swift.md:39` carries a comma clause that fails if pasted, and two rules state prose (`put the running sah binary on PATH`).
    - `:1848` — the roster tolerance contradicts the single-rule preconditions. `SHIPPED_UNUSED_DEPENDENCY_RULES` is a single row, so `exercised > 0` there is exactly "cargo-machete is present" said differently, while the file claims two contracts for that rule. The remedy asks that `require_tool_installed` be called per row.

    **Why this stopped rather than being settled inside an implement pass.** Finding `:1848` decides what every machine must provision, and decides it differently from the CI work just landed. Requiring every roster row makes `periphery`, `swiftlint`, `golangci-lint`, `eslint` and the Dart tools hard preconditions of `cargo nextest run --workspace` — on CI and on every developer laptop. Findings `:1521` and `:1479` change the text a failure prints, so their wording depends on which way `:1848` goes. That is a change to the developer contract, beyond this card's subject, and it is a person's call.

    Finding counts by iteration: 16, 2, 4, 3→7. No finding has repeated, so the mechanical guardrail is NOT hit. The card is stuck on a decision, not on a fix that will not take.

    ## A card-file incident to record

    The iteration-4 reviewer wrote findings by appending to `.kanban/tasks/01KZGSFWP4ZJKVPQRX8A4EBNW3.md` directly and then truncating the file to replace its own section. The truncation clobbered the front matter and reverted the card to `doing`. It restored a backup, re-appended, and moved the card back. It reports the description, all four findings sections, the 22 checked and 7 unchecked items, the 20 comments, tags and dependencies as intact, and the event log as grown from 36 to 43 with nothing lost. One artifact remains: `get task` reports `created` as today, and `started`/`updated` as null. The pre-change file is in git if those values are wanted back.

    Findings must be written through the kanban API, never by editing the card file.
  timestamp: 2026-08-09T14:34:33.048402+00:00
- actor: claude-code
  id: 01kzkgm6hceqpn34efd7g15p12
  text: |
    ### Iteration 5 — the install path is restored, and every roster row is required

    The owner reversed iteration 4. Installing is the intended design, so a tool-rule test now gets its tool from the rule's own `tool.install.commands`, never from commands written in test code.

    **`tool_install.rs` — the install is serialized machine-wide.** `InstallLock` takes an exclusive `flock(2)` on one file in the temp directory, held for as long as the install commands run. `install_tool_commands` checks presence, takes the lock, then checks presence again, because another installer can finish while this one waits. `serial_test` was not enough: it holds only inside one process, and `cargo nextest` gives each test its own process. `fs2` locks the open file description, so two threads that each open the lock file contend through the same call two processes use — which is what makes the RED honest.

    RED: `installs_never_overlap` wrote `entered, entered, entered, entered, left, left, left, left` — four installers inside one destination together. GREEN: `entered, left, entered, left, ...`.

    **`:1848` — the roster/single-rule contradiction is gone.** `verify_shipped_tool_rules_pass_fixtures` calls `require_tool_installed` for each row and then asserts `row.usable()` with no condition. The `exercised`/`absent` accounting and `absent_rule_label` are deleted. The degradation contract keeps its own two tool-free tests, so nothing lost coverage.

    RED: the change made three rows fail that the tolerance had hidden — `unused-code-go` (staticcheck), `magic-numbers-go` (golangci-lint) and `complexity-go` (gocognit).

    **`:1521` — the message names the binary that actually failed.** `checked_binaries` reads the names back out of `doctor.check_command`, and `missing_label` probes each one. `which cargo-clippy jq` failing on `jq` now says `jq`. A check that fails on something that is not a binary — `dead-code-swift` asks `test -f Package.swift` — is named by its command.

    **`:1479` — runnable and advisory are separate lines.** `remedy_label` is deleted. `precondition_report` puts `install.commands` after `run:` and `doctor.fix_hint` after `advice for a person, not a command to run:`. Three new tests hold that shape, including the case of a rule with no install command, which offers no `run:` line at all.

    ### Two defects the roster requirement uncovered

    1. **A bare `go install` does not satisfy its own doctor check.** It writes `$(go env GOPATH)/bin`, which a default PATH does not hold, so `which gocognit` still failed after the command exited 0. The five Go rules now state `mkdir -p "$HOME/.local/bin" && GOBIN="$HOME/.local/bin" go install ...`, which lands the binary in the same directory `uv tool install` and `pipx install` use. The version pin is unchanged. `builtin/validators/README.md` states the rule: an install command must put the binary where `check_command` can find it.

    2. **`dead-code-swift` states a working-directory precondition, not only a tool.** Its check is `which periphery swift jq && test -f Package.swift`, because periphery scans a built SPM package. No install command can satisfy that half. The roster helper therefore runs from a Swift package root of its own, built from the shipped `Package.swift.tmpl` fixture and pinned with `CurrentDirGuard`. The fixture runs are unaffected: doctor materializes each pair into its own scratch directory.

    ### CI

    The provisioning from `68c12d51c` stays. Four steps were added for the rows the roster now requires: `$HOME/.local/bin` on PATH, Node for the `npm install -g` rules, the Go and Swift toolchains, and Homebrew installs for `swiftlint`, `periphery` and `dart` — the three tools that declare no install command and so cannot install themselves.
  timestamp: 2026-08-09T15:00:26.796471+00:00
- actor: claude-code
  id: 01kzkgmh32j6q1btgnzrje73zt
  text: |
    ### implement — changed
    - evidence: 10 files — crates/swissarmyhammer-validators/src/review/tool_install.rs, crates/swissarmyhammer-validators/src/review/tool_rules.rs, crates/swissarmyhammer-validators/src/doctor.rs, crates/swissarmyhammer-validators/Cargo.toml, Cargo.lock, .github/workflows/ci.yml, builtin/validators/README.md, builtin/validators/code-hygiene/rules/{complexity-go,function-length-go,magic-numbers-go,missing-docs-go,unused-code-go}.md
    - findings addressed: tool_rules.rs :1521, :1479, :1848 — the review step verifies and checks them
    - RED: `installs_never_overlap` logged `entered, entered, entered, entered, left, left, left, left` before the lock; the roster change failed `unused-code-go`, `magic-numbers-go` and `complexity-go`, three rows the deleted tolerance had hidden
    - gates: `cargo nextest run --workspace --no-fail-fast` 13966 passed, 0 failed, 0 skipped; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean
    - HEAD: 68c12d51c
    - next: /review
  timestamp: 2026-08-09T15:00:37.602112+00:00
- actor: claude-code
  id: 01kzkhxh8kwc280b2f6xk3axsb
  text: |
    ### review — findings

    - scope: `review sha 68c12d51c..615593ecb`
    - evidence: 8 new findings — 4 from the engine (17 pairs attempted, 0 failed, 1 file skipped over the prompt cap), 4 from the directed judgment. `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1`, `:368`, `:440`, `:626`, `:2044`; `crates/swissarmyhammer-validators/src/review/tool_install.rs:295`, `:308`, `:407`.
    - directed judgment A (install lock): 3 findings. The agent half of the lifecycle runs outside the lock; the lock scope is per-`TMPDIR`, not the machine-wide scope the doc states; the wait has no deadline and is not reentrant. Clear on the rest — the location survives a read-only HOME and degrades with a warn, the double-check closes the race (`let _lock`, not `let _`), and the lock releases on panic and on kill.
    - directed judgment B (Go `GOBIN`, `CurrentDirGuard`): clear. All five Go doctor checks pass against `go install` binaries, `golangci-lint` reports the pinned 2.12.2. `CurrentDirGuard` holds a process-global mutex for its whole life, so isolation does not rest on nextest. One finding on the drop order of the `(TempDir, CurrentDirGuard)` pair.
    - directed judgment C (CI): clear. All 24 required roster rows resolve on the self-hosted Mac runner. No rule is required with neither an install command nor a CI step. Every pinned npm version resolves; the `dart-sdk` and `swiftlint` formulae exist; the periphery tap still resolves.
    - tests: `cargo nextest run -p swissarmyhammer-validators` over the 5 roster tests, the 4 single-rule acceptance tests and the 4 new precondition tests — 17 run, 17 passed, 0 failed.
    - next: address the 8 new findings and the 3 still-open findings from the 09:13 pass, then re-review. The card stays in `review`.
  timestamp: 2026-08-09T15:23:01.267096+00:00
- actor: claude-code
  id: 01kzkhzajhc546c8k1w9fm1jee
  text: |-
    ### finish iteration 6 — findings
    - implement: changed — 10 files. Install path restored per the owner's decision; every roster row required. `InstallLock` takes an exclusive `flock(2)`. Three prior findings addressed
    - test: green — cargo nextest run --workspace --no-fail-fast 13966 passed / 0 failed / 0 skipped, fmt clean, clippy -D warnings clean
    - commit: 615593ecb, parent 68c12d51c — 14 files, 565 insertions, 88 deletions
    - review: findings — 8 new, on `review sha 68c12d51c..615593ecb`. Engine: 17 pairs attempted, 4 confirmed, 0 failed, **1 file skipped over the prompt cap**. 4 more from the directed judgment

    **The roster requirement paid for itself at once.** Removing the tolerance exposed two defects in shipped rules that it had been hiding:
    - A bare `go install` writes `$(go env GOPATH)/bin`, which no default PATH holds, so **five Go rules could never satisfy their own doctor check**. They now set `GOBIN="$HOME/.local/bin"`, where `uv` and `pipx` already write. Verified: all five checks exit 0 and `golangci-lint --version` reports the pinned 2.12.2 from a `go install` build
    - `dead-code-swift` checks `test -f Package.swift` — a working-directory precondition no install can meet

    **RED was real twice.** `installs_never_overlap` logged `entered,entered,entered,entered,left,left,left,left` before the lock existed. The roster change failed `unused-code-go`, `magic-numbers-go` and `complexity-go` before the `GOBIN` fix.

    **What the lock gets right, verified rather than assumed.** The lock file sits under `std::env::temp_dir()`, which reads `$TMPDIR` and falls back to `/tmp` — neither derives from `HOME`, so a read-only HOME still gets a lock, and `acquire()` warns and returns `None` rather than failing. The double-check closes the race: presence, acquire, presence again, then every command. The binding is `let _lock`, not `let _` — the difference that would have made the whole lock a no-op. It releases on an unwinding panic through `Drop`, and on `SIGKILL` through the kernel closing the descriptor.

    **The 8 findings.**
    - `tool_rules.rs:1` — **the file is now 262265 rendered bytes against the 262144-byte cap**, so `duplication` could not review it. This is the same blind spot that hid nine validators from `code_context/mod.rs` on `^n0680p8`. Split it
    - `tool_rules.rs:368`, `:440` — `MatchedToolRule` and `ProjectToolRule` do not derive `Debug`
    - `tool_rules.rs:626` — `ScriptFailure` wraps `std::io::Error` but does not implement `std::error::Error`, so the error chain breaks
    - `tool_rules.rs:2044` — `swift_package_root` returns `(TempDir, CurrentDirGuard)` and the caller binds the pair to one name, so `TempDir` drops FIRST and removes the directory while it is still the process working directory. The type's own doc example shows the opposite order
    - `tool_install.rs:407` — the agent turn runs OUTSIDE the lock; the guard died when `install_tool_commands` returned. That is the half running commands no rule declared
    - `tool_install.rs:295` — the lock is per-`TMPDIR`, not machine-wide as the doc claims. Three required rules name `brew install swiftlint`, and Homebrew writes `/opt/homebrew`, shared by every user
    - `tool_install.rs:308` — `lock_exclusive()` waits with no deadline and `acquire()` opens the file fresh each call, so `flock(2)` conflicts even inside one process. A re-entrant call blocks forever with no log line. `sah init` pre-installs through this same function

    **CI is clear.** All 24 required rows resolve on the self-hosted Mac runner; no rule is required that CI cannot satisfy. The reviewer checked the externals rather than trusting them — all six pinned npm versions resolve on the registry, `dart-sdk` and `swiftlint` are in homebrew-core, and the `peripheryapp/homebrew-periphery` tap still returns HTTP 200.

    `CurrentDirGuard` isolates on its own merits, not on nextest's: it holds a `MutexGuard<'static, ()>` on a process-global lock for its whole life.

    Finding counts by iteration: 16, 2, 4, 3→7, 8. No finding has repeated.

    Guardrail: NOT hit.
  timestamp: 2026-08-09T15:23:59.953691+00:00
- actor: claude-code
  id: 01kzkjc286rgaa08b0cm17v7z7
  text: |-
    Picked up the eight open findings of the 2026-08-09 10:02 pass. HEAD is 615593ecb, as expected.

    Research notes before the edits:

    - Drop order (`tool_rules.rs:2044`). Confirmed. `swift_package_root` returns `(tempfile::TempDir, CurrentDirGuard)`; the caller binds `let _package_root = ...`, and a tuple drops field 0 first. So `TempDir` removes the directory while it is still the process working directory. Measured on this machine: `getcwd()` on a removed directory fails with ENOENT. That gives a real run-time RED — drop the first element of the pair, then call `current_dir()`.
    - Prompt cap (`tool_rules.rs:1`). The file is 2271 lines / 91180 bytes. Lines 728-2271 are one `#[cfg(test)] mod tests`, which is 1543 of the 2271 lines. The gutter `{line:>6} | {sha:8} {mark} | ` in `fleet/render.rs` costs 22 bytes for each line, and the per-file probe evidence adds the rest. The plan is to move the test module to `tool_rules/tests.rs` and split it into four files by subject.
    - Install lock (`tool_install.rs`). `fs2` gives `try_lock_exclusive` and `lock_contended_error`, so a bounded wait with a log line is available.
    - Debug sweep. `MatchedToolRule` and `ProjectToolRule` miss `Debug` in `tool_rules.rs`; `InstallLock` misses it in `tool_install.rs`, and the test-only `ScriptedInstallAgent` misses it too. `ScriptFailure` is the only error type in either file that breaks the chain.
  timestamp: 2026-08-09T15:30:57.414846+00:00
- actor: claude-code
  id: 01kzkkk23twkyktxety00yhd8y
  text: |-
    Work log for the eight findings of the 2026-08-09 10:02 pass.

    **RED evidence, where the defect is a run-time defect.**

    1. Drop order. Added `the_swift_package_root_restores_the_directory_before_it_removes_it`, which drops the first element of the returned pair and then reads `current_dir()`. Against the old order it failed with `left: None` — `getcwd` reported ENOENT, because `TempDir` had removed the directory the process still stood in. With the order swapped to `(CurrentDirGuard, TempDir)` the test passes.
    2. Install-lock deadlock. Added a temporary probe that called `InstallLock::acquire()` twice in one process. `timeout 60 cargo nextest run ...` killed it with SIGTERM at 56 s with no output — the second call never returned. The probe was then replaced by `a_contended_install_lock_gives_up_instead_of_waiting_for_ever` and `the_bounded_wait_takes_the_lock_the_holder_releases`, which drive `InstallLock::take` with a 50 ms deadline. The first passes in 0.076 s.

    **Behaviour-preserving, said plainly.** The file split, the two `#[derive(Debug)]` additions, the `InstallLock` and `ScriptedInstallAgent` `Debug` derives found by the sweep, and the `thiserror` conversion of `ScriptFailure` change no behaviour. No fake RED was staged for them. The split moved test code between files without editing it, except for the per-module import lists and the two items two modules share: `TODO_SCRIPT` moved up to `tests.rs`, and `require_tool_installed` is `pub(super)`.

    **How the split was made.** A script cut the test module into line ranges and removed four spaces of indentation from every line outside the one multi-line raw string, whose indentation is fixture text. The raw string (`COMPLEX_LIB_RS`) was read back afterwards and is byte-identical.

    **Sizes after the split**, as file bytes plus the 22-byte gutter each line costs:

    | file | lines | bytes | gutter-rendered |
    |---|---|---|---|
    | `tool_rules.rs` | 740 | 26671 | 42951 |
    | `tool_rules/tests.rs` | 270 | 10890 | 16830 |
    | `tool_rules/tests/plan.rs` | 345 | 12209 | 19799 |
    | `tool_rules/tests/execute.rs` | 154 | 5372 | 8760 |
    | `tool_rules/tests/preconditions.rs` | 237 | 9069 | 14283 |
    | `tool_rules/tests/shipped.rs` | 605 | 24868 | 38178 |

    The engine measured the old single file at 262265 rendered bytes against 91180 file bytes, so the per-file probe evidence and block framing cost about 1.9 times the gutter-rendered figure. Even at that whole ratio the largest file of the family lands near 80 KB, under a third of the 262144-byte cap.

    **Decisions taken on the three lock findings.**

    - The agent turn (`:407`): fixed the code. `ensure_tool_installed` now takes one lock and holds it over both halves. `install_tool_commands` keeps its own lock for the callers that use it alone (`sah init`), and both call the new `run_declared_install_commands`, which assumes the caller holds the lock. Taking a second lock inside the first would deadlock, which is the same defect finding `:308` names.
    - The lock scope (`:295`): corrected the doc. `$TMPDIR` covers every destination a rule's install commands write — `~/.local/bin`, `~/.cargo/bin`, the npm and go bin directories are all per user. The one destination it does not cover is the Homebrew prefix, and Homebrew locks per formula itself. Moving the lock to a path every user shares would need a world-writable file that any user could hold for ever, which is worse than what it buys. The doc now states the reach and names the exception.
    - The unbounded wait (`:308`): fixed the code. `INSTALL_LOCK_WAIT` is 300 s, longer than the slowest declared install (`cargo install cargo-machete@0.9.2 --locked` builds from source). `InstallLock::take` writes one `info` line naming the lock path when it finds the lock held, retries every 100 ms, and writes a `warn` line and installs unserialized when the deadline passes.

    **Sweep beyond the named lines.** `tool_rules.rs`: `MatchedToolRule` and `ProjectToolRule` were the only production types with no `Debug`; `ScriptFailure` was the only error type that broke the chain. `tool_install.rs`: `InstallLock` had no `Debug`, and the test-only `ScriptedInstallAgent` had none either. `PoolInstallAgent` keeps its hand-written `Debug`, and `ToolFallback` keeps `Display` with no `Error`, which its doc states is deliberate.
  timestamp: 2026-08-09T15:52:15.226572+00:00
- actor: claude-code
  id: 01kzkkkbc0ne4hkgnh4rp303m7
  text: |-
    ### implement — changed
    - evidence: 8 files — `crates/swissarmyhammer-validators/src/review/tool_rules.rs`, `crates/swissarmyhammer-validators/src/review/tool_install.rs`, and six new files `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs` plus `tool_rules/tests/{plan,execute,preconditions,shipped}.rs`. All 11 open findings checked (8 of the 2026-08-09 10:02 pass, and the 3 the same pass verified in the 09:13 section). RED proved for both run-time defects: `left: None` from `current_dir()` on the drop-order test, SIGTERM at 56 s on the double-`acquire()` probe. Gates: `cargo nextest run --workspace` 13969 run, 13969 passed, 0 failed, 0 skipped; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. Largest file of the split family: `tool_rules.rs`, 42951 gutter-rendered bytes against the 262144-byte cap. HEAD 615593ecb.
    - next: `/review`
  timestamp: 2026-08-09T15:52:24.704694+00:00
- actor: claude-code
  id: 01kzknjcaws3qxyne9trx6c3sm
  text: |-
    ### review — findings

    - evidence: 4 findings on scope `615593ecb..087d052b4` — `crates/swissarmyhammer-validators/src/review/tool_rules/tests/preconditions.rs:49`, `crates/swissarmyhammer-validators/src/review/tool_install.rs:77`, `tool_install.rs:354`, `tool_install.rs:315`. Engine fleet: 18 pairs attempted, 0 failed, 0 skipped. The engine reported 6; five were dropped under the skill's exception on refactoring test code that already existed at `615593ecb`.
    - directed judgment A — the split: CLEAN. Zero prompt-cap skips, 0 per file across all six files of the family. `tool_rules.rs` went 2271 lines / 91180 raw bytes (262265 rendered, the file that was skipped) to 740 lines / 26671 bytes; no file of the family is within a factor of six of the 262144-byte cap. The one deletion hunk `@@ -726,1546 +737,4 @@` begins on the `mod tests {` line, so the split moved tests only; the other changes to the file are the previous pass's own fixes (`Debug` derives, `ScriptFailure` on `thiserror`). Tests 30 before, 31 after — zero lost, one gained (`the_swift_package_root_restores_the_directory_before_it_removes_it`). Function names 83 before, 83 after, same membership.
    - directed judgment B — the lock deadline: TWO FINDINGS. The 300 s bound is justified against declared install commands only, while the same lock is held across the agent turn (`PROMPT_TURN_CEILING` 2700 s, `PROMPT_IDLE_TIMEOUT` 300 s), so a waiter times out while a healthy holder still runs. The timeout branch then installs unserialized on the one evidence a holder is live, and the prompt fallback that failing would degrade onto is already held by two tests.
    - directed judgment C — the Homebrew claim: VERIFIED TRUE. `FormulaInstaller#install` → `lock` → `Formula#lock` → `FormulaLock < LockFile` → `flock(LOCK_EX | LOCK_NB)`, lock files under `/opt/homebrew/var/homebrew/locks`. But one destination falls outside both locks: under a Homebrew node `npm install -g` writes the shared `/opt/homebrew` prefix and takes no brew lock — recorded as a finding.
    - next: the four items stay open; the card stays in `review`.
  timestamp: 2026-08-09T16:26:50.076145+00:00
- actor: claude-code
  id: 01kzknm1n0ed9sh1w7dng5v8k0
  text: |-
    ### finish iteration 7 — findings
    - implement: changed — 11 findings closed. Drop-order bug fixed, file split, lock bounded, `Debug` and error chain swept
    - test: green — cargo nextest run --workspace 13969 passed / 0 failed / 0 skipped, fmt clean, clippy -D warnings clean
    - commit: 087d052b4, parent 615593ecb — 9 files, 2007 insertions, 1577 deletions
    - review: findings — 4 open, on `review sha 615593ecb..087d052b4`. Engine: 18 pairs attempted, **0 failed, 0 skipped**, 6 reported, 5 released by the pre-existing-test exception

    **Both real bugs were proved RED.** The drop-order test failed with `current_dir()` returning `None`, because `getcwd` gives ENOENT once the directory the process stands in is removed. The deadlock probe — `acquire()` twice in one process — was killed by SIGTERM at 56 s with no output.

    **The split worked, and the reviewer proved it rather than assuming it.** Prompt-cap skips are now ZERO on every file of the family, against one skipped file last pass. The mass was not merely moved: `tool_rules.rs` was 2271 lines / 91180 raw bytes / 262265 rendered, and is now 740 lines / 26671 bytes. It carries exactly one deletion hunk, `@@ -726,1546 +737,4 @@`, starting on the `mod tests {` line; every other change to it is a previous finding's fix. Tests 30 before, 31 after — zero removed, one added (`the_swift_package_root_restores_the_directory_before_it_removes_it`). Function names 83 before, 83 after, same membership.

    **The four open findings are all about install concurrency, not duplication.**
    - `tool_install.rs:77`, `:354`, `:315` — the 300 s deadline is justified by the slowest DECLARED install, but the lock is also held across `agent.install(...).await`. The pool's `PROMPT_TURN_CEILING` is 2700 s and `PROMPT_IDLE_TIMEOUT` is 300 s, so a waiter burns its whole deadline while the holder sits in a turn the pool still calls healthy — the timeout becomes the ordinary outcome, not the exceptional one. Separately, `acquire` returns `None` both when no holder exists (harmless) and when a holder held throughout (a live race), and the caller cannot tell them apart. Failing on the deadline branch is safer, and is not a new degradation: `plan.rs:154` and `plan.rs:310` already hold the prompt fallback
    - `tests/preconditions.rs:49` — a binary name is interpolated into a shell string

    **The Homebrew claim was verified in Homebrew's own source, not accepted.** `FormulaInstaller#install` calls `lock` (`formula_installer.rb:550`); `Formula#lock` builds a `FormulaLock` (`formula.rb:1897`); `FormulaLock < LockFile` takes `flock(File::LOCK_EX | File::LOCK_NB)` (`lock_file.rb:44`) under the machine-shared `/opt/homebrew/var/homebrew/locks`. So the doc's exception holds. **But one destination falls outside BOTH locks:** `/opt/homebrew/lib/node_modules/npm/npmrc` sets `prefix = /opt/homebrew`, so under a Homebrew node `npm install -g` writes the shared prefix and takes no brew lock, because it is not a brew operation. Four shipped rules declare `npm install -g`.

    This reviewer ran no background sub-agents and wrote every finding through the kanban API. The card file keeps its front matter, all six findings sections and 37 checked marks.

    Finding counts by iteration: 16, 2, 4, 3→7, 8, 4. No finding has repeated.

    Guardrail: NOT hit.

    ---

    **Scope observation for a human.** The card's own subject — duplication as a deterministic tool rule with zero LLM calls — has been finished and clean since iteration 1. Every iteration since has followed the blast radius of its own fixes into the tool-rule test harness and now into install-lock concurrency. The four open findings name `tool_install.rs` and a test helper; none of them is about duplication. This is the review engine working correctly, and the work is real — it has already found five Go rules that could never satisfy their own doctor check, a deleted-CWD bug, and a silent deadlock — but it now belongs to a different subject.
  timestamp: 2026-08-09T16:27:44.672670+00:00
- actor: claude-code
  id: 01kzknrnxdv8f3z4epyehxvy9t
  text: |-
    ### The four open findings moved to `^t0dgame`

    The owner decided this card is done on its own subject, and that the remaining findings belong to a different one.

    Moved to `^t0dgame` — "tool install lock: bound the agent turn, tell a timeout from an idle lock, cover the npm shared prefix":
    - `tool_install.rs:77` — the 300 s deadline against an agent turn whose pool ceiling is 2700 s
    - `tool_install.rs:354` — the timeout branch installs unserialized, which is the race the lock prevents
    - `tool_install.rs:315` — `acquire` returns `None` for both an idle lock and a live race
    - `tests/preconditions.rs:49` — a binary name interpolated into a shell string

    The new card also carries the npm finding this pass turned up: `npm install -g` under a Homebrew node writes the shared `/opt/homebrew` prefix and takes no brew lock, so it falls outside both the `$TMPDIR` lock and Homebrew's own. Four shipped rules declare it.

    The four boxes above stay unchecked on this card on purpose. They were not fixed here, and checking them would say they were.

    ## What this card delivered

    Duplication is a deterministic tool rule. `cpd-core` 0.1.7 (jscpd's Rust engine, MIT) supplies the Rabin-Karp detector; the token stream comes from this workspace's own tree-sitter roster, so the same parse that finds a clone decides what is test code. `supersedes: [duplication, rust, swift]` names every rule in the four-rule set, and `prompt_rules_for` returns an empty reading list for a matched file — **zero LLM calls for the set**, held by test, with `attempted() == 1` and no fallbacks. One new package, zero new transitive crates.

    Structural test exclusion is decided by the parse and never by a path: it removes 4132 of 5077 raw findings, 81.4%, and a path glob reaches none of them.

    Seven iterations. Finding counts 16, 2, 4, 3→7, 8, 4 — no finding ever repeated, so the guardrail never fired. The card stopped because its subject was finished, not because it was stuck.

    **Defects found along the way that had nothing to do with duplication, and would not have surfaced without it:**
    - Five Go rules could never satisfy their own doctor check — a bare `go install` writes `$(go env GOPATH)/bin`, off a default PATH. Found only when the roster tolerance was removed
    - `swift_package_root` deleted a temp directory while the process still stood in it. RED: `current_dir()` returned `None`
    - `acquire()` twice in one process deadlocked silently. RED: killed by SIGTERM at 56 s
    - CI installed none of the tools the asserting tests required — found by reading the workflow
    - `tool_rules.rs` had grown past the review prompt cap, hiding it from a validator. Split; prompt-cap skips are now zero across the whole family

    Commits: `68092a2ff`, `bc83a4dd5`, `4ac042a8b`, `62a73bae5`, `68c12d51c`, `615593ecb`, `087d052b4`. Nothing pushed.
  timestamp: 2026-08-09T16:30:16.493790+00:00
- actor: claude-code
  id: 01kztetpvxvjjdm437xqq22bbb
  text: |-
    ## SUPERSEDED — 2026-08-12

    The STANDING ORDER in this card is no longer live. Do not act on it.

    This card built `duplication-parsed`. That rule spawned bash, which ran the
    `sah` binary again, to reach `find_duplicates_in` — a function already linked
    into the calling process and already called in-process by the `duplicates`
    probe. ^wwb6hk7 deletes the rule and the whole self-shell contract with it.

    The `duplication`, `rust` and `swift` prompt rules decide again, and the
    `duplicates` probe supplies their machine facts. That reverses this card's
    "supersede the prompt rules" order deliberately, with the user's decision on
    record.
  timestamp: 2026-08-12T07:43:43.997128+00:00
depends_on:
- 01KZFHBM7MWFYAMK3SQADF0D7H
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffd180
title: 'duplication goes objective: sah duplicates tool rule supersedes the prompt rule'
---
## STANDING ORDER

This card is a decision, not a proposal. Your job is to implement it exactly.
- Do NOT re-run the jscpd-versus-probe evaluation. It happened (^3b49ewn) and its conclusion is overridden here.
- Do NOT keep the prompt rules running for matched files. Supersede them as stated.
- Do NOT soften the gate into evidence for an LLM. The detector decides. Zero LLM calls for this set on matched files is the acceptance bar.
- Do NOT file a follow-up card in place of doing the work.
- Stop and escalate ONLY when a step is impossible. Report the exact command and its output.

## The work

Correction to ^3b49ewn. That card asked the wrong question — it compared detectors and kept the existing implementation, which left duplication on the slowest component, an LLM pass. The point is review speed. Make the detector the decider: a deterministic tool rule, zero LLM.

Detector choice — decide with a look at the source, in this order:

1. jscpd ships a Rust engine: https://github.com/kucherenko/jscpd/tree/master/rust (MIT). If it is usable as a crate, embed it in the sah op directly — token-based Rabin-Karp duplicate detection, no Node install, language-aware tokenizers. Token-hash detection is the right algorithm for a deterministic near-verbatim gate: exact spans, exact repeat counts, no similarity fuzz.
2. If that crate is immature, implement the same algorithm natively: Rabin-Karp over the tree-sitter token stream we already produce, minimum window ~50 tokens. It is a small, well-known algorithm.
3. The existing cosine `duplicates` probe does not feed this rule — its strength (fuzzy near-matches) is what the deleted judgment tier consumed. ^adf0d7h (intra-file blindness) gets fixed if the cosine probe remains in use elsewhere; otherwise fold its test cases into this op's tests and close it against this card.

The rule:

- New sah op: run the detector over the file arguments, emit one finding per clone pair: `path:line: verbatim duplicate of <path:line> (<n> lines / <t> tokens)`. Deterministic gate only — a token-identical window over the minimum size IS a finding.
- Structural test exclusion, deterministic: drop a finding whose span sits inside a test node — `#[cfg(test)]` / `#[test]` in Rust, framework markers at the definition in other grammars — decided by the tree-sitter parse, never by file path.
- Inline suppression: a marker comment on the block (one form, e.g. `// sah:allow duplication <reason>`), honored across comment syntaxes. Exemptions live in code, never prose.
- Tool rule `duplication-parsed` in the `duplication` set: files scope, `run: sah <op> "$@"`, `supersedes: [duplication, rust, swift]`. Match lists the grammar roster's extensions; languages without a grammar keep the prompt path. Doctor names the sah binary; no install commands.
- Fixtures: fail = two identical 15-line blocks in one file (proves the intra-file case); pass = a suppressed copy with the marker, a duplicate pair inside `#[cfg(test)]`, and a below-minimum window. Extend the shipped-rules acceptance test. Acceptance: a review whose only defect is a pasted block reports it with zero LLM calls for the duplication set.

Depends on ^adf0d7h (or close it into this card per point 3).

#tool-validators #objectivity

## Review Findings (2026-08-09 07:26)

- [x] `apps/swissarmyhammer-cli/Cargo.toml:38` — unused dependency `swissarmyhammer-code-context`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `apps/swissarmyhammer-cli/Cargo.toml:58` — unused dependency `is-terminal`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `apps/swissarmyhammer-cli/Cargo.toml:59` — unused dependency `dirs`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `apps/swissarmyhammer-cli/Cargo.toml:66` — unused dependency `futures-util`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `apps/swissarmyhammer-cli/Cargo.toml:85` — unused dependency `chrono`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `apps/swissarmyhammer-cli/Cargo.toml:90` — unused dependency `glob`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `apps/swissarmyhammer-cli/Cargo.toml:91` — unused dependency `ignore`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `apps/swissarmyhammer-cli/Cargo.toml:96` — unused dependency `libc`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `apps/swissarmyhammer-cli/Cargo.toml:98` — unused dependency `scopeguard`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `apps/swissarmyhammer-cli/Cargo.toml:99` — unused dependency `sha2`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `apps/swissarmyhammer-cli/Cargo.toml:105` — unused dependency `reqwest`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `apps/swissarmyhammer-cli/tests/duplication_tool_rule.rs:149` — The `duplication_work` test helper (lines 149–163) is 0.89 similar to `ruleset_with_body` in `crates/swissarmyhammer-validators/src/review/fleet/tests.rs:49`. Both construct validation work structures with similar patterns. This test infrastructure should be examined for shared utility extraction. Review whether `duplication_work` can extend or reuse `ruleset_with_body`, or extract common test work-list construction patterns to a shared helpers module.
- [x] `apps/swissarmyhammer-cli/tests/duplication_tool_rule.rs:166` — The `builtin_loader` function reimplements a helper that already exists in `apps/swissarmyhammer-cli/tests/commented_code_tool_rule.rs:134` (identical, 1.00 similarity). Test infrastructure like this should be extracted to a shared utility to avoid maintenance burden. Extract `builtin_loader` to a shared test utility (e.g., in `crates/swissarmyhammer-validators/src/review/test_support.rs` or a test helpers module) and call it from both test files.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs:26` — TokenPoint implements Eq but not Hash; Rust's Eq-implies-Hash rule requires consistency between equality and hashing to maintain invariants in HashMap/HashSet. Add Hash to the derive list: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]`.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs:41` — DuplicationToken implements Eq but not Hash; Rust's Eq-implies-Hash rule requires consistency between equality and hashing to maintain invariants in HashMap/HashSet. Add Hash to the derive list: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/execute.rs:319` — execute_find_duplication (lines 319–333) and execute_find_commented_code (lines 348–362) are near-verbatim duplicates: parameter extraction, working directory resolution, operation call, and result formatting are identical, differing only in the function invoked (find_duplication vs find_commented_code). Two blocks that differ only by a value are one function with an argument. Extract a shared handler function parameterized by the operation callback: extract_operation_result(args, context, operation_fn) where operation_fn is find_duplication or find_commented_code. This eliminates the repeated parameter-extraction, directory-resolution, and formatting logic.

## Review Findings (2026-08-09 08:18)

Scope: `68092a2ff..bc83a4dd5`.

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1707` — Test silently returns without asserting anything if the rule is not in the plan. If the tool is not installed or the rule is not planned, the test passes with zero assertions, violating the principle that every test should be able to fail. Either: (1) add an `assert!(run.is_some(), "shipped Python dead-code tool must plan a run")` before the return to make the test fail when the tool is missing, or (2) mark the test with `#[ignore]` and document why it requires the tool, or (3) use `install_project_tool_rules()` like `verify_shipped_tool_rules_pass_fixtures` does (line 1765) to guarantee the tool is installed before testing.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1961` — Test silently returns without asserting anything if the rule is not in the plan. If the tool is not installed, the test passes with zero assertions, violating the principle that every test should be able to fail. Either: (1) add an assertion before the return: `assert!(run.is_some(), "shipped Rust unused-dependency tool must plan a run")`, or (2) mark the test with `#[ignore]` and document why it requires the tool, or (3) use `install_project_tool_rules()` like the helper at line 1765 does to guarantee the tool is installed before testing.

### Directed judgments this pass — both clear

- Twelve deleted CLI dependencies: 0 unsafe of 12. No removed line carried `features`, `default-features`, `package =`, or `optional` — every one was a bare `workspace = true`, so it requested exactly the workspace-root baseline and could never have been the sole source of a feature another crate unified in. Removal cannot subtract from the resolved feature set. No `[features]` table exists in `apps/swissarmyhammer-cli/Cargo.toml`, so no `dep:` or implicit optional-dependency feature can break. No rename, no macro-expansion vector, no per-crate pin. `is_terminal()` resolves to `std::io::IsTerminal` (`apps/swissarmyhammer-cli/src/banner.rs:6`, `src/main.rs:367`, `src/main.rs:933`), and the `dirs`/`glob`/`ignore` text hits are prose, not code.
- `test-support` feature gate: sound. `crates/swissarmyhammer-validators/Cargo.toml:107` declares `test-support = ["model-embedding/test-support"]`; there is no `default` key, so the feature is off unless asked for. `crates/swissarmyhammer-validators/src/review/mod.rs:21` gates the module with `#[cfg(any(test, feature = "test-support"))]`, so no test-only code reaches a release build, and every non-doc reference to `test_support` sits inside a `#[cfg(test)]` module. The four routed call sites carry no behavioural difference: same purpose strings, same rule order, same content constants, same `ProbeNames::new([])` and empty `FileWork` arguments. No test lost an assertion, was deleted, or was ignored in this range.

## Review Findings (2026-08-09 08:50)

Scope: `3773be9a3..4ac042a8b` — the checkpoint commit `4ac042a8b` only.

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1481` — This acceptance test executes tool rules (via `verify_run_reports_one_finding` → `execute_tool_runs`), but does not install the tool first. Under nextest where each test runs in its own process, the Rust missing-docs tool may not be present. The invariant—call `install_project_tool_rules` before planning/executing tool rules—was added to similar tests at lines 1701 and 1959 but not here. Add `let project_types = ["rust"]; crate::review::tool_install::install_project_tool_rules(&loader, &project_types);` before line 1493, and replace `&[]` with `&project_types` in the `plan_tool_rules` call on that line.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1595` — This acceptance test executes tool rules (via `verify_run_reports_one_finding` → `execute_tool_runs`), but does not install the tool first. Under nextest where each test runs in its own process, the Rust complexity tool may not be present. The invariant—call `install_project_tool_rules` before planning/executing tool rules—was added to similar tests at lines 1701 and 1959 but not here. Add `let project_types = ["rust"]; crate::review::tool_install::install_project_tool_rules(&loader, &project_types);` before line 1604, and replace `&[]` with `&project_types` in the `plan_tool_rules` call on that line.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1701` — The added `install_project_tool_rules(&loader, &["python"])` call writes outside the test's temp dir. The test opens `let repo = tempfile::tempdir().unwrap();` at line 1692, but `install_project_tool_rules` takes only `(loader, project_types)` — no path argument — so it is not scoped to `repo`. It reaches `install_tool_commands` (`crates/swissarmyhammer-validators/src/review/tool_install.rs:282`), which executes the rule's install commands through `run_shell(command, None, &[])` (`tool_install.rs:295`). For `python` those commands are `uv tool install vulture==2.14` then `pipx install vulture==2.14` (`builtin/validators/code-hygiene/rules/dead-code-python.md:25`), which install into `~/.local/bin`. The same call at line 1959 with `&["rust"]` runs `cargo install cargo-machete@0.9.2 --locked` (`builtin/validators/manifests/rules/unused-dependencies-rust.md:47`), which installs into `~/.cargo/bin`. A test that installs software into the developer's HOME writes outside its sandbox. The same cause already sits at line 1771 in `verify_shipped_tool_rules_pass_fixtures`, reached by the tests at lines 1814, 1831, 1846, 1863 and 1880 — remove the cause from the whole file, not only from the two new sites. Make tool presence a precondition the test asserts, naming the missing tool and the command that installs it, or scope the install destination to the test's own temp dir. Do not install software into HOME from a test.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1701` — The added `install_project_tool_rules` call carries no lock, so two test processes can drive the same installer at the same destination at the same time. Under `cargo nextest` each test is its own process and tests run in parallel. There is no `Mutex`, `OnceLock`, file lock, or `serial_test` attribute on `install_project_tool_rules`, `install_tool_commands`, or `run_shell` in `crates/swissarmyhammer-validators/src/review/tool_install.rs` or `crates/swissarmyhammer-validators/src/review/doctor.rs`; the only `serial_test` uses are `doctor.rs:754` and `doctor.rs:798`, on unrelated env-var tests. This commit adds two more concurrent callers (lines 1701 and 1959) to the five that already reach line 1771, so on a machine that lacks the tool several processes can sit inside `uv tool install ruff==0.14.5` or `pipx install ...` writing the same `~/.local/bin/ruff`. `cargo install` is serialised by cargo's own `~/.cargo/.package-cache` lock; `uv`, `pipx`, `npm install -g` and `go install` get no such guarantee from this code. Serialise the install with a lock on the destination, or replace the install with an asserted precondition.

### Note on resolving these four together

Findings 1 and 2 ask for `install_project_tool_rules` at two more sites; findings 3 and 4 name a defect in that call. They share one subject, and one decision settles all four: choose how a test guarantees its tool, then apply that one way to every tool-rule test in the file. If the decision is "assert the precondition", findings 1 and 2 are satisfied by the assertion, not by a new install call.

### Directed judgment this pass — idempotence is clear

`install_project_tool_rules` is idempotent. `install_tool_commands` short-circuits at `crates/swissarmyhammer-validators/src/review/tool_install.rs:271` with `return ToolInstallOutcome::AlreadyPresent;` when `check_presence` reports `Present`, and `check_presence` (`doctor.rs:305`) runs the rule's `doctor.check_command`, which for these rules is a bare `which` (`builtin/validators/manifests/rules/unused-dependencies-rust.md:44`, `builtin/validators/code-hygiene/rules/dead-code-python.md:21`). The cost when the tool is present is one `bash -c which ...` per rule and nothing else, and `tool_install.rs:512` (`a_present_tool_runs_no_install_command`) holds that behaviour. It also re-checks after each command (`tool_install.rs:283`) and stops at the first command that satisfies the check, so the `pipx` fallback never runs once `uv` succeeded. No process-wide env or PATH mutation: there is no `set_var` on this path, and `doctor.rs:705` sets `SAH_BINARY_ENV` on the child `Command` only.

## Review Findings (2026-08-09 09:13)

Scope: `4ac042a8b..62a73bae5` — the checkpoint commit `62a73bae5` only. The engine fleet reported zero findings (9 pairs attempted, 0 failed, 0 skipped). Every finding below comes from the directed judgment on the cost of the decision. The decision itself is applied whole: no `install_project_tool_rules` call remains in `crates/swissarmyhammer-validators/src/review/tool_rules.rs`, and all four affected tests are plain `#[test]`, not `#[ignore]`, so `cargo nextest run` runs them.

- [x] `.github/workflows/ci.yml:40` — The Test job runs `cargo nextest run --no-fail-fast` and installs no Python tooling, so `vulture` is not on the runner unless something outside CI put it there. `the_shipped_python_dead_code_tool_rule_reports_and_suppresses_dead_code` now calls `require_tool_installed` at `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1759`, and that helper panics when `check_presence` reports `Missing`. The rule checks `which vulture sed` (`builtin/validators/code-hygiene/rules/dead-code-python.md:21`) and installs with `uv tool install vulture==2.14` or `pipx install vulture==2.14` (`:25`, `:26`); CI runs neither, and it installs neither `uv` nor `pipx` either. Before this commit the test installed the tool itself, so the run was self-healing; the commit removed the only mechanism that put `vulture` on the machine and added nothing in its place. The Test job steps are `actions/checkout`, `dtolnay/rust-toolchain@stable`, `rustup component add rust-analyzer`, `Swatinem/rust-cache`, `taiki-e/install-action@nextest`, and a Chromium check (`.github/workflows/ci.yml:20`–`:40`). No step in `.github/`, `scripts/`, `justfile` or `Makefile` installs it, and `.config/nextest.toml` declares no setup script. Add the install to the Test job.
- [x] `.github/workflows/ci.yml:40` — Same cause for `cargo-machete`. `the_shipped_rust_unused_dependency_tool_rule_reports_an_unused_dependency` calls `require_tool_installed` at `crates/swissarmyhammer-validators/src/review/tool_rules.rs:2034`. The rule checks `which cargo cargo-machete find grep awk mktemp head cut sort tr` (`builtin/validators/manifests/rules/unused-dependencies-rust.md:44`) and installs with `cargo install cargo-machete@0.9.2 --locked` (`:47`). CI installs `tauri-cli` in the `apps` job (`.github/workflows/ci.yml:95`) and `cargo-machete` nowhere. Add the install to the Test job.
- [x] `.github/workflows/ci.yml:27` — Same cause for the `clippy` component. `missing-docs-rust` and `complexity-rust` are both required preconditions now — `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1546` and `:1659` — and both check `which cargo-clippy jq` (`builtin/validators/code-hygiene/rules/missing-docs-rust.md:20`, `builtin/validators/code-hygiene/rules/complexity-rust.md:32`). The Test job asks `dtolnay/rust-toolchain@stable` for no components and adds only `rust-analyzer` (`.github/workflows/ci.yml:27`, `:29`). The repo treats clippy as a component a job must request: the Clippy job states `components: clippy` (`.github/workflows/ci.yml:136`) and the Rustfmt job states `components: rustfmt` (`.github/workflows/ci.yml:122`). Neither of those jobs can seed the Test job in any case — both declare `needs: test`, so they run after it. State the component in the job that needs it.
- [x] `.github/workflows/ci.yml:40` — Same cause for `jq`. Both clippy rules check `which cargo-clippy jq`, so `jq` is a hard precondition of the tests at `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1546` and `:1659`. No shipped rule installs `jq` and no CI step installs it — every `jq` occurrence in `.github/workflows/` is a use of the binary, never an install. Add the install, or state it as a documented runner prerequisite.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1521` — The panic prints the remedy for the rule's headline tool, but the precondition it reports came from `check_presence` over the whole `doctor.check_command`, which is all-or-nothing on the shell line (`crates/swissarmyhammer-validators/src/doctor.rs:310`–`:317`: a non-zero status is `ToolPresence::Missing`). For `missing-docs-rust` and `complexity-rust` the check is `which cargo-clippy jq`, so on a machine that has clippy and lacks `jq` the message reads "Install the tool and run the test again: rustup component add clippy" (`builtin/validators/code-hygiene/rules/missing-docs-rust.md:22`, `builtin/validators/code-hygiene/rules/complexity-rust.md:34`) — a command that is already satisfied and that cannot fix the failure it is printed for. Name the binary that actually failed the check in the message, or state a `jq` remedy on the rules that need it.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1479` — `remedy_label` joins `install.commands` and `doctor.fix_hint` into one string, and both call sites present that string as a command to run: the panic at `:1521` says "Install the tool and run the test again: {}", and `absent_rule_label` at `:1872` builds the roster failure list from it. `fix_hint` is not always a command. `builtin/validators/code-hygiene/rules/dead-code-swift.md:39` states `brew install peripheryapp/periphery/periphery, and run the review from the directory holding Package.swift`, which fails if pasted into a shell. `builtin/validators/code-hygiene/rules/no-commented-code-parsed.md:36` and `builtin/validators/duplication/rules/duplication-parsed.md:50` state `put the running sah binary on PATH, or set SAH_BIN to its path`, which is prose. Three more state a macOS-only remedy with no alternative, because they carry no `install` commands at all: `builtin/validators/code-hygiene/rules/complexity-swift.md:26`, `missing-docs-swift.md:21` and `magic-numbers-swift.md:22` all state `brew install swiftlint`, and `swiftlint` has no Linux formula. Separate the runnable commands from the advisory hint in the printed message so the text after "run the test again:" is always runnable.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1848` — The split between this helper and the four single-rule tests is incoherent, and this tolerance is the wrong half. The file now states two contradictory contracts for the same rules: `dead-code-python` is tolerated here and required at `:1759`, and `unused-dependencies-rust` is tolerated here and required at `:2034`. The tolerance is dead for one roster outright — `SHIPPED_UNUSED_DEPENDENCY_RULES` is a single row (`crates/swissarmyhammer-validators/src/review/tool_rules.rs:1396`–`:1397`), so `exercised > 0` at `:1861` is exactly "cargo-machete is present", the same requirement the single-rule test states, expressed differently. The degradation contract the doc comment cites at `:1815`–`:1821` is already held by two tests that need no tool at all: `plan_reports_a_fallback_when_the_tool_is_missing_and_suppresses_nothing` (`:891`) and `a_missing_tool_whose_installs_all_fail_stays_on_the_prompt_fallback` (`:1047`). What the tolerance buys instead is machine-dependent assertion strength that a green run never reports: `exercised` may be 1 of 6, and `absent` is read only in the failure message at `:1866`, so a broken Go, Swift, Dart or TypeScript rule stays green on a Mac-only CI fleet. Apply the decision this card took to every row — call `require_tool_installed` per row and delete the `exercised`/`absent` accounting at `:1828`, `:1829`, `:1855`, `:1857` and `:1861`–`:1867`.

### Correction to this pass's own earlier judgment

An earlier comment this pass recorded `remedy_label` and the roster split as clear. Both judgments were wrong and are withdrawn. `remedy_label` was judged on the four rules an asserting test names, which hid the `jq` case, because the remedy is read per rule while `check_presence` fails per binary. The split was judged coherent on the strength of the `exercised > 0` floor alone, without checking that one roster holds a single row, that the degradation contract has its own tool-free tests, or that this card's own decision is stated universally.

## Review Findings (2026-08-09 10:02)

Scope: `68c12d51c..615593ecb`. Engine fleet: 17 pairs attempted, 0 failed, 1 file skipped. Findings 1–4 are the engine's. Findings 5–8 come from the directed judgment on the install lock and the roster-test working directory.

> ⚠️ 1 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/swissarmyhammer-validators/src/review/tool_rules.rs` — 262265 rendered bytes, over the 262144-byte per-file cap; not reviewed by: duplication (split the file)

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1` — This file exceeds the review prompt cap — 262265 rendered bytes against the 262144-byte per-file cap — so these validators could not review it: duplication. Split the file into smaller modules that fit the review prompt cap.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:368` — MatchedToolRule is a pub(crate) struct with non-empty representation (4 fields including Vec<String>) but does not derive Debug. All public types with non-empty representation need Debug for debuggability by callers. Add #[derive(Debug)] to MatchedToolRule struct definition at line 368.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:440` — ProjectToolRule is a pub(crate) struct with non-empty representation (3 fields) but does not derive Debug. All public types with non-empty representation need Debug for debuggability by callers. Add #[derive(Debug)] to ProjectToolRule struct definition at line 440.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:626` — ScriptFailure wraps std::io::Error in the Start variant but does not implement std::error::Error, breaking the error chain. Library code must preserve error sources via Error::source() so callers can inspect the full chain. Derive ScriptFailure from thiserror::Error: #[derive(Debug, thiserror::Error)] with #[error(...)] attributes on each variant, and #[from] on the Start variant's std::io::Error field. This automatically implements Error with proper source() support.
- [x] `crates/swissarmyhammer-validators/src/review/tool_install.rs:407` — The install agent turn runs outside `InstallLock`. `install_tool_commands` takes the lock at `crates/swissarmyhammer-validators/src/review/tool_install.rs:344` and drops the guard when the function returns at `:367`; `ensure_tool_installed` then calls `agent.install(&request).await` at `:407` and re-checks presence at `:420`, both with no lock held. The module doc states `Installs are serialized machine-wide by [InstallLock]` (`:33`), and that is not true of this half. It is the half the doc's own rationale applies to most: the deterministic half runs commands a rule declared and pinned, while the agent half runs whatever the agent chose, and `:34`–`:35` gives the reason for the lock as `only cargo install holds a lock of its own`. Hold the lock across the agent turn, or state in the doc that the agent half is unserialized.
- [x] `crates/swissarmyhammer-validators/src/review/tool_install.rs:295` — The lock scope is not the scope the doc states, and it is not the scope of the destinations. `std::env::temp_dir()` returns `$TMPDIR` when it is set; on this machine that is `/var/folders/3k/6b488x1j6rg1f6w8fnyfwm9w0000gn/T/`, a per-user directory, so the lock is per-`TMPDIR` and a process launched with a different `TMPDIR` opens a different lock file and serializes against nothing. The doc calls the lock machine-wide at `:33` and `:275`–`:276`, and calls it `an exclusive lock over every tool install destination`. Three rules the roster now requires state `brew install swiftlint` as their only remedy — `builtin/validators/code-hygiene/rules/missing-docs-swift.md:21`, `magic-numbers-swift.md:22` and `complexity-swift.md:26` — and Homebrew writes `/opt/homebrew`, which every user of the machine shares, so a per-user lock does not cover it. State the scope the code has, or put the lock on a path every installing process shares.
- [x] `crates/swissarmyhammer-validators/src/review/tool_install.rs:308` — `file.lock_exclusive()` waits with no deadline, and the wait is not reentrant. `acquire()` opens the lock file fresh on every call (`:296`–`:299`), and `flock(2)` conflicts between two open file descriptions even inside one process, so a process that reaches `install_tool_commands` while it already holds the lock — directly, or through a child process an install command spawns — blocks forever. Nothing bounds the wait and nothing reports it: there is no `try_lock`, no deadline, and no log line before the wait, because `acquire()` traces only on failure (`:301`, `:310`). The re-entry path is reachable rather than hypothetical: the doc at `:337`–`:338` states `sah init` pre-installs runner tools through this same function, and the agent turn at `:407` runs commands no rule declared. Bound the wait and name the lock when it is contended, so a stalled run reports the reason instead of hanging silently.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:2044` — `swift_package_root` returns `(tempfile::TempDir, CurrentDirGuard)` (`crates/swissarmyhammer-validators/src/review/tool_rules.rs:1997`) and the caller binds the pair to one name, so the two drop in tuple order: `TempDir` first, `CurrentDirGuard` second. `TempDir::drop` removes the directory while it is still the process working directory, and the guard restores the working directory only afterwards, so between the two drops the process working directory is a deleted path. That is the inverse of the order the type documents — its own example holds the guard in an inner scope so the guard drops before the `TempDir` (`crates/swissarmyhammer-common/src/test_utils.rs:161`–`:165`) — and `CurrentDirGuard::new` carries two explicit checks against this same class of hazard, at `:192`–`:194` and `:64`–`:71` of that impl. Return the guard first, or bind the two so the guard drops before the directory is removed.

### Directed judgment A — the install lock: three findings above, and what is clear

Clear, and verified rather than assumed:

- **The lock file location survives a read-only or unusual HOME.** `std::env::temp_dir()` reads `$TMPDIR` and falls back to `/tmp`; neither derives from `HOME`, so a machine whose HOME is read-only still gets a lock. `acquire()` degrades rather than fails when the location is not writable — it returns `None` after a `tracing::warn!` at `crates/swissarmyhammer-validators/src/review/tool_install.rs:300`–`:306` and `:309`–`:315`, and the caller installs unserialized, which the doc states at `:290`–`:293`. The scope of that location is a separate matter, recorded as a finding above.
- **The double-check closes the race.** `install_tool_commands` checks presence at `:340`, acquires at `:344`, checks again at `:346`–`:349`, and every install command at `:358`–`:365` runs after that second check. The binding is `let _lock`, not `let _`, so the guard lives to the end of the function rather than dropping at the end of the statement — the difference that would have made the whole lock a no-op. No window remains between the second check and the last command.
- **The lock releases on panic and on kill.** `_lock` is a local, so an unwinding panic runs `Drop for InstallLock` at `:321`–`:327`, which calls `FileExt::unlock`. On `panic = "abort"` or `SIGKILL` the guard never runs, but the kernel closes the file descriptor at process exit and `flock(2)` releases with it. No stale lock survives either path.
- **Threads inside one process contend correctly.** `acquire()` opens the file fresh each call, so two threads hold two open file descriptions and `flock(2)` conflicts between them. `install_missing_tools` drives its rules in a sequential `for` loop with `.await` (`:475`–`:487`), so it never has two installs in flight in one process in any case.

### Directed judgment B — the two shipped-rule defects: clear

- **The Go `GOBIN` change satisfies each rule's own doctor check.** Each of the five rules now runs `mkdir -p "$HOME/.local/bin" && GOBIN="$HOME/.local/bin" go install …`, and `$HOME/.local/bin` is the directory `uv` and `pipx` already write, so one PATH entry serves all three installers. A bare `go install` writes `$(go env GOPATH)/bin` — `/Users/wballard/go/bin` on this machine, with `go env GOBIN` empty — which no default PATH holds, so the change is what makes `which` find the binary. All five checks pass here against binaries `go install` placed: `which gocognit go jq`, `which golangci-lint go jq`, `which revive jq`, `which staticcheck go jq` all exit 0, and `golangci-lint --version` reports `2.12.2`, the pinned version, running from a `go install` build. CI puts the same directory on PATH at `.github/workflows/ci.yml:69`–`:74`.
- **`CurrentDirGuard` genuinely isolates, and no test in the same binary runs concurrently with it.** The guard holds a `MutexGuard<'static, ()>` on the process-global `CURRENT_DIR_LOCK` for its whole life (`crates/swissarmyhammer-common/src/test_utils.rs:170`–`:178`), so no second `CurrentDirGuard` in the same process can chdir while it is held — the isolation does not rest on nextest's process-per-test model, it rests on the mutex. Under nextest each test is its own process, so nothing else runs in the process at all. The five roster tests ran concurrently as five processes and all passed. The drop order inside `swift_package_root` is a separate matter, recorded as a finding above.

Evidence: `cargo nextest run -p swissarmyhammer-validators` over the 5 roster tests, the 4 single-rule acceptance tests and the 4 new precondition tests — 17 run, 17 passed, 0 failed.

### Directed judgment C — what CI must carry: clear, no required rule CI cannot satisfy

The five rosters require 24 rows across six languages. Every row's `doctor.check_command` resolves on the self-hosted Mac runner, through a declared install command or a CI step:

- `cargo-clippy` — `missing-docs-rust`, `complexity-rust`. No install command; `.github/workflows/ci.yml:27`–`:33` states `components: clippy`.
- `jq` — 13 rows. No rule installs it; `.github/workflows/ci.yml:45`–`:50` brews it.
- `ruff`, `vulture` — 5 rows. `.github/workflows/ci.yml:51`–`:63` brews `uv`, installs `vulture==2.14` and puts `uv tool dir --bin` on `$GITHUB_PATH`; the `ruff` rows then install themselves through their own `uv tool install ruff==0.14.5` onto that same PATH.
- `eslint`, `ts-prune` — 5 rows, installed by the rules' own `npm install -g`. `actions/setup-node@v4` at `.github/workflows/ci.yml:75`–`:79` supplies `npm`. All six pinned versions resolve on the registry: `eslint@10.8.0`, `typescript-eslint@8.66.0`, `typescript@5.9.3`, `eslint-plugin-jsdoc@63.3.3`, `eslint-plugin-sonarjs@4.2.0`, `ts-prune@0.10.3`.
- `gocognit`, `golangci-lint`, `revive`, `staticcheck` — 5 rows, installed by the rules' own `GOBIN` commands. `.github/workflows/ci.yml:80`–`:88` brews `go`, and `:69`–`:74` puts `$HOME/.local/bin` on `$GITHUB_PATH`.
- `swiftlint` — `missing-docs-swift`, `magic-numbers-swift`, `complexity-swift`. No install command; `.github/workflows/ci.yml:89`–`:97` brews it. The formula exists in homebrew-core.
- `periphery`, `swift`, `Package.swift` — `dead-code-swift`. No install command; `.github/workflows/ci.yml:98`–`:100` brews it. `brew install peripheryapp/periphery/periphery` auto-taps and the tap repository still resolves (`https://github.com/peripheryapp/homebrew-periphery`, HTTP 200); the formula is also in homebrew-core now. `swift` comes from the runner's Xcode and `:88` asserts it with `swift --version`. The `test -f Package.swift` half of that check is a working-directory precondition no install can satisfy, and `swift_package_root` supplies it from the shipped fixture template `builtin/validators/code-hygiene/fixtures/Package.swift.tmpl`.
- `dart` — `missing-docs-dart`, `dead-code-dart`. No install command; `.github/workflows/ci.yml:101`–`:103` brews `dart-sdk`, which exists in homebrew-core at 3.12.2.
- `cargo-machete` — `unused-dependencies-rust`. `.github/workflows/ci.yml:64`–`:68` runs the rule's own pinned command.
- `cargo`, `find`, `grep`, `sed`, `awk`, `sort`, `tr`, `mktemp`, `head`, `cut` — the shell utilities `dead-code-rust` and `unused-dependencies-rust` name beside their tool. Present on any macOS runner.

No rule is required with neither an install command nor a CI step. `cargo nextest run --no-fail-fast` at `.github/workflows/ci.yml:104`–`:105` is the only test invocation in `.github/workflows/`, so the Test job is the only job that has to carry this set.

### Verified — the three prior open findings are addressed in this range

Recorded as evidence for whoever flips the marks; the marks are not this pass's to flip.

- `tool_rules.rs:1521` (the remedy named the wrong binary): `checked_binaries` and `missing_label` now read the binary names back out of the check command and report only the ones this machine lacks, so `which cargo-clippy jq` failing on `jq` names `jq`. Held by `the_precondition_failure_names_the_binary_that_actually_failed`, and a check that fails on something that is not a binary falls back to naming the command, held by `a_check_that_fails_on_no_binary_is_named_by_its_command`.
- `tool_rules.rs:1479` (`remedy_label` presented prose as a command): `remedy_label` is gone. `precondition_report` puts the install commands on a `run: ` line and the `doctor.fix_hint` on an `advice for a person, not a command to run: ` line. Held by `the_precondition_failure_keeps_the_hint_out_of_the_runnable_line` and `a_rule_with_no_install_command_offers_no_runnable_line`.
- `tool_rules.rs:1848` (the roster tolerated absent tools): the `exercised`/`absent` accounting and `absent_rule_label` are deleted. `verify_shipped_tool_rules_pass_fixtures` calls `require_tool_installed` per row at `:2048` and asserts the fixture contract for every row unconditionally.

### Where the 2026-08-09 10:02 findings landed

- Prompt cap. `tool_rules.rs` keeps its production code and now states `#[cfg(test)] mod tests;`. The test module moved to `tool_rules/tests.rs` (shared imports, the shipped rosters, `code_hygiene_work`, `verify_run_reports_one_finding`) with four subject modules under `tool_rules/tests/`: `plan.rs`, `execute.rs`, `preconditions.rs` and `shipped.rs`. The public surface did not change. Largest file of the family: `tool_rules.rs`, 740 lines / 26671 bytes / 42951 gutter-rendered bytes, against the 262144-byte cap.
- `Debug`. `MatchedToolRule` and `ProjectToolRule` derive it. The sweep over both files found one more type with no `Debug` — `InstallLock` in `tool_install.rs` — and one in test code, `ScriptedInstallAgent`. Both derive it now.
- Error chain. `ScriptFailure` is `#[derive(Debug, thiserror::Error)]` with `#[from]` on `Start`, so `source()` reaches the `std::io::Error`. It is the only error type in either file that broke the chain: `ToolRunError` already carried `thiserror` over a `String`, and `ToolFallback` is a report fact with `Display` alone, which its doc states.
- Install lock. `ensure_tool_installed` takes ONE `InstallLock` and holds it across the declared commands and the agent turn; `run_declared_install_commands` is the half that runs under a lock the caller holds. `InstallLock::take` waits with a deadline, logs the lock path when it finds the lock contended, and installs unserialized when the deadline passes. The doc states the scope the code has — every process that resolves the same temporary directory — and names Homebrew as the destination that scope does not cover.
- Drop order. `swift_package_root` returns `(CurrentDirGuard, tempfile::TempDir)`, so the guard restores the working directory before the directory is removed.

## Review Findings (2026-08-09 10:55)

Scope: `615593ecb..087d052b4`. Engine fleet: 18 pairs attempted, 0 failed, **0 files skipped**.

The engine reported 6 findings. Five are dropped under the skill's blanket exception on refactoring test code that already existed: each asks to deduplicate a helper or a constant that is present in `tool_rules.rs` at `615593ecb` and that this commit only relocated — `precondition_report`, `COMPLEX_PACKAGE_MANIFEST`, `complexity_work`, `UNUSED_DEPENDENCY_LIB_PATH` and `manifests_work` all occur at that sha. Finding 1 is the engine's and is kept: its subject is a defect, not a restyle. Findings 2–4 come from the directed judgment on the install lock and the reach the doc states.

- [ ] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/preconditions.rs:49` — Command injection: the `binary` parameter is interpolated directly into a shell command string without sanitization. If a binary name extracted from a check command contains shell metacharacters (e.g., `$(command)`, `;`, `&`), they will be executed when the command is passed to `run_shell`. Use a shell-safe escaping function (e.g., `shell_escape` or similar) to escape the `binary` parameter before interpolating it into the command string. Alternatively, refactor `run_shell` to accept arguments as a separate list parameter (like `subprocess.run` with `shell=False`) rather than embedding them in the command string.
- [ ] `crates/swissarmyhammer-validators/src/review/tool_install.rs:77` — The 300 s bound is justified against a smaller critical section than the one the lock holds. The doc at `:77`–`:79` states `The bound is longer than the slowest install a shipped rule declares: cargo install cargo-machete@0.9.2 --locked builds the tool from source`, and `INSTALL_LOCK_WAIT` is `Duration::from_secs(300)` at `:80`. But `ensure_tool_installed` takes one lock at `:498` and holds it across both halves — `run_declared_install_commands`, and `agent.install(&request).await` at `:510` — so the holder's critical section is the declared commands PLUS a full agent turn, and the agent turn runs commands no rule declares. The pool bounds that turn at `PROMPT_TURN_CEILING`, `Duration::from_secs(45 * 60)` (`crates/swissarmyhammer-validators/src/validators/pool.rs:88`) — 2700 s, nine times the whole wait — and abandons a turn that makes no streaming progress only after `PROMPT_IDLE_TIMEOUT`, `Duration::from_secs(300)` (`pool.rs:68`), which alone equals the waiter's entire deadline. A waiter therefore reaches its deadline while the holder is still inside a turn the pool considers healthy, which makes the timeout the ordinary outcome rather than the exceptional one whenever the agent half is reached. Bound the wait against the critical section it must outlast, or state the bound the code has.
- [ ] `crates/swissarmyhammer-validators/src/review/tool_install.rs:354` — `InstallLock::take` collapses two different situations into one `None`, and the caller cannot tell them apart. `acquire` returns `None` when the lock file cannot be opened (`:342`), where no holder exists and installing unserialized costs nothing; `take` returns `None` when the deadline passed with a holder throughout (`:379`), where a holder is demonstrably still installing and installing unserialized is the concurrent write the lock exists to prevent. The doc at `:328`–`:331` gives one rationale for both — `An install with no lock is worse than an install with one, and better than no install at all, so the caller goes ahead either way`. The presence re-check at the top of `run_declared_install_commands` covers only the holder that FINISHED, and its own comment says so — `Another installer may have finished while this one waited for the lock` — not the holder a timeout evidences, which is one still running. Reporting failure is available and is already held by tests rather than being a new degradation: `plan_reports_a_fallback_when_the_tool_is_missing_and_suppresses_nothing` (`crates/swissarmyhammer-validators/src/review/tool_rules/tests/plan.rs:154`) and `a_missing_tool_whose_installs_all_fail_stays_on_the_prompt_fallback` (`plan.rs:310`) hold the prompt fallback, so a deadline that reports failure degrades onto a documented, tested path instead of racing the destination. Separate the two branches: install unserialized when no holder exists, and report failure when the deadline passed on a live holder.
- [ ] `crates/swissarmyhammer-validators/src/review/tool_install.rs:315` — The doc states a destination belongs to one user that does not. `:313`–`:316` states `That reach fits the destinations a rule's install commands actually write. ~/.local/bin, ~/.cargo/bin and the npm and go bin directories all belong to one user. Homebrew is the exception`, and `:65`–`:66` repeats the assumption as `npm install -g and go install write their own bin directories`. Under a Homebrew node the npm global prefix IS the Homebrew prefix: `/opt/homebrew/lib/node_modules/npm/npmrc` states `prefix = /opt/homebrew`, `/opt/homebrew/bin/npm` links to `/opt/homebrew/Cellar/node/26.5.0/bin/npm`, and `/opt/homebrew/bin/npm config get prefix` reports `/opt/homebrew` — a prefix every user of the machine shares. Homebrew's own lock does not cover it either: `FormulaLock` is keyed on a Cellar rack name and is taken by `FormulaInstaller` (`Library/Homebrew/formula_installer.rb:550`), and `npm install -g` is not a brew operation, so it takes no brew lock. Four shipped rules declare `npm install -g` — `builtin/validators/code-hygiene/rules/complexity-typescript.md:51`, `dead-code-typescript.md:33`, `magic-numbers-typescript.md:51` and `missing-docs-typescript.md:58` — so their destination falls outside the per-`$TMPDIR` lock across users and outside Homebrew's lock as well. Name npm beside Homebrew as a destination the lock's reach does not cover, or put the lock on a path every installing process shares.

### Directed judgment A — the split: clean, and it solved what it was for

- **Zero prompt-cap skips.** The engine reports `skipped: 0` and `skipped_files: []` over 18 attempted pairs, 0 failed. The previous pass over the same code reported 1 skipped file. Per file of the family, the skip count is: `tool_rules.rs` 0, `tool_rules/tests.rs` 0, `tool_rules/tests/plan.rs` 0, `tool_rules/tests/execute.rs` 0, `tool_rules/tests/preconditions.rs` 0, `tool_rules/tests/shipped.rs` 0.
- **The mass is not merely moved.** `tool_rules.rs` was 2271 lines / 91180 raw bytes at `615593ecb`, which rendered to the 262265 bytes that broke the 262144-byte cap. It is now 740 lines / 26671 raw bytes. The rest of the family: `tests.rs` 270 lines / 10890 bytes, `tests/shipped.rs` 605 / 24868, `tests/plan.rs` 345 / 12209, `tests/preconditions.rs` 237 / 9069, `tests/execute.rs` 154 / 5372. The largest file of the family is now under a third of the raw size of the file that was skipped, and no file is within a factor of six of the cap.
- **Tests only, and no production behaviour changed by the split.** `tool_rules.rs` has exactly one deletion hunk, `@@ -726,1546 +737,4 @@`, and it begins on the `mod tests {` line — the whole 1546-line deletion is the test module and nothing above it. Every other change to that file is a fix for a finding of the previous pass: `#[derive(Debug)]` on `MatchedToolRule` and on `ProjectToolRule`, and `ScriptFailure` becoming `#[derive(Debug, thiserror::Error)]` with `#[from]` on `Start` plus the matching arm in `run_tool_script`. No other production line changed.
- **The 1577 deletions all reappear.** 1549 of them are the test module leaving `tool_rules.rs`, 5 are this card's own kanban file and 23 are `tool_install.rs`; the five new files add 1611 lines. Tests before: 30 `#[test]`/`#[tokio::test]` at `615593ecb`. Tests after: 31 across the family (`tests/shipped.rs` 11, `tests/plan.rs` 8, `tests/execute.rs` 8, `tests/preconditions.rs` 4, `tool_rules.rs` 0, `tests.rs` 0). Diffing the two sorted name lists shows zero removed and exactly one added — `the_swift_package_root_restores_the_directory_before_it_removes_it`, the regression test for the drop-order fix. Function names across the family: 83 before, 83 after, same membership; the one apparent miss, `require_tool_installed`, is a `pub(super) fn` at `tool_rules/tests/preconditions.rs:129` with seven call sites.

### Directed judgment C — the Homebrew claim: verified true

The doc's exception is correct, checked in Homebrew's own source rather than accepted. `FormulaInstaller#install` calls `lock` at `Library/Homebrew/formula_installer.rb:550` and `unlock` at `:1077`. `FormulaInstaller#lock` (`:1913`–`:1924`) locks the formula and every dependency through `self.class.locked.each(&:lock)`. `Formula#lock` (`Library/Homebrew/formula.rb:1897`–`:1898`) builds a `FormulaLock` and locks it. `FormulaLock < LockFile` is keyed on the Cellar rack name (`Library/Homebrew/lock_file/formula_lock.rb:6`–`:10`). `LockFile#lock` is `flock(File::LOCK_EX | File::LOCK_NB)` (`Library/Homebrew/lock_file.rb:44`), released with `LOCK_UN` at `:79`, and the lock files live in `/opt/homebrew/var/homebrew/locks`, a path every user of the machine shares. So Homebrew does serialize two concurrent installs of one formula machine-wide, and naming it as the exception the per-`$TMPDIR` lock need not cover is right.

The other half of the check — that no destination a shipped rule writes falls outside both locks — does not hold. npm is the destination that falls outside both, recorded as a finding above. The remaining declared destinations are per-user and are covered: `uv tool install` and `pipx install` write `~/.local/bin` (`uv tool dir --bin` reports `/Users/wballard/.local/bin`), `cargo install cargo-machete@0.9.2 --locked` writes `~/.cargo/bin`, and all five `go install` commands set `GOBIN="$HOME/.local/bin"` explicitly rather than taking the `GOPATH` default. `brew install` appears in no rule's `install.commands` at all — it appears only as a `doctor.fix_hint` on the Swift and Dart rules, which is prose the agent half may act on.

### The deadline test coverage this pass confirmed

`InstallLock::take` takes its deadline as a parameter, and two tests drive it: `a_contended_install_lock_gives_up_instead_of_waiting_for_ever` (`crates/swissarmyhammer-validators/src/review/tool_install.rs:807`) holds the lock and asserts the second take gives up, and `the_bounded_wait_takes_the_lock_the_holder_releases` (`:831`) asserts a waiter takes the lock a holder releases inside the wait. The bounded-wait mechanism itself is held by tests; the two findings above are about the size of the bound and about what the code does when the bound passes.