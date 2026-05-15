# swissarmyhammer-commands

Command trait, registry, and dispatch context for SwissArmyHammer.

## Architecture

Every user-facing action is a **command**. Commands have two parts:

1. **YAML definition** (`builtin/commands/*.yaml`) — metadata: id, name, scope, keybindings, params, `undoable` flag
2. **Rust implementation** — a struct implementing the `Command` trait (available + execute)

Both must exist. A YAML definition without a Rust impl is inert. A Rust impl without a YAML definition is invisible (no keybindings, no menu, no `undoable` tracking).

## Adding a New Command

### 1. Define in YAML

Add an entry to the appropriate file in `builtin/commands/`:

```yaml
- id: thing.do
  name: Do Thing
  scope: "entity:thing"       # required scope chain entries
  undoable: true               # whether this mutates entity data reversibly
  context_menu: true           # show in right-click menus
  keys:
    cua: Mod+D
    vim: d
  params:
    - name: thing
      from: scope_chain
      entity_type: thing
```

Key fields:
- `id` — unique command identifier (`namespace.verb` convention)
- `undoable` — if `true`, the dispatch layer wraps execution in a transaction and the undo stack tracks it. **Every undoable command must have undo/redo integration tests.**
- `scope` — comma-separated entity monikers required in the scope chain for the command to be available
- `params` — where each parameter comes from: `scope_chain`, `target`, `args`, or `default`

### 2. Implement in Rust

Create a struct implementing `Command` in the appropriate crate:

```rust
pub struct DoThingCmd;

#[async_trait]
impl Command for DoThingCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("thing")
    }

    async fn execute(&self, ctx: &CommandContext) -> Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        // ... do the work ...
        Ok(json!({ "id": thing_id, "operation_id": ulid }))
    }
}
```

### 3. Register

Add to `register_commands()` in `swissarmyhammer-kanban/src/commands/mod.rs`:

```rust
map.insert("thing.do".into(), Arc::new(thing_commands::DoThingCmd));
```

Update the command count assertion in the `register_commands_returns_expected_count` test.

### 4. Test

**Required tests for every command:**

- **Availability tests** — in `commands/mod.rs` tests: verify `available()` returns true/false based on scope chain
- **Execution test** — in `tests/command_dispatch_integration.rs`: dispatch through `TestEngine`, verify result

**Required tests for undoable commands (`undoable: true`):**

- **Undo/redo integration test** — in `tests/command_dispatch_integration.rs`:
  1. Execute the command
  2. Verify the mutation happened
  3. `app.undo` — verify the mutation is reversed
  4. `app.redo` — verify the mutation is reapplied

These tests use the `TestEngine` harness which wires up a temp board, command registry, and `EntityContext` with undo stack.

## Command Flow

```
User action (key/menu/palette)
  → Frontend executeCommand(id)
    → invoke("dispatch_command", { cmd, scope_chain, target, args })
      → dispatch_command_internal()
        → Registry lookup (YAML CommandDef)
        → Impl lookup (Rust Command)
        → If undoable: wrap in transaction
        → cmd.execute(ctx)
        → If undoable or undo/redo: flush_and_emit
        → Return result to frontend
```

## Undo/Redo

The undo system lives at the **entity layer** (`swissarmyhammer-entity`), not in this crate:

- `UndoStack` — pointer-based stack, persisted to `.kanban/undo_stack.yaml`
- `UndoCmd` / `RedoCmd` — read the stack top, call `EntityContext::undo()`/`redo()`
- `EntityContext::write()`/`delete()` automatically push onto the undo stack
- Transaction wrapping groups multi-entity mutations into a single undo step

The `undoable` flag in YAML controls:
- Whether `dispatch_command_internal` wraps execution in a transaction
- It does NOT control flush/emit — undo/redo commands also need flush/emit despite being `undoable: false`

## Crate Layout

```
swissarmyhammer-commands/
├── builtin/commands/     # YAML command definitions
│   ├── app.yaml          # app.quit, app.undo, app.redo
│   ├── entity.yaml       # task.*, entity.*, tag.*, column.*, attachment.*
│   ├── drag.yaml         # drag.start, drag.cancel, drag.complete
│   ├── file.yaml         # file.switchBoard, file.closeBoard, etc.
│   ├── settings.yaml     # settings.keymap.*
│   └── ui.yaml           # ui.inspect, ui.palette.*, ui.view.*
├── src/
│   ├── command.rs        # Command trait
│   ├── context.rs        # CommandContext (scope chain, args, extensions)
│   ├── error.rs          # CommandError
│   ├── registry.rs       # CommandsRegistry, YAML loading
│   ├── types.rs          # CommandDef, KeysDef, ParamDef
│   └── ui_state.rs       # UIState (window geometry, recent boards, etc.)
└── README.md
```
