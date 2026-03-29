---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffea80
title: ClipboardProvider trait + system clipboard integration
---
## What

Add a `ClipboardProvider` trait in the kanban crate so entity operations can read/write the system clipboard. Add the Tauri clipboard plugin and wire a real implementation.

### Files to create/modify
- `swissarmyhammer-kanban/src/clipboard.rs` — new: `ClipboardProvider` trait, clipboard JSON format with `swissarmyhammer_clipboard` wrapper, serialize/deserialize helpers
- `kanban-app/Cargo.toml` — add `tauri-plugin-clipboard-manager` dependency
- `kanban-app/src/main.rs` — register clipboard plugin, create TauriClipboardProvider impl
- `kanban-app/src/commands.rs` — inject ClipboardProvider as extension on CommandContext

### ClipboardProvider trait
```rust
#[async_trait]
pub trait ClipboardProvider: Send + Sync {
    async fn write_text(&self, text: &str) -> Result<(), String>;
    async fn read_text(&self) -> Result<Option<String>, String>;
}
```

### Clipboard JSON format
```json
{
  \"swissarmyhammer_clipboard\": {
    \"entity_type\": \"task\",
    \"entity_id\": \"01ABC\",
    \"mode\": \"copy\",
    \"fields\": { \"title\": \"Fix bug\", \"body\": \"...\", ... }
  }
}
```

### Test impl
`InMemoryClipboard` using `Arc<Mutex<Option<String>>>` for unit/integration tests.

### UIState changes
- Remove `ClipboardState`, `ClipboardMode`, `set_clipboard()`, `clipboard()`, `clear_clipboard()`, `has_clipboard()`
- Add `has_clipboard: bool` flag (transient, `#[serde(skip)]`) — set by copy/cut operations, checked by paste availability

## Acceptance Criteria
- [ ] `ClipboardProvider` trait defined in swissarmyhammer-kanban
- [ ] Tauri plugin registered, TauriClipboardProvider wired as CommandContext extension
- [ ] InMemoryClipboard available for tests
- [ ] JSON format includes type marker for safe deserialization
- [ ] Old ClipboardState/ClipboardMode removed from UIState
- [ ] `has_clipboard` flag on UIState for sync availability checks

## Tests
- [ ] Serialize/deserialize round-trip test for clipboard JSON format
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes
- [ ] `cargo check -p kanban-app` compiles"
<parameter name="assignees">[]