---
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb780
title: Remove semantic search from shelltool — keep grep only
---
## What

Remove the `search history` semantic-search operation and its entire embedding-indexing pipeline from the shell tool. Grep stays — it already reads the plain `.shell/log` file via `grep::searcher`, **not** the embedding DB, so removing embeddings does not affect it.

**Key constraint:** Do NOT remove the `swissarmyhammer-embedding` / `model_embedding` dependencies from `crates/swissarmyhammer-tools/Cargo.toml`. They are still used by `code_context/mod.rs`. This change touches only the shell tool's use of them.

### Subtasks

- [x] **Delete the `search history` operation.** Removed `search_history/` dir; dropped `pub mod search_history`, `SEARCH_HIST` static, `SHELL_OPERATIONS` entry, dispatch arm, unknown-op error entry, `with_embedder`; updated module doc + `schema()` description.

- [x] **Strip embedding indexing from `state.rs`.** Removed `ChunkJob`, SQLite `chunks` table + `db`, chunk/worker/buffer fields, `send_chunk`/`flush_chunks`/`flush_line_buffer`/`search_handle`/`search`/`embed_query`/`score_session_chunks`/`SearchResult`/`embedding_worker` + helpers, `encode/decode_embedding`, `with_dir_and_embedder`, `Drop` impl, embedding imports + consts. Kept grep/get_lines/lifecycle + their tests.

- [x] **Remove `flush_chunks` from execute_command.** Dropped the 3 calls, updated doc comments, deleted `test_execute_uses_injected_embedder`.

- [x] **Update docs/skill/README.** Cleaned `description.md`, `builtin/skills/shell/SKILL.md` (+ hand-synced `.sah/skills/shell/SKILL.md`), `apps/shelltool-cli/README.md`, `cli.rs`, `serve.rs`.

- [x] **Fix tests referencing the removed pipeline.** `file_size_limits.rs` now registers a plain `ShellExecuteTool::new()`; `mod.rs` op-count/unknown-op/dispatch tests updated. `semantic_search_e2e.rs` untouched.

## Acceptance Criteria

- [x] The shell MCP tool exposes exactly 5 operations: `execute command`, `list processes`, `kill process`, `grep history`, `get lines`. No `search history`.
- [x] Sending `{"op":"search history",...}` returns the "Unknown operation" error listing the 5 valid ops (no `search history`).
- [x] `grep history` still works end-to-end (regex + command_id filter + limit) over stored command output.
- [x] No remaining references to embeddings/chunks/`flush_chunks`/`with_embedder`/`MockEmbedder` in the `shell/` tree; `swissarmyhammer-embedding`/`model_embedding` remain depended-on only by `code_context`.
- [x] `description.md`, `builtin/skills/shell/SKILL.md` (+ regenerated `.sah/skills/shell/SKILL.md`), and `apps/shelltool-cli/README.md` no longer mention semantic search.
- [x] `cargo build -p swissarmyhammer-tools -p shelltool-cli` succeeds with no warnings about unused embedding imports.

## Tests

- [x] `test_shell_tool_has_operations` asserts 5 ops; `test_unknown_operation_lists_all_valid_ops` updated; `test_dispatch_search_history_missing_query` deleted.
- [x] `grep history` dispatch + behavior tests pass unchanged.
- [x] `file_size_limits.rs` shell tests construct the tool without an injected embedder; pass.
- [x] `cargo nextest run -p swissarmyhammer-tools shell` → 174 passed. `cargo nextest run -p shelltool-cli` → 45 passed.
- [x] `cargo build -p swissarmyhammer-tools -p shelltool-cli` → clean.

## Workflow

- Used a single foreground implementer subagent (no worktree/parallel). Build + nextest green; `cargo fmt` applied.

#shelltool #cleanup

## Review Findings (2026-05-29 12:05)

Independent review: 0 blockers, build/tests/clippy all green (174 + 45 tests, clippy 0 warnings). Stale references to removed functionality:

### Warnings
- [x] `crates/swissarmyhammer-tools/Cargo.toml:135` — FIXED: reworded the `MockEmbedder` dev-dep comment to "for code_context indexing tests" (dep retained).
- [x] `builtin/skills/shell/SKILL.md:3` (+ `.sah` copy) — FIXED: frontmatter `description:` now says "grep previous command output" and trigger "grep the last build output"; semantic-search wording dropped.
- [x] `builtin/skills/shell/SKILL.md:13`/`:18` (+ `.sah` copy) — FIXED: "stored and indexed" → "stored for later retrieval"; "search/get_lines" → "grep/get_lines".

### Nits (dispositioned — out of scope / pre-existing)
- [x] `description.md:50` & `grep_history/mod.rs:34` — grep `limit` doc says default 50 but code defaults to 10. Pre-existing mismatch NOT caused by this change; fixing it would edit the untouched `grep_history` file. Left per scope discipline — worth a separate cleanup task.
- [x] `~/.claude/skills/shell/SKILL.md` (installed global copy) still documents `search history`. Out of repo scope; regenerated on `shelltool init` after merge.

## Re-review (2026-05-29 12:06)

Warning fixes are comment/markdown-only (no compile impact); build/tests/clippy remain green. Confirmed no semantic-search references remain in either SKILL.md (only legitimate "searchable output" / ripgrep grep wording). Clean — moving to done.