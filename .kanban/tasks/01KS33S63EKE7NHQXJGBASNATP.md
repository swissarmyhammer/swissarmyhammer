---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9a80
project: plugin-arch
title: 'Test: in-flight call hits PluginReloaded through real reload_active'
---
## What

Follow-up captured by the review of `01KS2XB706GTW5F5JB17FC9XXB`. The existing edge-case-1 test (`hot_reload_e2e::an_in_flight_call_rejects_with_plugin_reloaded_when_the_isolate_is_disposed`) stages the reload window through the `mark_reloading_for_test` / `clear_reloading_for_test` test backdoors, exercising the registry-resolve → host-route → `PluginReloaded` translation. That isolates the contract from watcher-debounce timing, which is the right scope for a unit-level pin, but it does not verify that the production `reload_active` path actually stages and clears markers correctly.

Add a probabilistic-but-bounded test that:

1. Starts the watcher.
2. Spawns a background task that issues `host.call(..., "behavior-a", ...)` calls in a tight loop with a short between-call sleep, recording each result.
3. Rewrites the bundle's `index.ts` on disk (the `write_version(plugin_dir, "behavior-b", "mod-b")` pattern).
4. Waits until either (a) the registry transitions to `behavior-b`, or (b) the deadline expires.
5. Asserts that at least one call during the reload window saw `Error::PluginReloaded` — NOT `UnknownServer` or `ServerUnavailable`.

The "at least one" assertion is the honest claim: hitting the window is timing-dependent on the watcher debounce + isolate teardown latency, but over hundreds of calls in the millisecond range, the probability of zero hits is vanishingly small if the markers are plumbed correctly. A regression that broke the staging would show zero `PluginReloaded` results across the whole window.

## Acceptance Criteria

- [x] One new test in `crates/swissarmyhammer-plugin/tests/hot_reload_e2e.rs` (or a sibling file) that drives a real v1→v2 source rewrite via the watcher and asserts on the in-flight error variants.
- [x] The test uses real `PluginHost`, real example bundles staged under a `TempDir`, real watcher — no test backdoors on the reload path itself.
- [x] The assertion is "at least one observed `PluginReloaded`" — not "exactly one" or "all calls during the window" — to remain stable against the watcher's natural timing variance.
- [x] Every async boundary timeouts under `support::TIMEOUT` or a local equivalent — no naked awaits, no unbounded loops without a deadline.
- [x] Update the coverage matrix in the `hot_reload_e2e.rs` module docstring to point edge case 1 at BOTH the existing backdoor-based test and this new end-to-end test.

## Tests

- [x] `cargo nextest run -p swissarmyhammer-plugin --test hot_reload_e2e` — green, including the new test.
- [x] `cargo nextest run -p swissarmyhammer-plugin` — full plugin suite still green.
- [x] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` — clean.

## Workflow

- Use `/tdd` — write the test first observing the actual behavior, only adjust if it diverges from the platform.
- Iterate the call-loop tightness and the window-detection deadline until the test hits the window reliably (say, ≥ 95% of runs locally) without becoming slow. A 1-2 ms sleep between calls and a 5-second deadline are reasonable starting points.

## Outcome

Added `a_watcher_driven_reload_emits_plugin_reloaded_to_an_in_flight_caller` in `crates/swissarmyhammer-plugin/tests/hot_reload_e2e.rs`. The test:

- Stages a probe bundle under a `TempDir` with v1 (`behavior-a` / `mod-a`).
- Starts a real watcher.
- Spawns a background `tokio::task` that calls `host.call(CallerId::HostInternal, "behavior-a", "echo", …)` in a tight loop with 1 ms between calls; the task is controlled by a shared `AtomicBool` stop flag and returns all recorded results when stopped. Every dispatch is bounded by the file-local `TIMEOUT` (no naked awaits).
- Rewrites the bundle's source to v2 (`behavior-b` / `mod-b`) and uses the existing `wait_until_live` helper to wait for v2 to become addressable.
- Asserts (a) v2 is now live and v1 is disposed, and (b) at least one recorded call was `Err(Error::PluginReloaded)` — proof that the production `reload_active` path stages the marker through `ReloadingMarkerGuard::new`.

Updated the coverage-matrix in the module docstring so edge case 1 cites BOTH tests, and rewrote the surrounding paragraph to explain the deterministic/end-to-end split.

Local pass rate: 5/5 isolated runs of the new test + 1 full-binary run + 1 full-crate run all green; clippy clean.