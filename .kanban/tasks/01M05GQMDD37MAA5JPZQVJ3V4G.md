---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m05jjk8z20acvgn78qbtaje6
  text: |
    ## Measured first, before any edit

    cargo-machete 0.9.2 at `/Users/wballard/.cargo/bin/cargo-machete`. Probe tree of
    two manifests, staged so the one that CAN be read comes first in the script's
    `sort` order:

    - `Cargo.toml` — package `unused-dependency-probe`, uses `libc`, declares an
      unused `serde` on line 8, with `[workspace]` to keep cargo inside the tree.
    - `src/lib.rs` — names `libc` and never `serde`.
    - `unparsable/Cargo.toml` — `[dependencies` header with no closing bracket.
      `unparsable` sorts after `Cargo.toml` under a byte order AND under a
      case-folding order, so the order holds whatever locale `sort` reads.

    Raw machete, one process for each manifest:

    | the run | exit | stdout | stderr |
    |---|---|---|---|
    | `cargo-machete ./Cargo.toml` | 1 | `cargo-machete found the following unused dependencies` then `serde` | `Analyzing…`, `Done!` |
    | `cargo-machete ./unparsable/Cargo.toml` | 0 | `didn't find any unused dependencies` | `Analyzing…`, `error when handling ./unparsable/Cargo.toml: TOML parse error at line 6, column 14`, 11 lines more, `Done!` |

    The SHIPPED script over the whole probe tree, exactly as it stood:

    - stdout: 1 line —
      ``Cargo.toml:8: unused dependency `serde`: …``
    - stderr: machete's 13 raw lines, then
      `unused-dependencies-rust: cargo machete could not read unparsable/Cargo.toml`
    - exit: **1**

    That is the defect the card names, measured. The finding was written and the
    nonzero exit made the engine read none of it — `read_script_output` answers
    `Err` for a nonzero exit before it reads stdout at all.

    ## The finding loop was already sound

    The card's sibling defect — a loop iterating non-finding rows — is NOT present
    here. The `awk` filter starts a listing at `^cargo-machete found`, prints only
    indented lines, and closes the listing on a blank line or a line without ` -- `.
    Measured over the probe: exactly 1 line on stdout, and nothing from the
    `didn't find any unused dependencies` report. No change was needed there.
  timestamp: 2026-08-16T15:20:51.231328+00:00
