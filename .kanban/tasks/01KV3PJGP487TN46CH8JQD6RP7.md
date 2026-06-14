---
assignees:
- claude-code
position_column: todo
position_ordinal: '9e80'
project: local-review
title: Inline full changed-file source in review prompt (eliminate redundant re-reads)
---
## What

The review fan-out prompt hands each validator a **bounded** source slice per changed file (`render_file_block` → `bounded_slice`), but the local qwen model re-reads every changed file anyway via `read_file`/`glob`/`grep` before reasoning. Calcutron eval logs show the changed file's content is prefilled **twice** (once as the inlined slice, once as the `read_file` tool result the model fetches regardless), and ~82% of review wall-clock is spent in tool-call round-trips, not decoding (~78 tok/s decode is not the bottleneck).

Root cause: the bounded slice is incomplete, so the model reads the whole file to be safe. Fix by **giving in and inlining the whole changed file**, clearly framed so the model knows it does NOT need to read it. Keep `read_file`/`grep`/`glob` advertised — they remain essential for cross-file work (duplication evidence cites *other* files, refactor suggestions need them). Only the **changed** files get pre-provided in full so there's no reason to re-read them.

The full file text is already available at the slice-build site — `scope.rs:244` passes `resolved.after_content.get(file)` into `bounded_slice`, which currently discards most of it.

Files to modify:
- `crates/swissarmyhammer-validators/src/review/scope.rs` — populate `FileWork.source_slice` (or a new `full_source` field) with the **complete** `after_content` for the file. The constraint is the model's **context window**, NOT the 16384 generation cap (`max_tokens` in the prompt `_meta` bounds the *reply*, not the input). Typical source files inline whole with ample headroom (primed prefixes are only ~5k tokens). A guard is only needed for a pathologically large file relative to the configured context window: in that rare case fall back to the existing `bounded_slice` plus an explicit note that the file was too large to inline in full and `read_file` should be used for the remainder. Pick the cap from the model/context-window config, not a hardcoded 16k. Keep `bounded_slice` for that fallback path only.
- `crates/swissarmyhammer-validators/src/review/fleet.rs` — in `render_file_block`, reframe the per-file block so the intent is explicit: a clearly-fenced full-file section labeled as the complete current contents ("here is the full file — you do NOT need to read it"), followed by the "what changed" semantic diff. Update `OUTPUT_CONTRACT` / the file-block preamble so reads are scoped to *other* files (cross-file duplication, callers, type defs), not the changed files already provided in full.

Constraints:
- Do NOT remove or hide the intrinsic file tools — cross-file duplication detection and refactor suggestions depend on them.
- Keep the validator prefix (cached/primed part) unchanged; the full-file payload lives in the per-fork file payload, not the shared prefix.
- The size guard (if any) keys off the configured context window, not the 16384 generation cap.

## Acceptance Criteria
- [ ] For a changed file that fits the context window, the rendered payload contains the file's **complete** source in a clearly-labeled fenced block, plus the semantic diff, plus explicit text that the file need not be read.
- [ ] For a changed file too large for the context-window budget, the payload falls back to the bounded slice with a note directing the model to `read_file` for the rest.
- [ ] `read_file`/`glob`/`grep` remain advertised to the review session (no tool gating).
- [ ] The cached validator prefix bytes are unchanged (full-file content stays in the per-file/per-fork payload).

## Tests
- [ ] `crates/swissarmyhammer-validators/src/review/fleet.rs` unit test: `render_file_payload` for a small file emits the full source (assert a line that `bounded_slice` would have trimmed is present) and the "you do not need to read this file" framing.
- [ ] `crates/swissarmyhammer-validators/src/review/scope.rs` unit test: a file whose full content exceeds the context-window cap yields the bounded-slice fallback + the read-for-the-rest note; a file under the cap yields the full content.
- [ ] Unit test asserting the rendered prefix (shared/cached sections) does not contain the file source, so prefix caching is unaffected.
- [ ] `cargo test -p swissarmyhammer-validators review::` → green.

## Workflow
- Use `/tdd` — write the failing render/scope tests first, then implement.