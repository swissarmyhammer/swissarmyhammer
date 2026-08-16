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
- actor: claude-code
  id: 01m05m4n9xscaxespq6e324kat
  text: |
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (877ae089c) — engine 0 findings over 9 validators, 0 failed, 0 skipped; 2 hand-measured findings on added lines — builtin/validators/manifests/rules/unused-dependencies-rust.md:251, builtin/validators/manifests/rules/unused-dependencies-rust.md:44
    - measured by hand with cargo-machete 0.9.2: whole workspace stdout BYTE-IDENTICAL old vs new (`cmp` silent), 122 findings, 36 packages, 63 manifests, exit 0 both, 1.36 s; decline line opens `sah-diagnostic:` at byte 0 and `marked_diagnostics` (tool_rules.rs:943) is a strip_prefix; broken-run gate fires on stub 127, stub 2, and machete absent from PATH
    - BROKEN_RUN_EXIT_STATUS doc recounted call site by call site: 35 probes / 34 tests / 12 rules / 19 held (9+6+4) over 7 rules / 16 not held over 9 rules, union overlap 4 — every number holds, taxonomy closed
    - line accounting: 457 = 7+295+97+4+54, 82 = 44+4+34; 302 of 457 are this card's own kanban files
    - open finding 1: cargo-machete 0.9.2 DOES exit 2 per manifest for a single path's walk failure (nonexistent path, chmod 000 directory, broken symlink — all measured), so the added premise at :251 is false and the exit 1 it justifies discards findings already on stdout, measured end to end; untested, the machete-cannot-run probe stubs 127 only
    - open finding 2: `${reason#*: }` at :44 strips to the first `: `, not the path — a manifest at `a: b/Cargo.toml` leaves `b/Cargo.toml: ` inside the reason, so the prose at :275-276 states behaviour the expansion lacks
    - off-diff, recorded not raised: :234 and :177 state a `can't load root workspace at :` sentence machete never writes; :158 claims 63 files under both definitions where the measurement is 63 vs 64
    - next: fix both added-line findings, then re-review
  timestamp: 2026-08-16T15:48:11.709340+00:00
- actor: claude-code
  id: 01m05m6mrnkmgm3hdpbq14wwkz
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 3 files. Measured cargo-machete 0.9.2 first, watched RED, then fixed.
    - test: green — cargo fmt --check exit 0; cargo clippy --workspace --all-targets -- -D warnings exit 0, 0 warnings; cargo nextest run --workspace 14105 passed, 0 failed, 0 skipped; cargo build -p mirdan with the corrected script traced into the embed.
    - commit: 877ae089c fix(validators): keep findings when unused-dependencies-rust hits a bad manifest (^qvj3v4g) — 5 files changed, 457 insertions, 82 deletions
    - review: findings — 2 findings, both on lines this commit ADDED. builtin/validators/manifests/rules/unused-dependencies-rust.md:251 and :44. The engine answered 0 findings over 9 validators; both findings come from hand measurement.

    ### Finding 1 — the fix is HALF DONE, and the untested half still discards findings

    The commit added the premise "a machete that cannot do the work for one manifest cannot do it for any of them" to justify keeping `exit 1` for a status 2. **That premise is false.**

    `run_machete` bails `Errors when walking over directories` whenever `walkdir_errors` is non-empty, and those errors are PER INVOKED PATH. One machete process per manifest therefore makes a status 2 per manifest. The reviewer measured three constructions against the real 0.9.2 binary: a nonexistent path, a `chmod 000` parent directory, and a broken-symlink `Cargo.toml`.

    Driven end to end through the script's own loop: the `serde` finding stood on stdout and the run exited 1. `read_script_output` (tool_rules.rs:919) returns `Err` before stdout is parsed, so the finding reached no reader.

    That is EXACTLY the defect this commit fixed for the `error when handling ` shape, still live on the status-2 shape. Nothing tests it — the machete-cannot-run probe stubs 127 only.

    The implementer flagged this gap itself and said it could not construct a real per-manifest status-2. The reviewer constructed three. So the gap matters and the branch does discard findings.

    ### Finding 2 — the expansion does not do what the prose says

    `${reason#*: }` strips to the FIRST `: `, not to the manifest path. Measured with a package at `a: b/Cargo.toml`: the script wrote `sah-diagnostic: cargo machete could not read a: b/Cargo.toml: b/Cargo.toml: TOML parse error ...` — `a: ` came off and `b/Cargo.toml: ` stayed inside the reason. The prose at :275-276 claims "the prefix and the path taken off". Strip the known `$manifest` value rather than a positional `: `.

    ### Verified correct

    The decline line opens with `sah-diagnostic:` at byte 0, and `marked_diagnostics` is a `strip_prefix` on the trimmed line, so a prefix is what it needs. The broken-run gate fires on every way machete can FAIL — stub 127, stub 2, absent from PATH — each giving no finding, the named status line, exit 1. Whole workspace: `cmp` of old against new stdout is silent, so BYTE-IDENTICAL; 122 findings, 36 packages, 63 manifests, exit 0 both, 1.36 s. The `BROKEN_RUN_EXIT_STATUS` doc holds on every number this time, recounted call site by call site, and the taxonomy is closed with no probe left behind. Line accounting closes: 302 of the 457 insertions are this card's own board files.

    ### Recorded off-diff, not raised as findings

    Three further wrong claims sit on lines this commit did not write, so a diff-scoped review correctly declines them: `:234` and `:177` state a `can't load root workspace at :` sentence machete never emits, and `:158` claims both file definitions pick out 63 files where the measurement is 63 against 64.
    - next: iteration 2 works both open findings
  timestamp: 2026-08-16T15:49:16.693537+00:00
