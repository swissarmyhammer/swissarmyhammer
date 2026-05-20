---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
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

- [ ] One new test in `crates/swissarmyhammer-plugin/tests/hot_reload_e2e.rs` (or a sibling file) that drives a real v1→v2 source rewrite via the watcher and asserts on the in-flight error variants.
- [ ] The test uses real `PluginHost`, real example bundles staged under a `TempDir`, real watcher — no test backdoors on the reload path itself.
- [ ] The assertion is "at least one observed `PluginReloaded`" — not "exactly one" or "all calls during the window" — to remain stable against the watcher's natural timing variance.
- [ ] Every async boundary timeouts under `support::TIMEOUT` or a local equivalent — no naked awaits, no unbounded loops without a deadline.
- [ ] Update the coverage matrix in the `hot_reload_e2e.rs` module docstring to point edge case 1 at BOTH the existing backdoor-based test and this new end-to-end test.

## Tests

- [ ] `cargo nextest run -p swissarmyhammer-plugin --test hot_reload_e2e` — green, including the new test.
- [ ] `cargo nextest run -p swissarmyhammer-plugin` — full plugin suite still green.
- [ ] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` — clean.

## Workflow

- Use `/tdd` — write the test first observing the actual behavior, only adjust if it diverges from the platform.
- Iterate the call-loop tightness and the window-detection deadline until the test hits the window reliably (say, ≥ 95% of runs locally) without becoming slow. A 1-2 ms sleep between calls and a 5-second deadline are reasonable starting points.