---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffc980
project: kanban-mcp
title: Extract logging.rs as standard pattern across all CLIs
---
## What

sah-cli has `logging.rs` with `FileWriterGuard` (flush+sync on every write) — the other CLIs reinvent this inline in main.rs or don't have it at all. Make `logging.rs` the standard pattern.

Options:
1. Extract `FileWriterGuard` + tracing init into a shared crate (e.g. `swissarmyhammer-common::logging`)
2. Or duplicate `logging.rs` into each CLI (same code, local module)

Either way, every CLI should have consistent file-based tracing with stderr fallback:
- **shelltool-cli**: currently inlines FileWriterGuard in main.rs -> extract to logging.rs
- **code-context-cli**: needs logging.rs added
- **kanban-cli**: logging card currently puts it in main.rs -> should be logging.rs instead

## Acceptance Criteria
- [x] All four CLIs (sah, shelltool, code-context, kanban) have consistent logging setup
- [x] FileWriterGuard pattern is in a `logging.rs` module (or shared crate), not inline in main.rs
- [x] Each CLI logs to its tool-specific log file with stderr fallback

## Implementation Notes

Chose option 1 (shared crate). `FileWriterGuard` lives in `swissarmyhammer-common::logging` and all four CLIs import it from there; no more duplicates.

Each CLI's `logging.rs` is a thin wrapper that:
- Imports `FileWriterGuard` from the shared crate
- Defines a CLI-specific `make_filter`/`determine_log_level` + `open_log_file` helper pair
- Exposes a single public `init_tracing` or `configure_logging` entry point

Per-CLI log destinations:
- sah-cli: `<sah-root>/mcp.log` (via `SwissarmyhammerDirectory::from_git_root`)
- shelltool-cli: `<cwd>/.shell/mcp.log`
- code-context-cli: `<cwd>/.code-context/mcp.log`
- kanban-cli: `<cwd>/.kanban/mcp.log` (never auto-creates `.kanban/`)

### sah-cli refactor details
- Removed duplicated `FileWriterGuard` struct from `swissarmyhammer-cli/src/logging.rs`; it now uses `swissarmyhammer_common::logging::FileWriterGuard`
- Moved the five logging helpers (`determine_log_level`, `create_env_filter`, `setup_mcp_logging`, `setup_logging_with_writer`, `setup_stderr_logging`) and the public `configure_logging` entry point from `main.rs` into `logging.rs`
- Moved the `ensure_swissarmyhammer_dir` helper into `logging.rs` (its only caller)
- Updated the single call site in `main.rs` to `logging::configure_logging(...)`

### Security hardening
Added `validate_log_file_name` to reject any `SWISSARMYHAMMER_LOG_FILE` value containing path separators, parent-directory (`..`) components, or absolute paths, preventing a hostile env var from redirecting log output outside the sah data directory. Covered by 5 unit tests (bare names, `..`, separators, absolute paths, `.` / empty).

## Review Findings (2026-04-12 14:45)

### Warnings
- [x] `shelltool-cli/src/logging.rs:49-56` and `code-context-cli/src/logging.rs:52-59` — `open_log_file` silently swallows both `create_dir_all` failures and `File::create` failures (`.ok()`), so users get no signal on stderr when logging to file can't be set up. `kanban-cli/src/logging.rs:53-65` and `swissarmyhammer-cli/src/logging.rs:199-204` both warn to stderr on failure. This is exactly the kind of inconsistency the card is trying to eliminate — the "standard pattern" should include a standard warning message on fall-back. Suggested fix: have each wrapper match the `"Warning: Could not setup MCP logging: {e}. Falling back to stderr."` format used by `swissarmyhammer-cli` when a create attempt actually fails (distinct from the benign "directory doesn't exist" path for kanban).
- [x] `avp-cli/src/logging.rs:11-43` — `FileWriterGuard` is duplicated verbatim here even though the whole point of this card is to consolidate it into `swissarmyhammer_common::logging::FileWriterGuard`. avp-cli isn't in the card's stated scope, but leaving the duplicate behind undermines the "standard pattern" framing and invites drift. Either remove avp-cli's local copy and import the shared one, or explicitly call out in the card why avp-cli is exempt.