- actor: claude-code
  id: 01m05npxhs5m6czsdmweca0qzq
  text: |
    ## Both findings reproduced first, then fixed

    ### Finding 1 — reproduced with the REAL binary

    The reviewer's core claim holds. `run_machete` collects a walk failure for each
    path it was given and bails after the loop, so the failure is PER INVOKED PATH.
    Measured against cargo-machete 0.9.2, one process for each path:

    | the construction | exit | stderr |
    |---|---|---|
    | a path that holds no file | 2 | `Error: Errors when walking over directories:` then `znothere/Cargo.toml: IO error for operation on znothere/Cargo.toml: No such file or directory (os error 2)` |
    | a manifest inside a directory with mode 000 | 2 | the same two lines, `Permission denied (os error 13)` |
    | a `Cargo.toml` that is a broken symbolic link | 2 | the same two lines, `No such file or directory (os error 2)` |

    None of those three reaches machete through the script's own loop: `find` lists
    neither a path that holds no file nor anything inside a mode-000 directory, and
    `grep -q '^\[package\]'` fails on a broken symbolic link. So a FOURTH
    construction was needed, and it is the one the script really can meet — a macOS
    ACL that denies `readattr` while leaving the bytes readable:

        chmod +a "$(whoami) deny readattr" zwalkfail/Cargo.toml

    `find` lists it, `grep -q '^\[package\]'` exits 0, and `cargo-machete` exits 2
    with the walk sentence, because `open()` and `stat()` need different rights
    under that ACL.

    Driven end to end through the SHIPPED script's own loop, with a package
    declaring an unused `serde` sorting before it:

    - stdout: ``Cargo.toml:8: unused dependency `serde`: …``
    - stderr: machete's 4 raw lines, then
      `unused-dependencies-rust: cargo machete exited 2 over zwalkfail/Cargo.toml`
    - exit: **1** — and `read_script_output` answers `Err` before it reads stdout,
      so the finding reached no reader.

    The premise the last pass wrote is false, and the reviewer is right.

    ### Finding 2 — reproduced with the REAL binary

    Package staged at `a: b/Cargo.toml`, `[dependencies` header never closing.
    Machete wrote `error when handling a: b/Cargo.toml: TOML parse error at line 6,
    column 14`. The shipped script wrote:

        sah-diagnostic: cargo machete could not read a: b/Cargo.toml: b/Cargo.toml: TOML parse error at line 6, column 14

    `a: ` came off and `b/Cargo.toml: ` stayed inside the reason, exactly as stated.

    ## RED, watched before the fix

    Both probes were written first and both failed over the unchanged script:

    ```
    a script handed an item it cannot judge must judge the rest and exit 0: Exit("Analyzing dependencies of crates in zunwalkable/Cargo.toml...\nDone!\nError: Errors when walking over directories:\nzunwalkable/Cargo.toml: IO error for operation on zunwalkable/Cargo.toml: Permission denied (os error 13)\nunused-dependencies-rust: cargo machete exited 2 over zunwalkable/Cargo.toml")
    ```

    ```
    assertion `left == right` failed: the run must state the one item it declined, word for word
      left: ["cargo machete could not read a: b/Cargo.toml: b/Cargo.toml: TOML parse error at line 6, column 14"]
     right: ["cargo machete could not read a: b/Cargo.toml: TOML parse error at line 6, column 14"]
    ```

    The third new probe — the status-2 control — PASSED over the unchanged script,
    which is what a control must do.

    ## The fix

    A walk failure is read on machete's own two marks TOGETHER, and declines:

    ```
    if [ "$status" -eq 2 ] && grep -q '^Error: Errors when walking over directories' "$work/machete.err"; then
      reason="$(awk '/^Error: Errors when walking over directories/ {walked = 1; next}
                     walked {print; exit}' "$work/machete.err")"
      decline "$manifest" "${reason#"$scan": }"
      continue
    fi
    ```

    The broken-run gate stands under it, unchanged and untouched.

    The positional strip is gone from both declines. `${reason#"$scan": }` takes off
    the path the script HANDED machete. The `"$scan"` is quoted inside the pattern,
    so a path carrying a glob character is matched as the text it is — verified in
    `sh`, `bash`, `dash` and `zsh`, and with a path holding `[`.

    One `decline()` function carries the printf, so the two call sites do not copy
    it. `awk` and `head` are already named in `doctor.check_command`; no new binary.

    ## GREEN, measured with the real binary

    | the run | stdout | marked | exit |
    |---|---|---|---|
    | the ACL walk failure beside a good package | the `serde` finding | 1 line naming `zwalkfail/Cargo.toml` | 0 |
    | two walk failures alone | nothing | 2 lines | 0 |
    | `a: b/Cargo.toml` beside a good package | the `serde` finding | `could not read a: b/Cargo.toml: TOML parse error at line 6, column 14` | 0 |
    | the unparsable probe beside a good package | the `serde` finding | 1 line naming `unparsable/Cargo.toml` | 0 |
    | the unparsable manifest alone | nothing | 1 line | 0 |
    | two unparsable manifests beside a good package | the `serde` finding | 2 lines | 0 |
    | a renamed `version.workspace = true` copy | the `serde` finding | 1 line, `can't load root workspace: …` | 0 |

    ## The broken-run gate was NOT weakened

    Re-probed every way machete can fail, after the fix:

    | the run | stdout | marked | the script's line | exit |
    |---|---|---|---|---|
    | stub exits 127 | nothing | none | `cargo machete exited 127 over Cargo.toml` | **1** |
    | `cargo-machete` absent from `PATH` | nothing | none | `cargo machete exited 127 over Cargo.toml` | **1** |
    | stub exits 2 with NO walk sentence | nothing | none | `cargo machete exited 2 over Cargo.toml` | **1** |
    | stub exits 3 WITH the walk sentence | nothing | none | `cargo machete exited 3 over Cargo.toml` | **1** |

    The last row is the guard on the new reading: the sentence declines at machete's
    own failure status alone.

    ## Whole-workspace regression

    `cmp` of the old stdout against the new: silent. **BYTE-IDENTICAL.** 122
    findings, 36 distinct packages, exit 0 both, zero stderr lines both.

    Timing re-measured, three samples each: the per-manifest script 1.62 / 1.61 /
    1.71 s, one whole-tree machete run 0.40 / 0.39 / 0.39 s. The rule body said
    1.3 s and 0.3 s; corrected to 1.6 s and 0.4 s, from my runs.

    ## The three off-diff claims

    - `:158` — the reviewer is RIGHT. Counted: 68 `*.toml` files, 64 named
      `Cargo.toml`, 63 declaring `[package]`. The four under another name are
      `.config/nextest.toml`, `dist-workspace.toml`, `.cargo/config.toml` and
      `doc/book.toml`, none declaring a package; the one `Cargo.toml` that declares
      none is the virtual workspace root, which machete answers `didn't find any
      unused dependencies` at exit 0 for. Rewritten to state both counts.

    - `:234` and `:177` — the reviewer is WRONG, and my measurement says why. The
      ` at :` segment depends on whether the manifest declares its OWN `[workspace]`
      table. Measured, bare name `Cargo.toml`, `od -c` on both:

      - with `[workspace]`: `error when handling Cargo.toml: can't load root
        workspace at : No such file or directory (os error 2)…`
      - without it: `error when handling Cargo.toml: can't load root workspace: No
        such file or directory (os error 2)…`

      So the text that stood was right for the probe manifests, which all carry
      `[workspace]`, and wrong as a general claim. Rather than replace one wrong
      sentence with another, both places now state the plain sentence and a
      paragraph states the extra segment and what decides it. `./Cargo.toml` and
      `sub/Cargo.toml` never fail this way at all — measured 2×3.

    ## Why the new decline probe uses a stub

    Every construction that makes the REAL binary fail its walk also fails the
    script's own `grep -q '^\[package\]'` guard, except the macOS ACL, which is not
    portable. So the acceptance probe replays what the real binary answered —
    verbatim, path read off `$1` — while the ACL run above is the real-binary
    measurement recorded in the rule body. The control probe
    `..._breaks_when_machete_fails_over_no_walk` keeps the reading narrow.

    ## Harness change

    `verify_shipped_tree_breaks_without_run_of` hard-coded `exit 127` in its stub.
    Its stub-building half is now `lead_path_with_stub(binary, condition, answer)`
    plus `stubbed_run_condition(narrowed)`, and `verify_shipped_tree_breaks_with_stub`
    drives a probe with any answer. The 127 path routes through it unchanged, so all
    6 prior call sites keep their behaviour. `BROKEN_COMMAND_MARKER` is renamed
    `STUBBED_RUN_MARKER` (`.sah-stubbed-run`), because a decline probe stages it too
    and the marker names the RUN, not a broken command.

    `BROKEN_RUN_EXIT_STATUS` doc recounted call site by call site after the change:
    36 breaking probes across 35 tests and 12 rules; 20 held (`verify_shipped_tree_breaks`
    9, `verify_shipped_tree_breaks_with_stub` 7 — 6 of them through
    `verify_shipped_tree_breaks_without_run_of` — `verify_rust_function_length_breaks`
    4) over 7 rules; 16 not held over 9 rules.
  timestamp: 2026-08-16T16:15:38.553604+00:00
