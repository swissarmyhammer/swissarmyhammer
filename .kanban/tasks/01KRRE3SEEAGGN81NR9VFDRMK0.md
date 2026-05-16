---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
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
- [ ] `Watcher<C>`, `StackedEvent`, `LayerChange` exist, `pub`, exported from `swissarmyhammer-directory` lib.
- [ ] `Watcher::<SwissarmyhammerConfig>::watch("plugins")` returns a receiver; events carry the changed layer (`FileSource`) and the top-level `name`.
- [ ] Multiple file writes inside one named entry within the debounce window collapse to a single event for that name.
- [ ] Builtin layer is not watched.

## Tests
- [ ] Integration test in `swissarmyhammer-directory/tests/`: create a temp project layer with `plugins/foo/plugin.json`, start the watcher, write a file, assert exactly one `StackedEvent { subdirectory: "plugins", name: "foo", change: Added|Modified }` with the project `FileSource`.
- [ ] Test that removing `plugins/foo/` emits a `Removed` event.
- [ ] Test debounce: write three files under `plugins/foo/` rapidly, assert a single coalesced event for `foo`.
- [ ] Run: `cargo test -p swissarmyhammer-directory` — all green.

## Workflow
- Use `/tdd` — write the watch/debounce integration tests first against real temp dirs, then implement.