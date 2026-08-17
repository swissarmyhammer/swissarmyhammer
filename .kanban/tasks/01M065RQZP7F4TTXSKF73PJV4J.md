---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m0671tfyzq6j54pfn5k646rm
  text: |-
    ### research — measured on this machine

    **The keyspace is the 32-bit CRC alone.** `cksum` writes `<crc> <length>`, and `tr -dc '0-9'` glues the two together. Every probe path has the same byte length, so the length half is constant and the key is the CRC. Measured over 200000 tempdir-shaped paths (`$TMPDIR/.tmpXXXXXX`, and the same with `/repo` under it): 199996 distinct keys, 4 collisions — the birthday count over 2^32. The same 200000 paths under a sha256 key gave 200000 distinct keys.

    **CRC-32 is injective over a 4-byte window.** A first search that varied the first 4 characters of a 12-character name found NO collision in 4000000 candidates. A search that varies all 12 characters finds one in about 225000. `tempfile` varies 6 characters, which is past that window, so probe paths do collide at the birthday rate.

    **The collision reproduces the card's flake exactly.** Two working directories whose old keys are both `784381003151`:
    - OLD key: one cache directory. Workspace A (readable) filled it; workspace B, holding a file nobody may read, answered `exit=1 issues=1` — the WARM answer, where the acceptance test expects `exit=7 issues=0`.
    - NEW key (`shasum -a 256`): two cache directories. Workspace B answered `exit=7 issues=0`.
    The same pair also reproduces the wrong-path row of the rule body: under the OLD key the second workspace reported the FIRST workspace's `shapes/shapes.go`; under the NEW key each reported its own.

    **Why the cache stays — both reasons still hold.**
    1. The golangci-lint lock stands INSIDE the cache directory. `allow-serial-runners` makes a second instance wait on that lock, and this set ships two rules (`function-length-go`, `magic-numbers-go`) that drive golangci-lint over one workspace. Both must therefore name the SAME directory for one workspace, so the key stays a pure function of `$PWD` and the directory cannot be per-run.
    2. The measured "a file it cannot read" table needs a warm cache for row 3: a run whose cache already holds the answer still reports its findings and declines one item, where a cold run measures nothing at all.

    **So the pile-up is answered by a sweep, not by removal.** golangci-lint states its own lifetime: `internal/go/cache/cache.go` sets `trimLimit = 5 * 24 * time.Hour` and trims with `cutoff = now - trimLimit - mtimeInterval`. It trims INSIDE a cache directory it is given, and never removes a directory nobody names again — which is the pile-up. Measured `find -mtime +5`: it removed the 6-day, 7-day and 30-day directories and kept the 0, 1, 4, 5 and 5.5-day ones, so the cut is at six days, past golangci-lint's own limit. With 40 stale directories and another process removing 10 of them under the sweep, `find` still exited 0.

    **The standing set today.** 6609 directories, 422804 KiB, mean 64 KiB. Ages: 3703 under 1 day, 2846 of 1 to 2 days, 60 of 2 to 3 days, none older — so the rate is about 3300 a day and the platform, not the rule, is what bounds it.

    **Both go rules must change together**, or they stop sharing the one cache and the one lock the `allow-serial-runners` measurement rests on.
  timestamp: 2026-08-16T21:18:41.662059+00:00
- actor: claude-code
  id: 01m06970454sa90t35m1n23z3h
  text: |-
    ### the first sweep shape was slow, and the measurement caught it

    The first sweep this card wrote read the top of `TMPDIR` for `sah-golangci-lint-*`. Measured on this machine: `TMPDIR` holds 324623 entries and that `find` takes **2.58 s**, which stands in front of EVERY run of both Go rules. A bare `ls` of the same `TMPDIR` takes 1.83 s, so the flat shape cannot be made cheap — the cost IS reading the directory.

    That latency also broke the measured unreadable-file table. Driving the SHIPPED script three times over one workspace (unreadable/cold, readable, unreadable/warm):

    | the script | round 1 | round 2 | round 3 |
    |---|---|---|---|
    | before this card | 7 / 0 / 1 rows, deterministic over 3 rounds | same | same |
    | the flat sweep | scrambled | scrambled | — |
    | the flat sweep replaced by `sleep 2` | every row exit 7 | same | same |
    | the parent-directory sweep | 7 / 0 / 1 rows | same | same |

    The `sleep 2` row is the proof that the LATENCY was the cause and the sweep logic was not.

    **The answer**: the caches stand under `$TMPDIR/sah-golangci-lint/<digest>` rather than at the top of `TMPDIR`, and the sweep reads that parent with `-mindepth 1`.

    | the sweep | time |
    |---|---|
    | `TMPDIR`, caches named at the top of it | 2.58 s |
    | the parent directory, 7000 caches under it | 0.01 s |

    `-mindepth 1` is required: without it the parent itself matches `-type d -mtime +5`, and a parent nothing has touched for six days would take every cache under it.

    The parent also answers the count. After a whole workspace test run `TMPDIR` holds ONE entry for this set, and 56 caches stand under it, where the flat shape left 6609.
  timestamp: 2026-08-16T21:56:28.421594+00:00