- actor: claude-code
  id: 01m05nqf4dppbq8ss7beerw4ab
  text: |
    ### implement — changed

    - evidence: 3 files —
      `builtin/validators/manifests/rules/unused-dependencies-rust.md`,
      `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`,
      `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs`.

      REPRODUCED both findings with cargo-machete 0.9.2 before editing. Finding 1:
      a macOS ACL denying `readattr` makes machete exit 2 with
      `Error: Errors when walking over directories:` while `find` and `grep` still
      read the file, so the script's own loop wrote the `serde` finding to stdout,
      wrote `unused-dependencies-rust: cargo machete exited 2 over
      zwalkfail/Cargo.toml`, and exited 1 — the finding lost. Finding 2: a package
      at `a: b/Cargo.toml` gave
      `sah-diagnostic: cargo machete could not read a: b/Cargo.toml: b/Cargo.toml:
      TOML parse error at line 6, column 14`.

      RED watched on both new probes over the unchanged script:
      ``a script handed an item it cannot judge must judge the rest and exit 0:
      Exit("… unused-dependencies-rust: cargo machete exited 2 over
      zunwalkable/Cargo.toml")`` and ``left: ["cargo machete could not read a:
      b/Cargo.toml: b/Cargo.toml: TOML parse error at line 6, column 14"] right:
      ["cargo machete could not read a: b/Cargo.toml: TOML parse error at line 6,
      column 14"]``.

      GREEN after the fix, real binary: the walk failure declines with the finding
      kept (1 marked line, exit 0), the colon path states the reason with the path
      taken off, and the broken-run gate still exits 1 with no finding placed for a
      stub at 127, machete absent from `PATH`, a stub at 2 without the walk
      sentence, and a stub at 3 with it.

      Whole-workspace regression: `cmp` of old against new stdout silent —
      BYTE-IDENTICAL, 122 findings across 36 packages, exit 0 both.

      Three new acceptance tests:
      `..._declines_a_manifest_it_cannot_walk`,
      `..._breaks_when_machete_fails_over_no_walk`,
      `..._states_the_reason_with_the_path_taken_off`.

      `cargo fmt --all --check` exit 0.
      `cargo clippy --workspace --all-targets -- -D warnings` exit 0, 0 warnings.
      `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3523 tests run,
      3523 passed (23 slow), 0 failed, 0 skipped.
      `cargo build -p mirdan` with `build.rs` touched to force a fresh run:
      `target/debug/build/mirdan-2dd4355981eab855/out/builtin_validators.rs` carries
      the corrected script verbatim — `decline() {` once, the
      `sah-diagnostic: cargo machete could not read %s: %s` printf once, the
      `$status -eq 2` walk branch, and `cargo machete exited %s over %s` with
      `exit 1` still beside it.

    - next: `/review`. The card stands in `doing` with both findings checked, and
      nothing is committed.
  timestamp: 2026-08-16T16:15:56.557919+00:00
- actor: claude-code
  id: 01m05pgntxyy66vt938bv46mw0
  text: |-
    ### test — green

    **Gates**
    - `cargo fmt --check` — exit 0, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
    - `cargo nextest run --workspace` — exit 0. 14108 tests run: 14108 passed, 0 failed, 0 skipped.
    - `cargo build -p mirdan` — succeeded. `OUT_DIR=target/debug/build/mirdan-2dd4355981eab855/out`. Confirmed `builtin_validators.rs` at that path embeds the corrected `unused-dependencies-rust.md` byte for byte: `decline()` helper, the two-mark walk gate (`status -eq 2` AND the walk sentence), and the `${reason#"$scan": }` strip.
    - `cargo-machete --version` — 0.9.2, on PATH (matches the doctor/install pin).

    **Targeted unit tests** — `cargo nextest run --test-threads=1 the_shipped_rust_unused_dependency_tool_rule` in `swissarmyhammer-validators`: 6/6 passed (`_reports_an_unused_dependency`, `_declines_a_manifest_it_cannot_read`, `_states_the_reason_with_the_path_taken_off`, `_declines_a_manifest_it_cannot_walk`, `_breaks_when_machete_fails_over_no_walk`, `_breaks_when_machete_cannot_run`).

    **Behaviour checks — verified by running the extracted script directly, not just trusting the unit tests**

    Broken-run gate, four ways, over a probe package with `serde` unused (script run from the probe's own directory):
    1. stub `cargo-machete` exits 127 → stdout empty, stderr `unused-dependencies-rust: cargo machete exited 127 over Cargo.toml`, exit 1.
    2. `cargo-machete` absent from PATH (PATH built from only the tools `check_command` lists, no cargo-machete) → `cargo-machete: command not found`, stderr `unused-dependencies-rust: cargo machete exited 127 over Cargo.toml`, exit 1.
    3. stub exits 2 WITHOUT the walk sentence (`Error: serde not found in tables:`) → stdout empty, stderr `unused-dependencies-rust: cargo machete exited 2 over Cargo.toml`, exit 1.
    4. stub exits 3 WITH the walk sentence (`Error: Errors when walking over directories:` + IO-error line) → stdout empty, stderr `unused-dependencies-rust: cargo machete exited 3 over Cargo.toml`, exit 1. Confirms the new guard: the sentence alone at a status other than 2 still breaks the run.

    All four: no finding placed, exit nonzero.

    Walk-failure decline-and-continue, reproduced with a macOS ACL (`chmod +a "$(whoami) deny readattr" zwalkfail/Cargo.toml`) over a two-package tree (root package with unused `serde`, `zwalkfail/` denied readattr): stdout kept the root's finding (`Cargo.toml:7: unused dependency \`serde\`...`), stderr wrote one marked line (`sah-diagnostic: cargo machete could not read zwalkfail/Cargo.toml: IO error for operation on zwalkfail/Cargo.toml: Permission denied (os error 13)`), exit 0.

    `a: b/Cargo.toml` case (unparsable TOML staged at that path): stderr `sah-diagnostic: cargo machete could not read a: b/Cargo.toml: TOML parse error at line 6, column 14`, exit 0 — no repeated path fragment.

    Whole-workspace regression: ran the shipped (new) script and the pre-walk-reading (old, `git show HEAD:...`) script both over this repo. stdout byte-identical (md5 `2fba4fbdf53af8dc0532cbc18fdc1980` both), 122 findings, 36 distinct package directories, exit 0 both.

    **`BROKEN_RUN_EXIT_STATUS` doc recount** (independent, by grepping every call site across `shipped.rs` + `shipped/*.rs`, not by trusting the doc text):
    - `verify_shipped_tree_breaks`: 9 call sites (missing_docs_rust.rs ×4, dead_code_typescript.rs ×2, stuttering_name_go.rs ×1, dead_code_rust.rs ×2).
    - `verify_shipped_tree_breaks_with_stub`: 7 tests reach it — 1 direct (unused_dependencies.rs:394), 2 via `verify_shipped_tree_breaks_without_run_of` directly (missing_docs.rs:542,568), 4 via `verify_shipped_tree_breaks_without` → `..._without_run_of` → `..._with_stub` (unused_dependencies.rs:448, function_length_rust.rs:853, missing_docs_rust.rs:712, dead_code_rust.rs:356). 6 of the 7 go through `..._without_run_of`, matching the doc.
    - `verify_rust_function_length_breaks`: 4 call sites (function_length_rust.rs ×4).
    - Held total: 9+7+4 = 20, over 7 rules (missing-docs-rust, dead-code-typescript, stuttering-name-go, dead-code-rust, unused-dependencies-rust, missing-docs-dart, function-length-rust).
    - `verify_shipped_run_breaks`: 16 call sites (function_length_swift.rs ×3, magic_numbers.rs ×3, missing_docs.rs ×7, function_length_rust.rs ×1, missing_docs_rust.rs ×1, dead_code_rust.rs ×1), over 9 rules (adds missing-docs-go, missing-docs-python, missing-docs-swift, function-length-swift, magic-numbers-swift to the 4 overlapping with the held group).
    - Grand total: 20+16 = 36 probe calls, over 12 distinct rules (7+5 new). One test (`the_shipped_typescript_dead_code_tool_rule_breaks_on_a_project_ts_prune_cannot_read`) calls `verify_shipped_tree_breaks` twice, giving 35 tests for 36 calls.
    - Doc's stated numbers — 36 total, 35 tests, 12 shipped rules, 20 held over 7 rules, 16 not held over 9 rules — all independently confirmed correct.

    Left in `doing`. Not committed.
  timestamp: 2026-08-16T16:29:42.621116+00:00
- actor: claude-code
  id: 01m05pjxsatga4r0mx4y4qvsjw
  text: |-
    ### commit — changed
    - evidence: 2d4198b5f fix(validators): decline on BOTH machete walk marks, strip the quoted path (^qvj3v4g) — 5 files changed, 846 insertions(+), 118 deletions(-)
    - next: none
  timestamp: 2026-08-16T16:30:56.298509+00:00
- actor: claude-code
  id: 01m05qe7nbzh262kz0bs0a0phz
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (2d4198b5f) — engine 0 findings / 0 confirmed / 0 refuted over 9 attempted validators, 0 failed. 1 hand-measured finding on changed lines: `builtin/validators/manifests/rules/unused-dependencies-rust.md:131`, `:207`, `:410` — the `1.6 s` and `0.4 s` timing claims this commit rewrote do not reproduce (measured 1.34-1.37 s over 5 samples, and 0.25-0.28 s over 11 samples across all three forms of a whole-tree run).
    - both prior findings verified FIXED by running: the ACL walk failure declines at exit 0 with the `serde` finding kept and 1 marked line; `a: b/Cargo.toml` answers `could not read a: b/Cargo.toml: TOML parse error at line 6, column 14` with no repeated path fragment.
    - two-mark gate probed all three ways: status 2 without the sentence breaks (exit 1), status 3 with the sentence breaks (exit 1), status 2 with the sentence declines (exit 0). `cargo-machete` absent from PATH breaks at 127.
    - `${reason#"$scan": }` proved load-bearing against a `[a]/Cargo.toml` path; unquoted repeats the path.
    - whole workspace BYTE-IDENTICAL old vs new, 122 findings, 36 packages, exit 0, md5 2fba4fbdf53af8dc0532cbc18fdc1980.
    - doc counts recounted call site by call site: 36 probes / 35 tests / 12 rules / 20 held over 7 rules / 16 not over 9 — every number holds. Line accounting 846/118 accounted in full.
    - DISPUTED INSTRUCTION ADJUDICATED: the implementer is RIGHT. With a `[workspace]` table machete writes `can't load root workspace at : ...`; without one it writes `can't load root workspace: ...`. The prior instruction was measured against a manifest without the table while the acceptance probes carry it. The refusal is correct and is not a finding.
    - next: restore the measured timing values at the four call sites, or state the machine and load the 1.6 s / 0.4 s samples came from. Task stays in `review`.
  timestamp: 2026-08-16T16:45:51.147971+00:00
- actor: claude-code
  id: 01m05qfvyp3x97er1mwhwj6pgf
  text: |
    ### finish iteration 2 — findings
    - implement: changed — 3 files. Reproduced both findings first, watched RED on the new probes, then fixed.
    - test: green — cargo fmt --check exit 0; cargo clippy --workspace --all-targets -- -D warnings exit 0, 0 warnings; cargo nextest run --workspace 14108 passed, 0 failed, 0 skipped; embed traced and confirmed byte for byte.
    - commit: 2d4198b5f fix(validators): decline on BOTH machete walk marks, strip the quoted path (^qvj3v4g) — 5 files changed, 846 insertions, 118 deletions
    - review: findings — 1 finding, builtin/validators/manifests/rules/unused-dependencies-rust.md:131, :207, :410.

    ### BOTH prior findings are fixed, verified by RUNNING

    The walk failure declines and continues: reproduced with the macOS ACL, the `serde` finding survives on stdout, one marked line names the refused manifest, exit 0. The old script over the same probe loses the finding at exit 1.

    The `a: b/Cargo.toml` case answers with no repeated path fragment.

    The two-mark gate is right in all three directions — status 2 without the sentence breaks, status 3 WITH the sentence breaks (the control holds), status 2 with the sentence declines. And the quoting is load-bearing: against `[a]/Cargo.toml` the shipped `${reason#"$scan": }` strips correctly, while the same script with the quotes removed repeats the path, because `[a]` is then read as a bracket expression.

    Whole-workspace stdout is byte-identical, md5 `2fba4fbdf53af8dc0532cbc18fdc1980`.

    ### The disputed instruction — the IMPLEMENTER was right

    Iteration 2's review told the implementer to delete a ` at :` segment as a sentence machete never writes. The implementer measured the opposite and declined to follow it. This round the reviewer measured it both ways itself:

    - WITH a `[workspace]` table the manifest declares: `can't load root workspace at : No such file or directory (os error 2): ...` — the segment IS written.
    - WITHOUT the table: no ` at :` segment.

    `UNUSED_DEPENDENCY_PACKAGE_MANIFEST` carries `[workspace]`, so the probes meet the ` at :` form. The earlier instruction was measured against a manifest without the table.

    Recorded plainly because it matters to how this loop works: **a correct refusal to follow a wrong instruction is not a finding.** The implementer measured, disagreed, said so on the card, and was right.

    ### The one open finding — two timing numbers moved the wrong way

    This commit rewrote `1.3 s` to `1.6 s` at three call sites and `0.3 s` to `0.4 s` at one. Measured: 1.36, 1.36, 1.36, 1.34, 1.37 over five warm samples, and 0.25-0.28 s over eleven samples of the single whole-tree run. **The pre-change values were the accurate pair.**

    Every other number the commit wrote is exact, which is what makes these two stand out in a document whose whole job is measured fact.

    ### Guardrail state

    Two rounds, findings both times, but never the same finding: iteration 1 raised `:251` and `:44` and both are now ANSWERED and verified; iteration 2 raises `:131`/`:207`/`:410`, different lines and different content. Not tripped. Iteration 3 is a narrow numeric correction.
    - next: iteration 3 corrects the four timing call sites to the measured values
  timestamp: 2026-08-16T16:46:44.694408+00:00
- actor: claude-code
  id: 01m05rcwrj73d3bfwgyvkjq0nw
  text: |
    ## I measured it myself first, and my numbers agree with the reviewer

    Machine: Apple M5 Max, macOS 27.0, cargo-machete 0.9.2. The script is the rule's
    own `run:` block, extracted verbatim (lines 12-64 of the rule file, dedented) and
    driven with `bash` at the workspace root.

    Background load moved a lot through the whole session. `corespotlightd` was
    indexing and spiked between 10 % and 860 % CPU; the 1-minute load average moved
    between 5 and 22. That is the whole story of this finding, so I recorded the load
    beside each batch.

    ### Batch A — 8 warm samples, load average 6.3 to 7.6

        1.54 1.38 1.35 1.34 1.35 1.48 1.47 1.49

    ### Batch B — 12 warm samples, load average rising to 19

    Script: `1.63 1.66 1.59 1.60 1.71 1.65 1.60 2.01 1.58 1.62` (10 of the 12 read
    back). Whole-tree run, 6 samples of each of the three forms:

    | the form | samples |
    |---|---|
    | `cargo-machete .` | 0.42 0.40 0.40 0.43 0.40 0.39 |
    | `cargo-machete` with no argument | 0.41 0.26 0.41 0.38 0.42 0.41 |
    | `cargo machete .` | 0.42 0.42 0.45 0.39 0.42 0.45 |

    ### Batch C — 20 pairs, run alternately, load average 13 to 22

    Script `1.56 1.64 1.86 1.83 1.61 1.72 1.81 1.56 1.56 1.63 1.58 1.48 1.50 1.56
    1.48 1.52 1.50 1.46 1.44 1.45`; whole-tree `0.43 0.41 0.40 0.39 0.40 0.40 0.41
    0.40 0.35 0.37 0.27 0.35 0.26 0.26 0.29 0.27 0.36 0.27 0.26 0.31`. Both fell
    steadily through the batch as Spotlight let go.

    ### Batch D — 30 pairs, run alternately, the quietest window I could get

    Script `1.32` to `1.49`, median **1.34 s**. Whole-tree `0.26` to `0.42`, median
    **0.27 s**. The first 23 pairs, taken before load came back, hold to 1.32-1.35 s
    and 0.26-0.29 s with no exception.

    ### What the samples say

    **The reviewer is right, and my numbers are its numbers.** The reviewer measured
    1.36 1.36 1.36 1.34 1.37 and 0.25-0.28; my quiet window gives 1.32-1.35 and
    0.26-0.29. The pre-change pair, 1.3 s and 0.3 s, is what this tree costs.

    **And the 1.6 s / 0.4 s pair is not invented — it is this same tree under load.**
    Batches B and C reproduce it exactly: 1.58-1.71 s for the script and 0.39-0.45 s
    for the whole-tree run, on the same binary, the same script and the same
    workspace, with Spotlight indexing beside it. The last pass measured a busy
    machine and wrote the reading down as a bare figure. That is the whole defect.

    ### What I wrote

    `1.3 s` at the table row and at the closing sentence, `1.3 s` and `0.3 s` in the
    cost paragraph — the values the tree measures. Then, because a bare figure is
    what let a loaded reading pass as fact, the cost paragraph now states the sample
    count, the range and the median, and says plainly that the tails belong to
    whatever else the machine was doing:

        Measured over 30 warm samples of each, run alternately so that both met the
        same machine: 1.32 s to 1.49 s for the script and 0.26 s to 0.42 s for the
        whole-tree run, median 1.34 s and 0.27 s. Both tails belong to whatever else
        the machine was doing, so read the medians.

    A reader who now measures 1.6 s can see it is the tail of a known range, not a
    contradiction of the document.

    Nothing else in the file was touched. The `run:` block is byte for byte what it
    was, and the diff is three timing sites plus the one paragraph rewrap they
    forced.
  timestamp: 2026-08-16T17:02:35.794426+00:00
- actor: claude-code
  id: 01m05rdb8c8c0dq7rde35yah9e
  text: |
    ### implement — changed

    - evidence: 1 file —
      `builtin/validators/manifests/rules/unused-dependencies-rust.md`.

      MEASURED FIRST, my own samples, before any edit. 30 warm pairs of the shipped
      script and the whole-tree run, taken alternately: script 1.32 s to 1.49 s,
      median 1.34 s; whole-tree 0.26 s to 0.42 s, median 0.27 s. The 23 pairs of the
      30 taken in the quiet window hold to 1.32-1.35 s and 0.26-0.29 s. Three
      earlier batches (8, 12 and 20 samples) reproduce the 1.6 s / 0.4 s pair, but
      only while `corespotlightd` was indexing at 300-860 % CPU and the load average
      stood at 13 to 22. My numbers agree with the reviewer's 1.36 / 0.25-0.28. The
      pre-change pair is the accurate one.

      WROTE: `1.3 s` at the table row, `1.3 s` and `0.3 s` in the cost paragraph,
      `1.3 s` at the closing sentence. The cost paragraph now also states the sample
      count, the range and the median, because a bare figure is what let a loaded
      reading pass as measured fact.

      The `run:` block is byte for byte unchanged. The diff is the three timing
      sites plus the rewrap of the one paragraph they touched.

      `cargo fmt --all --check` exit 0.
      `cargo clippy --workspace --all-targets -- -D warnings` exit 0, 0 warnings.
      `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3523 tests run,
      3523 passed (25 slow), 0 failed, 0 skipped.
      `cargo build -p mirdan` with `build.rs` touched to force a fresh run: exit 0,
      and `target/debug/build/mirdan-2dd4355981eab855/out/builtin_validators.rs`
      carries all three corrected sites inside the
      `manifests/rules/unused-dependencies-rust.md` entry, with no `1.6 s` or
      `0.4 s` left in it (the four remaining hits in that file belong to other
      rules).

    - next: `/review`. The card stands in `doing` with the finding checked, and
      nothing is committed.
  timestamp: 2026-08-16T17:02:50.636017+00:00
- actor: claude-code
  id: 01m05rvf2aw0yn38zn6rgdk7zw
  text: |-
    ### test — green

    **Scope check**: only tracked file changed is `builtin/validators/manifests/rules/unused-dependencies-rust.md` (`git status --porcelain` shows nothing else tracked; other `.kanban/*` entries are untracked files from concurrent board activity). `git diff` on the file shows 3 hunks, all in prose sections (the results table row, the "One machete process..." paragraph, the final "Measured over this whole workspace..." line). The `run: |` block (lines 11-64) has zero diff lines inside it.

    **Gates**:
    - `cargo fmt --check` — exit 0, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
    - `cargo nextest run --workspace` — 14108 tests run: 14108 passed (25 slow), 0 failed, 0 skipped. Exit 0.
    - `cargo build -p mirdan` — succeeds. Traced OUT_DIR: the freshest `mirdan-*/out/builtin_validators.rs` fingerprint dirs (rebuilt at 12:04, after the source file's 11:56 mtime, by the clippy/build pass above) embed the `unused-dependencies-rust.md` content byte-for-byte identical to the working-tree file (22,783 bytes, exact match) — zero occurrences of `1.6 s` or `0.4 s`. Two stale out-dirs from an earlier 11:32 build still hold the old `1.6 s`/`0.4 s` text, confirming the check is discriminating and not a false pass.

    **Own timing samples** (5 warm samples each, alternated, machine load noted): first checked `ps aux` for load — `corespotlightd` was at 108% CPU (elevated from the earlier bad round's 300-860%, but not idle) when the timing block started; by the time sampling finished, `corespotlightd` had dropped out of the top-CPU list entirely (indexing done). Samples:
    - extracted `run:` script over the workspace: 1.337, 1.338, 1.375, 1.360, 1.354 s → median 1.354 s, range 1.337-1.375 s. Falls inside the written range 1.32 s-1.49 s.
    - single whole-tree `cargo-machete .`: 0.278, 0.284, 0.291, 0.283, 0.322 s → median 0.284 s, range 0.278-0.322 s. Falls inside the written range 0.26 s-0.42 s.
    - Output content matched claims: 122 findings, 36 unique packages, 0 declined manifests, matching the doc's "no manifest declined" line.

    All figures in the file are supportable on this machine right now.
  timestamp: 2026-08-16T17:10:33.290405+00:00
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

## Review Findings (2026-08-16 10:47)

> Scope: `review sha HEAD~1..HEAD` (877ae089c) — reviewed the diffs only — lines this change added or modified. The validator fleet returned 0 findings over 9 attempted validators. The two items below come from hand measurement of the shipped script with cargo-machete 0.9.2, and both land on lines this commit added.

- [x] `builtin/validators/manifests/rules/unused-dependencies-rust.md:251` `hand/measurement` — the added premise "a machete that cannot do the work for one manifest cannot do it for any of them" is false, and the `exit 1` it justifies still discards findings. cargo-machete 0.9.2 exits 2 for a SINGLE path's walk failure: `run_machete` bails `Errors when walking over directories` after the loop whenever `walkdir_errors` is non-empty, and those errors are per invoked path, so one machete process per manifest makes a status 2 per manifest. Three constructions measured against the real binary: a nonexistent path, a manifest inside a directory with `chmod 000`, and a `Cargo.toml` that is a broken symlink. Driven end to end through the script's own loop with a second manifest returning that shape: stdout carried the `serde` finding, stderr carried `unused-dependencies-rust: cargo machete exited 2 over zlocked/Cargo.toml`, exit 1 — and `read_script_output` (`crates/swissarmyhammer-validators/src/review/tool_rules.rs:919`) returns `Err` before stdout is parsed, so the finding reached no reader. This is the exact defect the commit fixed for the `error when handling ` shape, still live on the status-2 shape. Nothing covers it: `the_shipped_rust_unused_dependency_tool_rule_breaks_when_machete_cannot_run` (`crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs:242`) stubs status 127 only. Separate a per-path walk failure from a machete that cannot run — decline the manifest and continue, as the `error when handling ` shape now does — or state and test why a status 2 must end the run, and add the probe.
- [x] `builtin/validators/manifests/rules/unused-dependencies-rust.md:44` `hand/measurement` — `${reason#*: }` strips to the FIRST `: ` in the string, not the manifest path, so a path containing `: ` leaves a path fragment inside the reason. Measured with a package staged at `a: b/Cargo.toml`: machete wrote `error when handling a: b/Cargo.toml: TOML parse error at line 6, column 14`, and the script wrote `sah-diagnostic: cargo machete could not read a: b/Cargo.toml: b/Cargo.toml: TOML parse error at line 6, column 14` — `a: ` was taken off and `b/Cargo.toml: ` stayed in the reason. The added prose at `builtin/validators/manifests/rules/unused-dependencies-rust.md:275-276`, "machete's own first `error when handling ` line with the prefix and the path taken off", states behaviour the expansion does not have. Strip the known `$manifest` value rather than a positional `: `, and correct the sentence to what the expansion does.

### Measured and CONFIRMED in this pass

All by hand, cargo-machete 0.9.2, `bash -c "$script" bash` at the workspace root, matching `run_shell`.

- The decline line OPENS with `sah-diagnostic:` — `od -c` shows the marker at byte 0, and `marked_diagnostics` (`crates/swissarmyhammer-validators/src/review/tool_rules.rs:943`) is a `strip_prefix` on the trimmed line, so a prefix is what it needs.
- The two-manifest probe: the `serde` finding on stdout, exactly 1 marked stderr line naming `unparsable/Cargo.toml`, exit 0. The old script over the same probe: same finding on stdout, 13 raw machete lines then its own line, exit 1 — the row's "13" is exact.
- The refusing manifest alone (no finding, 1 marked line, exit 0), a second refusing manifest beside the first (finding, 2 marked lines, exit 0), and the renamed `version.workspace = true` copy (finding, 1 marked line reading `can't load root workspace: ...`, exit 0).
- The broken-run gate still fires on every way machete can FAIL: stub exiting 127, stub exiting 2, and cargo-machete absent from PATH all give no finding, the named status line, exit 1. No way for machete to fail became a silent success.
- Whole workspace: `cmp` of old vs new stdout is silent — BYTE-IDENTICAL. 122 findings, 36 distinct packages, 63 `*.toml` files declaring `[package]`, exit 0 for both, zero stderr lines. Timing 1.36 s over three samples against the claimed 1.3 s.
- All five rows of the `CARGO_PKG_NAME` table, and rows 1-5 of the status/stderr table.
- `BROKEN_RUN_EXIT_STATUS` doc counts in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, recounted call site by call site: 35 breaking probes, 34 tests, 12 rules, 19 held (`verify_shipped_tree_breaks` 9, `verify_shipped_tree_breaks_without_run_of` 6, `verify_rust_function_length_breaks` 4) over 7 rules, 16 not held over 9 rules, union 12 with an overlap of 4. Every number holds. The taxonomy is closed — `assert_shipped_break` has exactly 3 call sites and only `verify_shipped_run_breaks` asserts a nonempty `errors()`, so no probe is left behind.
- Line accounting: 457 insertions = 7 + 295 (the card's own `.jsonl` and `.md`) + 97 + 4 + 54; 82 deletions = 44 + 4 + 34. 302 of the 457 are kanban bookkeeping, leaving 155/82 across the three content files.

### Measured WRONG but OFF-DIFF, so not findings in this pass

The `sha` op reviews added and modified lines only. These sit on lines this commit did not write, and are recorded so they are not lost.

- `builtin/validators/manifests/rules/unused-dependencies-rust.md:234` and `:177` — both state machete answers `error when handling Cargo.toml: can't load root workspace at :`. Measured: `error when handling Cargo.toml: can't load root workspace: No such file or directory (os error 2): No such file or directory (os error 2): No such file or directory (os error 2)`. There is no ` at :` segment; `cargo_toml` emits ` at {path}` only when the workspace error carries a path, and this one does not.
- `builtin/validators/manifests/rules/unused-dependencies-rust.md:158` — "the two definitions pick out the same 63 files here". Measured: 64 files named `Cargo.toml`, 63 declaring `[package]`. They differ by the virtual workspace root `./Cargo.toml`.
- The script tests machete's status and its `error when handling ` stderr line, never that stdout carried one of machete's two sentences. A stub exiting 0 or 1 with unparseable, partial, or empty stdout ends exit 0 with no finding and no marked line. Real machete 0.9.2 cannot reach this — both `Ok` branches of `run_machete` print one of the two sentences first — so it needs a shim, a wrapper, or a future machete.

## Review Findings (2026-08-16 11:43)

> Scope: `review sha HEAD~1..HEAD` (2d4198b5f) — reviewed the diffs only — lines this change added or modified. The validator fleet returned 0 findings over 9 attempted validators, 0 refuted. The item below comes from hand measurement against cargo-machete 0.9.2, and lands on lines this commit changed.

- [x] `builtin/validators/manifests/rules/unused-dependencies-rust.md:131` `hand/measurement` — this commit rewrote both timing numbers of the rule body AWAY from what the tree measures, in a document whose stated job is measured fact. `1.6 s` stands at three call sites — `:131`, `:207`, `:410` — each changed from `1.3 s`; measured over five warm samples of the shipped script across this workspace: 1.36, 1.36, 1.36, 1.34, 1.37, median 1.36 s. `0.4 s` for "a single whole-tree run" stands at `:207`, changed from `0.3 s`; measured 0.25-0.28 s over eleven samples spanning all three forms of that run — `cargo-machete .`, `cargo-machete` with no argument, and `cargo machete .` — so no reading of a whole-tree run reaches 0.4 s. The pre-change values were the accurate pair, and the prior review pass recorded the same 1.36 s. Every other number this commit wrote is exact. Restore the measured values at all four call sites, or state the machine and the load the 1.6 s and 0.4 s samples were taken under.

### The disputed instruction — the implementer is RIGHT, and this is NOT a finding

The prior pass instructed that `:234` and `:177` state a ` at :` segment machete never writes. The implementer measured the opposite and declined the instruction. Measured both ways here with cargo-machete 0.9.2, handing it the bare name `Cargo.toml`, using the acceptance probes' own manifest:

- WITH a `[workspace]` table: `error when handling Cargo.toml: can't load root workspace at : No such file or directory (os error 2): No such file or directory (os error 2): No such file or directory (os error 2)` — the ` at :` segment IS written, and the tail is `No such file or directory (os error 2)`, word for word what `:271` now states.
- WITHOUT the table, the same manifest otherwise: `error when handling Cargo.toml: can't load root workspace: No such file or directory (os error 2): ...` — no ` at :` segment.

`UNUSED_DEPENDENCY_PACKAGE_MANIFEST` carries `"\n[workspace]\n"`, so the acceptance probes meet the ` at :` form, exactly as `:273` states. The earlier instruction was measured against a manifest without the table. The refusal to follow it is correct, and the paragraph at `:268-273` is accurate as written. A correct refusal of a wrong instruction is not a finding.

### Measured and CONFIRMED in this pass

By hand, cargo-machete 0.9.2, the script extracted verbatim from the rule's `run:` block and driven with `bash`.

- **Prior finding 1 is fixed.** The walk failure declines and the run continues. Reproduced with the macOS ACL `chmod +a "$(whoami) deny readattr"`: machete exits 2 with `Error: Errors when walking over directories:` and `zwalkfail/Cargo.toml: IO error for operation on zwalkfail/Cargo.toml: Permission denied (os error 13)`. Through the script with a package declaring an unused `serde` staged before it: the `serde` finding on stdout, exactly 1 marked line, exit 0. The old script over the same probe: the finding, machete's 4 raw lines, then `unused-dependencies-rust: cargo machete exited 2 over zwalkfail/Cargo.toml`, exit 1 — the row's "4" is exact.
- **Prior finding 2 is fixed.** With a package staged at `a: b/Cargo.toml`, machete writes `error when handling a: b/Cargo.toml: TOML parse error at line 6, column 14` and the script answers `sah-diagnostic: cargo machete could not read a: b/Cargo.toml: TOML parse error at line 6, column 14`. The path fragment no longer repeats inside the reason.
- **The quoting is load-bearing.** Staged at `[a]/Cargo.toml`: the shipped `${reason#"$scan": }` answers `could not read [a]/Cargo.toml: TOML parse error at line 6, column 14`; the same script with only the quotes removed answers `could not read [a]/Cargo.toml: [a]/Cargo.toml: TOML parse error at line 6, column 14`, because `[a]` is then read as a bracket expression that never matches the literal `[`.
- **The two-mark gate is right in all three directions.** Status 2 WITHOUT the sentence: no finding, `unused-dependencies-rust: cargo machete exited 2 over Cargo.toml`, exit 1. Status 3 WITH the sentence: no finding, `cargo machete exited 3 over Cargo.toml`, exit 1 — the control holds. Status 2 WITH the sentence: 1 marked line, exit 0. `cargo-machete` absent from PATH: no finding, `exited 127 over Cargo.toml`, exit 1.
- **All four walk constructions** exit 2 and write the sentence, and only the ACL one reaches machete: `find` lists neither a path that holds no file nor anything inside a mode-000 directory, and `grep -q '^\[package\]'` fails on the broken symbolic link before the loop calls the tool. The three stderr tails are `No such file or directory (os error 2)`, `Permission denied (os error 13)`, `No such file or directory (os error 2)`, matching rows `:253-255`. Two walk failures staged alone: no finding, two marked lines, exit 0.
- **Whole workspace unchanged.** `cmp` of old against new stdout is silent — BYTE-IDENTICAL. 122 findings, 36 distinct packages, exit 0 for both, zero stderr lines, md5 `2fba4fbdf53af8dc0532cbc18fdc1980`, matching the commit message.
- **File counts exact**: 68 `*.toml` in the script's find scope, 64 named `Cargo.toml`, 63 declaring `[package]`, and the four under another name are exactly `.config/nextest.toml`, `dist-workspace.toml`, `.cargo/config.toml`, `doc/book.toml`.
- **Doc counts at `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1219-1230` recounted call site by call site, and every number holds**: 36 breaking probes across 35 tests — the one test driving two probes is `the_shipped_typescript_dead_code_tool_rule_breaks_on_a_project_ts_prune_cannot_read`. 20 held: 9 `verify_shipped_tree_breaks`, 7 reaching `verify_shipped_tree_breaks_with_stub` (1 direct, 2 through `verify_shipped_tree_breaks_without_run_of`, 4 through `verify_shipped_tree_breaks_without`, so 6 through `_without_run_of`), and 4 `verify_rust_function_length_breaks` — over 7 rules. 16 not held, all `verify_shipped_run_breaks`, over 9 rules. Union 12 rules, overlap 4.
- **The six acceptance tests** the table at `:398-403` names all exist, and `cargo nextest run -p swissarmyhammer-validators -E 'test(the_shipped_rust_unused_dependency) or test(every_shipped_unused_dependency)'` reports 7 tests run, 7 passed, 0 failed, 0 skipped of those selected.
- **Line accounting**: 846 insertions = 9 + 347 (the card's own `.jsonl` and `.md`) + 152 + 115 + 223; 118 deletions = 0 + 1 + 62 + 37 + 18. 356 of the 846 are kanban bookkeeping, leaving 490/117 across the three content files.