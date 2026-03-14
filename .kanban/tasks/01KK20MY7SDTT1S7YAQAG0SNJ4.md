---
position_column: done
position_ordinal: ffcf80
title: entity.delete available() checks target field but YAML uses moniker arg
---
swissarmyhammer-kanban/src/commands/entity_commands.rs:47-49\n\nDeleteEntityCmd::available() checks `ctx.target.is_some()` but the YAML definition in entity.yaml defines:\n```yaml\nparams:\n  - name: moniker\n    from: args\n```\nThis means the frontend may pass the moniker as an arg rather than as the target field. The availability check on `ctx.target` would return false when the moniker is in args instead. Either the available() check should also look at args['moniker'], or the YAML param should be `from: target`.\n\nThe execute() method also reads from `ctx.target` (line 54-57), which is consistent with available() but inconsistent with the YAML.\n\nSuggestion: Change the YAML param to `from: target` to match the implementation, or update available()/execute() to also check `ctx.arg('moniker')`. #review-finding #warning