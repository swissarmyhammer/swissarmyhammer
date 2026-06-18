---
assignees:
- claude-code
position_column: todo
position_ordinal: a180
project: kv-prefix-reuse
title: Verify fork-chain reuse end-to-end; add only the minimal restore code the RED test proves necessary
---
## What
TEST-FIRST, de-risked. Adversarial review found the existing rollback-aware selector (zckz9va) likely ALREADY performs a zero-rollback fork restore once token-prefix stability (309wyrm) holds: a forked child's own cache entry is keyed by the child id and carries the parent's state + the parent's prompt-token fingerprint (`SessionStateStore::fork`, `queue.rs:407-411,422-454`). If the child tokens extend the parent tokens, `find_best_prefix_match(child_id, child_tokens)` yields `lcp == donor_len`, `rollback == 0` → passes the ≤64 gate → wins → `streaming_reuse_decision` returns `Some(donor_len)` → trim clears an empty range → forward-only restore. So this card may collapse to "regression test + docs."

DO THIS, in order:
1. Write the RED real-model fork-chain test (below) and run it with ONLY 309wyrm + the shipped selector landed.
2. If GREEN: this card is the test + documentation of the mechanism. Done. Do NOT add a redundant restore path.
3. If still RED: diagnose with the live-path evidence and add the MINIMAL change needed, choosing explicitly:
   - (a) If the child's own aliased entry isn't being selected though tokens extend the parent → fix selection/aliasing.
   - (b) If decode-time lineage is genuinely needed → thread the parent session id through `GenerationRequest` (`crates/llama-agent/src/types/generation.rs`) and `QueuedRequest` (`queue.rs:~1158`) the SAME way `pin_on_save` is threaded via ACP `_meta` (no `parent_session_id` field exists today — that wiring is the work), and prefer the named parent in selection. Only build this if the test proves it necessary.

Edge case to cover regardless: empty/near-empty suffix. `streaming_reuse_decision` returns `None` when `lcp >= new_len` (`queue.rs` ~2895) → full reprocess. A forked child whose suffix renders to zero new tokens must be handled as "fully cached, decode nothing new," not a cold reprocess.

General capability — keyed on fork lineage / token-prefix extension, NOT the review tool.

## Acceptance Criteria
- [ ] Gated real-model fork-chain test passes: prime a parent, fork ≥2 children with suffixes > 64 tokens via ACTUAL `session/fork`, each child reuses the full parent prefix at rollback 0 (no `seq_rm` failure, no full reprocess).
- [ ] Non-forked sessions unchanged (LCP path intact).
- [ ] Empty/near-empty suffix on the fork path does not cause a cold reprocess.
- [ ] Whatever code (if any) was added is the minimum the RED test required; if none was needed, that is documented.

## Tests
- [ ] Extend `crates/llama-agent/tests/integration/kv_prefix_reuse_recurrent.rs` to use real `session/fork` (not hand-built donors). RED before 309wyrm/this card, GREEN after.
- [ ] MTP on fork turns (corrected, NON-over-strict AC): assert `skipping MTP this turn` does NOT fire on fork turns *when the prime turn executed MTP*. Note explicitly: a prime that did not run MTP saves no draft, so the first fork legitimately cold-starts the draft — do not assert 0 skips in that case. (`apply_draft_kv_state` aliases parent `draft_state_bytes` at `queue.rs:443`; trim to `offset==donor_len` is a 0-distance trim that succeeds only if the parent draft length == donor_len.)
- [ ] `cargo test -p llama-agent --lib` green; clippy -D warnings clean; gated test passes on GPU.

## Depends on (prose — kanban depends_on edges dropped by a known bug): 309wyrm (token-prefix stability) MUST land first — it may be the entire fix. Builds on shipped zckz9va + x3181nx.

## Workflow
- Use `/tdd` — RED fork-chain test first; land 309wyrm; re-run; add minimal code ONLY if still RED.