- actor: claude-code
  id: 01m0697h2yq0wdb83z0sn5hmf1
  text: |-
    ### implement — changed

    - evidence: 5 files.
      - `builtin/validators/code-hygiene/rules/function-length-go.md` — the cache key becomes a sha-256 digest of `$PWD`, the caches move under `$TMPDIR/sah-golangci-lint/`, the script `touch`es its own cache and sweeps every sibling past golangci-lint's own `trimLimit`, `doctor.check_command` names `shasum mkdir touch find`, and the body states the reason the cache STAYS beside the sweep that bounds it.
      - `builtin/validators/code-hygiene/rules/magic-numbers-go.md` — the same script lines and the same `check_command`. The two rules must name one directory for one workspace, or they stop sharing the lock the `allow-serial-runners` measurement rests on.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs` — two acceptance tests, and `GoGeneratedPositions` extracted so the new test and the old one share one probe.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/golangci_cache.rs` — new set-wide coverage guard over both rules.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` — declares the module, and the head note now names four set-wide modules.

    **RED before the fix**, all five new tests, each for the right reason. The collision test failed with `left: ["/private/var/.../T/.tmpRJigdX/73hevoip9jcy/plain/staged.go"]` against `right: ["plain/staged.go"]` — the second workspace reported the FIRST one's path, which is exactly the harm.

    **GREEN after**: `cargo nextest run --workspace` — 14132 passed, 0 skipped, 0 leaky. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean. `cargo fmt --all -- --check` clean.

    **The cure, over the SHIPPED script of each version, on the colliding pair:**

    | the rule | cache directories | first workspace | second workspace |
    |---|---|---|---|
    | before | 1 | its own `plain/staged.go` | the FIRST one's `plain/staged.go` |
    | after | 2 | its own | its own |

    **Cold and warm still answer as the rule body says.** The shipped script, three rounds over one workspace: unreadable + cold cache → exit 7, no finding; readable → exit 0, one finding; unreadable + warm cache → exit 0, one finding and one diagnostic. Identical to the rule before this card, 3 of 3 rounds.

    **Swept**: 6633 accumulated `sah-golangci-lint-*` directories, 423696 KiB, removed by hand. `TMPDIR` now holds no flat cache of this set.
  timestamp: 2026-08-16T21:56:45.790488+00:00
- actor: claude-code
  id: 01m06fy9qhnztq0rq50rdvrmh3
  text: |-
    ### test — red

    **Standard tests pass.**
    - `cargo nextest run --workspace`, 2 runs. Each run: 14132 tests, 14132 pass, 0 fail, 0 skip.
    - `cargo nextest run -p swissarmyhammer-validators -E 'test(shipped::function_length_go::) + test(shipped::golangci_cache::) + test(shipped::magic_numbers::)'`, 20 sequential rounds. Each round: 42 tests, 42 pass, 0 fail.
    - Same filter, 10 rounds at `--test-threads=64`. Each round: 42 tests, 42 pass, 0 fail.
    - `cargo fmt --check`: clean, 0 diff.
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean, 0 warnings.

    **The digest fix removes the collision this card names.** A direct probe of the old `cksum` key found a colliding pair in 200000 tries. The new sha-256 digest key found 0 collisions in 200000 tries. 90+ real golangci-lint runs, staged with byte-identical content and driven in parallel up to 50-way (some runs paired with the actual 14132-test suite running at the same time), all keyed to unique digest directories, all reported the correct answer.

    **The sweep is safe.** Direct test of the sweep line against 5 shapes: a directory nothing touched for 30 days (removed, correct), a directory touched this second (kept), a plain file with an old mtime sitting beside the cache directories (kept, `-type d` excludes it), a directory outside the cache parent with an old mtime (kept, `find` never reads it), and an old file nested inside a fresh cache directory (kept, the sweep reads the top directory's own age, not what is inside it). 5 rounds of two real concurrent runs plus a third run's own sweep, launched at the same instant: both live caches survived every round, and the one deliberately staged 30-day-old directory was removed every round.

    **Timing matches the rule body.** One script run, cold: 0.148s. Same workspace, warm: 0.120s, 0.122s. On this machine `$TMPDIR` currently holds 255822 top-level entries; a bare `ls` of it takes 2.26s. The sweep, scoped to the one `sah-golangci-lint` parent, takes 0.005s. `$TMPDIR` before and after one script run: 255822 top-level entries both times — the run's own `mktemp -d` work directory is removed by its trap, and the cache write lands inside the one existing parent, not at the top of `$TMPDIR`. Exactly 1 top-level `$TMPDIR` entry matches `sah-golangci-lint*`.

    **Mirdan embeds both changed files byte-identically.** `cargo build -p mirdan`, then compared the `builtin_validators.rs` written to `OUT_DIR` against the files on disk: `function-length-go.md` (31615 bytes) and `magic-numbers-go.md` (10789 bytes) both match byte for byte.

    **But: the flake this card targets still shows under the exact load that first found it.** `cargo nextest run --workspace --test-threads=64`, 12 rounds. `the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read` failed in 4 of the 12 rounds — about the same rate the card reports for the OLD key ("once in 3 rounds"). Each failure carries the same shape: a fresh, uniquely-digested workspace's run reports the `funlen` finding AND a "declined an item" diagnostic, instead of breaking with "measured no function". That is the WARM-cache row, not the COLD-cache row this test stages.

    I ruled out a name collision: sha-256 over each failing workspace's own path is unique by construction, and the 200000-probe measurement above shows the digest has no collisions at this scale. I built a standalone driver that stages the exact shipped script over N byte-identical workspaces (same module name, same `shapes.go` content, same unreadable `noread` package) and runs them in parallel:
    - 50-way parallel, no other load: 0 leaks in 50.
    - 40-way parallel, run WHILE the real 14132-test workspace suite runs at `--test-threads=64` in the background: 1 leak per 40, in 3 of 3 rounds.

    So the leak needs genuine, whole-machine resource starvation to show — not merely many golangci-lint processes at once. An unrelated test, `swissarmyhammer-diagnostics::leader_watcher::watcher_redreport_on_direct_disk_write` (untouched by this diff, a real subprocess/LSP wall-clock test), failed in 12 of 12 of the same `--test-threads=64` full-workspace rounds, and passed in both plain `cargo nextest run --workspace` rounds. That confirms the machine enters a genuinely abnormal state under this specific stress condition.

    I did not change any file for this. I do not have a confirmed root cause — the leaking workspace's cache directory holds only its own content (checked directly), so this does not look like the workspace-name collision this card fixes. It looks like a golangci-lint-internal timing or concurrency behavior that only shows under extreme external CPU starvation. I recommend a new card to drive this to a root cause, scoped to: does golangci-lint's own internal package-load/cache logic have a race under scheduler starvation, separate from the `GOLANGCI_LINT_CACHE` directory naming this card already fixed.

    evidence:
    - `cargo nextest run --workspace` x2: 14132/14132 pass each time.
    - go/golangci filter x20 sequential + x10 at `--test-threads=64`: 42/42 pass every round.
    - `cargo nextest run --workspace --test-threads=64` x12: `watcher_redreport_on_direct_disk_write` fails 12/12 (unrelated, pre-existing); `the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read` fails 4/12, panic at `shipped.rs:1358:27`, `ScriptOutcome { findings: [.. LongProcedure ..], diagnostics: [.. permission denied ..] }`.
    - standalone driver, byte-identical content, unique digest dirs: 0/50 leak pure-parallel; 3/3 rounds leak 1/40 when combined with the real full-suite load.
    - sweep safety: 5-shape direct test + 5 rounds of live concurrent-run test, both clean every time.
    - timing: cold 0.148s, warm 0.120-0.122s; sweep 0.005s vs 2.26s bare `ls` of a 255822-entry `$TMPDIR`; 1 top-level `$TMPDIR` entry for the whole cache set.
    - mirdan embed: byte-identical, both files.
    - `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings`: both clean.

    next: a person decides whether the residual `--test-threads=64` flake blocks this card, or moves to a new card for root-causing golangci-lint's own behavior under extreme load.
  timestamp: 2026-08-16T23:54:03.377783+00:00
- actor: claude-code
  id: 01m06fzm32mxyjrxeekz9k0jnr
  text: |-
    ### finish iteration 1 — red
    - implement: changed — 5 files; the cache key becomes a sha-256 digest of $PWD, the caches move under one parent `$TMPDIR/sah-golangci-lint/`, and a `find ... -mtime +5` sweep at golangci-lint's own trimLimit answers the pile-up. 6633 accumulated directories swept by hand.
    - test: red — the card's own defect IS closed and proven (0 collisions in 200000 probes; the colliding pair the old key merged now gets two directories; the sweep keeps every live cache over 5 shapes and 5 concurrent rounds; the sweep costs 0.005 s against a TMPDIR of 255822 entries). `cargo nextest run --workspace` is green two times, 14132 passed. BUT under `--test-threads=64` (4 times the 18 CPUs) `the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read` fails 4 of 12 rounds, and reads a warm-shaped answer on a cold, uniquely named cache. A standalone driver shows 0 leaks over 50 pure-parallel runs and 1 leak in 40 when the real full suite runs beside it. The untouched `watcher_redreport_on_direct_disk_write` fails 12 of 12 under the same condition.
    - next: iteration 2 — root-cause the residual, or state what the test measures so it stops asserting a cold cache it cannot hold
  timestamp: 2026-08-16T23:54:46.754753+00:00
- actor: claude-code
  id: 01m06gpz612jegew66vsxes2pz
  text: |-
    ### iteration 2 research — the residual flake is a RACE inside golangci-lint, and the cache is not the variable

    **The cache is disproved by measurement.** I drove golangci-lint 2.12.2 over ONE probe workspace (the acceptance test's own shape: `go.mod`, `shapes/shapes.go` holding one function of 170 statements, `noread/unreadable.go` at mode 000) with a `GOLANGCI_LINT_CACHE` directory MADE EMPTY for every round and REMOVED after it, so a warm cache is impossible by construction. The tool still gave both answers:

    | `GOMAXPROCS` | rounds | reported the `funlen` row | reported nothing |
    |---|---|---|---|
    | 1 | 40 | 25 | 15 |
    | 2 | 40 | 0 | 40 |
    | 18 (this machine) | 40 | 0 | 40 |

    A cache empty by construction cannot make that split. The cache key this card changed is therefore not the cause.

    **What the leaking run wrote.** Every leaking round: `Issues` holds the one `funlen` row for `shapes/shapes.go:3`, stderr holds the one `level=error msg="[linters_context] typechecking error: open <repo>/noread/unreadable.go: permission denied"` line, and golangci-lint exits 1. Every other round: `Issues: []`, the same stderr line, exit 7. So the break condition `verify_staging_breaks` reads is the STATUS gate: the script breaks only while the status is neither 0 nor 1, and a leaking run exits 1.

    **The mechanism, in golangci-lint's own source.** `pkg/goanalysis/runner.go` gives every package of one run ONE context and a `loadSem` of `runtime.GOMAXPROCS(-1)` places. In `pkg/goanalysis/runner_loadingpackage.go` the action of the package nobody may read answers `analysis skipped: IllTypedError`; the `errgroup` hands that error up, and `analyze` calls `cancel()` on the shared context. A package that has not yet passed `select { case <-ctx.Done(): return; case loadSem <- struct{}{}: }`, or whose action has not yet passed the same test inside its `errgroup`, is dropped without a word. So WHICH packages a run measures is a race between the failing package and the sound ones. At `GOMAXPROCS=1` the semaphore holds one place, the two packages run one at a time, and the sound one wins about half the rounds. Four times the CPUs of this machine starves the tool's threads the same way, which is why `--test-threads=64` shows it.

    **The exit status follows.** golangci-lint sets `ErrorWasLogged` (7) only while the status is still `Success`. No row measured plus a logged error gives 7; the row measured gives `IssuesFound` (1) and the logged error never raises it.

    **Both answers are sound for this rule, measured through the SHIPPED script.**
    - default concurrency, 40 fresh workspaces: 40 broke with `golangci-lint exited 7 and measured no function`, 0 reported a row.
    - `GOMAXPROCS=1`, 24 fresh workspaces: 6 broke that same way, 18 reported the one `funlen` row at exit 0 with one `sah-diagnostic:` line naming the unreadable file. No run answered a third shape.

    **The WARM row IS a function of the cache.** One workspace linted with the file readable and then with the file unreadable, over the cache the first run filled: 20 rounds at `GOMAXPROCS=1` and 20 at `GOMAXPROCS=18`, all 40 reported the row at exit 1. Through the shipped script, 20 rounds: all 20 gave exit 0, the one row, and the one `sah-diagnostic:` line.

    **So the answer is (b).** `the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read` holds the run to ONE branch of that race, and no test can own goroutine scheduling. The rule body states the same wrong cause: the section "A file it cannot read, which the CACHE answers for" says the run's answer for the OTHER packages "depends on its cache", and the test doc says a probe over a fresh workspace always reads a cold cache and therefore always measures nothing.
  timestamp: 2026-08-17T00:07:31.777533+00:00
- actor: claude-code
  id: 01m06kag102v7h6hj9jddeb4ff
  text: |-
    ### implement (iteration 2) — changed

    The open finding is answered by (b): the test held the run to one branch of a race it cannot own. The cause is NOT in this card's change, and the measurement above proves it.

    **2 files.**

    - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs` — the probe of `the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read` now stages `go.mod` beside the unreadable package and NO second package, so nothing else is measurable and the answer is one shape. `GO_UNREADABLE_ERRORS`, `GO_UNREADABLE_REASON` and `GO_UNREADABLE_PATH` state that workspace. The doc of the test, of `GO_DECLINED_ANSWER` and of the declines test each drop the sentence that named the CACHE as the cause and state the measured race instead.
    - `builtin/validators/code-hygiene/rules/function-length-go.md` — the section "A file it cannot read, which the CACHE answers for" becomes "A file it cannot read, which costs the run the package it never read", and carries the `GOMAXPROCS` table, golangci-lint's own source for the cancel, the warm-cache row that IS deterministic, and the shipped-script split. The two rows of the status table and the `TMPDIR` bullet name the same measurement, and the closing paragraph states what each acceptance test now stages.

    `magic-numbers-go.md` needed no change: it carries no claim about a file the tool cannot read.

    **Nothing of iteration 1 is undone.** The sha-256 cache key, the one cache parent, the `touch` and the sweep all stand.

    **RED before, GREEN after, over the condition that reproduces the flake in 5 seconds instead of two hours.** `GOMAXPROCS=1 cargo nextest run -p swissarmyhammer-validators -E 'test(...breaks_on_a_file_it_may_not_read)'`:
    - the old probe: FAIL in 5 of 6 rounds.
    - the new probe: PASS in 10 of 10 rounds.

    **The measured failure rate at `--test-threads=64` after this change, 6 rounds of the whole workspace**: `the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read` failed 0 of 6, where iteration 1 measured 4 of 12. Every round ran 14132 tests and 14131 passed; the one failure of every round is
    `swissarmyhammer-diagnostics::leader_watcher::watcher_redreport_on_direct_disk_write`, which this diff does not touch and which card ^axr7bvb already holds.

    **The suite at the default concurrency**: `cargo nextest run --workspace` — 14132 run, 14132 passed, 0 skipped.

    `cargo fmt --all -- --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean.
    `cargo nextest run -p swissarmyhammer-validators -E 'test(shipped::function_length_go::) + test(shipped::golangci_cache::) + test(shipped::magic_numbers::)'` — 42 run, 42 passed.
    `cargo build -p mirdan`, then the generated `builtin_validators.rs` compared against the files on disk: `function-length-go.md` (33384 bytes) and `magic-numbers-go.md` (10789 bytes) both embedded byte for byte.

    ### implement — changed
    - evidence: 2 files — `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs` stages the unreadable package as the workspace's only package and states the race in every doc that named the cache; `builtin/validators/code-hygiene/rules/function-length-go.md` replaces the cache attribution with the measured race. `--test-threads=64` over 6 rounds: 0 of 6 for the target test, against 4 of 12 before. `cargo nextest run --workspace`: 14132 of 14132.
    - next: /review
  timestamp: 2026-08-17T00:53:08.768171+00:00
- actor: claude-code
  id: 01m06m9dcbwx8e1bqmytp52k9n
  text: |-
    ### test — red

    evidence:
    - `cargo fmt --check`: clean.
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean.
    - `cargo nextest run --workspace`, twice, default concurrency: round 1 = 14132 passed, 0 failed, 0 skipped; round 2 = 14132 passed, 0 failed, 0 skipped.
    - Reproducer `the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read` under `GOMAXPROCS=1`, 10 rounds: 10 of 10 PASS.
    - Old-shape reconstruction (a readable function beside the unreadable package, one probe repository holding two packages), same `verify_staging_breaks` assertion, 6 rounds under `GOMAXPROCS=1`: 4 PASS, 2 FAIL — non-deterministic, which is the race the rule body names. The new probe (unreadable package alone) is deterministic; the old probe shape is not. Reconstruction removed after the measurement; the test file is back to its pre-edit state (confirmed by re-diff and by a `grep` for the temporary test name, 0 matches).
    - `--test-threads=64`, twice, target test only: PASS both rounds.
    - Probe still measures what it claims: `the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read` calls `verify_staging_breaks`, which asserts the run breaks (`assert_shipped_break`), names every fragment of `GO_UNREADABLE_ERRORS` (the unreadable path, "golangci-lint exited", "measured no function"), and asserts `placed.is_empty()`. Not weakened.
    - mirdan embed: read `target/debug/build/mirdan-fe0ade5ce9f852e6/out/builtin_validators.rs`, extracted the `"code-hygiene/rules/function-length-go.md"` raw-string entry, compared byte-for-byte against the source file — 33384 bytes each side, identical.
    - golangci-lint 2.12.2 source (`~/go/pkg/mod/github.com/golangci/golangci-lint/v2@v2.12.2`) confirms: `pkg/goanalysis/runner.go` lines 259/262/266 — `gomaxprocs := runtime.GOMAXPROCS(-1)`, `loadSem := make(chan struct{}, gomaxprocs)`, `ctx, cancel := context.WithCancel(context.Background())`. `pkg/goanalysis/runner_loadingpackage.go` — `analyze(ctx, cancel, ...)` runs the `select { case <-ctx.Done(): ...; case loadSem <- struct{}{}: ... }`, and calls `cancel()` when `actsWg.Wait()` returns an error.

    **Finding — a claim wider than the measurement**, `builtin/validators/code-hygiene/rules/function-length-go.md`:

    > "In `pkg/goanalysis/runner_loadingpackage.go` the action of the package nobody may read answers `analysis skipped: IllTypedError`, the `errgroup` hands that error up, and `analyze` calls `cancel()` on the shared context."

    This sentence names one file, `pkg/goanalysis/runner_loadingpackage.go`, for all three things. Only two of the three stand there. The string `analysis skipped: IllTypedError` is produced by `func (act *action) analyze()`, defined in `pkg/goanalysis/runner_checker.go` (confirmed at that file, the line `return nil, fmt.Errorf("analysis skipped: %w", &pkgerrors.IllTypedError{Pkg: act.Package})`). `runner_loadingpackage.go` calls `act.analyzeSafe()` (defined in `runner_action.go`, which itself calls `act.analyze` in `runner_checker.go`) inside its `errgroup`, and its own `(lp *loadingPackage) analyze(...)` method is what hands the error up and calls `cancel()`. The rule names the wrong file for the part that answers `analysis skipped: IllTypedError`.

    Not fixed in this step — test does not own rule-body prose edits, and the file's established prose style is not ASD-STE100, so a same-step rewrite risks either breaking the document's established voice or violating the user's ASD-STE100 instruction for file writes. Needs an implement pass: correct the file attribution (the answer comes from `runner_checker.go`; the errgroup hand-off and `cancel()` call are what stand in `runner_loadingpackage.go`).

    next: implement — correct the misattributed source path in `function-length-go.md`, then re-run `/test`.
  timestamp: 2026-08-17T01:10:01.867811+00:00
- actor: claude-code
  id: 01m06majamafjr8472pc71hhnw
  text: |-
    ### finish iteration 2 — red
    - implement: changed — 2 files. The residual is ROOT-CAUSED and it is not the cache: golangci-lint 2.12.2 gives every package of one run ONE context and a `loadSem` of `GOMAXPROCS(-1)` places; the package nobody may read answers `analysis skipped: IllTypedError`, and `analyze` then cancels that shared context, so a package still waiting on the semaphore is dropped without a word. Over a cache made empty for every round, `GOMAXPROCS=1` reported the row 25 of 40 times and `GOMAXPROCS=18` 0 of 40. The probe now stages the unreadable package as the workspace's ONLY Go package, so one shape is the only answer. No assertion was removed.
    - test: red — one finding. Everything mechanical is clean: fmt, clippy, `cargo nextest run --workspace` 14132 passed two times, the reproducer 10 of 10 at `GOMAXPROCS=1`, the target test passing 2 of 2 at 64 threads, and the mirdan embed byte-identical at 33384 bytes. The tester also rebuilt the OLD probe shape and drove it 6 rounds at `GOMAXPROCS=1`: 4 passed and 2 failed, which proves the race is real and the new probe is the cure rather than a probe that stopped measuring. The finding: `builtin/validators/code-hygiene/rules/function-length-go.md` says `pkg/goanalysis/runner_loadingpackage.go` writes `analysis skipped: IllTypedError`. `func (act *action) analyze()` in `pkg/goanalysis/runner_checker.go` writes it. Only the errgroup hand-off and the `cancel()` call stand in `runner_loadingpackage.go`.
    - next: iteration 3 — correct that sentence to the two files it really names
  timestamp: 2026-08-17T01:10:39.700717+00:00
- actor: claude-code
  id: 01m06mgcbf4ezjtz8syvzkhpv8
  text: |-
    ### implement (iteration 3) — changed

    The one open finding is answered. I read both golangci-lint files myself before I wrote the sentence.

    **Measured in `~/go/pkg/mod/github.com/golangci/golangci-lint/v2@v2.12.2`:**

    | the step | the file that really holds it |
    |---|---|
    | `analysis skipped: IllTypedError` | `pkg/goanalysis/runner_checker.go:175`, inside `func (act *action) analyze()` (72..221). The line is `act.Result, act.Err = func() (any, error) { if act.Package.IllTyped { return nil, fmt.Errorf("analysis skipped: %w", &pkgerrors.IllTypedError{Pkg: act.Package}) } ... }` |
    | the `errgroup` hand-off | `pkg/goanalysis/runner_loadingpackage.go`, inside `func (lp *loadingPackage) analyze(...)`: `actsWg, ctxGroup := errgroup.WithContext(ctx)`, each goroutine calls `act.analyzeSafe()` and returns `act.Err` |
    | `cancel()` | the same `(lp *loadingPackage) analyze`: `err := actsWg.Wait(); if err != nil { cancel() }` |

    **1 file — `builtin/validators/code-hygiene/rules/function-length-go.md`.** The sentence now names each file for the step it really holds, and it names each `analyze` by its RECEIVER. That second part is necessary: each of the two files holds a method named `analyze`, so `analyze` alone is ambiguous once the passage names both files.

    **Every other source path, line and symbol of the same passage was checked the same way, and each one stands:**
    - `pkg/goanalysis/runner.go` — ONE context and a `loadSem` of `runtime.GOMAXPROCS(-1)` places: `gomaxprocs := runtime.GOMAXPROCS(-1)` (259), `loadSem := make(chan struct{}, gomaxprocs)` (262), `ctx, cancel := context.WithCancel(context.Background())` (266). Correct.
    - The quoted `select { case <-ctx.Done(): return; case loadSem <- struct{}{}: }` — it stands in `(lp *loadingPackage) analyze`, and the quote matches the source. Correct.
    - `saveIssuesToCache` "runs only for a run that met no error" — `runners.go` calls it under `defer func() { if len(errs) == 0 { saveIssuesToCache(...) } }()`. Correct.

    I checked the neighbouring sections as well, because a wrong claim usually sits beside others. All correct: `pkg/exitcodes` names 0 `Success` through 7 `ErrorWasLogged` in that order; `setupExitCode` in `pkg/commands/run.go` returns early at `if c.exitCode != exitcodes.Success` before it sets `ErrorWasLogged`, so "only while the status is still `Success`" is right; `invalid_issue.go` holds `if len(tcIssues) > 0 { return tcIssues, nil }`; `internal/go/cache/cache.go` sets `trimLimit = 5 * 24 * time.Hour` and `cutoff := now.Add(-trimLimit - mtimeInterval)`.

    **Nothing else changed.** No test file, no script, no other rule. `git diff --stat` outside `.kanban` names the 4 files of iterations 1 and 2, and my hunk is the only new one in them.

    **Verified:**
    - `cargo nextest run -p swissarmyhammer-validators -E 'test(shipped::function_length_go::) + test(shipped::golangci_cache::)'` — 14 run, 14 passed, 0 failed.
    - `cargo build -p mirdan` — finished, so the rule still embeds.

    ### implement — changed
    - evidence: 1 file — `builtin/validators/code-hygiene/rules/function-length-go.md`. The sentence now reads: "In `pkg/goanalysis/runner_checker.go` the method `(act *action) analyze` answers `analysis skipped: IllTypedError` for the action of the package nobody may read. In `pkg/goanalysis/runner_loadingpackage.go` the `errgroup` hands that error up, and the method `(lp *loadingPackage) analyze` calls `cancel()` on the shared context." Read in the golangci-lint 2.12.2 module: the string is at `runner_checker.go:175` inside `(act *action) analyze()`; the errgroup and the `cancel()` after `actsWg.Wait()` are in `(lp *loadingPackage) analyze` in `runner_loadingpackage.go`. 14 of 14 tests pass, mirdan builds.
    - next: /test
  timestamp: 2026-08-17T01:13:50.191727+00:00
- actor: claude-code
  id: 01m06mwn6kxfsnpy4gjb4b1j7r
  text: |
    ### test — green

    - evidence: I checked the corrected text in `builtin/validators/code-hygiene/rules/function-length-go.md` against golangci-lint v2.12.2 at `~/go/pkg/mod/github.com/golangci/golangci-lint/v2@v2.12.2`. I checked each claim myself.
      - `pkg/goanalysis/runner_checker.go`: the method `(act *action) analyze` returns `analysis skipped: IllTypedError` for the action of the package nobody may read. This is correct.
      - `pkg/goanalysis/runner_loadingpackage.go`: the method `(lp *loadingPackage) analyze` uses `errgroup.WithContext` to run each action, and it calls `cancel()` when an action returns an error. This is correct.
      - `pkg/goanalysis/runner.go`: line 259 sets `gomaxprocs`, line 262 makes `loadSem`, line 266 makes the shared context. These line numbers are correct.
      - The quoted `select` block is in `pkg/goanalysis/runner_loadingpackage.go`. This is correct.
      - `saveIssuesToCache` runs under `if len(errs) == 0` in `pkg/goanalysis/runners.go`. This is correct.
      - `pkg/exitcodes`: the eight named statuses (0 Success through 7 ErrorWasLogged) match the file exactly.
      - `setupExitCode` in `pkg/commands/run.go` sets a new status only when `c.exitCode == exitcodes.Success`. This is correct.
      - `invalid_issue.go` returns `tcIssues` alone when `len(tcIssues) > 0`. This is correct.
      - `trimLimit = 5 * 24 * time.Hour` is in `internal/go/cache/cache.go`. This is correct.
      - I found no wrong claim in the passage.
    - evidence: `cargo fmt --check` — clean, no output.
    - evidence: `cargo clippy --workspace --all-targets -- -D warnings` — clean, 0 warnings.
    - evidence: `cargo nextest run --workspace` (default concurrency, one run) — 14132 tests run, 14132 passed, 0 failed, 0 skipped.
    - evidence: I checked the mirdan build output at `target/debug/build/mirdan-66497afbb02f504c/out/builtin_validators.rs`. I pulled the embedded `code-hygiene/rules/function-length-go.md` entry and compared it byte for byte against the file on disk. Both hold 33537 bytes and match exactly. The byte count changed from 33384, as expected, because the prose changed.
    - next: none. The step is green.
  timestamp: 2026-08-17T01:20:32.467585+00:00
- actor: claude-code
  id: 01m06njhkne2nk2xeh2ycgh8cj
  text: |-
    ### review — findings
    - evidence: 3 findings, 3 confirmed — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs:839, :858, :900
    - next: correct the three `rust/error-handling` findings in the full file, then run the review again.
  timestamp: 2026-08-17T01:32:29.685817+00:00
- actor: claude-code
  id: 01m06nkakshp27p0vk7tjt4sy9
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — 1 file, 1 hunk. The passage said `runner_loadingpackage.go` writes `analysis skipped: IllTypedError`. `(act *action) analyze` in `runner_checker.go:175` writes it. The correction found a second problem in the same sentence: BOTH files hold a method named `analyze`, so each is now named by its receiver. Every other path, symbol and line the passage names was re-read against golangci-lint v2.12.2 and all were correct.
    - test: green — the corrected passage verified against the source again by the test step; fmt and clippy clean; `cargo nextest run --workspace` 14132 passed, 0 failed, 0 skipped; the mirdan embed matches the disk file at 33537 bytes.
    - commit: 1e9978eb6
    - review: findings — 3 confirmed of 9 validators attempted, all from `rust/error-handling` and all in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs`: line 839 (`.unwrap()` on the temporary directory), line 858 (`.expect()` on `run_script`), line 900 (`.expect()` on `create_dir_all`). The two rule documents and the new `golangci_cache.rs` guard drew none.
  timestamp: 2026-08-17T01:32:55.289206+00:00
