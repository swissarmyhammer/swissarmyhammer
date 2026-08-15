---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m035pe6xzf9844kmapktnypk
  text: |-
    ### New evidence from the round-4 review of ^yxky1aj — the stdout escape does not work either

    `^yxky1aj` tried the second option listed under **What to do** — carrying a declined item out on stdout as a `scope: workspace` finding. **It does not reach the author.** Verified by hand:

    `run_tool_script` (`crates/swissarmyhammer-validators/src/review/tool_rules.rs:95-100`) keeps a `scope: workspace` finding only when its path stands in `run.files`:

    ```rust
    if run.spec.scope == ToolScope::Workspace {
        let matched: BTreeSet<&str> = run.files.iter().map(String::as_str).collect();
        findings.retain(|finding| {
            matched.contains(normalize_tool_path(&finding.file, repo_root).as_str())
        });
    }
    ```

    `run.files` is not the changed-file list — it is that list filtered through the rule's OWN globs (`matched_rule_files` calls `rule.matches(...)` per path). So a rule can only report at a path its own globs select.

    **A diagnostic is therefore homeless by construction.** The natural anchor for "the tool could not answer" is a config or manifest file — `tsconfig.json`, `pyproject.toml`, `pubspec.yaml` — and a rule that lints source never declares a glob for its own config. Both channels are now measured shut:

    - **stderr + exit 0** → dropped at `tool_rules.rs:770-776` (this card).
    - **stdout + exit 0** → dropped by the workspace retain above.
    - **exit nonzero** → works, but fails the whole run over one declined item.

    So the fix cannot be a stdout convention alone. It needs a carrier that is not subject to the per-rule glob filter, because the point of a diagnostic is that it is ABOUT the run rather than about a reviewed file.

    Suggests the first option — a carrier on `ToolOutcome` — is the one to take. Whatever is chosen must be reachable from a rule whose globs match no plausible anchor path.

    `^yxky1aj` is stuck in `review` behind this card. Expect its drop-reporting apparatus to be DELETED once this lands, not repaired.
  timestamp: 2026-08-15T16:57:16.765251+00:00
- actor: claude-code
  id: 01m0361fatx257fyhxqwe8ktnv
  text: |-
    ### Research — the carrier is on `ToolOutcome`, and it reads a MARKED stderr line

    Read the code and measured the two things the design turns on.

    **1. The comment's argument holds.** `run_tool_script` filters `findings` for a `scope: workspace` run against `run.files`, and `run.files` is the changed-file list through the rule's own globs. A diagnostic is about the RUN, not about a reviewed file, so it cannot ride in `findings` and survive. A separate vector on `ToolOutcome` is the right carrier. Taking it.

    **2. Raw exit-0 stderr cannot be the diagnostic.** A tool-rule script runs a third-party tool, and the engine cannot tell a deliberate statement from progress, a deprecation notice, a lock wait or build-script output. Three shipped rules already leak raw tool stderr on the success path — `missing-docs-swift`, `function-length-swift` and `magic-numbers-swift` each run `cat "$work/lint.err" >&2` before the status gate, unconditionally. Rendering that in the report makes the block a log dump.

    So a diagnostic is a stderr line that opens with a marker. The marker is what makes the line deliberate and attributable to the rule. Unmarked stderr on a successful run goes to `tracing` at debug, so nothing is dropped on the floor any more.

    **3. What `doctor` does with stderr today.** `run_fixture` (`crates/swissarmyhammer-validators/src/doctor.rs`) calls the shared runner and counts the findings whose path is the fixture under test. On the success path it reads stdout alone — the same drop. It will read the diagnostics and write a `tracing` record; the doctor has no report block to render one in, and giving it one is a different card.

    **4. A live example of the bug, already shipped.** `missing-docs-swift` writes
    `missing-docs-swift: swiftlint cannot read .swiftlint.yml beside this rule. The run drops the project exclude list.` to stderr and carries on to exit 0 — dropped today. Its `No lintable files found` branch exits 0 with no finding and says nothing at all. Both are the shape this card names.

    Measured while judging: swiftlint 0.65.0 with `--quiet` writes 0 bytes to stderr on a clean run, and `cargo clippy --message-format=json --quiet` writes 0 bytes too. Neither measurement releases the marker — a rule cannot depend on every tool it drives staying quiet.
  timestamp: 2026-08-15T17:03:18.362200+00:00
