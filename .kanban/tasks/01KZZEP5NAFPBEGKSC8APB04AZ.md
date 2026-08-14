---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m0004hfd8yvk1gcsyzc4b51v
  text: |
    ### Traced the real path before editing

    What the reviewing agent is handed TODAY, for all three ops identically:

    1. `scope.rs::scope_review` resolves the scope. `resolve_working`/`resolve_sha` pick FILES from a diff; `resolve_file`/`resolve_glob` pick files by path.
    2. `FileWork::source_slice` is, despite the name, the file's COMPLETE current source for every scope.
    3. `fleet/render.rs::render_file_block` inlines that whole source under `### Full current contents` with: "This whole file is the review boundary: report every place a rule fires anywhere in it, including pre-existing instances that sit outside the change described below."
    4. The semantic diff renders under `### What changed (semantic diff — orientation only, NOT the review boundary)` with: "They are context, not the review scope: do NOT limit findings to these lines."
    5. `render_focus_files` says: "Review every line of these files, not only the lines the change touched."
    6. `fleet.rs::OUTPUT_CONTRACT` has a `## Review scope` section: "The review boundary is the WHOLE current file, not the changed lines... Pre-existing instances of a rule... are in scope and must be reported now."
    7. `fleet.rs::FOLLOWUP_PROMPT` sweeps up to 4 more turns for "pre-existing matches outside the changed region".
    8. `builtin/validators/code-hygiene/rules/missing-docs.md` says "check the WHOLE file and report every place docs are missing, not just the diff".

    So the engine does not merely fail to state the boundary — it states the OPPOSITE, four times per prompt, for a `working`/`sha` op exactly as for a `file` op. That is why zero of 53 findings landed on the change.

    The enabler already exists and is unused as a boundary: `LineAnnotation::touched()` renders a `+` mark per line and comes from the scope stage's own before/after diff. Nothing consumes it.

    Also confirmed: `resolve_sha`/`resolve_working` set `old_file_path: None` and never run git rename/copy detection, so a moved file has no base side and diffs as wholly added.
  timestamp: 2026-08-14T11:22:24.109508+00:00
- actor: claude-code
  id: 01m0029y0y75rn4ywbar2ns6cm
  text: |
    ### Implemented

    `ReviewSubject` — `Diffs` or `Files` — is the one thing the op selects. No new argument, no tool-surface change. `Scope::subject()` maps `Working`/`Sha` to `Diffs` and `File`/`Glob` to `Files`; `scope_review` reads it once and the `WorkList` carries it (batches project it verbatim).

    **What the agent is handed now.** `render_file_block` takes the subject. Under `Diffs` it prints only the changed regions — each touched line widened by a 20-line context band, overlapping bands merged, unprinted stretches replaced by an elision line, every printed line keeping its TRUE number so `Finding.line` still reads off the printed number. Under `Files` it prints the file whole, as before.

    **Where REVIEW/CONSIDER is stated.** The file block header, the focus-file list, the output contract's `## Review scope` section, the follow-up sweep prompt, `builtin/skills/review/SKILL.md`, and `builtin/validators/README.md` (the rule-authoring contract). `OUTPUT_CONTRACT` and `FOLLOWUP_PROMPT` became `output_contract(subject)` / `followup_prompt(subject)`, composed from a shared finding-field body plus three subject-specific sections, so the finding shape cannot drift between subjects.

    **Enforcement, not only instruction.** `scope::line_is_reviewed` is the single predicate. The verify guard calls it as a structural refutation beside `line_out_of_bounds`, so an off-change candidate never costs an agent turn. Tool-rule findings never pass through verify, so `run_review` applies the same predicate to them before synthesis. The `prompt-cap` engine findings are created after that filter and are never dropped.

    **Moves.** `resolve_working` and `resolve_sha` now run git rename/copy detection (`find_similar`, renames+copies at 50%). A moved file reads its BEFORE side from the path it came from and carries `old_file_path`. Detection failure degrades to an empty map — the old behaviour — never a scope error.

    **Scope always reported.** `ReviewedScope` opens every report: the op as typed, what it reviews, files reviewed, files not reviewed. Unconditional, so a narrowed scope cannot read as clean.

    **Deleted.** The existing-tests blanket exception at SKILL.md is gone, with no replacement anywhere (grep for "blanket exception" / "overrides every other rule" / "never ask to refactor" across `builtin/` returns nothing). `code-hygiene/rules/missing-docs.md` no longer says "check the WHOLE file... not just the diff".

    ### Measured, RED then GREEN

    Three real-path tests drive `scope_review` + `render_run_prime` over a real git repo:

    - one-line edit to a 4000-line file, `review sha`: with the fix the prime prints the changed band; with `Scope::subject` forced back to `Files` the prime is **196728 bytes against the whole file's 196758** — the card's defect, reproduced exactly.
    - `review file` on the same repo prints the whole file (the other half, so neither op can quietly become the other).
    - diff view at least 10x smaller than the file view.

    Plus `a_relocated_file_reviews_as_the_lines_the_move_also_changed`: a 200-line file moved with one added line marks exactly line 201; with rename detection disabled it marks all 201.

    Plus, at the tool boundary, `review_working_drops_a_finding_on_a_line_the_change_did_not_touch`: the scripted agent reports on the pre-existing line 1 and the rigged verifier would confirm it; the report renders zero findings and one refuted. With the guard disabled the finding renders.

    ### Test-fixture corrections worth knowing

    Five `drive` scenarios and three tool-layer scenarios asserted on a finding at `src/lib.rs:1` — the fixture's COMMITTED line, which the change never touched. They passed only because the engine reviewed whole files. Their findings moved to the first added line (3). This is the contract working, not a weakened test.

    ### Verified

    - `cargo nextest run --workspace` — 14185 passed, 0 failed.
    - `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
  timestamp: 2026-08-14T12:00:17.950478+00:00
- actor: claude-code
  id: 01m002a6bdw5ns7sc178n0nv2h
  text: |
    ### implement — changed
    - evidence: 17 files. Engine: `crates/swissarmyhammer-validators/src/review/{scope.rs, scope/resolve.rs, scope/batch.rs, scope/tests.rs, scope/tests_matching.rs, fleet.rs, fleet/render.rs, fleet/tests/renderer.rs, fleet/tests/budget.rs, verify.rs, synthesize.rs, drive.rs, tree_sitter_probes.rs, mod.rs}`; tools: `crates/swissarmyhammer-tools/src/mcp/tools/review/{tests.rs, review_op/tests.rs}`; docs: `builtin/skills/review/SKILL.md`, `builtin/validators/README.md`, `builtin/validators/code-hygiene/rules/missing-docs.md`. `cargo nextest run --workspace` 14185 passed / 0 failed; fmt + clippy clean.
    - next: ready for `/review`
  timestamp: 2026-08-14T12:00:26.477783+00:00
- actor: claude-code
  id: 01m002xtjh6vn9h9drqqkaknyd
  text: |-
    ### test — green
    - evidence: `cargo fmt --check` clean (0 findings). `cargo clippy --workspace --all-targets -- -D warnings` clean (0 warnings). `cargo nextest run --workspace` — 14186 tests run: 14186 passed, 0 failed, 0 skipped. Targeted re-run `cargo nextest run -p mirdan -p swissarmyhammer-skills` — 650 passed, 0 failed, 0 skipped. Targeted re-run `cargo nextest run -p swissarmyhammer-validators -p swissarmyhammer-tools` — 2233 passed, 0 failed, 0 skipped.
    - checked: no test asserts the deleted SKILL.md "existing-tests blanket exception" text (only `.kanban/` history comments reference it, not `.rs`/`.md` source). Remaining `src/lib.rs:1` assertions in `drive.rs`, `tests.rs` verified legitimate: two are `drive_file_review` (whole-file `review file` scope, where line 1 is valid), and the `review_working`/`review_sha`/`review_file` batch-size-skip tests assert on `skip_findings()` in `synthesize.rs`, which always renders a skipped file at the fixed `FILE_START_LINE` sentinel regardless of scope by design — not a stale diff-scope assertion.
    - no `#[ignore]`/skip attributes added (one `#[ignore]` hit is inside a string-literal test fixture in `tree_sitter_probes.rs`, not a real disabled test); no new `#[allow(...)]` suppressions in the changed files.
    - next: ready for review.
    task: ^apb04az
  timestamp: 2026-08-14T12:11:09.777792+00:00
