---
position_column: done
position_ordinal: l5
title: YAML command registry with builtin loading and .kanban/commands/ overrides
---
Build the `CommandsRegistry` that loads command definitions from YAML and supports layered overrides.

## Scope

- Define YAML format for command definitions (id, name, scope, visible, keys, params, undoable, context_menu)
- Define `ParamDef` struct with `name` and `from` (scope_chain | target | args | default)
- Define `KeysDef` struct with optional vim, cua, emacs bindings
- Build `CommandsRegistry` struct with `HashMap<String, CommandDef>`
- Implement `from_yaml_sources(sources: &[(String, &str)])` — loads from pre-resolved YAML entries, later sources override earlier by id (partial merge: override only specified fields)
- Implement `available_commands(scope_chain: &[String])` — static pre-filter by scope field before calling trait `available()`
- Create `builtin/commands/` directory in `swissarmyhammer-commands` with initial YAML files: `app.yaml`, `entity.yaml`, `ui.yaml`, `settings.yaml`
- Support `.kanban/commands/` directory for user overrides

## Testing

- Test: load builtin YAML files, verify all commands parse
- Test: override a builtin command's keybinding via a second YAML source
- Test: override preserves unspecified fields from builtin
- Test: `available_commands` filters by scope — command with `scope: entity:tag` excluded when no tag in scope chain
- Test: `available_commands` includes command when scope matches
- Test: user-defined command in override YAML loads alongside builtins
- Test: unknown fields in YAML are ignored (forward compatibility)