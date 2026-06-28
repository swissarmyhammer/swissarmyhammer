---
assignees:
- claude-code
depends_on:
- 01KRRE967SBZ5TH2JPDMSV21BY
- 01KRRE2K6TTREE37RJEARGRN9K
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffe80
project: plugin-arch
title: 'plugin: TypesEmitter — auto-maintained app.d.ts codegen'
---
## What
Implement `TypesEmitter` — the host component that keeps a generated `.d.ts` file in sync with the live server registry, so plugin authors get editor autocomplete with no build step.

In `crates/swissarmyhammer-plugin/src/codegen.rs`:
- `TypesEmitter` subscribes to registry events: server registered → query it via `tools/list`, regenerate; server unregistered → regenerate without it; `notifications/tools/list_changed` → regenerate for that server; plugin load/unload → flush boundary.
- Debounce regeneration ~100ms so a plugin registering many tools during `load()` produces ONE file write. Write atomically (write-then-rename) so language servers never see a half-written file.
- Output: one nested namespace per registered server on an `App` interface. Walk each tool's `Tool` definition:
  - flat tool (no `io.swissarmyhammer/operations` `_meta`) → one method named for the tool, input type from `inputSchema`.
  - operation tool → walk the noun → verb → parameters tree; emit `tool.<noun>.<verb>(input)` per leaf, input type built from that verb's `parameters` map. Emitted shape mirrors the `_meta` tree exactly — no schema inference.
- Output path configurable; default `.swissarmyhammer/types/app.d.ts`. Host only writes it when a dev-mode flag is set; production writes nothing.
- Stale types are safe: types are decoupled from runtime; the emitter does pure metadata → types copying.

## Acceptance Criteria
- [x] `TypesEmitter` regenerates on register/unregister/`list_changed`/load-unload events, debounced ~100ms, written atomically.
- [x] A flat tool emits a single typed method; an operation tool emits `<noun>.<verb>(input)` methods mirroring its `_meta` tree.
- [x] No file is written in production (dev-mode flag off).

## Tests
- [x] Unit test: feed the emitter a fake registry with one flat tool + one operation tool (operation tool carrying a real `io.swissarmyhammer/operations` `_meta`); assert the emitted `.d.ts` text contains the flat method signature and the nested `noun.verb(input)` signatures with the right parameter object types.
- [x] Test debounce: register several tools within the window; assert exactly one file write.
- [x] Test atomic write: the output path never contains a partial file (assert via the write-then-rename path).
- [x] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the emitted-`.d.ts`-content assertions first, then implement.

## Depends on
PluginHost lifecycle (registry events) and the operations `_meta` tree generator.

## Implementation notes
Implemented entirely in `crates/swissarmyhammer-plugin/src/codegen.rs`. `TypesEmitter` is a clonable host-owned component driven by direct method calls (`server_registered` / `server_unregistered` / `tools_changed` / `flush`, plus the general `handle(RegistryEvent)`). It keeps a `BTreeMap` snapshot of every registered server's tools; each event arms a ~100ms debounce backed by a generation counter — a later event cancels an earlier event's pending write, collapsing a `load()` burst into one write. `flush` regenerates synchronously. Writes are atomic via a unique sibling temp file + same-directory rename. The dev-mode flag gates disk writes: production renders the text in memory but writes nothing.

## Review Findings (2026-05-17 14:35)

### Warnings
- [x] `crates/swissarmyhammer-plugin/src/host.rs` — Host wiring is missing: the task scope is "the `TypesEmitter` itself **+ wiring it to registry events**", but no call site in `host.rs` ever drives the emitter. `HostState` has no `TypesEmitter` field and no dev-mode flag; `connect_and_register` (host.rs:~1745), the `unregister` bridge handler (host.rs:~1651), and `dispose_handle`'s `Server` arm (host.rs:~1372) all mutate `registry` without notifying any emitter; the `load`/`unload` flow has no `flush()` boundary. The code index confirms zero `TypesEmitter` references outside `codegen.rs` — the component is dead code. Acceptance criterion "`TypesEmitter` regenerates on register/unregister/`list_changed`/load-unload events" is not true of the assembled system: nothing emits those events. No other board task owns these call sites — `01KRRFHRY9FJHQ2H1485D6A7GF` (PluginHost → AppState integration) is an app-layer task that constructs the host with layer roots and says nothing about a `TypesEmitter` or a `HostState` dev-mode flag — so leaving it here would orphan the wiring. Fix: add a `TypesEmitter` (and a dev-mode flag) to `HostInner`/`HostState`, constructed in `PluginHost::new`/`for_tests`; call `server_registered`/`tools_changed` from `connect_and_register`, `server_unregistered` from the `unregister` handler and from `dispose_handle`'s `Server` arm, and `flush()` at the `load`/`unload` boundaries. The emitter code in `codegen.rs` itself is correct and needs no rework — only the host integration the task explicitly scoped in is missing.

### Resolution (2026-05-17)
Wired the `TypesEmitter` into `PluginHost`. New `HostInner.types_emitter` field (a `TypesEmitter`, `Clone` + internally synchronized, sits outside the host mutex). `PluginHost::new` gained `dev_mode: bool` and `types_dir: PathBuf` params (caller-supplied, host-agnostic); the shared `with_roots` builder now takes the emitter. `for_tests` builds the emitter dev-mode-off (test dirs stay clean); a new `PluginHost::with_types_dev_mode` constructor builds it dev-mode-on against a temp dir for tests that observe the file. Call sites: `connect_and_register` snapshots `server.tools()` and calls `server_registered` after the registry insert; the bridge `unregister` handler and `dispose_handle`'s `Server` arm both call `server_unregistered` when a server was actually removed; `load_resolved`'s success arm and `unload` both call `flush()` at their boundaries. `tools_changed` is left callable but unwired: the `notifications/tools/list_changed` notification is consumed inside `CliServer`/`UrlServer` and never surfaced to the host, so no host-side path exists to wire it to (per task instruction not to invent one). Two integration tests added in `tests/plugin_host.rs` — `dev_mode_host_writes_generated_types_for_a_registered_server` (proves `app.d.ts` is written and carries the registered server's namespace) and `production_mode_host_writes_no_generated_types` (dev-mode-off writes nothing). The dev-mode test was confirmed to fail against the unwired code before the wiring was restored. Verified: `cargo fmt`, `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings`, `cargo test -p swissarmyhammer-plugin` (all suites green), `cargo build --workspace` — zero failures, zero warnings.