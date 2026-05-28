---
assignees:
- claude-code
depends_on:
- 01KSQBEAVG5FCXF3TT411A88Z7
- 01KSQBEQ4XMETVGMBTJW25BDAC
- 01KSQBF4J2WV5XDEVA5QZXY8TV
- 01KSQBFMECY2QGC545BRXGR3JT
- 01KSQBG2EW2HNHQ911SHN6G6YK
- 01KSQBGPHT216JC640GNAA5NRA
position_column: todo
position_ordinal: 8f80
project: llama-coverage
title: Add a CI coverage gate for llama-agent so the bar can't regress
---
## What

Once the coverage cards land, lock the bar in so a future change can't silently drop back to the state that let the 0-token bug ship. Add a CI coverage gate for `llama-agent`.

## Steps

1. Add a CI step (in the existing CI provider — check `.github/workflows` or whatever the `ci` skill detects) that runs `cargo llvm-cov --package llama-agent` and fails the build if region/line coverage drops below a threshold.
2. Set the threshold from the achieved post-epic number minus a small margin (e.g. if the epic reaches 94%, gate at 90%). Do NOT set it at 100% — that invites coverage-gaming and flaky exclusions. Gate the behavior-critical modules higher (generation, queue, stopper, acp translation/server) if the tool supports per-path thresholds.
3. Document any deliberate exclusions (the real-model FFI decode in `model.rs` is the legitimate one — it needs a real model and is covered by the small real-model smoke tests, not unit coverage) with `#[cfg]` / coverage-ignore annotations and a comment explaining why.
4. Make sure the gate does NOT require downloading the 27B model — the unit coverage runs on scripted-model tests + the small qwen-0.6b smoke tests only.

## Acceptance Criteria

- [ ] CI fails when `llama-agent` coverage drops below the threshold.
- [ ] The threshold is recorded and justified (achieved % minus margin).
- [ ] Legitimate exclusions (real-model FFI) are annotated and explained, not silently dropped.
- [ ] The gate runs without the 27B model download.

## Tests

- [ ] Demonstrate the gate: temporarily delete a covered test locally and confirm the coverage step fails; restore it.
- [ ] Run the CI command locally: `cargo llvm-cov --package llama-agent --fail-under-lines <threshold>` (or tool equivalent) exits non-zero below the bar, zero above.

## Workflow

- Use the `ci` skill to find and modify the right workflow file.
- Final card of the epic — depends on all the coverage cards landing first.