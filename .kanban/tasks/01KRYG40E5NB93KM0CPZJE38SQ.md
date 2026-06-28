---
assignees:
- claude-code
depends_on:
- 01KRYG1VWTF16P6FQCX1ZRTZZX
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8b80
project: plugin-examples
title: 'Example plugin: cli-echo (CLI stdio transport + unload lifecycle)'
---
## What

Add a committed example that registers an MCP server over the `{ cli }` stdio-subprocess transport and demonstrates the `unload()` lifecycle hook. This is the only example covering a non-`rust` `ServerSource`, and the only one demonstrating teardown.

- Create `crates/swissarmyhammer-plugin/examples/plugins/cli-echo/plugin.json` — `id: "cli-echo"`, `entry: "entry.ts"`, `provides: ["echo"]`.
- Create `crates/swissarmyhammer-plugin/examples/plugins/cli-echo/entry.ts` — a `Plugin` subclass whose `load()` does `this.register("echo", { cli: [<path>] })` and calls the flat `echo` tool (`await this.echo.echo({ message: "..." })`), and whose `unload()` calls `this.unregister("echo")` then `super.unload()`. Document that the CLI command path is supplied by the host/test (an example cannot hard-code an absolute binary path), and that `unload()` is where a plugin releases what it set up.
  - Decide the cleanest way for the committed example to receive the fixture binary path. Preferred: the example registers a placeholder and the test rewrites the staged copy's `entry.ts` with the real `env!("CARGO_BIN_EXE_cli_server_fixture")` path after `stage_example` — OR add a `stage_example_with(name, layer_root, substitutions)` helper to `tests/support/mod.rs` that does token replacement on the staged copy. The COMMITTED file stays a clean, readable example; only the staged temp copy is specialized. Document whichever approach is chosen.
- Create `crates/swissarmyhammer-plugin/tests/cli_echo_e2e.rs` (`mod support;`): stage the bundle, point it at `env!("CARGO_BIN_EXE_cli_server_fixture")`, `discover_and_load_all`, assert the `echo` server answers a real call routed over stdio; then `host.unload(&plugin_id)` and assert the `echo` server is no longer in the live registry (`UnknownServer`/`ServerUnavailable`).

## Resolution

- Chosen staging approach: added `stage_example_with(name, layer_root, substitutions)` to `tests/support/mod.rs`. It stages the bundle, then does literal token replacement across every staged file. The committed `entry.ts` carries the named placeholder token `__CLI_ECHO_COMMAND__`; the test rewrites it in the throwaway staged copy with `env!("CARGO_BIN_EXE_cli_server_fixture")`. Documented in the example's `entry.ts` and in `examples/plugins/README.md`.
- The entry module keeps the one `Plugin` instance in a module-level variable and exports both `load` and `unload`, since the host drives a module-level `unload` export (`UNLOAD_EXPORT`) — a plugin's `unload()` hook is unreachable without the export. Both run on the same isolate.
- The e2e test routes the `echo` call through `PluginHost::call` both before unload (asserts the live round-trip over stdio) and after unload (asserts `Error::ServerUnavailable` — the disposed-name tombstone).

## Acceptance Criteria
- [x] `examples/plugins/cli-echo/{plugin.json,entry.ts}` exist; `entry.ts` registers a `{ cli }` server and implements `unload()`.
- [x] The committed `entry.ts` is a clean readable example — only the staged temp copy is specialized with the fixture path.
- [x] The e2e test proves a `tools/call` round-trips over the stdio subprocess transport.
- [x] After `host.unload`, the `echo` server is gone — the unload hook + disposal ran.
- [x] README in `examples/plugins/` updated to describe `cli-echo`.

## Tests
- [x] New: `tests/cli_echo_e2e.rs::cli_echo_plugin_round_trips_over_stdio_and_unloads` — asserts the echo round-trip, then asserts the server is gone after unload.
- [x] Run `cargo nextest run -p swissarmyhammer-plugin --test cli_echo_e2e` — passes.

## Workflow
- Use `/tdd` — write the failing test first, then the example bundle and any harness substitution helper.

## Review Findings (2026-05-18 22:38)

