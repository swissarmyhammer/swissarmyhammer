---
assignees:
- claude-code
position_column: todo
position_ordinal: d980
title: Triage 13 pre-existing findings surfaced against swissarmyhammer-entity io.rs and store.rs
---
Reviewing `42e32c3a3` for ^fpcbeth produced 13 confirmed findings that target code the commit never touched. Split out here rather than folded into ^fpcbeth, matching the convention in `0eabb9e10` and `aecbd1216`.

**Read ^j4d2613 first.** The cited line numbers are wrong — the engine reported against a stale revision. Re-derive each location before acting; do not trust the numbers below.

## What was reported

- Four on attachment handling in `copy_attachment` (and something the engine called `read_attachment`), described as unvalidated `source` and `filename` reaching `Path::join` and `fs::copy`.
- Two on the `PathBuf::from("<serialization>")` sentinel used as a stand-in path in error values (really at io.rs:443 and 460).
- Two on the `.tmp_{Ulid}` temp-file naming (really at io.rs:115 and 543).
- Three magic numbers at io.rs:1155 / 1228 / 1321 — all inside `#[cfg(test)]` code. The review skill has a standing "never refactor existing tests" exception, so these are almost certainly not actionable.
- The remainder on `write_entity`, `restore_entity_files`, `read_entity_dir`, `reconcile_read_results`.

## Verify before fixing — the attachment ones look overstated

A spot check of `copy_attachment` (io.rs:503) does not support a straightforward traversal claim:

- The destination filename is `{ulid}-{sanitize_filename(original_name)}`. `sanitize_filename` (io.rs:484) strips path separators, null bytes, and leading dots, and is documented as preventing traversal and hidden files. So the write target is constrained to `.attachments/`.
- `source` is an arbitrary caller-supplied read path, but reading a caller-named file is the entire point of attaching one. That is a capability question — whether an agent-supplied path should be able to read anywhere on disk — not a parser bug.
- There is no `read_attachment` function in `io.rs` at all. The only match is a test name in `context.rs:2202`.

So treat these as candidates, not confirmed defects. If the real concern is that an agent can attach `/etc/passwd`, that is worth its own card stated in those terms, with a decision about which roots are legitimate sources.

## Acceptance

- Each of the 13 is resolved to a real location, then either fixed or closed with a one-line reason it is not a defect.
- Anything confirmed as a genuine security issue is lifted into its own card with a concrete exploit path, not left in a triage list.
- The `#[cfg(test)]` magic numbers are closed under the existing tests exception unless someone argues otherwise. #review