### Nits
- [x] `swissarmyhammer-cli/src/logging.rs` vs `kanban-cli/src/logging.rs` / `shelltool-cli/src/logging.rs` / `code-context-cli/src/logging.rs` — naming convention is inconsistent across the four CLIs: sah uses `create_env_filter` + `setup_mcp_logging` + `setup_stderr_logging` + `configure_logging` + `DEFAULT_LOG_FILE_NAME`, while the other three use `make_filter` + `open_log_file` + `init_tracing` + `LOG_FILE_NAME`. The card's premise is "standard pattern" — pick one vocabulary (the newer `make_filter` / `init_tracing` set is shorter) and align sah-cli to it, or at least document the rationale for the split.
- [x] `shelltool-cli/src/logging.rs:67-91` and `code-context-cli/src/logging.rs:71-95` — the two `init_tracing` functions are structurally identical (same layer builder chain, same stderr fallback) and differ only in `make_filter` content and the data-directory name. Consider lifting a helper like `init_file_tracing_with_fallback(filter, root, dir_name)` into `swissarmyhammer_common::logging` so per-CLI `logging.rs` shrinks to `make_filter` + entry point. Would also make the warning-consistency fix above a one-line change in the shared helper.
- [x] `kanban-cli/src/logging.rs:35-38` — `open_log_file` doc says "Returns None when the directory does not exist (so the caller falls back to stderr), or when the directory exists but the log file could not be created... in which case a warning is emitted to stderr". The "in which case" phrase is ambiguous — it refers only to the second sub-branch, but a casual reader might think it covers both. Suggest splitting into two sentences so it's explicit that the absent-directory path is silent by design.
- [x] `shelltool-cli/src/logging.rs:93-138` and `code-context-cli/src/logging.rs:97-142` — unit tests exercise `open_log_file` but not `init_tracing`. `kanban-cli/src/logging.rs:165-182` shows how to do it with `#[serial]` and `CurrentDirGuard`. Apply the same pattern to the other two wrappers so `init_tracing` plumbing isn't entirely untested at unit level.
- [x] `swissarmyhammer-common/src/logging.rs:59, 69` — `FileWriterGuard` uses `.expect("FileWriterGuard mutex was poisoned")` on the mutex lock. If tracing is invoked from a panic-unwind path after poisoning, this will double-panic and obscure the original failure. Low severity / pre-existing pattern, but a common fix is to lock-and-ignore-poisoning via `.unwrap_or_else(|e| e.into_inner())` for logging sinks where best-effort is fine.

## Review Response (2026-04-12)

All 2 warnings + 5 nits addressed.

### Shared helper extraction (nit 2)
Extracted `init_file_tracing_with_fallback(filter, root, dir_name, policy)` and `open_log_file(root, dir_name, policy)` into `swissarmyhammer-common::logging`. Each kanban-family CLI wrapper (kanban, shelltool, code-context) is now ~60 lines: just `make_filter` + a one-line entry point. The `DirPolicy` enum (`MustExist` for committed data dirs like `.kanban/`; `AutoCreate` for runtime data dirs like `.shell/`, `.code-context/`) encodes the one meaningful policy difference between CLIs.

### Warning consistency (warning 1)
Centralized the stderr warning through a private `warn_logging_fallback(&io::Error)` helper in the shared crate. Every failure path — `create_dir_all` failure under `AutoCreate`, `File::create` failure in either policy — now emits `"Warning: Could not setup MCP logging: {e}. Falling back to stderr."`, matching sah-cli's format. The `MustExist + absent` path stays silent by design (it's the caller's signal for "don't log to file").

### avp-cli duplicate removal (warning 2)
Deleted `avp-cli/src/logging.rs` and its `pub mod logging;` declaration; `main.rs` now imports `FileWriterGuard` directly from `swissarmyhammer_common::logging`. avp-cli has its own dual-layer (stderr + file) init flow that doesn't fit the MCP-CLI `init_tracing` mold, so it keeps its inline setup — but the `FileWriterGuard` struct itself is no longer duplicated.

