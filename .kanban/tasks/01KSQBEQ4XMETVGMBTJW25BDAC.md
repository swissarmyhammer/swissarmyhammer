---
assignees:
- claude-code
depends_on:
- 01KSQBCTMV4K3ATFZ5RFQ0FJBB
position_column: todo
position_ordinal: 8a80
project: llama-coverage
title: Cover stop conditions (stopper/eos.rs, max_tokens.rs, mod.rs) — pure logic, no model
---
## What

The `crates/llama-agent/src/stopper/` module decides when generation halts. It is pure logic — no model needed — and a stop bug silently truncates or runs away. Cover it exhaustively.

## Cover

- `stopper/eos.rs` — EOS token detection: the EOS id, a non-EOS id, and any model-specific alternate end tokens.
- `stopper/max_tokens.rs` — boundary: stops at exactly N, not N-1, not N+1.
- `stopper/mod.rs` — the composite/dispatch: when multiple stoppers are active, the first to fire wins; ordering and precedence.
- **Stop sequence straddling a chunk boundary** — if stop-string matching exists, a stop sequence split across two decode steps must still be detected. (If string-stop lives elsewhere, note where and cover it there.)

## Acceptance Criteria

- [ ] Each stopper type has explicit boundary tests (fires / does-not-fire at the edges).
- [ ] The composite stopper's precedence is pinned.
- [ ] `stopper/` region coverage reaches the epic threshold (target >95%).
- [ ] No real model used — these are pure predicate tests.

## Tests

- [ ] Unit tests colocated in each `stopper/*.rs` `#[cfg(test)]` module or a `stopper/tests.rs`.
- [ ] Run: `cargo test -p llama-agent stopper` and confirm the coverage delta for `stopper/`.

## Workflow

- Use `/tdd`. This card is independent of the scripted-model harness — pure predicates.