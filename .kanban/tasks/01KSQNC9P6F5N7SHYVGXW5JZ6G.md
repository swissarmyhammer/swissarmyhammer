---
assignees:
- claude-code
depends_on:
- 01KSQBDM9M4RJJYGQDTZYJA107
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffba80
project: llama-coverage
title: 'Bug+dedup: stale duplicate streaming loop in generator.rs carries the fixed 0-token bugs (0% covered)'
---
## What

The streaming token-budget / chunk-accounting bugs fixed in production (commit `16f7aad5a`, bug `01KSNJ7CBK9333J0T9G4TCA7DH`) existed in a **second, stale copy** of the streaming loop that the production queue path does NOT use — `crates/llama-agent/src/generation/generator.rs::generate_stream_with_context` (~lines 704-885), the `TextGenerator` impl on `LlamaCppGenerator`. Baseline coverage measured this file at **0.00%**, so nothing exercised it.

### Resolution: DELETED (fully dead code)

Investigation confirmed `LlamaCppGenerator` is **never constructed** anywhere — production or tests. Verified via call graph + grep:
- `LlamaCppGenerator::new` appears only in a doc comment (mod.rs), never as a real construction.
- The only `TextGenerator` implementors were `LlamaCppGenerator` (dead) and `ScriptedModel` (test double). The production path uses `GenerationHelper` free functions in `generation/mod.rs`, not the `TextGenerator` trait over a `LlamaCppGenerator`.
- generator.rs had no tests and 0 external references to any of its methods.

Because it was a buggy, unused, drifting copy, the entire `generator.rs` (1275 lines) was deleted rather than fixed. This collapses to ONE canonical streaming loop: `GenerationHelper` in `generation/mod.rs`. A code note was added to the `mod.rs` module doc stating `GenerationHelper` is canonical and warning against reintroducing a parallel hand-copied loop.

### Files changed
- DELETED `crates/llama-agent/src/generation/generator.rs`
- `crates/llama-agent/src/generation/mod.rs` — removed `pub mod generator;` + `pub use generator::LlamaCppGenerator;`; added canonical-loop note; updated module doc example.
- `crates/llama-agent/src/lib.rs` — removed `LlamaCppGenerator` from the re-export.
- `crates/llama-agent/src/generation/scripted.rs` — updated doc comments that linked to the removed `LlamaCppGenerator` (now point to `GenerationHelper`).

No files owned by the concurrent agents (queue.rs, acp/server.rs, acp/agent.rs, acp/session.rs) were touched.

## Acceptance Criteria

- [x] `generator.rs` no longer contains an independent streaming loop with the double-push / cumulative-count / gated-send bugs — deleted (it was dead).
- [x] Deleted: confirmed nothing references it; the crate builds and all tests pass.
- [x] (Kept branch N/A — code was dead, so deletion path taken.)
- [x] A short note in the code documents which streaming loop is canonical (`GenerationHelper` in mod.rs), so future copies aren't made.

## Tests

- [x] Deleted: `cargo test -p llama-agent` green with the dead code removed (1274 tests: 906 + 19 + 86 + 225 unit/integration + 38 doctests, 0 failures).
- [x] Run: `cargo build -p llama-agent` (ok), `cargo clippy -p llama-agent --all-targets -- -D warnings` (ok, no warnings).

## Workflow

- Used code_context call graph + grep to settle the dead-vs-reachable question before touching code.
- Lineage: found during the llama-coverage keystone review (`01KSQBDM9M4RJJYGQDTZYJA107`); same bug family as `01KSNJ7CBK9333J0T9G4TCA7DH`.