---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9880
project: plugin-arch
title: 'Test: server-name collision policy across plugins'
---
## What

The spec is explicit that MCP servers do **not** have override semantics — the first registration of a name wins, subsequent attempts fail with `ServerNameTaken`. With the manifest gone, there is no install-time `provides` declaration to surface this earlier; it is now a strictly runtime guarantee. We don't have an integration test that proves it from a plugin author's perspective.

(The spec's testing table also lists `Override stack (Command svc)` — that one belongs to the future Command service per `ideas/plugins/command-service.md` and is out of scope here. This task is the platform-level analogue: prove the no-override policy at the server-registry level.)

Write `crates/swissarmyhammer-plugin/tests/server_name_collision_e2e.rs`:

- Stage two committed example plugins under `crates/swissarmyhammer-plugin/examples/plugins/` that each try to `this.register("collide-probe", { rust: <some-shared-id> })` in their `load()`. Real bundles on disk, real `index.ts` (no manifests), real `Plugin` subclasses with `readonly name`/`version`/`description` props — following the existing `examples/plugins/` layout established by `kanban-tasks`, `file-notes`, etc.
- A small documentation README under `examples/plugins/collide-probe-a/` and `examples/plugins/collide-probe-b/` explaining what each one demonstrates and which test exercises it.
- The test uses the shared `tests/support/mod.rs` helpers (`stage_example`, `build_mcp_server`, etc.) to load both bundles into one `PluginHost` and asserts:
  1. The plugin loaded first registers the server successfully.
  2. The plugin loaded second sees its `load()` reject with the platform's `ServerNameTaken` error (visible from the TS side as a thrown error, since `register()` is sync).
  3. The first plugin's server remains live and addressable after the collision — the failed second load does not poison it.
  4. After unloading the first plugin, the second plugin's `register("collide-probe", …)` would succeed (load the second plugin fresh and observe success).

### Design discoveries (documented at the test / bundle / harness layer)

- An in-process `{ rust }` source is **single-activation** — the host's `activate_rust_module` moves the module out of the available-modules table on first activation, so two bundles sharing one `{ rust }` id resolve the second to `UnknownServer` rather than reaching the name-uniqueness check. To exercise `ServerNameTaken` honestly, each bundle activates its **own** distinct `{ rust }` module (`collide-probe-a-mod`, `collide-probe-b-mod`) while both register under the same **name** (`"collide-probe"`). The collision is on the registered name, exactly as the registry's policy specifies. This is documented in both bundle READMEs and in the test's module docstring.
- `PluginHost::discover_and_load_all` is **atomic-on-failure**: if any discovered plugin fails to load, every plugin the same call already loaded is rolled back. Loading both bundles through one discovery scan would therefore lose bundle A's registration when bundle B's load fails, defeating assertion 3. The test stages both bundles into a project layer with `support::stage_example` but loads each through `host.load(<bundle>)` directly — one load per bundle, isolating their fates.

No platform code needed to change — `ServerNameTaken` already propagates from the Rust registry across the SDK bridge into the V8 isolate as a thrown JS `Error` whose message carries the `Display` form of `Error::ServerNameTaken`.

## Acceptance Criteria

- [x] Two new committed example bundles under `crates/swissarmyhammer-plugin/examples/plugins/collide-probe-a/` and `…/collide-probe-b/`, each with `index.ts` and a short `README.md`.
- [x] One new e2e test file: `crates/swissarmyhammer-plugin/tests/server_name_collision_e2e.rs`.
- [x] The test covers the four assertions above using `cargo nextest run` through the existing harness.
- [x] No new platform code unless the test reveals an actual gap (e.g. the error type is not propagated to the TS isolate). If a gap is found, fix it in the same task and document the fix in the test. *No platform-code gap was found — `ServerNameTaken` already propagates as a thrown JS Error with the host's Display message. Only the test-support harness was extended (a real `rmcp` `ProbeEchoServer` + two helpers `expose_collide_probe_module` / `expose_collide_probe_modules`).*
- [x] `examples/plugins/README.md` index gains a line describing the two new examples.

## Tests

- [x] `cargo nextest run -p swissarmyhammer-plugin --test server_name_collision_e2e` — green (1 test passed, 59ms).
- [x] `cargo nextest run -p swissarmyhammer-plugin` — full plugin suite still green (131 tests passed, 0 failures, 0 skipped, 7 slow).
- [x] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` — clean.

## Workflow

- Use `/tdd` — write the four assertions as a failing test first, then build the example bundles to satisfy them.
- Mirror the existing example-plugin testing pattern in `crates/swissarmyhammer-plugin/tests/example_plugins_e2e.rs` and `tests/support/mod.rs`. Do not create a parallel harness.