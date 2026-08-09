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