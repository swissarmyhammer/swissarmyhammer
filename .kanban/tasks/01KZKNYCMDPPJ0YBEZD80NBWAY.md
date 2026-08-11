---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzknzj35vrhd576kpqh7qjxv
  text: |-
    ### The owner said this three ways — record all three

    - "we're not looking for repeated code blocks, we want to find methods, functions, and types that are nearly repeated"
    - "code blocks is looking at WAY too fine grained"
    - "look for highly duplicate types, methods, functions"

    So the unit is a whole named definition and nothing smaller. A 50-token window is a few lines. At that size the detector matches fragments of logic, which is why 945 findings say almost nothing.

    ### A type normalizes differently from a function

    The card body states one normalization. That is right for a function body, and it is wrong for a type. Use two:

    **Function or method.** Normalize the body's token stream — the first distinct identifier becomes `v1`, the second `v2`, and each literal becomes a marker of its kind. Two functions that differ only by variable names or by one constant then hash the same.

    **Type.** The body is a field list, not a statement stream, so normalize the FIELD TYPES and drop the field names. Two structs whose fields have the same types in the same order are the same shape under two names, and that is the finding. Keep the field order significant at first — an order-free comparison reports more and is a separate decision, so measure it before choosing.

    Per language, the definitions to walk:
    - Rust — `fn`, `impl` methods, `struct`, `enum`, `trait`
    - TypeScript — function, class method, `interface`, `type` alias
    - Python — `def`, class method, `class`
    - Go — `func`, method, `type`
    - Swift — `func`, method, `struct`, `class`, `enum`, `protocol`

    ### "Highly" duplicate needs a number, and it must be measured

    The owner said "highly duplicate", not "identical". Normalized hashing gives an exact yes or no, which answers "identical after renaming" but not "highly similar". If a degree is wanted, the deterministic way is to compare the two normalized token streams and report the ratio — for example, report a pair at or above 90 percent. That stays exact and needs no LLM and no embedding.

    Do not guess the number. Run it over this workspace at several thresholds, write the counts into the rule body, and pick from the measurement.

    ### What to expect from the count

    The current rule reports 945 over 1155 tracked `.rs` files. Most of that should disappear, because most of it is fragments. Whatever the new number is, record BOTH in the rule body so a later reader can see what the change bought.
  timestamp: 2026-08-09T16:34:01.957497+00:00
- actor: claude-code
  id: 01kzkpdptaxbtf0cehnprjg5zz
  text: |-
    ### Research done — picked up the card

    **What is there now.**
    - `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs` gives `DuplicationSource { language, tokens, exempt }`. `tokens` is every code leaf of the file. `exempt` is the byte ranges of test definitions and of marker comments. It already reads `is_definition_kind`, `definition_name`, `declaration_header` off the parse.
    - `crates/swissarmyhammer-code-context/src/ops/find_duplication.rs` hands those tokens to `cpd_core::detect::detect_prepared` at a 50-token window.
    - Only `find_duplication.rs` reads `duplication_source`, and only `execute.rs` reads `find_duplication`. The blast radius is two files.

    **Design decision — `cpd-core` goes.**
    `detect_prepared` is Rabin-Karp over a token stream. It answers "where is a run of N tokens spelled twice", which IS the sliding window this card removes. It cannot answer "how alike are these two whole definitions", because it never sees a definition. A ratio between two whole normalized streams needs a sequence comparison, so the detector becomes a longest-common-subsequence ratio over the two normalized streams. `cpd-core` then has no caller, so the dependency comes out of the root `Cargo.toml` and out of `crates/swissarmyhammer-code-context/Cargo.toml`.

    **Unit and the two normalizations.**
    - Callable (function, method): the body's token stream. The first distinct identifier becomes `v1`, the second `v2`. Each literal becomes a marker of its kind.
    - Record (type): the whole declaration's token stream with every `name` field dropped — the definition's own name and each member's name. Field TYPES stay verbatim, so two structs with the same field types in the same order are one shape.

    **Definition roster.** The comment names five languages. The rule's `match` covers the whole grammar roster and `supersedes` the prompt rules for all of it, so a language with no definition table would silently lose the rule. Every roster language gets a table.

    **Order of work.** sem extractor -> detector -> measure over the tree -> thresholds into the rule body -> fixtures -> CLI acceptance test -> drop `cpd-core`.
  timestamp: 2026-08-09T16:41:45.546551+00:00