- actor: claude-code
  id: 01m006dhaexkgtnhd9eyxp5q6a
  text: |
    ### review — findings
    - scope: `review sha 4b17421e5~1..4b17421e5` (that commit only), nothing excluded
    - engine state: the running engine PREDATES this commit. Both `sah serve` pids exec binaries built 2026-08-14 06:00:57; the commit is 07:12:06. `strings` finds the commit's new prompt text `is out of scope, however real it is` ONLY in `target/debug/sah`, which no process runs. This review therefore ran under the OLD whole-file engine.
    - counts: 31 findings, 31 confirmed, 23 refuted, 81 attempted, 0 failed
    - attribution: 13 findings land on lines this commit added or modified; 18 land on pre-existing lines — the old engine doing what this card fixes, not a failure of the fix
    - honesty: all 31 named lines were read and match their claim. 0 findings dropped for a false premise.
    - deleted exception: the existing-tests exception is gone, so it was NOT applied. It would have dropped 17 findings; all 17 are recorded instead.
    - evidence: crates/swissarmyhammer-tools/src/mcp/tools/review/review_op/tests.rs:499,647; crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:1486,1615,1689,1870,2079,2084,2215,2307,2340; crates/swissarmyhammer-validators/src/review/drive.rs:620,625,722,846,898,1019,1163,1243,1549; crates/swissarmyhammer-validators/src/review/fleet/render.rs:675; crates/swissarmyhammer-validators/src/review/fleet/tests/budget.rs:409,538; crates/swissarmyhammer-validators/src/review/fleet/tests/renderer.rs:558; crates/swissarmyhammer-validators/src/review/scope/tests.rs:197,198,357,761,875; crates/swissarmyhammer-validators/src/review/verify.rs:441
    - next: rebuild and reinstall `sah` so the engine contains the change, then re-review to get true acceptance evidence; fix the 13 on-change findings and the 18 pre-existing ones now recorded
  timestamp: 2026-08-14T13:12:10.318397+00:00
- actor: claude-code
  id: 01m006fsr5bgfpdt01ksz1dj0q
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 19 files. The engine did not merely fail to state the boundary, it stated the OPPOSITE four times per prompt, identically for all three ops: the file block called the whole file the review boundary and told the agent to report pre-existing instances, and the follow-up prompt spent up to four turns hunting matches outside the changed region. `ReviewSubject` (Diffs|Files) is now chosen by `Scope::subject()` alone — no new argument, no tool-surface change. Enforced rather than only asked for: `scope::line_is_reviewed` is called by the verify guard, so an off-change candidate never costs an agent turn, and again over tool-rule findings which never reach verify. The blanket existing-tests exception is deleted with no replacement.
    - The skill wording was sharpened after the implementer finished, on the user's reading: the engine prompts were already blunt, the SKILL was not. It now states the prohibition outright — report a finding only on an added or modified line, report nothing on any other line, a real defect on an unchanged line is not a finding.
    - test: green — cargo nextest run --workspace 14186 passed, 0 failed, 0 skipped; mirdan + skills 650 passed; validators + tools 2233 passed. fmt and clippy clean.
    - commit: 4b17421e5
    - review: findings — 31 recorded.

    **THE RUNNING ENGINE PREDATES THIS COMMIT, so this review is not acceptance evidence.** Proved three ways, the third being content rather than mtime inference: the new prompt text `is out of scope, however real it is` appears only in `target/debug/sah` (mtime 07:02), which nothing runs, and is ABSENT from both binaries the two live `sah serve` processes actually exec — `target/release/sah` and `~/.cargo/bin/sah`, both mtime 06:00:57, against a commit dated 07:12:06.

    So this run is the OLD whole-file engine. The finding split follows directly, by `git blame` on all 31:
    - **13 land on lines this commit added or modified** — real work, to be done now.
    - **18 land on pre-existing lines** — the old engine reading whole files, which is the precise defect this commit removes. They are not evidence the fix failed.

    17 of those 18 would have been dropped under the blanket existing-tests exception this very commit deletes. They are recorded rather than dropped, correctly.

    **Plan**: work the 13 on-change findings, then rebuild and install `sah` and re-review the same range. That rebuild is the acceptance step — and the 18 become a TEST of the fix rather than blind work: under the new engine they should not be raised at all. If any of them IS raised again, it is real and gets worked.
  timestamp: 2026-08-14T13:13:24.485291+00:00
- actor: claude-code
  id: 01m007qte9kzrcvwjbxphjp6c4
  text: |
    ### Worked the 13 on-change findings

    Scope of this pass: only the 13 findings that `git blame` puts on a line commit `4b17421e5` added or modified. The other 18 sit on pre-existing lines, are left unchecked and untouched, and get re-tested after the engine is rebuilt.

    **FIRST_CHANGED_LINE propagation (9 findings, 2 files).** Every assertion that spelled the fixture's first changed line as a literal now reads the constant.

    - `drive.rs` — the 6 named sites (620, 625, 722, 846, 1019, 1549) plus **2 extra sites the findings did not name**: the batch-merge assertions at 934 (`src/lib.rs:3`) and 939 (`src/other.rs:3`). Same cause, same file, so both were fixed.
    - `tools/review/tests.rs` — the 3 named sites (1486, 1870, 2307). **0 extra sites.** The remaining `src/lib.rs:1` at 1938 is a different cause: it is the engine's fixed skipped-file sentinel, not the fixture's changed line.

    **Deliberate non-fix, so the next pass knows why.** `drive.rs:889` spells the same literal `3` as the *input* to `shared_findings_json("src/lib.rs", 3, ...)`. It is the sibling of `drive.rs:898`, which is finding #18 on the pre-existing list and is explicitly out of scope for this pass. Fixing one of the pair without the other is worse than fixing neither, so both were left. Assertions now read the constant while these two script inputs still spell `3`; the tests pass because both are 3 today. `drive.rs:898` closes the gap when it is worked.

    **`render.rs:675` — REGION_MERGE_GAP.** Extracted `const REGION_MERGE_GAP: usize = 1;` with docs stating what the gap means (overlapping or abutting bands merge, so no single line is ever elided between two regions). **0 extra sites**: the other `1`s in the file are index arithmetic (`index + 1`, `lines[line - 1]`, `.max(1)`) and the elision-loop adjacency test, which are not this knob.

    **`scope/tests.rs:357` and `:761` — Scope::Sha now renders under ReviewSubject::Diffs.** Both tests build a `Scope::Sha` work list, so they now render with the subject that scope prescribes. **0 extra sites**: `:611` renders under `Scope::Glob`, where `Files` is correct and stays. Both tests still hold: at `:357` the 3-line file's context band covers all 3 lines, so the untouched-line-renders-a-space assertion is unchanged; at `:761` the change sits at line 191 of 191, so the band prints it with its TRUE number while the 190 filler lines above elide — a stronger test of the numbering than before.

    **`fleet/tests/renderer.rs:558` — ReviewSubject::Diffs coverage.** The suite had 0 uses of `Diffs` against 14 of `Files`. Added 8 tests covering every renderer that takes a subject:

    - `output_contract(Diffs)` × 3, mirroring the three `Files` contract tests: the marked lines are the boundary (and NOT "whole current file"), an off-change finding is refuted, a pre-existing defect is out of scope, reads beyond the inlined band are invited rather than discouraged, and completeness is demanded across every marked line.
    - `render_file_payload(Diffs)` × 3 on a 200-line fixture with one changed line: the changed region prints with its true number and `+` mark while distant lines elide; the SAME fixture under `Files` prints every line the diff view elides; and a file the change never touched says it has nothing to review rather than rendering empty.
    - `followup_prompt` × 1, both subjects — the diff sweep stays on the marked lines, the whole-file sweep still reaches outside the changed region. It had no test at all before.
    - `render_fleet_prompt(Diffs)` × 1 — the subject reaches the end of the chain.

    **RED verified, not assumed.** With `render_file_block`'s `Diffs` arm forced to `SourceView::Whole`, `diff_payload_prints_the_changed_region_and_elides_the_rest` and `diff_payload_says_so_when_the_change_touched_no_line_of_the_file` both FAIL; the mutation was reverted and both pass. The `Diffs`/`Files` pairs also assert mutually exclusive things about the same input, so no implementation that ignores the subject can satisfy both.
  timestamp: 2026-08-14T13:35:15.913179+00:00
