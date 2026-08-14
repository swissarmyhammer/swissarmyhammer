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
2. **It breaks an explicit request.** `/review tests/some_test.rs` is a legitimate and wanted thing to do — reviewing a test file on purpose, to clean it up. A blanket exception that overrides every other rule guts exactly that request, silently. The user asked about that file; the answer must not be pre-emptied.

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