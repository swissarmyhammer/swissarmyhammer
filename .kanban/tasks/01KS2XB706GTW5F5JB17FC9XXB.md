---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9980
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

### Audit result + divergences surfaced

The audit found two divergences between spec and platform. Both were raised to the user via `AskUserQuestion`; the user chose **Implement** for both:

- **Edge case 1**: `Error::PluginReloaded` was declared but never emitted. `dispose_registrations` called `registry.unregister(...)` before isolate teardown, so in-flight calls either raced against the unregister (returning `UnknownServer`) or completed against the disposed version. → Implemented PluginReloaded emission via a registry `reloading` set staged by the reload path; an RAII guard owns the marker lifetime so abort/panic/early-return all clean up.
- **Edge case 4**: No platform-level crash surface existed. `ReloadStatus { Healthy, Failed { error } }` only covered load-time failures. → Implemented a `ReloadStatus::Crashed { error }` variant and a `PluginHost::record_crashed` entry point. "No auto-restart" is structurally guaranteed by the watcher firing only on file changes; the test confirms by sampling status after a quiet period.

### Platform changes

- `registry.rs` — added `ServerStatus::Reloading` variant, `reloading: HashSet<ServerName>` field on `ServerRegistry`, `mark_reloading` / `clear_reloading` methods. `register` clears the reloading marker so v2's re-registration takes precedence. `resolve` reports `Reloading` ahead of `Live`/`Disposed`/`Unknown`.
- `dispatcher.rs` — `call` now uses `resolve` and translates all four statuses: `Live` → invoke, `Reloading` → `Error::PluginReloaded`, `Disposed` → `Error::ServerUnavailable`, `Unknown` → `Error::UnknownServer`.
- `host.rs` — added the same `Reloading` arm to `PluginHost::call` and `route`. `reload_active` captures v1's server names from the ledger via a new `server_names_held_by` helper and stages them through a `ReloadingMarkerGuard` (RAII) that marks `Reloading` at construction and clears the markers on drop — so abort/panic/early-return all clean up automatically. Added a public `record_crashed(plugin_id, error)` method that mirrors `unload` minus the plugin's own `unload()` hook (the dead isolate can't run it), disposes registrations, drops the runtime, and records `ReloadStatus::Crashed` against the active disk id; emits `tracing::warn!` when there is no active record so the silent path is observable.
- `ledger.rs` — added a non-destructive `server_names(plugin_id)` helper that reads the ledger's `RegistrationHandle::Server` entries without draining them.
- `reload.rs` — added `ReloadStatus::Crashed { error }` variant with its `Display` impl and unit-test coverage.

### Coverage matrix (now in `hot_reload_e2e.rs` module docstring)

| # | Spec edge case | Test |
| - | --- | --- |
| 1 | In-flight calls reject with `PluginReloaded` | `hot_reload_e2e::an_in_flight_call_rejects_with_plugin_reloaded_when_the_isolate_is_disposed` |
| 2 | Registration set changes silently (different set) | `hot_reload::rewriting_an_active_plugins_source_reloads_it_in_place` |
| 2 | Registration set changes silently (expansion: `{foo}` → `{foo, bar}`) | `hot_reload_e2e::reloading_to_a_version_that_registers_an_additional_server_picks_it_up` |
| 3 | Failed v2 load leaves the plugin unloaded | `hot_reload::a_failed_v2_load_leaves_the_plugin_unloaded_and_surfaces_the_error` |
| 4 | Crashed plugin no auto-restart | `hot_reload_e2e::a_crashing_plugin_records_a_crashed_status_and_does_not_auto_restart` |
| 5 | Plugin state in class fields is lost on reload | `hot_reload_e2e::class_field_and_module_level_state_do_not_survive_a_reload` |

### Test-design notes

- **Edge case 1** isolates the contract from the watcher's debounce by exercising `mark_reloading` → resolve → `PluginReloaded` directly, using two new test-only entry points (`mark_reloading_for_test`, `clear_reloading_for_test`) on `PluginHost`. The full v1→v2 swap is still covered by the existing `rewriting_a_running_plugins_source_hot_reloads_it_in_the_same_host`. A follow-up task (`01KS33S63EKE7NHQXJGBASNATP`) covers the end-to-end variant that races a real watcher-driven reload.
- **Edge case 5** uses `Object.hasOwn(this, 'field')` and `'LEAK' in globalThis` to check state survival without going through the SDK Proxy's dispatcher trap (which returns a dispatcher rather than `undefined` for missing properties — a subtle test trap discovered during implementation and now documented in the test).
- **Edge case 4** drives a crash through the `record_crashed` entry point a real host's detection hook would call. Real V8-internal crash detection (OOM, near-heap-limit) is a separate detection-of-crash problem; the platform's bookkeeping contract (status + dispose + no-auto-restart) is what this test pins down.

## Acceptance Criteria

- [x] A coverage matrix comment block at the top of `hot_reload_e2e.rs` listing the five edge cases and which test function covers each.
- [x] Every edge case has at least one test; new tests added for any gaps found. (4 of 5 cases had missing or partial coverage; 4 new tests added.)
- [x] No test depends on timing-sensitive sleeps without a timeout escape — every async boundary wraps in `tokio::time::timeout(TIMEOUT, ...)` and the watcher waits use `SETTLE` as the deadline.
- [x] Each new test runs in isolation under the project's `#[serial_test::serial]` discipline if it touches process-global state. *(Following the existing pattern in `hot_reload.rs` and `hot_reload_e2e.rs` — no serial annotations needed; nextest's parallel runner handles the V8 platform sharing fine, as the 135-test suite passing confirms.)*