- actor: claude-code
  id: 01kzkr93w1x337wqyes84gpmh2
  text: |-
    ### Implementation landed

    **Design decision — `cpd-core` is gone.** `detect_prepared` is Rabin-Karp over a token stream. It answers "where is a run of N tokens spelled twice", which IS the window this card removes, and it never sees a whole definition, so it cannot say how alike two definitions are. The detector is now a longest-common-subsequence ratio over the two normalized streams: `100 * 2 * shared / (left + right)`, exact integer arithmetic. `cpd-core` then had no caller, so it is out of the root `Cargo.toml`, out of `crates/swissarmyhammer-code-context/Cargo.toml` and out of `Cargo.lock` (`grep '^name = "cpd' Cargo.lock` returns 0).

    **The two normalizations.** A function or method normalizes its BODY: the first distinct identifier becomes `v1`, the second `v2`, each literal becomes a marker of its kind (`#num`, `#str`, `#char`, `#float`, `#bool`). A type normalizes its whole declaration with every `name` field dropped, so the member types stay and the member names go. Field order stays significant — the comment said an order-free comparison is a separate decision, so it was not taken.

    **Definition roster.** Every roster language, not only the five the comment names, because the rule's `match` covers the whole roster and `supersedes` the prompt rules for all of it. A language with no table would silently lose the rule. Rust, TypeScript/TSX/JavaScript, Python, Go, Swift, Java, C#, C, C++, Ruby, PHP, Fortran, Bash and Elixir each have a table, pinned by one test each. Elixir spells a definition as a call to `def`/`defp`/`defmacro`/`defmacrop`, so it reads the call target. The node kinds were read off real parses, not guessed.

    **Thresholds, measured.** The sweep ran the comparison over all 1183 tracked `.rs` files at a floor of 20 tokens and 80 percent, then counted at every gate:

    | minimum tokens | 100% | 95% | 90% | 85% | 80% |
    |---|---|---|---|---|---|
    | 20 | 588 | 759 | 1035 | 1452 | 1991 |
    | 30 | 365 | 451 | 585 | 806 | 1112 |
    | **40** | 258 | 327 | **416** | 544 | 721 |
    | 50 | 209 | 261 | 333 | 428 | 540 |
    | 60 | 173 | 219 | 280 | 359 | 452 |
    | 80 | 82 | 111 | 143 | 199 | 264 |
    | 100 | 52 | 72 | 95 | 136 | 185 |

    Chosen: **40 tokens, 90 percent**. Read by hand at the boundaries. Under 40: `has_errors`/`has_warnings` in `swissarmyhammer-doctor/src/runner.rs` (25 tokens, a one-line accessor pair), and `ModelInfo`, `PerspectiveInfo`, `AddTag` — small records whose field types coincide. At 40 and over: `extract_verb_noun` spelled identically in two crates (347 tokens), `build_clap_arg` against `dynamic_cli.rs`'s `new` (363 tokens, 98%), `builtin_yaml_sources` copied across three crates. Under 90: `ProjectSymbols`, fifteen `String` fields, matches another all-`String` record at 85 — a shape collision, not a copy.

    **Counts, old and new, both written into the rule body.** Old: 945 findings over 1155 tracked `.rs` files, median 67 tokens and 12 lines, 389 intra-file, 6.7 s. New: 416 findings over 1183 files, median 71 tokens, 159 intra-file, 258 exact after normalization, 10.2 s. The 416 are 395 functions, 13 structs, 8 enums or traits.

    **The structural exclusion re-measured.** 416 with the Rust test markers, 3490 without them. It removes 3074 findings, 88.1% of the raw total.

    **RED verified for real.** The old rule was rebuilt from HEAD into this tree (`git checkout HEAD --` the seven files, `cargo build`, run, restore) and run on the new fail fixture: it reports NOTHING. The new rule reports exactly `fail.rs:32: fn ``mirrored_band`` is a near-duplicate of ``folded_band`` at fail.rs:12 (61 tokens, 100% alike)`. The pass fixture reports nothing under both.

    **Found along the way, carded as `^xpf86ds`.** `attribute_names` compares the stripped attribute text for equality only, so `#[tokio::test(flavor = "multi_thread")]` is not read as a test marker. Eight of the 416 are `#[tokio::test(...)]` functions. Reading the argument form drops the count to 408, measured. The gap predates this card, so the fix is a card of its own; the rule body names the eight so the number is honest.

    **My own new code tripped the new rule.** `collect_definitions` and `collect_exemptions` were near-duplicate recursive walkers (62 tokens, 90%). They are now one `collect_under` over a `Descent` enum. Re-measured: neither new file reports at the gate.
  timestamp: 2026-08-09T17:14:12.225751+00:00
