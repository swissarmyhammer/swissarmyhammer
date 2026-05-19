---
assignees:
- claude-code
depends_on:
- 01KS042GT4J98KQ45T9SXY9R9X
- 01KS04359YHCZV13T7F1EZN1DD
- 01KS043W7MBVPJJPB000TTJGVM
position_column: todo
position_ordinal: '8680'
project: plugin-tsonly
title: Remove plugin.json, Manifest, and provides entirely
---
## What

Every committed bundle and every test now uses the TS-only layout (tasks 3–5). Delete the transitional `plugin.json` support so the manifest is gone for good.

- `crates/swissarmyhammer-plugin/src/manifest.rs` — delete the file (or, if a tiny entry-resolution/containment helper is still wanted, shrink it to exactly that and rename it away from "manifest"). Remove the `Manifest` struct, `MANIFEST_FILE`, `Manifest::load/parse/resolve_entry`, and the manifest unit tests.
- `crates/swissarmyhammer-plugin/src/discovery.rs` — remove the `plugin.json` detection branch; a bundle is now ONLY a directory containing `index.ts`/`index.js`. `DiscoveredPlugin` drops the `Option<Manifest>` field — it carries just `id` (directory name), `entry`, `directory`, `source`.
- `crates/swissarmyhammer-plugin/src/host.rs` — delete the `provides` enforcement entirely: the `provides`/`ProvidesViolation` validation, any reserved-host-name pre-check that was gated on `provides`, and the now-dead manifest plumbing. A plugin may register any server name; a genuine collision still surfaces naturally (`ServerNameTaken`/registry error) — keep that, only the `provides` allowlist goes.
- `crates/swissarmyhammer-plugin/src/error.rs` — remove `Error::Manifest` and `Error::ProvidesViolation` (and any variant now unreferenced). Re-point or delete error sites.
- `crates/swissarmyhammer-plugin/src/lib.rs` — drop the `Manifest`/`MANIFEST_FILE` re-exports.
- `crates/swissarmyhammer-plugin/src/{reload.rs,runtime/mod.rs}` — remove any remaining manifest/`resolve_entry`/`provides` references.
- `apps/kanban-app/src/plugins.rs` — remove any manifest/`provides` references; confirm builtin loading still works purely through `index.ts` discovery.
- Search the whole workspace for residual `Manifest`, `MANIFEST_FILE`, `plugin.json`, `provides`, `ProvidesViolation`, `resolve_entry` and clear every plugin-platform reference.

This task lands the breaking removal in one coherent change; tasks 3–5 guarantee nothing still depends on `plugin.json`.

## Acceptance Criteria
- [ ] `Manifest`, `MANIFEST_FILE`, and `Manifest::resolve_entry` no longer exist.
- [ ] Discovery accepts only `index.ts`/`index.js`; the `plugin.json` branch is gone.
- [ ] `provides` enforcement and `Error::ProvidesViolation` are removed; a plugin can register any (non-colliding) server name.
- [ ] `Error::Manifest` is removed; no dead error variants remain.
- [ ] No `plugin.json` / `Manifest` / `provides`-allowlist reference remains anywhere in `swissarmyhammer-plugin` or `apps/kanban-app`.

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-plugin` — full crate green.
- [ ] `cargo nextest run -p kanban-app` — green.
- [ ] `cargo nextest run -p swissarmyhammer-tools` — green (it dev-depends on the plugin crate).
- [ ] `cargo clippy -p swissarmyhammer-plugin -p kanban-app --all-targets -- -D warnings` — clean (no dead-code/unused warnings from the removal).
- [ ] `cargo build --workspace` — clean.

## Workflow
- Use `/tdd` — removal is verified by the existing (already-migrated) suite staying green; let the compiler and clippy drive out every dead reference.