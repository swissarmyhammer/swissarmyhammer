---
assignees:
- claude-code
position_column: todo
position_ordinal: a480
project: kv-prefix-reuse
title: Cover MTP draft-KV reuse on the prime donor (no per-turn MTP skip on recurrent)
---
## What
The selector keystone fixes the TARGET context's donor choice, but the MTP draft context is trimmed separately in `apply_draft_kv_state` (`crates/llama-agent/src/queue.rs:2708`): `draft_ctx.clear_kv_cache_seq(Some(0), Some(offset as u32), None)` at `queue.rs:2757`, with the same `Ok(false)` → `skipping MTP this turn` fallback at `queue.rs:2760-2767`. The code comment there notes the draft rollback distance is "the prior turn's generation length," which on the recurrent model exceeds `n_rs_seq=64` — so MTP speculative decoding could be lost on every sibling review turn even after the target-side fix.

Investigate and resolve:
- The draft state bytes travel WITH the chosen donor (`draft_state_bytes` in the same `PrefixMatch`). Confirm whether, once the target selects the zero-rollback PINNED PRIME donor, the prime's draft snapshot also ends at the prime PROMPT boundary (it should, since `on_prefill_complete` fires for both target and draft in `crates/llama-agent/src/generation/mtp/streaming.rs:140`). If so, the draft trim offset == prime draft len → rollback 0 and MTP survives "for free" — in which case this task is a regression test that proves it.
- If the prime's draft snapshot does NOT end at the prompt boundary (so the draft trim still rolls back > 64), extend rollback-awareness / boundary-snapshotting to the draft path so MTP is retained.

## Acceptance Criteria
- [ ] Determined (with evidence from code + the real-model test) whether MTP is retained or skipped on sibling turns once the prime donor is selected.
- [ ] If retained: a test asserts `skipping MTP this turn` does NOT fire on sibling turns. If not retained: the draft path is fixed so it doesn't, and the same test passes.

## Tests
- [ ] Extend the real-model integration test (kv_prefix_reuse_recurrent.rs) to assert no `skipping MTP this turn` WARN on sibling turns with MTP active, OR add a focused unit test on the draft-offset rollback computation if a pure seam exists.
- [ ] `cargo test -p llama-agent` green.

## Depends on (prose — kanban depends_on edges currently dropped by a known bug): keystone 01KVBK83218VM915ZTVZCKZ9VA; shares the real-model harness with 01KVBK9JMPNKK1RTSE4CRM896P.

## Workflow
- Use `/tdd` — first measure (assert current behavior), then fix only if the measurement shows MTP is being dropped.