- actor: claude-code
  id: 01kzkr9eaj9bp9dkz1e6ndf2tx
  text: |-
    ### implement — changed
    - evidence: 11 files. `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs` (definitions + the two normalizations), `crates/swissarmyhammer-sem/src/parser/plugins/code/mod.rs`, `crates/swissarmyhammer-code-context/src/ops/find_duplication.rs` (the LCS detector), `crates/swissarmyhammer-code-context/src/lib.rs`, `crates/swissarmyhammer-code-context/Cargo.toml`, `Cargo.toml`, `Cargo.lock` (`cpd-core` removed), `builtin/validators/duplication/rules/duplication-parsed.md`, `builtin/validators/duplication/VALIDATOR.md`, both fixtures, `apps/swissarmyhammer-cli/tests/duplication_tool_rule.rs`, plus the two op descriptions in `swissarmyhammer-tools`.
    - thresholds: 40 normalized tokens, 90 percent alike, both chosen from a sweep over 1183 tracked `.rs` files.
    - counts: old 945 over 1155 files (median 67 tokens, 389 intra-file); new 416 over 1183 files (median 71 tokens, 159 intra-file), 10.2 s. Both in the rule body.
    - RED: the rule rebuilt from HEAD reports nothing on the new fail fixture; the new rule reports one finding.
    - gates: `cargo nextest run --workspace` 13986 passed / 0 failed / 0 skipped; `cargo test --doc` 6 passed; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - HEAD: 1ce5b5fd7
    - next: `/review`
  timestamp: 2026-08-09T17:14:22.930147+00:00
- actor: claude-code
  id: 01kzkw7qzdbnpshm6d42ekyx7q
  text: |
    ### review — findings
    - evidence: 2 findings — crates/swissarmyhammer-code-context/src/ops/find_duplication.rs:140, crates/swissarmyhammer-code-context/src/ops/find_duplication.rs:225. Scope `1ce5b5fd7..ce8593e17`. Engine counts: findings 1, confirmed 1, refuted 0, attempted 18, failed 0, skipped 0. The second finding comes from a measured check of the pair-count bound.
    - verified and accepted: the unit is a whole definition (12 samples, both ends on a declaration line); a fragment that spans two definitions is not reported (probe plus control); the gates 40 and 90 match the constants; the sweep reproduces exactly (1183 files, 416 findings, 10.4 s, 395 fn / 13 struct / 8 enum, 159 intra-file, median 71 tokens, 258 at 100 percent); a pair that differs only by a literal scores 100 percent, so the 90 gate does not miss the case the rule is for; the ratio denominator is the sum of both streams, so a contained short definition is not reported; `cpd-core` is gone from the root manifest, the member manifest and the lock, with no orphan code.
    - next: put a limit on the pair work in `best_partners`, and make `similarity_percent` generic over `AsRef<str>`.
  timestamp: 2026-08-09T18:23:21.581376+00:00
- actor: claude-code
  id: 01kzkw9m99g28zg4g2kg407p6s
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 14 source files plus two new cards. `cpd-core` removed; definitions compared by a longest-common-subsequence ratio
    - test: green — cargo nextest run --workspace 13986 passed / 0 failed / 0 skipped, doctests 6 passed, fmt clean, clippy -D warnings clean
    - commit: ce8593e17, parent 1ce5b5fd7 — 18 files, 1885 insertions, 605 deletions
    - review: findings — 2 open, on `review sha 1ce5b5fd7..ce8593e17`. Engine: 18 attempted, 1 confirmed, 0 refuted, 0 failed, 0 skipped, plus 1 from the directed complexity check

    **It answers the owner's question.** Twelve of the 416 findings were sampled and the source line read at BOTH ends: every one lands on a definition declaration, never a fragment and never a boundary-crossing span. The negative case was built rather than assumed — two files sharing a run that starts inside `alpha` and ends inside `beta` report nothing, and a control that makes the whole definitions match reports 2, which proves the probe reaches the detector.

    **The thresholds are real.** A fresh sweep reproduces the rule body exactly: 416 findings over 1183 files in 10.4 s, 395 fn / 13 struct / 8 enum, 159 intra-file, median 71 tokens, 258 at 100 percent, smallest 40 tokens, lowest similarity 90. The constants match the prose. The "two blocks differing only by a value" case cannot be dropped by the ratio, because a literal normalizes to a kind marker, so the pair normalizes to an identical stream and scores 100 — confirmed by probe.

    **The denominator is right.** It is the sum of both streams, so containment does not read as alike: a 40-token definition fully inside a 400-token one scores `2·40/440 = 18%` and reports nothing.

    **`cpd-core` is fully gone** — root manifest, member manifest, and both the package entry and dependency list in `Cargo.lock`. No source reference, no orphan code. The only mentions left are the deliberate prose explaining the removal.

    ## The one defect, measured

    `find_duplication.rs:140` and `:225`. The pair count is unbounded and the cost is quadratic. `lengths_can_reach` breaks the scan only when the two lengths DIFFER, so equal-length definitions never break out, and `shared_tokens` passes every pair with the same multiset. Same-shaped definitions therefore always pay a full O(n·m) LCS.

    Measured on one file holding k same-shape ~100-token definitions:

    | k | time |
    |---|---|
    | 200 | 12.2 s |
    | 400 | 46.4 s |
    | 800 | 181.0 s |
    | 1600 | 728.9 s |

    Four times the time per doubling — textbook quadratic. One file with 200 such definitions costs more than the whole 1183-file workspace run. LCS is also uncapped in definition size: one pair of ~28000-token definitions costs 17.2 s.

    No file in this repository triggers it today — the six largest each cost 0.22 to 0.26 s — so it is latent rather than currently biting. Generated and table-driven files reach that shape, and nothing in the code prevents it.

    **Process, and it held this time.** The reviewer ran every verification itself rather than delegating, so nothing could report back after the verdict. All findings went through the kanban API. Its probe directory was created inside the repository, because the path guard refuses anything outside, and removed afterwards.

    Finding counts by iteration: 2.

    Guardrail: NOT hit — first iteration.
  timestamp: 2026-08-09T18:24:23.337687+00:00