- actor: claude-code
  id: 01m007r28f9tp3q3yb99gh6f1k
  text: |
    ### implement — changed
    - evidence: 5 files — `crates/swissarmyhammer-validators/src/review/fleet/render.rs`, `crates/swissarmyhammer-validators/src/review/fleet/tests/renderer.rs`, `crates/swissarmyhammer-validators/src/review/drive.rs`, `crates/swissarmyhammer-validators/src/review/scope/tests.rs`, `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`. 13 of 13 on-change findings checked off; 2 extra sites of the same cause fixed in `drive.rs` (934, 939). `cargo nextest run --workspace` — 14194 run, 14194 passed, 0 failed, 0 skipped. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: the 18 pre-existing-line findings stay open by design — rebuild and install `sah`, then re-review the same range under the new engine.
    task: ^apb04az
  timestamp: 2026-08-14T13:35:23.919380+00:00
- actor: claude-code
  id: 01m007vf2yam307sbm0wpdzshx
  text: |-
    ### finish iteration 2 — blocked on a rebuild, not on code
    - implement: changed — 5 files. All 13 on-change findings worked and checked off. 2 extra sites found and fixed beyond the named lines, both the same hardcoded-line cause in drive.rs batch-merge assertions. 0 false premises — every one of the 13 held exactly as described.
    - One tension recorded rather than hidden: `drive.rs:889` spells the same literal as the input to a helper whose sibling assertion at `drive.rs:898` is on the deferred list of 18. Fixing one of the pair without the other is worse than fixing neither, so both stand; the state is coherent today (assertions read the constant, the two script inputs still spell `3`, and both are 3) and `drive.rs:898` closes it when worked.
    - test: green — cargo nextest run --workspace 14194 passed, 0 failed, 0 skipped. fmt and clippy clean. RED proved by mutation: forcing the `Diffs` arm of `render_file_block` to `SourceView::Whole` fails two of the new diff-payload tests, and the `Diffs`/`Files` pairs assert mutually exclusive things about the same input, so no renderer that ignores the subject can satisfy both.
    - commit: 3194ebcb4
    - review: NOT RUN this iteration.

    **Why the loop stops here.** 18 findings remain open, and re-running the review now cannot resolve them: the running engine still predates 4b17421e5, so it would read whole files again and raise the same 18 for the same wrong reason. Looping on that would be the churn this card exists to remove.

    The 18 are a TEST of the fix, and running that test needs the new engine live:
    1. Rebuild `sah` and install it, so `~/.cargo/bin/sah` and `target/release/sah` carry `ReviewSubject`.
    2. Restart the MCP servers so the running `sah serve` execs the new binary.
    3. Re-review `4b17421e5~1..3194ebcb4`.

    Expected result, which is the acceptance evidence: the 18 pre-existing findings are NOT raised, because a diff op no longer reads unchanged lines. Any that IS raised again is real and gets worked.

    Step 2 is the user's to do — it replaces their installed CLI and restarts servers other sessions share. Asked rather than assumed.
  timestamp: 2026-08-14T13:37:15.358455+00:00
- actor: claude-code
  id: 01m00apxmnnvqr27g0qj9pkppt
  text: |-
    ### review — findings

    - scope: `review sha 4b17421e5~1..3194ebcb4` — both commits of this card. Engine's own scope line: "reviewed the diffs only — lines this change added or modified. 16 file(s) reviewed, 0 not reviewed."
    - counts: 4 findings, 4 confirmed, 2 refuted, 45 attempted, 0 failed, 0 skipped.

    **ENGINE STATE — this is the acceptance run.** The running engine CONTAINS the change. MCP server pid 82279 started 08:53:44 and `lsof` shows it execs `target/release/sah`, mtime 08:50:26, in which `strings` finds `is out of scope, however real it is` 3 times and `CONSIDER` 10 times. Note the correction to the earlier assumption: the live server maps `target/release/sah`, not `~/.cargo/bin/sah`; both are the same 08:50:26 build and both carry the marker, so the conclusion holds either way.

    **THE PREDICTION HELD. Zero of the previous round's 18 pre-existing findings were raised. None reappeared.**

    This is suppression, not silent repair — proved two ways rather than assumed:
    - Every file carrying the 18 is inside the 16 the engine reviewed (all appear in `git diff --name-only 4b17421e5~1 3194ebcb4`). The engine held those files and raised nothing on their unchanged lines.
    - 17 of the 18 subjects are still in the tree at `3194ebcb4`, unfixed: `verify.rs:441` still `candidates: Vec<Candidate>`; `budget.rs:409` `.repeat(2_400)`, `:538` `.repeat(500)`; `scope/tests.rs:197,198` `(0..30)`, timeout now `:879`; `drive.rs` chunk `6` now `:1177`/`:1257`; `tests.rs:1689` `9999`, `:2083` `.repeat(15)`, `:2215` `json!(1.5)`; `review_op/tests.rs:499` `* 6`, `:647` `* 3`.
    - The 18th, `renderer.rs:558`, was genuinely FIXED by `3194ebcb4` (0 → 9 uses of `ReviewSubject::Diffs`) and was already checked off, so its absence proves nothing either way.

    No path let one through — not the verify guard, not the tool-rule filter, not the rendered prompt.

    - attribution: 4 of 4 findings land on lines these commits ADDED. `git blame` at `3194ebcb4`: renderer.rs:616 and :624 → `3194ebcb4b`; resolve.rs:435, :516, :560 → `4b17421e54`; synthesize.rs:467 → `4b17421e54`. Zero pre-existing. The blame was run to report this evidence, not to decide scope.
    - honesty: I read all 4 named lines and confirmed each premise. **0 findings dropped for a false premise.** `marked_row`/`unmarked_row` really differ only by `+` against a space; `working_rename_sources` (435-444) and `range_rename_sources` (461-470) really carry the same match/warn shape; resolve.rs 516-523 and 560-567 really are character-identical; `retain_findings_on_the_change` really takes `Vec<VerifiedFinding>`.
    - deleted exception: gone and NOT applied. It would have dropped nothing here — `renderer.rs:616` is test code, but code this change ADDED, never pre-existing test code.
    - note for the orchestrator: `synthesize.rs:467` fires the same `rust/api-design` rule as the old `verify.rs:441` finding, on a different function in a different file, on a line this change wrote. It is a new finding, not a reappearance.
    - evidence: crates/swissarmyhammer-validators/src/review/fleet/tests/renderer.rs:616; crates/swissarmyhammer-validators/src/review/scope/resolve.rs:435; crates/swissarmyhammer-validators/src/review/scope/resolve.rs:516; crates/swissarmyhammer-validators/src/review/synthesize.rs:467
    - next: the 4 new findings are requirements and stand. The 18 prior unchecked items were artifacts of the old whole-file read and are NOT re-raised; I did not check them off — the orchestrator decides.
    task: ^apb04az
  timestamp: 2026-08-14T14:27:12.149136+00:00
