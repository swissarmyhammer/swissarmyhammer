---
assignees:
- claude-code
position_column: todo
position_ordinal: ffcf80
title: 'CLI error types and docs: use anyhow::Result, not String or boxed errors'
---
## Problem

15 review findings in `apps/swissarmyhammer-cli` are open. They came off card
^hxd1r4r, which only renamed one getter in each of these files and pulled them
into review scope. Every one of them predates that card — `git blame` puts each
line in an older commit, named below.

The dominant cause is error typing. The CLI returns `Result<T, String>` and
`Result<T, Box<dyn Error>>` in many places. Both stop a caller from matching a
specific failure, and both discard the error chain. Application code must return
`anyhow::Result<T>` and attach `.context(...)` at each fallible call.

## Findings

### apps/swissarmyhammer-cli/src/cli_executor.rs

- [ ] `:55` (blame `b34e5321b8`) — Public struct `CliExecutor` does not implement `Debug`. Per the trait-implementations rule, public types must implement all applicable traits. It holds `Arc` and `RwLock` with non-Debug internals, so write a manual `impl Debug for CliExecutor` like the one on `CliToolContext`, showing opaque placeholders for complex types such as `ToolRegistry`.
- [ ] `:62` (blame `b34e5321b8`) — Returns `Result<T, Box<dyn Error + Send + Sync>>` instead of `anyhow::Result<T>`. Change to `pub async fn new(working_dir: &Path) -> anyhow::Result<Self>`, add `use anyhow::Context;`, and give line 65 `.context("failed to initialize tool context")?`.
- [ ] `:65` (blame `b34e5321b8`) — Uses a manual `.map_err()` with `.to_string()`, which breaks the error chain. Replace `.map_err(|e| Box::<dyn Error + Send + Sync>::from(e.to_string()))?` with `.context("failed to initialize tool context")?`.

### apps/swissarmyhammer-cli/src/main.rs

- [ ] `:1` (blame `074a2b0065`) — missing documentation for the crate.
- [ ] `:370` (blame `d0e19df80f`) — the hardcoded `5` for `max_warnings` is a display threshold. Name it, for example `const MAX_VALIDATION_WARNINGS_DISPLAY: usize = 5;`.
- [ ] `:503` (blame `d0e19df80f`) — the hardcoded `10` for `max_errors` is a display threshold. Name it, for example `const MAX_VALIDATION_ERRORS_DISPLAY: usize = 10;`.
- [ ] `:715` (blame `d0e19df80f`) — returns `Result<String, String>`. Change to `anyhow::Result<String>` and use `.context()` at the error sites.
- [ ] `:815` (blame `1bea6b3ce4`) — returns `Result<T, String>`. Change to `anyhow::Result<T>` and use `.context()`.
- [ ] `:842` (blame `894d179933`) — returns `Result<bool, String>`. Change to `anyhow::Result<bool>` and use `.context()`.
- [ ] `:852` (blame `a1ea138f02`) — returns `Result<serde_json::Value, String>`. Change to `anyhow::Result<serde_json::Value>` and use `.context()`.
- [ ] `:864` (blame `a1ea138f02`) — returns `Result<serde_json::Map<String, serde_json::Value>, String>`. Change to `anyhow::Result<serde_json::Map<String, serde_json::Value>>` and use `.context()`.
- [ ] `:993` (blame `6e2231fbd4`) — returns `Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>>`. Change to `anyhow::Result<...>` and replace the `.map_err()` chains with `.context()`.

### apps/swissarmyhammer-cli/src/validate.rs

- [ ] `:47` (blame `074a2b0065`) — missing documentation for an associated function.
- [ ] `:56` (blame `d5b834e593`) — missing documentation for a method.
- [ ] `:375` (blame `a1ea138f02`) — `tool_validation_success_issue` is a near-duplicate of `tool_context_init_error_issue` at `validate.rs:338` (53 tokens, 92% alike).

## Approach

A finding shows one example of a cause. Remove each cause from the whole file:
sweep every `Result<T, String>` and `Box<dyn Error>` return in all three files,
not only the lines named above.

Expect the review scope to widen. Changing a public return type reaches every
call site, and each file that a call site sits in enters review. Work file by
file and commit each file on its own, so one review round covers one file.

## Done when

- No `Result<T, String>` and no `Box<dyn Error>` return is left in
  `cli_executor.rs`, `main.rs`, or `validate.rs`.
- Every fallible call in those files carries `.context(...)`.
- `cargo nextest run --workspace` is green, `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` are clean.

## History

Split off ^hxd1r4r on 2026-08-12. That card removed the inline-on-edit LSP
diagnostics feature. Its finding count went 1, then 4, then 17, because a getter
rename kept pulling new files into review scope. 15 of the 17 were these
pre-existing defects. The 2 that card actually caused are fixed in commit
7d06f3225.