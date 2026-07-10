---
assignees:
- claude-code
depends_on:
- 01KS0416MQYVFSQFZMM2E9VAVX
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9480
project: plugin-tsonly
title: Migrate remaining tests + support harness off inline plugin.json
---
## What

The remaining integration tests and the shared test harness build plugin bundles by writing a `plugin.json` + `entry.ts` literal. Migrate each to the TS-only layout: an `index.ts` entry only, no `plugin.json`, identity = bundle directory name.

Files:
- `crates/swissarmyhammer-plugin/tests/discovery.rs`
- `crates/swissarmyhammer-plugin/tests/hot_reload.rs`
- `crates/swissarmyhammer-plugin/tests/module_loader.rs`
- `crates/swissarmyhammer-plugin/tests/callbacks.rs`
- `crates/swissarmyhammer-plugin/tests/plugin_host.rs`
- `crates/swissarmyhammer-plugin/tests/support/mod.rs`

Note: `tests/sdk.rs` is intentionally NOT in this list — its `plugin.json` handling is covered by the SDK `name`/`version` task; if `sdk.rs` writes a bundle, migrate it there or here, but do not double-handle.

For each: replace the `plugin.json` + `entry.ts` writing with an `index.ts`-only writer; drop `id`/`entry`/`provides`; switch any plugin-id expectation to the bundle directory name. `tests/support/mod.rs` — its `stage_example`/`copy_dir_recursive` already copy whatever files a bundle contains, so once task 3 migrates the committed bundles `stage_example` needs no change; only update any helper here that *constructs* a `plugin.json` or doc-comments referring to `entry.ts`/the manifest.

Keep every test's verified behavior identical — this is a mechanical layout migration, not a behavior change.

## Acceptance Criteria
- [x] None of the listed files writes or constructs a `plugin.json`; bundles are `index.ts`-only.
- [x] Plugin-id expectations use the bundle directory name.
- [x] No `provides` declarations remain in these tests' bundle setup.
- [x] `tests/support/mod.rs` has no stale `plugin.json`/`entry.ts`/manifest references in code or doc comments.
- [x] Each test verifies the same behavior as before.

## Tests
- [x] `cargo nextest run -p swissarmyhammer-plugin --test discovery --test hot_reload --test module_loader --test callbacks --test plugin_host` — all pass (32/32).
- [x] `cargo nextest run -p swissarmyhammer-plugin` — full crate green (143/143; every test binary that uses `support` still compiles and passes).
- [x] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` — clean.

## Workflow
- Use `/tdd` — convert the bundle-writing helpers, run each affected test red→green.

## Implementation notes

Two design forks the description did not resolve were elicited from the user; the user's answer was "no manifests at all — index.ts (or .js) is the plugin."

1. **Manifest-only tests deleted, not migrated.** Several `discovery.rs`/`hot_reload.rs` tests verified behavior that has no meaning without a manifest, so they were deleted rather than migrated: `discovery.rs::register_of_a_name_absent_from_provides_is_rejected`, `discovery.rs::a_manifest_entry_escaping_the_bundle_is_rejected`, `discovery.rs::provides_colliding_with_a_reserved_host_name_is_rejected`, `hot_reload.rs::a_provides_expansion_is_gated_by_the_reload_policy`. `discovery.rs::a_failed_discovery_scan_rolls_back_already_loaded_plugins` previously forced its mid-scan failure via a `provides` violation; it now fails by having the bad bundle's `load()` throw — same rolled-back-scan behavior verified. The unused `DenyProvidesExpansion` import was dropped from `hot_reload.rs`.

2. **`host.load()` now resolves `index.ts`/`index.js` for manifest-less bundles.** `callbacks.rs`/`module_loader.rs`/`plugin_host.rs` never wrote a `plugin.json` — they used the legacy bare-`entry.ts` convention. `module_loader.rs`/`callbacks.rs` pass the entry filename explicitly to `call_plugin_lifecycle`, so renaming to `index.ts` was free. `plugin_host.rs` uses `host.load(plugin_dir)`, whose manifest-less arm previously hardcoded `ENTRY_FILE = "entry.ts"`. To make every bundle `index.ts`-only end to end, `host.load()` now resolves a manifest-less bundle's entry via the shared `discovery::resolve_index_entry` (made `pub(crate)`) — `index.ts` then `index.js`, canonicalized and containment-checked — and the `ENTRY_FILE` const was removed. One out-of-scope test outside the task's file list, `crates/swissarmyhammer-tools/tests/plugin_module_exposure_test.rs`, also used `host.load()` with an `entry.ts` bundle and was updated to `index.ts` so it stays green. Stale `entry.ts`/legacy-bundle doc comments in `host.rs` and `callback_e2e.rs` were corrected. The `Manifest`/`provides` machinery itself is untouched — its removal is the next task (01KS044HKZQ1V002TYQCZGC56Z).