- actor: claude-code
  id: 01m00ashzhgr78y36yrrg4r0fp
  text: |-
    ### finish iteration 3 — ACCEPTANCE PASSED, 4 new findings
    - rebuild: `cargo build --release` + `cargo install --path apps/swissarmyhammer-cli` at 08:50:26. MCP restarted by the user; the serving process pid 82279 started 08:53:44 and execs a binary carrying the change, confirmed by CONTENT (`strings` finds the new prompt text 3 times and `CONSIDER` 10 times), not by mtime.
    - One correction to my own note: pid 82279 maps `target/release/sah`, not `~/.cargo/bin/sah`. Both are the same 08:50:26 build and both carry the marker, so the conclusion is unchanged.
    - review: findings — 4, over `4b17421e5~1..3194ebcb4`. 45 attempted, 0 failed, 0 skipped.

    **THE PREDICTION HELD. Zero of the previous round's 18 pre-existing findings reappeared.**

    And it is suppression by scope, not silent repair — proved rather than assumed:
    - Every file carrying the 18 is in `git diff --name-only 4b17421e5~1 3194ebcb4` and among the 16 files the engine reviewed. The engine HELD those files and raised nothing on their unchanged lines.
    - **17 of the 18 subjects are still in the tree at 3194ebcb4, unfixed** — `verify.rs:441` still `candidates: Vec<Candidate>`; `budget.rs:409` still `.repeat(2_400)`, `:538` still `.repeat(500)`; `scope/tests.rs:197,198` still `(0..30)`; `drive.rs` chunk count `6`; `tools/review/tests.rs:1689` still `9999`, `:2083` still `.repeat(15)`, `:2215` still `json!(1.5)`; `review_op/tests.rs:499` still `TEST_KEEP_ALIVE * 6`, `:647` still `* 3`.
    - The 18th, `renderer.rs:558`, was genuinely fixed by 3194ebcb4 (0 → 9 uses of `ReviewSubject::Diffs`) and was already checked. Its absence proves nothing, so the evidence rests on the 17 that demonstrably remain.

    No path let one through: not the verify guard, not the tool-rule filter, not the rendered prompt.

    **A second done-when item is now visible in the product.** The report opens with a scope line: `> Scope: review sha 4b17421e5~1..3194ebcb4 — reviewed the diffs only — lines this change added or modified. 16 file(s) reviewed, 0 not reviewed.`

    **All 4 new findings land on lines these commits ADDED**, by `git blame` against 3194ebcb4 — renderer.rs:616 (3194ebcb4), resolve.rs:435, resolve.rs:516, synthesize.rs:467 (all 4b17421e5). Zero pre-existing. That is the engine doing exactly what the card asked for.

    0 findings dropped for a false premise; every named line was read and confirmed. The deleted exception was not applied and would have dropped nothing regardless — `renderer.rs:616` is test code this change ADDED, not pre-existing test code.

    Note: `synthesize.rs:467` fires the same `rust/api-design` rule as the old `verify.rs:441` finding, but in a different function in a different file on a line this change wrote. A new finding, not a reappearance.

    **The 17 remaining pre-existing defects are correctly out of scope for a diff op and stay in the tree.** That is the design, not an oversight. `/review <path>` is the tool for them when someone wants to clean those files.
  timestamp: 2026-08-14T14:28:38.513283+00:00
