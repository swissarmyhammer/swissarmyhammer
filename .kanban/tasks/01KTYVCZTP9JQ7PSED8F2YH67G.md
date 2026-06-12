---
assignees:
- claude-code
position_column: todo
position_ordinal: 9d80
project: local-review
title: 'eval(review): primed-prefix forks for verify turns — measured prefix share ~<10%, deferred'
---
## Context

Task 01KTY91Y7AJRPJNBCVTV59HCJJ moved the fleet stage to primed prefix sessions + forks. The same checklist asked whether verify turns (147–161 candidates/review sharing verification instructions) should use the structure too.

## Measurement (2026-06-12)

Verify prompts (`render_verify_prompt` in crates/swissarmyhammer-validators/src/review/verify.rs) share only:
- the adversary header: ~280 bytes BEFORE the first per-candidate byte
- `VERIFY_OUTPUT_CONTRACT`: ~420 bytes, at the END of the prompt (not a shared prefix without reordering)

Everything else — the claim block, the file's bounded `source_slice`, the probe evidence — is per-candidate. Even reordering the contract to the front yields ~700 shared bytes (~175 tokens) of a typically multi-thousand-token prompt: under ~10% prefix share, vs ~80–90% for fleet prompts.

## Decision

Skipped: a prime turn plus fork/status/pin round-trips per review costs more than the ~175-token reuse saves. Rationale documented in verify.rs module docs ("Why verify does NOT use primed prefix sessions").

## Revisit if

- verify prompts grow a large shared instruction block (e.g. per-validator verification rubrics), or
- real-model log evidence shows verify turns paying significant cold prefill that a shared prefix would cover.