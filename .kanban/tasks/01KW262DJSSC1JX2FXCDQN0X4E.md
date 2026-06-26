---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw2r1awg12mjs3c6hxvgc90a
  text: |-
    Implemented via /implement (TDD).

    Files:
    - src/surface/mod.rs (new): `SurfaceAdapter` trait — provision/drive/observe/teardown, with associated-type `ProvisionedSut` so http/db/file adapters slot in with their own handle type.
    - src/surface/cli.rs (new): `CliAdapter` + `CliSut` + `CliCommands`. drive = run argv via std::process; observe = stdout/stderr/exit + named output files into CliState; setup overrides detected (last setup command = launch, earlier = build), absent setup falls back to expect's own exhaustive ProjectType→{build,launch} map. Honors timeout.
    - src/error.rs: added `Surface(String)` + `Timeout { timeout_ms }` variants.
    - src/lib.rs: `pub mod surface;` + re-exports.
    - Cargo.toml: nix (unix-only) for process-group kill.

    Tests: 60 pass (`cargo nextest run -p swissarmyhammer-expect`), 3 doctests pass, clippy --all-targets -D warnings clean, fmt applied. RED watched first for every behavior.

    double-check verdict was REVISE. Resolution:
    - Finding 1 (HIGH, FIXED): timeout path could hang forever — `join_drain` blocks on a grandchild that inherited the pipe (the default `cargo run --` case). Added a RED regression test (child shell spawns `sleep` grandchild holding the pipe) that hung ~8s before the fix; fix spawns the child in its own process group and `killpg`s the whole group on timeout, and no longer joins the drain threads on the abort path (output is discarded there anyway). Test now returns in ~0.08s.
    - Finding 2 (MEDIUM, deferred w/ justification): cli builds in repo_root in place and teardown is a documented no-op (cli owns no scratch). Full Isolation::Fresh / scratch instancing is out of scope for the first adapter; the trait already supports it via per-adapter SUT handles. mod.rs/teardown docs state this. "cleans scratch" AC is vacuously satisfied (cli provisions none).
    - Finding 3 (LOW, deferred): detected (fallback) path is covered at resolution level; a full E2E detected build+launch needs a real toolchain (make/cargo) — left to integration scope.
    - Finding 4 (LOW, deferred): `tokenize` is whitespace-split (no shell quoting); documented limitation.

    Left in `doing`, green, ready for /review.
  timestamp: 2026-06-26T19:55:46.448172+00:00
- actor: claude-code
  id: 01kw2rzfryn1qa6y6bm7f58t97
  text: |-
    Resolved all review findings in crates/swissarmyhammer-expect/src/surface/cli.rs:

    - BLOCKER (path traversal): added `safe_output_path(work_dir, name)` which rejects absolute paths and any `..` (ParentDir) component before joining, returning ExpectError::Surface; `observe` now routes every output-file name through it. Regression tests: `safe_output_path_rejects_traversal_and_accepts_plain_names` (unit: accepts plain/nested, rejects `../../etc/passwd`, mid-path `..`, and absolute) plus `observe_rejects_output_file_path_traversal` (through the public observe path with a `../escape.txt` capture).
    - WARNING (11-arm match): replaced the ProjectType match with a static `DETECTED_COMMANDS: &[DetectedRow]` table; `detected_commands` is now a single `.find()` lookup. Added `detected_commands_table_covers_every_project_type` guard test (one row per variant, no dups) to recover the exhaustiveness the match gave for free.
    - NITS (magic 50ms): extracted `const TEST_TIMEOUT_SHORT: Duration = Duration::from_millis(50)`, used in both timeout tests.

    Verification: cargo nextest -p swissarmyhammer-expect = 63 passed/0 failed; cargo test --doc = 3 passed; cargo fmt applied; cargo clippy --all-targets -D warnings clean (also factored the table tuple into a `DetectedRow` type alias to satisfy clippy::type_complexity).
  timestamp: 2026-06-26T20:12:14.494043+00:00