- actor: claude-code
  id: 01m00bpnds1v5t3kwec7bk7t5q
  text: |
    ### Worked the 4 acceptance-run findings, and closed the 18 deferred ones

    **JOB 1 — the 4 findings, all on lines these commits ADDED.** Every premise held; 0 findings dropped for a false premise. Each file was swept for the same cause, and the extra-site count is stated rather than implied.

    - `renderer.rs:616` — `marked_row`/`unmarked_row` are now one `format_row(line, mark)`, with named `CHANGED_MARK` (`'+'`) and `CONTEXT_MARK` (`' '`) at the 3 call sites, so a bare `' '` argument cannot be misread. **0 extra sites**: `fixture_line_text` and `file_work_with_changed_lines` are the file's only other helpers and neither has a near-twin. The `output_contract_under_diffs_*` tests mirror their `Files` siblings on purpose — they assert mutually exclusive things about the same input, which is a different cause.
    - `resolve.rs:435` — `renames_of_diff(diff, what)` carries the shared `match`/`find_renames`/warn/`BTreeMap::new()` tail; each caller now only builds its own diff and names it. The two warnings became one, with the lookup as a `diff` field, so the log still says which diff degraded. **0 extra sites**: `find_renames` warns on its own failure, which is a different failure at a different layer.
    - `resolve.rs:516` — `push_moved_file(builder, path, before, after, renames)` replaces both character-identical blocks, taking the rename map exactly as the finding's suggested signature does. That is deliberate rather than incidental: this file documents at `FileVersions::moved_from` that "a second bare `FilePath` parameter beside the destination path would be exactly the transposition the typed halves exist to prevent", so the helper takes the map and does the lookup itself. **0 extra sites**: `resolve_file` and `resolve_glob` each push once, with a different base side (`before` local against `BeforeContent::absent()`) and no rename lookup at all, so neither is a copy of this block.
    - `synthesize.rs:467` — `retain_findings_on_the_change` now takes `impl IntoIterator<Item = VerifiedFinding>`. **1 extra site found and fixed**: `build_candidates(work, findings: Vec<Finding>)` at the old `:968` is the same cause — a parameter that is only ever iterated — and is now `impl IntoIterator<Item = Finding>`. Two sites were measured and left, with reasons: `validator_work(name, files: Vec<FileWork>)` forwards its argument straight into `ValidatorWork::new`, which requires an owned `Vec`, so a generic there only adds a re-collect; and `ReviewCounts::skipped_files` / `SkipGroup::validators` are struct fields, not parameters. The file already used `dedup_exact(findings: impl Iterator<…>)`, so the generic form is this file's prevailing pattern, not a new one.

    **JOB 2 — the 18 deferred findings from the 07:13 section are checked off**, each with a one-line reason. This is the acceptance evidence, not a courtesy tick: every file carrying them was inside the 16 the new engine reviewed, and 17 of the 18 subjects are STILL in the tree, unfixed — so the engine held those files and correctly raised nothing on their unchanged lines. The 18th, `renderer.rs:558`, was genuinely fixed by `3194ebcb4` and was already checked.

    **The 17 pre-existing defects stay in the tree by design.** They were NOT fixed. A pre-existing defect is out of scope for a diff op, which is the whole point of this card; `/review <path>` is the tool for them when someone wants to clean those files.

    ### Verified

    - `cargo nextest run --workspace` — 14194 tests run, 14194 passed, 0 failed, 0 skipped.
    - `cargo nextest run -p swissarmyhammer-validators` — 720 passed, 0 failed, 0 skipped.
    - `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - Blast radius: every symbol touched is private. `push_moved_file` and `renames_of_diff` are new file-private helpers; `retain_findings_on_the_change` and `build_candidates` are private to `synthesize.rs` and both callers pass a `Vec`, which `impl IntoIterator` accepts unchanged; `format_row` is test-only. No public declaration changed.
  timestamp: 2026-08-14T14:44:32.313685+00:00
- actor: claude-code
  id: 01m00bpwx1rates1z35fd1kmak
  text: |
    ### implement — changed
    - evidence: 3 files — `crates/swissarmyhammer-validators/src/review/fleet/tests/renderer.rs`, `crates/swissarmyhammer-validators/src/review/scope/resolve.rs`, `crates/swissarmyhammer-validators/src/review/synthesize.rs`. 4 of 4 acceptance findings worked and checked; 1 extra site of the same cause fixed in `synthesize.rs` (`build_candidates`). 18 deferred 07:13 findings checked off with a reason each; 0 unchecked items remain on the card. `cargo nextest run --workspace` — 14194 run, 14194 passed, 0 failed, 0 skipped. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: ready for `/review`. The 17 pre-existing defects named in the 07:13 section are deliberately still in the tree — out of scope for a diff op, and `/review <path>` is the tool for them.
    task: ^apb04az
  timestamp: 2026-08-14T14:44:39.969256+00:00
position_column: doing
position_ordinal: '8280'
title: Review the diffs for a working or sha op, and the whole file for a file op
---
The review engine reviews whole FILES that a change touched, not the CHANGE. So a one-line edit to a 4000-line file offers the reviewer 4000 lines, and the reviewer reports defects that were there before the author arrived.

This is the single largest cost in the finish loop. It does not merely add noise — it changes the agent's behaviour. Every round the author must stop, run `git blame` on each finding, decide whether the line is theirs, and write a paragraph justifying what it did and did not touch. That paragraph is the tell: the agent is manually reconstructing, per finding, the delta the engine should have handed it.

## Measured across one session, 2026-08-12 to 2026-08-14

| card | findings raised | on the change | pre-existing | dropped by hand |
| --- | --- | --- | --- | --- |
| `^wwb6hk7` | 3 | **0** | 3 (other cards' work in the same commit) | 0 |
| `^c9pb2f3` | 2 | 0 | 1 | 1 exception + 1 false premise |
| `^q2cncse` re-review | 17 | **0** | 17 | 17 exception |
| `^4kzxdex` round 1 | 3 | **0** | 3 (all blamed to 236d021f8) | 0 |
| `^0fn6dbf` | 28 | **0** | 28 | 18 exception |

**Zero of 53 findings landed on the change under review.**

- `^4kzxdex` took FOUR rounds; rounds 2 and 3 found defects created by the previous round's own remediation of pre-existing code.
- `^0fn6dbf` spawned **175 concurrent agents** and killed two review attempts before its scope was cut by hand to 11 files.
- `^wwb6hk7` sat stuck a round on three findings its own work never produced.

## The op already says which mode is meant

**No new argument, and no change to the tool surface.** The three ops already carry the intent, and it only needs to be stated and obeyed:

| op | means | review |
| --- | --- | --- |
| `review working` | uncommitted changes vs `HEAD` | **the diffs only** |
| `review sha` (commit or range) | the changes in that commit or range | **the diffs only** |
| `review file` (path or glob) | these files | **the whole file** |

Asking about a sha or the working tree is asking about a CHANGE. Asking about a file or a glob is asking about a FILE. The engine currently answers both the same way — it uses the diff to pick files, then hands over whole files.

**Proof that scope-selection is not the missing piece**: `builtin/skills/review/SKILL.md:102` already has `/finish` pass `HEAD~1..HEAD`, so every finish review this session was already narrowed to one commit's files. It made no difference — all 53 findings above came from reviews scoped exactly that tightly. Narrowing WHICH FILES cannot fix reading the WHOLE FILE.

## So the work is instructions, not plumbing

What is missing is a clear statement, everywhere the reviewing agent is prompted, of **what to review** against **what to merely consider**:

- **Review** — the subject. Only lines the change added or modified. A finding must land on one of them.
- **Consider** — context. Surrounding and related pre-existing code, read to judge the change correctly, never itself a subject of a finding. A pre-existing defect is out of scope even when it is real, and even when a validator flags it.

That distinction has to be stated in each place an agent is told what to do, not only once:
- `builtin/skills/review/SKILL.md` — the driver. Say which ops mean diffs and which mean files, and that `/finish` always uses a diff op.
- The fan-out prompt each review agent receives.
- The rule prompts themselves, where they speak about what to flag.

Watch the vocabulary: `SKILL.md` already uses "mode" for task-mode against range-mode (WHERE FINDINGS LAND) and "scope" for the op (WHICH FILES). Do not add a third meaning for either word — say "review the diffs" and "review the files" plainly.

## DELETE the existing-tests exception

`SKILL.md:27` carries a blanket exception dropping any finding whose subject is changing pre-existing TEST code, stated as overriding every other rule. **Remove it entirely.**

It is wrong in both directions:

1. **It is a manual patch over this card's defect.** 36 of the 53 findings above were dropped through it by hand, each needing a `git show <sha>^` to confirm. Once a diff op reviews only diffs, pre-existing test code is out of scope structurally, and nothing needs dropping.
2. **It breaks an explicit request.** `/review tests/some_test.rs` is a legitimate and wanted thing to do — reviewing a test file on purpose, to clean it up. A blanket exception that overrides every other rule guts exactly that request, silently. The user asked about that file; the answer must not be pre-empted.

Its stated secondary reason — that rewriting an existing test file collides with the upstream test suite a change is graded against — needs no exception either, because both paths already handle it: a diff op never raises those findings, and a file op means the user deliberately asked for that file.

It also never covered production code, which is exactly how `^4kzxdex` lost four rounds — its findings were production lines blamed to a commit from days earlier, so no exception applied and they counted as requirements. Fixing the read covers production and test code alike, with no special case for either.

## Two further items

**Recognise a move as a move.** Use git's rename and copy detection when resolving a diff. `^0fn6dbf` read as 70 files, 20548 insertions and 20097 deletions when its real delta was about 39 lines of module wiring — each source shed thousands of lines and gained one `mod` line. A relocation is a move of CONTENT plus an EDIT of the source; the engine sees only a delete plus an add.

**Report the scope, always.** State what was reviewed and what was excluded, so a narrowed scope can never read as a clean result. `^0fn6dbf` moved to `review` with zero findings after its agent stalled, which read as clean and was nothing at all.

## Done when

- `review working` and `review sha` report findings only on added or modified lines.
- `review file` on a path or glob reviews those files whole — including a test file, with no exception suppressing the answer.
- No review round needs `git blame` to decide whether a finding belongs to the change.
- The existing-tests exception is GONE from `SKILL.md`, and no replacement special case for test code exists anywhere.
- `SKILL.md`, the fan-out prompt, and the rule prompts each state what to review against what to consider.
- A relocation commit reviews in proportion to its real delta.
- Every report names its scope and its exclusions.

#tool-validators

## Review Scope and Engine State (2026-08-14 07:13)

Scope reviewed: `review sha 4b17421e5~1..4b17421e5` — that commit only. Nothing excluded.

**The running review engine does NOT contain this commit.** Evidence:

- Commit `4b17421e5` is dated 2026-08-14 07:12:06 -0500.
- Both `sah serve` processes exec binaries built BEFORE it: pid 65954 runs `target/release/sah` (mtime 2026-08-14 06:00:57, started 06:31:16), pid 59780 runs `~/.cargo/bin/sah` (same mtime).
- Direct string test: `strings` finds the commit's new prompt text `is out of scope, however real it is` in `target/debug/sah` only. It is MISSING from `~/.cargo/bin/sah` and from `target/release/sah`. No process runs the debug binary.

So this review ran under the OLD whole-file engine. 18 of the 31 findings below sit on lines this commit never touched — that is the old engine doing exactly what this card fixes, NOT evidence the fix failed. Per-finding attribution by `git blame` against `4b17421e5`:

- **On lines this commit added or modified (13)**: tools/review/tests.rs:1486, 1870, 2307; drive.rs:620, 625, 722, 846, 1019, 1549; render.rs:675; scope/tests.rs:357, 761. (renderer.rs:558 anchors on an older line but its subject, `ReviewSubject`, was introduced by THIS commit.)
- **On pre-existing lines (18)**: review_op/tests.rs:499, 647; tools/review/tests.rs:1615, 1689, 2079, 2084, 2215, 2340; drive.rs:898, 1163, 1243; budget.rs:409, 538; renderer.rs:558; scope/tests.rs:197, 198, 875; verify.rs:441.

**Honesty check**: every one of the 31 findings names a line that exists and matches its claim. I read each named line. **Zero findings were dropped for a false premise.** Specifically confirmed: `FIRST_CHANGED_LINE` really is defined in-scope in both `tools/review/tests.rs:2468` (file-level const, flat file) and `drive.rs:560` (inside the single flat `mod tests`, 424..2216); `renderer.rs` really has 0 uses of `ReviewSubject::Diffs` against 14 of `ReviewSubject::Files`; `scope/tests.rs:357` and `:761` really do build `Scope::Sha` work lists and then render with `ReviewSubject::Files`; `verify_findings` at `verify.rs:441` really takes `candidates: Vec<Candidate>` and only ever borrows it (`run_guard(&candidates, subject)`).

**Deleted exception**: the blanket existing-tests exception was removed by this commit, so it was NOT applied. Under it I would have dropped 17 findings — review_op/tests.rs:499, 647; tools/review/tests.rs:1615, 1689, 2079, 2084, 2215, 2340; drive.rs:898, 1163, 1243; budget.rs:409, 538; renderer.rs:558; scope/tests.rs:197, 198, 875. All 17 are recorded below instead.

**Closed 2026-08-14 (acceptance): the 18 pre-existing-line findings below are checked off.** The acceptance run under the NEW engine (`review sha 4b17421e5~1..3194ebcb4`, section below) reviewed every file that carries them and raised none of them. That is suppression by scope, not silent repair: 17 of the 18 subjects are demonstrably STILL in the tree, unfixed, and they stay there by design — a pre-existing defect is out of scope for a diff op. `/review <path>` is the tool for them when someone wants to clean those files.

## Review Findings (2026-08-14 07:13)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op/tests.rs:499` `magic-numbers/no-magic-numbers` — Multiplier 6 configures test timing (advancing clock 6 intervals) but is unexplained; should be a named constant describing what 6 represents. Define a named constant like `const KEEP_ALIVE_INTERVALS_TO_VERIFY_DISARMED: usize = 6;` and use it: `advance(TEST_KEEP_ALIVE * KEEP_ALIVE_INTERVALS_TO_VERIFY_DISARMED).await;`. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op/tests.rs:647` `magic-numbers/no-magic-numbers` — Multiplier 3 configures test timing (advancing clock 3 intervals) but is unexplained; should be a named constant describing what 3 represents. Define a named constant like `const KEEP_ALIVE_WINDOWS_FOR_MULTIPLE_TICKS: usize = 3;` and use it: `advance(TEST_KEEP_ALIVE * KEEP_ALIVE_WINDOWS_FOR_MULTIPLE_TICKS).await;`. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:1486` `code-hygiene/magic-numbers` — The literal `3` is hardcoded in a test assertion string and should use the `FIRST_CHANGED_LINE` constant defined elsewhere in the file. Use `format!("- [ ] `src/lib.rs:{}`", FIRST_CHANGED_LINE)` to build the expected string dynamically, or assign the string to a test constant.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:1615` `reuse/reuse` — planted_duplicate_fixture_committed reimplements the entire fixture setup that planted_duplicate_fixture provides. Except for one additional commit, the functions are identical in validator setup, index seeding, and agent scripting. Parameterize planted_duplicate_fixture to accept a boolean flag controlling whether to commit the duplicate (leaving it as working-tree change or committed), eliminating the duplication. Or make planted_duplicate_fixture_committed call planted_duplicate_fixture and then issue the extra commit. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:1689` `magic-numbers/no-magic-numbers` — Magic number 9999 used as test input for out-of-bounds line number — should be a named constant to explain the test's intent. Create a named constant like `const OUT_OF_BOUNDS_LINE: u32 = 9999;` and use it in place of the literal. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:1870` `code-hygiene/magic-numbers` — The literal `3` is hardcoded in a test assertion string and should use the `FIRST_CHANGED_LINE` constant defined elsewhere in the file. Use `format!("- [ ] `src/lib.rs:{}`", FIRST_CHANGED_LINE)` to build the expected string dynamically.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:2079` `magic-numbers/no-magic-numbers` — Magic number 4 used as denominator in ratio calculation (3/4 of file cap) — should be a named constant to clarify the fraction. Create named constants like `const CAP_FRACTION_DENOMINATOR: u64 = 4;` or better yet, use `const TARGET_FRACTION: f64 = 0.75;` to express the intent directly. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:2079` `magic-numbers/no-magic-numbers` — Magic number 3 used as numerator in ratio calculation (3/4 of file cap) — should be a named constant to clarify the fraction. Create a named constant like `const CAP_FRACTION_NUMERATOR: u64 = 3;` or use `const TARGET_FRACTION: f64 = 0.75;` for clarity. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:2084` `magic-numbers/no-magic-numbers` — Magic number 15 used as string repetition count for test fixture generation — should be a named constant to document the deliberate test size. Create a named constant like `const FILLER_TEXT_REPETITIONS: usize = 15;` and use it in place of the literal. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:2215` `magic-numbers/no-magic-numbers` — Magic number 1.5 used as test input for fractional batch_size validation — should be a named constant to clarify the test intent. Create a named constant like `const FRACTIONAL_BATCH_SIZE_TEST_INPUT: f64 = 1.5;` to explain this is deliberately testing non-integer input handling. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:2307` `code-hygiene/magic-numbers` — The literal `3` is hardcoded in a test assertion string and should use the `FIRST_CHANGED_LINE` constant defined elsewhere in the file. Use `format!("- [ ] `src/lib.rs:{}`", FIRST_CHANGED_LINE)` to build the expected string dynamically.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:2340` `magic-numbers/no-magic-numbers` — Magic number 2 used to configure review tool concurrency for testing — should be a named constant to clarify the test configuration intent. Create a named constant like `const TEST_CONCURRENCY_LEVEL: usize = 2;` to document why this specific concurrency is chosen for testing. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:620` `completeness/invariant-propagation` — The constant FIRST_CHANGED_LINE was introduced to centralize the invariant 'first changed line is 3 for seeded_dup_repo tests', but this assertion still uses the hardcoded literal instead of the constant, creating maintenance risk if the invariant ever changes. Replace the hardcoded string "3" with a format!() call: `report.markdown().contains(&format!("- [ ] `src/lib.rs:{}`", FIRST_CHANGED_LINE))`.
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:625` `completeness/invariant-propagation` — Hardcoded line number in assertion should use FIRST_CHANGED_LINE constant for consistency and maintainability. Replace hardcoded string with `report.markdown().contains(&format!("src/lib.rs:{}", FIRST_CHANGED_LINE))`.
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:722` `completeness/invariant-propagation` — Hardcoded line number in assertion should use FIRST_CHANGED_LINE constant for consistency and maintainability. Replace hardcoded string with `report.markdown().contains(&format!("- [ ] `src/lib.rs:{}`", FIRST_CHANGED_LINE))`.
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:846` `completeness/invariant-propagation` — Hardcoded line number in assertion should use FIRST_CHANGED_LINE constant for consistency and maintainability. Replace hardcoded string with `report.markdown().contains(&format!("- [ ] `src/lib.rs:{}`", FIRST_CHANGED_LINE))`.
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:898` `magic-numbers/no-magic-numbers` — Magic number 3 hardcodes the first changed line in test findings — should use the already-defined FIRST_CHANGED_LINE constant to keep test line expectations in sync. Replace 3 with FIRST_CHANGED_LINE: shared_findings_json("src/other.rs", FIRST_CHANGED_LINE, "r", "other-dup-claim"). — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:1019` `completeness/invariant-propagation` — Hardcoded line number in assertion should use FIRST_CHANGED_LINE constant for consistency and maintainability. Replace hardcoded string with `report.markdown().contains(&format!("- [ ] `src/lib.rs:{}`", FIRST_CHANGED_LINE))`.
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:1163` `magic-numbers/no-magic-numbers` — Magic number 6 configures test stream chunking without explanation — should be a named constant describing the intent (e.g., test chunk count for replica invariant tests). Extract a const TEST_STREAM_CHUNK_COUNT: usize = 6; and use it both at line 1163 and line 1243. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:1243` `magic-numbers/no-magic-numbers` — Magic number 6 configures test stream chunking without explanation — should be a named constant describing the intent (e.g., test chunk count for drain window regression tests). Extract a const TEST_STREAM_CHUNK_COUNT: usize = 6; and use it both at line 1163 and line 1243. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:1549` `completeness/invariant-propagation` — Hardcoded line number in assertion should use FIRST_CHANGED_LINE constant for consistency and maintainability. Replace hardcoded string with `report.markdown().contains(&format!("- [ ] `src/lib.rs:{}`", FIRST_CHANGED_LINE))`.
- [x] `crates/swissarmyhammer-validators/src/review/fleet/render.rs:675` `magic-numbers/no-magic-numbers` — Unexplained numeric literal 1 in region-merge logic — this represents the maximum line gap for merging adjacent changed regions and should be a named constant. Extract `const REGION_MERGE_GAP: usize = 1;` and use `start <= last.1 + REGION_MERGE_GAP`.
- [x] `crates/swissarmyhammer-validators/src/review/fleet/tests/budget.rs:409` `magic-numbers/no-magic-numbers` — Unexplained numeric literal 2_400 in test fixture — represents the line count for oversized test files and should be a named constant. Extract `const OVER_CAP_TEST_LINES: usize = 2_400;` and use `.repeat(OVER_CAP_TEST_LINES)`. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-validators/src/review/fleet/tests/budget.rs:538` `magic-numbers/no-magic-numbers` — Unexplained numeric literal 500 in test fixture — represents the repetition count for test rule-body sizing and should be a named constant. Extract `const TEST_RULE_BODY_REPETITIONS: usize = 500;` and use `.repeat(TEST_RULE_BODY_REPETITIONS)`. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-validators/src/review/fleet/tests/renderer.rs:558` `completeness/invariant-propagation` — The test suite exercises ReviewSubject::Files exclusively, but ReviewSubject::Diffs is a real enum variant (created by Scope::subject() for working-tree and range reviews). Render functions must handle both variants consistently, requiring tests for both code paths. Add tests for ReviewSubject::Diffs to mirror the Files tests—e.g., a test that calls output_contract(ReviewSubject::Diffs) and verifies the contract correctly names the changed lines (diffs) as the review boundary, not the whole file.
- [x] `crates/swissarmyhammer-validators/src/review/scope/tests.rs:197` `magic-numbers/no-magic-numbers` — Hardcoded literal `30` configures test fixture padding generation and should be a named constant for clarity and maintainability. Extract to a named constant: `const PADDING_LINES: usize = 30;` at the top of the test function, then use `(0..PADDING_LINES)` in both places. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-validators/src/review/scope/tests.rs:198` `magic-numbers/no-magic-numbers` — Hardcoded literal `30` configures test fixture padding generation and should be a named constant for clarity and maintainability. Use the same named constant `PADDING_LINES` instead of repeating the literal `30`. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-validators/src/review/scope/tests.rs:357` `completeness/invariant-propagation` — Scope::Sha (sha range review) should render diffs, but test explicitly passes ReviewSubject::Files. Per change: "review working and review sha review the diffs only, review file reviews the whole file." — the scope choice should determine ReviewSubject, not contradict it. Change line 357 to use ReviewSubject::Diffs to match the scope's prescribed behavior.
- [x] `crates/swissarmyhammer-validators/src/review/scope/tests.rs:761` `completeness/invariant-propagation` — Scope::Sha (sha range review) should render diffs, but test explicitly passes ReviewSubject::Files. Same issue as line 357 — the scope choice (sha) contradicts the rendering choice (Files). Change line 761 to use ReviewSubject::Diffs to match the scope's prescribed behavior.
- [x] `crates/swissarmyhammer-validators/src/review/scope/tests.rs:875` `magic-numbers/no-magic-numbers` — Hardcoded timeout of 5 seconds is unexplained and should be a named constant. The comment above discusses test expectations but does not explicitly justify the 5-second limit. Extract to a named constant at the top of the test: `const BLAME_TIMEOUT_SECS: u64 = 5;` then use `Duration::from_secs(BLAME_TIMEOUT_SECS)`. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**
- [x] `crates/swissarmyhammer-validators/src/review/verify.rs:441` `rust/api-design` — Function parameter accepts `Vec<Candidate>` but should accept `&[Candidate]` per the rule 'Accept generics, not concrete types' — the function only borrows the parameter and never requires ownership, forcing unnecessary moves on callers. Change parameter from `candidates: Vec<Candidate>` to `candidates: &[Candidate]`. Callers can pass `&vec` or `&vec[..]` with no friction. — **Closed: the acceptance run reviewed this file and did not raise it; a pre-existing line is out of scope for a diff op.**

