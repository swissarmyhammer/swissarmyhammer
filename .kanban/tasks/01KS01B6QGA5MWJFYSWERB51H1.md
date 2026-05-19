---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8e80
project: plugin-examples
title: 'PluginHost: first-class builtin discovery layer (builtin/plugins, test/builtin/plugins)'
---
## What

The plugin platform has no builtin discovery layer. Today `PluginHost::discovery_layers()` feeds the `swissarmyhammer-directory` `VirtualFileSystem` only two roots — user (`FileSource::User`) and project (`FileSource::Local`) — so `discover_and_load_all` can never discover a builtin bundle and never tags one `FileSource::Builtin`. The kanban app works around this by `include_dir!`-bundling `builtin/plugins/`, extracting it to a cache, and `host.load()`-ing each bundle one-by-one, outside discovery.

Make the builtin layer a **first-class discovery layer**, lowest precedence, stacking under user and project. The `FileSource::Builtin` variant and `discover_plugins`' multi-layer support already exist — only the host's `discovery_layers()` needs the third root. Discovery already delegates layer stacking to `swissarmyhammer-directory` (`resolve_plugin_dirs`); this does NOT duplicate it.

User intent (confirmed 2026-05-19): real builtins resolve from `builtin/plugins/`; tests resolve from `test/builtin/plugins/`. Both must work; `~`/cwd layers stack on top.

## Design

- `crates/swissarmyhammer-plugin/src/host.rs`: add `builtin_root: Option<PathBuf>` to `HostInner`. `discovery_layers()` prepends `LayerRoot::new(builtin_root, FileSource::Builtin)` (lowest precedence) when set, so `discover_and_load_all` scans builtin → user → project. Add a public constructor that supplies a builtin root (e.g. `for_tests_with_builtin(builtin_root, user_root, project_root)`), and thread a builtin root through `PluginHost::new` / `with_roots`. Existing `for_tests` keeps builtin = `None`.
- `apps/kanban-app/src/plugins.rs`: keep the `include_dir!` assembly bundling of `builtin/plugins/` and extraction to the cache dir, but instead of `host.load()`-ing each builtin bundle, hand the extracted cache dir to the host as its builtin layer root so builtins go through `discover_and_load_all` and get `FileSource::Builtin`. The hot-reload watcher must not watch the read-only builtin layer.
- Add a committed `test/builtin/plugins/` fixture tree for tests that need a real builtin bundle.

## Acceptance Criteria
- [x] `PluginHost` has a builtin layer root; `discovery_layers()` returns builtin → user → project when a builtin root is set.
- [x] `discover_and_load_all` discovers builtin bundles and they carry `FileSource::Builtin`.
- [x] `apps/kanban-app` routes its builtin plugins through the builtin discovery layer; the watcher does not watch the builtin layer.
- [x] Existing `for_tests` and `PluginHost::new` callers keep compiling and passing.
- [x] `test/builtin/plugins/` fixture tree committed for test use.

## Tests
- [x] Unit/integration test in the plugin crate proving a bundle staged into a builtin layer root is discovered with `FileSource::Builtin` and stacks below user/project.
- [x] `cargo nextest run -p swissarmyhammer-plugin` and `-p kanban-app` green.

## Implementation Notes

- `PluginHost::new` changed from `async fn(builtins: Vec<PathBuf>, ...) -> Result<Self>` to a synchronous `fn(builtin_root: Option<PathBuf>, ...) -> Self`. Builtins are no longer eagerly `host.load()`-ed at construction — they are a discovery layer now, loaded by `discover_and_load_all`. This is consistent with the design's "builtins go through `discover_and_load_all`" and makes `new` infallible like `with_roots`/`for_tests`. The two `new` callers (kanban-app `build`, plugin crate `new_constructor_takes_explicit_layer_roots` test) were updated.
- The kanban app extracts the embedded `builtin/plugins/` tree into `<builtin_cache>/plugins/` (the `PLUGINS_SUBDIR`) so handing `<builtin_cache>` to the host as the builtin layer root makes discovery resolve the bundles under `<root>/plugins/`.
- `watch_roots()` already only watched user + project; its doc was updated to explain the builtin layer is deliberately not watched. A watcher-driven reconcile still re-runs full discovery (including the builtin layer), so builtin copies keep participating in layer precedence.
- Committed fixture: `test/builtin/plugins/builtin-probe/` (self-contained probe that registers no server).