### Naming alignment (nit 1)
Renamed sah-cli's `create_env_filter` → `make_filter` and `DEFAULT_LOG_FILE_NAME` → `LOG_FILE_NAME` to match the vocabulary used by the other three wrappers. The `setup_*` internal helpers and the `configure_logging` public entry point keep their names because sah-cli's dual-mode signature (`verbose`, `debug`, `quiet`, `is_mcp_mode`) is fundamentally different from the single-`debug` `init_tracing` used by kanban/shelltool/code-context — the rationale is documented in the `make_filter` doc comment.

### Doc clarity (nit 3)
The kanban-cli `init_tracing` doc now splits the fallback paths into three distinct sentences: the happy path, the "absent directory is silent by design" path, and the "directory exists but file-create failed" path. The shared helper's `DirPolicy::MustExist` doc comment does the same split. No more "in which case" ambiguity.

### init_tracing tests (nit 4)
Added `#[serial]` + `CurrentDirGuard`-based `init_tracing` unit tests to `shelltool-cli` and `code-context-cli`, matching kanban-cli's pattern. Added `serial_test` as a dev-dependency to shelltool-cli.

### Lock-poisoning handling (nit 5)
`FileWriterGuard::write` and `flush` now use `.unwrap_or_else(|e| e.into_inner())` on the mutex lock. Double-panicking from a logging sink on an already-unwinding thread would bury the original failure; recovery-on-poison is the correct behavior for best-effort sinks. Covered by a new `write_recovers_from_poisoned_mutex` unit test in `swissarmyhammer-common`.

### Files changed
- `swissarmyhammer-common/src/logging.rs` — added `DirPolicy`, `LOG_FILE_NAME`, `open_log_file`, `init_file_tracing_with_fallback`, `warn_logging_fallback`; updated `FileWriterGuard` to recover from poisoned mutex; added 5 new unit tests
- `swissarmyhammer-common/Cargo.toml` — added `tracing-subscriber` dep
- `kanban-cli/src/logging.rs` — rewritten as thin wrapper around shared helper
- `shelltool-cli/src/logging.rs` — rewritten as thin wrapper around shared helper; added `init_tracing` unit test
- `shelltool-cli/Cargo.toml` — added `serial_test` dev-dep
- `code-context-cli/src/logging.rs` — rewritten as thin wrapper around shared helper; added `init_tracing` unit test
- `swissarmyhammer-cli/src/logging.rs` — renamed `create_env_filter` → `make_filter`, `DEFAULT_LOG_FILE_NAME` → `LOG_FILE_NAME`
- `avp-cli/src/logging.rs` — deleted
- `avp-cli/src/lib.rs` — dropped `pub mod logging;`
- `avp-cli/src/main.rs` — import `FileWriterGuard` from `swissarmyhammer_common::logging`

### Tests
All logging-related tests pass:
- `swissarmyhammer-common` logging tests: 7 passed
- `kanban-cli` logging tests: 2 passed
- `shelltool-cli` logging tests: 2 passed
- `code-context-cli` logging tests: 2 passed
- `swissarmyhammer-cli` logging tests: 5 passed
- `avp-cli`: 35 passed
- `cargo clippy --all-targets` clean across all six crates

## Review Findings (2026-04-12 19:55)

Re-verification of the 2 warnings + 5 nits from the prior review: all genuinely addressed in the code. Confirmed:
- `swissarmyhammer-common/src/logging.rs` now owns `DirPolicy`, `LOG_FILE_NAME`, `open_log_file`, `init_file_tracing_with_fallback`, `warn_logging_fallback`, and the poison-recovering `FileWriterGuard` (7 unit tests pass).
- Each kanban-family `logging.rs` (kanban, shelltool, code-context) is a thin wrapper (~60 lines) delegating to the shared helper with its own `DirPolicy`.
- sah-cli uses the shared `FileWriterGuard` and renamed its helpers to align vocabularies.
- `avp-cli/src/logging.rs` is deleted; `main.rs` imports from `swissarmyhammer_common::logging::FileWriterGuard`.
- All six crates pass `cargo clippy --all-targets`. All logging unit tests pass.