- actor: claude-code
  id: 01m05jkf8yzhm6v5bv8ca5np15
  text: |
    ## The RED I watched

    Wrote `..._declines_a_manifest_it_cannot_read` first, over the shipped script as
    it stood:

    ```
    thread 'review::tool_rules::tests::shipped::unused_dependencies::the_shipped_rust_unused_dependency_tool_rule_declines_a_manifest_it_cannot_read'
    panicked at crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1657:14:
    a script handed an item it cannot judge must judge the rest and exit 0: Exit("Analyzing dependencies of crates in unparsable/Cargo.toml...\nerror when handling unparsable/Cargo.toml: TOML parse error at line 6, column 14\n  |\n6 | [dependencies\n  |              ^\nunclosed table, expected `]`\n: TOML parse error at line 6, column 14\n  |\n6 | [dependencies\n  |              ^\nunclosed table, expected `]`\n\nDone!\nunused-dependencies-rust: cargo machete could not read unparsable/Cargo.toml")
    ```

    Then fixed the script and watched it pass.

    ## The fix

    The `exit 1` for an `error when handling ` line becomes a marked line and a
    `continue`:

    ```
    if grep -q '^error when handling ' "$work/machete.err"; then
      reason="$(grep '^error when handling ' "$work/machete.err" | head -1)"
      reason="${reason#error when handling }"
      printf 'sah-diagnostic: cargo machete could not read %s: %s\n' "$manifest" "${reason#*: }" >&2
      continue
    fi
    ```

    The raw `cat "$work/machete.err" >&2` goes with it, and machete's own reason
    moves INSIDE the marked line. At exit 0 the engine keeps marked lines and drops
    every other stderr line as tool chatter, so a raw dump would have reached no
    reader; machete writes `Analyzing…` and `Done!` on every run.

    The marker OPENS the line. `marked_diagnostics` in
    `crates/swissarmyhammer-validators/src/review/tool_rules.rs` keeps only lines
    that open with `TOOL_DIAGNOSTIC_MARKER`, so a line opening with the rule name
    would have been dropped as chatter.

    ## The harness

    `verify_unjudged_file_is_declined` — landed by `bb126a9fb` for `^s8d7fva` — is
    the right shape and took no change: the declined manifest is ordinary staged
    text that the TOOL alone cannot judge, not a path that refuses a reader. It
    derives the argument list from the staged paths, and `script_args` hands a
    `workspace`-scope rule the empty list whatever that list holds.

    `the_shipped_rust_unused_dependency_tool_rule_breaks_on_a_manifest_it_cannot_read`
    is gone with the behaviour it locked in, and `UNPARSABLE_MANIFEST_PROBE`,
    `MACHETE_UNREADABLE_LINE` and `MACHETE_UNREADABLE_ERROR` with it.
    `the_shipped_rust_unused_dependency_tool_rule_breaks_when_machete_cannot_run`
    stays: it is the control that stops a fix from answering every failure with a
    marked line.

    ## GREEN, and each way the tool can still fail

    Every row measured by extracting the shipped `run:` block and running it over a
    probe tree.

    | the run | stdout | stderr | exit |
    |---|---|---|---|
    | `Cargo.toml` + `unparsable/Cargo.toml` | the `serde` finding | 1 marked line naming `unparsable/Cargo.toml` | 0 |
    | the refusing manifest ALONE | nothing | the same 1 marked line | 0 |
    | a SECOND refusing manifest beside the first | the `serde` finding | 2 marked lines | 0 |
    | a renamed manifest whose `version.workspace = true` loses its root in the copy | the `serde` finding | 1 marked line, `can't load root workspace: …` | 0 |
    | `cargo-machete` stubbed on `PATH` to exit 127 | nothing | the stub's stderr, then `unused-dependencies-rust: cargo machete exited 127 over Cargo.toml` | **1** |
    | `cargo-machete` stubbed to exit 2 | nothing | the stub's stderr, then `… exited 2 over Cargo.toml` | **1** |

    The broken-run gate holds. A machete that answers a status outside its own two
    did no work at all, and it will do none for any other manifest either, so the
    whole run breaks and places no finding.

    ## Whole-workspace regression check

    Ran the OLD script and the NEW script over this repository and diffed the two
    stdouts: IDENTICAL, 122 findings across 36 packages, 63 package manifests, exit
    0, 1.3 s each. No manifest of this workspace declines. Whole-tree single machete
    run for the cost comparison: 0.3 s.

    The rule body's earlier numbers (126 across 37 packages, 2 s) were measured over
    an earlier state of this tree; two of the twelve hand-checked findings —
    `swissarmyhammer-validators`' `chrono` and `sha2` — are gone from the tree since,
    which is why the count moved. The script did not move it: the two shapes answer
    byte for byte alike.

    ## A stale count corrected beside the change

    The `BROKEN_RUN_EXIT_STATUS` doc comment in `shipped.rs` counts the breaking
    probes, and `bb126a9fb` left it one behind when it removed the
    `function-length-python` breaking probe. Counted the call sites: 35 probes across
    34 tests and 12 shipped rules after this card (9 `verify_shipped_tree_breaks`, 6
    `verify_shipped_tree_breaks_without_run_of`, 4 `verify_rust_function_length_breaks`
    = 19 held over 7 rules; 16 `verify_shipped_run_breaks` not held, over 9 rules).
    Corrected the whole sentence rather than only the one number my change moved.
  timestamp: 2026-08-16T15:21:19.902836+00:00
- actor: claude-code
  id: 01m05jky011tnv450rm21p017m
  text: |
    ## Sibling survey — do the cards already raised cover the rest?

    The six named cards cover every OTHER rule I met while working this one:

    - `^hqe8qwv` — missing-docs-python, three `exit 1` declines.
    - `^r73ddrr` — magic-numbers-python, a bare pipe.
    - `^nc2bcjr` — function-length-dart, four per-file declines.
    - `^jttzhnd` — missing-docs-go and stuttering-name-go, one revive record.
    - `^z69hh5a` — three rules with a pre-flight readability guard.
    - `^rfjsx87` — function-length-go and function-length-typescript read an
      unparsable file as clean.

    Two things I saw that NONE of the six names:

    1. **A doc-comment count in `shipped.rs` drifts with every probe added or
       removed, and nothing holds it.** `BROKEN_RUN_EXIT_STATUS` states 7 counts of
       the breaking probes, and `bb126a9fb` left every one of them a probe behind. I
       corrected them by counting the call sites, but the next card that adds or
       removes a breaking probe will silently break them again. There is no guard
       test. A card for a guard that counts the call sites and holds the doc to them
       would end this class.

    2. **`unused-dependencies-rust`'s own status gate still discards findings
       already written, for one manifest.** A machete status outside 0 and 1 exits
       the whole run, and the manifests already scanned had their findings on
       stdout. I kept it, this card says to keep the broken-run gate, and the
       argument in the rule body is sound — a machete that answers such a status did
       no work at all and will do none for the next manifest either, so measured
       with a stub the run correctly breaks with no finding placed. But it is the
       same SHAPE as the defect this card fixed, and it rests on an argument rather
       than on a measurement of a real machete that fails for one manifest only. I
       could not construct such a machete: the only measured status-2 shape is "a
       path that holds no file", which the script never hands it. Worth its own card
       only if someone finds a real per-manifest status-2.

    Nothing else. `dead-code-swift` and `magic-numbers-dart` exit nonzero for
    whole-run setup failures where nothing was judged, which the contract allows.
  timestamp: 2026-08-16T15:21:34.977511+00:00