### Warnings
- [x] `crates/swissarmyhammer-plugin/tests/cli_echo_e2e.rs:173-188` — Assertion 2 (`ServerUnavailable` after `host.unload`) does NOT prove the plugin's `unload()` hook ran — it proves only that `host.unload`'s host-side disposal ran. `PluginHost::unload` (src/host.rs:1115) calls `run_plugin_unload` and then *unconditionally* `dispose_registrations`; `run_plugin_unload` (src/host.rs:1527) swallows every error from the `unload` export at debug level (`unload()` is optional by contract). `dispose_registrations` is the authoritative cleanup that produces the `ServerUnavailable` tombstone "whether or not the plugin's own `unload()` did anything." Direct proof: the sibling `tests/unload_disposal_e2e.rs` probe plugin exports `load` ONLY (no `unload` export at all) and still gets `ServerUnavailable` after `host.unload`. So `cli_echo_e2e` would pass identically if `entry.ts`'s `unload` export were deleted, or if its `unload()` body threw before reaching `unregister`. The task's explicit review charge — "confirm the unload path genuinely exercises the `unload()` hook" — is therefore not met by the current assertions. Fix: give the plugin's `unload()` an observable side effect that disposal alone cannot produce and assert it — e.g. have `unload()` call a host-exposed tool (write a sentinel file/board entry) and assert that effect appears only after unload; or assert the host's `unload`-export invocation some other observable way. As written, deleting the entire `unload` export from `entry.ts` leaves the test green.
  - RESOLVED. The `cli-echo` plugin now registers the host-exposed `kanban` operation tool as `board` in `load()`, and its `unload()` adds a sentinel task (`"cli-echo unload() ran"`) to the board before unregistering `echo`. The e2e test seeds an empty board, asserts the sentinel is ABSENT while loaded, and asserts it is the ONLY task present after `host.unload`. Adding a kanban task is an effect host-side `dispose_registrations` can never produce, so the sentinel's presence is hook-specific proof. Deleting the bundle's `unload` export, or throwing inside `unload()` before the sentinel write, now fails the test.
  - HOST BUG FOUND AND FIXED (required to make the hook reachable at all). While wiring the observable effect, the swallowed `run_plugin_unload` error surfaced: `failed to load plugin: Trying to create "main" module (...), when one already exists (...)`. Root cause: the load path passes `Manifest::resolve_entry` — a **canonicalized** absolute entry path — so the isolate's module map keys the entry under e.g. `/private/var/.../entry.ts`. `run_plugin_unload` re-derived a bare `ENTRY_FILE` (`"entry.ts"`) joined onto the non-canonical `bundle_dir` (`/var/.../entry.ts`), and `deno_core` rejects creating a second "main" module under a URL that differs only by an unresolved symlink. The plugin's `unload()` hook therefore never ran on macOS — the exact gap this finding describes, but with a deeper root cause. Fix: `LoadedPlugin` now stores the resolved `entry_file` used by the `load` call; `run_plugin_unload` reuses it so both lifecycle calls address the identical module URL. Files: `crates/swissarmyhammer-plugin/src/host.rs` (`LoadedPlugin` struct, `load_resolved`, `run_plugin_unload`).
- [x] `crates/swissarmyhammer-plugin/tests/cli_echo_e2e.rs:9-10` and `crates/swissarmyhammer-plugin/examples/plugins/README.md:228-230` — Documentation overclaims what the test proves. The test docstring says it "asserts the plugin's `unload()` hook ran by observing that the server it registered is gone afterward" and the README says "The test proves the teardown end to end." Per the finding above, observing the server is gone proves only host-side disposal, not the hook. Even after fixing the test, if a hook-specific assertion is not added, both claims must be softened to state the test proves the server is disposed after `host.unload` (the disposal capability), not that the `unregister` call inside `unload()` specifically ran.
  - RESOLVED. With the sentinel-task assertion now in place, the test genuinely proves the `unload()` hook body ran, so the docs are accurate rather than softened. The `cli_echo_e2e.rs` module docstring now distinguishes the two facts the test proves separately: the sentinel task's appearance proves the `unload()` hook body ran; the `echo` server's `ServerUnavailable` tombstone proves the host's authoritative registration disposal ran. The README's "What the test proves, and what it does not" subsection spells out the same distinction — that observing the server gone proves only disposal, which is why `unload()` does something disposal cannot.

### Nits
- [x] `crates/swissarmyhammer-plugin/tests/support/mod.rs:137-170` — `apply_substitutions` reads every staged file as UTF-8 via `read_to_string`. A future example bundle carrying a binary asset (e.g. a small fixture data file) would panic the helper on a non-UTF-8 read. Not a problem for any bundle today (all are `plugin.json` + `entry.ts`), and the panic message is clear, but a one-line doc note on `stage_example_with` stating it only supports text bundles would prevent a surprising failure later.
  - RESOLVED. `stage_example_with`'s doc comment now carries a paragraph stating it supports text bundles only — every staged file is read as UTF-8 to scan for tokens, so a binary-asset bundle would panic and needs a different staging path.