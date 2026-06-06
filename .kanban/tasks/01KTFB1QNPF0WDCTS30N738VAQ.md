---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8580
project: local-review
title: 'Fix: real-agent review found nothing — required `validator` field dropped every finding; `validators` subset ignored'
---
## What (found by manually shelling out to real `claude`)
Drove the `review` MCP tool through a real `sah serve --model claude-code` against a temp repo with a planted dead function + duplicate. Two bugs that the scripted-agent e2e masked:

1. **CRITICAL — every real-agent finding was silently dropped.** `Finding.validator` was a required (no `#[serde(default)]`) field, but the fan-out OUTPUT_CONTRACT never asks the agent to emit `validator` (the engine knows the shard and re-tags via `fleet::tag_findings`). Real claude follows the contract and omits it, so `parse_findings` failed with `missing field 'validator'` and the WHOLE batch degraded to zero findings. Live proof: a review of the planted repo returned `0 blockers / 0 confirmed / 0 refuted` with `WARN fleet task response did not parse ... error=missing field 'validator'`. The scripted e2e agent serialized `Finding` structs (which include `validator`), so it never reproduced a real agent's output.
   - Fix: `#[serde(default)]` on `Finding::validator`; the fleet re-tag fills the authoritative name. Regression test `parse_findings_tolerates_a_finding_without_validator`.

2. **`validators` subset modifier was ignored.** `review_op.rs` did `let _ = &request.validators; // later refinement` — so `validators=["dead-code"]` ran the full matching set (reuse, rust, …) anyway.
   - Fix: `ValidatorLoader::retain_rulesets(&names)` + wired into `run_review_request_inner` (empty = all). Regression test `retain_rulesets_keeps_only_the_named_subset`.

## Proof it works now
After both fixes, a real-claude `review working` scoped to `dead-code` returned: `### Blockers - [ ] lib.rs:5 — Private function never_called_helper has no inbound callers ... dead code ...`, counts `blockers:1, confirmed:1, refuted:0`. (Before: 0/0.) Parse failures went 2 → 0.

## Acceptance Criteria
- [x] `Finding.validator` is `#[serde(default)]`; a contract-shaped agent response (no `validator`) parses and is engine-tagged.
- [x] `validators` subset is honored via `retain_rulesets`; empty = all.
- [x] Real `claude` `review working` produces a confirmed finding end-to-end (verified by hand).
- [x] `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools` green; clippy clean.