- actor: claude-code
  id: 01m06p2xvcaxf36far6fnsndwa
  text: |
    ### implement (iteration 4) — changed

    The three `rust/error-handling` findings are closed. I read the rule first. `builtin/validators/rust/rules/error-handling.md` says: "Panics are for bugs only — internal invariant violations. Never panic on expected failure modes (bad input, missing files, network errors)."

    **The cause, removed from the WHOLE file.** The three lines the review named are three of EIGHT places in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs` that panicked on an expected failure. I removed all eight:

    | the line | what it panicked on | what it is now |
    |---|---|---|
    | `run.outcome` | `ScriptFailure` of the stubbed run | `?` |
    | `workspace_names_one_checksum_merged` | a search that met no pair | `Err(...)` |
    | `tempfile::tempdir()` | `std::io::Error` | `?` |
    | `run_script(...)` | `ScriptFailure` | `?` |
    | `duration_since(UNIX_EPOCH)` | `SystemTimeError` | `?` |
    | `create_dir_all(...)` | `std::io::Error` | `?` |
    | `File::open(...)` | `std::io::Error` | `?` |
    | `set_times(...)` | `std::io::Error` | `?` |

    `grep` over the file for `.unwrap()`, `.expect(` and `panic!(` now gives 0 matches.

    **The shapes.**
    - A new `ProbeResult<T> = Result<T, Box<dyn std::error::Error>>` alias, with a doc comment that states why the tests answer `Result`. The box keeps each failure's own `std::error::Error::source` reachable, which the same rule asks for ("`Error::source()` chains must exist for wrapped errors — don't flatten the chain"). A `map_err` to a string would have flattened it.
    - Three tests now answer `ProbeResult<()>`: `..._declines_a_file_it_may_not_read`, `..._reads_the_workspace_one_checksum_merged`, `..._sweeps_a_stale_cache_directory`.
    - `stage_cache_directory` answers `std::io::Result<PathBuf>`, which is the exact shape the finding at line 900 asks for.
    - `unique_cache_name` answers `Result<String, SystemTimeError>`.
    - `workspace_names_one_checksum_merged` answers `ProbeResult<(String, String)>`. The birthday search over a 32-bit checksum meets a pair long before the 1000000 limit, but no pigeonhole makes that certain, so exhaustion is an expected failure and not an invariant.

    **Why NOT `.expect()`, which is the shape of the sibling files.** The finding at line 839 accepts `.expect()`, but the findings at 858 and 900 name `.expect()` itself as the defect. `.expect()` therefore closes one finding of the three and leaves two open. The review scope was `review sha HEAD~1..HEAD`, so the `.expect()` lines of the sibling files were never read by the review. They are unreviewed, not approved.

    **The tests still MEASURE.** No assertion was removed, weakened or made conditional. I proved that a `Result` test fails on `Err` in THIS crate, with the two shapes this file carries: a bare `#[test]` and a `#[test] #[serial_test::serial(env)]`. Two temporary probes answering `Err("the temporary probe must fail")` both FAILED under `cargo nextest run -p swissarmyhammer-validators -E 'test(temporary_red_probe)'` — "2 tests run: 0 passed, 2 failed", each with `Error: "the temporary probe must fail"`. Both probes were then removed; `grep` for `temporary_red_probe` over `crates/` gives 0 matches.

    **The two sentences that `.expect()` carried are kept as comments beside the `?`**, because each states what the run must do rather than what the probe must do. The `?` now shows the script's own stderr, which is more than the sentence gave.

    **Verified.**
    - `cargo nextest run -p swissarmyhammer-validators` — 790 run, 790 passed, 0 failed, 0 skipped.
    - `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` — clean, 0 warnings.
    - `cargo fmt --check` — clean.
    - `git diff --stat` outside `.kanban` names ONE file.
  timestamp: 2026-08-17T01:41:26.508458+00:00
- actor: claude-code
  id: 01m06p35e2bqncxf8b0cjne8zy
  text: |
    ### implement — changed
    - evidence: 1 file — `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs`. 8 panics on expected failures removed (3 named by the review, 5 more of the same cause in the same file); each is now `?` or `Err(...)` through a new `ProbeResult` alias, `std::io::Result<PathBuf>` on `stage_cache_directory`, and `Result<String, SystemTimeError>` on `unique_cache_name`. 0 matches for `.unwrap()`, `.expect(` and `panic!(` remain. Two temporary `Err` probes proved a `Result` test still fails, plain and under `serial_test`. 790 of 790 tests pass; clippy and fmt clean.
    - next: /review
  timestamp: 2026-08-17T01:41:34.274768+00:00
- actor: claude-code
  id: 01m06ppjwpbh37ker4xpmm1rvg
  text: |
    ### test — green

    I checked the one changed file: `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs`.

    **I proved each RED-on-Err case myself.** I saved the exact original bytes first. Then, one at a time, I broke the real condition each test checks, and I ran `cargo nextest run` on that one test. Then I put the file back to the exact original bytes and checked the SHA-256 hash and a `diff` to prove the restore worked.

    1. `the_shipped_go_function_length_tool_rule_declines_a_file_it_may_not_read` — I broke the JSON text in `GO_DECLINED_ANSWER`. The script's own `jq` step could not read it. Result: FAILED, with `Error: Exit("jq: parse error... jq could not read the golangci-lint report")`. The `run.outcome?` line passed the error up. It did not pass quietly.
    2. `the_shipped_go_function_length_tool_rule_reads_the_workspace_one_checksum_merged` — I set `CANDIDATE_NAME_LIMIT` to 5 for a moment. The search could not find a matching pair. Result: FAILED, with `Error: "the search met no pair of names one checksum merges..."`. The `workspace_names_one_checksum_merged(...)?` line passed the error up.
    3. `the_shipped_go_function_length_tool_rule_sweeps_a_stale_cache_directory` — I put a NUL byte in the cache directory name for a moment. `create_dir_all` failed. Result: FAILED, with `Error: Error { kind: InvalidInput, message: "file name contained an unexpected NUL byte" }`. The `stage_cache_directory(...)?` line passed the error up.

    After each break, I put the file back. The final SHA-256 hash matched the saved original hash, and `diff` showed no change.

    **No check was dropped.** I read the full diff against `HEAD`. Every `assert_eq!` and `assert!` line stays the same. Only `.unwrap()`, `.expect()`, and `panic!()` calls changed to `?` or `Err(...)`.

    **Zero matches for banned calls.** `grep -n '\.unwrap()\|\.expect(\|panic!('` on the file: 0 matches.

    **Format and lint are clean.**
    - `cargo fmt --check`: 0 diff.
    - `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.

    **Full test run is clean.**
    - `cargo nextest run --workspace`, one run, default concurrency: 14132 tests run, 14132 passed, 0 failed, 0 skipped.

    next: none. The step is green.
  timestamp: 2026-08-17T01:52:10.646876+00:00
position_column: doing
position_ordinal: '8280'
title: function-length-go golangci-lint cache key collides after thousands of accumulated runs
---
`builtin/validators/code-hygiene/rules/function-length-go.md` names its golangci-lint result cache as `${TMPDIR:-/tmp}/sah-golangci-lint-$(printf '%s' "$PWD" | cksum | tr -dc '0-9')` and never removes it — the rule body states this on purpose ("the cache stays"). Measured on this machine: 6609 such directories have accumulated under `$TMPDIR`.

Every test probe builds its working directory from `tempfile::tempdir()` plus a fixed suffix, so probe `$PWD` strings are all the same byte length. `cksum` folds that constant byte count into its output alongside the CRC, so the byte-count half contributes zero entropy across probes — the real keyspace is only the 32-bit CRC. With 6609+ same-shaped accumulated names competing over a 2^32 space and growing every day the suite runs, a birthday collision becomes non-negligible and grows over time.

Reproduced: `cargo nextest run -p swissarmyhammer-validators -E 'test(shipped::function_length_go::)'` failed once in 20 sequential rounds, and `cargo nextest run --workspace --test-threads=64` failed once in 3 rounds — both times the same test, `the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read`, which expects a cold-cache result (`Issues: []`, exit 7) but instead got a stale warm-cache result (a finding present, exit 1) — the exact shape a hash collision with an older accumulated cache directory would produce, since `function_length_go.rs`'s fixtures reuse the same `go_procedure("LongProcedure", OVER_THE_GATE_STATEMENTS)` content across many tests in the file.

Found while probing an unrelated Go flake for #tool-validators card ^r73ddrr. Not caused by and not touched by that change — `function-length-go.md`'s `run:` block is untouched by that diff.

Work: give the cache key real entropy (hash the full path content, not a checksum with a constant-length blind spot, or add a nonce) so accumulated stale directories cannot collide, and/or prune old cache directories on a TTL so they stop accumulating without bound. #test-failure

## Review Findings (2026-08-16 20:21)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 3 file(s) reviewed, 2 not reviewed.

> 2 file(s) not reviewed — excluded by an ignore rule:
> - `.kanban/ (from .reviewignore)` — 2 file(s)

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs:839` `rust/error-handling` — `.unwrap()` panics on I/O errors when creating a temporary directory; I/O failures (permission denied, disk full) are expected failure modes, not bugs. Use `.expect("message")` at minimum, or change the test signature to `fn(...) -> Result<(), Box<dyn std::error::Error>>` to propagate the error. Either use `.expect("failed to create temp directory")` to provide context, or refactor the test to return `Result` so errors can be propagated without panicking.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs:858` `rust/error-handling` — `.expect()` panics when `run_script()` fails; script execution is an I/O operation that can fail on expected conditions (subprocess errors, I/O errors). The error-handling rule forbids panicking on expected failure modes. Refactor to return `Result` or handle the error explicitly without panicking.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs:900` `rust/error-handling` — `.expect()` panics when `create_dir_all` fails; filesystem operations (permission denied, no space) are expected failure modes, not bugs. The rule states: 'Panics are for bugs only — internal invariant violations. Never panic on expected failure modes (bad input, missing files, network errors).'. Refactor `stage_cache_directory` to return `Result<PathBuf, std::io::Error>`, or change callers to use a test signature that returns `Result`.