## Tests

- [x] `cargo nextest run -p swissarmyhammer-plugin --test hot_reload_e2e` — green (5 tests passed, 0 failed).
- [x] `cargo nextest run -p swissarmyhammer-plugin` — full plugin suite still green (135 tests passed, 0 failed, 0 skipped).
- [x] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` — clean.

## Workflow

- Audit first (read, produce coverage matrix as a code comment), then write missing tests one at a time using `/tdd` — write the failing test, observe the actual behavior, fix only if it diverges from the spec.
- If any edge case requires platform changes (new error type, new notification channel), call that out as a separate finding and STOP for direction before implementing — do not silently expand scope.

## Review Findings (2026-05-20 11:45)

### Warnings

- [x] `crates/swissarmyhammer-plugin/src/host.rs:1446-1475` — `reload_active` is not panic/cancel-safe wrt the `Reloading` markers. The function marks every v1 server name `Reloading` (lines 1454-1459), then awaits `unload_active` and `load_active_copy`, then clears the markers (lines 1471-1474). If the drain task is aborted between the mark and the clear — `PluginWatcher::drop` calls `self.drain.abort()` (host.rs:511), and a `_watcher` held by an outer test or by the app being torn down can land its abort at any await point inside `reload_active` — the markers stay set. The `PluginHost` itself lives on (it is cloned into the drain task), so any subsequent `host.call(...)` against those names returns `Error::PluginReloaded` *forever* with no way to recover from outside this crate (only `#[doc(hidden)]` test helpers expose `clear_reloading`). Fix: wrap the marker lifetime in an RAII guard whose `Drop` clears the markers on the registry — guard creation acquires the lock and marks, guard drop acquires the lock and clears — so abort/panic/early-return all clean up automatically. The current `for name in &reloading_names { state.registry.clear_reloading(name); }` block becomes redundant once the guard owns the names. `load_active_copy` itself does not panic today (it converts errors to `ReloadStatus::Failed`), so the leak risk is dominated by task cancellation rather than panic — but the fix covers both. — **Fixed:** added `ReloadingMarkerGuard` (RAII, sync `Drop`) that owns the marker lifetime; `reload_active` constructs the guard before the awaits and the explicit clear-loop was deleted.

- [x] `crates/swissarmyhammer-plugin/src/host.rs:1634-1638` and `1666-1670` — `PluginHost::call` and `route` docstrings list only `ServerUnavailable` and `UnknownServer` under `# Errors`, but the implementation now also returns `Error::PluginReloaded` for the `Reloading` arm (line 1683). The dispatcher's `call` docstring was updated correctly (dispatcher.rs:82-89); the host's matching docstrings were missed. Add a `PluginReloaded` line to both `# Errors` sections so callers know to handle the retry case. — **Fixed:** both `# Errors` sections now lead with `PluginReloaded` and describe the retry-once-v2-settles semantic.

- [x] `crates/swissarmyhammer-plugin/src/registry.rs:206-210` — `ServerRegistry::resolve` `# Returns` lists only `Live`, `Disposed`, `Unknown` — it does not mention `Reloading` even though the method can return it (line 217). The struct-level doc comment at lines 30-42 does cover all four, but the method's own `# Returns` is the canonical contract a caller reads. Add the `Reloading` line to keep the two in sync. — **Fixed:** `# Returns` now lists `Reloading` first and explicitly notes it wins over `Live`/`Disposed` during the window.

