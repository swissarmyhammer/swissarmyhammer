---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m00y99fs584dy0p4pm84shzw
  text: |-
    Second, independent reproduction of defect 2 (the shared cache), found while ^1h52223 changed this rule's gate. `builtin/validators/code-hygiene/rules/function-length-go.md` is untouched in the pipeline region this card owns; ^1h52223 changed the heredoc configuration alone.

    Measured with golangci-lint 2.12.2: one probe Go module copied to two directories, `cachea` and `cacheb`, each run with the shipped script's configuration. Both runs reported the paths of the ORIGINAL directory the cache was filled from, not their own. The engine then drops every finding, because no path places inside the workspace.

    There is a second symptom this card does not yet state: the generated-code carve-out fails OPEN under the same cause. `linters.exclusions.generated` reads the head of the file at the reported path, and a stale path names a file that is no longer there, so the filter lets the finding through. Measured over two files that hold the same function over the gate, one under `// Code generated ... DO NOT EDIT.`: on a cold cache the run reported the plain file alone, and on a warm cache it reported both. So a shared cache also produces a WRONG finding on generated code, not only a missing one.

    The three new acceptance tests in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs` work around this with `go_uncached`, which appends a wall-clock comment to each probe source so no run can take a cached answer. When this card lands the per-workspace `GOLANGCI_LINT_CACHE`, that helper can go away; its doc comment names this card.
  timestamp: 2026-08-14T20:09:17.049290+00:00
- actor: claude-code
  id: 01m00zk26x8fe85y1vsbqepwc0
  text: |-
    Research done. Every number measured with golangci-lint 2.12.2, staticcheck 2025.1.1, go 1.26.5 on darwin/arm64, over a probe module holding one 170-statement `LongProcedure` in a plain file and the same function in a file under `// Code generated ... DO NOT EDIT.`.

    THE LOCK, over the script as it ships today. Eight runs started together in ONE workspace reported `2 2 2 2 0 0 0 2`, `2 0 2 2 2 0 2 0` and `2 2 2 0 2 0 2 0` over three rounds — three zeros every round. The same script with stderr kept named the reason on each zero: `Error: parallel golangci-lint is running`. Four runs started together never clashed on a warm cache, so eight is the number the probe needs. With `run: allow-serial-runners: true` all eight reported the one finding, in 7.2 s from a cold cache.

    THE LOCK IS IN THE CACHE DIRECTORY. So the per-workspace cache alone silences the clash BETWEEN workspaces, and the clash INSIDE one workspace stays: `magic-numbers-go` and `function-length-go` name the same cache for one workspace and run together in one review. That is why the lock probe must drive one workspace, and why the two fixes are not one fix.

    THE SHARED CACHE. Two directories holding the same bytes under the same module name, one shared cache: the second run reported the FIRST directory's paths. With the first directory then REMOVED, the same run reported both files — the generated-code carve-out failed OPEN, because `linters.exclusions.generated` reads the head of the file at the reported path and a path that is no longer there lets the finding through. With a cache of its own each run reported its own path and dropped the generated file. This reproduces the symptom the previous comment recorded.

    STATICCHECK does NOT store an absolute path the same way. Measured over a module of 400 packages: the first run took 0.828 s, the same run again took 0.209 s — a cache hit — and the FIRST run over a copy of the same bytes at another path took 0.841 s, the cold number. The copy is a cache MISS, so no cached answer can carry a foreign path, and each of the three runs reported its own. Eight staticcheck runs started together in one workspace each reported all 400 findings, so it takes no lock either. `dead-code-go` therefore needs no change beyond stating the measurement.

    TMPDIR, with the cache added: the first run raises the count of entries by 2 before the trap, one for the cache and one for the configuration, and each run after it raises it by 1. After the trap the configuration is gone and the cache stays.
  timestamp: 2026-08-14T20:32:05.853890+00:00