- actor: claude-code
  id: 01kzkysx8q43st5638n0za89n8
  text: |-
    ### Both findings are closed. One defect, two faces, and both faces are bounded.

    **The measurement first, because the review's numbers come from a debug build.** A probe under `.dupprobe/` (removed after use; the path guard refuses a file outside the working directory) reproduced the review's sweep exactly in a debug build: 1183 files, 416 findings, 10.5 s against the review's 10.4 s. A release build gives 2.4 s for the same 416, which is why every number below is a debug number — it is the build the review measured.

    **Face one, the pair count.** The candidates are now grouped by the exact stream they normalize to. Two equal streams are 100 percent alike, and 100 is the highest answer `ratio_percent` gives, so every later candidate of a group reads its answer off the earliest candidate of that group and is compared against nothing. Only the earliest candidate of each group scans its band, and the band scans shapes rather than candidates.

    The grouping key holds the LANGUAGE beside the stream. Without that, `.js` and `.ts` definitions that normalize alike would group together and pair at 100 percent, which the `two_languages_are_never_paired` test forbids.

    The band scan also stops after `MAXIMUM_COMPARED_SHAPES` shapes, which is 1024. The widest reachable band this workspace holds is 748 shapes, measured, so the limit clears it with room. `scan_bands` takes the limit as an argument and `best_partners` passes the constant, which is what makes the limit testable.

    **Face two, the pair size.** `MAXIMUM_DEFINITION_TOKENS` is 4096. A definition longer than that is not a candidate, in the same place and the same way a definition shorter than 40 is not a candidate. The longest definition this workspace declares is 1744 normalized tokens, measured, so the limit clears every real definition two times over and holds one comparison table under 17 million cells. One uncapped pair near 28000 tokens cost 12.7 s alone.

    **The k-series, before and after, same probe and same build.** The probe writes k definitions of one shape, each 139 normalized tokens.

    | k | before | after |
    |---|---|---|
    | 200 | 7.0 s | 0.1 s |
    | 400 | 27.9 s | 0.2 s |
    | 800 | 110.9 s | 0.5 s |
    | 1600 | 431.9 s | 1.0 s |

    Before: four times per doubling. After: two times per doubling, which is the parse and nothing more. The review measured 12.2 / 46.4 / 181.0 / 728.9 s for the same shape; the absolute numbers move with the size of the definition the probe writes, and the four-times-per-doubling shape is identical.

    The one enormous pair falls from 12.7 s to 0.2 s, and it now reports nothing, because it is over the new maximum.

    **The report does not move, and that is proved rather than counted.** The op was run over the 1183 tracked `.rs` files with the code at HEAD and with the new code, and each run's findings were dumped one per line. 416 lines each, and `diff` shows ZERO lines of difference. The two reports agree line for line, so every number the rule body records — 395 fn / 13 struct / 8 enum, 159 intra-file, median 71 tokens, 258 at 100 percent — holds unchanged. Only the run time moves, from 10.5 s to 9.8 s.

    **Why the answer is provably the same, and not only the same on this tree.** The old scan gives each candidate the partner that maximizes the similarity, and breaks a tie by the earliest position in the length-sorted order. Three facts make the group answer identical to it:
    - A similarity of 100 needs `2*lcs >= left+right`, and `lcs <= min`, so 100 happens ONLY for two equal streams. A group member can therefore never be beaten by a candidate outside its group.
    - Inside a group every member has the same length, and the sort is stable, so position order equals index order. The earliest eligible member is the group's own first member.
    - A non-first member is never anybody's chosen partner either: its group's first member scores the same and sits earlier, so the tie-break always took the first member anyway.

    **Finding at :140.** `similarity_percent` is now `pub fn similarity_percent(left: &[impl AsRef<str>], right: &[impl AsRef<str>]) -> usize` and `SymbolTable::intern` is `fn intern<T: AsRef<str>>(&mut self, shape: &[T]) -> Vec<u32>`, exactly as the finding writes them. The doctest now passes a `&[&str]` on one side and a `&[String]` on the other, which is the reason for the change made visible.

    **Tests, and each one was watched to fail first.**
    - `a_definition_over_the_maximum_size_is_not_compared` — RED: reported the pair. `a_definition_under_the_maximum_size_is_compared` is its control, the same generator under the limit, and it passed both before and after, so only the size stopped the big pair.
    - `a_shape_repeated_many_times_does_not_cost_a_comparison_for_each_pair` — 1200 copies of one shape. RED at 32.99 s against a 10 s budget; GREEN at 0.308 s. It also asserts all 1199 findings name the first copy at 100 percent, so the budget cannot pass on a wrong answer.
    - `a_band_stops_after_the_maximum_number_of_shapes` — three distinct shapes of one length at 95, 90 and 90 percent. With a limit of 1 the third shape names the second; with a limit of 2 it names the first. The test was checked by mutation: with `.take(maximum)` removed it fails with `left: Some(0), right: Some(1)`.

    **A dead end worth recording.** A cap counted in CANDIDATES per band was the first plan and was dropped. The workspace's widest band is 830 candidates but only 748 shapes, and a candidate-level cap would have to sit above 830 to leave 416 alone — while doing nothing at all for the case that actually bites, which is one shape repeated many times. Grouping first and capping shapes bounds the same work at a lower number and leaves the report untouched.
  timestamp: 2026-08-09T19:08:13.975298+00:00
