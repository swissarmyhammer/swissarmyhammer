---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc480
title: Add ClipboardState to UIState
---
## What

Add clipboard data structures and methods to `swissarmyhammer-commands/src/ui_state.rs`. The clipboard is transient (not persisted to YAML) and holds a snapshot of an entity for cut/copy/paste operations.

### Files to modify
- `swissarmyhammer-commands/src/ui_state.rs` — add structs, enum, methods, UIStateChange variant

### Implementation

Add `ClipboardMode` enum (`Cut`, `Copy`) and `ClipboardState` struct:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardMode { Cut, Copy }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardState {
    pub entity_type: String,   // \"task\"
    pub entity_id: String,     // source entity ID
    pub mode: ClipboardMode,   // Cut or Copy
    pub data: serde_json::Value, // snapshot of entity fields at copy time
}
```

Add to `UIStateInner` (transient, `#[serde(skip)]`):
```rust
#[serde(skip)]
clipboard: Option<ClipboardState>,
```

Add UIState methods:
- `set_clipboard(&self, state: ClipboardState) -> Option<UIStateChange>`
- `clipboard(&self) -> Option<ClipboardState>`
- `clear_clipboard(&self) -> Option<UIStateChange>`

Add `UIStateChange::Clipboard(Option<ClipboardState>)` variant.

## Acceptance Criteria
- [ ] `ClipboardState` struct and `ClipboardMode` enum exist and are `Clone + Serialize + Deserialize`
- [ ] UIState methods set/get/clear clipboard correctly
- [ ] Clipboard is transient (`#[serde(skip)]`) — not persisted to YAML
- [ ] UIStateChange::Clipboard variant emitted on mutations

## Tests
- [ ] `swissarmyhammer-commands/src/ui_state.rs` — unit tests: set_clipboard stores, clipboard retrieves, clear_clipboard clears, clipboard not persisted in round-trip
- [ ] `cargo nextest run -p swissarmyhammer-commands` passes"
<parameter name="assignees">[]