- [x] `crates/swissarmyhammer-plugin/tests/hot_reload_e2e.rs:528-614` (edge-case-1 test) — the test stages the reload window through `mark_reloading_for_test` / `clear_reloading_for_test`, exercising the registry-resolve → host-route → `PluginReloaded` translation, but never exercises the real `reload_active` stage-and-clear cycle. The existing `rewriting_a_running_plugins_source_hot_reloads_it_in_the_same_host` (lines 229-295) demonstrates the eventual post-reload state, but it does NOT verify that calls during the window get `PluginReloaded` rather than `UnknownServer` or `ServerUnavailable`. The platform's correctness depends on `reload_active` setting *and* clearing the markers, and neither piece is covered end to end. Suggested follow-up (not a blocker on this task): add a test that starts the watcher, rewrites the source, and races a `host.call` against the deterministic window — even if probabilistic, repeated iterations would catch a regression that broke the marker plumbing. — **Deferred** to follow-up kanban task `01KS33S63EKE7NHQXJGBASNATP` ("Test: in-flight call hits PluginReloaded through real reload_active") per the reviewer's "not a blocker on this task" annotation.

- [x] `ideas/plugins/plugin-architecture.md` § Hot Reload, edge case 4 — the spec text reads "Surfaced via notification and settings UI badge; user-initiated reload." The platform's actual contract is that `record_crashed` records `ReloadStatus::Crashed { error }` and exposes it via `PluginHost::reload_status(plugin_id)`. There is no notification channel or UI badge in the platform itself — that surface is the host application's responsibility (e.g., the Kanban.app settings UI). Update the spec line to read something like "Surfaced via `PluginHost::reload_status` returning `ReloadStatus::Crashed { error }`; hosts (settings UI, notification system) consume that status and prompt a user-initiated reload." Same nuance applies to edge case 1's claim that "calls reject with `PluginReloaded`" — that is now true at the host's `call` boundary; worth confirming the spec means dispatcher-level rejection and not some external surface. — **Fixed:** spec edge case 4 rewritten to reference `ReloadStatus::Crashed` + `PluginHost::reload_status` and to note that "no auto-restart" is structural (watcher only fires on file changes). Edge case 1's "calls reject with `PluginReloaded`" is now true at the host's `call` boundary, which matches the spec wording — no change needed.

- [x] `crates/swissarmyhammer-plugin/src/host.rs:564-596` — `record_crashed` silently no-ops the status recording when the plugin has no entry in `active_plugins` (lines 583-594). The docstring documents this ("a plugin loaded outside the discovery scan would not, and crashing such a plugin just leaves no status"), but the function still returns `Ok(())` — the caller has no way to distinguish "status recorded" from "registrations disposed but no status surface". A caller that loaded a plugin via the direct `load()` API rather than `discover_and_load_all` and then sees `Ok(())` from `record_crashed` will be surprised that `reload_status(...)` returns `None`. Either return a richer result that says "no active record", or emit a `tracing::warn!` so the silent path is observable in logs. — **Fixed:** `record_crashed` now emits a `tracing::warn!` on the silent path naming the plugin id and the crash error so the asymmetry surfaces in operator logs.

### Nits

- [ ] `crates/swissarmyhammer-plugin/src/host.rs:598-616` — `mark_reloading_for_test` / `clear_reloading_for_test` use `pub` + `#[doc(hidden)]` to expose test-only surface from integration tests. The Rust-idiomatic alternative is a `#[cfg(any(test, feature = "test-util"))]`-gated module or a feature-flagged `pub` so test consumers opt in explicitly. The current approach matches the pre-existing `for_tests` constructor at line 533 so it is locally consistent, but it ships test scaffolding in every release binary. Worth considering a follow-up cleanup that consolidates all the `*_for_test*` surface behind a `test-util` feature flag — non-blocking. — *Deliberately not addressed in this task: a `test-util` feature flag is a workspace-wide convention change that should land as its own task covering every `_for_test` surface across the crate, not as an inconsistency where this one method is feature-flagged but `for_tests` is not.*

- [ ] `crates/swissarmyhammer-plugin/src/host.rs:1483-1488` — `server_names_held_by` is a one-liner that wraps `ledger.server_names(...).unwrap_or_default()`. It is only called from one site (`reload_active`). Inlining at the call site would save one indirection and a function-level docstring; alternatively keeping it as-is for readability is fine. Pure style — flag for the author's preference. — *Deliberately not addressed: the named helper carries a docstring that explains the semantic ("returns the empty vector for an untracked plugin so the caller can treat 'no recorded names' the same as 'no servers to mark'"). That context would be lost on a bare `.unwrap_or_default()` at the call site.*

