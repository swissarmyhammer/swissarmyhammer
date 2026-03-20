---
position_column: done
position_ordinal: ffff9f80
title: 'W7: tag.update and attachment.delete YAML defs have no Command trait implementation'
---
The builtin entity.yaml defines `tag.update` (line 79) and `attachment.delete` (line 98) command definitions, but `register_commands()` in swissarmyhammer-kanban/src/commands/mod.rs does not register implementations for these IDs. The registry test asserts 21 YAML commands but only 19 trait implementations. If `dispatch_command` receives either of these IDs, it will fail with \"No implementation for command\" at runtime.\n\nEither add stub implementations or remove these definitions from the builtin YAML. Currently the command count mismatch (21 defs vs 19 impls) is a silent gap.\n\nFile: swissarmyhammer-commands/builtin/commands/entity.yaml:79-106, swissarmyhammer-kanban/src/commands/mod.rs:20-88 #review-finding #warning