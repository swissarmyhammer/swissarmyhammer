---
assignees:
- claude-code
position_column: todo
position_ordinal: f380
title: 'swissarmyhammer-treesitter::index::tests: HuggingFace network flakes'
---
## Symptom

Multiple tests in `swissarmyhammer-treesitter::index::tests` fail intermittently when HuggingFace returns HTTP 500 while resolving the ANE CoreML model `wballard/Qwen3-Embedding-0.6B-CoreML`.

Observed failures:
- `test_status_after_scan`
- `test_get_nonexistent`
- `test_scan_directory`

Likely the entire module shares the failure mode — any test that goes through `IndexContext::new` + `scan()` is exposed.

This is network-dependent test design — fragile by construction.

## Fix direction

One of:
- Skip the model resolution when offline (and assert reduced behavior)
- Cache the model locally so CI can resolve from disk
- Decouple the status/scan tests from the model resolution path

## Reproduction

`cargo nextest run -p swissarmyhammer-treesitter index::tests::test_status_after_scan` (or any of the affected tests) — passes when HF is up, panics when HF returns 500.

## Acceptance Criteria

- [ ] No test in `swissarmyhammer-treesitter::index::tests` depends on a live HuggingFace fetch.
- [ ] CI can run the workspace offline without panicking.
- [ ] The replacement still asserts something useful about post-scan status.

## Tags

#test-failure