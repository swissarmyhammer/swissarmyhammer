---
position_column: done
position_ordinal: ffd580
title: 'WARNING: resolveCommand uses id-only key, mismatched with collectAvailableCommands target-aware key'
---
command-scope.tsx: `resolveCommand()` looks up by `id` alone (line 106: `current.commands.get(id)`), but `collectAvailableCommands()` uses target-aware shadow keys (`id + ':' + (target ?? '')`). This means `resolveCommand('entity.inspect')` returns the first match in the chain regardless of target, while `collectAvailableCommands` would accumulate multiple entity.inspect commands with different targets. The KeybindingHandler in app-shell.tsx uses `resolveCommand` for focused-scope dispatch, meaning keyboard shortcuts for `entity.inspect` will always hit the innermost scope's version, even when multiple target-differentiated inspect commands exist. This may be intentional (keyboard shortcut acts on nearest entity) but should be documented since it differs from the context menu behavior. #review-finding