---
assignees:
- claude-code
depends_on:
- 01KS0416MQYVFSQFZMM2E9VAVX
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9380
project: plugin-tsonly
title: Migrate e2e tests off inline plugin.json (transport/dispatch group)
---
## What

These integration tests build throwaway plugin bundles at runtime by writing a `plugin.json` + `entry.ts` string literal into a temp dir. Migrate each to the TS-only layout: write an `index.ts` only, no `plugin.json`. Discovery (task 1) loads such bundles; identity is the temp directory name, so each test must use / assert the directory name as the plugin id rather than a manifest `id`.

Files (each has its own `write_*_plugin`-style helper to convert):
- `crates/swissarmyhammer-plugin/tests/files_dispatch_e2e.rs`
- `crates/swissarmyhammer-plugin/tests/operation_meta_e2e.rs`
- `crates/swissarmyhammer-plugin/tests/callback_e2e.rs`
- `crates/swissarmyhammer-plugin/tests/cli_server_e2e.rs`
- `crates/swissarmyhammer-plugin/tests/url_server_e2e.rs`
- `crates/swissarmyhammer-plugin/tests/discovery_layering_e2e.rs`
- `crates/swissarmyhammer-plugin/tests/hot_reload_e2e.rs`
- `crates/swissarmyhammer-plugin/tests/unload_disposal_e2e.rs`
- `crates/swissarmyhammer-plugin/tests/failed_load_e2e.rs`

For each: drop the `plugin.json` write; write the entry as `index.ts`; remove `provides`/`id`/`entry` from the bundle setup. Where a test asserts on a plugin id, switch the expectation to the temp bundle directory name. `discovery_layering_e2e.rs` keys two layer copies by a shared id — that shared identity now comes from giving both copies the same **directory name** in their respective layer roots; keep the test's intent (project copy shadows user copy).

This is mechanical and per-file; the test *logic* and assertions stay the same except for the id-source change. Do NOT change behavior the tests verify.

## Acceptance Criteria
- [x] None of the nine files writes a `plugin.json`; each writes only an `index.ts` entry.
- [x] Each test's plugin-id expectations use the bundle directory name.
- [x] No `provides` declarations remain in these tests' bundle setup.
- [x] Every test still verifies the same behavior it verified before (transports, dispatch, callbacks, layering, hot reload, unload, failed load).

## Tests
- [x] `cargo nextest run -p swissarmyhammer-plugin --test files_dispatch_e2e --test operation_meta_e2e --test callback_e2e --test cli_server_e2e --test url_server_e2e --test discovery_layering_e2e --test hot_reload_e2e --test unload_disposal_e2e --test failed_load_e2e` — all pass.
- [x] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` — clean.

## Workflow
- Use `/tdd` — these tests ARE the spec; convert the bundle-writing helper, run each test red→green.

## Implementation notes
- `callback_e2e.rs` and `unload_disposal_e2e.rs` bypass `discover_and_load_all`. Per user decision (option B), both were migrated to `index.ts` uniformly. `callback_e2e.rs` calls `runtime.call_plugin_lifecycle` directly — its entry argument was updated to `"index.ts"`. `unload_disposal_e2e.rs` previously called `host.load(bundle.path())`, whose manifest-less by-path path hardcodes `entry.ts`; it was switched to stage the bundle under `<layer_root>/plugins/weather-probe/` and load via `discover_and_load_all`, taking the returned `PluginId` from the loaded vec.
- `hot_reload_e2e.rs`: the `write_manifest` helper was deleted entirely — a manifest-less bundle declares no `provides`, so a v1→v2 reload can never be a `provides` expansion (per `provides_expansion()` in host.rs). Doc comments updated to reflect this.
- Doc comments in all nine files were updated to describe the manifest-less / `index.ts` shape instead of `plugin.json` + `entry.ts`.