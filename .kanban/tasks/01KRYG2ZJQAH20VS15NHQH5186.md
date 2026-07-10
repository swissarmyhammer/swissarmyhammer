---
assignees:
- claude-code
depends_on:
- 01KRYG1VWTF16P6FQCX1ZRTZZX
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8c80
project: plugin-examples
title: 'Example plugin: file-notes (in-process files tool, write/read round-trip)'
---
## What

Add a committed example that drives the real in-process `files` MCP tool: write a note file, read it back, write the read-back content into a second file. This is the filesystem-effect example — the most relatable for plugin authors — and proves an in-process Rust module round-trips return values back into the isolate.

- Create `crates/swissarmyhammer-plugin/examples/plugins/file-notes/plugin.json` — `id: "file-notes"`, `entry: "entry.ts"`, `provides: ["fs"]`.
- Create `crates/swissarmyhammer-plugin/examples/plugins/file-notes/entry.ts` — a `Plugin` subclass whose `load()` registers `{ rust: "files" }` as `"fs"` and performs `write file` → `read file` → `write file` against **relative paths** (e.g. `notes/hello.txt`, `notes/echo.txt`). A committed example cannot hard-code an absolute temp path, so it must use relative paths; the `files` tool resolves these against the process current directory (`shared_utils.rs`). Document this contract in the file.
- Create `crates/swissarmyhammer-plugin/tests/file_notes_e2e.rs` (`mod support;`). Because the `files` tool resolves relative paths against the **process** CWD, the test MUST:
  - set the process CWD to a fresh `tempfile::TempDir` for the duration of the test (use the repo's existing current-dir guard — search for `CurrentDirGuard` / equivalent; if none exists, add a small RAII guard in `tests/support/mod.rs`);
  - be annotated `#[serial_test::serial]` so it never races other CWD-touching tests (see the project's test-isolation guidance — process-CWD tests must be temp-isolated AND serialized);
  - after `discover_and_load_all`, assert both note files exist under the temp CWD with the expected contents.

## Acceptance Criteria
- [x] `examples/plugins/file-notes/{plugin.json,entry.ts}` exist; `entry.ts` uses relative paths and documents the working-directory contract.
- [x] `tests/file_notes_e2e.rs` runs under a temp-CWD guard and is `#[serial_test::serial]`; it loads the COMMITTED bundle via `stage_example`.
- [x] Both note files land under the temp CWD with exact expected contents; the real source tree is never written to.
- [x] README in `examples/plugins/` updated to describe `file-notes` and the relative-path contract.

## Tests
- [x] New: `tests/file_notes_e2e.rs::file_notes_plugin_round_trips_through_files_tool` — asserts both files' contents after load.
- [x] Run `cargo nextest run -p swissarmyhammer-plugin --test file_notes_e2e` — passes; run it twice back-to-back to confirm no CWD leakage.
- [x] Confirm `git status` is clean after the run — no stray files written into the repo.

## Workflow
- Use `/tdd` — write the failing test (with the CWD guard) first, then the example bundle.
- If `serial_test` is not already a dev-dependency of the crate, add it (it is used elsewhere in the workspace).

## Implementation Notes
- The existing `CurrentDirGuard` in `swissarmyhammer_common::test_utils` was reused (no new guard needed); it serializes CWD changes via a global mutex and restores on drop.
- Added `swissarmyhammer-common` and `serial_test` as workspace dev-dependencies of the `swissarmyhammer-plugin` crate.
- The example uses the direct `op` dispatch form (mirroring `files_dispatch_e2e`), with `entry.ts` documenting the relative-path / process-CWD contract.
- Verified: `file_notes_e2e` passes twice back-to-back; full `swissarmyhammer-plugin` suite (131 tests) green; `cargo clippy --tests` clean; the test run leaves no stray files in the repo.

## Review Findings (2026-05-18 16:42)

### Nits
- [x] `examples/plugins/file-notes/entry.ts:35` and `tests/file_notes_e2e.rs:13` — Both attribute the relative-path / process-CWD resolution to `shared_utils.rs::validate_file_path`. The documented behavior is correct, but neither `files` operation the example uses actually calls that function: `execute_write` (`files/write/mod.rs:186-198`) does its own inline `std::env::current_dir().join(...)` resolution, and `read file` (`files/read/mod.rs:205`) resolves through a `PathValidator::validate_path`. `validate_file_path` exists and resolves relatives the same way, so the contract claim holds — only the specific function citation is imprecise. Suggest rewording to attribute resolution to the `files` tool generally (or to the actual write/read resolution sites) so a reader who follows the citation does not find a dead end.
  - Resolved: reworded both `entry.ts` and `file_notes_e2e.rs` to drop the `shared_utils.rs::validate_file_path` citation and instead attribute resolution to the actual sites — `write file` joins onto `std::env::current_dir()` (`files/write/mod.rs`), `read file` resolves through the tool's `FilePathValidator` (`files/read/mod.rs`).