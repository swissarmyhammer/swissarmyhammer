---
assignees:
- claude-code
depends_on:
- 01KPG5XK61ND4JKXW3FCM3CC97
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffe480
title: 'Commands: paste dispatcher mechanism (PasteHandler trait, PasteMatrix registry, PasteEntityCmd walker)'
---
## What

Build the paste-dispatch plumbing so handlers can be added one file at a time. No handler logic lives in this card — it ships the trait, the registry, the `PasteEntityCmd` chain walker, the `register_paste_handlers()` init point (empty), and a hygiene test that wires future handlers to files. Each handler then gets its own follow-up card.

### Pieces

```rust
#[async_trait]
pub trait PasteHandler: Send + Sync {
    fn matches(&self) -> (&'static str, &'static str);
    fn available(&self, clipboard: &ClipboardPayload, target: &str, ctx: &CommandContext) -> bool { true }
    async fn execute(&self, clipboard: &ClipboardPayload, target: &str, ctx: &CommandContext) -> Result<Value>;
}

pub struct PasteMatrix {
    handlers: HashMap<(&'static str, &'static str), Arc<dyn PasteHandler>>,
}

impl PasteMatrix {
    pub fn register(&mut self, h: impl PasteHandler + 'static) { /* … */ }
    pub fn find(&self, clip: &str, target: &str) -> Option<&Arc<dyn PasteHandler>> { /* … */ }
}

pub fn register_paste_handlers() -> PasteMatrix {
    PasteMatrix::default()
    // handlers added by follow-up cards
}
```

`PasteEntityCmd::execute` walks `ctx.scope_chain` innermost-first, parses each moniker's type, queries the matrix with `(clipboard.entity_type, target_type)`, and dispatches to the first matching handler whose `available()` is true. `PasteEntityCmd::available` runs the same walk and returns `true` if any handler matches.

### Files to create / modify

- CREATE `swissarmyhammer-kanban/src/commands/paste_handlers/mod.rs` — trait, matrix, empty `register_paste_handlers()`.
- MODIFY `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — rename `PasteTaskCmd` → `PasteEntityCmd`; execute/available delegate to matrix. Remove all hard-coded paste logic.
- MODIFY `swissarmyhammer-kanban/src/context.rs` — stash `Arc<PasteMatrix>` on `KanbanContext`.
- MODIFY `swissarmyhammer-kanban/src/commands/mod.rs` — route `entity.paste` to the new dispatcher.
- MODIFY `swissarmyhammer-commands/builtin/commands/entity.yaml` — `entity.paste` params → `{name: moniker, from: target}` with no scope pin.

### Subtasks

- [ ] Implement `PasteHandler` trait and `PasteMatrix`.
- [ ] Implement `PasteEntityCmd` chain-walking dispatcher.
- [ ] Wire `PasteMatrix` onto `KanbanContext`.
- [ ] Update `entity.yaml` paste params.
- [ ] Add hygiene test `every_registered_handler_has_a_source_file` that iterates the matrix and asserts a colocated `paste_handlers/{clip}_onto_{target}.rs` exists. Passes trivially with empty matrix; catches drift once handlers are added.

## Acceptance Criteria

- [ ] `PasteHandler` trait, `PasteMatrix`, and `register_paste_handlers()` exist.
- [ ] `PasteEntityCmd` compiles, implements the walk correctly, and returns `Unavailable` when the matrix is empty.
- [ ] Old `PasteTaskCmd` deleted (the replacement supersedes it completely).
- [ ] `entity.paste` YAML declares `from: target` with no scope pin.
- [ ] Hygiene test passes.

## Tests

- [ ] `paste_entity_cmd_returns_unavailable_when_matrix_empty` — verifies the dispatcher's base case.
- [ ] `paste_entity_cmd_walks_chain_innermost_first` — stub a handler for `(task, column)` only; chain `["task:X", "column:Y", "board:Z"]` with task clipboard skips `task:X` (no match), matches `column:Y`, stops before `board:Z`. Uses a test-only handler inside the test module, not a real one.
- [ ] `paste_entity_cmd_respects_handler_available` — handler's `matches()` fires but `available()` returns false; dispatcher continues walk.
- [ ] Hygiene test `every_registered_handler_has_a_source_file`.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers clipboard` — all green.

## Workflow

- Use `/tdd` — write the three behavioral tests with test-only stub handlers first. The real handler cards (follow-ups) each verify their own behavior.

#commands

Depends on: 01KPG5XK61ND4JKXW3FCM3CC97 (copy/cut must land first — the `ClipboardPayload` structure is shared)