- actor: claude-code
  id: 01m03730eh0c8b8tsdyzqf30n9
  text: |-
    ### implement — changed
    - evidence: 9 files — `builtin/validators/README.md`, `crates/swissarmyhammer-validators/src/doctor.rs`, `crates/swissarmyhammer-validators/src/review/mod.rs`, `crates/swissarmyhammer-validators/src/review/synthesize.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/execute.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_swift.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/stuttering_name_go.rs`
    - what landed: `ToolDiagnostic` on `ToolOutcome` and `ToolReport`, read from stderr lines opening `sah-diagnostic:` on a run that exited 0, rendered as its own report block. The vector stands beside the findings, so the `scope: workspace` matched-file filter never reaches it. `ToolReport::is_inert` reads it too, so a diagnostic-only run stops printing "Nothing in scope to review."
    - the shared runner: `run_script_findings` is now `run_script` and answers `ScriptOutcome { findings, diagnostics }`. The doctor fixture path counts findings exactly as before and writes a `tracing` warning per diagnostic.
    - next: test

    ### test — green
    - evidence: `cargo nextest run -p swissarmyhammer-validators` — 756 passed, 0 skipped. `cargo nextest run -p swissarmyhammer-tools -E 'test(review)'` — 86 passed. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - the load-bearing test: `a_run_that_declines_an_item_differs_from_a_run_that_found_nothing` compares two `ToolOutcome`s that agree on findings and on errors and asserts they are NOT equal. Watched RED first — the whole set failed to compile because the carrier did not exist.
    - next: commit

    ### commit — changed
    - evidence: `9fcdd8387` — 9 files changed, 456 insertions, 51 deletions
    - next: review

    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` — 8 files reviewed, 18 tasks attempted, 0 failed, 0 findings
    - next: done

    ### Follow-up filed
    `^8nbxwq5` — three shipped swiftlint rules decline an item silently and must adopt the marker. `missing-docs-swift`, `function-length-swift` and `magic-numbers-swift` each exit 0 with no finding when swiftlint answers `No lintable files found`, and each writes an unmarked diagnostic when it cannot read the project `.swiftlint.yml`.

    `^yxky1aj` is unblocked. Its stdout drop-reporting apparatus can now be deleted in favour of the marker.
  timestamp: 2026-08-15T17:21:37.233690+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff8f80
title: A tool rule's stderr is discarded on a successful run, so a diagnostic reaches nobody
---
`crates/swissarmyhammer-validators/src/review/tool_rules.rs:770-776` reads `output.stderr` **only** inside the `!output.status.success()` branch. The success path reads `output.stdout` alone and drops the stderr buffer when `output` falls out of scope.

`crates/swissarmyhammer-common/src/command.rs:56` sets `.stderr(Stdio::piped())`, so nothing is inherited to a terminal either.

**So anything a rule writes to stderr while exiting 0 reaches no author, no `tracing` record, and no report block.**

## Why this matters generally

A rule that meets something it cannot judge, and correctly declines to guess, has exactly one channel for saying so while still exiting 0 — and that channel is discarded. The rule then reads as a clean run over the thing it refused.

That is the failure mode this project has spent a whole session eliminating, stated in its own words at `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:931`:

> A run that reports no finding and exits 0 over a tree the tool never judged reads exactly like a clean tree, so a broken run must state what broke.

The rules corrected this session all took the same escape: exit NONZERO so the message survives. That is right for a broken tool, but it is the wrong shape for a rule that judged the tree successfully and declined one item — exiting nonzero there would fail the whole run over one unresolvable path.

## Found by

The reviewer of `a4a4160fe` (`^yxky1aj`). That rule refuses a path it cannot confirm and writes the refusal to stderr; the rule body claims the refusal is "said out loud" and it is not. That specific rule is being fixed on its own card by carrying the refusal out on stdout instead — this card is the general mechanism, which no single rule should have to work around.

## What to do

- Decide how a successful run reports a diagnostic. Options worth judging rather than assuming: surface exit-0 stderr through a new carrier on `ToolOutcome`; or define a stdout convention for a declined item, so it travels the channel findings already use.
- Whichever is chosen, it must reach the author — a report block or a `tracing` record, not a buffer that is dropped.
- Note the runner is shared with the doctor fixture path, so a change to `run_script_findings` has a second consumer. Check what `doctor` does with stderr today before moving it.

## Done when

- A rule that writes a diagnostic to stderr and exits 0 has that diagnostic reach the author.
- A test holds it — a rule that declines an item is observably different from a rule that found nothing.

#tool-validators