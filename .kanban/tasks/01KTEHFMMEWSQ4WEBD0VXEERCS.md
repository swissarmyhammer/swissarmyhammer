---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8380
project: local-review
title: 'Review observability: log which validators + rules were selected/run; surface validator load/parse failures in check validators + doctor'
---
## What
When you run the `review` MCP tool there is currently no way to tell, from the outside, **which validators matched the scope and which validators/rules actually ran**. And a related gap found while verifying user-level loading: a user validator that fails to parse (e.g. a fat-fingered `VALIDATOR.md`) is **silently dropped** — the loader emits a buried `WARN` and skips it, and `check validators` / `sah doctor` still report `N loaded, all valid`. Both are observability problems: you can't tell what the engine selected, ran, or rejected.

(Verified empirically: a correctly-formatted user validator in `~/.validators/` DOES load — `sah doctor` count went 18→19 — but two malformed user validators were dropped with no surfaced error, doctor still said "18, all valid".)

## Part A — log selected/run validators + rules (the primary ask)
Add INFO-level `tracing` (the established pattern; `review::fleet` already uses `tracing::info!` for batching) at the key engine stages so a `review` run makes the selection/execution legible. Use structured fields, not just prose.

- **scope (`crates/swissarmyhammer-validators/src/review/scope.rs`)**: after building the `WorkList`, log the change purpose source, the list of **matched validator names**, and per validator the **file count** and **declared probes**. One concise summary line (e.g. `tracing::info!(validators = ?names, files = n, "review scope resolved")`) plus a per-validator debug line.
- **fleet (`.../review/fleet.rs`)**: for each submitted `(validator, batch)` task log the validator name, the files in the batch, and the **rule names** being applied (the rules come from `loader.get_ruleset`), plus the existing batching info. So the log shows exactly which validator×file×rules ran.
- **verify (`.../review/verify.rs`)**: log guard auto-refutes (which finding, which `fact` probe) and agent verify submissions/verdicts at INFO/DEBUG.
- **synthesize / run_review (`.../review/synthesize.rs`)**: log the final counts (blockers/warnings/nits/confirmed/refuted) and the total validators/files/tasks run.
- **review tool (`crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs`)**: ensure these traces fire on the real tool path (the tool runs the pipeline inside spawn_blocking — confirm the tracing subscriber/context propagates so the lines actually appear in `sah serve` logs). If the MCP server has a logging-notification channel, that's a nice-to-have, but tracing at INFO is the requirement.

Pick levels so a normal `review` run at default log level shows the **selection summary + which validators/rules ran** without drowning in per-token noise (summary lines INFO; per-rule/per-finding detail DEBUG).

## Part B — surface load/parse failures (the gap found in testing)
Make malformed validators visible instead of silently dropped.

- The loader (`crates/swissarmyhammer-validators/src/validators/loader.rs`) currently logs `WARN ... Failed to parse RuleSet` and skips. Collect those parse failures (path + error) during `load_all` so they can be reported, rather than only logged.
- `check validators` (the engine lint in `crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs` and/or the loader's `diagnostics`) must include load/parse failures as errors: each dropped validator → an error naming its path + the parse problem + a fix hint. So `sah doctor` shows `Error  Validator <path>  <problem>` instead of `N loaded, all valid` when a user validator is broken.
- Keep the existing behavior that a broken validator doesn't crash the run — it's reported, the rest still load.

## Acceptance Criteria
- [ ] Running `review working` (or the e2e/integration path) emits INFO `tracing` that lists the matched validator names, file counts, and the validator×rule combinations actually run — verifiable in a test that captures tracing output.
- [ ] The traces fire on the real `review` tool path (not just direct engine calls) — propagation through spawn_blocking confirmed.
- [ ] A malformed validator (bad frontmatter / unknown probe) in the user or project dir is surfaced by `check validators` / `sah doctor` as a named Error with a fix, NOT silently dropped; `doctor` no longer reports `all valid` when a validator failed to load.
- [ ] A valid user validator still loads and is counted; the run is never aborted by one broken validator (it's reported, others proceed).
- [ ] `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools` green; build + clippy clean.

## Tests
- [ ] Tracing-capture test (e.g. `tracing-test` / a subscriber) over a small `WorkList` → asserts the selection summary + per-validator rule lines are emitted with the right validator/rule names.
- [ ] check-validators/doctor test: a temp user dir (`~/.validators` via temp HOME) or project `./.validators` with one malformed validator → `check validators` returns an Error naming it; a valid one alongside still loads.
- [ ] `cargo test` green for the owning crates.

## Workflow
- Use `/tdd` for the parse-failure-surfacing (assert the Error is reported) and the tracing-capture assertions. Reuse the existing `tracing` usage in `review::fleet` as the pattern; do not invent a new logging framework. Keep INFO concise (summary) and put detail at DEBUG.