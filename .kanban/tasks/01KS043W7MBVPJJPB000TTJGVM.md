---
assignees:
- claude-code
depends_on:
- 01KS0416MQYVFSQFZMM2E9VAVX
position_column: todo
position_ordinal: '8580'
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
- [ ] None of the listed files writes or constructs a `plugin.json`; bundles are `index.ts`-only.
- [ ] Plugin-id expectations use the bundle directory name.
- [ ] No `provides` declarations remain in these tests' bundle setup.
- [ ] `tests/support/mod.rs` has no stale `plugin.json`/`entry.ts`/manifest references in code or doc comments.
- [ ] Each test verifies the same behavior as before.

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-plugin --test discovery --test hot_reload --test module_loader --test callbacks --test plugin_host` — all pass.
- [ ] `cargo nextest run -p swissarmyhammer-plugin` — full crate green (every test binary that uses `support` still compiles and passes).
- [ ] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` — clean.

## Workflow
- Use `/tdd` — convert the bundle-writing helpers, run each affected test red→green.