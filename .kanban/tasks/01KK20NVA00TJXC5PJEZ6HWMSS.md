---
position_column: done
position_ordinal: ffffbe80
title: list_available_commands clones scope and creates HashMap on every retain iteration
---
swissarmyhammer-kanban-app/src/commands.rs:570-588\n\nInside the `available.retain()` closure, a new CommandContext is constructed for each command definition. This clones the scope chain and creates a new empty HashMap on each iteration. For a registry with ~21 commands, this is not a real performance concern, but it is worth noting as the registry grows.\n\nAdditionally, `active_handle` is awaited once and then the Arc is cloned via `ref handle` inside the closure, which is correct.\n\nSuggestion: Hoist the args HashMap and build a single reusable context template that only swaps the command_id. Low priority given current scale. #review-finding #warning