## Acceptance Re-Review Under the NEW Engine (2026-08-14 08:55)

Scope reviewed: `review sha 4b17421e5~1..3194ebcb4` — both commits of this card, not the working tree.

### 1. The running engine CONTAINS this card's change

- MCP server pid 82279 started 2026-08-14 08:53:44. `lsof` shows it execs `/Users/wballard/github/swissarmyhammer/swissarmyhammer/target/release/sah` (inode 389059752), NOT `~/.cargo/bin/sah`.
- That binary has mtime 2026-08-14 08:50:26. `strings` finds the new prompt text `is out of scope, however real it is` 3 times in it, and `CONSIDER` 10 times. `~/.cargo/bin/sah` is the same 08:50:26 build and carries the same marker.
- Third, independent confirmation from the output itself: the report now opens with a scope line — "reviewed the diffs only — lines this change added or modified. 16 file(s) reviewed, 0 not reviewed." That is the card's "Report the scope, always" item, visible in the product. The range touches 21 paths; the 5 the engine did not take are the 2 `.kanban/` task files and the 3 markdown documents.

### 2. THE PREDICTION HELD — zero of the 18 pre-existing findings reappeared

Not one of the 18 was raised. Named reappearances: **none**.

This is suppression, not silent repair. Verified at `3194ebcb4`:

