---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
project: plugin-arch
title: Audit hot_reload_e2e.rs against spec's five hot-reload edge cases
---
## What

The spec (`ideas/plugins/plugin-architecture.md`, Hot Reload section) names five edge cases the platform must handle correctly. We have `crates/swissarmyhammer-plugin/tests/hot_reload_e2e.rs` but I have not verified each edge case is actually covered. This task is a coverage audit + gap fix.

The five edge cases (per the updated spec):

1. **In-flight operations terminate abruptly.** Isolate is killed; calls reject with `PluginReloaded`.
2. **Registration set may change silently.** Nothing declares ahead of time which servers a plugin registers, so a v2 that registers more (or different) servers than v1 just does so when its `load()` runs. Conflicts surface as `ServerNameTaken` from the new registrations. *(Manifest removal updated this from the original "`provides` expansion re-approval" edge case.)*
3. **Failed v2 load leaves the plugin unloaded.** No fallback to v1; v1 is already torn down by the time v2 is attempted. Manual retry.
4. **Crashed plugins do not auto-restart.** Surfaced via notification and settings UI badge; user-initiated reload.
5. **Plugin state in class fields is lost on reload.** Intended.

### Audit step

Read `crates/swissarmyhammer-plugin/tests/hot_reload_e2e.rs` in full and produce, *as task comments before any code changes*, a coverage matrix that maps each of the five edge cases to either a specific test function (with line range) or "missing." Do this before writing any new tests so the scope of the gap is explicit.

### Implementation step

For each "missing" edge case, add one focused test to the same file. Each test must use a real `PluginHost`, real example bundles staged under a `TempDir` (or reuse the existing `examples/plugins/` bundles where they fit), and observe an effect that only happens if the spec's behavior is real:

- **(1) `PluginReloaded`**: start a long-running call from a plugin (e.g. one that awaits a callback that the test deliberately never invokes), trigger reload mid-flight, assert the in-flight call rejects with the platform's `PluginReloaded` error visible to TS.
- **(2) Registration set changes**: load v1 that registers `{foo}`, swap the bundle's `index.ts` on disk to a v2 that registers `{foo, bar}`, observe `this.bar` becomes addressable after reload — no error, no prompt. Conversely: v1 registers `{foo}`, v2 registers `{baz}` — `this.foo` is gone, `this.baz` is live.
- **(3) Failed v2 load**: v1 loads cleanly, v2 throws in `load()`, assert the plugin host reports the error, `this.<v1-server>` is unavailable (v1 was already torn down), and no zombie isolate exists (check via `PluginHost::status` or whatever introspection exists).
- **(4) Crashed plugin no auto-restart**: simulate a plugin crash (e.g. exhaust isolate memory or throw an unhandled rejection that crosses to the host), assert the host surfaces a notification, the plugin's servers are unregistered, and the host does NOT re-attempt load until the test explicitly asks for one.
- **(5) Class-field state lost**: plugin's `load()` sets `this.counter = 0`; trigger reload; assert the new instance has `this.counter` re-initialized (not preserved).

If the existing test already covers some cases, do not duplicate — extend or rename for clarity.

### Spec back-edits

If the audit reveals the actual platform behavior diverges from the spec (e.g. crashes *do* auto-restart, or `PluginReloaded` is not the surface error), do not silently match the test to the code. Stop, surface the divergence in the task notes, and ask before changing either spec or platform.

## Acceptance Criteria

- [ ] A coverage matrix comment block at the top of `hot_reload_e2e.rs` listing the five edge cases and which test function covers each.
- [ ] Every edge case has at least one test; new tests added for any gaps found.
- [ ] No test depends on timing-sensitive sleeps without a timeout escape — use deterministic synchronization (channels, awaitable futures, the existing `support::TIMEOUT`).
- [ ] Each new test runs in isolation under the project's `#[serial_test::serial]` discipline if it touches process-global state (the V8 platform is a known shared resource).

## Tests

- [ ] `cargo nextest run -p swissarmyhammer-plugin --test hot_reload_e2e` — green.
- [ ] `cargo nextest run -p swissarmyhammer-plugin` — full plugin suite still green.
- [ ] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` — clean.

## Workflow

- Audit first (read, produce coverage matrix as a code comment), then write missing tests one at a time using `/tdd` — write the failing test, observe the actual behavior, fix only if it diverges from the spec.
- If any edge case requires platform changes (new error type, new notification channel), call that out as a separate finding and STOP for direction before implementing — do not silently expand scope.