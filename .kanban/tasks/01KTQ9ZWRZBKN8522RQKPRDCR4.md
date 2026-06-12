---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9180
project: local-review
title: Concurrent serves truncate the shared mcp.log, destroying each other's logs
---
## Problem (observed)

When multiple `sah serve` processes run in the same workspace — e.g. a `/finish` scoped-batch that spawns ~16 parallel `claude --print` subagents, each with its own `sah serve` — they all write to the **same** `<root>/.sah/mcp.log`, and each one **truncates it on startup**. The result: the log is repeatedly clobbered and interleaved, so a concurrent local-model review run is effectively **unobservable** (this is exactly what made the calcutron review run impossible to read — the log kept resetting from 5.9 MB back to 32 KB).

## Root cause

`crates/swissarmyhammer-common/src/logging.rs` `open_log_file` (the `File::create(&log_file_path)` call) truncates the file every time a process starts. `LOG_FILE_NAME` is a single shared `"mcp.log"`, so every concurrent process opens and truncates the same path. `init_file_tracing_with_fallback` then writes the live serve's logs into that shared, truncated file.

## Fix direction (decide during implementation)

Stop concurrent processes from destroying each other's logs, while **preserving fresh-per-run semantics and bounded growth** (do NOT switch to a plain unbounded `append` to the shared file — the single-serve log already reached tens of MB per run; appending across every run would grow without bound and still interleave 16 serves into one unreadable file).

Recommended: **per-process log files** — write to `mcp.<pid>.log` (or include the pid/a short uid in the name) so each serve gets its own fresh, independently-readable file and never clobbers another. Keep `init_file_tracing_with_fallback`/`open_log_file` the single chokepoint so every CLI (sah, kanban, shell, code-context) inherits the fix. Consider whether to keep a stable `mcp.log` pointer (e.g. the newest) for the common single-serve case, and whether old per-pid files need a cleanup/rotation policy — but the core requirement is: **two live processes in one workspace must not lose each other's logs.**

Whatever path is chosen, document it and keep the change minimal and centralized in `logging.rs`.

## Acceptance criteria

- Two processes initializing file logging against the same workspace dir do NOT truncate/destroy each other's log output — both processes' lines survive and are independently readable.
- Fresh-per-run behavior is preserved (a process does not inherit/append to unbounded prior content) and growth stays bounded per process.
- A unit test in `logging.rs` proves the no-clobber property: open a log for "process A", write a marker, then open a second log for "process B" against the same dir, write a marker, and assert A's marker is still present (today's `File::create` truncation would delete it).
- All CLIs continue to log (the change stays in the shared `open_log_file`/`init_file_tracing_with_fallback` path; no per-CLI duplication).
- `cargo test -p swissarmyhammer-common` and `cargo clippy -p swissarmyhammer-common --all-targets -- -D warnings` clean.

Context: surfaced while verifying local-model review end-to-end; see [[project_review_local_queue_full_silent_drop]].