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
depends_on:
- 01KZFHBM7MWFYAMK3SQADF0D7H
position_column: doing
position_ordinal: '8480'
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