- actor: claude-code
  id: 01m05jmbfkm8zqryhjd6a8tj9e
  text: |
    ### implement — changed

    - evidence: 3 files —
      `builtin/validators/manifests/rules/unused-dependencies-rust.md`,
      `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`,
      `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs`.

      MEASURED first (cargo-machete 0.9.2, the shipped `run:` block, a probe of
      `Cargo.toml` declaring an unused `serde` + `unparsable/Cargo.toml`): raw
      machete answers the `serde` finding at exit 1 for the first manifest and
      `error when handling …: TOML parse error` at exit 0 for the second. The
      shipped script wrote the `serde` finding to stdout, machete's 13 raw lines
      plus its own line to stderr, and exited 1 — the finding lost.

      RED, watched first: ``a script handed an item it cannot judge must judge the
      rest and exit 0: Exit("… unused-dependencies-rust: cargo machete could not
      read unparsable/Cargo.toml")``.

      GREEN, measured after: 1 finding on stdout, 1 marked line on stderr, exit 0.
      The refusing manifest alone: no finding, the same marked line, exit 0. Two
      refusing manifests: the finding, 2 marked lines, exit 0. Broken-run gate
      probed both ways — `cargo-machete` stubbed to exit 127 and stubbed to exit 2:
      no finding placed, the script's own status line, exit 1 each time.

      Whole-workspace regression: the old and the new script write byte-identical
      stdout — 122 findings across 36 packages, 63 package manifests, exit 0, 1.3 s.

      `cargo fmt` clean. `cargo clippy --workspace --all-targets -- -D warnings`
      clean, 0 warnings. `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'`
      — 3520 tests run, 3520 passed, 0 failed, 0 skipped. `cargo build -p mirdan`
      succeeds and the freshly generated
      `target/debug/build/mirdan-2dd4355981eab855/out/builtin_validators.rs` carries
      the corrected script verbatim: the `sah-diagnostic: cargo machete could not
      read %s: %s` printf with `continue`, and the broken-run gate
      `cargo machete exited %s over %s` with `exit 1` still beside it.

    - next: `/review`. The card is left in `doing`, nothing committed.
  timestamp: 2026-08-16T15:21:48.787834+00:00
- actor: claude-code
  id: 01m05k4ectby8252az1d0w9g1p
  text: |
    ### test — green

    **Gates**
    - `cargo fmt --check` — exit 0, no diff
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings (re-verified after touching the 2 changed test files to force a real recheck of `swissarmyhammer-validators`: `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` recompiled the crate and its test binaries, exit 0)
    - `cargo nextest run --workspace` — 14105 tests run: 14105 passed (26 slow), 0 failed, 0 skipped
    - `cargo nextest run -p swissarmyhammer-validators` (re-run after the forced recheck) — 763 tests run: 763 passed (19 slow), 0 failed, 0 skipped
    - `cargo build -p mirdan` — succeeds. Traced OUT_DIR: forced a fresh build (`touch crates/mirdan/build.rs && cargo build -p mirdan`), newest `target/debug/build/mirdan-*/out/builtin_validators.rs` (`mirdan-2dd4355981eab855`) contains, at the `manifests/rules/unused-dependencies-rust.md` entry, the corrected line `sah-diagnostic: cargo machete could not read %s: %s` followed by `continue` — the embed is current.

    **Behaviour checks (run, not read)**
    - Extracted the shipped script's `run:` block verbatim and ran it (with `cargo-machete` 0.9.2 on `PATH`) over a probe: a readable `Cargo.toml` (declares unused `serde`, uses only `libc`) sorting before `unparsable/Cargo.toml` (malformed `[dependencies` header).
      - stdout: `Cargo.toml:8: unused dependency \`serde\`: no source file of this package names it; delete it, or list it under \`[package.metadata.cargo-machete] ignored\` with a comment saying why`
      - stderr: `sah-diagnostic: cargo machete could not read unparsable/Cargo.toml: TOML parse error at line 6, column 14`
      - exit 0
      - Confirms the marker OPENS the stderr line (matches `marked_diagnostics` in `tool_rules.rs`, which does `line.trim().strip_prefix(TOOL_DIAGNOSTIC_MARKER)`), and the readable manifest's finding survives the declined one.
    - Broken-run gate, stub `cargo-machete` exiting 127: no finding on stdout, stderr carries the stub's own message plus `unused-dependencies-rust: cargo machete exited 127 over Cargo.toml`, exit 1.
    - Broken-run gate, stub `cargo-machete` exiting 2: no finding on stdout, stderr carries `Error: Errors when walking over directories` plus `unused-dependencies-rust: cargo machete exited 2 over Cargo.toml`, exit 1.
    - `cargo-machete --version` confirmed 0.9.2 (already installed, matches the doc's measured version).

    **Independent recount of the doc's numbers** (grepped every call site under `crates/swissarmyhammer-validators/src/review/tool_rules/tests/` rather than trusting the prose):
    - `verify_shipped_tree_breaks(` — 9 call sites (dead_code_rust.rs ×2, stuttering_name_go.rs ×1, missing_docs_rust.rs ×4, dead_code_typescript.rs ×2).
    - `verify_shipped_tree_breaks_without(` (wrapper) + direct `verify_shipped_tree_breaks_without_run_of(` calls — 4 wrapper call sites (dead_code_rust.rs, missing_docs_rust.rs, function_length_rust.rs, unused_dependencies.rs) + 2 direct calls (missing_docs.rs) = 6.
    - `verify_rust_function_length_breaks(` — 4 call sites, all in function_length_rust.rs.
    - Subtotal held to `BROKEN_RUN_EXIT_STATUS`: 9 + 6 + 4 = 19, matching the doc.
    - `verify_shipped_run_breaks(` — 16 call sites (dead_code_rust.rs ×1, missing_docs_rust.rs ×1, function_length_rust.rs ×1, function_length_swift.rs ×3, missing_docs.rs ×7, magic_numbers.rs ×3), matching the doc's 16 not held to the constant.
    - Total breaking-probe call sites: 19 + 16 = 35, matching the doc.
    - Distinct test functions: 35 call sites minus 1 (dead_code_typescript.rs's single test `the_shipped_typescript_dead_code_tool_rule_breaks_on_a_project_ts_prune_cannot_read` makes 2 `verify_shipped_tree_breaks` calls) = 34 tests, matching the doc.
    - Distinct shipped rules: dead-code-rust, stuttering-name-go, missing-docs-rust, dead-code-typescript, function-length-rust, unused-dependencies-rust, function-length-swift, magic-numbers-swift (8 files/rules) + missing_docs.rs covers 4 separate rules (`missing-docs-dart`, `missing-docs-go`, `missing-docs-python`, `missing-docs-swift`, confirmed via distinct `*_MISSING_DOCS_RULE` constants) = 12 rules, matching the doc.

    All doc counts (35 probes / 34 tests / 12 rules / 19 held via 9+6+4 / 16 across 9 rules) independently verified correct.

    No files touched beyond the three already-uncommitted ones; git status confirms only `builtin/validators/manifests/rules/unused-dependencies-rust.md`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, and `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs` are modified. Task left in `doing`, not committed.
  timestamp: 2026-08-16T15:30:36.058840+00:00
position_column: doing
position_ordinal: '8280'
title: unused-dependencies-rust writes findings to stdout, then exits 1 on a later manifest and discards them
---
`builtin/validators/manifests/rules/unused-dependencies-rust.md` prints its
findings to stdout INSIDE the manifest loop, then breaks the whole run on a
later manifest cargo-machete could not read:

    if grep -q '^error when handling ' "$work/machete.err"; then
      cat "$work/machete.err" >&2
      printf 'unused-dependencies-rust: cargo machete could not read %s\n' "$manifest" >&2
      exit 1
    fi

Every manifest already scanned had its findings written, and the nonzero exit
makes the engine read none of them. `builtin/validators/README.md`: "A nonzero
exit means the script broke — its stderr goes to the diagnosing agent, and no
findings are read... Do not exit nonzero for a declined item."

One manifest the tool could not handle is ONE item of a run that handled the
rest, so the answer is a line opening `sah-diagnostic:` at exit 0.

The work:

- Measure it: two manifests, one cargo-machete reads with an unused dependency
  and one it cannot handle. State the findings, stderr and exit of the shipped
  script.
- Replace the exit with a marked line at exit 0. The marker must OPEN the line.
- Add or rewrite the acceptance test so it stages a manifest with a real finding
  BEFORE the one that refuses, and holds the run to both halves: the finding
  survives AND one diagnostic names the refused manifest.
- State the measurement in the rule body.

Found while implementing `^s8d7fva`. #tool-validators #objectivity