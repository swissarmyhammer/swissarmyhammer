---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvtwqpw8g7e7f6t6ahkmyded
  text: 'Picked up by /finish $file-edit-tools (scoped-batch). First foundational task — no deps; blocks read/cascade/result-contract/write-guard. Note: the file-edit-tools project didn''t persist at plan time (add-task project field was a no-op without an existing project); created the `file-edit-tools` project and reassigned all 11 tasks before starting. Driving via /implement → /test → /review → local /commit.'
  timestamp: 2026-06-23T18:43:58.472087+00:00
- actor: claude-code
  id: 01kvtwrzkw8796h2vsh0955d6m
  text: 'Picked up. Read task + comments. Read root Cargo.toml workspace pattern, swissarmyhammer-operations/Cargo.toml as sibling reference, and LineEnding::detect in swissarmyhammer-tools/.../files/edit/mod.rs (simple count-based detector, 4 variants Lf/CrLf/Cr/Mixed). Plan: create crates/swissarmyhammer-hashline with crc32fast only, add to workspace members + [workspace.dependencies] crc32fast. TDD via /tdd: failing tests first for hash_line/tag/parse_anchor/apply, watch fail, implement. Tests run with `cargo nextest run -p swissarmyhammer-hashline` (NOT cargo test).'
  timestamp: 2026-06-23T18:44:40.188339+00:00
