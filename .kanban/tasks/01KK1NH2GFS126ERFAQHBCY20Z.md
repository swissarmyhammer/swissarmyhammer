---
position_column: done
position_ordinal: n2
title: 'W5: CommandDef does not reject unknown YAML fields (deny_unknown_fields missing)'
---
In `swissarmyhammer-commands/src/types.rs`, `CommandDef` intentionally accepts unknown YAML fields (verified by the `unknown_fields_in_yaml_ignored` test). While forward-compatible, this silently swallows typos in command YAML files. A user writing `undoabel: true` instead of `undoable: true` gets no feedback. Consider at least logging a warning when unknown fields are present (like the registry already does for invalid YAML), or adding a validation pass.\n\nFile: swissarmyhammer-commands/src/types.rs:42-58, registry.rs test at line 349 #review-finding #warning