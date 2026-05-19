---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9080
project: plugin-tsonly
title: 'Discovery: index.ts entry convention + directory-name identity'
---
## What

Teach plugin discovery to recognize a manifest-less, TS-only plugin bundle: a `plugins/<dir>/` directory whose entry module is `index.ts` (preferred) or `index.js` — no `plugin.json` required. The plugin's identity is its bundle directory name.

This is **transitional** — `plugin.json` bundles must keep working unchanged so the build and all ~22 existing tests stay green. The `plugin.json` path is removed later (final task of this project).

Design decisions (assert these; flag to the user only if a real conflict appears):
- Identity (`id`) of a manifest-less bundle = the bundle **directory name**.
- Entry = `<dir>/index.ts`; if absent, `<dir>/index.js`.
- A manifest-less plugin has no `provides` — the host's `provides` gate is simply skipped for it (not removed yet).

Files:
- `crates/swissarmyhammer-plugin/src/discovery.rs` — `scan_layer` detects a bundle when the directory contains `plugin.json` OR `index.ts`/`index.js`. `DiscoveredPlugin` must no longer assume a manifest: carry `manifest: Option<Manifest>` plus an explicit resolved `id: String` and `entry: PathBuf`, so downstream code reads those directly. A directory with neither a manifest nor `index.{ts,js}` is still skipped (keep the debug log).
- `crates/swissarmyhammer-plugin/src/host.rs` — where the host resolves a discovered plugin's id/entry, use the new explicit fields. When a discovered plugin has no manifest, skip `provides` enforcement.
- `crates/swissarmyhammer-plugin/src/reload.rs` — follow the `DiscoveredPlugin` shape change.
- Keep a canonicalize/containment check on the resolved `index.{ts,js}` so a symlinked entry cannot escape the bundle (mirror `Manifest::resolve_entry`'s `starts_with` rule).
- Do NOT delete `Manifest`, `MANIFEST_FILE`, or `provides` — that is the final task.

## Acceptance Criteria
- [x] A `plugins/<dir>/` directory with only `index.ts` (no `plugin.json`) is discovered: `id` = `<dir>`, entry = the `index.ts`.
- [x] `index.js` is used as the entry when no `index.ts` is present.
- [x] Existing `plugin.json` bundles still discover and load exactly as before — no behavior change for them.
- [x] A directory with neither a manifest nor `index.{ts,js}` is skipped without error.
- [x] A manifest-less plugin loads without any `provides` check.

## Tests
- [x] New unit tests in `discovery.rs::tests`: a manifest-less `index.ts` bundle is discovered with dir-name id; `index.js` fallback works; a manifest-less directory with no entry is skipped; a manifest-less bundle in one layer shadows/stacks correctly against another layer.
- [x] New integration assertion (extend `tests/discovery.rs`): a manifest-less bundle staged into a temp layer is discovered and `discover_and_load_all` loads it.
- [x] Run `cargo nextest run -p swissarmyhammer-plugin` — all green, including every existing `plugin.json` test.
- [x] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` — clean.

## Workflow
- Use `/tdd` — write the failing manifest-less discovery tests first, then implement.

## Implementation notes
- `reload.rs` carries no reference to `DiscoveredPlugin` — it only defines `ReloadPolicy`/`ReloadStatus`. No change was needed there; the `DiscoveredPlugin` shape change is fully absorbed by `discovery.rs` and `host.rs`.
- `host.rs::load_resolved` now takes the resolved `entry_file: String` explicitly (rather than re-deriving it from a manifest). The public `load()` resolves entry itself for the legacy by-path path; `discover_and_load_all` and `load_active_copy` pass `DiscoveredPlugin::entry`. This keeps the legacy bare-`entry.ts` `host.load()` path unchanged.