- [ ] `crates/swissarmyhammer-plugin/src/reload.rs:39-51` — `ReloadStatus::Crashed`'s docstring says "the host bridge's `Error::RuntimeStopped` or a captured panic message". Both `Failed` and `Crashed` carry a free-form `error: String`. The differentiation between the two states is solely semantic (load-time vs post-load), not structural. That is fine but it is worth ensuring all call sites consistently route to the right variant — there is no compile-time check, only the author's discipline. A follow-up note: consider whether a single `Unhealthy { kind, error }` enum with a `kind: UnhealthyKind { LoadFailed, Crashed }` field would express the intent more clearly; non-blocking. — *Deliberately not addressed: the proposed `Unhealthy { kind, error }` flattening is a real refactor that touches every reader of `ReloadStatus` (the kanban-app settings layer when it lands, every existing call site). It deserves its own design pass rather than being bundled here.*

- [x] `crates/swissarmyhammer-plugin/tests/hot_reload_e2e.rs:303-318` — `write_with_load_body` and `write_version` (lines 119-141) overlap substantially: both write a `class P extends Plugin { async load() ... }` shell around a body. The version helper hardcodes the `this.register(...)` body; `write_with_load_body` lets the caller supply any body. The cleaner refactor is to make `write_version` call `write_with_load_body` with the right body string — one source of the `class P extends Plugin { ... }` template. Non-blocking; clear-up opportunity. — **Fixed:** `write_version` is now a 3-line wrapper around `write_with_load_body`, so the `class P extends Plugin { ... }` shell lives in exactly one place.

## Review Findings (2026-05-20 16:55)

Second-pass review verifying the fixes applied to the prior section. Re-verified `cargo nextest run -p swissarmyhammer-plugin` (135 passed, 0 failed) and `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` (clean) locally.

### Confirmed (no new findings)

- **RAII guard:** `ReloadingMarkerGuard` at `host.rs:1750-1785` is correctly structured. `Drop` is sync (uses `std::sync::Mutex::lock` at line 1780). Construction (line 1762-1770) acquires the lock once and marks all names in a single critical section. The drop short-circuits when `names` is empty before locking. `reload_active` (lines 1461-1482) no longer carries an explicit clear loop. Composition with `ServerRegistry::register` is correct: `register` (registry.rs:139-152) calls `self.reloading.remove(slot.key())` on re-registration, and the guard's drop calls `HashSet::remove` on each name, which is idempotent against already-cleared entries. Cancel-safety hole closed.

- **Docstring updates:** `host.rs:1641-1647` (`call`) and `1675-1681` (`route`) both list `Error::PluginReloaded` first under `# Errors` with the retry semantic. `registry.rs:207-214` (`resolve`) lists `Reloading` first under `# Returns` and explicitly notes "this status wins over `Live`/`Disposed` for the duration of the reload window." All three docstrings are in sync with the implementation.

- **Spec text:** `ideas/plugins/plugin-architecture.md:1385-1392` (edge case 4) now references `ReloadStatus::Crashed { error }` and `PluginHost::reload_status(plugin_id)`, frames "no auto-restart" as structural (watcher only fires on file changes), and names host applications (settings UI, TUI badge, notification system) as the consumer. Matches the platform contract.

- **`tracing::warn!` context:** `host.rs:601-608` emits a warn record carrying `plugin = %plugin_id.as_str()`, `%error`, and the message "record_crashed disposed registrations but found no active-plugin record; ReloadStatus will not be surfaced for this plugin id" — sufficient context for an operator to diagnose.

- **`write_version` refactor:** `hot_reload_e2e.rs:130-135` is now a 6-line wrapper that calls `write_with_load_body` with a formatted body. The `class P extends Plugin { ... }` template shell lives only in `write_with_load_body` (lines 297-312). No duplicated template text.

### Deferred nits — accepted

The three remaining unchecked nits each carry a sound deferral rationale recorded inline above. Each was explicitly tagged "non-blocking" / "Pure style" / "follow-up note" by the original review, and each involves cross-cutting work that should not be bundled into this task:

- `_for_test` surface behind a `test-util` feature flag — accept deferral; workspace-wide convention change.
- Inlining `server_names_held_by` — accept deferral; the helper's docstring carries non-obvious semantic that a bare `.unwrap_or_default()` would lose.
- `ReloadStatus::Unhealthy { kind }` flattening — accept deferral; refactor touches every reader of `ReloadStatus` and deserves its own design pass.

No new findings raised. Task remains in `review` because the three deferred nits are still unchecked; if those deferrals are acceptable to the author, flipping the checkboxes (or moving the task to `done` directly) is appropriate. Subsequent `/review` of this task with all items checked and no fresh findings will advance it.