- actor: claude-code
  id: 01m0105tar7jj0kt698v52sacd
  text: |-
    Correction to the lock numbers of the comment above. The first sweep ran over a workspace whose bytes were ALREADY in the shared cache, so the non-zero runs reported 2 rather than 1 — the stale-path defect, not the lock. The clean sweep drives one workspace and one cache, and counts the runs that reported nothing, three rounds from a cold cache and three from a warm one:

    | runs started together | runs that reported nothing |
    |---|---|
    | four, without the key | 0 of 4, in each of the six rounds |
    | eight, without the key | 3 of 8, in each of the six rounds |
    | eight, with the key | 0 of 8, in each of the six rounds |

    Four runs never clash, so the probe needs eight. That is the number the acceptance test takes, and the rule body and the test doc comment now state these numbers.

    RED, watched before the fix: all five Go function-length tests failed against the shipped bytes. The lock test reported `[[], [], ["shapes/shapes.go:3"], [], ["shapes/shapes.go:3"], ["shapes/shapes.go:3"], ["shapes/shapes.go:3"], []]` — four of eight judged nothing. The other four failed on the shared cache, which is why they had needed the `go_uncached` nonce.

    GREEN after the fix: 727 of 727 in `swissarmyhammer-validators`, twice. `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean. `go_uncached` is gone, and the three tests that used it now run on the shipped bytes alone.

    ### implement — changed
    - evidence: 5 files — builtin/validators/code-hygiene/rules/function-length-go.md, builtin/validators/code-hygiene/rules/dead-code-go.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_swift.rs
    - next: /review
  timestamp: 2026-08-14T20:42:20.376188+00:00
- actor: claude-code
  id: 01m010g6gk2fsyzpajew1mznr4
  text: |-
    ### review — clean
    - evidence: `review sha 32913e339~1..32913e339` — 0 findings, 0 confirmed, 0 refuted, 9 validator runs attempted, 0 failed, 0 skipped. Scope line reports "reviewed the diffs only", 3 files reviewed, 0 not reviewed.
    - next: none — task moved to done.
  timestamp: 2026-08-14T20:48:00.531183+00:00
- actor: claude-code
  id: 01m010hbd9m23nvpd2g30g6gye
  text: |-
    ### finish iteration 1 — clean. Card done.
    - implement: changed — 5 files. Both defects fixed on the shipped bytes, each held by an acceptance test watched RED first. The `lines: 10000, statements: 160` gate and the name-based test exclusion from ^1h52223 are untouched.
    - **The two fixes are COUPLED in a way neither card stated: the lock stands IN the cache directory.** So a per-workspace cache alone does not answer the lock, because two rules of this set drive golangci-lint over one workspace and share that directory. Both fixes are needed. Measured over one workspace, three cold rounds and three warm: without `allow-serial-runners`, 4 runs together lost nothing but 8 runs lost 3, in EVERY round; with the key, 8 runs lost none, in every round.
    - **The shared-cache failure is worse than a silent zero — the generated-code carve-out FAILS OPEN.** With the first directory removed, the second run reported the first directory's plain file AND its generated file, because `linters.exclusions.generated` reads the head of the file at the REPORTED path, which is not in this workspace. That produces a WRONG FINDING, not merely a missing one. Stated in the rule as a four-row table naming the fail-open as the stronger consequence. This symptom was found by ^1h52223's implementer and recorded here; the card did not originally state it.
    - **staticcheck does NOT share the defect**, measured rather than assumed: its cache key includes the workspace path. Over a module of 400 packages — first run 0.82s, same run again 0.21s (a hit), first run over a copy at another path 0.84s, the COLD number, so a copy is a cache MISS and no cached answer can carry a foreign path. Eight concurrent runs each reported all 400 findings, so it takes no lock either. Stated in `dead-code-go.md` — the renamed rule, since ^n8ptdxb renamed it this session and this card predates that.
    - The `go_uncached` nonce workaround ^1h52223 used in tests is DELETED; those three tests now run on the shipped bytes alone. Two new acceptance tests: one drives the generated-code probe over two workspaces, one releases eight runs together over one workspace. The shared `finding_rows` helper and `NO_SCRIPT_FILES` moved up to `shipped.rs` rather than being copied a third time.
    - test: green — 727 validators tests, twice. fmt and clippy clean.
    - commit: 32913e339
    - review: clean — 0 findings, 9 attempted, 0 failed, 0 skipped, 3 files reviewed, 0 not reviewed.
  timestamp: 2026-08-14T20:48:38.313486+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff8480
title: function-length-go silently reports nothing on a lock clash or a shared golangci-lint cache
---
`builtin/validators/code-hygiene/rules/function-length-go.md` runs `funlen` through `golangci-lint` with the same script shape `magic-numbers-go` had before ^s2ftjys. Two defects were measured on that shape, and both make the rule report ZERO findings and name no reason:

1. **The lock.** `golangci-lint` takes one file lock for each run. A second instance stops with `Error: parallel golangci-lint is running` on stderr and writes nothing to stdout. The script sends stderr to `/dev/null`, so the review reads a clean file. Measured: eight runs of the `magic-numbers-go` script started together reported `6, 6, 6, 0, 6, 0, 6, 0`.
   Fix: `run: allow-serial-runners: true` in the written config. With the key, all eight reported `6`.

2. **The shared cache.** `golangci-lint` answers by package content and stores each finding with the ABSOLUTE path of the run that first cached it. A second workspace that holds the same bytes under the same module name gets the FIRST workspace's paths back, and the engine drops a finding it cannot place in the workspace. Measured: a run in directory B reported `/private/var/.../T/.tmpuFxLbQ/src/magic_numbers_go_fail.go`, a path outside itself. Two checkouts of one repository are the everyday form of this, and a review runs in a worktree.
   Fix: name a cache directory for the workspace —
   `cache="${TMPDIR:-/tmp}/sah-golangci-lint-$(printf '%s' "$PWD" | cksum | tr -dc '0-9')"` and run with `GOLANGCI_LINT_CACHE="$cache"`. With a cache of its own the same run reported its own path.

`^s2ftjys` made both changes in `magic-numbers-go.md` and states each with its measurement. Copy the two lines and the two paragraphs, and measure both again for `funlen`.

Also measure `unused-code-go`, which runs `staticcheck` at workspace scope. `staticcheck` keeps its own cache; find out whether it stores absolute paths the same way, and state the answer in that rule.

Found while ^s2ftjys added an end-to-end acceptance test for `magic-numbers-go`. The test failed for both reasons, and it passes only after both fixes. #tool-validators #objectivity