- All six files carrying the 18 are inside the 16 the engine reviewed — `review_op/tests.rs`, `tools/review/tests.rs`, `drive.rs`, `budget.rs`, `renderer.rs`, `scope/tests.rs`, `verify.rs` are all in `git diff --name-only 4b17421e5~1 3194ebcb4`. The engine held these files and raised nothing on their unchanged lines.
- 17 of the 18 subjects are still present in the tree, unfixed: `verify.rs:441` still reads `candidates: Vec<Candidate>`; `budget.rs:409` still `.repeat(2_400)` and `:538` still `.repeat(500)`; `scope/tests.rs:197,198` still `(0..30)` and the 5-second timeout now at `:879`; `drive.rs` chunk count `6` now at `:1177` and `:1257`; `tools/review/tests.rs:1689` still `9999`, `:2083` still `.repeat(15)`, `:2215` still `json!(1.5)`; `review_op/tests.rs:499` still `TEST_KEEP_ALIVE * 6` and `:647` still `* 3`.
- The 18th, `renderer.rs:558`, was genuinely FIXED by `3194ebcb4` and was already checked off — that file went from 0 uses of `ReviewSubject::Diffs` to 9. Its absence proves nothing either way, so the suppression evidence rests on the 17 that are demonstrably still there.

No path let a pre-existing finding through: not the verify guard, not the tool-rule filter, not the rendered prompt.

