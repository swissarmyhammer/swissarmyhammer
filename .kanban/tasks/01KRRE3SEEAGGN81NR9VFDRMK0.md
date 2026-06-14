---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff080
project: plugin-arch
title: 'directory: stack-aware Watcher&lt;C&gt; with shared async-watcher helper'
---
## What
Add a generic, stack-aware filesystem watcher to `swissarmyhammer-directory` so plugins (and later skills/prompts/modes/agents) get hot-reload events. Today discovery is point-in-time; the only fs watcher in the workspace is `swissarmyhammer-tools/src/mcp/file_watcher.rs` (built on `async-watcher`), bespoke to its use.

- Add `async-watcher` to `crates/swissarmyhammer-directory/Cargo.toml` (already a workspace dep — see `swissarmyhammer-tools`).
- New module `crates/swissarmyhammer-directory/src/watcher.rs`:
  - `pub enum LayerChange { Added { layer: FileSource, path: PathBuf }, Modified {..}, Removed {..} }`
  - `pub struct StackedEvent { subdirectory: String, name: String, change: LayerChange }` — `name` is the top-level entry within the subdirectory (e.g. plugin dir name), not a raw file path.
  - `pub struct Watcher<C: DirectoryConfig>` with `pub fn watch(subdirectory: &str) -> Result<(Self, mpsc::Receiver<StackedEvent>)>`.
  - Fans out across every writable layer the config exposes (user + project); builtin is read-only and NOT watched. Reuse `ManagedDirectory::<C>` / `VirtualFileSystem` path resolution to know the layer roots.
  - Debounce inside the watcher: a save touching the manifest + several source files under `plugins/weather/` yields ONE `StackedEvent` per affected `name`, not one per file.
- Extract the `async-watcher` plumbing (debounce, cancellation, teardown) into a shared internal helper module so this watcher and (future) others share it rather than copying. Scope: only `swissarmyhammer-directory` in this task — do NOT refactor the `swissarmyhammer-tools` file_watcher here.

## Acceptance Criteria
- [x] `Watcher<C>`, `StackedEvent`, `LayerChange` exist, `pub`, exported from `swissarmyhammer-directory` lib.
- [x] `Watcher::<SwissarmyhammerConfig>::watch("plugins")` returns a receiver; events carry the changed layer (`FileSource`) and the top-level `name`.
- [x] Multiple file writes inside one named entry within the debounce window collapse to a single event for that name.
- [x] Builtin layer is not watched.

## Tests
- [x] Integration test in `swissarmyhammer-directory/tests/`: create a temp project layer with `plugins/foo/plugin.json`, start the watcher, write a file, assert exactly one `StackedEvent { subdirectory: "plugins", name: "foo", change: Added|Modified }` with the project `FileSource`.
- [x] Test that removing `plugins/foo/` emits a `Removed` event.
- [x] Test debounce: write three files under `plugins/foo/` rapidly, assert a single coalesced event for `foo`.
- [x] Run: `cargo test -p swissarmyhammer-directory` — all green.

## Workflow
- Use `/tdd` — write the watch/debounce integration tests first against real temp dirs, then implement.

## Review Findings (2026-05-16)

Task-mode review. Build, `cargo test -p swissarmyhammer-directory`, clippy `-D warnings`, and `cargo fmt --check` all clean (per tester). Two Low-severity findings; no Critical/High/Medium. Findings do not block the acceptance criteria — the watcher is correct for the common in-a-git-repo case — but are recorded for follow-up.

- [x] ### Finding 1 (Low) — Project-layer resolution lacks the discovery code's `from_custom_root` fallback
`resolve_writable_layers` in `watcher.rs` resolves the project (`Local`) layer only via `ManagedDirectory::<C>::from_git_root()`. The discovery code the watcher must stay in sync with — `VirtualFileSystem::load_local_files_managed` and `VirtualFileSystem::get_directories` in `file_loader.rs` — falls back to `ManagedDirectory::<C>::from_custom_root(current_dir)` when `from_git_root()` fails (i.e. when not inside a git repository). Consequence: when `watch()` is called outside a git repo, discovery loads plugins from `{cwd}/{DIR_NAME}/{subdirectory}` while the watcher silently omits the project layer; if the XDG layer also fails to resolve, `watch()` returns an `Err` rather than watching the directory discovery actually reads — so hot reload silently never fires for that layer. This is exactly the layer-root mismatch risk called out in the review brief. Recommend mirroring the discovery fallback (resolve the project root via `from_custom_root(current_dir)` when `from_git_root()` errors) so the watcher watches the same roots future plugin discovery loads from.

RESOLVED: Extracted a `resolve_project_layer<C>` helper that mirrors the discovery `else`-fallback exactly — git root preferred, `from_custom_root(current_dir)` when `from_git_root()` errors. `resolve_writable_layers` now calls it. Added unit tests `project_layer_prefers_git_root`, `project_layer_falls_back_to_current_dir_outside_git_repo`, and `project_layer_none_when_no_git_root_and_no_current_dir`.

- [x] ### Finding 2 (Low) — `merge_change` doc/comment claims path refresh that the code does not perform
The doc comment on `merge_change` states "The merged change keeps the most recent path so consumers see a concrete triggering path", and the inline comment in `translate_batch` says "later paths in the batch refine the chosen change". The implementation only assigns `*existing = incoming.clone()` when `dominates` is true; in the non-dominating arm (`_ => false`) — e.g. an existing `Modified` receiving another `Modified`, or an existing `Added` receiving a `Modified` — the first-seen `path` is kept and never refreshed. The documentation and the behavior disagree. Functionally harmless (any path under the entry is a valid trigger), but misleading. Recommend either correcting the doc/comment to say the first triggering path is kept, or refreshing `path` unconditionally on merge to match the stated intent.

RESOLVED: Corrected the docs to match the actual behavior — the non-dominating arm keeps the first-seen change and path; refreshing unconditionally would add filesystem-event-ordering nondeterminism. Updated the `merge_change` doc comment, its inline arm comment, and the inline comment in `translate_batch`. Added unit test `merge_keeps_first_path_when_not_dominating`.