- actor: claude-code
  id: 01kvtxc6y27p0rzx4qkpgd69e6
  text: |-
    Implemented crate crates/swissarmyhammer-hashline via strict TDD (watched RED -> GREEN).

    API (src/lib.rs): hash_line (crc32fast over line trimmed of spaces/tabs only via trim_matches([' ','\t']), mod 256), render_hash (format!("{:02x}")), tag (N:HH|line via internal split_lines that preserves each line's original terminator -> LF/CRLF/CR/mixed/trailing-newline all preserved), parse_anchor (N:HH, optional |text suffix, exactly-2-char hex enforced), apply (exact line N first, then symmetric proximity search PROXIMITY_WINDOW=50 expanding outward nearest-wins; HashlineError::Mismatch carries op + tag(content,1) re-tagged content). LineEnding ported (copy, not dep) into src/line_ending.rs.

    Deps: crc32fast ONLY (added crc32fast="1.5" to [workspace.dependencies]); proptest dev-dep. cargo tree confirms no swissarmyhammer-tools edge. Registered crate in root Cargo.toml [workspace] members + path entry.

    Tests: 18 unit + 10 integration/property (proptest) in tests/properties.rs. `cargo nextest run -p swissarmyhammer-hashline` => 28/28 passed, 0 failed. NOTE: nextest reused a stale test binary twice mid-session (reported todo! panics at old line numbers after edits); `touch`-ing the source forced a correct rebuild — real results only after that. Worth knowing for the next agent.

    Quality gates green: `cargo fmt -p swissarmyhammer-hashline -- --check` clean; `cargo clippy -p swissarmyhammer-hashline --all-targets -- -D warnings` exit 0; `cargo build -p swissarmyhammer-hashline` ok; `cargo metadata --no-deps` workspace resolves.

    really-done: verification command green + double-check agent verdict PASS (independently confirmed hash_line doesn't over-strip \r/\n, proximity nearest-wins, apply preserves trailing newlines, parse_anchor rejects 42:a3a/42:zz/:a3/42:, no tools-crate dep). Generated tests/.proptest-regressions seed file left in place as a regression guard.

    Moving to review. Not committing (orchestrator handles commits).
  timestamp: 2026-06-23T18:55:10.274633+00:00
- actor: claude-code
  id: 01kvty5vy994ey7sgfh4xhbebj
  text: |-
    Worked the single review finding (Nits): added a runnable crate-level doc example in src/lib.rs walking tag(content,1) -> parse_anchor() on a tagged line -> build AnchorOp -> apply, with assertions. It compiles and runs as a doctest. Note: I initially guessed the hash for "hello" as 5d; the doctest caught it (actual hash 86) — corrected to `1:86|hello`.

    Verification (fresh): `cargo test -p swissarmyhammer-hashline --doc` => 1 passed (only allowed cargo test use; nextest can't run doctests). `cargo nextest run -p swissarmyhammer-hashline` => 28/28 passed. `cargo fmt` applied. `cargo clippy -p swissarmyhammer-hashline --all-targets -- -D warnings` => exit 0.

    Flipped the finding to [x]. Moving back to review. Not committing (orchestrator commits).
  timestamp: 2026-06-23T19:09:10.985588+00:00
- actor: claude-code
  id: 01kvtysapd9d79kdgvtnhprm3e
  text: |-
    Worked the 3 unchecked findings from Review Findings (2026-06-23 13:09), all in crates/swissarmyhammer-hashline/src/lib.rs:

    1. (Warning) Rewrote the crate-level doc example as `fn main() -> Result<(), Box<dyn std::error::Error>>` using `?` and `.ok_or(...)?` (for `.nth(1)` and `parse_anchor`), removing all `.unwrap()`/`.expect()`. This required adding `impl Display for HashlineError` (message "anchor N:HH did not resolve to a matching line", via render_hash) and `impl std::error::Error for HashlineError {}` so `apply(...)?` converts into `Box<dyn Error>`. Minimal, no behavior change; the Mismatch pattern-match test still passes.
    2. (Nit) Added a doc comment to `pub use line_ending::LineEnding;` explaining the re-export, with an intra-doc link to LineEnding::detect.
    3. (Nit) Extracted the capacity magic numbers into documented consts: TAGGING_OVERHEAD_RATIO=4 and TAGGING_OVERHEAD_BYTES=16 (values unchanged); `tag` uses them.

    double-check (advisory) caught a doc-merge bug from finding 3: placing the const doc comments immediately after `tag`'s doc comment (no item between) merged tag's doc onto the first const and left `pub fn tag` undocumented. Fixed by reordering: consts (with their docs) first, then tag's doc comment directly above `pub fn tag`. Re-verified all gates after the fix.

    Verification (fresh, after a forced rebuild — note nextest/doctest reuse stale binaries; `touch`-ing the source forces a correct rebuild as the prior agent flagged):
    - `cargo test -p swissarmyhammer-hashline --doc` => 1 passed (only allowed cargo test use; nextest can't run doctests)
    - `cargo nextest run -p swissarmyhammer-hashline` => 28/28 passed, 0 failed
    - `cargo fmt -p swissarmyhammer-hashline` applied (clean)
    - `cargo clippy -p swissarmyhammer-hashline --all-targets -- -D warnings` => exit 0

    All 3 findings flipped to [x]. Moving back to review. Not committing (orchestrator commits).
  timestamp: 2026-06-23T19:19:48.685225+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd280
project: file-edit-tools
title: swissarmyhammer-hashline crate — pure hashline core (hash/tag/parse/apply)
---
## What
Create a new pure, IO-free crate `crates/swissarmyhammer-hashline` and add it to the workspace `members` in the root `Cargo.toml`. It provides the hashline anchor primitives used by `read files` (to tag lines) and `edit files` (to resolve anchors). No file IO in this crate.

Public API (in `src/lib.rs` + small submodules):
- `hash_line(&str) -> u8` rendered as 2 lowercase hex chars by a helper: `crc32fast` over the line with **leading/trailing horizontal whitespace (spaces and tabs) stripped, interior preserved**, then `mod 256`. 256 values = staleness detection, not uniqueness; the line number disambiguates collisions.
- `tag(content: &str, start_line: usize) -> String` — annotate each line as `N:HH|line` where N is the absolute 1-based line number (= `start_line` for the first line).
- `parse_anchor(s: &str) -> Option<(usize, u8)>` — parse `N:HH`; tolerate an optional `|text` suffix (ignored here, used by the caller for verification/fallback).
- `apply(content: &str, ops: &[AnchorOp]) -> Result<Applied, HashlineError>` where `AnchorOp { line: usize, hash: u8, replacement: String }`: resolve each op to a line (exact line N first; else proximity-search nearby lines for one hashing to HH), reject on mismatch returning current re-tagged content in the error, preserve original line endings and encoding. Reuse/port the line-ending detection from `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs` (`LineEnding::detect`) — lift a copy here rather than depending on the tools crate (keep this crate dependency-light: `crc32fast` only).

## Acceptance Criteria
- [ ] `cargo build -p swissarmyhammer-hashline` succeeds; crate is a workspace member with no dependency on `swissarmyhammer-tools`.
- [ ] `hash_line` strips only horizontal whitespace (a re-indented line hashes identically; an interior change hashes differently).
- [ ] `tag("a\nb", 1)` yields `1:HH|a\n2:HH|b` with correct per-line hashes.
- [ ] `parse_anchor("42:a3")` and `parse_anchor("42:a3|text")` both yield `(42, 0xa3)`; non-anchors yield `None`.
- [ ] `apply` resolves an exact-line anchor, finds a drifted anchor by proximity, and rejects a true hash mismatch with re-tagged current content in the error.

## Tests
- [ ] Property tests in `crates/swissarmyhammer-hashline/tests/` (proptest or hand-rolled perturbation): tag→edit→re-tag round-trips; mismatch rejects; proximity finds a drifted anchor; reformatting (re-indentation) preserves anchors.
- [ ] Unit tests for `hash_line`, `parse_anchor`, `tag` edge cases (empty content, trailing newline, CRLF).
- [ ] `cargo test -p swissarmyhammer-hashline` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-06-23 12:56)

> ⚠️ 1/15 review tasks failed — results are INCOMPLETE.

### Nits
- [x] `crates/swissarmyhammer-hashline/src/lib.rs:1` — Crate-level documentation should include a usage example. The current docs explain what hashline anchors are and how they work conceptually, but don't show code demonstrating the common workflow. Add a code example in the crate-level doc comment showing: tag(content, 1), parse_anchor() on a tagged line, construct an AnchorOp, and call apply(). This makes the module immediately usable without reading tests.

## Review Findings (2026-06-23 13:09)

### Warnings
- [x] `crates/swissarmyhammer-hashline/src/lib.rs:29` — Crate-level example uses .unwrap() and .expect() instead of the ? operator, teaching incorrect error handling patterns. Examples must demonstrate idiomatic Rust. Wrap the example in a function that returns Result: `fn main() -> Result<(), Box<dyn std::error::Error>> { ... }` and replace unwrap/expect with `?`. For .nth(1), use `.ok_or(...)? ` to convert Option to Result.

### Nits
- [x] `crates/swissarmyhammer-hashline/src/lib.rs:24` — Re-export of LineEnding lacks a doc comment. Public re-exports should have documentation explaining what is being re-exported and why. Add a doc comment explaining the re-export: `/// Line ending mode detection and rendering.
pub use line_ending::LineEnding;`.
- [x] `crates/swissarmyhammer-hashline/src/lib.rs:96` — Hardcoded capacity offset `16` is unexplained. The capacity calculation `content.len() + content.len() / 4 + 16` estimates tagging overhead, but the `16` bytes offset should be a named constant with documentation of its purpose. Extract `16` as a named constant, e.g., `const TAGGING_OVERHEAD_BYTES: usize = 16;` with a doc comment explaining it accounts for anchor prefixes and delimiters.
- [x] `crates/swissarmyhammer-hashline/src/lib.rs:96` — Hardcoded divisor `4` is unexplained. The division `content.len() / 4` estimates a 25% capacity overhead for tagging, but the ratio should be a named constant with documentation. Extract `4` as a named constant, e.g., `const TAGGING_OVERHEAD_RATIO: usize = 4;` (meaning 1/4 or ~25% overhead) with a doc comment explaining the capacity estimation strategy.