depends_on:
- 01KW25YZ4MKNR09RXYR1B4S05T
- 01KW25ZW4NED0J1BD77HPK7DNX
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffea80
project: expect
title: 'cli surface adapter: provision + drive + observe + teardown'
---
## What
The first surface adapter — `cli` — the deterministic, no-agent path. A `SurfaceAdapter` trait + a `CliAdapter` impl that provisions the SUT, drives it mechanically, and observes authoritative state. Per `ideas/expect.md` §"Surface adapters" (cli row) and §"Provisioning and Isolation".

- New `crates/swissarmyhammer-expect/src/surface/mod.rs` defining the `SurfaceAdapter` trait:
  - `provision(&self, setup, repo_root) -> ProvisionedSut` (build + ready the binary; uses `setup:` or falls back to detected build/launch).
  - `drive(&self, sut, when_step) -> ()` (cause the transition).
  - `observe(&self, sut) -> SurfaceState` (authoritative read).
  - `teardown(&self, sut)`.
- New `crates/swissarmyhammer-expect/src/surface/cli.rs` — `CliAdapter`: build via `detected-projects` (`crates/swissarmyhammer-project-detection`) → `ProjectType`, with expect's own `ProjectType → {build, launch}` command map (the structured commands don't exist in project-detection; mirror the consumer pattern in `code_context/detect.rs:229`). Drive = run argv (`std::process`); observe = capture stdout/stderr/exit code + named output files into a cli `SurfaceState`. Honor the spec `timeout`.
- Provisioning lifecycle owned by expect (build now, not "whatever's running").

## Acceptance Criteria
- [ ] Against a trivial fixture CLI (e.g. a tiny `echo`-like script or a built binary), `CliAdapter` provisions, runs an argv, and observes stdout/stderr/exit in a `SurfaceState`.
- [ ] `setup:` declaration overrides auto-detected build/launch; absent `setup:` falls back to detected commands.
- [ ] Teardown cleans up any provisioned scratch state.
- [ ] A run exceeding `timeout` is aborted and surfaced as an error (not a hang).

## Tests
- [ ] `crates/swissarmyhammer-expect/src/surface/cli.rs` integration test driving a real fixture command in a `tempfile` dir, asserting captured stdout/exit.
- [ ] Timeout test: a sleep-longer-than-timeout command returns a timeout error.
- [ ] `cargo nextest run -p swissarmyhammer-expect cli_adapter` passes.

## Workflow
- Use `/tdd`.

## Review Findings (2026-06-26 14:56)

### Blockers
- [x] `crates/swissarmyhammer-expect/src/surface/cli.rs:316` — Path traversal via unvalidated file names in output_files. A malicious or misconfigured spec could specify output file names like `../../etc/passwd` to read arbitrary files outside the work_dir. Validate that each file name is a simple filename without `..`, `/`, or other path traversal sequences. Use either: (1) `Path::file_name()` to extract only the basename, (2) explicit checks rejecting `..` and absolute paths, or (3) `std::path::Path::canonicalize()` to normalize and verify the resolved path remains within work_dir.

### Warnings
- [x] `crates/swissarmyhammer-expect/src/surface/cli.rs:122` — Match expression with 11 arms over ProjectType (Rust, NodeJs, Python, Go, JavaMaven, JavaGradle, CSharp, CMake, Makefile, Flutter, Php), where all arms differ only in constant command lists, should be expressed as a static data table instead of parallel code paths. The function's own documentation refers to it as a 'map'; expressing it as actual data eliminates the maintenance burden of keeping all arms in lockstep when new ProjectTypes are added. Convert to a static lookup table (HashMap, BTreeMap, or phf map) that maps ProjectType → (build_commands, launch_command), then simplify `detected_commands()` to a single table lookup instead of a 11-way match.

### Nits
- [x] `crates/swissarmyhammer-expect/src/surface/cli.rs:413` — Hardcoded timeout value `50` (milliseconds) configures test behavior and should be a named constant, not a magic number. Extract to a test-level constant like `const TEST_TIMEOUT_SHORT: Duration = Duration::from_millis(50);` and reuse it in both tests that need this value.
- [x] `crates/swissarmyhammer-expect/src/surface/cli.rs:455` — Hardcoded timeout value `50` (milliseconds) configures test behavior and should be a named constant, not a magic number. Extract to a test-level constant (same constant as the duplicate at line 413) and reuse it.