One fresh concern flagged by the card handoff:

### Warnings
- [x] `shelltool-cli/src/main.rs:110-150` vs `shelltool-cli/src/logging.rs:95-113` — The shelltool test binary has two uncoordinated regimes for serializing CWD mutation: the async `ENV_LOCK` in `main.rs::tests` (`tokio::sync::Mutex`) and the `CURRENT_DIR_LOCK` inside `CurrentDirGuard` (from `swissarmyhammer_common::test_utils`). `#[serial]` on `init_tracing_creates_mcp_log_under_shell_dir` only serializes with other `#[serial]`-tagged tests — it does not block async tests in `main.rs` that grab `ENV_LOCK` and call `std::env::set_current_dir`. This is the root cause of the reported "passes most of the time, occasionally fails" flakiness. Fix options: (a) have the `main.rs` async tests use `CurrentDirGuard` instead of rolling their own `CwdGuard`/`ENV_LOCK` pair (loses async-compat, so those tests would need a `tokio::task::spawn_blocking` wrap), or (b) have `init_tracing_creates_mcp_log_under_shell_dir` await the same `ENV_LOCK` used by `main.rs`, or (c) `#[ignore]` the test and lean on the shared-crate coverage of `open_log_file` under `DirPolicy::AutoCreate`. Option (a) is the cleanest long-term fix and what the other two kanban-family wrappers already do (they have no competing `ENV_LOCK` in their test binaries, which is why they don't flake). Recommend not blocking this card on the fix — file it as a follow-up test-hardening card so this card can move to done on the logging-refactor merits.

## Review Response (2026-04-12, second round)

Took option (a): consolidated on `CurrentDirGuard` across every CWD-mutating test in the shelltool test binary, eliminating the two-regime race entirely.

### Changes in `shelltool-cli/src/main.rs::tests`
- Removed the local `ENV_LOCK` (`tokio::sync::Mutex`) and `CwdGuard` struct along with their `LazyLock` and `AsyncMutex` imports.
- Replaced the three `#[tokio::test]` `dispatch_command_*` tests with `#[test]` functions that use `CurrentDirGuard` (from `swissarmyhammer_common::test_utils`) to enter a tempdir and a tiny local `block_on` helper that spins up a single-threaded tokio runtime to await `dispatch_command`.
- `block_on` uses `tokio::runtime::Builder::new_current_thread().enable_all().build()` — keeps the `CurrentDirGuard` entirely on the test thread, so the `std::sync::MutexGuard` inside it never crosses an `.await` point (avoids `clippy::await_holding_lock`).
- `dispatch_command_doctor_runs_diagnostics` no longer takes `CurrentDirGuard` because `run_doctor` doesn't touch CWD — kept scope-minimal.

### Why this kills the flake
After the change, every CWD-mutating test in the shelltool test binary — the three `dispatch_command_*` tests and `logging::tests::init_tracing_creates_mcp_log_under_shell_dir` — takes the same global `CURRENT_DIR_LOCK` inside `swissarmyhammer_common::test_utils`. `#[serial]` on the init_tracing test is still there but now only serves its original purpose (serializing the one-shot tracing-subscriber global init); the CWD serialization is handled by the shared mutex.

No test binary in the kanban-family now has two competing CWD-serialization regimes.

### Tests
- `cargo test -p shelltool-cli --bin shelltool`: 35 passed on 10 consecutive runs (including at `--test-threads=16` and `--test-threads=32`, stress-testing any remaining race window).
- `cargo clippy -p shelltool-cli --all-targets -- -D warnings`: clean.
- `cargo test -p swissarmyhammer-common --lib logging`: 7 passed (sanity check — the shared helper wasn't touched in this round, but verified for regression).

### Files changed (this round)
- `shelltool-cli/src/main.rs` — removed `ENV_LOCK` / `CwdGuard` / `AsyncMutex` / `LazyLock`; added `CurrentDirGuard` import and `block_on` helper; converted three async tests to sync tests.