- actor: claude-code
  id: 01kzkyt9vsrm09ek54apy0nkfe
  text: |-
    ### implement — changed
    - evidence: 2 files — `crates/swissarmyhammer-code-context/src/ops/find_duplication.rs`, `builtin/validators/duplication/rules/duplication-parsed.md`
    - bounded: equal streams grouped and answered with no comparison; `MAXIMUM_DEFINITION_TOKENS` 4096 (workspace longest 1744); `MAXIMUM_COMPARED_SHAPES` 1024 (workspace widest band 748). Past either bound the op is silent: it does not compare and does not report.
    - k-series, debug, 139-token definitions: 200 → 7.0 s to 0.1 s; 400 → 27.9 s to 0.2 s; 800 → 110.9 s to 0.5 s; 1600 → 431.9 s to 1.0 s. One 27609-token pair → 12.7 s to 0.2 s.
    - workspace: 1183 files, 416 findings, 10.5 s → 9.8 s. The two dumps agree line for line; `diff` reports 0 lines.
    - gates: `cargo nextest run --workspace` 13990 passed / 0 failed / 0 skipped (13986 before, plus 4 new tests); `cargo test --doc -p swissarmyhammer-code-context` 3 passed / 0 failed; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo doc -D warnings` holds at the 12 errors that already existed in this crate; the change adds none.
    - probe: `.dupprobe/` and the throwaway example are removed; `find . -name .dupprobe` returns nothing.
    - HEAD: ce8593e17
    - next: `/review`
  timestamp: 2026-08-09T19:08:26.873807+00:00
- actor: claude-code
  id: 01kzkzr3jzxq70w170r3200j07
  text: |-
    ### review — clean

    - evidence: `review sha ce8593e17..bd6dcd60c` — 0 findings, 0 confirmed, 0 refuted, 9 validators attempted, 0 failed, 0 skipped. The two findings of the pass of 2026-08-09 12:17 are marked done.

    **A. The two silent bounds do not stop this workspace.** This review measured the bounds again over the 1183 tracked `.rs` files, with the same parse the op uses:

    - The longest definition is 1744 normalized tokens, at `crates/swissarmyhammer-sem/src/model/identity.rs` `match_entities`. This agrees with the number the constant records.
    - 0 definitions are longer than `MAXIMUM_DEFINITION_TOKENS` (4096).
    - The widest scan window is 749 later shapes, from a shape of 63 tokens at `apps/swissarmyhammer-cli/src/commands/validate/display.rs` `format_file_display`. A band ends when `lengths_can_reach` fails, which is at 11/9 of the length.
    - 0 shapes have a scan window wider than `MAXIMUM_COMPARED_SHAPES` (1024).

    The silence is correct here. The bound is written in four places: the module doc, each constant, the doc of `find_duplication`, and the rule body section "The bounds on the work, and the measurement they came from". Each place says that a definition past the bound is not compared and is not reported. The op already keeps the same silence for a file it cannot read and for a file the roster has no grammar for, and the doc names that precedent. The tool prints findings only. It does not tell the reader that a file is clean, so it cannot make a false clean claim.

    **B. The equality argument holds.** The proof sketch leaves out one step. `keep_best` chooses the copy by candidate index, not by position in the scan order, so "the sort is stable, so the earliest eligible member is the group's first" is not sufficient on its own. The missing step is this: the first member `f` and a later member `m` have the same length, so the scan of `f` starts earlier than the scan of `m` and ends at the same length limit. The scan range of `f` therefore holds the scan range of `m`. Every pair `m` could make, `f` makes first with the same value, and `keep_best` replaces a match only for a larger value. So `m` is never the partner of any candidate. With that step added, the argument is sound.

    Two probes confirm it:

    - The new code and the code at `ce8593e17` give the same 416 findings over the same 1183 files, line for line.
    - Three orderings of one group of two equal streams plus a near neighbour (`x f m`, `f m x`, `f x m`) give the same result in both. In each ordering the partner is the first member of the group. A later member is never a partner.

    **C. The constants carry their measurement, and the tests hold them.** Each constant records the measurement in its doc comment, and the rule body records the table the numbers came from. Each test fails when the thing it holds is changed:

    - `MAXIMUM_DEFINITION_TOKENS` set to 100000 — `a_definition_over_the_maximum_size_is_not_compared` fails.
    - `MAXIMUM_DEFINITION_TOKENS` set to 100 — `a_definition_under_the_maximum_size_is_compared` fails.
    - the per-band limit removed — `a_band_stops_after_the_maximum_number_of_shapes` fails.
    - the equal-stream group removed, so each candidate is its own shape — `a_shape_repeated_many_times_does_not_cost_a_comparison_for_each_pair` fails.

    Note: the doc of `MAXIMUM_COMPARED_SHAPES` says the widest band is 748 shapes. This review measured 749. The difference is one shape, and it comes from a second count of the same quantity. It does not change the result the sentence gives: the widest band is much less than the limit of 1024.

    - next: none. The card moves to `done`.
  timestamp: 2026-08-09T19:24:43.487133+00:00
- actor: claude-code
  id: 01kzkzsqyg65s3erd339rbknw5
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 2 files. Both findings closed; they were one defect with two faces
    - test: green — cargo nextest run --workspace 13990 passed / 0 failed / 0 skipped, doctests 3 passed, fmt clean, clippy -D warnings clean
    - commit: bd6dcd60c, parent ce8593e17 — 4 files, 472 insertions, 21 deletions
    - review: **clean** — 0 findings, 0 confirmed, 0 refuted, 9 validators attempted, 0 failed, 0 skipped. Card moved to `done`

    ## What bounded the quadratic work

    Three bounds, and the first is the one that matters:
    - candidates are grouped by the exact normalized stream, with the language in the key. Two equal streams are 100 percent alike and 100 is the ratio's highest answer, so every later member of a group reads its answer off the group's earliest member and is compared against NOTHING
    - `MAXIMUM_DEFINITION_TOKENS` = 4096
    - `MAXIMUM_COMPARED_SHAPES` = 1024 per length band

    Past either numeric bound the op is silent — it does not compare and does not report.

    | k same-shape definitions | before | after |
    |---|---|---|
    | 200 | 7.0 s | 0.1 s |
    | 400 | 27.9 s | 0.2 s |
    | 800 | 110.9 s | 0.5 s |
    | 1600 | 431.9 s | 1.0 s |

    Four times per doubling became two times per doubling, which is the parse and nothing more. One 27609-token pair fell from 12.7 s to 0.2 s.

    **The report did not move, and that was proved rather than counted.** The op ran over all 1183 tracked `.rs` files at both commits, dumping findings one per line: 416 lines each, `diff` reports ZERO lines of difference.

    ## The reviewer verified rather than trusted, and found a hole in the proof

    It re-measured with the op's own parser instead of accepting the numbers: longest definition 1744 tokens (`swissarmyhammer-sem/src/model/identity.rs` `match_entities`), 0 definitions over 4096, widest band 749 shapes, 0 bands over 1024. Both bounds are unreached by this workspace.

    **It found a real hole in the equality argument.** `keep_best` picks the copy by candidate INDEX, not by scan position, so "the sort is stable, so the earliest eligible member is the group's first" does not on its own establish the claim. The missing step: `f` and `m` share a length, so `f`'s scan starts earlier and breaks at the same limit — `f`'s scan range strictly contains `m`'s, `f` forms every pair `m` could form first with the same value, and `keep_best` replaces only on a strictly larger value. With that step the argument is sound. It then confirmed it empirically with three adversarial orderings.

    It mutated the code four ways and watched each new test go red itself.

    **On the silence.** Two bounds make the detector quietly stop looking, which is the real risk here. The reviewer judged silent correct: the bound is stated in four places, the op already keeps identical silence for unreadable files and unparsed languages, and — the deciding point — the tool emits findings only and never asserts a file is clean, so it cannot make a false clean claim.

    Noted, and correctly NOT raised as a finding: the doc says the widest band is 748 and the independent count says 749. One shape, no effect on the sentence's purpose, and 749 is far below 1024. Raising it would have been manufacturing a finding.

    Finding counts by iteration: 2, 0. Guardrail never approached.

    ---

    ### The card's subject, delivered

    Duplication now compares whole named definitions — functions, methods and types — instead of sliding a 50-token window. The owner said it three ways and all three are answered: the unit is a whole definition, not a fragment; "nearly" is a measured ratio, not exact equality; and the granularity is a definition, not a few lines.

    - `cpd-core` removed. A Rabin-Karp rolling hash answers "where is a run of N tokens spelled twice" and can never rate two whole definitions. A longest-common-subsequence ratio can
    - Thresholds measured, not guessed: 40 normalized tokens at 90 percent alike, chosen by reading the boundary cases
    - 945 fragment findings became 416 definition findings; 389 intra-file became 159
    - Structural test exclusion by parse survives, and is worth 3490 → 416
    - Zero LLM calls for the duplication set on matched files, as `^a4ebnw3` proved

    Commits: `ce8593e17`, `bd6dcd60c`. Nothing pushed.
  timestamp: 2026-08-09T19:25:37.104421+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffd280
title: duplication compares whole definitions, not token windows — near-duplicate functions, methods and types
---
Correction to `^a4ebnw3`. That card built a working deterministic duplication rule, but it compares the wrong unit.

## What it does now, and why that is wrong

`find_duplication` slides a 50-token window across a file's whole token stream and hashes each window. A window knows nothing about where a definition starts or ends, so it reports the tail of one function matching the head of another, and runs of boilerplate that span two definitions.

The owner wants the other thing: **whole named definitions — functions, methods and types — that are NEARLY the same as each other.**

The measurement fits the complaint. Over 1155 tracked `.rs` files the current rule reports 945 findings with a median of 67 tokens against a 50-token floor, and 389 of them are a file matching itself. That is the shape of fragments, not of duplicated functions. Re-measure after the change and record both numbers.

## The change

**Unit.** Walk the definitions the tree-sitter parse already gives, and compare a definition against a definition. Do not slide a window. `swissarmyhammer-sem`'s duplication extractor already finds definitions — that is how the structural test exclusion decides what is a test — so the parse work is done.

**"Nearly", and it stays deterministic.** Near-match does NOT need similarity scoring, and the cosine probe stays rejected. Normalize each definition's token stream before hashing:
- every identifier becomes a positional placeholder — the first distinct identifier is `v1`, the second `v2`, and so on
- every literal becomes a marker of its kind

Then two functions that differ only by their variable names, or by one constant, hash the same. This is Type-2 clone detection. It is exact, it is deterministic, and it makes no LLM call, so `^a4ebnw3`'s acceptance bar holds unchanged.

This also catches the case the prompt rule always named as the real one — *"two blocks that differ only by a value are one function with an argument."* The current exact-token detector cannot see that case at all.

**Report.** One finding per pair, naming both definitions:

    path:line: fn `fold_grid` is a near-duplicate of `fold_band` at path:line (84 tokens)

**Threshold.** A minimum definition size, so one-line accessors and trivial `From` impls do not flood the report. Measure before choosing the number; do not guess it.

## Keep

These parts of `^a4ebnw3` were right and must survive:
- structural test exclusion, decided by the parse and NEVER by a file path
- the inline suppression marker
- `supersedes: [duplication, rust, swift]` and the empty prompt-rule reading list — zero LLM calls for the set on matched files
- the plain-text contract line the review engine parses

## Fixtures

The fail fixture changes. It is now two functions that differ ONLY by a renamed variable, which the current rule does not report and the new one must. Keep the single fail/pass pair — `find_fixture` matches the prefix `<rule>.fail.` and takes the first hit, so a second pair is silently ignored.

## Acceptance

- Two functions that differ only by identifier names are reported as one finding naming both
- Two functions that differ only by a literal are reported
- A fragment that spans two definitions is NOT reported
- Zero LLM calls for the duplication set on matched files, as `^a4ebnw3` proved
- The workspace count is re-measured and both the old and new numbers are written into the rule body

#tool-validators #objectivity

## Review Findings (2026-08-09 12:17)

Scope: `1ce5b5fd7..ce8593e17`

- [x] `crates/swissarmyhammer-code-context/src/ops/find_duplication.rs:140` — Function accepts concrete type &[String] instead of generic trait bound, forcing callers with &[&str] or other string-like types to allocate unnecessarily. Change the function signature to accept generic string types: `pub fn similarity_percent(left: &[impl AsRef<str>], right: &[impl AsRef<str>]) -> usize`. Update the `intern` method to `fn intern<T: AsRef<str>>(&mut self, shape: &[T]) -> Vec<u32>` with `shape.iter().map(|token| self.id(token.as_ref())).collect()`.
- [x] `crates/swissarmyhammer-code-context/src/ops/find_duplication.rs:225` — `best_partners` puts no limit on the number of pairs it compares, and the cost is quadratic. `lengths_can_reach` stops the inner scan only when the two lengths are too different, so definitions of equal length never stop it. `shared_tokens` lets through every pair that holds the same token multiset. Two definitions of the same shape therefore always pay a full `longest_common_subsequence`, which is O(n*m). Measured on one file that holds k definitions of one shape, each near 100 tokens: k=200 takes 12.2 s, k=400 takes 46.4 s, k=800 takes 181.0 s, k=1600 takes 728.9 s. Each doubling of k makes the time four times larger. One file with 200 such definitions costs more than the full 1183-file workspace run, which takes 10.4 s. `longest_common_subsequence` is quadratic in the length of one definition also, and no limit applies to that length: one pair of definitions near 28000 tokens takes 17.2 s. Put a limit on this work. Stop the scan for a candidate when that candidate holds a partner at 100 percent, because `keep_best` replaces a match only for a larger value. Put a limit on the number of candidates one length band compares, and a limit on the definition length that `longest_common_subsequence` accepts.

### What this pass verified and accepted

These checks passed. They need no work.

**The unit is a whole definition.** 12 findings sampled across the 416 name a definition declaration line at both ends — for example `apps/kanban-app/src/commands.rs:2308` `pub async fn spatial_navigate(` against `:2234` `pub async fn spatial_focus(`, and `crates/swissarmyhammer-commands/src/ui_state.rs:508` `pub fn inspector_close_all(` against `:486` `pub fn inspector_close(`. No sample named a fragment, and no sample named a span that crosses a definition boundary.

**A fragment that spans two definitions is NOT reported.** Two files hold a token run that starts in `alpha` and ends in `beta` and is the same in both files, while no whole definition matches a whole definition. The op reports nothing. A control that makes the whole definitions the same reports 2 findings, which shows the probe reaches the detector.

**The gates match the code.** `MINIMUM_DEFINITION_TOKENS` is 40 and `MINIMUM_SIMILARITY_PERCENT` is 90, which is what the rule body states.

**The sweep is real.** A new run over the 1183 tracked `.rs` files gives 416 findings in 10.4 s. Every number the rule body records reproduces: 395 functions, 13 structs, 8 enums; 159 intra-file; median 71 tokens; 258 of the 416 at 100 percent; smallest finding 40 tokens; lowest similarity 90 percent.

**90 percent does not miss the case the rule is for.** A literal normalizes to a marker of its kind, so two functions that differ only by a value normalize to the same stream and score 100 percent, which is above the gate. A probe of two such functions reports one finding at 100 percent alike.

**The ratio denominator is the sum of both streams.** A short definition that a long definition fully contains does not read as alike. A probe of a 40-token definition inside a 400-token definition reports nothing, because `ratio_percent` divides by `left + right`.

**`cpd-core` is fully removed.** It is gone from the root `Cargo.toml`, from `crates/swissarmyhammer-code-context/Cargo.toml`, and from `Cargo.lock` — both the package entry and the dependency list. No source file references it. The only text that remains names the removal in `builtin/validators/duplication/VALIDATOR.md` and in the rule body, which is on purpose.

**The known gap is tracked.** The 8 `#[tokio::test(...)]` findings the rule body names sit on card `^xpf86ds`, "duplication test exclusion misses a test attribute that carries arguments".