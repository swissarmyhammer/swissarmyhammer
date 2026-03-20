---
position_column: done
position_ordinal: '9180'
title: Add deep-link support to kanban-app
---
## What
Register `kanban://` URL scheme in kanban-app so the CLI can trigger "open this board". Follow the mirdan-app pattern.

**Files to modify:**
- `kanban-app/Cargo.toml` — add `tauri-plugin-deep-link = "2"`, `urlencoding = { workspace = true }`
- `kanban-app/tauri.conf.json` — add `"plugins": { "deep-link": { "desktop": { "schemes": ["kanban"] } } }`
- `kanban-app/src/main.rs` — add `mod deeplink`, `.plugin(tauri_plugin_deep_link::init())`, cold-start + warm-start URL handlers in `setup()`

**Files to create:**
- `kanban-app/src/deeplink.rs` — `extract_open_path(url: &str) -> Option<PathBuf>` strips `kanban://open/` prefix, URL-decodes; `handle_url(app: &AppHandle, url: String)` calls `AppState::open_board()` on the resolved path

**Pattern:** Mirror `mirdan-app/src/deeplink.rs` and `mirdan-app/src/main.rs:32-66`
**Key reuse:** `AppState::open_board()` already handles path resolution, dedup, watcher startup, MRU tracking

## Acceptance Criteria
- [ ] `kanban://open/<url-encoded-path>` opens a board when the app is running
- [ ] Cold-start URLs are handled (app launched via URL)
- [ ] Warm-start URLs are handled (app already running)
- [ ] URL parsing unit tests pass

## Tests
- [ ] Unit tests in `deeplink.rs` for URL extraction (valid, trailing slash, wrong scheme, empty, encoded chars)
- [ ] `cargo nextest run -p kanban-app` passes