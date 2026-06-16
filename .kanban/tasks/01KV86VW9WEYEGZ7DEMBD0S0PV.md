---
assignees:
- claude-code
position_column: todo
position_ordinal: a580
project: diagnostics
title: Settle/debounce engine for diagnostics quiescence
---
## What
Language servers re-flow diagnostics as they analyze; never report mid-analysis state. In `swissarmyhammer-diagnostics`, add a settle engine that subscribes to the session's diagnostics fan-out and waits for a quiescence window before emitting a settled set; on timeout it returns `pending` as a backstop for pathological analysis only.

- `settle(uris, config) -> SettleOutcome { Settled(Vec<DiagnosticRecord>) | Pending }`: subscribe to the per-uri broadcast, reset a debounce timer on each update, emit when no update arrives within `settle_window`, or `Pending` after a hard timeout.
- Generous settle by default (a few seconds in-tool beats an extra model turn — see design "do more per call").
- Pure async logic driven by the fan-out channel; the timer source must be injectable so tests are deterministic and fast.

## Depends on
- "Capture publishDiagnostics and add in-process fan-out in swissarmyhammer-lsp" (the subscribe source)
- "Create swissarmyhammer-diagnostics crate: report types, config, lsp_types mapping" (record/config types)

## Acceptance Criteria
- [ ] `settle()` emits only the settled diagnostic set after a quiescence window; never an intermediate re-flow.
- [ ] Hard timeout yields `Pending`.
- [ ] Timer/clock injectable for deterministic tests.

## Tests
- [ ] `cargo test -p swissarmyhammer-diagnostics`: scripted revision stream (e.g. 3 rapid updates then quiet) asserts only the final settled set is emitted; a never-quiescing stream asserts `Pending` at timeout. Model-free, uses a fake clock, <1s.

## Workflow
- Use `/tdd` — write the scripted-revision-stream test first. #diagnostics