### 3. Per-finding attribution — all 4 land on the change

`git blame` against `3194ebcb4`:

- `renderer.rs:616` and its pair at `:624` — both `3194ebcb4b`. Added by this card.
- `resolve.rs:435` — `4b17421e54`. Added by this card.
- `resolve.rs:516` and its pair at `:560` — both `4b17421e54`. Added by this card.
- `synthesize.rs:467` — `4b17421e54`. Added by this card.

4 of 4 on added lines. Zero pre-existing. The `git blame` above was run to report this evidence, not to decide scope — the engine had already decided it.

### 4. Honesty check

I read every named line and confirmed each premise before recording. **Zero findings dropped for a false premise.**

- `marked_row` (616-621) and `unmarked_row` (624-629) really differ only by `+` against a space in the format string.
- `working_rename_sources` (435-444) and `range_rename_sources` (461-470) really carry the same `match`/`Ok(mut diff) => find_renames(&mut diff)`/`Err(e) => warn + BTreeMap::new()` shape, differing only in the diff call and the message.
- `resolve_working` lines 516-523 and `resolve_sha` lines 560-567 really are character-identical, `moved_from: moved_from(&renames, path)` included.
- `retain_findings_on_the_change` at `synthesize.rs:466` really takes `findings: Vec<VerifiedFinding>`.

**Deleted exception**: the blanket existing-tests exception is gone and was NOT applied. It would have dropped nothing here in any case — `renderer.rs:616` is test code, but code this change ADDED, never pre-existing test code.

## Review Findings (2026-08-14 08:55)

> Scope: `review sha 4b17421e5~1..3194ebcb4` — reviewed the diffs only — lines this change added or modified. 16 file(s) reviewed, 0 not reviewed.

- [x] `crates/swissarmyhammer-validators/src/review/fleet/tests/renderer.rs:616` `duplication/duplication` — Function `marked_row` and `unmarked_row` (line 624) are identical except for the mark character (`+` vs space). These should be one function parameterized by the mark character to avoid drift if one is updated without the other. Extract a single helper function `format_row(line: usize, mark: char)` that constructs the row with the mark parameter, eliminating both `marked_row` and `unmarked_row`. — **Fixed: one `format_row(line, mark)` replaces both, with named `CHANGED_MARK`/`CONTEXT_MARK` at the 3 call sites. Swept renderer.rs: 0 extra sites of the same cause.**
- [x] `crates/swissarmyhammer-validators/src/review/scope/resolve.rs:435` `duplication/rust` — Error handling pattern duplicates between `working_rename_sources` and `range_rename_sources`: both have identical match blocks that call `find_renames(&mut diff)` on success and warn + return empty map on failure, differing only in the diff method call and message string. Extract a helper function parameterized by diff method and error message. Extract a helper function `fn find_renames_from_diff_result(diff_result: Result<Diff>, error_msg: &str) -> BTreeMap` to unify the error handling, or use a closure-based helper that accepts a function producing the Diff, eliminating the duplicated match/warning pattern across both functions. — **Fixed: `renames_of_diff(diff, what)` carries the match/warn tail; each caller now only builds its own diff. Swept resolve.rs: 0 extra sites of the same cause.**
- [x] `crates/swissarmyhammer-validators/src/review/scope/resolve.rs:516` `duplication/rust` — Verbatim duplication of `builder.push` call: lines 516-523 in `resolve_working` are character-identical to lines 560-567 in `resolve_sha`, including the FileVersions struct construction with `moved_from: moved_from(&renames, path)`. Extract a helper to eliminate the repeated block. Extract a helper function `fn push_file_versions(builder: &mut FileChangeBuilder, path: &str, before: BeforeContent, after: AfterContent, renames: &BTreeMap<String, String>)` and call it from both loops, or refactor to reduce this structural duplication. — **Fixed: `push_moved_file(builder, path, before, after, renames)` replaces both blocks, with the signature the finding names. Swept resolve.rs: 0 extra sites — `resolve_file` and `resolve_glob` each push once, with a different base side and no rename lookup, so neither is a copy of this block.**
- [x] `crates/swissarmyhammer-validators/src/review/synthesize.rs:467` `rust/api-design` — Function accepts concrete `Vec<VerifiedFinding>` instead of generic `impl IntoIterator<Item = VerifiedFinding>`, limiting flexibility to the caller. Change parameter to `findings: impl IntoIterator<Item = VerifiedFinding>` and collect upfront: `let findings_vec: Vec<_> = findings.collect();` after the function signature, then use `findings_vec` throughout. This allows callers to pass any iterator, not just owned Vec instances. — **Fixed: the parameter is now `impl IntoIterator<Item = VerifiedFinding>`. Swept synthesize.rs: 1 extra site of the same cause, `build_candidates(work, findings: Vec<Finding>)`, fixed the same way.**